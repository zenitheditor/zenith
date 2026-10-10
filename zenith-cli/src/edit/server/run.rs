//! [`EditServer`]: bind, start the threads, and stop them.

use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::json;
use zenith_editor::EditorError;

use super::events::Hub;
use super::guard::{Guard, Token};
use super::routes;
use super::shared::Shared;
use super::slots::Slots;
use crate::edit::browser::{self, LaunchFile};
use crate::edit::doc::{DocState, Event, Target};
use crate::edit::http::{HttpError, Response};

/// Open connections, each served by its own thread.
const MAX_CONNECTIONS: usize = 64;
/// Open connections from one non-loopback peer.
const MAX_PER_PEER: usize = 16;
/// Longest wait at shutdown for requests in flight to finish.
const DRAIN: Duration = Duration::from_secs(10);
/// Pause after a failed `accept` (for example, out of file descriptors).
const ACCEPT_BACKOFF: Duration = Duration::from_millis(20);
/// Open event streams.
const MAX_STREAMS: usize = 8;
/// Keep-alive comment interval on event streams.
const PING: Duration = Duration::from_secs(15);
/// Disk poll interval.
const POLL: Duration = Duration::from_millis(250);

/// How to start `zenith edit`.
#[derive(Debug, Clone)]
pub struct EditOptions {
    /// The document.
    pub path: PathBuf,
    /// The directory project reads stay under. Default: the document's
    /// directory.
    pub root: Option<PathBuf>,
    /// The bind address: an IP address or `localhost`.
    pub host: String,
    /// The port. `0` picks a free one.
    pub port: u16,
    /// Allow a non-loopback `host`.
    pub allow_remote: bool,
}

/// A running editor server.
pub struct EditServer {
    shared: Arc<Shared>,
    threads: Vec<JoinHandle<()>>,
    url: String,
    /// The canonical document path, read at start: the document lock is
    /// held while the document opens.
    path: PathBuf,
    /// The read-only state at start.
    readonly: bool,
}

/// What a stop signal (Ctrl-C, `SIGTERM`) did. See
/// [`StopHandle::interrupt`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Interrupt {
    /// The session had no unsaved edits: shutdown started.
    Stopping,
    /// The session has unsaved edits. Every page got a `stopping` event.
    /// The server keeps running. The next signal discards the edits.
    Unsaved {
        /// The document.
        path: PathBuf,
        /// The session version.
        version: u64,
    },
    /// A second signal with unsaved edits: they are dropped and shutdown
    /// started.
    Discarded,
}

/// A handle that stops the server from another thread (a signal handler).
#[derive(Clone)]
pub struct StopHandle {
    shared: Arc<Shared>,
}

impl StopHandle {
    /// Handle one stop signal.
    ///
    /// - Clean session: start shutdown.
    /// - Unsaved edits, first signal: send `stopping` to every page and
    ///   keep running, so the edits can still be saved.
    /// - Unsaved edits, signal after a warning: start shutdown and drop
    ///   them.
    ///
    /// A save (or any return to a clean session) clears the warning, so a
    /// later signal with new edits warns again.
    #[must_use]
    pub fn interrupt(&self) -> Interrupt {
        let shared = &self.shared;
        let doc = shared.doc();
        if !doc.dirty() {
            drop(doc);
            shared.request_stop();
            return Interrupt::Stopping;
        }
        if shared.stop_armed.swap(true, Ordering::SeqCst) {
            drop(doc);
            shared.request_stop();
            return Interrupt::Discarded;
        }
        let path = doc.path().to_path_buf();
        let version = doc.session().version;
        shared.hub.send(vec![Event::new(
            "stopping",
            json!({
                "reason": "signal",
                "path": path,
                "version": version,
                "dirty": true,
            }),
        )]);
        Interrupt::Unsaved { path, version }
    }
}

