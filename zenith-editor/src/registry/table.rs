//! The command table: every command the engine answers, in a stable order.

use super::spec::{CommandSpec, always, when_displayable, when_redo, when_undo, when_valid};
use crate::commands::{
    at_offset, batch, buffer, diagnose, fonts, format, gesture, handles, history, inspect, list,
    marquee, open, outline, render, select, set, structure, tx, view,
};

/// The diff every text-changing command returns.
const EDIT_RESULT: &str = "{changed, version, delta?: {start, end, from, to, insert}, \
    reformatted, removed_comments: [string], ops: [op], selection: [id], valid, stale, \
    diagnostics: [diagnostic], tx_diagnostics: [diagnostic], notes: [diagnostic]}";

/// The gesture params shared by `gesture.preview` and `gesture.commit`.
const GESTURE_PARAMS: &str = "{node?: id, nodes?: [id] (several nodes as one selection; \
    default: the selection), handle?: \
    \"nw\"|\"n\"|\"ne\"|\"e\"|\"se\"|\"s\"|\"sw\"|\"w\"|\"rotate\"|\"start\"|\"end\"|\"p<i>\"|\
    \"a<s>.<i>\"|\"a<s>.<i>.in\"|\"a<s>.<i>.out\" (absent: move), dx?: page px = 0, dy?: \
    page px = 0, angle?: degrees clockwise = 0, snap?: degrees, constrain?: bool, \
    from_center?: bool, snap_distance?: page px (snap moves and grips to other nodes and the \
    page; 0 or absent: off), detach?: bool, detach_anchor?: bool, confirm_size?: bool, \
    replace?: bool, absolute?: bool, reorder?: bool}";

