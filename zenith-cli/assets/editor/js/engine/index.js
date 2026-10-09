// The Engine interface: the one way the page reaches the editor engine.
//
// The page never calls `fetch` or `EventSource` itself. A host supplies an
// Engine. `zenith edit` supplies `HttpEngine`. A static host supplies
// `WasmEngine` (the engine in a Worker, no server) with the same shape, and
// the page needs no change. `engine.kind` is `"http"` or `"wasm"`.
//
//   engine.clientId: string
//       The id this page sends with every command. Events that carry the
//       same `client` came from this page.
//
//   engine.run(command, params = {}, { version } = {}) -> Promise<Envelope>
//       Run one engine or host command. `version` is required by commands
//       that change the text (see `commands.list`, `needs_version`).
//       Envelope = { ok, command, version, dirty, work,
//                    result?,                       when ok
//                    error?: { code, message, diagnostics?, offers? },
//                    image?: { url, sha256, width, height, page } }
//       An engine error resolves with `ok: false`. The promise rejects only
//       with `EngineUnavailable` when the engine cannot be reached.
//
//       `commands.batch {steps}` runs several engine commands in one call
//       (the page sends a keystroke as one batch). `splitBatch(env, steps)`
//       gives one envelope per step, the image on the step that rendered.
//
//   engine.save({ version, overwrite }) -> Promise<Envelope>
//       `file.save`. Same envelope and error rules as `run`.
//
//   engine.state({ text }) -> Promise<Summary>
//       Summary = { ok, path, root, version, dirty, valid, stale, conflict,
//                   missing, page, selection, mtime_ms, saved_sha256,
//                   text?, disk_text? }   (text with `text: true`)
//       Rejects with `EngineUnavailable`.
//
//   engine.image(image) -> Promise<Blob>
//       The PNG named by `envelope.image`. Rejects with `EngineUnavailable`
//       or `EngineError` (code `edit.image_expired` when the render left the
//       cache).
//
//   engine.subscribe({ open, event, lost, stopped }) -> () => void
//       Live events. `open()` runs on every (re)connect: the page reads the
//       state with text after it, since events do not resume. `event(name,
//       data)` runs for `state`, `session`, `external_change`, `saved`,
//       `shutdown`. `lost(retryMs)` runs when the stream drops and names the
//       wait before the next attempt. `stopped()` runs after `shutdown`.
//       Returns the function that ends the subscription.

export { HttpEngine } from "./http.js";
export { WasmEngine } from "./wasm.js";
export { EngineError, EngineUnavailable } from "./errors.js";
export { batchDiagnostics, splitBatch } from "./batch.js";