impl EditServer {
    /// Open the document, bind, and start serving.
    ///
    /// # Errors
    ///
    /// - `edit.bad_host` when `host` is not an IP address or `localhost`.
    /// - `edit.remote_refused` for a non-loopback `host` without
    ///   `allow_remote`.
    /// - `edit.missing_file`, `edit.not_a_file`, `edit.bad_root`,
    ///   `edit.read_failed` from opening the document.
    /// - `edit.bind_failed` when the address cannot be bound.
    /// - `edit.start_failed` when a thread or the token cannot be made.
    pub fn start(options: &EditOptions) -> Result<EditServer, EditorError> {
        let ip = bind_ip(&options.host, options.allow_remote)?;
        let target = Target::resolve(&options.path, options.root.as_deref())?;
        // Read now, open in the engine on a thread: the URL prints before
        // the first validation of a large document. Requests wait on the
        // document lock, which the load holds until the document is open.
        let doc = DocState::read(target)?;
        let (path, readonly) = (doc.path().to_path_buf(), doc.readonly());
        let listener = TcpListener::bind(SocketAddr::new(ip, options.port)).map_err(|e| {
            EditorError::new(
                "edit.bind_failed",
                format!(
                    "cannot listen on {}:{}: {e}; pick another --port, or omit it to use a free one",
                    options.host, options.port
                ),
            )
        })?;
        let addr = listener.local_addr().map_err(start_failed)?;
        let token = Token::generate().map_err(|e| {
            EditorError::new("edit.start_failed", format!("cannot make a token: {e}"))
        })?;
        // The token rides in the fragment: a browser never sends it to a
        // server, and it is not in a `Referer`.
        let url = format!("http://{}/#token={}", url_host(addr), token.as_str());
        let guard = Guard::new(token, addr.port(), &options.host);
        let (hub, hub_thread) = Hub::start(MAX_STREAMS, PING).map_err(start_failed)?;
        let slots = Slots::new(MAX_CONNECTIONS, MAX_PER_PEER);
        let shared = Arc::new(Shared::new(doc, hub, guard, addr, slots));
        let mut threads = vec![hub_thread];
        // The load takes the document lock before any connection is
        // accepted, so no request sees the document before it is open.
        let (locked, held) = std::sync::mpsc::channel::<()>();
        let load_shared = Arc::clone(&shared);
        threads.push(spawn("zenith-edit-load", move || {
            load(&load_shared, &locked);
        })?);
        // A closed channel (the thread ended early) also ends the wait.
        let _ = held.recv();
        let watch_shared = Arc::clone(&shared);
        threads.push(spawn("zenith-edit-watch", move || watch(&watch_shared))?);
        let accept_shared = Arc::clone(&shared);
        threads.push(spawn("zenith-edit-accept", move || {
            accept(&accept_shared, &listener);
        })?);
        Ok(EditServer {
            shared,
            threads,
            url,
            path,
            readonly,
        })
    }

    /// The bound address.
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.shared.addr
    }

    /// The per-run token.
    #[must_use]
    pub fn token(&self) -> &str {
        self.shared.guard.token().as_str()
    }

    /// The page URL, with the token.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The canonical document path. It does not wait for the document to
    /// open.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    /// `true` when the document was read-only at start, so a save fails
    /// until it is writable. It does not wait for the document to open.
    #[must_use]
    pub fn readonly(&self) -> bool {
        self.readonly
    }

    /// Start shutdown. [`EditServer::wait`] returns once every thread ended.
    pub fn shutdown(&self) {
        self.shared.request_stop();
    }

    /// A handle that stops the server from another thread.
    #[must_use]
    pub fn stop_handle(&self) -> StopHandle {
        StopHandle {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Open the page in the default browser. The token stays off every
    /// command line: the opener gets a private redirect file (see
    /// `browser`), which the server removes after the first authenticated
    /// request, or at exit.
    ///
    /// # Errors
    ///
    /// The I/O error of the file or the opener.
    pub fn open_browser(&self) -> std::io::Result<()> {
        let file = LaunchFile::create(&self.url)?;
        let path = file.path().to_path_buf();
        self.shared.keep_launch(file);
        browser::open(&path).inspect_err(|_| self.shared.drop_launch())
    }

    /// Block until shutdown (`POST /api/shutdown`, a stop signal, or
    /// [`EditServer::shutdown`]) finished. Requests in flight get up to
    /// 10 s to end.
    pub fn wait(self) {
        for thread in self.threads {
            let _ = thread.join();
        }
        self.shared.slots.wait_idle(DRAIN);
        self.shared.drop_launch();
    }
}

/// Open the read document in the engine, holding the document lock from
/// before `locked` is signalled until the document is open. A failed open
/// keeps the read text, which the next command validates.
fn load(shared: &Arc<Shared>, locked: &std::sync::mpsc::Sender<()>) {
    let mut doc = shared.doc();
    let _ = locked.send(());
    if let Err(e) = doc.load() {
        eprintln!("warning[{}]: {}", e.code, e.message);
    }
}

/// Accept connections until shutdown. Each gets a thread of its own while a
/// slot is free. Past a cap, the client gets 503 from a non-blocking write,
/// so a slow client never stalls this loop.
fn accept(shared: &Arc<Shared>, listener: &TcpListener) {
    for stream in listener.incoming() {
        if shared.stopping() {
            break;
        }
        let stream = match stream {
            Ok(stream) => stream,
            Err(_) => {
                std::thread::sleep(ACCEPT_BACKOFF);
                continue;
            }
        };
        let peer = stream
            .peer_addr()
            .map_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED), |a| a.ip());
        let Some(slot) = shared.slots.acquire(peer) else {
            busy(&stream);
            continue;
        };
        let conn = Arc::clone(shared);
        // A failed spawn drops the closure: the socket closes and the slot
        // returns.
        let _ = std::thread::Builder::new()
            .name("zenith-edit-conn".into())
            .spawn(move || {
                routes::serve(&conn, stream);
                drop(slot);
            });
    }
}

