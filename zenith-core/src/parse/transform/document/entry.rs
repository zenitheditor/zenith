//! Top-level `transform` entry point and the root `zenith`-node attributes.

use kdl::KdlDocument;

use crate::ast::action::ActionDef;
use crate::ast::asset::AssetBlock;
use crate::ast::brand::BrandContract;
use crate::ast::document::{
    ComponentDef, Document, DocumentBody, ImportDecl, MasterDef, Project, SectionDef,
};
use crate::ast::library::LibraryDef;
use crate::ast::policy::DiagnosticPolicy;
use crate::ast::provenance::ProvenanceDef;
use crate::ast::recipe::RecipeDef;
use crate::ast::style::StyleBlock;
use crate::ast::token::TokenBlock;
use crate::ast::variant::VariantDef;
use crate::error::{ParseError, ParseErrorCode};
use crate::parse::transform::helpers::{
    optional_bool_prop, optional_dimension_prop, optional_string_prop, required_u32_prop,
};
use crate::parse::transform::tokens::{transform_styles, transform_tokens};

use super::assets::transform_assets;
use super::body::transform_document_body;
use super::brand::transform_brand_contract;
use super::components::{transform_components, transform_project};
use super::imports::transform_imports;
use super::policy::transform_diagnostic_policy;
use super::structure::{
    transform_actions, transform_libraries, transform_masters, transform_provenance,
    transform_sections,
};
use super::variants::{transform_recipes, transform_variants};

/// Canonical set of property names recognised on the document-level surface.
///
/// Covers both the root `zenith` node attributes (version, colorspace,
/// doc-id, mirror-margins, page-progression, page-parity-start,
/// facing-pages, spread-gutter, margin-*) and the required `document`
/// child-block attributes (id, title).
///
/// Both the hyphenated spelling (canonical) and the underscored alias are
/// included for each attribute that accepts either form, matching the lenient
/// parser behaviour. Used by `zenith-core::schema` to surface the authorable
/// attribute list for the `zenith schema document` subcommand.
pub(crate) const DOCUMENT_KNOWN_PROPS: &[&str] = &[
    // root `zenith` node
    "version",
    "colorspace",
    "doc-id",
    "doc_id",
    "mirror-margins",
    "mirror_margins",
    "page-progression",
    "page_progression",
    "page-parity-start",
    "page_parity_start",
    "facing-pages",
    "facing_pages",
    "spread-gutter",
    "spread_gutter",
    "margin-inner",
    "margin_inner",
    "margin-outer",
    "margin_outer",
    "margin-top",
    "margin_top",
    "margin-bottom",
    "margin_bottom",
    // `document { … }` child block
    "id",
    "title",
];

