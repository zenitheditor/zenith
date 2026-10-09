//! `pxv` and `doc_with`, shared by the `validate_*` test binaries that build
//! documents. Needs the sibling `px` module declared at the binary root.

use super::px::px;
use zenith_core::{
    AssetBlock, BrandContract, Document, DocumentBody, Page, PropertyValue, StyleBlock, Token,
    TokenBlock,
};

/// A raw `(px)v` dimension wrapped as a geometry `PropertyValue`, for the
/// `x`/`y`/`w`/`h` fields that now accept a dimension literal OR a token ref.
pub fn pxv(v: f64) -> PropertyValue {
    PropertyValue::Dimension(px(v))
}

pub fn doc_with(tokens: Vec<Token>, pages: Vec<Page>) -> Document {
    Document {
        version: 1,
        colorspace: None,
        doc_id: None,
        mirror_margins: None,
        facing_pages: None,
        spread_gutter: None,
        page_progression: None,
        page_parity_start: None,
        margin_inner: None,
        margin_outer: None,
        margin_top: None,
        margin_bottom: None,
        project: None,
        assets: AssetBlock::default(),
        libraries: Vec::new(),
        imports: Vec::new(),
        actions: Vec::new(),
        tokens: TokenBlock {
            format: "zenith-token-v1".to_owned(),
            tokens,
        },
        styles: StyleBlock::default(),
        defaults: Default::default(),
        components: Vec::new(),
        masters: Vec::new(),
        sections: Vec::new(),
        provenance: Vec::new(),
        variants: Vec::new(),
        recipes: Vec::new(),
        diagnostic_policy: zenith_core::DiagnosticPolicy::default(),
        brand_contract: BrandContract::default(),
        body: DocumentBody {
            id: "doc.main".to_owned(),
            title: None,
            block_styles: Vec::new(),
            pages,
        },
        unsupported_children: Vec::new(),
    }
}
