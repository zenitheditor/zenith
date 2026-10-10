//! Dispatch for `zenith edit`: start the server, print its URL, open the
//! browser, handle stop signals, and block until shutdown.

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::json;

use crate::cli::EditArgs;
use crate::edit::{EditOptions, EditServer, Interrupt, StopHandle};
use crate::report::CliError;

use super::output::warn;

/// Exit code after a second stop signal dropped unsaved edits (128 +
/// SIGINT, as a shell reports an interrupted command).
const DISCARDED_EXIT: u8 = 130;

pub(super) fn dispatch_edit(args: EditArgs) -> ExitCode {
    let json = args.json;
    let options = EditOptions {
        path: args.path,
        root: args.root,
        host: args.host,
        port: args.port.unwrap_or(0),
        allow_remote: args.allow_remote,
    };
    let server = match EditServer::start(&options) {
        Ok(server) => server,
        Err(e) => {
            return CliError::new(
                e.code.clone(),
                format!("error[{}]: {}", e.code, e.message),
                2,
            )
            .emit(json);
        }
    };
    let addr = server.addr();
    if !addr.ip().is_loopback() {
        warn(format!(
            "serving on non-loopback address {addr} over plain HTTP: the token and the document \
             cross the network unencrypted, and anyone who reaches the port can try the token. \
             Put a TLS reverse proxy in front, or reach a loopback server through an SSH tunnel \
             (ssh -L {port}:127.0.0.1:{port} <host>)",
            port = addr.port()
        ));
    }
    let discarded = Arc::new(AtomicBool::new(false));
    if let Err(e) = on_stop_signal(server.stop_handle(), Arc::clone(&discarded)) {
        warn(format!(
            "cannot handle Ctrl-C ({e}); stop the server with POST /api/shutdown"
        ));
    }
    let path = server.path();
    if server.readonly() {
        warn(format!(
            "'{}' is read-only: the editor opens it, but Save fails until the file is \
             writable. Make it writable first, or copy the text out of the editor",
            path.display()
        ));
    }
    if json {
        println!(
            "{}",
            json!({
                "schema": "zenith-edit-v1",
                "url": server.url(),
                "host": addr.ip().to_string(),
                "port": addr.port(),
                "token": server.token(),
                "path": path,
            })
        );
    } else {
        println!("zenith edit: serving '{}'", path.display());
        println!("  {}", server.url());
        println!("Stop with Ctrl-C or POST /api/shutdown.");
    }
    if !args.no_open
        && let Err(e) = server.open_browser()
    {
        warn(format!("cannot open a browser ({e}); open the URL above"));
    }
    server.wait();
    if discarded.load(Ordering::SeqCst) {
        eprintln!("zenith edit: stopped; unsaved edits were discarded");
        return ExitCode::from(DISCARDED_EXIT);
    }
    eprintln!("zenith edit: stopped");
    ExitCode::SUCCESS
}

/// Run `stop.interrupt()` on every Ctrl-C, `SIGTERM`, or `SIGHUP`, and
/// print what it did. `discarded` turns true when a signal dropped unsaved
/// edits.
fn on_stop_signal(stop: StopHandle, discarded: Arc<AtomicBool>) -> Result<(), ctrlc::Error> {
    ctrlc::set_handler(move || match stop.interrupt() {
        Interrupt::Stopping => {}
        Interrupt::Unsaved { path, version } => {
            eprintln!(
                "zenith edit: '{}' has unsaved edits (session version {version}). Save them in \
                 the editor page, or press Ctrl-C again to discard them and stop.",
                path.display()
            );
        }
        Interrupt::Discarded => discarded.store(true, Ordering::SeqCst),
    })
}
