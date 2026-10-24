//! `zenith-editor-wasm`: one request on stdin, one JSON response on stdout.
//!
//! Stdin holds the JSON envelope, then any font bytes it lists (see
//! `zenith_editor_wasm::handle_request`).

use std::io::{Read, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut input = Vec::new();
    let response = match std::io::stdin().read_to_end(&mut input) {
        Ok(_) => zenith_editor_wasm::handle_request(&input),
        Err(e) => zenith_editor_wasm::error_response(
            "request.stdin_read_failed",
            &format!(
                "could not read the request from stdin: {e}; send one UTF-8 JSON envelope, then any font bytes it lists"
            ),
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