/// Transform a parsed `KdlDocument` into a Zenith `Document` AST.
pub fn transform(doc: &KdlDocument) -> Result<Document, ParseError> {
    // Find the single top-level `zenith` node.
    let zenith_node = doc
        .nodes()
        .iter()
        .find(|n| n.name().value() == "zenith")
        .ok_or_else(|| {
            ParseError::spanless(
                ParseErrorCode::MissingZenithRoot,
                "no top-level `zenith` node found",
            )
        })?;

    let version = required_u32_prop(zenith_node, "version")?;
    // Optional export color space attribute on the root `zenith` node. Value
    // validity ("srgb"|"cmyk") is checked by the validator, not the parser, so
    // an unrecognized value is preserved verbatim for a precise warning.
    let colorspace = optional_string_prop(zenith_node, "colorspace").map(str::to_owned);

    // Optional stable document identity (`doc-id="01ARZ3NDEKTSV4RRFFQ69G5FAV"`).
    // The value is a ULID (Crockford base-32) minted at document creation; it
    // is preserved verbatim without validation — the parser accepts whatever
    // string the author wrote and lets the validator decide. Both the hyphenated
    // and underscored spellings are accepted for forward-compat.
    let doc_id = optional_string_prop(zenith_node, "doc-id")
        .or_else(|| optional_string_prop(zenith_node, "doc_id"))
        .map(str::to_owned);

    // Optional mirrored-margins toggle (`mirror-margins=#true`). Forward-compat:
    // both the hyphenated and underscored spellings are accepted.
    let mirror_margins = optional_bool_prop(zenith_node, "mirror-margins")
        .or_else(|| optional_bool_prop(zenith_node, "mirror_margins"));

    // Optional page-progression attribute (`page-progression="rtl"`). Value
    // validity ("ltr"|"rtl") is checked by the validator, not the parser, so an
    // unrecognized value is preserved verbatim for a precise warning.
    let page_progression = optional_string_prop(zenith_node, "page-progression")
        .or_else(|| optional_string_prop(zenith_node, "page_progression"))
        .map(str::to_owned);

    // Optional starting-parity attribute (`page-parity-start="verso"`). Value
    // validity ("recto"|"verso") is checked by the validator, not the parser, so
    // an unrecognized value is preserved verbatim for a precise warning. Both the
    // hyphenated and underscored spellings are accepted for forward-compat.
    let page_parity_start = optional_string_prop(zenith_node, "page-parity-start")
        .or_else(|| optional_string_prop(zenith_node, "page_parity_start"))
        .map(str::to_owned);

    // Optional facing-pages toggle (`facing-pages=#true`). Forward-compat:
    // both the hyphenated and underscored spellings are accepted. This is
    // informational metadata only; pages still render independently.
    let facing_pages = optional_bool_prop(zenith_node, "facing-pages")
        .or_else(|| optional_bool_prop(zenith_node, "facing_pages"));

    // Optional spread-gutter dimension (`spread-gutter=(px)40`). Drives the
    // transparent gap between the two pages of a `--spread` composite.
    // Both hyphenated and underscored spellings are accepted for forward-compat.
    let spread_gutter = optional_dimension_prop(zenith_node, "spread-gutter")
        .or_else(|| optional_dimension_prop(zenith_node, "spread_gutter"));

    // Optional DOCUMENT-LEVEL default book live-area margins. Same KDL syntax as
    // on a page (`margin-inner=(px)225`); a page that omits its own margin
    // inherits these via `Document::effective_margins`. Both hyphenated and
    // underscored spellings are accepted for forward-compat.
    let margin_inner = optional_dimension_prop(zenith_node, "margin-inner")
        .or_else(|| optional_dimension_prop(zenith_node, "margin_inner"));
    let margin_outer = optional_dimension_prop(zenith_node, "margin-outer")
        .or_else(|| optional_dimension_prop(zenith_node, "margin_outer"));
    let margin_top = optional_dimension_prop(zenith_node, "margin-top")
        .or_else(|| optional_dimension_prop(zenith_node, "margin_top"));
    let margin_bottom = optional_dimension_prop(zenith_node, "margin-bottom")
        .or_else(|| optional_dimension_prop(zenith_node, "margin_bottom"));

    let children_doc = zenith_node.children().ok_or_else(|| {
        ParseError::spanless(
            ParseErrorCode::MissingZenithRoot,
            "`zenith` node has no children block",
        )
    })?;

    let mut project: Option<Project> = None;
    let mut assets = AssetBlock::default();
    let mut libraries: Vec<LibraryDef> = Vec::new();
    let mut imports: Vec<ImportDecl> = Vec::new();
    let mut actions: Vec<ActionDef> = Vec::new();
    let mut tokens = TokenBlock::default();
    let mut styles = StyleBlock::default();
    let mut components: Vec<ComponentDef> = Vec::new();
    let mut masters: Vec<MasterDef> = Vec::new();
    let mut sections: Vec<SectionDef> = Vec::new();
    let mut provenance: Vec<ProvenanceDef> = Vec::new();
    let mut variants: Vec<VariantDef> = Vec::new();
    let mut recipes: Vec<RecipeDef> = Vec::new();
    let mut diagnostic_policy = DiagnosticPolicy::default();
    let mut brand_contract = BrandContract::default();
    let mut body: Option<DocumentBody> = None;

    for child in children_doc.nodes() {
        match child.name().value() {
            "project" => {
                project = Some(transform_project(child)?);
            }
            "assets" => {
                assets = transform_assets(child)?;
            }
            "libraries" => {
                libraries = transform_libraries(child)?;
            }
            "imports" => {
                imports = transform_imports(child)?;
            }
            "actions" => {
                actions = transform_actions(child)?;
            }
            "tokens" => {
                tokens = transform_tokens(child)?;
            }
            "styles" => {
                styles = transform_styles(child)?;
            }
            "components" => {
                components = transform_components(child)?;
            }
            "masters" => {
                masters = transform_masters(child)?;
            }
            "sections" => {
                sections = transform_sections(child)?;
            }
            "provenance" => {
                provenance = transform_provenance(child)?;
            }
            "variants" => {
                variants = transform_variants(child)?;
            }
            "recipes" => {
                recipes = transform_recipes(child)?;
            }
            "diagnostics" => {
                diagnostic_policy = transform_diagnostic_policy(child)?;
            }
            "brand" => {
                brand_contract = transform_brand_contract(child)?;
            }
            "document" => {
                body = Some(transform_document_body(child)?);
            }
            // Any other unknown top-level children are accepted without error
            // (forward-compat); they simply are not represented in the v0 AST.
            _ => {}
        }
    }

    let body = body.ok_or_else(|| {
        ParseError::spanless(
            ParseErrorCode::MissingZenithRoot,
            "`zenith` node is missing a `document` child",
        )
    })?;

    Ok(Document {
        version,
        colorspace,
        doc_id,
        mirror_margins,
        facing_pages,
        spread_gutter,
        page_progression,
        page_parity_start,
        margin_inner,
        margin_outer,
        margin_top,
        margin_bottom,
        project,
        assets,
        libraries,
        imports,
        actions,
        tokens,
        styles,
        components,
        masters,
        sections,
        provenance,
        variants,
        recipes,
        diagnostic_policy,
        brand_contract,
        body,
    })
}
