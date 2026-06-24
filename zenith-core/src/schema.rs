//! Static schema metadata for the authorable node kinds.
//!
//! Exposes the canonical list of node kinds, one-line summaries, and the
//! recognized attribute names for each kind. The attribute list is derived
//! directly from the parser's own `known_props_for_kind` table so the two
//! can never silently diverge.

use crate::parse::transform::known_props_for_kind;

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
        "instance",
        "line",
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
        "frame" => Some("Container that clips and positions its children within a fixed box."),
        "group" => Some("Transparent grouping container for related nodes."),
        "image" => Some("Raster or SVG image positioned within a bounding box."),
        "polygon" => Some("Closed polygon defined by an ordered vertex list."),
        "polyline" => Some("Open polyline defined by an ordered vertex list."),
        "instance" => Some("Reference to a master component, optionally with overrides."),
        "field" => Some("Editable variable-data text field bound to a named slot."),
        "footnote" => Some("Page-level footnote referenced by text span markers."),
        "toc" => Some("Table-of-contents placeholder resolved to text by the scene compiler."),
        "table" => Some("Structured data table with columns, rows, and cells."),
        "shape" => Some("Preset geometric shape with an optional text label."),
        "connector" => Some("Directed connector line between two anchor points on nodes."),
        "pattern" => Some("Procedural grid or scatter tiling of one motif node."),
        _ => None,
    }
}

// ── Attribute names ───────────────────────────────────────────────────────────

/// Return the recognized attribute names for the given node kind.
///
/// Derived from the parser's own known-props table (same source of truth as
/// the validator's "did you mean?" helper). Alias spellings (e.g. `stroke_width`
/// alongside `stroke-width`) are de-duplicated to their canonical kebab-case
/// form and the result is sorted for deterministic output.
///
/// Returns an empty slice for unrecognised kinds or kinds without a fixed
/// prop list (e.g. "cell", "row", "column").
pub fn node_attributes(kind: &str) -> Vec<&'static str> {
    // The parser's known-props table carries BOTH spellings of hyphenated
    // attributes (e.g. `stroke-width` and `stroke_width`) for lenient parsing.
    // For the schema surface we collapse each pair to its canonical kebab-case
    // form: map every raw name to the kebab spelling (preferring the interned
    // kebab entry when present), then sort + dedup for deterministic output.
    let raw = known_props_for_kind(kind);
    let mut out: Vec<&'static str> = raw
        .iter()
        .map(|&name| {
            let kebab = name.replace('_', "-");
            raw.iter().copied().find(|n| *n == kebab).unwrap_or(name)
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

// ── Drift-guard tests ─────────────────────────────────────────────────────────

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
            Node::Instance(_) => 1,
            Node::Field(_) => 1,
            Node::Footnote(_) => 1,
            Node::Toc(_) => 1,
            Node::Table(_) => 1,
            Node::Shape(_) => 1,
            Node::Connector(_) => 1,
            Node::Pattern(_) => 1,
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
    const TOTAL_NODE_VARIANTS: usize = 19; // 18 authorable + 1 Unknown

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

    #[test]
    fn node_attributes_nonempty_for_geometry_kinds() {
        // rect must include "fill", "x", and "w".
        let rect_attrs = node_attributes("rect");
        assert!(!rect_attrs.is_empty(), "rect attributes must not be empty");
        assert!(
            rect_attrs.contains(&"fill"),
            "rect attributes must contain \"fill\"; got: {:?}",
            rect_attrs
        );
        assert!(
            rect_attrs.contains(&"x"),
            "rect attributes must contain \"x\"; got: {:?}",
            rect_attrs
        );
        assert!(
            rect_attrs.contains(&"w"),
            "rect attributes must contain \"w\"; got: {:?}",
            rect_attrs
        );

        // text must include "x", "y", "w", "h".
        let text_attrs = node_attributes("text");
        assert!(!text_attrs.is_empty(), "text attributes must not be empty");
        assert!(
            text_attrs.contains(&"x"),
            "text attributes must contain \"x\"; got: {:?}",
            text_attrs
        );

        // pattern must include "kind" and "spacing".
        let pattern_attrs = node_attributes("pattern");
        assert!(
            !pattern_attrs.is_empty(),
            "pattern attributes must not be empty"
        );
        assert!(
            pattern_attrs.contains(&"kind"),
            "pattern attributes must contain \"kind\"; got: {:?}",
            pattern_attrs
        );
        assert!(
            pattern_attrs.contains(&"spacing"),
            "pattern attributes must contain \"spacing\"; got: {:?}",
            pattern_attrs
        );

        // frame must include "x", "y", "w", "h".
        let frame_attrs = node_attributes("frame");
        assert!(
            !frame_attrs.is_empty(),
            "frame attributes must not be empty"
        );
        assert!(
            frame_attrs.contains(&"x"),
            "frame attributes must contain \"x\"; got: {:?}",
            frame_attrs
        );
        assert!(
            frame_attrs.contains(&"w"),
            "frame attributes must contain \"w\"; got: {:?}",
            frame_attrs
        );
    }

    #[test]
    fn node_attributes_empty_for_unknown_kind() {
        assert!(
            node_attributes("not-a-real-kind").is_empty(),
            "unrecognised kinds must return an empty slice"
        );
    }
}
