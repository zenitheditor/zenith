//! Tests for the `zenith schema` command surfaces.

use super::*;

#[test]
fn overview_human_contains_counts() {
    let (text, code) = overview(false);
    assert_eq!(code, 0);
    assert!(text.contains("node kinds"), "must mention node kinds");
    assert!(text.contains("tx ops"), "must mention tx ops");
}

#[test]
fn overview_json_schema_field() {
    let (text, code) = overview(true);
    assert_eq!(code, 0);
    assert!(
        text.contains("zenith-schema-v1"),
        "JSON must carry schema field"
    );
    assert!(
        text.contains("node_kinds"),
        "JSON must carry node_kinds count"
    );
}

#[test]
fn nodes_human_contains_rect() {
    let (text, code) = nodes(false);
    assert_eq!(code, 0);
    assert!(text.contains("rect"), "must list rect kind");
    assert!(text.contains("Rectangle"), "must include rect summary");
}

#[test]
fn nodes_json_schema_field() {
    let (text, code) = nodes(true);
    assert_eq!(code, 0);
    assert!(text.contains("zenith-schema-v1"));
    assert!(text.contains("\"kind\""));
}

#[test]
fn node_detail_known_kind() {
    let (text, code) = node_detail("rect", false);
    assert_eq!(code, 0);
    assert!(text.contains("rect"), "must name the kind");
    assert!(text.contains("Attributes:"), "must list attributes");
    assert!(text.contains("fill"), "rect must have a fill attribute");
    assert!(
        text.contains("token ref"),
        "fill must show its type hint (token ref)"
    );
    assert!(text.contains("—"), "attributes must use — separator");
    assert!(
        text.contains("zenith validate"),
        "must mention zenith validate for types"
    );
}

#[test]
fn node_detail_human_shows_name_and_type() {
    // Human output: each attribute line is "  <name>  —  <type>"
    let (text, code) = node_detail("rect", false);
    assert_eq!(code, 0);
    // x is a px literal, fill is a token ref.
    assert!(text.contains("x  "), "must list x attribute; got:\n{text}");
    assert!(
        text.contains("px literal"),
        "x must show px literal type; got:\n{text}"
    );
    assert!(
        text.contains("token ref: color/gradient"),
        "fill must show token ref type; got:\n{text}"
    );
}

#[test]
fn node_detail_json_known_kind() {
    let (text, code) = node_detail("pattern", true);
    assert_eq!(code, 0);
    assert!(text.contains("zenith-schema-v1"));
    assert!(text.contains("\"kind\""));
    assert!(text.contains("\"attributes\""));
    // New shape: attributes is an array of {name, ty} objects.
    assert!(
        text.contains("\"name\""),
        "attribute objects must have name field"
    );
    assert!(
        text.contains("\"ty\""),
        "attribute objects must have ty field"
    );
}

#[test]
fn node_detail_json_attr_has_type_hint() {
    let (text, code) = node_detail("rect", true);
    assert_eq!(code, 0);
    // fill must appear with its type.
    assert!(
        text.contains("\"fill\""),
        "fill attribute must appear; got:\n{text}"
    );
    assert!(
        text.contains("token ref"),
        "fill type must be a token ref; got:\n{text}"
    );
    // x must appear with px literal type.
    assert!(
        text.contains("px literal"),
        "x must have px literal type; got:\n{text}"
    );
}

#[test]
fn node_detail_unknown_kind_returns_error() {
    let (text, code) = node_detail("not-a-kind", false);
    assert_eq!(code, 1);
    assert!(
        text.contains("unknown node kind"),
        "must report unknown kind"
    );
    assert!(text.contains("valid kinds"), "must list valid kinds");
}

#[test]
fn ops_human_contains_set_fill() {
    let (text, code) = ops(false);
    assert_eq!(code, 0);
    assert!(text.contains("set_fill"), "must list set_fill op");
}

#[test]
fn ops_json_schema_field() {
    let (text, code) = ops(true);
    assert_eq!(code, 0);
    assert!(text.contains("zenith-schema-v1"));
    assert!(text.contains("\"op\""));
}

