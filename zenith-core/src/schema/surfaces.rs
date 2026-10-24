//! Non-node authorable surfaces: the `variants`/`override` and `diagnostics` blocks.

use crate::diag_catalog::{DIAGNOSTIC_CODES, DIAGNOSTIC_VERBS, DiagnosticCodeInfo};

// ── Variant / override surface ────────────────────────────────────────────────

/// Full schema descriptor for the `variants` / `override` surface.
pub struct VariantDescriptor {
    /// One-line summary of the surface.
    pub summary: &'static str,
    /// Description of the `variants { … }` block structure.
    pub block_structure: &'static str,
    /// Description of the `variant id=… source=… w=… h=… { … }` node.
    pub variant_node: &'static str,
    /// Description of the `override node="<id>" …` entry and its recognised keys.
    pub override_entry: &'static str,
    /// Recognised properties on an `override` entry, as `(name, type, required)` tuples.
    pub override_props: &'static [(&'static str, &'static str, bool)],
    /// A worked example of a `variants` block containing an override.
    pub example: &'static str,
}

/// Return the descriptor for the `variants` / `override` surface.
///
/// This surface is not a node kind (it is not renderable on its own), so it
/// does not appear in `node_kinds()` or `node_summary()`. It is discoverable
/// via `zenith schema variant`.
pub fn variant_descriptor() -> VariantDescriptor {
    VariantDescriptor {
        summary: "Variant system — named page-level derivatives with per-node property overrides.",
        block_structure: "A `variants { … }` block sits at the document root, as a sibling of \
            `document` (canonical order: after `provenance`, before `document`) — NOT inside a \
            page. It contains one or more `variant` entries, each with its own child block of \
            `override` entries that apply to that variant.",
        variant_node: "variant id=<id> source=<page-id> w=(px)N h=(px)N { … }\n\
            \n\
            • id         — unique identifier for this variant (string, required)\n\
            • source     — the page id to base this variant on (page id string, required)\n\
            • w          — override canvas width in pixels, e.g. (px)1920 (dimension, required)\n\
            • h          — override canvas height in pixels, e.g. (px)1080 (dimension, required)\n\
            \n\
            The child block of `variant { … }` contains `override` entries (see below).",
        override_entry: "override node=\"<id>\" …\n\
            \n\
            Targets the node whose id equals the `node=` value, and applies one or more \
            property overrides. The `node` key is the only required field. All other keys \
            are optional and independent. An omitted key keeps the source page value.\n\
            \n\
            Layout keys re-run auto-layout at the variant size. Build the source page from \
            layout frames, then retarget it with a few overrides: set the outer frame `w`/`h`, \
            switch a frame between `row` and `column`, and change `gap`, `padding*`, \
            `font-size`, `justify`, or `align`. `font-size` needs a text, code, field, \
            footnote, or toc node. `layout` and the container keys need a `frame`. A \
            `hug`/`fill` size needs a box node.\n\
            \n\
            IMPORTANT: the selector key is `node` (the target node's id string), NOT `id`.\n\
            Wrong:   override id=\"hero\" visible=#false\n\
            Correct: override node=\"hero\" visible=#false",
        override_props: &[
            ("node", "string — target node id selector (required)", true),
            ("visible", "#true or #false", false),
            ("text", "string — replacement text content", false),
            ("fill", "token ref — color or gradient token", false),
            ("x", "typed dimension, e.g. (px)100", false),
            ("y", "typed dimension, e.g. (px)50", false),
            (
                "w",
                "typed dimension, e.g. (px)800, or \"hug\" / \"fill\"",
                false,
            ),
            (
                "h",
                "typed dimension, e.g. (px)600, or \"hug\" / \"fill\"",
                false,
            ),
            (
                "font-size",
                "dimension token ref, e.g. (token)\"size.title\" (text kinds)",
                false,
            ),
            ("layout", "absolute | row | column | grid (frame)", false),
            ("gap", "dimension or dimension token ref (frame)", false),
            (
                "wrap-gap",
                "dimension or dimension token ref (frame)",
                false,
            ),
            ("padding", "dimension or dimension token ref (frame)", false),
            (
                "padding-x",
                "dimension or dimension token ref (frame)",
                false,
            ),
            (
                "padding-y",
                "dimension or dimension token ref (frame)",
                false,
            ),
            (
                "padding-top",
                "dimension or dimension token ref (frame)",
                false,
            ),
            (
                "padding-right",
                "dimension or dimension token ref (frame)",
                false,
            ),
            (
                "padding-bottom",
                "dimension or dimension token ref (frame)",
                false,
            ),
            (
                "padding-left",
                "dimension or dimension token ref (frame)",
                false,
            ),
            (
                "justify",
                "start | center | end | space-between (frame)",
                false,
            ),
            ("align", "start | center | end | stretch (frame)", false),
            ("wrap", "#true or #false (frame)", false),
        ],
        example: concat!(
            "variants {\n",
            "  variant id=\"story\" source=\"page.main\" w=(px)1080 h=(px)1920 {\n",
            "    // grow the outer layout frame to the new page\n",
            "    override node=\"root\" w=(px)1080 h=(px)1920 padding=(px)96\n",
            "    // stack the cards instead of placing them side by side\n",
            "    override node=\"cards\" layout=\"column\" gap=(px)32\n",
            "    // a larger headline for the tall format\n",
            "    override node=\"headline\" font-size=(token)\"size.title.story\"\n",
            "    // hide the desktop-only sidebar\n",
            "    override node=\"sidebar\" visible=#false\n",
            "  }\n",
            "}",
        ),
    }
}

