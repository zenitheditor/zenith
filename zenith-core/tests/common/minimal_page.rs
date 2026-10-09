//! `minimal_page`, shared by several `validate_*` test binaries. Needs the sibling
//! `px` module declared at the binary root.

use super::px::px;
use zenith_core::{ConstructionBlock, Node, Page};

pub fn minimal_page(id: &str, children: Vec<Node>) -> Page {
    Page {
        id: id.to_owned(),
        name: None,
        source: None,
        fit: None,
        width: px(1280.0),
        height: px(720.0),
        background: None,
        bleed: None,
        margin_inner: None,
        margin_outer: None,
        margin_top: None,
        margin_bottom: None,
        baseline_grid: None,
        line_jumps: None,
        parity: None,
        master: None,
        safe_zones: Vec::new(),
        folds: Vec::new(),
        construction: ConstructionBlock::default(),
        ports: Vec::new(),
        block_styles: Vec::new(),
        defaults: Default::default(),
        children,
        source_span: None,
    }
}