#[test]
fn op_detail_known_op() {
    let (text, code) = op_detail("set_fill", false);
    assert_eq!(code, 0);
    assert!(text.contains("set_fill"), "must name the op");
    assert!(text.contains("fill"), "must mention the fill field");
    assert!(text.contains("Fields:"), "must include Fields section");
    assert!(text.contains("Example:"), "must include Example section");
}

#[test]
fn op_detail_json_known_op() {
    let (text, code) = op_detail("add_node", true);
    assert_eq!(code, 0);
    assert!(text.contains("zenith-schema-v1"));
    assert!(text.contains("\"op\""));
    assert!(
        text.contains("\"fields\""),
        "JSON must include fields array"
    );
    assert!(
        text.contains("\"example\""),
        "JSON must include example string"
    );
}

#[test]
fn op_detail_detach_pattern_human() {
    let (text, code) = op_detail("detach_pattern", false);
    assert_eq!(code, 0);
    assert!(text.contains("detach_pattern"));
    assert!(text.contains("Fields:"));
    assert!(text.contains("node"));
    assert!(text.contains("Example:"));
}

#[test]
fn op_detail_set_fill_json_has_node_and_fill_fields() {
    let (text, code) = op_detail("set_fill", true);
    assert_eq!(code, 0);
    assert!(text.contains("\"node\""), "fields must include node");
    assert!(text.contains("\"fill\""), "fields must include fill");
    assert!(text.contains("token ref"), "fill type must be token ref");
    assert!(
        text.contains("color.brand"),
        "example must use realistic value"
    );
}

#[test]
fn op_detail_unknown_op_returns_error() {
    let (text, code) = op_detail("not_an_op", false);
    assert_eq!(code, 1);
    assert!(text.contains("unknown op"), "must report unknown op");
    assert!(text.contains("valid ops"), "must list valid ops");
}

#[test]
fn overview_mentions_new_surfaces() {
    let (text, code) = overview(false);
    assert_eq!(code, 0);
    assert!(text.contains("page"), "overview must mention page surface");
    assert!(
        text.contains("asset"),
        "overview must mention asset surface"
    );
    assert!(
        text.contains("document"),
        "overview must mention document surface"
    );
}

#[test]
fn page_human_contains_geometry_attrs() {
    let (text, code) = page(false);
    assert_eq!(code, 0);
    assert!(text.contains("page"), "must name the surface");
    assert!(text.contains("Attributes:"), "must list attributes");
    assert!(text.contains("w"), "page must have w attribute");
    assert!(text.contains("h"), "page must have h attribute");
    assert!(text.contains("—"), "attributes must use — separator");
    assert!(
        text.contains("px literal"),
        "w/h must show px literal type hint"
    );
    assert!(
        text.contains("zenith validate"),
        "must mention zenith validate"
    );
}

#[test]
fn page_json_schema_field() {
    let (text, code) = page(true);
    assert_eq!(code, 0);
    assert!(text.contains("zenith-schema-v1"));
    assert!(text.contains("\"surface\""));
    assert!(text.contains("\"attributes\""));
    assert!(text.contains("\"page\""));
    // New shape: attributes is an array of {name, ty} objects.
    assert!(
        text.contains("\"name\""),
        "attribute objects must have name field"
    );
    assert!(
        text.contains("\"ty\""),
        "attribute objects must have ty field"
    );
}

#[test]
fn asset_human_contains_provenance_attrs() {
    let (text, code) = asset(false);
    assert_eq!(code, 0);
    assert!(text.contains("asset"), "must name the surface");
    assert!(text.contains("sha256"), "asset must include sha256");
    assert!(text.contains("ai-prompt"), "asset must include ai-prompt");
}

#[test]
fn asset_json_schema_field() {
    let (text, code) = asset(true);
    assert_eq!(code, 0);
    assert!(text.contains("zenith-schema-v1"));
    assert!(text.contains("\"asset\""));
}

