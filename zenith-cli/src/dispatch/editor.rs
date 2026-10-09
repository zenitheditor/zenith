//! Dispatch for `zenith edit`: start the server, print its URL, open the
//! browser, and block until shutdown.

use std::process::ExitCode;

use serde_json::json;

use crate::cli::EditArgs;
use crate::edit::{EditOptions, EditServer, open_browser};
use crate::report::CliError;

use super::output::warn;

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
            "serving on non-loopback address {addr}; anyone who reaches it can try the token, and \
             the token grants read and write access under the editor root"
        ));
    }
    let path = server.path();
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
        && let Err(e) = open_browser(server.url())
    {
        warn(format!("cannot open a browser ({e}); open the URL above"));
    }
    server.wait();
    eprintln!("zenith edit: stopped");
    ExitCode::SUCCESS
}
