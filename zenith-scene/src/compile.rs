//! Scene compilation: `Document` → `CompileResult`.
//!
//! Entry point: [`compile`].
//!
//! Rect nodes and text nodes are compiled; the page background is emitted
//! first; unknown nodes produce an advisory diagnostic and are skipped.

use std::collections::BTreeMap;

use zenith_core::{
    Diagnostic, Document, FontProvider, FontStyle, Node, PropertyValue, ResolvedToken,
    ResolvedValue, Span, Unit, resolve_tokens,
};
use zenith_layout::{RustybuzzEngine, ShapeRequest, TextLayoutEngine};

use crate::color::parse_srgb_hex;
use crate::ir::{Color, Scene, SceneCommand, SceneGlyph};

// ── Public result type ────────────────────────────────────────────────────────

/// The result of compiling a [`Document`] into a [`Scene`].
#[derive(Debug, Clone)]
pub struct CompileResult {
    /// The compiled display list.
    pub scene: Scene,
    /// All diagnostics collected during compilation (may include token-resolution
    /// diagnostics, unit advisories, and unsupported-node advisories).
    pub diagnostics: Vec<Diagnostic>,
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Compile `doc` into a [`CompileResult`], using `fonts` to shape text nodes.
///
/// Only the first page is compiled.  If the document has no pages an empty
/// scene is returned with an advisory diagnostic.
///
/// Pass `&zenith_core::default_provider()` to use the bundled Noto Sans
/// font, which is sufficient for basic text rendering.
///
/// # No-panic guarantee
///
/// This function never calls `unwrap`, `expect`, `panic!`, `todo!`,
/// `unimplemented!`, or performs unchecked indexing.  All failure paths push a
/// diagnostic and continue.
pub fn compile(doc: &Document, fonts: &dyn FontProvider) -> CompileResult {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    // ── Step 1: resolve tokens ────────────────────────────────────────────
    let token_resolution = resolve_tokens(&doc.tokens);
    diagnostics.extend(token_resolution.diagnostics);
    let resolved = &token_resolution.resolved;

    // ── Step 2: select the first page ────────────────────────────────────
    let Some(page) = doc.body.pages.first() else {
        diagnostics.push(Diagnostic::advisory(
            "scene.no_pages",
            "document has no pages; an empty scene is returned",
            None,
            Some(doc.body.id.clone()),
        ));
        return CompileResult {
            scene: Scene::new(0.0, 0.0),
            diagnostics,
        };
    };

    // ── Step 3: page dimensions → pixels ─────────────────────────────────
    let page_w = match dim_to_px(page.width.value, &page.width.unit) {
        Some(v) => v,
        None => {
            diagnostics.push(Diagnostic::advisory(
                "scene.unsupported_unit",
                format!(
                    "page '{}' width uses an unsupported unit; cannot compile scene",
                    page.id
                ),
                page.source_span,
                Some(page.id.clone()),
            ));
            return CompileResult {
                scene: Scene::new(0.0, 0.0),
                diagnostics,
            };
        }
    };
    let page_h = match dim_to_px(page.height.value, &page.height.unit) {
        Some(v) => v,
        None => {
            diagnostics.push(Diagnostic::advisory(
                "scene.unsupported_unit",
                format!(
                    "page '{}' height uses an unsupported unit; cannot compile scene",
                    page.id
                ),
                page.source_span,
                Some(page.id.clone()),
            ));
            return CompileResult {
                scene: Scene::new(0.0, 0.0),
                diagnostics,
            };
        }
    };

    let mut scene = Scene::new(page_w, page_h);

    // ── Step 4: outermost page-edge clip (doc 09 normative rule) ─────────
    scene.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: page_w,
        h: page_h,
    });

    // ── Step 5: optional page background ─────────────────────────────────
    if let Some(bg_prop) = &page.background
        && let Some(color) = resolve_property_color(bg_prop, resolved, &mut diagnostics, &page.id)
    {
        scene.commands.push(SceneCommand::FillRect {
            x: 0.0,
            y: 0.0,
            w: page_w,
            h: page_h,
            color,
        });
    }

    // ── Step 6: children in source order (z-order: first = bottom) ───────
    let engine = RustybuzzEngine::new();
    for node in &page.children {
        compile_node(
            node,
            resolved,
            fonts,
            &engine,
            &mut scene.commands,
            &mut diagnostics,
        );
    }

    // ── Step 7: close the outermost clip ─────────────────────────────────
    scene.commands.push(SceneCommand::PopClip);

    CompileResult { scene, diagnostics }
}

// ── Node dispatch ─────────────────────────────────────────────────────────────