#[test]
fn document_human_contains_root_attrs() {
    let (text, code) = document(false);
    assert_eq!(code, 0);
    assert!(text.contains("document"), "must name the surface");
    assert!(
        text.contains("colorspace"),
        "document must include colorspace"
    );
    assert!(text.contains("doc-id"), "document must include doc-id");
}

#[test]
fn document_json_schema_field() {
    let (text, code) = document(true);
    assert_eq!(code, 0);
    assert!(text.contains("zenith-schema-v1"));
    assert!(text.contains("\"document\""));
}

#[test]
fn overview_mentions_token_types() {
    let (text, code) = overview(false);
    assert_eq!(code, 0);
    assert!(
        text.contains("token types"),
        "overview must mention token types; got:\n{text}"
    );
    assert!(
        text.contains("zenith schema tokens"),
        "overview must mention 'zenith schema tokens'; got:\n{text}"
    );
    assert!(
        text.contains("zenith schema token"),
        "overview must mention 'zenith schema token <type>'; got:\n{text}"
    );
}

#[test]
fn overview_json_has_token_types_count() {
    let (text, code) = overview(true);
    assert_eq!(code, 0);
    assert!(
        text.contains("token_types"),
        "JSON overview must carry token_types count; got:\n{text}"
    );
}

#[test]
fn tokens_human_lists_all_types() {
    let (text, code) = tokens(false);
    assert_eq!(code, 0);
    assert!(text.contains("color"), "must list color type");
    assert!(text.contains("gradient"), "must list gradient type");
    assert!(text.contains("shadow"), "must list shadow type");
    assert!(text.contains("filter"), "must list filter type");
    assert!(text.contains("mask"), "must list mask type");
    assert!(text.contains("dimension"), "must list dimension type");
    assert!(text.contains("number"), "must list number type");
    assert!(text.contains("fontFamily"), "must list fontFamily type");
    assert!(text.contains("fontWeight"), "must list fontWeight type");
}

#[test]
fn tokens_json_schema_field() {
    let (text, code) = tokens(true);
    assert_eq!(code, 0);
    assert!(text.contains("zenith-schema-v1"));
    assert!(text.contains("\"token_types\""));
    assert!(text.contains("\"ty\""));
    assert!(text.contains("\"summary\""));
}

#[test]
fn token_detail_color_human() {
    let (text, code) = token_detail("color", false);
    assert_eq!(code, 0);
    assert!(text.contains("color"), "must name the type");
    assert!(
        text.contains("Value form:"),
        "must include Value form section"
    );
    assert!(text.contains("#rrggbb"), "must describe hex color form");
    assert!(text.contains("Example:"), "must include Example section");
}

#[test]
fn token_detail_gradient_human() {
    let (text, code) = token_detail("gradient", false);
    assert_eq!(code, 0);
    assert!(text.contains("gradient"), "must name the type");
    assert!(
        text.contains("Child nodes:"),
        "gradient must include Child nodes section"
    );
    assert!(text.contains("stop"), "gradient must describe stop child");
    assert!(text.contains("Example:"), "must include Example section");
}

#[test]
fn token_detail_shadow_human() {
    let (text, code) = token_detail("shadow", false);
    assert_eq!(code, 0);
    assert!(text.contains("shadow"), "must name the type");
    assert!(
        text.contains("Child nodes:"),
        "shadow must include Child nodes section"
    );
    assert!(text.contains("layer"), "shadow must describe layer child");
}

#[test]
fn token_detail_json_has_all_fields() {
    let (text, code) = token_detail("gradient", true);
    assert_eq!(code, 0);
    assert!(text.contains("zenith-schema-v1"));
    assert!(text.contains("\"token\""));
    assert!(text.contains("\"ty\""));
    assert!(text.contains("\"summary\""));
    assert!(text.contains("\"value_form\""));
    assert!(text.contains("\"child_nodes\""));
    assert!(text.contains("\"example\""));
}

#[test]
fn token_detail_human_documents_set_attribute() {
    // Every token type's human detail must mention the common `set=`
    // provenance attribute (documented once, not duplicated per type).
    let (text, code) = token_detail("color", false);
    assert_eq!(code, 0);
    assert!(
        text.contains("set=") && text.contains("provenance"),
        "must document the set= provenance attribute; got:\n{text}"
    );
}