// ── Ports surface ─────────────────────────────────────────────────────────────

/// Full schema descriptor for the `ports` / `port` connector-anchor surface.
pub struct PortsDescriptor {
    /// One-line summary of the surface.
    pub summary: &'static str,
    /// Where a `ports { … }` block may be declared.
    pub placement: &'static str,
    /// Description of the `ports { port … }` block and `port` entry structure.
    pub block_structure: &'static str,
    /// Recognised attributes on a `port` entry, as `(name, type, required)` tuples.
    pub port_props: &'static [(&'static str, &'static str, bool)],
    /// A worked example of a `ports` block feeding a connector endpoint.
    pub example: &'static str,
}

/// Return the descriptor for the `ports` / `port` surface.
///
/// This surface is not a node kind (it is authored metadata that resolves a
/// connector endpoint's target and anchor), so it does not appear in
/// `node_kinds()` or `node_summary()`. It is discoverable via
/// `zenith schema ports`.
pub fn ports_descriptor() -> PortsDescriptor {
    PortsDescriptor {
        summary: "Named connector ports — stable attachment points on a node that a connector \
            endpoint references as `node#port`, decoupling the connector from a raw anchor value.",
        placement: "A `ports { … }` block may appear at PAGE scope (a child of `page id=… { … }`) \
            and at COMPONENT scope (a child of a `component id=… { … }` definition, so each \
            instance re-exposes the ports on its own id). A connector then names an endpoint as \
            `from=\"<node>#<port>\"` / `to=\"<node>#<port>\"` instead of a bare node id.",
        block_structure: "ports {\n\
            \x20 port node=\"<node-id>\" id=\"<port-id>\" anchor=\"<anchor>\"\n\
            \x20 port node=\"<node-id>\" id=\"<port-id>\" anchor=\"<anchor>\"\n\
            }\n\
            \n\
            Each `port` entry binds a port id (local to the target node) to a resolved anchor on \
            that node. When a connector endpoint is written `node#port`, the port's `node` and \
            `anchor` replace the endpoint's own node/anchor. Port ids must be unique per node \
            (a duplicate emits `connector.port_duplicate`); a `node` that matches no local node \
            emits `connector.port_invalid_target`; a connector referencing an undeclared port \
            emits `connector.unknown_port`.",
        port_props: &[
            (
                "node",
                "string — the id of the node this port sits on",
                true,
            ),
            (
                "id",
                "string — the port id, local to that node, used as `node#id`",
                true,
            ),
            (
                "anchor",
                "string — where on the node the port attaches: `auto`, a nine-point grid anchor \
                 (top/center/bottom × left/center/right, e.g. `bottom-right`), or a divided \
                 anchor `i/N`",
                true,
            ),
        ],
        example: concat!(
            "page id=\"pg\" w=(px)800 h=(px)400 {\n",
            "  rect id=\"box.a\" x=(px)80  y=(px)160 w=(px)160 h=(px)80\n",
            "  rect id=\"box.b\" x=(px)560 y=(px)160 w=(px)160 h=(px)80\n",
            "  ports {\n",
            "    port node=\"box.a\" id=\"out\" anchor=\"center-right\"\n",
            "    port node=\"box.b\" id=\"in\"  anchor=\"center-left\"\n",
            "  }\n",
            "  connector id=\"edge\" from=\"box.a#out\" to=\"box.b#in\" marker-end=\"arrow\"\n",
            "}",
        ),
    }
}

// ── Diagnostics surface ────────────────────────────────────────────────────────

/// One-line description of the `diagnostics` surface (the root `diagnostics { … }`
/// lint-policy block).
pub fn diagnostics_summary() -> &'static str {
    "In-file diagnostic policy — allow/deny/warn specific diagnostic codes \
     (integrity Errors cannot be suppressed)."
}

/// The policy verbs accepted inside a `diagnostics { … }` block, in canonical
/// order (`allow`, `deny`, `warn`).
///
/// Single source of truth: re-exposed from [`crate::diag_catalog`].
pub fn diagnostics_verbs() -> &'static [&'static str] {
    DIAGNOSTIC_VERBS
}

/// The full catalog of diagnostic codes the engine can emit, each with its
/// severity and a one-line summary.
///
/// Single source of truth: re-exposed from [`crate::diag_catalog`]. The same
/// table drives the diagnostic-policy validator in [`crate::validate()`], so the
/// `zenith schema diagnostics` surface and the policy checker can never diverge.
pub fn diagnostic_codes() -> &'static [DiagnosticCodeInfo] {
    DIAGNOSTIC_CODES
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::transform::known_props_for_kind;

    #[test]
    fn variant_override_props_match_the_parser() {
        let schema: Vec<&str> = variant_descriptor()
            .override_props
            .iter()
            .map(|(name, _, _)| *name)
            .collect();
        let mut parser: Vec<&str> = known_props_for_kind("override").to_vec();
        let mut listed = schema.clone();
        parser.sort_unstable();
        listed.sort_unstable();
        assert_eq!(
            listed, parser,
            "schema variant must list every override key"
        );
        for key in [
            "font-size",
            "layout",
            "gap",
            "padding-x",
            "justify",
            "align",
            "wrap",
        ] {
            assert!(schema.contains(&key), "missing {key}");
        }
    }
}
