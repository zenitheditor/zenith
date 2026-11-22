//! Convert an arbitrary vector SVG into a complete, editable, tokenized `.zen` [`Document`].
//!
//! Reuses the `usvg` geometry & Bezier conversion helpers from [`super::svg_native`]
//! so SVG primitives, arcs, and nested transforms become native [`Node::Path`] nodes
//! with deduplicated `color.svg.*` and `size.stroke.*` design tokens.

use std::collections::BTreeMap;

use usvg::{NodeKind, Paint, TreeParsing, Visibility};
use zenith_core::{
    Document, KdlAdapter, KdlSource, Node, PathNode, PropertyValue, Token, TokenLiteral, TokenType,
    TokenValue,
};

use crate::ProduceError;
use crate::svg_native::{Affine, path_subpaths, px, stroke_linecap, stroke_linejoin};

/// Options controlling SVG → `.zen` document conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgImportOptions {
    /// Identifier slug used for `proj.<slug>` and `doc.<slug>`.
    pub slug: String,
    /// Human-readable project and document title.
    pub name: String,
}

/// Output of [`svg_to_document`]: a valid, tokenized [`Document`] plus conversion metrics.
#[derive(Debug, Clone)]
pub struct SvgImportOutput {
    /// Complete `.zen` document ready for formatting, validation, editing, or rendering.
    pub document: Document,
    /// Imported canvas width in px.
    pub width: f64,
    /// Imported canvas height in px.
    pub height: f64,
    /// Number of SVG paths converted into native Zenith `path` nodes.
    pub paths_imported: usize,
    /// Number of visible SVG paths skipped (no supported solid fill/stroke or empty geometry).
    pub paths_skipped: usize,
    /// Number of unsupported non-vector SVG nodes skipped (embedded raster images or text).
    pub unsupported_nodes: usize,
    /// Number of deduplicated color tokens minted.
    pub color_tokens: usize,
    /// Number of deduplicated stroke-width dimension tokens minted.
    pub stroke_tokens: usize,
}

/// Mutable state accumulated during a deterministic pre-order SVG tree walk.
#[derive(Default)]
struct ImportState {
    /// Canonical hex string (`#rrggbb` or `#rrggbbaa`) -> token id (`color.svg.<hex>`).
    colors: BTreeMap<String, String>,
    /// Quantized stroke width (hundredths of px) -> `(token_id, rounded_px)`.
    strokes: BTreeMap<i64, (String, f64)>,
    nodes: Vec<Node>,
    paths_imported: usize,
    paths_skipped: usize,
    unsupported_nodes: usize,
}

impl ImportState {
    fn intern_color(&mut self, color: usvg::Color, opacity: f32) -> String {
        let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        let (hex_val, hex_id) = if alpha == 255 {
            let h = format!("{:02x}{:02x}{:02x}", color.red, color.green, color.blue);
            (format!("#{h}"), h)
        } else {
            let h = format!(
                "{:02x}{:02x}{:02x}{:02x}",
                color.red, color.green, color.blue, alpha
            );
            (format!("#{h}"), h)
        };
        self.colors
            .entry(hex_val)
            .or_insert_with(|| format!("color.svg.{hex_id}"))
            .clone()
    }

    fn intern_stroke_width(&mut self, width_px: f64) -> Option<String> {
        if !width_px.is_finite() || width_px <= 0.0 {
            return None;
        }
        let hundredths = (width_px * 100.0).round() as i64;
        if hundredths <= 0 {
            return None;
        }
        let rounded = hundredths as f64 / 100.0;
        let next_idx = self.strokes.len() + 1;
        let (token_id, _) = self
            .strokes
            .entry(hundredths)
            .or_insert_with(|| (format!("size.stroke.{next_idx}"), rounded));
        Some(token_id.clone())
    }
}