#[test]
fn token_detail_unknown_type_returns_error() {
    let (text, code) = token_detail("bogus", false);
    assert_eq!(code, 1);
    assert!(
        text.contains("unknown token type"),
        "must report unknown type"
    );
    assert!(text.contains("valid types"), "must list valid types");
}

#[test]
fn node_detail_override_kind_hints_variant_surface() {
    // "override" is not a node kind; the error must hint at `zenith schema variant`.
    let (text, code) = node_detail("override", false);
    assert_eq!(code, 1);
    assert!(
        text.contains("unknown node kind"),
        "must report unknown kind"
    );
    assert!(
        text.contains("zenith schema variant"),
        "error for 'override' must hint at `zenith schema variant`; got:\n{text}"
    );
}

#[test]
fn node_detail_variant_kind_hints_variant_surface() {
    // "variant" is also not a node kind; same hint applies.
    let (text, code) = node_detail("variant", false);
    assert_eq!(code, 1);
    assert!(
        text.contains("unknown node kind"),
        "must report unknown kind"
    );
    assert!(
        text.contains("zenith schema variant"),
        "error for 'variant' must hint at `zenith schema variant`; got:\n{text}"
    );
}

#[test]
fn node_detail_other_unknown_no_variant_hint() {
    // Truly unknown kinds get no variant hint.
    let (text, code) = node_detail("frobnicate", false);
    assert_eq!(code, 1);
    assert!(
        text.contains("unknown node kind"),
        "must report unknown kind"
    );
    assert!(
        !text.contains("zenith schema variant"),
        "generic unknown kind must not mention variant surface; got:\n{text}"
    );
}

#[test]
fn variant_human_contains_key_sections() {
    let (text, code) = variant(false);
    assert_eq!(code, 0);
    assert!(text.contains("variant"), "must name the surface");
    assert!(
        text.contains("Override properties:"),
        "must list override properties"
    );
    assert!(
        text.contains("node"),
        "override properties must include 'node' selector"
    );
    assert!(
        text.contains("visible"),
        "override properties must include 'visible'"
    );
    assert!(
        text.contains("x") && text.contains("y") && text.contains("w") && text.contains("h"),
        "override properties must include geometry keys x/y/w/h; got:\n{text}"
    );
    assert!(
        text.contains("Example:"),
        "must include a worked example section"
    );
    assert!(
        text.contains("source="),
        "example must show the source= attribute on a variant node"
    );
}

#[test]
fn variant_human_override_node_selector_note() {
    let (text, code) = variant(false);
    assert_eq!(code, 0);
    // The override entry description must emphasise that the key is `node`, not `id`.
    assert!(
        text.contains("node"),
        "override entry must describe the 'node' selector key; got:\n{text}"
    );
    // Must warn about the wrong key.
    assert!(
        text.to_lowercase().contains("not") || text.contains("NOT"),
        "override entry should warn that 'id' is the wrong key; got:\n{text}"
    );
}

#[test]
fn variant_json_schema_field() {
    let (text, code) = variant(true);
    assert_eq!(code, 0);
    assert!(
        text.contains("zenith-schema-v1"),
        "JSON must carry schema field"
    );
    assert!(
        text.contains("\"summary\""),
        "JSON must carry summary field"
    );
    assert!(
        text.contains("\"override_props\""),
        "JSON must carry override_props array"
    );
    assert!(
        text.contains("\"example\""),
        "JSON must carry example field"
    );
}

#[test]
fn variant_json_override_props_have_geometry() {
    let (text, code) = variant(true);
    assert_eq!(code, 0);
    // x, y, w, h must all appear as override prop names.
    for key in &["\"x\"", "\"y\"", "\"w\"", "\"h\""] {
        assert!(
            text.contains(key),
            "variant JSON override_props must include {key}; got:\n{text}"
        );
    }
    // node must be required.
    assert!(
        text.contains("\"node\""),
        "variant JSON override_props must include node; got:\n{text}"
    );
}

