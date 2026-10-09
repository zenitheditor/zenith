//! `zenith-editor-wasm`: one JSON request on stdin, one JSON response on stdout.

use std::io::{Read, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut input = String::new();
    let response = match std::io::stdin().read_to_string(&mut input) {
        Ok(_) => zenith_editor_wasm::handle_request(&input),
        Err(e) => zenith_editor_wasm::error_response(
            "request.stdin_read_failed",
            &format!("could not read the request from stdin: {e}; send one UTF-8 JSON object"),
        ),
    };
    let mut stdout = std::io::stdout().lock();
    match stdout
        .write_all(response.as_bytes())
        .and_then(|()| stdout.flush())
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
