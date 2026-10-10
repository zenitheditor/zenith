//! `zenith edit <file.zen>`: a local HTTP host for the browser editor and
//! the agent control surface. Wiring and the protocol reference.
//!
//! The server holds one `zenith-editor` session for the document. The
//! page and agents share it, so an edit from either side reaches the
//! other through the `session` event. The engine command protocol is the
//! `zenith_editor` crate rustdoc. This page covers the HTTP layer.
//!
//! # Transport
//!
//! - HTTP/1.1 over `std::net`, one request per connection
//!   (`Connection: close`). Bodies use `Content-Length`. Chunked bodies get
//!   501.
//! - Limits: head 16 KiB and 64 headers, read within 5 s. Body 16 MiB,
//!   read within 60 s. A client that sends nothing for 5 s is dropped. Past
//!   a limit the reply is 431, 413, or 408. Malformed input gets 400.
//! - The head is read and checked (`Host`, `Origin`, token) before the
//!   body. A request without the token never makes the server read or hold
//!   a body.
//! - One thread per connection, at most 64 open connections and 16 from
//!   one non-loopback peer. Past a cap the accept loop answers 503
//!   `edit.busy` with one non-blocking write, so idle or slow clients never
//!   stall it. One event thread serves every event stream.
//!
//! # Security
//!
//! - **Bind.** Loopback by default. A non-loopback `--host` needs
//!   `--allow-remote` and prints a warning: the traffic is plain HTTP, so
//!   the token crosses the network in clear. Use a TLS reverse proxy or an
//!   SSH tunnel instead.
//! - **Token.** 32 bytes from the OS RNG, 64 hex characters, new per run,
//!   compared in constant time. Every `/api/*` request sends
//!   `Authorization: Bearer <token>`. The server accepts the token nowhere
//!   else: no cookie (cookies ignore the port, so every service on
//!   127.0.0.1 would receive it) and no query (it would land in logs and
//!   history).
//! - **Page bootstrap.** The page URL is `http://127.0.0.1:<port>/#token=<t>`.
//!   A fragment is never sent to a server and never in a `Referer`. The page
//!   reads it, drops it from the address bar and history
//!   (`history.replaceState`), keeps it in memory, and copies it to this
//!   tab's `sessionStorage` so a reload works. It sends it as the bearer
//!   header on every `fetch`, and reads `/api/events` with `fetch` (an
//!   `EventSource` cannot send a header). The page files (`/`, `/js/…`) are
//!   the public editor bundle and need no token.
//! - **Browser launch.** The opener command line (`xdg-open`, `open`,
//!   `rundll32`) is readable by other local users, so it never holds
//!   the URL. The server writes a redirect page, mode `0600` with a random
//!   name, into `$XDG_RUNTIME_DIR` (else the temporary directory) and opens
//!   that file. The server removes it after the first authenticated request,
//!   or at exit. A sandboxed browser that cannot read the file shows an
//!   error: open the printed URL instead.
//! - **Host.** Must be `localhost`, an IP literal, or the `--host` name,
//!   with the bound port. A DNS-rebinding page carries its own name and
//!   gets 403 `edit.bad_host`.
//! - **Origin.** When present, it must be `http://<Host>`. A cross-site
//!   `Sec-Fetch-Site` is refused too (403 `edit.bad_origin`). No CORS
//!   header is ever sent. POST bodies must be `application/json` (415
//!   otherwise), which a foreign page cannot send without a preflight.
//! - **Files.** Static routes serve only the files embedded from
//!   `assets/editor/`, by exact path. There is no directory listing. The
//!   document and its imports, assets, fonts, and text sources are read
//!   only through the engine, confined to the document's directory (or
//!   `--root`). `..` and symlinks that leave it read as errors. Config
//!   files (the nearest `.zenith.kdl` and the global config) and system
//!   fonts are read as every other CLI command reads them.
//! - **Writes.** Saves go through the CLI write path: history and
//!   `doc-id` stamping as `tx --apply`, then an atomic sibling-file
//!   rename after the bytes are synced to disk. A symlinked document keeps
//!   its link. A read-only document opens with a warning, reports
//!   `readonly: true`, and a save fails with `edit.readonly`.
//! - Every page response carries a strict Content-Security-Policy,
//!   `X-Frame-Options: DENY`, and `nosniff`. The page itself is
//!   `Cache-Control: no-store`.
//!
//! # Stop signals
//!
//! Ctrl-C, `SIGTERM`, and `SIGHUP` stop a clean session at once (exit 0).
//! With unsaved edits the first signal stops nothing: the terminal names
//! the file and every page gets a `stopping` event and a banner with Save.
//! A second signal drops the edits and exits with 130. A save (any return
//! to a clean session) clears the warning.
//!
//! # Routes
//!
//! | Method | Path | Body | Reply |
//! |---|---|---|---|
//! | GET, HEAD | `/`, `/<asset>` | — | embedded page file (no token needed) |
//! | POST | `/api/cmd` | `{command, params?, version?, diff?}` | envelope |
//! | POST | `/api/save` | `{version?, overwrite?, diff?}` or empty | envelope of `file.save` |
//! | GET | `/api/state[?text=1]` | — | `{ok, path, root, version, dirty, valid, stale, conflict, missing, readonly, page, selection, mtime_ms, saved_sha256, text?, disk_text?}` |
//! | GET | `/api/events` | — | `text/event-stream` |
//! | GET | `/api/image/<sha256>.png` | — | PNG, `Cache-Control: immutable` |
//! | POST | `/api/shutdown` | `{force?}` or empty | `{ok, stopping, dirty}`. 409 `edit.unsaved` while dirty without `force` |
//!
//! Optional request header `X-Zenith-Client: <id>` (1–64 of
//! `[A-Za-z0-9_-]`) names the sender in `session` events.
//!
//! **Envelope** (always HTTP 200 once the command ran):
//!
//! ```text
//! {ok, command, version, dirty, work,
//!  result?          the command reply (ok: true)
//!  error?           {code, message, diagnostics?, offers?} (ok: false)
//!  image?           {url, sha256, width, height, page,
//!                    rect?, scale?, device_size?, page_size?}
//!                    with a viewport render (see zenith-editor "Viewport render")
//!  diff?            unified diff of the text change (with diff: true)}
//! ```
//!
//! HTTP-level errors (auth, limits, bad JSON) use a 4xx/5xx status and
//! `{ok: false, error: {code, message}}`.
//!
//! **PNG delivery.** `doc.render` and `gesture.preview` put the PNG in the
//! image cache (last 32 renders) and return its URL. The page fetches it
//! with `fetch(url)` and shows it through `URL.createObjectURL(blob)`. A
//! separate route avoids base64 (a third larger, and parsed as JSON on
//! every preview), and the content-addressed URL lets the browser reuse a
//! render it already has.
//!
//! **Batches.** `commands.batch {steps}` runs several engine commands in
//! one request (see zenith-editor "Batches"). The page sends each keystroke
//! as one batch. `image` is the PNG of step `result.image_step`, and the
//! `session` event names `commands.batch`. Host commands cannot be steps.
//!
//! # Host commands
//!
//! `/api/cmd` also takes three commands the engine does not have (it does
//! no I/O). `commands.list` lists them after the engine commands.
//!
//! | id | params | reply |
//! |---|---|---|
//! | `file.save` | `{overwrite?=false}` + optional version | `{saved, path, version, bytes, sha256, mtime_ms, stamped, delta?, warning?, dirty}` |
//! | `file.reload` | `{}` + version | `{reloaded, changed, version, delta?, valid, stale, diagnostics}` |
//! | `file.state` | `{}` | the `/api/state` summary |
//!
//! A first save stamps a `doc-id` into the text (`stamped: true`). The
//! reply `delta` and a `session` event carry that change.
//!
//! # Events
//!
//! `GET /api/events` streams `id:`, `event:`, `data: <json>` records. At
//! most 8 streams (503 `edit.too_many_streams`). A `: ping` comment every
//! 15 s drops dead clients. Ids do not resume: after a reconnect, read
//! `/api/state?text=1`.
//!
//! | event | data |
//! |---|---|
//! | `state` | the summary, first record on every stream, and again when `readonly` changes |
//! | `session` | `{client, command, base_version, version, delta?, selection, page, valid, stale, dirty, conflict}` |
//! | `external_change` | `{path, deleted, mtime_ms, text, conflict, reloaded, base_version, version, delta?, valid, stale, dirty}` |
//! | `saved` | `{client, path, version, sha256, mtime_ms, stamped}` |
//! | `stopping` | `{reason: "signal", path, version, dirty: true}`: a stop signal met unsaved edits |
//! | `shutdown` | `{}`, then the stream closes |
//!
//! A removed file sends `external_change` with `deleted: true` and no
//! text. A later save writes it again. A file that holds the saved text
//! again (restored, or `git checkout` during a conflict) sends
//! `external_change` with `conflict: false` and `deleted: false`.
//!
//! **Applying a delta.** `delta` is `{start, end, from, to, insert}`
//! (`from`/`to` in UTF-16 units) against the text at `base_version`.
//! Ignore a `session` event whose `client` is your own id. When your
//! version equals `base_version`, apply the delta and take `version`.
//! Otherwise read `/api/state?text=1` and replace the buffer.
//!
//! # Conflict policy
//!
//! The server keeps the text last read from or written to disk. The
//! session is dirty while its text differs. The disk is polled every
//! 250 ms.
//!
//! - Disk changed, session clean: the session takes the disk text as one
//!   undoable entry. `external_change` has `reloaded: true` and a delta.
//! - Disk changed, session dirty: the session keeps its text and is in
//!   conflict. `external_change` has `conflict: true` and the disk `text`.
//!   `file.save` fails with `edit.conflict`, offering `file.reload` (take
//!   the disk text) and `file.save {overwrite: true}` (replace it).
//! - A save that finds an unseen disk change applies the same rule first.
//! - Disk back at the saved text: the conflict ends. The session keeps
//!   its edits and saves again.
//!
//! Neither side is ever dropped silently. Stale-version rules of the
//! engine stay in force for every text-changing command.

mod browser;
pub(crate) mod client;
pub(crate) mod doc;
mod http;
mod server;

#[cfg(feature = "http")]
pub(crate) use http::{HttpError, HttpRequest, Limits, Method, Response, read_body, read_head};
pub use server::{EditOptions, EditServer, Interrupt, StopHandle};
#[cfg(feature = "http")]
pub(crate) use server::{Token, split_host};