/// Convert vector SVG `bytes` into an editable, tokenized `.zen` [`Document`].
///
/// Solid fills and strokes are deduplicated into `color.svg.*` tokens, stroke
/// widths into `size.stroke.*` dimension tokens, and all visible paths into
/// native [`Node::Path`] nodes preserving Bezier curves, fill rules, line caps,
/// line joins, and group opacity/transforms.
///
/// # Errors
///
/// Returns [`ProduceError::SvgNative`] if `bytes` is not valid SVG or has
/// non-positive dimensions.
pub fn svg_to_document(
    bytes: &[u8],
    options: &SvgImportOptions,
) -> Result<SvgImportOutput, ProduceError> {
    let usvg_opts = usvg::Options::default();
    let tree = usvg::Tree::from_data(bytes, &usvg_opts)
        .map_err(|e| ProduceError::SvgNative(e.to_string()))?;

    let width = f64::from(tree.size.width());
    let height = f64::from(tree.size.height());
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(ProduceError::SvgNative(format!(
            "SVG has invalid canvas size {width}x{height}"
        )));
    }

    let mut state = ImportState::default();
    walk_node(&tree.root, Affine::identity(), 1.0, &mut state);

    let slug = sanitize_slug(&options.slug);
    let title = escape_kdl(&options.name);
    let w_round = (width * 100.0).round() / 100.0;
    let h_round = (height * 100.0).round() / 100.0;
    let skeleton = format!(
        "zenith version=1 {{\n  project id=\"proj.{slug}\" name=\"{title}\"\n  tokens format=\"zenith-token-v1\" {{}}\n  styles {{}}\n  document id=\"doc.{slug}\" title=\"{title}\" {{\n    page id=\"page.main\" w=(px){w_round} h=(px){h_round} {{}}\n  }}\n}}\n"
    );
    let mut doc = KdlAdapter
        .parse(skeleton.as_bytes())
        .map_err(|e| ProduceError::SvgNative(format!("internal document skeleton error: {e}")))?;

    for (hex, token_id) in &state.colors {
        doc.tokens.tokens.push(Token {
            id: token_id.clone(),
            token_type: TokenType::Color,
            value: TokenValue::Literal(TokenLiteral::String(hex.clone())),
            set: None,
            source_span: None,
        });
    }
    for (token_id, width_px) in state.strokes.values() {
        doc.tokens.tokens.push(Token {
            id: token_id.clone(),
            token_type: TokenType::Dimension,
            value: TokenValue::Literal(TokenLiteral::Dimension(px(*width_px))),
            set: None,
            source_span: None,
        });
    }
    if let Some(page) = doc.body.pages.first_mut() {
        page.children = state.nodes;
    }

    Ok(SvgImportOutput {
        document: doc,
        width: w_round,
        height: h_round,
        paths_imported: state.paths_imported,
        paths_skipped: state.paths_skipped,
        unsupported_nodes: state.unsupported_nodes,
        color_tokens: state.colors.len(),
        stroke_tokens: state.strokes.len(),
    })
}

fn walk_node(
    node: &usvg::Node,
    parent_to_scene: Affine,
    parent_opacity: f32,
    state: &mut ImportState,
) {
    let kind = node.borrow();
    let local_to_scene = parent_to_scene.then(Affine::from_usvg(kind.transform()));
    match &*kind {
        NodeKind::Group(group) => {
            let opacity = parent_opacity * group.opacity.get();
            for child in node.children() {
                walk_node(&child, local_to_scene, opacity, state);
            }
        }
        NodeKind::Path(path) => {
            if path.visibility != Visibility::Visible {
                return;
            }
            if let Some(zen_path) = convert_doc_path(path, local_to_scene, parent_opacity, state) {
                state.paths_imported += 1;
                state.nodes.push(Node::Path(zen_path));
            } else {
                state.paths_skipped += 1;
            }
        }
        NodeKind::Image(_) | NodeKind::Text(_) => {
            state.unsupported_nodes += 1;
        }
    }
}

