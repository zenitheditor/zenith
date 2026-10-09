//! `node.inspect {id?}`: everything the inspector panel shows for one node.

use std::rc::Rc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use zenith_core::{Document, line_col, resolve_tokens};
use zenith_tx::{FlowPlacement, layout_flow};

use super::attrs::{Attribute, attributes};
use super::edit::{Editable, editable};
use super::style::{StyleSource, style_source};
use crate::commands::common::{params, target};
use crate::ctx::Ctx;
use crate::doc::tree::{Root, locate};
use crate::error::EditorError;
use crate::geom::{Pt, angle_deg};
use crate::gesture::target::{hidden_by, locked_by, resolve};
use crate::wire::to_json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InspectParams {
    #[serde(default)]
    id: Option<String>,
}

#[derive(Debug, Serialize)]
struct SourceSpan {
    start: usize,
    end: usize,
    line: usize,
    col: usize,
    end_line: usize,
    end_col: usize,
}

#[derive(Debug, Serialize)]
struct ResolvedBox {
    /// The 1-based page the box is on.
    page: usize,
    /// The box in the node's own space before its rotation (page px for a
    /// node with no rotated ancestor).
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    /// The drawn corners in page px.
    corners: [Pt; 4],
    /// The node's own authored rotation, degrees.
    #[serde(skip_serializing_if = "Option::is_none")]
    rotate: Option<f64>,
    /// The page angle of the drawn box's x axis, degrees.
    angle: f64,
}

#[derive(Debug, Serialize)]
struct Lock {
    /// The node itself has `locked=#true`.
    locked: bool,
    /// The node that locks it for the editor: itself or an ancestor.
    #[serde(skip_serializing_if = "Option::is_none")]
    locked_by: Option<String>,
    /// The node that hides it: itself or an ancestor.
    #[serde(skip_serializing_if = "Option::is_none")]
    hidden_by: Option<String>,
    /// The layout frame that places it, when in flow.
    #[serde(skip_serializing_if = "Option::is_none")]
    in_flow: Option<Flow>,
    /// An anchor attribute places it.
    anchored: bool,
}

#[derive(Debug, Serialize)]
struct Flow {
    frame: String,
    mode: &'static str,
}

#[derive(Debug, Serialize)]
struct Inspect {
    id: String,
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    /// The 1-based page that holds the node; absent for master content.
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<usize>,
    /// The master that holds the node.
    #[serde(skip_serializing_if = "Option::is_none")]
    master: Option<String>,
    /// The id of the parent node, page, or master.
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<String>,
    attributes: Vec<Attribute>,
    #[serde(rename = "box", skip_serializing_if = "Option::is_none")]
    resolved: Option<ResolvedBox>,
    /// The node's byte range in the current text, for jump-to-code. Absent
    /// while the text does not parse.
    #[serde(skip_serializing_if = "Option::is_none")]
    span: Option<SourceSpan>,
    lock: Lock,
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<StyleSource>,
    style_provenance: &'static str,
    stale: bool,
    /// The `node.set` fields this node takes, with their current values.
    /// Absent while the text has errors or does not hold the node.
    #[serde(skip_serializing_if = "Option::is_none")]
    edit: Option<Editable>,
}

/// What `style_provenance` says about the depth of `style`.
const PROVENANCE: &str = "style names the style the node draws with (its own style, else the \
    page or document defaults entry for its kind); which of node attribute, style, or defaults \
    set each drawn value is not tracked by the AST, so attributes list only what the node \
    writes itself";

/// Inspect one node (default: the one selected node).
///
/// The attributes, span, and lock state come from the current text when it
/// parses; otherwise from the last valid text (`stale: true`, no span).
/// Attributes are read from the node's own source: each with its value as
/// written, its unit or `token` / `data` annotation, the bound token with
/// its type and resolved value, and the px value when one resolves. `box`
/// is the compiled box on the page the node draws on (from the display
/// text).
pub(crate) fn run(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: InspectParams = params(ctx, raw)?;
    let id = target(ctx, p.id)?;
    let text = ctx.session.text.clone();
    let current: Option<Rc<Document>> = ctx
        .parse(&text)
        .ok()
        .filter(|doc| locate(doc, &id).is_some());
    let display = ctx.display().ok();
    let (doc, source_text, from_current) = match (&current, &display) {
        (Some(doc), _) => (doc, text.as_str(), true),
        (None, Some(d)) => (&d.doc, d.text.as_str(), false),
        (None, None) => return Err(EditorError::unknown_node(&id)),
    };
    let located = locate(doc, &id).ok_or_else(|| EditorError::unknown_node(&id))?;
    let node = located.node;
    let tokens = resolve_tokens(&doc.tokens).resolved;
    let node_span = node.source_span();
    let attributes = node_span
        .and_then(|s| source_text.get(s.start..s.end))
        .map(|src| attributes(src, &tokens))
        .unwrap_or_default();
    let span = node_span.filter(|_| from_current).and_then(|s| {
        let (line, col) = line_col(source_text, s.start)?;
        let (end_line, end_col) = line_col(source_text, s.end)?;
        Some(SourceSpan {
            start: s.start,
            end: s.end,
            line,
            col,
            end_line,
            end_col,
        })
    });
    let (page, master) = match located.root {
        Root::Page(i) => (Some(i + 1), None),
        Root::Master(m) => (None, doc.masters.get(m).map(|m| m.id.clone())),
    };
    let lock = Lock {
        locked: node.is_locked(),
        locked_by: locked_by(&located).map(str::to_owned),
        hidden_by: hidden_by(&located).map(str::to_owned),
        in_flow: layout_flow(doc, &id).map(|FlowPlacement { frame, mode }| Flow { frame, mode }),
        anchored: node.box_view().is_some_and(|v| v.anchored),
    };
    let style = style_source(doc, &located, &tokens);
    let mut reply = Inspect {
        id: id.clone(),
        kind: node.kind_str(),
        name: node.name().map(str::to_owned),
        page,
        master,
        parent: located.container_id(doc).map(str::to_owned),
        attributes,
        resolved: None,
        span,
        lock,
        style,
        style_provenance: PROVENANCE,
        stale: !from_current || display.as_ref().is_some_and(|d| d.stale),
        edit: None,
    };
    if let Some(d) = &display
        && let Ok(t) = resolve(ctx, &d.doc, &id)
    {
        let l = t.bx.local;
        reply.resolved = Some(ResolvedBox {
            page: t.page.index + 1,
            x: l.x,
            y: l.y,
            w: l.w,
            h: l.h,
            corners: t.bx.corners(),
            rotate: t.bx.rotate,
            angle: angle_deg(t.bx.transform()),
        });
        if !reply.stale {
            reply.edit = Some(editable(
                &d.doc,
                &t,
                &reply.attributes,
                reply.style.as_ref(),
            ));
        }
    }
    to_json(&reply)
}
