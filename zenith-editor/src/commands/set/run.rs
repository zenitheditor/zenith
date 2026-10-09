//! `node.set`: write a node's common properties, as the inspector does.

use serde_json::Value;
use zenith_core::Diagnostic;
use zenith_scene::Affine2;
use zenith_tx::{Op, OpSpan, Permissions};

use super::geometry::{EPSILON, box_ops, reject};
use super::params::SetParams;
use super::text::plain_text;
use super::tokens::{Kind, Tokens};
use crate::commands::common::{params, target};
use crate::ctx::Ctx;
use crate::doc::tree::locate;
use crate::edit::ops::{OpsEdit, apply_ops};
use crate::error::EditorError;
use crate::gesture::rotate::plan_rotate;
use crate::gesture::target::check_editable;
use crate::wire::DiagnosticOut;

/// Set the common properties of one node (default: the one selected node)
/// in one transaction, and keep it selected.
///
/// - `x` / `y` / `w` / `h`: authored px targets of a box node (the values
///   `node.inspect` lists under `edit`). Each changed axis goes through the
///   same mapping as a gesture: px and pt values keep their unit, a
///   token-bound axis needs `detach`, an anchored position moves the anchor
///   gap or needs `detach_anchor`, a computed size needs `confirm_size`, a
///   value with no px conversion needs `replace`, and an in-flow child's
///   position needs `absolute`. A rejection offers `node.set` again with
///   the flag that goes ahead.
/// - `rotate`: degrees; 0 removes the attribute.
/// - `opacity`: 0 to 1 (`set_opacity`).
/// - Token-backed fields take a token id, or `{value}` with a raw value:
///   `fill` / `stroke` (`#rrggbb` / `#rrggbbaa`), `stroke_width`,
///   `radius`, `font_size` (px), `font_family` (a family name), and
///   `font_weight` (100 to 900). A raw value
///   binds the first token (by id) of that type that holds it, else a new
///   token (`color.custom.<hex>`, `size.<property>.<n>`, `font.<name>`,
///   `weight.<n>`) the same transaction creates:
///   Zenith documents bind visual properties to tokens. `null` removes
///   `radius`, `font_family`, `font_size`, or `font_weight`, so the style
///   or the default applies.
/// - No line height: text nodes have no such attribute, and no layout code
///   reads a style's `line-height`, so it would change nothing drawn.
/// - `align`: `start`, `center`, `end`, or `justify` (`set_text_align`).
/// - `text`: the content of a `text` node with one plain span
///   (`replace_text`). `spans: [{index, text}]`: the text of single spans
///   of a `text` or `shape` node, their attributes kept (`set_span_text`).
/// - `visible` / `locked`: show, hide, lock, or unlock. These two alone
///   also work on a locked or hidden node.
///
/// A value equal to the current one changes nothing.
pub(crate) fn run(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: SetParams = params(ctx, raw.clone())?;
    if p.numbers().iter().flatten().any(|v| !v.is_finite()) {
        return Err(EditorError::new(
            "editor.invalid_params",
            "x, y, w, h, rotate, and opacity must be finite numbers",
        ));
    }
    if p.opacity.is_some_and(|o| !(0.0..=1.0).contains(&o)) {
        return Err(EditorError::new(
            "editor.invalid_params",
            "opacity must be between 0 and 1",
        ));
    }
    let id = target(ctx, p.id.clone())?;
    let doc = ctx.editable()?;
    let located = locate(&doc, &id).ok_or_else(|| EditorError::unknown_node(&id))?;
    if !p.only_flags() {
        check_editable(&located, &id)?;
    }
    let node = located.node;
    let mut ops: Vec<Op> = Vec::new();
    let mut notes: Vec<DiagnosticOut> = Vec::new();
    let geometry = [p.x, p.y, p.w, p.h].iter().any(Option::is_some);
    if geometry {
        let (more, more_notes) = box_ops(ctx, &doc, &id, &p, &raw)?;
        ops.extend(more);
        notes.extend(more_notes);
    }
    if let Some(rotate) = p.rotate {
        let current = node.rotate().map_or(0.0, |d| d.value);
        if (rotate - current).abs() > EPSILON {
            let turned = plan_rotate(node, &id, Affine2::IDENTITY, rotate - current, None)
                .map_err(|e| {
                    if e.code.starts_with("tx.") {
                        let cause = Diagnostic::error(&e.code, e.message, None, Some(id.clone()));
                        reject(ctx, &raw, vec![cause])
                    } else {
                        e
                    }
                })?;
            ops.extend(turned);
        }
    }
    if let Some(opacity) = p.opacity
        && (node.opacity().unwrap_or(1.0) - opacity).abs() > EPSILON
    {
        ops.push(Op::SetOpacity {
            node: id.clone(),
            opacity,
        });
    }
    let mut tokens = Tokens::new(&doc);
    if let Some(fill) = &p.fill {
        let fill = tokens.bind("fill", "fill", Kind::Color, fill)?;
        ops.push(Op::SetFill {
            node: id.clone(),
            fill,
        });
    }
    if let Some(stroke) = &p.stroke {
        let stroke = tokens.bind("stroke", "stroke", Kind::Color, stroke)?;
        ops.push(Op::SetStroke {
            node: id.clone(),
            stroke,
        });
    }
    if let Some(width) = &p.stroke_width {
        let stroke_width = tokens.bind("stroke_width", "stroke-width", Kind::Dimension, width)?;
        ops.push(Op::SetStrokeWidth {
            node: id.clone(),
            stroke_width,
        });
    }
    for (field, property, kind, value) in [
        ("radius", "radius", Kind::Dimension, &p.radius),
        ("font_family", "font-family", Kind::Family, &p.font_family),
        ("font_size", "font-size", Kind::Dimension, &p.font_size),
        ("font_weight", "font-weight", Kind::Weight, &p.font_weight),
    ] {
        let Some(value) = value else { continue };
        let token = match value {
            Some(v) => Some(tokens.bind(field, property, kind, v)?),
            None => None,
        };
        ops.push(Op::SetNodeToken {
            node: id.clone(),
            property: property.to_owned(),
            token,
        });
    }
    if let Some(align) = &p.align {
        ops.push(Op::SetTextAlign {
            node: id.clone(),
            align: align.clone(),
        });
    }
    if let Some(text) = &p.text {
        if plain_text(node).is_none() {
            return Err(EditorError::new(
                "editor.unsupported",
                format!(
                    "{} '{id}' has no single plain span to replace; edit its spans with \
                     spans: [{{index, text}}] or in the code",
                    node.kind_str()
                ),
            ));
        }
        if plain_text(node) != Some(text.as_str()) {
            ops.push(Op::ReplaceText {
                node: id.clone(),
                spans: vec![OpSpan {
                    text: text.clone(),
                    fill: None,
                    font_weight: None,
                    italic: None,
                    underline: None,
                    strikethrough: None,
                    vertical_align: None,
                    footnote_ref: None,
                }],
            });
        }
    }
    for edit in p.spans.iter().flatten() {
        ops.push(Op::SetSpanText {
            node: id.clone(),
            span: edit.index,
            text: edit.text.clone(),
        });
    }
    if let Some(visible) = p.visible
        && visible != node.is_visible()
    {
        ops.push(Op::SetVisible {
            node: id.clone(),
            visible,
        });
    }
    if let Some(locked) = p.locked
        && locked != node.is_locked()
    {
        ops.push(Op::SetLocked {
            node: id.clone(),
            locked,
        });
    }
    let mut all = std::mem::take(&mut tokens.ops);
    all.extend(ops);
    apply_ops(
        ctx,
        &doc,
        OpsEdit {
            label: None,
            ops: all,
            permissions: Permissions::default(),
            selection: Some(vec![id]),
            notes,
            gesture: true,
            raw,
        },
    )
}
