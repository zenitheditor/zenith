//! [`EditServer`]: bind, start the threads, and stop them.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TrySendError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use zenith_editor::EditorError;

use super::events::Hub;
use super::guard::{Guard, Token};
use super::routes;
use super::shared::Shared;
use crate::edit::doc::{DocState, Target};
use crate::edit::http::{HttpError, Response};

/// Request worker threads.
const WORKERS: usize = 4;
/// Accepted connections waiting for a worker.
const QUEUE: usize = 64;
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
        let (doc, _) = DocState::open(target)?;
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
        let url = format!("http://{}/?token={}", url_host(addr), token.as_str());
        let guard = Guard::new(token, addr.port(), &options.host);
        let (hub, hub_thread) = Hub::start(MAX_STREAMS, PING).map_err(start_failed)?;
        let shared = Arc::new(Shared::new(doc, hub, guard, addr));
        let mut threads = vec![hub_thread];
        let (tx, rx) = mpsc::sync_channel::<TcpStream>(QUEUE);
        let rx = Arc::new(Mutex::new(rx));
        for i in 0..WORKERS {
            let shared = Arc::clone(&shared);
            let rx = Arc::clone(&rx);
            threads.push(spawn(&format!("zenith-edit-worker-{i}"), move || {
                worker(&shared, &rx);
            })?);
        }
        let watch_shared = Arc::clone(&shared);
        threads.push(spawn("zenith-edit-watch", move || watch(&watch_shared))?);
        let accept_shared = Arc::clone(&shared);
        threads.push(spawn("zenith-edit-accept", move || {
            for stream in listener.incoming() {
                if accept_shared.stopping() {
                    break;
                }
                let Ok(stream) = stream else { continue };
                match tx.try_send(stream) {
                    Ok(()) => {}
                    Err(TrySendError::Full(mut stream)) => {
                        let busy = HttpError::new(
                            503,
                            "edit.busy",
                            "the server is busy; retry in a moment",
                        );
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
                        let _ = Response::error(&busy).write_to(&mut stream);
                    }
                    Err(TrySendError::Disconnected(_)) => break,
                }
            }
        })?);
        Ok(EditServer {
            shared,
            threads,
            url,
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

    /// The canonical document path.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.shared.doc().path().to_path_buf()
    }

    /// Start shutdown. [`EditServer::wait`] returns once every thread ended.
    pub fn shutdown(&self) {
        self.shared.request_stop();
    }

    /// Block until shutdown (`POST /api/shutdown` or
    /// [`EditServer::shutdown`]) finished.
    pub fn wait(self) {
        for thread in self.threads {
            let _ = thread.join();
        }
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

/// Pull connections from the queue until it closes.
fn worker(shared: &Shared, rx: &Mutex<Receiver<TcpStream>>) {
    loop {
        let next = rx.lock().unwrap_or_else(PoisonError::into_inner).recv();
        match next {
            Ok(stream) => routes::serve(shared, stream),
            Err(_) => break,
        }
    }
}

/// Poll the disk until shutdown.
fn watch(shared: &Shared) {
    while !shared.stopping() {
        std::thread::sleep(POLL);
        let mut doc = shared.doc();
        let events = doc.poll_disk(true);
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
