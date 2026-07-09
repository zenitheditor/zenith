//! Attribute name lists (node + non-node surfaces) and agent-readable type hints.

use crate::parse::transform::PAGE_KNOWN_PROPS;
use crate::parse::transform::{ASSET_KNOWN_PROPS, DOCUMENT_KNOWN_PROPS, known_props_for_kind};

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
    // form via `dedupe_to_kebab`, then sort + dedup for deterministic output.
    dedupe_to_kebab(known_props_for_kind(kind))
}

// ── Non-node surface summaries ────────────────────────────────────────────────

/// One-line description of the `page` surface.
pub fn page_summary() -> &'static str {
    "Page declaration — geometry (w/h), margins, bleed, baseline grid, and workflow metadata."
}

/// One-line description of the `asset` surface.
pub fn asset_summary() -> &'static str {
    "Asset declaration (image/svg/font) — provenance including sha256 and AI-generation fields."
}

/// One-line description of the `document` surface (the root `zenith` node).
pub fn document_summary() -> &'static str {
    "Document root — colorspace, pagination, spread gutter, and document-level default margins."
}

// ── Non-node surface attribute lists ─────────────────────────────────────────

/// Return the recognized attribute names for a `page` node.
///
/// Derived from the parser's own `PAGE_KNOWN_PROPS` constant. Alias spellings
/// (e.g. `margin_inner` alongside `margin-inner`) are de-duplicated to their
/// canonical kebab-case form; the result is sorted for deterministic output.
pub fn page_attributes() -> Vec<&'static str> {
    dedupe_to_kebab(PAGE_KNOWN_PROPS)
}

/// Return the recognized attribute names for an `asset` declaration node.
///
/// Derived from the parser's own `ASSET_KNOWN_PROPS` constant, sorted and
/// de-duplicated for deterministic output.
pub fn asset_attributes() -> Vec<&'static str> {
    dedupe_to_kebab(ASSET_KNOWN_PROPS)
}

/// Return the recognized attribute names for the root `zenith` document node.
///
/// Derived from the parser's own `DOCUMENT_KNOWN_PROPS` constant. Alias
/// spellings (e.g. `doc_id` alongside `doc-id`) are de-duplicated to their
/// canonical kebab-case form; the result is sorted for deterministic output.
pub fn document_attributes() -> Vec<&'static str> {
    dedupe_to_kebab(DOCUMENT_KNOWN_PROPS)
}

// ── Attribute type hints ──────────────────────────────────────────────────────

/// Return a concise, agent-readable type hint for the named attribute on the
/// given node kind.
///
/// This is the accurate, kind-aware entry point.  For attributes whose type
/// depends on the node kind (primarily the paint/visual attributes) the hint
/// reflects what the validator actually enforces for that specific kind.
///
/// Use `attribute_type` for non-node surfaces (page, asset, document) where
/// the attribute name alone is sufficient.
///
/// The hint describes *what value to write*, not the Rust representation.
/// Categories used:
/// - `"px literal"` — bare number suffixed `px` (e.g. `x=100px`).
/// - `"token ref: <kind>"` — a token identifier from the token block (must
///   reference a declared token, never a raw literal; e.g. `fill="color.brand"`).
/// - `"f64 (0.0–1.0)"` — bare floating-point ratio.
/// - `"i64"` — integer.
/// - `"bool"` — `true` or `false`.
/// - `"string"` — arbitrary string.
/// - `"node id"` — the `id` of another node in the document.
/// - `"string (enum)"` — one of a fixed set of values (exact set confirmed in
///   the validator; use `zenith validate` for the authoritative list).
/// - `"enum: a|b|…"` — one of the explicitly-listed values.
///
/// Returns `"string"` as a safe fallback for anything not in the dictionary.
pub fn attribute_type_for_kind(kind: &str, name: &str) -> &'static str {
    attribute_type_for_kind_inner(kind, name, "string")
}

