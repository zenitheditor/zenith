//! Canonical node-kind list, one-line summaries, and full-node examples.

// ── Canonical kind list ───────────────────────────────────────────────────────

/// All authorable node kinds in their canonical KDL-name form.
///
/// `Unknown` is excluded: it is a forward-compat placeholder, not an authorable
/// kind. The list is sorted for deterministic output.
pub fn node_kinds() -> &'static [&'static str] {
    // Exhaustive correspondence is enforced by the `node_variant_count_exhaustive`
    // helper in the `#[cfg(test)]` drift-guard below: adding a new `Node` variant
    // without updating that match causes a compile error in the tests module.
    &[
        "code",
        "connector",
        "ellipse",
        "field",
        "footnote",
        "frame",
        "group",
        "image",
        "chart",
        "instance",
        "line",
        "light",
        "mesh",
        "path",
        "pattern",
        "polygon",
        "polyline",
        "rect",
        "shape",
        "table",
        "text",
        "toc",
    ]
}

// ── One-line summaries ────────────────────────────────────────────────────────

/// Return a one-line description of the named node kind, or `None` if the kind
/// is not recognised.
///
/// The `match` arm set here must stay exhaustive over `node_kinds()`. The
/// drift-guard test `node_summary_covers_every_node_kind` enforces that.
pub fn node_summary(kind: &str) -> Option<&'static str> {
    match kind {
        "rect" => Some("Rectangle with optional fill, stroke, and rounded corners."),
        "ellipse" => Some("Ellipse or circle with optional fill and stroke."),
        "line" => Some("Straight line segment between two endpoints."),
        "text" => Some("Multi-span text block with typography and layout properties."),
        "code" => Some("Monospace code block with syntax-theme highlighting."),
        "frame" => Some(
            "Container box. layout=\"row\"|\"column\"|\"grid\" places and sizes its children \
             (gap, padding, justify, align, wrap); children size with w/h=\"hug\"|\"fill\", \
             min/max-w/h, and leave the flow with position=\"absolute\".",
        ),
        "group" => Some("Transparent grouping container for related nodes."),
        "image" => Some("Raster or SVG image positioned within a bounding box."),
        "polygon" => Some("Closed polygon defined by an ordered vertex list."),
        "polyline" => Some("Open polyline defined by an ordered vertex list."),
        "path" => Some("Structured Bezier path defined by anchors and optional handles."),
        "instance" => Some("Reference to a master component, optionally with overrides."),
        "field" => Some("Editable variable-data text field bound to a named slot."),
        "footnote" => Some("Page-level footnote referenced by text span markers."),
        "toc" => Some("Table-of-contents placeholder resolved to text by the scene compiler."),
        "table" => Some("Structured data table with columns, rows, and cells."),
        "shape" => Some("Preset geometric shape with an optional text label."),
        "connector" => Some("Directed connector line between two anchor points on nodes."),
        "pattern" => Some("Procedural grid or scatter tiling of one motif node."),
        "chart" => Some(
            "Data-visualization chart (bar, line, area, sparkline, pie, donut) with inline series data.",
        ),
        "light" => Some("Effect node that emits a soft radial ambient light."),
        "mesh" => {
            Some("Effect node that emits a procedural orthographic or perspective grid mesh.")
        }
        _ => None,
    }
}

/// Two auto-layout frames: a wrapping row of hugging chips, and a column card
/// whose texts fill its width, with an absolute badge out of the flow.
const FRAME_EXAMPLE: &str = r#"frame id="chips" x=(px)40 y=(px)40 w=(px)360 layout="row" gap=(px)8 wrap=#true align="center" {
  frame id="chip.design" w="hug" h="hug" layout="row" padding-x=(px)12 padding-y=(px)6 fill=(token)"color.chip" radius=(token)"radius.pill" {
    text id="chip.design.label" font-size=(token)"size.label" fill=(token)"color.ink" {
      span "Design"
    }
  }
  frame id="chip.layout" w="hug" h="hug" layout="row" padding-x=(px)12 padding-y=(px)6 fill=(token)"color.chip" radius=(token)"radius.pill" {
    text id="chip.layout.label" font-size=(token)"size.label" fill=(token)"color.ink" {
      span "Layout"
    }
  }
}
frame id="card" x=(px)40 y=(px)120 w=(px)320 layout="column" gap=(token)"space.md" padding=(token)"space.lg" fill=(token)"color.surface" {
  text id="card.title" w="fill" font-size=(token)"size.title" fill=(token)"color.ink" {
    span "Card title"
  }
  text id="card.body" w="fill" font-size=(token)"size.label" fill=(token)"color.ink" {
    span "Body copy wraps to the card width."
  }
  rect id="card.badge" x=(px)284 y=(px)12 w=(px)24 h=(px)24 position="absolute" fill=(token)"color.chip"
}"#;

