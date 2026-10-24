//! The Zenith engine behind a one-request JSON protocol.
//!
//! The `zenith-editor-wasm` binary reads one request object from stdin and
//! writes one response object to stdout. Built for `wasm32-wasip1`, it runs in
//! a browser Worker under a small WASI shim, so the crate needs no `unsafe`
//! and no FFI. Every function here is pure and runs natively in tests.
//!
//! Request: `{"method": "ping" | "diagnose" | "fonts" | "render" | "editor", "params": {...}}`.
//! Response: `{"ok": true, "result": ...}` or `{"ok": false, "error": ...}`.
//!
//! `diagnose` and `render` run the `zenith-pipeline` entry points the CLI
//! runs, over an in-memory project built from the request's `files` map
//! (path → base64). Config policy, composition imports, assets, project
//! fonts, text sources, and data files therefore resolve as on the CLI.
//!
//! `editor` runs one `zenith-editor` command: `{session?, command: {command,
//! params?, version?}, files?, ...}` returns `{session, result, work,
//! png_base64?}`. The page holds the session between calls; the module
//! keeps no state. `commands.batch` runs several commands in one call, so
//! the session and the project decode once per keystroke, not per command.
//!
//! The module embeds only Noto Sans Regular and Bold. `fonts` lists the other
//! bundled and fallback faces a document needs. The page fetches those font
//! files and sends their raw bytes after the envelope: the envelope lists
//! `"fonts": [{"file", "size"}]`, then one NUL byte, then each file's bytes
//! in list order. Every method reads them. Raw bytes skip base64 and JSON,
//! so a 5.6 MB face costs one copy per request, not a decode.

mod dispatch;
mod framing;
mod methods;
mod protocol;

pub use dispatch::{error_response, handle_request};
