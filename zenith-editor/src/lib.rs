//! The Zenith editor engine: one stateless command registry behind every
//! editing surface.
//!
//! The browser page (through `zenith-editor-wasm`), the native
//! `zenith edit` server, the MCP `editor.*` tools, and tests all call
//! [`execute`]`(env, session, request)`. The UI holds no logic: it renders
//! what the engine returns and sends commands back.
//!
//! # Protocol reference
//!
//! ## Model
//!
//! - **Stateless.** The engine keeps nothing between calls. The caller
//!   holds the [`Session`] and sends it with each [`Request`]; the
//!   [`Outcome`] carries the next session. On an error the session comes
//!   back unchanged.
//! - **The code pane owns the text.** The page sends `buffer.set` after a
//!   typing debounce. The engine never rewrites text the user is typing;
//!   engine edits return a minimal `delta` the page applies as one
//!   transaction.
//! - **Versions.** [`Session::version`] counts text changes. Every command
//!   that changes the text needs `version` in the request; any request
//!   that carries a `version` other than the session's is rejected with
//!   `editor.stale_version`, so a buffer or gesture made against an older
//!   text never lands.
//! - **Stale render.** While the text has an Error diagnostic, the session
//!   keeps the last valid text and render, hit, outline, and handle
//!   commands use it with `stale: true`. Edits (gestures, `tx.apply`,
//!   `node.*`) are disabled with `editor.buffer_invalid`: the transaction
//!   engine rejects documents with errors.
//! - **History.** Undo and redo hold text deltas with their inverses, never
//!   snapshots (default 200 entries and 1 MiB, see [`HistoryLimit`]).
//!   Typing and engine edits share one history; contiguous `buffer.set`
//!   bursts coalesce until a line break or 1 KiB.
//! - **Work.** [`Outcome::work`] counts parses, validations, transaction
//!   runs, patches, compiles, and rasters: compare commands by it, not by
//!   wall-clock time. One call parses each text once, however many steps
//!   use it.
//! - **Batches.** `commands.batch` runs several commands in one call (see
//!   Batches). The page sends each keystroke as one batch.
//!
//! ## Session (JSON)
//!
//! ```text
//! {version, text, valid, last_valid_text?, page, selection: [id],
//!  viewport: {zoom, pan_x, pan_y},
//!  history: {undo: [entry], redo: [entry], limit: {entries, bytes}}}
//! entry = {label, typing, forward: delta, inverse: delta,
//!          selection_before: [id], selection_after: [id]}
//! delta = {start, end, insert}            byte offsets
//! ```
//!
//! ## Request and result (JSON)
//!
//! ```text
//! request = {command, params?, version?}
//! result  = command reply, or error {code, message, diagnostics?, offers?}
//! offer   = {id, label, command, params}  send it as-is to go ahead
//! diagnostic = {code, severity, message, subject_id?, import?,
//!               start?, end?, line?, col?}
//! ```
//!
//! Every text-changing command except `buffer.set` replies with the edit
//! shape:
//!
//! ```text
//! {changed, version, delta?: {start, end, from, to, insert}, reformatted,
//!  removed_comments: [string], ops: [op], selection: [id], valid, stale,
//!  diagnostics: [diagnostic], tx_diagnostics: [diagnostic],
//!  notes: [diagnostic]}
//! ```
//!
//! `start`/`end` are bytes, `from`/`to` UTF-16 code units (CodeMirror),
//! both in the text before the change. `reformatted` means the patcher fell
//! back to canonical text and comments are gone. `removed_comments` lists
//! every comment the change dropped (`//`, `/* … */`, and `/-`), each as
//! `line N: <comment>`, with `(in: <line>)` when it shares its line with
//! other text. A removed node takes the comments directly above it and on
//! its lines.
//!
//! ## Commands
//!
//! | id | params | reply |
//! |---|---|---|
//! | `doc.open` | `{text}` | `{version, valid, stale, page_count?, diagnostics}` |
//! | `buffer.set` | `{text, coalesce?=true}` + version | `{changed, version, valid, stale, selection, diagnostics}` |
//! | `doc.diagnose` | `{}` | `{valid, exit_code, stale, diagnostics}` |
//! | `doc.render` | `{page?, scale?, viewport?}` | `{page, page_count, width, height, scale, sha256, stale, diagnostics, rect?, device_size?, page_size?, view?}` + PNG |
//! | `doc.outline` | `{}` | `{stale, pages: [{page, id, name?, master?, w, h, children}], masters}` |
//! | `node.inspect` | `{id?}` | attributes, box, span, lock, style, `edit` (the `node.set` fields and their values) |
//! | `select.hit` | `{x, y, page?, tolerance?, extend?, select?}` | `{page, stale, hits: [{id, raw_id, kind, name?, locked, master?, via?}], selection}` |
//! | `select.set` | `{ids}` | `{selection}` |
//! | `select.marquee` | `{x, y, w, h, page?, contain?, extend?, select?}` | `{page, stale, hits: [{id, kind, name?, master?}], selection}` |
//! | `select.at_offset` | `{offset, select?=true}` | `{parsed, id?, kind?, page?, master?, selection}` |
//! | `node.handles` | `{id?, ids?, rotate_offset?=24}` | `{corners, center, angle, handles: [{id, role, x, y, enabled, reason?}], disabled, master?, …}`; with `ids`: the selection box, `members` |
//! | `gesture.preview` | gesture + `{scale?, render?=true, viewport?}` | `{node \| nodes, page, ops, corners, members?, notes, snap?, width?, height?, sha256?, rect?, device_size?, page_size?, view?}` + PNG |
//! | `gesture.commit` | gesture + version | edit |
//! | `tx.apply` | `{ops, permissions?, label?, select?}` + version | edit |
//! | `node.set` | `{id?, x?, y?, w?, h?, rotate?, opacity?, fill?, stroke?, stroke_width?, radius?, font_family?, font_size?, font_weight?, align?, text?, spans?, visible?, locked?}` + flags + version | edit |
//! | `node.remove` | `{ids?}` + version | edit |
//! | `node.duplicate` | `{id?, ids?, new_id?, dx?, dy?}` + version | edit |
//! | `node.reorder` | `{id?, to: forward\|backward\|front\|back}` + version | edit |
//! | `node.group` | `{ids?, group_id?}` + version | edit |
//! | `node.ungroup` | `{id?}` + version | edit |
//! | `history.undo` | `{}` + version | edit |
//! | `history.redo` | `{}` + version | edit |
//! | `doc.format` | `{}` + version | edit |
//! | `doc.tokens` | `{type?}` | `{stale, tokens: [{id, type, value}]}` |
//! | `fonts.required` | `{}` | `{faces: [{family, weight, style, file?}], embedded, complete}` |
//! | `view.set` | `{page?, zoom?, pan_x?, pan_y?}` | `{page, viewport}` |
//! | `commands.list` | `{}` | `{commands: [{id, label, params, result, mutates, needs_version, enabled, disabled?}]}` |
//! | `commands.batch` | `{steps: [request]}` | `{steps: [{command, ok, result?, error?, work?}], image_step?}` + PNG |
//!
//! `select.at_offset` maps a code-pane cursor to a node: `offset` is a
//! byte offset into the current text (convert from UTF-16 first), and the
//! reply names the innermost node with an id whose span holds it (end
//! included). A page that holds the offset becomes the session page. While
//! the text does not parse, the reply has `parsed: false` and the session
//! stays as it was.
//!
//! `node.set` is the inspector's write: `x`/`y`/`w`/`h` are authored px
//! targets (the values `node.inspect` lists under `edit`), mapped axis by
//! axis as a gesture is (see Gestures), with the same confirmation flags
//! except `reorder`. The token-backed fields (`fill`, `stroke`,
//! `stroke_width`, `radius`, `font_family`, `font_size`, `font_weight`)
//! take a token id, or `{value}` with a raw value (`#rrggbb` /
//! `#rrggbbaa`, px, a family name, a weight 100–900): Zenith binds visual
//! properties to tokens, so a raw value binds the first token (by id) that
//! holds it, else a new one the same transaction creates
//! (`color.custom.<hex>`, `size.<property>.<n>`, `font.<name>`,
//! `weight.<n>`). `null` removes `radius` or a font field (the style or
//! default applies). `align` sets a text's alignment; `text` replaces the
//! content of a `text` node with one plain span; `spans: [{index, text}]`
//! sets single spans of a `text` or `shape` node, their attributes kept;
//! `visible` / `locked` alone also work on a locked or hidden node. There
//! is no line height field: no layout code reads a style's `line-height`.
//!
//! `id?` / `ids?` default to the selection. The PNG of `doc.render` and
//! `gesture.preview` is [`Outcome::image`], not JSON.
//!
//! ## Batches
//!
//! `commands.batch {steps: [{command, params?, version?}]}` runs the steps
//! in order, each on the session the step before left, in one call.
//!
//! - **Same as alone.** A step runs exactly as the same request sent by
//!   itself: the same checks (each step carries its own `version`), the
//!   same reply, and on an error the session stays as the step found it.
//!   The reply lists `{command, ok: true, result, work}` or
//!   `{command, ok: false, error, work}` per step.
//! - **One parse.** The steps share one parse per text. A keystroke frame
//!   (`buffer.set`, `doc.render`, `doc.outline`, `doc.tokens`,
//!   `select.at_offset`) parses the new text once instead of five times,
//!   and a stateless host (the wasm module) decodes the session and the
//!   project files once.
//! - **Stop rule.** A failed step that carries `version` stops the batch:
//!   later steps reply `editor.skipped`. Other failed steps do not.
//! - **Limits.** At most 16 steps, at most one `doc.render` or
//!   `gesture.preview` (its PNG is the batch image and `image_step` its
//!   index), and no nested `commands.batch` (`editor.invalid_params`).
//!   Host commands (`file.*`) are not engine commands and cannot be steps.
//!
//! ## Viewport render
//!
//! `doc.render` (and `gesture.preview`) with `viewport: {x, y, w, h}` in page
//! px renders only that window of the page, at `scale` device px per page px.
//! The canvas sends the visible region plus a margin with
//! `scale = zoom × devicePixelRatio`, so the PNG maps 1:1 to device pixels.
//!
//! - **Snap.** The window snaps out to whole device px:
//!   `x0 = floor(x × scale)`, `x1 = ceil((x + w) × scale)`, the same for `y`,
//!   clamped to the page. The reply gives the result as `rect {x, y, w, h}`
//!   (device px), `device_size {w, h}` (the page in device px), `page_size
//!   {w, h}` (the page in page px, bleed included), and `view {x, y, w, h}`
//!   (page px, `rect / scale`). A 1 × 1 px window at the origin is a cheap
//!   way to learn the page size. Place the PNG at `rect.x / dpr` CSS px with
//!   CSS size `w / dpr` for a pixel-exact image.
//! - **Limits.** No fixed scale cap. The window is at most 8191 device px
//!   per side and 16 777 216 px in total (`render.region_too_large`). The
//!   page at `scale` is at most 1 048 576 device px per side
//!   (`render.scale_too_large`). A non-finite or empty window, or one off
//!   the page, is `render.invalid_viewport`. A scale that is not finite and
//!   `> 0` is `render.invalid_scale`.
//! - **Pixels.** While the whole page at `scale` fits those window limits,
//!   the PNG equals the same window of the whole-page render, byte for byte.
//!   On larger pages edge pixels of shapes that cross the window border can
//!   differ by a few anti-aliasing steps (no seam).
//! - Without `viewport` the reply and the PNG are the whole-page render,
//!   and `scale` is at most 4.
//!
//! ## Gestures
//!
//! Params: `{node?, nodes?, handle?, dx, dy, angle?, snap?, constrain?,
//! from_center?, snap_distance?}` plus the confirmation flags `detach`, `detach_anchor`,
//! `confirm_size`, `replace`, `absolute`, `reorder`. `dx`/`dy` are the
//! pointer delta in page px; the engine maps it into the node's authored
//! space through the inverse of its compiled `world` transform (and its
//! own `spin` for resize and point edits). `handle` is an id from
//! `node.handles`; absent means move.
//!
//! | node kind | move | resize grip | rotate | point handles |
//! |---|---|---|---|---|
//! | box kinds, instance | `nudge_geometry dx dy` | `nudge_geometry dx dy dw dh`, opposite grip fixed on the page | `set_geometry rotate` | — |
//! | line | `nudge_line_points` (all four) | — | — | `start`/`end`: `nudge_line_points` |
//! | polygon, polyline | `set_points` | `set_points` scaled into the new frame | `set_geometry rotate` | `p<i>`: `set_points` |
//! | path | `transform_path_anchors translate` | `transform_path_anchors scale` + `translate` | `set_geometry rotate` | `a<s>.<i>`: `move_path_anchor`; `.in`/`.out`: `move_path_handle` |
//! | connector | rejected `tx.derived_geometry` | — | — | — |
//!
//! Rejections come back as `editor.rejected` with `tx.*` diagnostics and
//! offers: `tx.token_bound` → `detach`; `tx.anchored` → `detach_anchor`
//! (`[detach_anchor, nudge_geometry]`); `tx.computed_size` (absent or
//! `hug`/`fill` size) → `set_size` (`set_geometry` measured px + delta);
//! `tx.value_unresolved` → `replace_value`; `tx.layout_managed` (moving an
//! in-flow child of a row/column/grid frame) → `reorder` (`reparent` to the
//! nearest flow slot) or `absolute` (`set_layout position=absolute` +
//! `set_geometry x y`). An edge-anchored node moves its `anchor-gap`
//! (`nudge_anchor_gap`); the cross axis is dropped with a note. An absent
//! `x`/`y` with no anchor is written as px. Locked or hidden nodes (or
//! nodes inside a locked or hidden container) fail with `editor.locked` /
//! `editor.hidden` and offer `unlock` / `show`.
//!
//! ## Selections
//!
//! `gesture.*` with `nodes` (or no `node`, with several nodes selected)
//! moves, resizes, or turns the nodes as one transaction: one history
//! entry, one undo.
//!
//! - **Move**: each node moves by the same page delta, mapped into its own
//!   parent space through its `world` (mixed and turned parents).
//! - **Resize**: the grips are those of the selection box, the page bounds
//!   of every node's drawn box (`node.handles {ids}`). The page plane maps
//!   the box onto the resized one; each node's centre follows the map and
//!   its own axes stretch by the map's scale along them (exact for nodes
//!   whose axes are page axes; a turned node keeps its angle). A line maps
//!   its endpoints.
//! - **Rotate**: every node turns by the angle (`snap` steps the angle
//!   itself) and its centre orbits the selection box centre; a line turns
//!   its endpoints.
//! - **Refusals**: each node maps with the single-node policy. Any refusing
//!   node refuses the gesture: the error lists every refusing node's
//!   diagnostics and the union of the offers; an offer resends the whole
//!   gesture with its flag. Locked or hidden nodes refuse first.
//! - A node inside another selected node rides along with it. A connector
//!   (`editor.follows_targets`), footnote, or light (`editor.no_geometry`)
//!   rides along with a note. Point handles belong to one node
//!   (`editor.unknown_handle`); nodes of several pages are
//!   `editor.mixed_pages`.
//!
//! `node.duplicate {ids}` copies several nodes in one transaction and
//! selects the copies.
//!
//! ## Snapping
//!
//! `snap_distance` (page px: the page sends its screen threshold divided by
//! the zoom) snaps a move, and a grip drag of a box node with no turn of
//! its own under an unflipped axis-aligned scale (or of a selection), to
//! the left / centre / right and top / middle / bottom lines of every other
//! drawn box on the page (containers included) and of the page itself. The
//! closest line within the distance wins per axis; a tie keeps the first
//! moving line, then the first target in raw id order (the page last). The
//! snapped page delta then maps to the same ops as an unsnapped one (no new
//! syntax). Preview and commit reply with `snap {dx, dy, guides: [{axis:
//! x|y, at, from, to}]}`: every line of the moved box that lies on a target
//! line, spanning both boxes (a grip reports only the edges it moved). No
//! equal-spacing hints.
//!
//! ## Marquee
//!
//! `select.marquee` selects the nodes a page rectangle meets (painted shape
//! for vector nodes, drawn box otherwise, inside their clips), or with
//! `contain` the nodes whose whole drawn box it holds. Locked nodes never
//! count. A container whose box holds the whole rectangle is entered: its
//! content counts. Only the outermost counted node of a subtree is
//! selected. Master and instance content selects what a click selects.
//! `extend` adds to the selection.
//!
//! # Modules
//!
//! - [`session`] — the session, deltas, and history.
//! - [`registry`] — the command table.
//! - [`Env`] / [`MemProject`] — the project a command runs against.
//! - `commands`, `gesture`, `edit`, `doc`, `geom` — internals.

mod commands;
mod ctx;
mod doc;
mod edit;
mod env;
mod error;
mod execute;
pub mod fonts;
mod geom;
mod gesture;
pub mod registry;
pub mod session;
mod wire;

pub use ctx::{ImageRegion, RenderedImage, Work, hex_sha256};
pub use env::{DEFAULT_DOCUMENT_PATH, Env, MemProject};
pub use error::{EditorError, Offer};
pub use execute::{Outcome, Request, execute};
pub use registry::{CommandSpec, Disabled, commands};
pub use session::{History, HistoryLimit, Session, TextDelta, Viewport};
pub use wire::{DeltaOut, DiagnosticOut, severity_name};