#[test]
fn ports_human_contains_key_sections() {
    let (text, code) = ports(false);
    assert_eq!(code, 0);
    assert!(text.contains("ports"), "must name the surface");
    assert!(
        text.contains("Placement:"),
        "must describe where a ports block may appear"
    );
    assert!(
        text.contains("Port properties:"),
        "must list the port properties"
    );
    // The three required attributes.
    assert!(
        text.contains("node") && text.contains("id") && text.contains("anchor"),
        "port properties must include node/id/anchor; got:\n{text}"
    );
    // Placement covers both page and component scope.
    assert!(
        text.to_lowercase().contains("page") && text.to_lowercase().contains("component"),
        "placement must mention page and component scope; got:\n{text}"
    );
    assert!(
        text.contains("Example:") && text.contains("connector"),
        "must include a worked example wiring a connector to a port; got:\n{text}"
    );
}

#[test]
fn ports_json_has_expected_fields() {
    let (text, code) = ports(true);
    assert_eq!(code, 0);
    assert!(
        text.contains("zenith-schema-v1"),
        "JSON must carry schema field"
    );
    for key in &[
        "\"summary\"",
        "\"placement\"",
        "\"block_structure\"",
        "\"port_props\"",
        "\"example\"",
    ] {
        assert!(
            text.contains(key),
            "ports JSON must carry {key}; got:\n{text}"
        );
    }
    // All three required attributes present and marked required.
    for key in &["\"node\"", "\"id\"", "\"anchor\""] {
        assert!(
            text.contains(key),
            "ports JSON port_props must include {key}; got:\n{text}"
        );
    }
    assert!(
        text.contains("\"required\": true"),
        "ports JSON port_props must mark required attributes; got:\n{text}"
    );
}

#[test]
fn op_detail_add_node_position_describes_id_field() {
    // Regression: before/after variants use `id` (sibling id), not `sibling`.
    let (text, code) = op_detail("add_node", false);
    assert_eq!(code, 0);
    assert!(
        text.contains("id"),
        "add_node position description must mention the 'id' field; got:\n{text}"
    );
    assert!(
        text.contains("before") && text.contains("after"),
        "add_node position description must mention before/after variants; got:\n{text}"
    );
    assert!(
        text.contains("index"),
        "add_node position description must mention index variant; got:\n{text}"
    );
}

#[test]
fn op_detail_add_node_position_json_has_correct_shape() {
    let (text, code) = op_detail("add_node", true);
    assert_eq!(code, 0);
    // The ty string must contain "id" to describe the before/after sibling key.
    assert!(
        text.contains("sibling-id") || text.contains("\"id\""),
        "add_node position field ty must describe the sibling id key; got:\n{text}"
    );
}

#[test]
fn token_detail_fontweight_no_value_form_confusion() {
    // fontWeight must explicitly say bare integer, NOT a string or dimension.
    let (text, code) = token_detail("fontWeight", false);
    assert_eq!(code, 0);
    assert!(
        text.contains("700"),
        "fontWeight example must use a bare integer"
    );
    // The value form must not suggest string or dimension syntax.
    assert!(
        !text.contains("\"700\""),
        "fontWeight must not suggest string form"
    );
}

// ── Content section tests ─────────────────────────────────────────────────

#[test]
fn node_detail_shape_human_shows_content_section() {
    let (text, code) = node_detail("shape", false);
    assert_eq!(code, 0);
    assert!(
        text.contains("Content:"),
        "shape detail must include Content section; got:\n{text}"
    );
    assert!(
        text.contains("span"),
        "shape Content section must mention span children; got:\n{text}"
    );
    assert!(
        text.contains("label") || text.contains("centered"),
        "shape Content section must describe the owned label behaviour; got:\n{text}"
    );
    assert!(
        text.contains("Example:"),
        "shape Content section must include an example; got:\n{text}"
    );
}

