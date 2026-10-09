//! The MCP tool catalog: 20 tools with token-lean JSON-Schema inputs.
//!
//! The surface is deliberately small and stable (clients cache `tools/list`
//! once). All node/op/surface schema detail lives behind the single
//! `zenith_schema` meta-tool — agents fetch one node kind or one tx op on demand
//! instead of carrying every schema in context. Every other tool returns a
//! trimmed structured result and expands only on opt-in params.
//!
//! A `doc` argument accepts a path or the 26-character `doc-id`.
//! Schema, fonts, theme creation, and workspace `unbundle` do not require `doc`.
//! Check tool schemas and operation requirements for document arguments.
//! See [`super::doc_ref`] for document identity resolution.

use serde_json::{Value, json};

/// A single MCP tool definition.
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: Value,
}

/// The `doc` argument schema fragment shared by every document-scoped tool.
fn doc_arg() -> Value {
    json!({ "type": "string", "description": "Document path or 26-char doc-id." })
}

/// The full tool catalog.
pub fn catalog() -> Vec<Tool> {
    vec![
        Tool {
            name: "zenith_schema",
            description: "Look up the document schema ON DEMAND: node kinds, a node's attributes, \
tx ops, or one op's fields + a ready-to-edit JSON example. Use surface=variant to discover the \
variants block and override entry structure (override/variant are NOT node kinds). \
Call this before building a zenith_tx instead of guessing.",
            schema: json!({
                "type": "object",
                "properties": {
                    "surface": {
                        "type": "string",
                        "enum": ["overview", "nodes", "node", "ops", "op", "tokens", "token", "page", "asset", "document", "variant", "diagnostics"],
                        "description": "Which schema surface to fetch."
                    },
                    "name": { "type": "string", "description": "Node kind (surface=node), op name (surface=op), or token type e.g. \"gradient\" (surface=token)." }
                },
                "required": ["surface"]
            }),
        },
        Tool {
            name: "zenith_fonts",
            description: "List the font families available to the renderer: Bundled (portable, \
deterministic across machines) vs Local/system (this machine only — using one emits a `font.local` \
advisory). Use a bundled family for reproducible output.",
            schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        Tool {
            name: "zenith_validate",
            description: "Validate a .zen document. Returns counts + the blocking errors only; \
hard (Error) diagnostics block rendering — fix them first. Set severity to also see warnings/advisories.",
            schema: json!({
                "type": "object",
                "properties": {
                    "doc": doc_arg(),
                    "severity": {
                        "type": "string",
                        "enum": ["error", "warning", "advisory"],
                        "description": "Lowest severity to include in the diagnostics array (default: error)."
                    }
                },
                "required": ["doc"]
            }),
        },
        Tool {
            name: "zenith_inspect",
            description: "Discover node ids and structure (read-only). Returns a shallow tree by \
default (deeper levels collapse to child_count). Use node/depth to drill in, detail for geometry: \
authored `geometry` plus the final page-absolute `box` after auto-layout. \
Large trees are returned as a resource link.",
            schema: json!({
                "type": "object",
                "properties": {
                    "doc": doc_arg(),
                    "node": { "type": "string", "description": "Only the subtree at this node id." },
                    "depth": { "type": "integer", "minimum": 0, "description": "Levels to expand (default 1)." },
                    "detail": { "type": "boolean", "description": "Include geometry, resolved box, visible, and locked per node." }
                },
                "required": ["doc"]
            }),
        },
        Tool {
            name: "zenith_tokens",
            description: "List every design token and its resolved value. Visual properties must \
reference tokens, so this reveals the palette/type/spacing a document exposes.",
            schema: json!({
                "type": "object",
                "properties": {
                    "doc": doc_arg(),
                    "diagnostics": { "type": "boolean", "description": "Include token diagnostics (default false)." }
                },
                "required": ["doc"]
            }),
        },
        Tool {
            name: "zenith_tx",
            description: "Apply a typed transaction (JSON edit script) to a document. Dry-run by \
default (returns status, affected ids, source_diff, and boxes: the moved/resized node boxes; a \
node whose whole subtree moved rigidly is one entry with descendants: N); set \
apply=true to write. Set diff=true to also get the resulting (after) source as a resource link, and \
source_diff and boxes with apply=true. Enforces id-uniqueness and referential integrity. \
Use zenith_schema op to learn an op's shape.",
            schema: json!({
                "type": "object",
                "properties": {
                    "doc": doc_arg(),
                    "transaction": {
                        "description": "The transaction as a JSON object, array, or string.",
                        "type": ["object", "string", "array"]
                    },
                    "apply": { "type": "boolean", "description": "Write the result to disk." },
                    "diff": { "type": "boolean", "description": "Return a resource link to the after source; with apply, also return source_diff and boxes." }
                },
                "required": ["doc", "transaction"]
            }),
        },
        Tool {
            name: "zenith_fix",
            description: "Apply machine-fixable diagnostics in one step: raw visual literals become \
token references (exact-value token, else a minted token), typo'd property names, token ids, and \
enum values become their unique did-you-mean, and attributes with no effect \
(layout.position_ignored, layout.inert_attribute) are removed. Dry-run by default; set apply=true \
to write. Returns \
the applied fixes and the remaining errors.",
            schema: json!({
                "type": "object",
                "properties": {
                    "doc": doc_arg(),
                    "apply": { "type": "boolean", "description": "Write the fixed source to disk." }
                },
                "required": ["doc"]
            }),
        },
        Tool {
            name: "zenith_render",
            description: "Render a document deterministically to png, svg, pdf, or scene (display-list \
JSON). Returns a resource link to the artifact (never inlines bytes); blocked by hard diagnostics \
— validate first. Pass out to also write the file to a path you choose. For cheap visual checks \
use png with scale 0.25-0.5, or contact_sheet=true to see every page in one image. \
SVG exports page 1 by default, outlines text, embeds images, and reports raster fallback diagnostics.",
            schema: json!({
                "type": "object",
                "properties": {
                    "doc": doc_arg(),
                    "format": { "type": "string", "enum": ["png", "svg", "pdf", "scene"] },
                    "page": { "type": "integer", "minimum": 1, "description": "1-based page. Omit for every PDF page or page 1 in other formats." },
                    "locked": { "type": "boolean", "description": "Verify asset sha256 and fail on mismatch." },
                    "out": { "type": "string", "description": "Optional path to also write the artifact to." },
                    "diagnostics": { "type": "boolean", "description": "Include soft diagnostics (default false)." },
                    "scale": { "type": "number", "exclusiveMinimum": 0, "maximum": 4, "description": "png only: raster scale, 0 < scale <= 4 (default 1). Each side is max(1, round(page_px * scale)); drawn at that scale, not resampled." },
                    "raster_scale": { "type": "number", "exclusiveMinimum": 0, "maximum": 4, "description": "svg/pdf only: raster fallback resolution, 0 < raster_scale <= 4 (default 1). Vector page dimensions and geometry remain unchanged." },
                    "contact_sheet": { "type": "boolean", "description": "png only: tile every page (or `page`) into one labelled PNG, ceil(sqrt(n)) columns. Without scale, fits 2048 px wide." }
                },
                "required": ["doc", "format"]
            }),
        },
        Tool {
            name: "zenith_fmt",
            description: "Canonicalize a document in place (idempotent). Returns whether the file \
changed and its content hash.",
            schema: json!({
                "type": "object",
                "properties": { "doc": doc_arg() },
                "required": ["doc"]
            }),
        },
        Tool {
            name: "zenith_merge",
            description: "Mail-merge a .zen template with CSV data, writing PNG or SVG pages per row. Mark \
variable nodes with role=\"data.<column>\". Use for localized/personalized/batch variants.",
            schema: json!({
                "type": "object",
                "properties": {
                    "doc": doc_arg(),
                    "data": { "type": "string", "description": "CSV data file path." },
                    "out_dir": { "type": "string", "description": "Directory for committed row pages." },
                    "name_by": { "type": "string", "description": "CSV column to name files by." },
                    "manifest": { "type": "string", "description": "Write a reproducibility manifest here." },
                    "format": { "type": "string", "enum": ["png", "svg"], "default": "png", "description": "Output image format (default png)." },
                    "raster_scale": { "type": "number", "exclusiveMinimum": 0, "maximum": 4, "description": "svg only: raster fallback resolution, 0 < raster_scale <= 4 (default 1). Vector geometry remains unchanged." }
                },
                "required": ["doc", "data", "out_dir"]
            }),
        },
        Tool {
            name: "zenith_theme_new",
            description: "Synthesize a complete theme pack (.zen: tokens, fixed styles, and a \
defaults block for text, shape, and connector) from brand colours, with APCA-correct content \
pairings for WCAG 3 contrast. Returns the generated source; pass out to \
write it to a file instead.",
            schema: theme_schema(),
        },
        Tool {
            name: "zenith_workspace_scratch",
            description: "Manage scratch candidates — point-in-time .zen snapshots that keep design \
iteration out of the deliverable file. op=new snapshots the current doc; op=list/show enumerate. \
Each candidate is addressable as a resource.",
            schema: json!({
                "type": "object",
                "properties": {
                    "doc": doc_arg(),
                    "op": { "type": "string", "enum": ["new", "list", "show"] },
                    "page": { "type": "string", "description": "Page id this candidate captures (default whole doc)." },
                    "candidate_id": { "type": "string", "description": "Candidate id (for op=show)." },
                    "status": { "type": "string", "enum": ["draft", "selected", "rejected"], "description": "Initial status for op=new." },
                    "notes": { "type": "string" },
                    "workspace_role": { "type": "string", "description": "e.g. hero, fallback." },
                    "promotion_target": { "type": "string" },
                    "cleanup_policy": { "type": "string", "description": "e.g. delete." }
                },
                "required": ["doc", "op"]
            }),
        },
        Tool {
            name: "zenith_workspace_candidate",
            description: "Transition a scratch candidate's lifecycle: draft → selected | rejected. \
Only a selected candidate can be promoted.",
            schema: json!({
                "type": "object",
                "properties": {
                    "doc": doc_arg(),
                    "candidate_id": { "type": "string" },
                    "status": { "type": "string", "enum": ["draft", "selected", "rejected"] }
                },
                "required": ["doc", "candidate_id", "status"]
            }),
        },
        Tool {
            name: "zenith_workspace_promote",
            description: "Promote a selected candidate's page into a target page of the deliverable \
document (deep-copies, suffixes ids, validates, writes in place).",
            schema: json!({
                "type": "object",
                "properties": {
                    "doc": doc_arg(),
                    "candidate_id": { "type": "string" },
                    "target_page": { "type": "string", "description": "Destination page id." },
                    "id_suffix": { "type": "string", "description": "Suffix appended to cloned ids (default .promoted)." }
                },
                "required": ["doc", "candidate_id", "target_page"]
            }),
        },
        Tool {
            name: "zenith_workspace_finalize",
            description: "op=finalize cleans up rejected candidates per cleanup-policy. op=bundle \
packs the doc's whole session store into a portable .zenithbundle (returns a resource link). \
op=unbundle restores one from a bundle path. Read any preview resources BEFORE finalizing.",
            schema: json!({
                "type": "object",
                "properties": {
                    "doc": doc_arg(),
                    "op": { "type": "string", "enum": ["finalize", "bundle", "unbundle"] },
                    "bundle": { "type": "string", "description": "Bundle file path (out for op=bundle, in for op=unbundle)." }
                },
                "required": ["op"]
            }),
        },
        Tool {
            name: "zenith_editor_open",
            description: "Open a .zen file in an editor session (selection, gestures, undo, live \
render). Returns the session id. Reopening a clean session rereads the file; a dirty one needs \
discard=true. Commands: send command commands.list via zenith_editor_command.",
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Document path." },
                    "root": { "type": "string", "description": "Directory project reads stay under (default: the document's directory)." },
                    "discard": { "type": "boolean", "description": "Reopen even with unsaved edits, dropping them." }
                },
                "required": ["path"]
            }),
        },
        Tool {
            name: "zenith_editor_command",
            description: "Run one editor command on a session: select.hit, gesture.commit, \
tx.apply, history.undo, doc.outline, and more. See commands.list for every command and its \
params. Text-changing commands need the session version and return a delta and a unified diff. \
Host commands: file.save, file.reload, file.state. doc.render and gesture.preview return a PNG.",
            schema: json!({
                "type": "object",
                "properties": {
                    "session": { "type": "string", "description": "Session id from zenith_editor_open or zenith_editor_attach." },
                    "path": { "type": "string", "description": "Document path instead of a session id (opens a session when none exists)." },
                    "command": { "type": "string", "description": "Command id, e.g. commands.list." },
                    "params": { "type": "object", "description": "Command params (see commands.list)." },
                    "version": { "type": "integer", "minimum": 0, "description": "Session version the edit was made at." }
                },
                "required": ["command"]
            }),
        },
        Tool {
            name: "zenith_editor_render",
            description: "Render a session page to PNG. Returns an MCP image plus width, height, \
and sha256. Renders the last valid text while the text has errors (stale: true).",
            schema: json!({
                "type": "object",
                "properties": {
                    "session": { "type": "string", "description": "Session id." },
                    "path": { "type": "string", "description": "Document path instead of a session id." },
                    "page": { "type": "integer", "minimum": 1, "description": "1-based page (default: the session page)." },
                    "scale": { "type": "number", "exclusiveMinimum": 0, "maximum": 4, "description": "Raster scale (default: the session zoom)." }
                }
            }),
        },
        Tool {
            name: "zenith_editor_sessions",
            description: "List the editor sessions of this server: id, kind (local or attached), \
path, version, dirty, valid, conflict.",
            schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            name: "zenith_editor_attach",
            description: "Attach to a running `zenith edit` on this machine, so a human watching \
the page sees your edits live. Pass the URL it printed (and the token when the URL lacks it). \
Then use the returned session id with zenith_editor_command and zenith_editor_render.",
            schema: json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "The http://127.0.0.1:<port>/?token=... URL `zenith edit` printed." },
                    "token": { "type": "string", "description": "The token, when the URL has none." }
                },
                "required": ["url"]
            }),
        },
    ]
}

/// The (necessarily larger) input schema for `zenith_theme_new`.
fn theme_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "scheme": { "type": "string", "enum": ["light", "dark"] },
            "primary": { "type": "string", "description": "#rrggbb" },
            "secondary": { "type": "string" },
            "accent": { "type": "string" },
            "neutral": { "type": "string" },
            "info": { "type": "string" },
            "success": { "type": "string" },
            "warning": { "type": "string" },
            "error": { "type": "string" },
            "radius_box": { "type": "number" },
            "radius_field": { "type": "number" },
            "radius_selector": { "type": "number" },
            "border": { "type": "number" },
            "depth": { "type": "boolean" },
            "noise": { "type": "boolean" },
            "out": { "type": "string", "description": "Write the theme here instead of returning the source." }
        },
        "required": ["name", "scheme", "primary"]
    })
}

/// Render the catalog as the `tools/list` result payload.
pub fn list_payload() -> Value {
    let tools: Vec<Value> = catalog()
        .into_iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": t.schema,
            })
        })
        .collect();
    json!({ "tools": tools })
}