fn compile_node(
    node: &Node,
    resolved: &BTreeMap<String, ResolvedToken>,
    fonts: &dyn FontProvider,
    engine: &RustybuzzEngine,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match node {
        Node::Rect(rect) => {
            // Skip invisible rects.
            if rect.visible == Some(false) {
                return;
            }

            // Resolve geometry — all four are required; skip if any is absent
            // or uses an unsupported unit.
            let (Some(x_dim), Some(y_dim), Some(w_dim), Some(h_dim)) =
                (&rect.x, &rect.y, &rect.w, &rect.h)
            else {
                diagnostics.push(Diagnostic::advisory(
                    "scene.missing_geometry",
                    format!(
                        "rect '{}' is missing one or more geometry properties (x, y, w, h); \
                         skipped",
                        rect.id
                    ),
                    rect.source_span,
                    Some(rect.id.clone()),
                ));
                return;
            };

            let Some(x) = dim_to_px(x_dim.value, &x_dim.unit) else {
                diagnostics.push(unsupported_unit_diag(
                    "rect",
                    &rect.id,
                    "x",
                    rect.source_span,
                ));
                return;
            };
            let Some(y) = dim_to_px(y_dim.value, &y_dim.unit) else {
                diagnostics.push(unsupported_unit_diag(
                    "rect",
                    &rect.id,
                    "y",
                    rect.source_span,
                ));
                return;
            };
            let Some(w) = dim_to_px(w_dim.value, &w_dim.unit) else {
                diagnostics.push(unsupported_unit_diag(
                    "rect",
                    &rect.id,
                    "w",
                    rect.source_span,
                ));
                return;
            };
            let Some(h) = dim_to_px(h_dim.value, &h_dim.unit) else {
                diagnostics.push(unsupported_unit_diag(
                    "rect",
                    &rect.id,
                    "h",
                    rect.source_span,
                ));
                return;
            };

            // Resolve fill color.
            let Some(fill_prop) = &rect.fill else {
                // No fill → nothing to draw for a fill-only skeleton.
                return;
            };
            let Some(mut color) =
                resolve_property_color(fill_prop, resolved, diagnostics, &rect.id)
            else {
                return;
            };

            // Apply opacity.
            if let Some(opacity) = rect.opacity {
                let o = opacity.clamp(0.0, 1.0);
                color.a = (color.a as f64 * o).round() as u8;
            }

            commands.push(SceneCommand::FillRect { x, y, w, h, color });
        }

        Node::Ellipse(ellipse) => {
            // Skip invisible ellipses.
            if ellipse.visible == Some(false) {
                return;
            }

            // Resolve geometry — all four are required; skip if any is absent
            // or uses an unsupported unit.
            let (Some(x_dim), Some(y_dim), Some(w_dim), Some(h_dim)) =
                (&ellipse.x, &ellipse.y, &ellipse.w, &ellipse.h)
            else {
                diagnostics.push(Diagnostic::advisory(
                    "scene.missing_geometry",
                    format!(
                        "ellipse '{}' is missing one or more geometry properties (x, y, w, h); \
                         skipped",
                        ellipse.id
                    ),
                    ellipse.source_span,
                    Some(ellipse.id.clone()),
                ));
                return;
            };

            let Some(x) = dim_to_px(x_dim.value, &x_dim.unit) else {
                diagnostics.push(unsupported_unit_diag(
                    "ellipse",
                    &ellipse.id,
                    "x",
                    ellipse.source_span,
                ));
                return;
            };
            let Some(y) = dim_to_px(y_dim.value, &y_dim.unit) else {
                diagnostics.push(unsupported_unit_diag(
                    "ellipse",
                    &ellipse.id,
                    "y",
                    ellipse.source_span,
                ));
                return;
            };
            let Some(w) = dim_to_px(w_dim.value, &w_dim.unit) else {
                diagnostics.push(unsupported_unit_diag(
                    "ellipse",
                    &ellipse.id,
                    "w",
                    ellipse.source_span,
                ));
                return;
            };
            let Some(h) = dim_to_px(h_dim.value, &h_dim.unit) else {
                diagnostics.push(unsupported_unit_diag(
                    "ellipse",
                    &ellipse.id,
                    "h",
                    ellipse.source_span,
                ));
                return;
            };

            // Resolve fill color.
            let Some(fill_prop) = &ellipse.fill else {
                // No fill → nothing to draw for a fill-only primitive.
                return;
            };
            let Some(mut color) =
                resolve_property_color(fill_prop, resolved, diagnostics, &ellipse.id)
            else {
                return;
            };

            // Apply opacity.
            if let Some(opacity) = ellipse.opacity {
                let o = opacity.clamp(0.0, 1.0);
                color.a = (color.a as f64 * o).round() as u8;
            }

            commands.push(SceneCommand::FillEllipse { x, y, w, h, color });
        }

        Node::Text(text) => {
            // Skip invisible text nodes.
            if text.visible == Some(false) {
                return;
            }

            // Resolve geometry — x and y are required; skip if absent or bad unit.
            let (Some(x_dim), Some(y_dim)) = (&text.x, &text.y) else {
                diagnostics.push(Diagnostic::advisory(
                    "scene.missing_geometry",
                    format!(
                        "text node '{}' is missing x or y geometry; skipped",
                        text.id
                    ),
                    text.source_span,
                    Some(text.id.clone()),
                ));
                return;
            };

            let Some(text_x) = dim_to_px(x_dim.value, &x_dim.unit) else {
                diagnostics.push(unsupported_unit_diag(
                    "text node",
                    &text.id,
                    "x",
                    text.source_span,
                ));
                return;
            };
            let Some(text_y) = dim_to_px(y_dim.value, &y_dim.unit) else {
                diagnostics.push(unsupported_unit_diag(
                    "text node",
                    &text.id,
                    "y",
                    text.source_span,
                ));
                return;
            };

            // Concatenate span text; skip silently if empty (nothing to draw).
            let content: String = text.spans.iter().map(|s| s.text.as_str()).collect();
            if content.is_empty() {
                return;
            }

            // Resolve font family.
            // Priority: font_family property → default "Noto Sans".
            let family_name: String = match &text.font_family {
                Some(PropertyValue::TokenRef(token_id)) => match resolved.get(token_id.as_str()) {
                    Some(rt) => match &rt.value {
                        ResolvedValue::FontFamily(name) => name.clone(),
                        _ => "Noto Sans".to_owned(),
                    },
                    None => "Noto Sans".to_owned(),
                },
                Some(PropertyValue::Literal(name)) => name.clone(),
                None => "Noto Sans".to_owned(),
            };
            let families = vec![family_name];

            // Resolve font size in pixels; default to 16.0 if absent.
            let font_size: f32 =
                resolve_property_dimension_px(&text.font_size, resolved, 16.0) as f32;

            // Resolve fill color; default to opaque black.
            let mut color = text
                .fill
                .as_ref()
                .and_then(|fp| resolve_property_color(fp, resolved, diagnostics, &text.id))
                .unwrap_or(Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                });

            // Apply opacity.
            if let Some(opacity) = text.opacity {
                let o = opacity.clamp(0.0, 1.0);
                color.a = (color.a as f64 * o).round() as u8;
            }

            // Shape the text.
            // Weight and style are hardcoded to 400/Normal — the TextNode AST
            // does not yet carry weight/style fields (future unit).
            let req = ShapeRequest {
                text: &content,
                families: &families,
                weight: 400,
                style: FontStyle::Normal,
                font_size,
            };

            match engine.shape(&req, fonts) {
                Err(e) => {
                    diagnostics.push(Diagnostic::advisory(
                        "scene.text_unshaped",
                        format!("text node '{}' could not be shaped: {}", text.id, e.message),
                        text.source_span,
                        Some(text.id.clone()),
                    ));
                }
                Ok(run) => {
                    let baseline_y = text_y + run.ascent as f64;
                    let glyphs: Vec<SceneGlyph> = run
                        .glyphs
                        .iter()
                        .map(|g| SceneGlyph {
                            glyph_id: g.glyph_id,
                            dx: g.x,
                            dy: g.y,
                        })
                        .collect();

                    commands.push(SceneCommand::DrawGlyphRun {
                        x: text_x,
                        y: baseline_y,
                        font_id: run.font_id,
                        font_size: run.font_size,
                        color,
                        glyphs,
                    });
                }
            }
        }

        Node::Line(line) => {
            // Skip invisible lines.
            if line.visible == Some(false) {
                return;
            }

            // Require all four endpoints; skip if any is absent or bad unit.
            let (Some(x1d), Some(y1d), Some(x2d), Some(y2d)) =
                (&line.x1, &line.y1, &line.x2, &line.y2)
            else {
                diagnostics.push(Diagnostic::advisory(
                    "scene.missing_geometry",
                    format!(
                        "line '{}' is missing one or more endpoint properties (x1, y1, x2, y2); \
                         skipped",
                        line.id
                    ),
                    line.source_span,
                    Some(line.id.clone()),
                ));
                return;
            };

            let Some(x1) = dim_to_px(x1d.value, &x1d.unit) else {
                diagnostics.push(unsupported_unit_diag(
                    "line",
                    &line.id,
                    "x1",
                    line.source_span,
                ));
                return;
            };
            let Some(y1) = dim_to_px(y1d.value, &y1d.unit) else {
                diagnostics.push(unsupported_unit_diag(
                    "line",
                    &line.id,
                    "y1",
                    line.source_span,
                ));
                return;
            };
            let Some(x2) = dim_to_px(x2d.value, &x2d.unit) else {
                diagnostics.push(unsupported_unit_diag(
                    "line",
                    &line.id,
                    "x2",
                    line.source_span,
                ));
                return;
            };
            let Some(y2) = dim_to_px(y2d.value, &y2d.unit) else {
                diagnostics.push(unsupported_unit_diag(
                    "line",
                    &line.id,
                    "y2",
                    line.source_span,
                ));
                return;
            };

            // Stroke is optional in validation, but a stroke-less line draws nothing.
            let Some(stroke_prop) = &line.stroke else {
                return;
            };
            let Some(mut color) =
                resolve_property_color(stroke_prop, resolved, diagnostics, &line.id)
            else {
                return;
            };

            // Apply opacity.
            if let Some(opacity) = line.opacity {
                let o = opacity.clamp(0.0, 1.0);
                color.a = (color.a as f64 * o).round() as u8;
            }

            // Resolve stroke_width to px; default 1.0 when absent.
            let stroke_width: f64 =
                resolve_property_dimension_px(&line.stroke_width, resolved, 1.0);

            commands.push(SceneCommand::StrokeLine {
                x1,
                y1,
                x2,
                y2,
                color,
                stroke_width,
            });
        }

        Node::Unknown(unknown) => {
            diagnostics.push(Diagnostic::advisory(
                "scene.unsupported_node",
                format!(
                    "unknown node kind '{}' cannot be compiled; the node is skipped \
                     (forward-compatibility: this kind may be supported in a later version)",
                    unknown.kind
                ),
                unknown.source_span,
                None,
            ));
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Convert a dimension value + unit to pixels.
///
/// Returns `None` for unsupported / unknown units (caller pushes advisory).
fn dim_to_px(value: f64, unit: &Unit) -> Option<f64> {
    match unit {
        Unit::Px => Some(value),
        Unit::Pt => Some(value * 96.0 / 72.0),
        Unit::Pct | Unit::Deg | Unit::Unknown(_) => None,
    }
}

/// Build a `scene.unsupported_unit` advisory for a named geometry field.
///
/// `kind` is the human-readable node kind ("rect" or "text node") used in the
/// diagnostic message.
fn unsupported_unit_diag(kind: &str, node_id: &str, field: &str, span: Option<Span>) -> Diagnostic {
    Diagnostic::advisory(
        "scene.unsupported_unit",
        format!(
            "{} '{}' field '{}' uses an unsupported unit; the {} is skipped",
            kind, node_id, field, kind
        ),
        span,
        Some(node_id.to_owned()),
    )
}

/// Resolve an optional dimension-valued property to pixels.
///
/// Returns `default` when the property is absent, is a raw literal, references
/// a non-dimension (or unresolved) token, or carries an unsupported unit. The
/// idiomatic path is a token ref resolving to a `Dimension`. Shared by
/// font-size and stroke-width resolution.
fn resolve_property_dimension_px(
    prop: &Option<PropertyValue>,
    resolved: &BTreeMap<String, ResolvedToken>,
    default: f64,
) -> f64 {
    match prop {
        Some(PropertyValue::TokenRef(token_id)) => match resolved.get(token_id.as_str()) {
            Some(rt) => match &rt.value {
                ResolvedValue::Dimension(dim) => dim_to_px(dim.value, &dim.unit).unwrap_or(default),
                _ => default,
            },
            None => default,
        },
        _ => default,
    }
}

/// Resolve a `PropertyValue` to a `Color`, or push a diagnostic and return
/// `None`.
///
/// Accepts:
/// - `TokenRef(id)` → looks up in `resolved`, must be a `ResolvedValue::Color`.
/// - `Literal(hex)` → parses as sRGB hex string directly.
fn resolve_property_color(
    prop: &PropertyValue,
    resolved: &BTreeMap<String, ResolvedToken>,
    diagnostics: &mut Vec<Diagnostic>,
    subject_id: &str,
) -> Option<Color> {
    match prop {
        PropertyValue::TokenRef(token_id) => {
            match resolved.get(token_id.as_str()) {
                Some(rt) => match &rt.value {
                    ResolvedValue::Color(hex) => match parse_srgb_hex(hex) {
                        Some(c) => Some(c),
                        None => {
                            // Should not happen — token resolution validates hex —
                            // but be robust.
                            diagnostics.push(Diagnostic::advisory(
                                "scene.invalid_color",
                                format!(
                                    "token '{}' resolved to '{}' which is not a valid \
                                     sRGB hex color; skipped",
                                    token_id, hex
                                ),
                                None,
                                Some(subject_id.to_owned()),
                            ));
                            None
                        }
                    },
                    other => {
                        diagnostics.push(Diagnostic::advisory(
                            "scene.wrong_token_type",
                            format!(
                                "node '{}' references token '{}' which resolved to a \
                                 non-color value ({:?}); skipped",
                                subject_id, token_id, other
                            ),
                            None,
                            Some(subject_id.to_owned()),
                        ));
                        None
                    }
                },
                None => {
                    diagnostics.push(Diagnostic::advisory(
                        "scene.unresolved_token",
                        format!(
                            "node '{}' references token '{}' which did not resolve \
                             (check token diagnostics); skipped",
                            subject_id, token_id
                        ),
                        None,
                        Some(subject_id.to_owned()),
                    ));
                    None
                }
            }
        }
        PropertyValue::Literal(hex) => match parse_srgb_hex(hex) {
            Some(c) => Some(c),
            None => {
                diagnostics.push(Diagnostic::advisory(
                    "scene.invalid_color",
                    format!(
                        "node '{}' has a fill literal '{}' that is not a valid \
                         sRGB hex color; skipped",
                        subject_id, hex
                    ),
                    None,
                    Some(subject_id.to_owned()),
                ));
                None
            }
        },
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource, default_provider};

    // ── Helper to parse a .zen source string ──────────────────────────────

    fn parse(src: &str) -> Document {
        KdlAdapter
            .parse(src.as_bytes())
            .expect("test document must parse")
    }

    // ── Minimal single-rect document ──────────────────────────────────────

    /// A page with a single full-page rect filled via a token color.
    /// Expected scene: PushClip → FillRect (bg from token) → FillRect (rect) → PopClip.
    /// In this test the page has no background, so background FillRect is absent.
    #[test]
    fn single_rect_token_fill_compiles_correctly() {
        let src = r##"zenith version=1 {
  project id="proj.t1" name="T1"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#f8fafc"
  }
  styles {}
  document id="doc.t1" title="T1" {
    page id="page.t1" w=(px)640 h=(px)360 {
      rect id="rect.t1" x=(px)0 y=(px)0 w=(px)640 h=(px)360 fill=(token)"color.fill"
    }
  }
}
"##;
        let doc = parse(src);
        let result = compile(&doc, &default_provider());

        assert!(
            result.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            result.diagnostics
        );

        let cmds = &result.scene.commands;
        // PushClip, FillRect, PopClip
        assert_eq!(cmds.len(), 3, "expected 3 commands, got: {:?}", cmds);

        assert!(
            matches!(cmds[0], SceneCommand::PushClip { x, y, w, h } if x == 0.0 && y == 0.0 && w == 640.0 && h == 360.0),
            "first command must be PushClip covering the page"
        );

        match &cmds[1] {
            SceneCommand::FillRect { x, y, w, h, color } => {
                assert_eq!(*x, 0.0);
                assert_eq!(*y, 0.0);
                assert_eq!(*w, 640.0);
                assert_eq!(*h, 360.0);
                // #f8fafc → r=0xf8=248, g=0xfa=250, b=0xfc=252, a=255
                assert_eq!(color.r, 0xf8);
                assert_eq!(color.g, 0xfa);
                assert_eq!(color.b, 0xfc);
                assert_eq!(color.a, 255);
            }
            other => panic!("expected FillRect, got {other:?}"),
        }

        assert!(
            matches!(cmds[2], SceneCommand::PopClip),
            "last command must be PopClip"
        );
    }

    // ── Two rects → two FillRects in source order ─────────────────────────

    #[test]
    fn two_rects_emitted_in_source_order() {
        let src = r##"zenith version=1 {
  project id="proj.t2" name="T2"
  tokens format="zenith-token-v1" {
    token id="color.a" type="color" value="#111111"
    token id="color.b" type="color" value="#222222"
  }
  styles {}
  document id="doc.t2" title="T2" {
    page id="page.t2" w=(px)100 h=(px)100 {
      rect id="rect.a" x=(px)0 y=(px)0 w=(px)50 h=(px)50 fill=(token)"color.a"
      rect id="rect.b" x=(px)50 y=(px)50 w=(px)50 h=(px)50 fill=(token)"color.b"
    }
  }
}
"##;
        let doc = parse(src);
        let result = compile(&doc, &default_provider());

        assert!(
            result.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            result.diagnostics
        );

        let cmds = &result.scene.commands;
        // PushClip, FillRect(a), FillRect(b), PopClip
        assert_eq!(cmds.len(), 4, "expected 4 commands, got: {:?}", cmds);

        match &cmds[1] {
            SceneCommand::FillRect { color, .. } => assert_eq!(color.r, 0x11),
            other => panic!("expected FillRect for rect.a, got {other:?}"),
        }
        match &cmds[2] {
            SceneCommand::FillRect { color, .. } => assert_eq!(color.r, 0x22),
            other => panic!("expected FillRect for rect.b, got {other:?}"),
        }
    }

    // ── visible=false rect is not emitted ─────────────────────────────────

    #[test]
    fn invisible_rect_not_emitted() {
        let src = r##"zenith version=1 {
  project id="proj.t3" name="T3"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#abcdef"
  }
  styles {}
  document id="doc.t3" title="T3" {
    page id="page.t3" w=(px)100 h=(px)100 {
      rect id="rect.hidden" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.fill" visible=#false
    }
  }
}
"##;
        let doc = parse(src);
        let result = compile(&doc, &default_provider());

        // No diagnostics expected (visible=false is a normal skip, not an error).
        assert!(
            result.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            result.diagnostics
        );

        let cmds = &result.scene.commands;
        // Only PushClip + PopClip; no FillRect.
        assert_eq!(
            cmds.len(),
            2,
            "expected PushClip + PopClip only; got: {:?}",
            cmds
        );
        assert!(matches!(cmds[0], SceneCommand::PushClip { .. }));
        assert!(matches!(cmds[1], SceneCommand::PopClip));
    }

    // ── JSON schema field is "zenith-scene-v1" ────────────────────────────

    #[test]
    fn json_schema_field_value() {
        let src = r##"zenith version=1 {
  project id="proj.t5" name="T5"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.t5" title="T5" {
    page id="page.t5" w=(px)100 h=(px)100 {}
  }
}
"##;
        let doc = parse(src);
        let result = compile(&doc, &default_provider());
        let json = result.scene.to_json().expect("serialize must succeed");
        assert!(
            json.contains(r#""schema": "zenith-scene-v1""#),
            "JSON must contain schema field; got snippet: {}",
            &json[..json.len().min(200)]
        );
    }

    // ── JSON determinism ──────────────────────────────────────────────────

    #[test]
    fn json_serialization_is_deterministic() {
        let src = r##"zenith version=1 {
  project id="proj.t6" name="T6"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#aabbcc"
  }
  styles {}
  document id="doc.t6" title="T6" {
    page id="page.t6" w=(px)200 h=(px)100 {
      rect id="rect.t6" x=(px)10 y=(px)20 w=(px)100 h=(px)50 fill=(token)"color.fill"
    }
  }
}
"##;
        let doc = parse(src);
        let r1 = compile(&doc, &default_provider());
        let r2 = compile(&doc, &default_provider());
        let j1 = r1.scene.to_json().expect("serialize 1");
        let j2 = r2.scene.to_json().expect("serialize 2");
        assert_eq!(
            j1, j2,
            "two compiles of the same doc must produce identical JSON"
        );
    }

    // ── Page background emitted as first FillRect ─────────────────────────

    #[test]
    fn page_background_emitted_before_children() {
        let src = r##"zenith version=1 {
  project id="proj.t7" name="T7"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.fill" type="color" value="#000000"
  }
  styles {}
  document id="doc.t7" title="T7" {
    page id="page.t7" w=(px)100 h=(px)100 background=(token)"color.bg" {
      rect id="rect.t7" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.fill"
    }
  }
}
"##;
        let doc = parse(src);
        let result = compile(&doc, &default_provider());

        assert!(
            result.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            result.diagnostics
        );

        let cmds = &result.scene.commands;
        // PushClip, FillRect(bg=white), FillRect(rect=black), PopClip
        assert_eq!(cmds.len(), 4, "expected 4 commands; got: {:?}", cmds);

        // Background fill must be white.
        match &cmds[1] {
            SceneCommand::FillRect { color, .. } => {
                assert_eq!(color.r, 255, "bg must be white");
                assert_eq!(color.g, 255);
                assert_eq!(color.b, 255);
            }
            other => panic!("expected background FillRect, got {other:?}"),
        }

        // Child rect must be black.
        match &cmds[2] {
            SceneCommand::FillRect { color, .. } => {
                assert_eq!(color.r, 0, "child rect must be black");
                assert_eq!(color.g, 0);
                assert_eq!(color.b, 0);
            }
            other => panic!("expected child FillRect, got {other:?}"),
        }
    }

    // ── Opacity multiplied into alpha ─────────────────────────────────────

    #[test]
    fn opacity_applied_to_fill_alpha() {
        // A full-alpha color (#ffffff, a=255) with opacity=0.5 → a≈128.
        let src = r##"zenith version=1 {
  project id="proj.t8" name="T8"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#ffffff"
  }
  styles {}
  document id="doc.t8" title="T8" {
    page id="page.t8" w=(px)100 h=(px)100 {
      rect id="rect.t8" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.fill" opacity=0.5
    }
  }
}
"##;
        let doc = parse(src);
        let result = compile(&doc, &default_provider());
        assert!(
            result.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            result.diagnostics
        );

        match &result.scene.commands[1] {
            SceneCommand::FillRect { color, .. } => {
                // 255 * 0.5 = 127.5 → rounds to 128.
                assert_eq!(color.a, 128, "opacity 0.5 must give a=128; got {}", color.a);
            }
            other => panic!("expected FillRect, got {other:?}"),
        }
    }

    // ── Text node with token-resolved fill/font/size → DrawGlyphRun ───────

    #[test]
    fn text_node_token_resolved_compiles_to_draw_glyph_run() {
        // A page with a text node whose fill, font-family, and font-size all
        // reference tokens.  Shaping uses the bundled Noto Sans provider.
        let src = r##"zenith version=1 {
  project id="proj.tx1" name="TX1"
  tokens format="zenith-token-v1" {
    token id="color.ink"     type="color"      value="#111827"
    token id="font.body"     type="fontFamily" value="Noto Sans"
    token id="size.body"     type="dimension"  value=(px)24
  }
  styles {}
  document id="doc.tx1" title="TX1" {
    page id="page.tx1" w=(px)400 h=(px)200 {
      text id="label.tx1" x=(px)10 y=(px)20 w=(px)380 h=(px)40 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
        span "Hello Zenith"
      }
    }
  }
}
"##;
        let doc = parse(src);
        let result = compile(&doc, &default_provider());

        // No shaping errors expected.
        let unshaped: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code == "scene.text_unshaped")
            .collect();
        assert!(
            unshaped.is_empty(),
            "no text_unshaped diagnostics expected; got: {:?}",
            result.diagnostics
        );

        // Commands: PushClip, DrawGlyphRun, PopClip.
        let cmds = &result.scene.commands;
        assert_eq!(cmds.len(), 3, "expected 3 commands; got: {:?}", cmds);
        assert!(matches!(cmds[0], SceneCommand::PushClip { .. }));
        assert!(matches!(cmds[2], SceneCommand::PopClip));

        match &cmds[1] {
            SceneCommand::DrawGlyphRun {
                x,
                y,
                font_id,
                font_size,
                color,
                glyphs,
            } => {
                // x is the text-box origin x.
                assert_eq!(*x, 10.0, "x must be text-box origin (10px)");
                // y is baseline = text_y + ascent; ascent > 0, so y > 20.0.
                assert!(*y > 20.0, "baseline y must be > text_y (20px); got {}", y);
                // font_id must be the stable Noto Sans id.
                assert_eq!(
                    font_id, "noto-sans-400-normal",
                    "font_id must be noto-sans-400-normal"
                );
                assert_eq!(*font_size, 24.0, "font_size must be 24px");
                // Fill color: #111827 → r=0x11=17, g=0x18=24, b=0x27=39.
                assert_eq!(color.r, 0x11, "color.r must be 0x11");
                assert_eq!(color.g, 0x18, "color.g must be 0x18");
                assert_eq!(color.b, 0x27, "color.b must be 0x27");
                assert_eq!(color.a, 255, "color.a must be 255 (opaque)");
                // Glyph run must be non-empty.
                assert!(
                    !glyphs.is_empty(),
                    "glyphs must be non-empty for 'Hello Zenith'"
                );
            }
            other => panic!("expected DrawGlyphRun, got {other:?}"),
        }
    }

    // ── Rect then text → FillRect before DrawGlyphRun (z-order) ──────────

    #[test]
    fn rect_then_text_z_order_preserved() {
        let src = r##"zenith version=1 {
  project id="proj.tx2" name="TX2"
  tokens format="zenith-token-v1" {
    token id="color.bg"  type="color"      value="#ffffff"
    token id="color.ink" type="color"      value="#000000"
    token id="font.body" type="fontFamily" value="Noto Sans"
    token id="size.body" type="dimension"  value=(px)16
  }
  styles {}
  document id="doc.tx2" title="TX2" {
    page id="page.tx2" w=(px)400 h=(px)200 {
      rect id="bg.rect" x=(px)0 y=(px)0 w=(px)400 h=(px)200 fill=(token)"color.bg"
      text id="label.tx2" x=(px)10 y=(px)20 w=(px)380 h=(px)40 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
        span "Hello"
      }
    }
  }
}
"##;
        let doc = parse(src);
        let result = compile(&doc, &default_provider());

        let cmds = &result.scene.commands;
        // PushClip, FillRect, DrawGlyphRun, PopClip
        assert_eq!(cmds.len(), 4, "expected 4 commands; got: {:?}", cmds);
        assert!(
            matches!(cmds[1], SceneCommand::FillRect { .. }),
            "second command must be FillRect (rect comes first)"
        );
        assert!(
            matches!(cmds[2], SceneCommand::DrawGlyphRun { .. }),
            "third command must be DrawGlyphRun (text comes after rect)"
        );
    }

    // ── Scene JSON of text contains DrawGlyphRun op + font_id, no byte arrays ─

    #[test]
    fn scene_json_draw_glyph_run_op_and_font_id_no_bytes() {
        let src = r##"zenith version=1 {
  project id="proj.tx3" name="TX3"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color"      value="#333333"
    token id="font.body" type="fontFamily" value="Noto Sans"
    token id="size.body" type="dimension"  value=(px)18
  }
  styles {}
  document id="doc.tx3" title="TX3" {
    page id="page.tx3" w=(px)300 h=(px)100 {
      text id="label.tx3" x=(px)0 y=(px)0 w=(px)300 h=(px)50 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
        span "Hi"
      }
    }
  }
}
"##;
        let doc = parse(src);
        let result = compile(&doc, &default_provider());

        let j1 = result.scene.to_json().expect("serialize 1");
        let j2 = result.scene.to_json().expect("serialize 2");

        // Must contain the op tag.
        assert!(
            j1.contains(r#""op": "DrawGlyphRun""#),
            "JSON must contain DrawGlyphRun op; snippet: {}",
            &j1[..j1.len().min(500)]
        );
        // Must contain the font_id string.
        assert!(
            j1.contains("noto-sans-400-normal"),
            "JSON must contain font_id; snippet: {}",
            &j1[..j1.len().min(500)]
        );
        // Must NOT contain a large byte array (no font bytes in IR).
        // Large byte arrays appear as `[1, 2, 3, ...]` with > ~50 numbers.
        // A simple heuristic: no run of more than 10 consecutive numbers separated by ", ".
        // We check that the JSON does not contain "bytes" as a key.
        assert!(
            !j1.contains(r#""bytes""#),
            "JSON must not contain a 'bytes' field; font bytes must not appear in the IR"
        );
        // Determinism: two serializations must be identical.
        assert_eq!(j1, j2, "two serializations must be identical (determinism)");
    }

    // ── Unresolvable font → text_unshaped advisory, no DrawGlyphRun ──────

    #[test]
    fn unresolvable_font_family_produces_text_unshaped_advisory() {
        let src = r##"zenith version=1 {
  project id="proj.tx4" name="TX4"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.tx4" title="TX4" {
    page id="page.tx4" w=(px)200 h=(px)100 {
      text id="label.tx4" x=(px)0 y=(px)0 w=(px)200 h=(px)50 fill="#000000" font-family="Nonexistent" {
        span "test"
      }
    }
  }
}
"##;
        let doc = parse(src);
        let result = compile(&doc, &default_provider());

        // Must have exactly one advisory with code "scene.text_unshaped".
        let unshaped: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code == "scene.text_unshaped")
            .collect();
        assert_eq!(
            unshaped.len(),
            1,
            "expected 1 text_unshaped advisory; got: {:?}",
            result.diagnostics
        );

        // No DrawGlyphRun emitted.
        let glyph_cmds: Vec<_> = result
            .scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }))
            .collect();
        assert!(
            glyph_cmds.is_empty(),
            "no DrawGlyphRun expected when font is unresolvable; got: {:?}",
            glyph_cmds
        );
    }

    // ── Ellipse: token fill compiles to FillEllipse ───────────────────────

    #[test]
    fn single_ellipse_token_fill_compiles_correctly() {
        let src = r##"zenith version=1 {
  project id="proj.e1" name="E1"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#f8fafc"
  }
  styles {}
  document id="doc.e1" title="E1" {
    page id="page.e1" w=(px)640 h=(px)360 {
      ellipse id="ellipse.e1" x=(px)0 y=(px)0 w=(px)640 h=(px)360 fill=(token)"color.fill"
    }
  }
}
"##;
        let doc = parse(src);
        let result = compile(&doc, &default_provider());

        assert!(
            result.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            result.diagnostics
        );

        let cmds = &result.scene.commands;
        // PushClip, FillEllipse, PopClip
        assert_eq!(cmds.len(), 3, "expected 3 commands, got: {:?}", cmds);

        assert!(
            matches!(cmds[0], SceneCommand::PushClip { x, y, w, h } if x == 0.0 && y == 0.0 && w == 640.0 && h == 360.0),
            "first command must be PushClip covering the page"
        );

        match &cmds[1] {
            SceneCommand::FillEllipse { x, y, w, h, color } => {
                assert_eq!(*x, 0.0);
                assert_eq!(*y, 0.0);
                assert_eq!(*w, 640.0);
                assert_eq!(*h, 360.0);
                // #f8fafc → r=0xf8=248, g=0xfa=250, b=0xfc=252, a=255
                assert_eq!(color.r, 0xf8);
                assert_eq!(color.g, 0xfa);
                assert_eq!(color.b, 0xfc);
                assert_eq!(color.a, 255);
            }
            other => panic!("expected FillEllipse, got {other:?}"),
        }

        assert!(
            matches!(cmds[2], SceneCommand::PopClip),
            "last command must be PopClip"
        );
    }

    // ── Ellipse: visible=false not emitted ────────────────────────────────

    #[test]
    fn invisible_ellipse_not_emitted() {
        let src = r##"zenith version=1 {
  project id="proj.e2" name="E2"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#abcdef"
  }
  styles {}
  document id="doc.e2" title="E2" {
    page id="page.e2" w=(px)100 h=(px)100 {
      ellipse id="ellipse.hidden" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.fill" visible=#false
    }
  }
}
"##;
        let doc = parse(src);
        let result = compile(&doc, &default_provider());

        assert!(
            result.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            result.diagnostics
        );

        let cmds = &result.scene.commands;
        // Only PushClip + PopClip; no FillEllipse.
        assert_eq!(
            cmds.len(),
            2,
            "expected PushClip + PopClip only; got: {:?}",
            cmds
        );
        assert!(matches!(cmds[0], SceneCommand::PushClip { .. }));
        assert!(matches!(cmds[1], SceneCommand::PopClip));
    }

    // ── Line: token stroke compiles to StrokeLine ─────────────────────────

    #[test]
    fn single_line_token_stroke_compiles_correctly() {
        let src = r##"zenith version=1 {
  project id="proj.l1" name="L1"
  tokens format="zenith-token-v1" {
    token id="color.rule" type="color" value="#94a3b8"
    token id="size.stroke" type="dimension" value=(px)2
  }
  styles {}
  document id="doc.l1" title="L1" {
    page id="page.l1" w=(px)320 h=(px)200 {
      line id="line.divider" x1=(px)40 y1=(px)100 x2=(px)280 y2=(px)100 stroke=(token)"color.rule" stroke-width=(token)"size.stroke"
    }
  }
}
"##;
        let doc = parse(src);
        let result = compile(&doc, &default_provider());

        assert!(
            result.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            result.diagnostics
        );

        let cmds = &result.scene.commands;
        // PushClip, StrokeLine, PopClip
        assert_eq!(cmds.len(), 3, "expected 3 commands, got: {:?}", cmds);

        assert!(
            matches!(cmds[0], SceneCommand::PushClip { .. }),
            "first command must be PushClip"
        );

        match &cmds[1] {
            SceneCommand::StrokeLine {
                x1,
                y1,
                x2,
                y2,
                color,
                stroke_width,
            } => {
                assert_eq!(*x1, 40.0);
                assert_eq!(*y1, 100.0);
                assert_eq!(*x2, 280.0);
                assert_eq!(*y2, 100.0);
                // #94a3b8 → r=0x94=148, g=0xa3=163, b=0xb8=184
                assert_eq!(color.r, 0x94);
                assert_eq!(color.g, 0xa3);
                assert_eq!(color.b, 0xb8);
                assert_eq!(color.a, 255);
                // size.stroke = (px)2
                assert_eq!(*stroke_width, 2.0);
            }
            other => panic!("expected StrokeLine, got {other:?}"),
        }

        assert!(
            matches!(cmds[2], SceneCommand::PopClip),
            "last command must be PopClip"
        );
    }

    // ── Line: visible=false not emitted ──────────────────────────────────

    #[test]
    fn invisible_line_not_emitted() {
        let src = r##"zenith version=1 {
  project id="proj.l2" name="L2"
  tokens format="zenith-token-v1" {
    token id="color.rule" type="color" value="#94a3b8"
  }
  styles {}
  document id="doc.l2" title="L2" {
    page id="page.l2" w=(px)100 h=(px)100 {
      line id="line.hidden" x1=(px)0 y1=(px)50 x2=(px)100 y2=(px)50 stroke=(token)"color.rule" visible=#false
    }
  }
}
"##;
        let doc = parse(src);
        let result = compile(&doc, &default_provider());

        assert!(
            result.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            result.diagnostics
        );

        let cmds = &result.scene.commands;
        // Only PushClip + PopClip; no StrokeLine.
        assert_eq!(
            cmds.len(),
            2,
            "expected PushClip + PopClip only; got: {:?}",
            cmds
        );
        assert!(matches!(cmds[0], SceneCommand::PushClip { .. }));
        assert!(matches!(cmds[1], SceneCommand::PopClip));
    }
}