/// Answer 503 with one non-blocking write, then close.
fn busy(stream: &TcpStream) {
    let error = HttpError::new(
        503,
        "edit.busy",
        "the server has too many open connections; retry in a moment",
    );
    let mut bytes = Vec::new();
    if Response::error(&error).write_to(&mut bytes).is_ok() && stream.set_nonblocking(true).is_ok()
    {
        let mut writer = stream;
        let _ = writer.write(&bytes);
    }
}

/// The bind IP of `host`.
fn bind_ip(host: &str, allow_remote: bool) -> Result<IpAddr, EditorError> {
    let ip = if host.eq_ignore_ascii_case("localhost") {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    } else {
        host.parse::<IpAddr>().map_err(|_| {
            EditorError::new(
                "edit.bad_host",
                format!("--host '{host}' is not an IP address or 'localhost'; pass e.g. 127.0.0.1"),
            )
        })?
    };
    if !ip.is_loopback() && !allow_remote {
        return Err(EditorError::new(
            "edit.remote_refused",
            format!(
                "--host '{host}' is not a loopback address; anyone who reaches it can try the \
                 server. Add --allow-remote to serve on it anyway"
            ),
        ));
    }
    Ok(ip)
}

/// The host part of the page URL for `addr`.
fn url_host(addr: SocketAddr) -> String {
    match addr {
        SocketAddr::V4(a) if a.ip().is_unspecified() => format!("127.0.0.1:{}", a.port()),
        SocketAddr::V6(a) if a.ip().is_unspecified() => format!("[::1]:{}", a.port()),
        SocketAddr::V4(a) => a.to_string(),
        SocketAddr::V6(a) => format!("[{}]:{}", a.ip(), a.port()),
    }
}

/// Poll the disk until shutdown.
fn watch(shared: &Shared) {
    while !shared.stopping() {
        std::thread::sleep(POLL);
        let mut doc = shared.doc();
        let events = doc.poll_disk(true);
        shared.note_clean(doc.dirty());
        shared.hub.send(events);
    }
}

fn spawn(name: &str, f: impl FnOnce() + Send + 'static) -> Result<JoinHandle<()>, EditorError> {
    std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(f)
        .map_err(start_failed)
}

fn start_failed(e: std::io::Error) -> EditorError {
    EditorError::new("edit.start_failed", format!("cannot start the server: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_loopback_needs_allow_remote() {
        assert_eq!(
            bind_ip("0.0.0.0", false).expect_err("remote").code,
            "edit.remote_refused"
        );
        assert!(bind_ip("0.0.0.0", true).is_ok());
        assert!(bind_ip("localhost", false).is_ok());
        assert!(bind_ip("::1", false).is_ok());
        assert_eq!(
            bind_ip("example.com", true).expect_err("name").code,
            "edit.bad_host"
        );
    }

    #[test]
    fn url_hosts_are_dialable() {
        assert_eq!(url_host(SocketAddr::from(([0, 0, 0, 0], 5))), "127.0.0.1:5");
        assert_eq!(url_host("[::1]:7".parse().expect("addr")), "[::1]:7");
    }
}