/// Every command, in `commands.list` order.
static COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        id: "doc.open",
        label: "Open document",
        params: "{text: string}",
        result: "{version, valid, stale, page_count?, diagnostics: [diagnostic]}",
        mutates: true,
        needs_version: false,
        enabled: always,
        run: open::run,
    },
    CommandSpec {
        id: "buffer.set",
        label: "Update text",
        params: "{text: string, coalesce?: bool = true}",
        result: "{changed, version, valid, stale, selection: [id], diagnostics: [diagnostic]}",
        mutates: true,
        needs_version: true,
        enabled: always,
        run: buffer::run,
    },
    CommandSpec {
        id: "doc.diagnose",
        label: "Check document",
        params: "{}",
        result: "{valid, exit_code, stale, diagnostics: [diagnostic]}",
        mutates: false,
        needs_version: false,
        enabled: always,
        run: diagnose::run,
    },
    CommandSpec {
        id: "doc.render",
        label: "Render page",
        params: "{page?: 1-based = session page, scale?: number = viewport zoom (at most 4 \
            without viewport), viewport?: {x, y, w, h} page px (render only this window, \
            snapped out to whole device px; no fixed scale cap)}",
        result: "{page, page_count, width, height, scale, sha256, stale, diagnostics: \
            [diagnostic], rect?: {x, y, w, h} device px, device_size?: {w, h} device px, \
            page_size?: {w, h} page px, view?: {x, y, w, h} page px} + the PNG out of band; \
            rect, device_size, page_size, and view come with viewport",
        mutates: false,
        needs_version: false,
        enabled: when_displayable,
        run: render::run,
    },
    CommandSpec {
        id: "doc.outline",
        label: "Layers",
        params: "{}",
        result: "{stale, pages: [{page, id, name?, master?, w, h, children: [layer]}], \
            masters: [{id, children: [layer]}]}; layer = {id?, kind, name?, visible, locked, \
            guide, children: [layer]}",
        mutates: false,
        needs_version: false,
        enabled: when_displayable,
        run: outline::run,
    },
    CommandSpec {
        id: "node.inspect",
        label: "Inspect node",
        params: "{id?: id = the single selected node}",
        result: "{id, kind, name?, page?, master?, parent?, attributes: [{name, value, unit?, \
            token?: {id, type, value?}, px?}], box?, span?: {start, end, line, col, end_line, \
            end_col}, lock: {locked, locked_by?, hidden, in_flow?, anchored}, style?, \
            style_provenance, stale, edit?: {fields: [name], x?, y?, w?, h?, axes: {axis: \
            length|token|absent|keyword|unresolved|anchor|flow}, rotate?, opacity?, fill?, \
            stroke?, stroke_width?, radius?, font_family?, font_size?, font_weight? (each \
            {token?, value?, resolved?, style?}), align?, text?, spans?: [{text, styled}], \
            visible?, locked?}}",
        mutates: false,
        needs_version: false,
        enabled: when_displayable,
        run: inspect::run,
    },
    CommandSpec {
        id: "select.hit",
        label: "Select at point",
        params: "{x: page px, y: page px, page?: 1-based = session page, tolerance?: page px \
            = 0, extend?: bool, select?: bool = true}",
        result: "{page, stale, hits: [{id, raw_id, kind, name?, locked, master?, via?}], \
            selection: [id]}",
        mutates: false,
        needs_version: false,
        enabled: when_displayable,
        run: select::hit,
    },
    CommandSpec {
        id: "select.set",
        label: "Select",
        params: "{ids: [id]}",
        result: "{selection: [id]}",
        mutates: false,
        needs_version: false,
        enabled: always,
        run: select::set,
    },
    CommandSpec {
        id: "select.marquee",
        label: "Select in rectangle",
        params: "{x, y, w, h: page px (negative w / h extend left / up), page?: 1-based = \
            session page, contain?: bool (whole drawn box inside), extend?: bool (add to the \
            selection), select?: bool = true}",
        result: "{page, stale, hits: [{id, kind, name?, master?}], selection: [id]}",
        mutates: false,
        needs_version: false,
        enabled: when_displayable,
        run: marquee::run,
    },
    CommandSpec {
        id: "select.at_offset",
        label: "Select at cursor",
        params: "{offset: byte offset into the current text, select?: bool = true}",
        result: "{parsed, id: id|null, kind?, page: 1-based|null, master?, selection: [id]}",
        mutates: false,
        needs_version: false,
        enabled: always,
        run: at_offset::run,
    },
    CommandSpec {
        id: "node.handles",
        label: "Selection handles",
        params: "{id?: id = the single selected node, ids?: [id] (two or more: the selection \
            box), rotate_offset?: page px = 24}",
        result: "{id, kind, page, stale, corners?: [[x, y]; 4], center?, angle, handles: \
            [{id, role, x, y, enabled, reason?}], disabled: [{action, code, axes?, detail, \
            node?}], master?, pivot_follows_content}; with ids: {ids, kind: \"selection\", page, \
            stale, corners, center, angle: 0, handles, disabled, members: [{id, kind, corners}], \
            pivot_follows_content}",
        mutates: false,
        needs_version: false,
        enabled: when_displayable,
        run: handles::run,
    },
    CommandSpec {
        id: "gesture.preview",
        label: "Preview gesture",
        params: GESTURE_PARAMS,
        result: "{node | nodes, ops: [op], corners?: [[x, y]; 4], members?: [{id, corners}], \
            notes: [diagnostic], snap?: {dx, dy, guides: [{axis, at, from, to}]}, width?, \
            height?, sha256?, rect?, device_size?, page_size?, view?} + the PNG out of band when \
            render \
            (params: \
            scale?, render?: bool = true, viewport?: {x, y, w, h} page px as doc.render)",
        mutates: false,
        needs_version: false,
        enabled: when_valid,
        run: gesture::preview,
    },
    CommandSpec {
        id: "gesture.commit",
        label: "Apply gesture",
        params: GESTURE_PARAMS,
        result: EDIT_RESULT,
        mutates: true,
        needs_version: true,
        enabled: when_valid,
        run: gesture::commit,
    },
    CommandSpec {
        id: "tx.apply",
        label: "Apply transaction",
        params: "{ops: [op], permissions?: {allow_locked, allow_raw_visual_literals}, label?: \
            string, select?: [id]}",
        result: EDIT_RESULT,
        mutates: true,
        needs_version: true,
        enabled: when_valid,
        run: tx::run,
    },
    CommandSpec {
        id: "node.set",
        label: "Set properties",
        params: "{id?: id = the single selected node, x?: authored px, y?: authored px, w?: \
            px, h?: px, rotate?: degrees, opacity?: 0..1, fill?, stroke?, stroke_width?, \
            radius?, font_family?, font_size?, font_weight?: token id | {value: raw} (radius \
            and font_* also null), align?: start|center|end|justify, text?: \
            string, spans?: [{index, text}], visible?: bool, locked?: bool, detach?: bool, \
            detach_anchor?: bool, confirm_size?: bool, replace?: bool, absolute?: bool}",
        result: EDIT_RESULT,
        mutates: true,
        needs_version: true,
        enabled: when_valid,
        run: set::run,
    },
    CommandSpec {
        id: "node.remove",
        label: "Delete",
        params: "{ids?: [id] = selection}",
        result: EDIT_RESULT,
        mutates: true,
        needs_version: true,
        enabled: when_valid,
        run: structure::remove,
    },
    CommandSpec {
        id: "node.duplicate",
        label: "Duplicate",
        params: "{id?: id, ids?: [id] (default: the selection), new_id?: id = \"<id>-copy\" \
            (one node), dx?: authored px = 0, dy?: authored px = 0}",
        result: EDIT_RESULT,
        mutates: true,
        needs_version: true,
        enabled: when_valid,
        run: structure::duplicate,
    },
    CommandSpec {
        id: "node.reorder",
        label: "Arrange",
        params: "{id?: id = the single selected node, to: \"forward\"|\"backward\"|\"front\"|\
            \"back\"}",
        result: EDIT_RESULT,
        mutates: true,
        needs_version: true,
        enabled: when_valid,
        run: structure::reorder,
    },
    CommandSpec {
        id: "node.group",
        label: "Group",
        params: "{ids?: [id] = selection, group_id?: id = \"group\"}",
        result: EDIT_RESULT,
        mutates: true,
        needs_version: true,
        enabled: when_valid,
        run: structure::group,
    },
    CommandSpec {
        id: "node.ungroup",
        label: "Ungroup",
        params: "{id?: id = the single selected node}",
        result: EDIT_RESULT,
        mutates: true,
        needs_version: true,
        enabled: when_valid,
        run: structure::ungroup,
    },
    CommandSpec {
        id: "history.undo",
        label: "Undo",
        params: "{}",
        result: EDIT_RESULT,
        mutates: true,
        needs_version: true,
        enabled: when_undo,
        run: history::undo,
    },
    CommandSpec {
        id: "history.redo",
        label: "Redo",
        params: "{}",
        result: EDIT_RESULT,
        mutates: true,
        needs_version: true,
        enabled: when_redo,
        run: history::redo,
    },
    CommandSpec {
        id: "doc.format",
        label: "Format document",
        params: "{}",
        result: EDIT_RESULT,
        mutates: true,
        needs_version: true,
        enabled: always,
        run: format::run,
    },
    CommandSpec {
        id: "doc.tokens",
        label: "Tokens",
        params: "{type?: color|dimension|number|fontFamily|fontWeight|gradient|shadow|filter|mask}",
        result: "{stale, tokens: [{id, type, value}]}",
        mutates: false,
        needs_version: false,
        enabled: when_displayable,
        run: inspect::tokens,
    },
    CommandSpec {
        id: "fonts.required",
        label: "Fonts to fetch",
        params: "{}",
        result: "{faces: [{family, weight, style, file?}], embedded: [file], complete}",
        mutates: false,
        needs_version: false,
        enabled: always,
        run: fonts::run,
    },
    CommandSpec {
        id: "view.set",
        label: "Set view",
        params: "{page?: 1-based, zoom?: number, pan_x?: page px, pan_y?: page px}",
        result: "{page, viewport: {zoom, pan_x, pan_y}}",
        mutates: false,
        needs_version: false,
        enabled: always,
        run: view::run,
    },
    CommandSpec {
        id: "commands.list",
        label: "Commands",
        params: "{}",
        result: "{commands: [{id, label, params, result, mutates, needs_version, enabled, \
            disabled?: {code, reason}}]}",
        mutates: false,
        needs_version: false,
        enabled: always,
        run: list::run,
    },
    CommandSpec {
        id: "commands.batch",
        label: "Run commands",
        params: "{steps: [{command, params?, version?}] (at most 16; at most one doc.render or \
            gesture.preview; no commands.batch)}",
        result: "{steps: [{command, ok, result? | error?, work?}], image_step?: index of the \
            step whose PNG comes out of band}; a failed step with version stops the batch, \
            later steps reply editor.skipped",
        mutates: true,
        needs_version: false,
        enabled: always,
        run: batch::run,
    },
];

/// Every command the engine answers, in a stable order.
#[must_use]
pub fn commands() -> &'static [CommandSpec] {
    COMMANDS
}

/// The command named `id`.
#[must_use]
pub fn find(id: &str) -> Option<&'static CommandSpec> {
    COMMANDS.iter().find(|spec| spec.id == id)
}