#[test]
fn node_detail_shape_json_has_content_field() {
    let (text, code) = node_detail("shape", true);
    assert_eq!(code, 0);
    assert!(
        text.contains("\"content\""),
        "shape JSON must carry a content field; got:\n{text}"
    );
    assert!(
        text.contains("\"description\""),
        "shape JSON content must carry a description; got:\n{text}"
    );
    assert!(
        text.contains("\"example\""),
        "shape JSON content must carry an example; got:\n{text}"
    );
    // content must be non-null
    assert!(
        !text.contains("\"content\": null"),
        "shape JSON content must be non-null; got:\n{text}"
    );
}

#[test]
fn node_detail_polygon_human_shows_content_section() {
    let (text, code) = node_detail("polygon", false);
    assert_eq!(code, 0);
    assert!(
        text.contains("Content:"),
        "polygon detail must include Content section; got:\n{text}"
    );
    assert!(
        text.contains("point"),
        "polygon Content section must mention point children; got:\n{text}"
    );
}

#[test]
fn node_detail_text_human_shows_content_section() {
    let (text, code) = node_detail("text", false);
    assert_eq!(code, 0);
    assert!(
        text.contains("Content:"),
        "text detail must include Content section; got:\n{text}"
    );
    assert!(
        text.contains("span"),
        "text Content section must mention span children; got:\n{text}"
    );
}

#[test]
fn node_detail_rect_human_no_content_section() {
    // rect has no child content; the Content section must be absent.
    let (text, code) = node_detail("rect", false);
    assert_eq!(code, 0);
    assert!(
        !text.contains("Content:"),
        "rect detail must NOT include a Content section; got:\n{text}"
    );
}

#[test]
fn node_detail_rect_json_content_is_absent() {
    let (text, code) = node_detail("rect", true);
    assert_eq!(code, 0);
    // For a leaf node with no child content, the content field must be absent entirely.
    assert!(
        !text.contains("\"content\""),
        "rect JSON must not carry a content field (skip_serializing_if = None); got:\n{text}"
    );
}

#[test]
fn node_detail_light_human_shows_example_without_content() {
    let (text, code) = node_detail("light", false);
    assert_eq!(code, 0);
    assert!(
        text.contains("Example:"),
        "light must show authoring example; got:\n{text}"
    );
    assert!(
        text.contains("light id=\"bg.glow\""),
        "light example must be concrete; got:\n{text}"
    );
    assert!(
        !text.contains("Content:"),
        "light is a leaf node and must not show Content section; got:\n{text}"
    );
}

#[test]
fn node_detail_light_json_has_example_without_content() {
    let (text, code) = node_detail("light", true);
    assert_eq!(code, 0);
    assert!(
        text.contains("\"example\""),
        "light JSON must carry example; got:\n{text}"
    );
    assert!(
        text.contains("bg.glow"),
        "light JSON example must include usable node id; got:\n{text}"
    );
    assert!(
        !text.contains("\"content\""),
        "light JSON must not carry child content; got:\n{text}"
    );
}

#[test]
fn node_detail_mesh_human_shows_example_without_content() {
    let (text, code) = node_detail("mesh", false);
    assert_eq!(code, 0);
    assert!(
        text.contains("Example:"),
        "mesh must show authoring example; got:\n{text}"
    );
    assert!(
        text.contains("mesh id=\"bg.mesh\""),
        "mesh example must be concrete; got:\n{text}"
    );
    assert!(
        !text.contains("Content:"),
        "mesh is a leaf node and must not show Content section; got:\n{text}"
    );
}

#[test]
fn node_detail_mesh_json_has_example_without_content() {
    let (text, code) = node_detail("mesh", true);
    assert_eq!(code, 0);
    assert!(
        text.contains("\"example\""),
        "mesh JSON must carry example; got:\n{text}"
    );
    assert!(
        text.contains("bg.mesh"),
        "mesh JSON example must include usable node id; got:\n{text}"
    );
    assert!(
        !text.contains("\"content\""),
        "mesh JSON must not carry child content; got:\n{text}"
    );
}

// ── Diagnostics surface tests ────────────────────────────────────────────