/// Return a minimal, syntactically correct full-node example for a node kind.
///
/// This is separate from [`crate::schema::node_content`]: leaf nodes have no
/// child-content descriptor, but still need an authoring example in CLI schema
/// output.
pub fn node_example(kind: &str) -> Option<&'static str> {
    match kind {
        "frame" => Some(FRAME_EXAMPLE),
        "light" => Some(
            "light id=\"bg.glow\" kind=\"ambient\" x=(%)85 y=(%)12 \
             radius=(token)\"size.glow\" color=(token)\"color.glow\" opacity=0.35",
        ),
        "mesh" => Some(
            "mesh id=\"bg.mesh\" kind=\"perspective\" x=(px)0 y=(px)0 w=(px)1920 h=(px)1080 \
             rows=7 columns=8 vanishing-x=(px)1260 vanishing-y=(px)-420 extend=(px)160 \
             stroke=(token)\"color.grid\" stroke-width=(token)\"stroke.hairline\" opacity=0.34",
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Node;

    /// Exhaustive match over every `Node` variant: the compile-time drift guard.
    ///
    /// When a new variant `Node::Foo(…)` is added:
    /// 1. The `match` here becomes non-exhaustive → **compile error**.
    /// 2. Developer adds a `Node::Foo(_) => 1` arm here.
    /// 3. The developer also updates `TOTAL_NODE_VARIANTS`.
    /// 4. The `assert_eq` in `node_summary_covers_every_node_kind` then fails,
    ///    prompting the developer to add `"foo"` to `node_kinds()` and `node_summary()`.
    ///
    /// This function is only ever referenced via a function pointer in the test
    /// body (never actually called); the pointer reference forces the compiler to
    /// type-check the exhaustive match.
    fn node_variant_count_exhaustive(node: &Node) -> usize {
        match node {
            Node::Rect(_) => 1,
            Node::Ellipse(_) => 1,
            Node::Line(_) => 1,
            Node::Text(_) => 1,
            Node::Code(_) => 1,
            Node::Frame(_) => 1,
            Node::Group(_) => 1,
            Node::Image(_) => 1,
            Node::Polygon(_) => 1,
            Node::Polyline(_) => 1,
            Node::Path(_) => 1,
            Node::Instance(_) => 1,
            Node::Field(_) => 1,
            Node::Footnote(_) => 1,
            Node::Toc(_) => 1,
            Node::Table(_) => 1,
            Node::Shape(_) => 1,
            Node::Connector(_) => 1,
            Node::Pattern(_) => 1,
            Node::Chart(_) => 1,
            Node::Light(_) => 1,
            Node::Mesh(_) => 1,
            // Unknown is intentionally excluded from the authorable kind list.
            // This arm is required for exhaustiveness; the count still returns 1
            // so the total reflects all variants (authorable + Unknown).
            Node::Unknown(_) => 1,
        }
    }

    /// Total number of `Node` variants as recorded in the exhaustive match above.
    ///
    /// This is the count returned by `node_variant_count_exhaustive` for any
    /// `Node`, summed across all variants — i.e. the total variant count.
    /// Updated by hand when a variant is added (compile error forces it).
    const TOTAL_NODE_VARIANTS: usize = 23; // 22 authorable + 1 Unknown

    #[test]
    fn node_summary_covers_every_node_kind() {
        // Cross-check: node_kinds() must have exactly TOTAL_NODE_VARIANTS − 1
        // entries (all variants except Unknown).
        let expected_authorable = TOTAL_NODE_VARIANTS - 1; // subtract Unknown
        assert_eq!(
            node_kinds().len(),
            expected_authorable,
            "node_kinds() has {} entries but the exhaustive Node match covers {} authorable \
             variants (plus Unknown). Update node_kinds() and node_summary() when adding a variant.",
            node_kinds().len(),
            expected_authorable,
        );

        // Suppress the "never used" lint on node_variant_count_exhaustive by
        // taking a function pointer — this forces the compiler to type-check the
        // fn's exhaustive match without calling it.
        let _guard: fn(&Node) -> usize = node_variant_count_exhaustive;

        // Every listed kind must have a summary.
        for kind in node_kinds() {
            assert!(
                node_summary(kind).is_some(),
                "node_summary(\"{kind}\") returned None — add a one-liner to node_summary()",
            );
        }
    }
}