fn convert_doc_path(
    path: &usvg::Path,
    transform: Affine,
    group_opacity: f32,
    state: &mut ImportState,
) -> Option<PathNode> {
    let fill_color = path.fill.as_ref().and_then(|fill| match fill.paint {
        Paint::Color(c) => Some((c, group_opacity * fill.opacity.get())),
        _ => None,
    });
    let stroke_spec = path.stroke.as_ref().and_then(|stroke| match stroke.paint {
        Paint::Color(c) => Some((
            c,
            group_opacity * stroke.opacity.get(),
            f64::from(stroke.width.get()) * transform.uniform_scale(),
        )),
        _ => None,
    });
    if fill_color.is_none() && stroke_spec.is_none() {
        return None;
    }
    let subpaths = path_subpaths(path, transform);
    if subpaths.is_empty() {
        return None;
    }

    let fill = fill_color.map(|(c, op)| PropertyValue::TokenRef(state.intern_color(c, op)));
    let (stroke, stroke_width) = match stroke_spec {
        Some((c, op, width_px)) => {
            let stroke_tok = PropertyValue::TokenRef(state.intern_color(c, op));
            let width_tok = state
                .intern_stroke_width(width_px)
                .map(PropertyValue::TokenRef);
            (Some(stroke_tok), width_tok)
        }
        None => (None, None),
    };

    let name = (!path.id.is_empty()).then(|| path.id.clone());
    let id = format!("svg.{}", state.paths_imported);

    Some(PathNode {
        id,
        name,
        role: None,
        closed: None,
        fill,
        stroke,
        stroke_width,
        stroke_alignment: None,
        stroke_linejoin: path.stroke.as_ref().and_then(stroke_linejoin),
        stroke_linecap: path.stroke.as_ref().and_then(stroke_linecap),
        stroke_miter_limit: None,
        fill_rule: path.fill.as_ref().and_then(|fill| match fill.rule {
            usvg::FillRule::NonZero => None,
            usvg::FillRule::EvenOdd => Some("evenodd".to_owned()),
        }),
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        style: None,
        anchors: Vec::new(),
        subpaths,
        source_span: None,
        unknown_props: BTreeMap::new(),
    })
}

fn sanitize_slug(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('-');
    if trimmed.is_empty() {
        "imported".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn escape_kdl(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{Diagnostic, validate};

    const SAMPLE_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="120" viewBox="0 0 200 120">
      <rect id="bg" x="0" y="0" width="200" height="120" fill="#1e3a5f"/>
      <g transform="translate(20, 10)" opacity="0.5">
        <circle id="dot" cx="40" cy="40" r="24" fill="#f59e0b" stroke="#1e3a5f" stroke-width="2.5"/>
      </g>
      <path d="M10 100 L190 100" fill="none" stroke="#1e3a5f" stroke-width="2.5" stroke-linecap="round"/>
    </svg>"##;

    #[test]
    fn converts_multicolor_svg_to_valid_deterministic_zen_document() {
        let opts = SvgImportOptions {
            slug: "Brand Logo!".to_owned(),
            name: "Brand Logo".to_owned(),
        };
        let out1 = svg_to_document(SAMPLE_SVG, &opts).expect("convert svg");
        let out2 = svg_to_document(SAMPLE_SVG, &opts).expect("convert svg twice");

        assert_eq!(out1.width, 200.0);
        assert_eq!(out1.height, 120.0);
        assert_eq!(out1.paths_imported, 3);
        assert_eq!(out1.paths_skipped, 0);
        assert_eq!(out1.unsupported_nodes, 0);
        // #1e3a5f (opaque, reused twice), #1e3a5f80 (50% opacity stroke), #f59e0b80 (50% opacity fill)
        assert_eq!(out1.color_tokens, 3);
        // 2.5px stroke width deduplicated across circle + line
        assert_eq!(out1.stroke_tokens, 1);

        let report = validate(&out1.document);
        assert!(
            !Diagnostic::has_errors(&report.diagnostics),
            "imported document must validate without errors: {:#?}",
            report.diagnostics
        );

        let formatted1 = KdlAdapter.format(&out1.document).expect("format 1");
        let formatted2 = KdlAdapter.format(&out2.document).expect("format 2");
        assert_eq!(formatted1, formatted2, "SVG import must be deterministic");

        // Round-trip parse must succeed cleanly.
        let reparsed = KdlAdapter
            .parse(&formatted1)
            .expect("reparse formatted .zen");
        assert_eq!(reparsed.body.pages[0].children.len(), 3);
    }

    #[test]
    fn rejects_invalid_svg() {
        let opts = SvgImportOptions {
            slug: "bad".to_owned(),
            name: "Bad".to_owned(),
        };
        assert!(svg_to_document(b"not valid svg", &opts).is_err());
    }
}