#[test]
fn diagnostics_human_mentions_scoped_policy_syntax() {
    let (text, code) = diagnostics(false);
    assert_eq!(code, 0);
    assert!(
        text.contains("allow \"<code>\" \"<subject-id>\""),
        "human output must show scoped diagnostic policy syntax; got:\n{text}"
    );
    assert!(
        text.contains("allow \"layout.off_canvas\" \"bg.glow\" \"bg.rim\""),
        "human output must include multi-subject example; got:\n{text}"
    );
}

#[test]
fn diagnostics_json_carries_policy_syntax() {
    let (text, code) = diagnostics(true);
    assert_eq!(code, 0);
    assert!(
        text.contains("\"syntax\""),
        "JSON must carry syntax examples; got:\n{text}"
    );
    assert!(
        text.contains("allow \\\"<code>\\\" \\\"<subject-id>\\\""),
        "JSON must include scoped diagnostic policy syntax; got:\n{text}"
    );
}

/// `token.set_partially_used` is defined in the core diagnostic catalog and
/// must flow through automatically to the `zenith schema diagnostics`
/// listing (both human and JSON), with no CLI-side row needed.
#[test]
fn diagnostics_listing_includes_token_set_partially_used() {
    let (human, code) = diagnostics(false);
    assert_eq!(code, 0);
    assert!(
        human.contains("token.set_partially_used"),
        "human diagnostics listing must include the new code; got:\n{human}"
    );

    let (json, code) = diagnostics(true);
    assert_eq!(code, 0);
    assert!(
        json.contains("\"token.set_partially_used\""),
        "JSON diagnostics listing must include the new code; got:\n{json}"
    );
    assert!(
        json.contains("\"advisory\""),
        "JSON diagnostics listing must carry a severity string; got:\n{json}"
    );
}

// ── Brand surface tests ───────────────────────────────────────────────────

#[test]
fn brand_human_contains_key_sections() {
    let (text, code) = brand(false);
    assert_eq!(code, 0);
    assert!(
        text.contains("brand {"),
        "human output must include worked example with 'brand {{'; got:\n{text}"
    );
    assert!(
        text.contains("colors"),
        "human output must describe the colors child node; got:\n{text}"
    );
    assert!(
        text.contains("fonts"),
        "human output must describe the fonts child node; got:\n{text}"
    );
    assert!(
        text.contains("weights"),
        "human output must describe the weights child node; got:\n{text}"
    );
    assert!(
        text.contains("brand.color_off_palette"),
        "human output must list brand.color_off_palette diagnostic code; got:\n{text}"
    );
    assert!(
        text.contains("brand.font_not_allowed"),
        "human output must list brand.font_not_allowed diagnostic code; got:\n{text}"
    );
    assert!(
        text.contains("brand.weight_not_allowed"),
        "human output must list brand.weight_not_allowed diagnostic code; got:\n{text}"
    );
    assert!(
        text.contains("--deny"),
        "human output must mention --deny for CI gate; got:\n{text}"
    );
}

#[test]
fn brand_json_schema_field() {
    let (text, code) = brand(true);
    assert_eq!(code, 0);
    assert!(
        text.contains("zenith-schema-v1"),
        "JSON must carry schema field; got:\n{text}"
    );
    assert!(
        text.contains("\"summary\""),
        "JSON must carry summary field; got:\n{text}"
    );
    assert!(
        text.contains("\"child_nodes\""),
        "JSON must carry child_nodes array; got:\n{text}"
    );
    assert!(
        text.contains("\"diagnostic_codes\""),
        "JSON must carry diagnostic_codes array; got:\n{text}"
    );
}

#[test]
fn overview_mentions_brand_surface() {
    let (text, code) = overview(false);
    assert_eq!(code, 0);
    assert!(
        text.contains("zenith schema brand"),
        "overview must mention 'zenith schema brand'; got:\n{text}"
    );
    assert!(
        text.contains("zenith schema block"),
        "overview must mention 'zenith schema block'; got:\n{text}"
    );
    assert!(
        text.contains("8 non-node surfaces"),
        "overview must count 8 non-node surfaces after adding ports; got:\n{text}"
    );
    assert!(
        text.contains("zenith schema ports"),
        "overview must mention 'zenith schema ports'; got:\n{text}"
    );
}