/// Return a concise, agent-readable type hint for the named attribute.
///
/// This is the kind-agnostic entry point intended for non-node surfaces (page,
/// asset, document) where the attribute name alone determines the type.  For
/// node attributes, prefer [`attribute_type_for_kind`] to get accurate
/// per-kind paint/visual hints.
///
/// The completeness drift test (`attribute_type_covers_all_known_attrs`) uses
/// the kind-agnostic path with the sentinel `"<unmapped>"` fallback.
pub fn attribute_type(name: &str) -> &'static str {
    // Non-node surfaces carry no fill/stroke, so the kind-agnostic path is
    // accurate for them.  We route through the kind-aware inner function with
    // an empty kind string; the paint-specific branch will fall through to the
    // generic arms.
    attribute_type_for_kind_inner("", name, "string")
}

/// Internal helper — kind-aware attribute type resolution.
///
/// `kind` is the canonical node-kind string (e.g. `"rect"`, `"text"`) or `""`
/// for non-node surfaces.  `fallback` is `"string"` for the public APIs and
/// `"<unmapped>"` in the completeness drift test.
fn attribute_type_for_kind_inner(kind: &str, name: &str, fallback: &'static str) -> &'static str {
    // ── Kind-specific overrides for paint/visual attributes ───────────────
    //
    // These must be checked first, before the generic arm, because the correct
    // token type varies by node kind.  Each entry is verified against the
    // validator's `VisualExpect` at the cited source location.
    //
    // Also covers enum attributes whose value set differs by node kind (e.g.
    // `kind` means different things on `shape` vs `pattern`).
    match (kind, name) {
        // fill: ColorOrGradient — rect (leaf.rs check_visual_props→shared.rs:804),
        //   ellipse (leaf.rs:218), polygon (special.rs:83), polyline (special.rs:213),
        //   pattern (pattern.rs:101→shared.rs:804).
        ("rect" | "ellipse" | "polygon" | "polyline" | "path" | "pattern" | "chart", "fill") => {
            "token ref: color/gradient"
        }
        // fill: Color — text (text.rs:113), shape (shape.rs:108), code (leaf.rs:561).
        // table fill is also Color (container.rs:304→312).
        ("text" | "shape" | "code" | "table", "fill") => "token ref: color",
        // stroke: Color on every node kind that has it — verified at:
        //   shared.rs:813 (rect/pattern), leaf.rs:227 (ellipse), leaf.rs:409 (line),
        //   special.rs:92 (polygon), special.rs:222 (polyline),
        //   text.rs:122, shape.rs:117, shape.rs:248 (connector).
        // There is no node kind where stroke accepts a gradient.
        (_, "stroke") => "token ref: color",
        ("image", "svg-stroke" | "svg-fill") => "token ref: color",
        // shadow / filter / mask: dedicated token types; NOT color/gradient.
        // Verified at shared.rs:960 (Shadow), 969 (Filter), 978 (Mask).
        // These are only present on kinds that go through check_visual_props
        // (rect, pattern) or equivalent, but the type is uniform across all kinds.
        (_, "shadow") => "token ref: shadow",
        (_, "filter") => "token ref: filter",
        (_, "mask") => "token ref: mask",
        // background: page surface only; accepts Color or Gradient (driver.rs:639).
        // The empty-kind non-node path also falls here for correctness.
        (_, "background") => "token ref: color/gradient",
        // Per-side border colors and stroke-outer: Color (shared.rs:887).
        (_, "border-top" | "border-bottom" | "border-left" | "border-right" | "stroke-outer") => {
            "token ref: color"
        }
        // border (table): Color (container.rs:305→312).
        (_, "border") => "token ref: color",
        // contrast-bg (text): Color (text.rs:139).
        // header-fill (table): Color (container.rs:306→312).
        (_, "contrast-bg" | "header-fill") => "token ref: color",
        // kind: the `kind` attribute means different things on different surfaces.
        //   shape: process/decision/terminator/ellipse (validate/check/nodes/node/shape.rs:152).
        //   pattern: grid/scatter (validate/check/nodes/node/pattern.rs:140).
        //   asset surface (kind=""): image/svg/font (ast/asset.rs:29-33).
        // The attribute name alone is insufficient — each surface has a distinct enum.
        ("shape", "kind") => "enum: process|decision|terminator|ellipse",
        ("pattern", "kind") => "enum: grid|scatter",
        ("chart", "kind") => "enum: bar|line|area|sparkline|pie|donut",
        ("light", "kind") => "enum: ambient|glow|key|rim",
        ("light", "color") => "token ref: color",
        ("light", "angle") => "dimension: deg",
        ("mesh", "kind") => "enum: orthographic|perspective",
        ("mesh", "stroke-width") | ("mesh", "stroke-dash") | ("mesh", "stroke-gap") => {
            "dimension literal or token ref: dimension"
        }
        ("mesh", "x")
        | ("mesh", "y")
        | ("mesh", "w")
        | ("mesh", "h")
        | ("mesh", "vanishing-x")
        | ("mesh", "vanishing-y")
        | ("mesh", "extend") => "dimension literal or token ref: dimension",
        ("mesh", "rows") | ("mesh", "columns") => "u32 (>0)",
        ("mesh", "stroke-linecap") => "enum: butt|round|square",
        ("path", "stroke-linejoin") => "enum: miter|round|bevel",
        ("path", "stroke-miter-limit") => "f64 (>0)",
        ("group", "symmetry-mode") => "enum: radial|mirror",
        // Import composition fit is contain/fill/none only (no `cover`); it scales
        // the imported subtree into the instance/page box.
        ("instance", "fit") | ("page", "fit") => "enum: contain|fill|none",
        ("import", "kind") => "enum: zen",
        ("token-map", "from") | ("token-map", "to") => "string",
        // chart axis/legend/caption/bar-mode/orientation/legend-position/legend-layout/legend-align: chart-only attributes (validate/check/nodes/node/chart.rs).
        ("chart", "legend") => "bool",
        ("chart", "caption") => "string",
        ("chart", "axis-min" | "axis-max") => "f64",
        ("chart", "axis-style") => "string",
        ("chart", "legend-position") => "enum: right|left|top|bottom",
        ("chart", "legend-layout") => "enum: wrapped|list",
        ("chart", "legend-align") => "enum: center|left|right",
        ("chart", "bar-mode") => "enum: grouped|stacked",
        ("chart", "orientation") => "enum: vertical|horizontal",
        ("chart", "point-placement") => "enum: edge|center",
        ("chart", "value-labels") => "enum: auto|none|top|center",
        ("chart", "value-color") => "token ref: color",
        // label-color is a named prop on series children; surfaces here for type-hint purposes.
        ("chart", "label-color") => "token ref: color",
        // Asset surface (non-node): kind="" is used by attribute_type() / the
        // completeness drift test for non-node attributes.
        ("", "kind") => "enum: image|svg|font",
        // route: connector-only; values validated at shape.rs:309 as
        //   straight/orthogonal/avoid (avoid is validated; maps to straight today).
        ("connector", "route") => "enum: straight|orthogonal|avoid",
        // layout: frame-only; AST documents absolute/flow/grid (container.rs:34-39).
        //   Validator only enforces grid semantics (advisory) but all three values are
        //   spec'd. Other values fall through to absolute-positioning.
        ("frame", "layout") => "enum: absolute|flow|grid",
        // All other attributes fall through to the generic arm below.
        _ => attribute_type_generic(name, fallback),
    }
}

/// Generic attribute type resolution — kind-independent properties.
///
/// Called by `attribute_type_for_kind_inner` for every attribute that is not
/// a paint/visual property requiring per-kind disambiguation.
fn attribute_type_generic(name: &str, fallback: &'static str) -> &'static str {
    match name {
        // ── Identity / labelling ──────────────────────────────────────────
        "id" => "string",
        "name" => "string",
        "role" => "string",
        "style" => "string",
        "source" => "string",

        // ── Geometry (px literals) ────────────────────────────────────────
        "x" | "y" | "w" | "h" => "px literal or token ref: dimension",
        "x1" | "y1" | "x2" | "y2" => "px literal",
        "rx" | "ry" => "px literal",
        "rotate" => "px literal",
        "spacing" => "px literal",
        "padding-left" | "text-indent" => "px literal",
        "bullet-gap" => "px literal",
        "anchor-gap" => "px literal",
        "blur" => "px literal",
        "bleed" => "px literal",
        "spread-gutter" => "px literal",
        "margin-inner" | "margin-outer" | "margin-top" | "margin-bottom" => "px literal",
        "symmetry-cx" | "symmetry-cy" => "px literal",
        "symmetry-start-angle" => "dimension: deg",

        // ── Visual — token refs: dimension ────────────────────────────────
        "radius" | "radius-tl" | "radius-tr" | "radius-br" | "radius-bl" => "token ref: dimension",
        "stroke-width" | "stroke-dash" | "stroke-gap" | "stroke-outer-width"
        | "svg-stroke-width" => "token ref: dimension",
        "border-width" => "token ref: dimension",
        "font-size" | "font-size-min" => "token ref: dimension",
        "baseline-grid" => "token ref: dimension",
        "gap" | "cell-padding" | "padding" => "token ref: dimension",
        // src-* image crop coords are px literals (geometry), not token refs.
        "src-x" | "src-y" | "src-w" | "src-h" => "px literal",
        // object-position values are f64 ratios, not token refs.
        "object-position-x" | "object-position-y" => "f64 (0.0–1.0)",
        // clip-radius is a token ref (same discipline as radius).
        "clip-radius" => "token ref: dimension",

        // ── Visual — token refs: font ─────────────────────────────────────
        "font-family" => "token ref: fontFamily",
        "font-weight" => "token ref: fontWeight",
        "font-features" => "OpenType feature list",
        "font-alternates" => "OpenType alternate list",
        "letter-spacing" | "tracking" => "token ref: dimension",

        // ── Floating-point ratios ─────────────────────────────────────────
        "opacity" | "jitter" | "intensity" => "f64 (0.0–1.0)",

        // ── Integers ─────────────────────────────────────────────────────
        "seed" | "count" => "i64",
        "drop-cap-lines" | "widow-orphan" | "tab-width" | "line-numbers" => "i64",
        "colspan" | "rowspan" | "header-rows" | "columns" | "rows" => "i64",
        "layer-priority" => "i64",
        "symmetry-count" => "u32 (1..=72)",

        // ── Booleans ─────────────────────────────────────────────────────
        "visible" | "locked" | "anchor-parent" | "selectable" | "closed" => "bool",
        "hyphenate" | "suppress-first" | "border-collapse" => "bool",
        "mirror-margins" | "facing-pages" => "bool",
        "line-jumps" => "bool",

        // ── Named enums (values confirmed in the validator) ───────────────
        "anchor" => {
            "enum: top-left|top-center|top-right|center-left|center|center-right|bottom-left|bottom-center|bottom-right"
        }
        "anchor-edge" => "enum: above|below|before|after",
        "align" => "enum: left|center|right|justify",
        "overflow" => "enum: clip|visible|scroll",
        "blend-mode" => {
            "enum: normal|multiply|screen|overlay|darken|lighten|color-dodge|color-burn|hard-light|soft-light|difference|exclusion|hue|saturation|color|luminosity"
        }
        "stroke-alignment" => "enum: inside|center|outside",
        "stroke-linecap" => "enum: butt|round|square",
        "fill-rule" => "enum: nonzero|evenodd",
        "fit" => "enum: fill|contain|cover|none",
        "clip" => "bool",
        "parity" => "enum: left|right",
        "page-parity-start" => "enum: left|right",
        "page-progression" => "enum: ltr|rtl",
        "colorspace" => "enum: srgb|display-p3|rec2020",
        "direction" => "enum: ltr|rtl",
        "overflow-wrap" => "enum: normal|break-word",
        "h-align" => "enum: left|center|right",
        "v-align" => "enum: top|middle|bottom",

        // ── Connector-specific ────────────────────────────────────────────
        "from" | "to" => "node id or node#port",
        "from-anchor" | "to-anchor" => {
            "enum/string: auto|grid anchor|divided anchor i/N (0 <= i < N)"
        }
        "marker-start" | "marker-end" => "string",

        // ── Text-specific strings ─────────────────────────────────────────
        // chain: shared id string (NOT a node id — it is a user-chosen label that
        // groups text nodes into a threaded flow). All text nodes with the same
        // chain value form one chain; the first span-bearing member is the content
        // source and the rest are empty continuation boxes. See `zenith schema node text`.
        "chain" => "string (chain id)",
        "tab-leader" | "text-exclusion" | "bullet" => "string",
        "language" => "string",
        "syntax-theme" => "string",

        // ── Data binding (on a `span`) ────────────────────────────────────
        "data-ref" => "string",
        "format" => "enum: currency|percent|number",
        "precision" => "i64",
        "locale" => "string",

        // ── Field / TOC / Footnote ────────────────────────────────────────
        "type" => "string",
        "recto" | "verso" | "target" => "string",
        "folio-style" | "header-style" | "text-style" => "string",
        "marker" => "string",
        "match-role" | "match-style" | "leader" => "string",
        "flows" => "string",

        // ── Image ─────────────────────────────────────────────────────────
        "asset" => "asset id",

        // ── Instance / component ──────────────────────────────────────────
        "component" => "string",

        // ── Group semantic extras ─────────────────────────────────────────
        "semantic-role" => "string",

        // ── Page workflow metadata ────────────────────────────────────────
        "master" => "string",

        // ── Document root ─────────────────────────────────────────────────
        "version" => "string",
        "doc-id" => "string",
        "title" => "string",

        // ── Asset provenance ──────────────────────────────────────────────
        "src" => "string",
        "sha256" => "string",
        "producer-kind" => "string",
        "producer-source" => "string",
        "ai-prompt" => "string",
        "ai-model" => "string",
        "ai-provider" => "string",
        "ai-seed" => "i64",
        "ai-generation-date" => "string",
        "ai-license" => "string",
        "ai-source-rights" => "string",
        "ai-safety-status" => "string",
        "ai-reuse-policy" => "string",

        // ── Anchor zone ───────────────────────────────────────────────────
        "anchor-zone" | "anchor-sibling" => "string",

        _ => fallback,
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Collapse a raw known-props slice (which may contain both `foo-bar` and
/// `foo_bar` spellings) to sorted, deduplicated kebab-case names.
///
/// For every raw name: map underscores to hyphens to get the kebab form; then
/// find the first entry in the slice that exactly equals that kebab string.
/// If found, use that interned static str; otherwise keep the raw entry as-is.
/// After collecting, sort and dedup.
fn dedupe_to_kebab(raw: &'static [&'static str]) -> Vec<&'static str> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::node_kinds;

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

    // ── Non-node surface drift guards ─────────────────────────────────────────

    /// Anchor check: `page_attributes()` must be non-empty and contain the
    /// key geometry and workflow attrs we know the parser reads. This ensures
    /// `PAGE_KNOWN_PROPS` is not accidentally emptied or truncated.
    #[test]
    fn page_attributes_anchor_check() {
        let attrs = page_attributes();
        assert!(!attrs.is_empty(), "page_attributes() must not be empty");
        for anchor in &["w", "h", "line-jumps"] {
            assert!(
                attrs.contains(anchor),
                "page_attributes() must contain \"{anchor}\"; got: {attrs:?}",
            );
        }
        // Alias spellings must be collapsed: only the kebab form should appear.
        assert!(
            !attrs.contains(&"line_jumps"),
            "underscore alias \"line_jumps\" must be collapsed; got: {attrs:?}",
        );
    }

    /// Anchor check: `asset_attributes()` must be non-empty and contain the
    /// provenance fields the parser reads.
    #[test]
    fn asset_attributes_anchor_check() {
        let attrs = asset_attributes();
        assert!(!attrs.is_empty(), "asset_attributes() must not be empty");
        for anchor in &["sha256", "ai-prompt", "ai-model", "src", "kind"] {
            assert!(
                attrs.contains(anchor),
                "asset_attributes() must contain \"{anchor}\"; got: {attrs:?}",
            );
        }
    }

    /// Anchor check: `document_attributes()` must be non-empty and contain the
    /// root-node fields the parser reads.
    #[test]
    fn document_attributes_anchor_check() {
        let attrs = document_attributes();
        assert!(!attrs.is_empty(), "document_attributes() must not be empty");
        for anchor in &["title", "colorspace", "doc-id", "spread-gutter"] {
            assert!(
                attrs.contains(anchor),
                "document_attributes() must contain \"{anchor}\"; got: {attrs:?}",
            );
        }
        // Alias spellings must be collapsed: only the kebab form should appear.
        assert!(
            !attrs.contains(&"doc_id"),
            "underscore alias \"doc_id\" must be collapsed; got: {attrs:?}",
        );
    }

    // ── Attribute type completeness drift guard ───────────────────────────

    /// Every attribute returned by any of the four public attribute-list
    /// functions must have an explicit entry in `attribute_type_for_kind_inner`
    /// or `attribute_type_generic` — not just the silent `"string"` fallback.
    ///
    /// When a new attribute is added to a KNOWN_PROPS constant, this test
    /// fails with a list of unmapped names, forcing the developer to add a
    /// corresponding arm to the appropriate function.
    ///
    /// The sentinel `"<unmapped>"` is used here instead of `"string"` so the
    /// test can distinguish "no entry at all" from a deliberate `"string"`
    /// annotation on reference/metadata fields.
    ///
    /// For node attributes the test probes with the first kind that lists the
    /// attribute; the completeness check just needs at least one mapped path.
    /// The per-kind accuracy tests below verify the per-kind correctness.
    #[test]
    fn attribute_type_covers_all_known_attrs() {
        use std::collections::BTreeMap;
        use std::collections::BTreeSet;

        // Build a map from each attribute name to the first kind that lists it
        // (so we can probe with a real kind).
        let mut attr_to_kind: BTreeMap<&'static str, &'static str> = BTreeMap::new();
        for &kind in node_kinds() {
            for attr in node_attributes(kind) {
                attr_to_kind.entry(attr).or_insert(kind);
            }
        }

        // Non-node surface attributes: probe with empty kind (kind-agnostic path).
        let mut surface_attrs: BTreeSet<&'static str> = BTreeSet::new();
        for attr in page_attributes() {
            surface_attrs.insert(attr);
        }
        for attr in asset_attributes() {
            surface_attrs.insert(attr);
        }
        for attr in document_attributes() {
            surface_attrs.insert(attr);
        }

        // Collect any attribute whose type resolves to the unmapped sentinel.
        let mut unmapped: Vec<String> = Vec::new();

        for (attr, kind) in &attr_to_kind {
            if attribute_type_for_kind_inner(kind, attr, "<unmapped>") == "<unmapped>" {
                unmapped.push(format!("{attr} (on {kind})"));
            }
        }
        for attr in &surface_attrs {
            // Only probe surface-only attrs (those not already covered via a node kind).
            if !attr_to_kind.contains_key(attr)
                && attribute_type_for_kind_inner("", attr, "<unmapped>") == "<unmapped>"
            {
                unmapped.push(format!("{attr} (surface)"));
            }
        }

        assert!(
            unmapped.is_empty(),
            "attribute_type_for_kind_inner() has no entry for {} attribute(s): {:?}\n\
             Add an arm to `attribute_type_for_kind_inner` or `attribute_type_generic` \
             in zenith-core/src/schema.rs.",
            unmapped.len(),
            unmapped,
        );
    }

    // ── Paint-attribute accuracy tests ────────────────────────────────────────

    /// `fill` must report color/gradient for geometry kinds and color-only for
    /// text, shape, and code — matching what the validator enforces via
    /// `VisualExpect`.
    #[test]
    fn fill_type_hint_is_kind_accurate() {
        // ColorOrGradient kinds: rect (shared.rs:804), ellipse (leaf.rs:218),
        // polygon (special.rs:83), polyline (special.rs:213), path (special.rs),
        // pattern (pattern.rs:101).
        for kind in &["rect", "ellipse", "polygon", "polyline", "path", "pattern"] {
            assert_eq!(
                attribute_type_for_kind(kind, "fill"),
                "token ref: color/gradient",
                "fill on {kind} should accept color/gradient (validator uses ColorOrGradient)",
            );
        }
        // Color-only kinds: text (text.rs:113), shape (shape.rs:108), code (leaf.rs:561),
        // table (container.rs:304).
        for kind in &["text", "shape", "code", "table"] {
            assert_eq!(
                attribute_type_for_kind(kind, "fill"),
                "token ref: color",
                "fill on {kind} should be color-only (validator uses VisualExpect::Color)",
            );
        }
    }

    /// `stroke` is Color on every kind that has it — never color/gradient.
    /// Verified at shared.rs:813, leaf.rs:227/409, special.rs:92/222,
    /// text.rs:122, shape.rs:117/248.
    #[test]
    fn stroke_type_hint_is_color_only() {
        for kind in &[
            "rect",
            "ellipse",
            "line",
            "polygon",
            "polyline",
            "path",
            "pattern",
            "text",
            "shape",
            "connector",
        ] {
            assert_eq!(
                attribute_type_for_kind(kind, "stroke"),
                "token ref: color",
                "stroke on {kind} must be color-only (validator uses VisualExpect::Color)",
            );
        }
    }

    /// `shadow`, `filter`, and `mask` must each report their own dedicated token
    /// type — NOT color or color/gradient.
    /// Verified at shared.rs:960 (Shadow), 969 (Filter), 978 (Mask).
    #[test]
    fn shadow_filter_mask_report_own_token_types() {
        for kind in &["rect", "pattern"] {
            assert_eq!(
                attribute_type_for_kind(kind, "shadow"),
                "token ref: shadow",
                "shadow on {kind} must reference a shadow token",
            );
            assert_eq!(
                attribute_type_for_kind(kind, "filter"),
                "token ref: filter",
                "filter on {kind} must reference a filter token",
            );
            assert_eq!(
                attribute_type_for_kind(kind, "mask"),
                "token ref: mask",
                "mask on {kind} must reference a mask token",
            );
        }
        // Verify the kind-agnostic public API also gives the right answer.
        assert_eq!(attribute_type("shadow"), "token ref: shadow");
        assert_eq!(attribute_type("filter"), "token ref: filter");
        assert_eq!(attribute_type("mask"), "token ref: mask");
    }

    #[test]
    fn container_effect_attributes_are_discoverable() {
        for kind in &["group", "frame"] {
            let attrs = node_attributes(kind);
            for attr in &["shadow", "filter", "mask", "blur", "blend-mode"] {
                assert!(
                    attrs.contains(attr),
                    "{kind} schema must include {attr}; attrs: {attrs:?}"
                );
            }
            assert_eq!(attribute_type_for_kind(kind, "shadow"), "token ref: shadow");
            assert_eq!(attribute_type_for_kind(kind, "filter"), "token ref: filter");
            assert_eq!(attribute_type_for_kind(kind, "mask"), "token ref: mask");
        }
    }

    #[test]
    fn group_live_symmetry_attributes_are_discoverable() {
        let attrs = node_attributes("group");
        for attr in &[
            "symmetry-count",
            "symmetry-cx",
            "symmetry-cy",
            "symmetry-start-angle",
            "symmetry-mode",
        ] {
            assert!(
                attrs.contains(attr),
                "group schema must include {attr}; attrs: {attrs:?}"
            );
        }
        assert_eq!(
            attribute_type_for_kind("group", "symmetry-count"),
            "u32 (1..=72)"
        );
        assert_eq!(
            attribute_type_for_kind("group", "symmetry-cx"),
            "px literal"
        );
        assert_eq!(
            attribute_type_for_kind("group", "symmetry-cy"),
            "px literal"
        );
        assert_eq!(
            attribute_type_for_kind("group", "symmetry-start-angle"),
            "dimension: deg"
        );
        assert_eq!(
            attribute_type_for_kind("group", "symmetry-mode"),
            "enum: radial|mirror"
        );
    }

    /// `background` (page surface) reports color/gradient — driver.rs:639 uses
    /// VisualExpect::ColorOrGradient.
    #[test]
    fn background_type_hint_is_color_or_gradient() {
        assert_eq!(attribute_type("background"), "token ref: color/gradient");
        assert_eq!(
            attribute_type_for_kind("", "background"),
            "token ref: color/gradient"
        );
    }

    /// Border/stroke-outer type hints are color-only — shared.rs:887 uses
    /// VisualExpect::Color for all per-side border props.
    #[test]
    fn border_and_stroke_outer_are_color_only() {
        for attr in &[
            "border-top",
            "border-bottom",
            "border-left",
            "border-right",
            "stroke-outer",
        ] {
            assert_eq!(
                attribute_type_for_kind("rect", attr),
                "token ref: color",
                "{attr} on rect must be color-only (validator uses VisualExpect::Color)",
            );
        }
    }

    // ── kind / route / layout node-aware accuracy tests ──────────────────────

    /// `kind` must report node-specific enum strings per kind.
    ///
    /// shape.kind: process/decision/terminator/ellipse (shape.rs:152).
    /// pattern.kind: grid/scatter (pattern.rs:140).
    /// These must NOT cross-contaminate: the same attribute name, different enums.
    #[test]
    fn kind_type_hint_is_node_aware() {
        assert_eq!(
            attribute_type_for_kind("shape", "kind"),
            "enum: process|decision|terminator|ellipse",
            "shape.kind must enumerate the flowchart shape variants",
        );
        assert_eq!(
            attribute_type_for_kind("pattern", "kind"),
            "enum: grid|scatter",
            "pattern.kind must enumerate the tiling mode variants",
        );
        // The two must differ — this is the root bug being fixed.
        assert_ne!(
            attribute_type_for_kind("shape", "kind"),
            attribute_type_for_kind("pattern", "kind"),
            "shape.kind and pattern.kind must NOT return the same hint (they are different enums)",
        );
    }

    /// `route` on connector must enumerate straight/orthogonal/avoid.
    ///
    /// Validated at validate/check/nodes/node/shape.rs:309.
    #[test]
    fn route_type_hint_is_connector_specific() {
        assert_eq!(
            attribute_type_for_kind("connector", "route"),
            "enum: straight|orthogonal|avoid",
            "connector.route must enumerate all three routing modes",
        );
    }

    /// `layout` on frame must enumerate absolute/flow/grid.
    ///
    /// Documented in ast/node/container.rs:34-39; grid semantics validated
    /// at validate/check/nodes/node/container.rs:122.
    #[test]
    fn layout_type_hint_is_frame_specific() {
        assert_eq!(
            attribute_type_for_kind("frame", "layout"),
            "enum: absolute|flow|grid",
            "frame.layout must enumerate the three layout modes",
        );
    }
}
