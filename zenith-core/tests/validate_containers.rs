//! Integration tests: containers validation.
//!
//! Test bodies moved verbatim from the former in-`src` `validate/check/tests/`
//! concern files; only import paths changed (`crate::`/`super::common` ->
//! `zenith_core::`/`common`).

use std::collections::BTreeMap;

#[path = "common/bounded_page_rect_at.rs"]
mod bounded_page_rect_at;
#[path = "common/codes.rs"]
mod codes;
#[path = "common/color_token.rs"]
mod color_token;
#[path = "common/has_code.rs"]
mod has_code;
#[path = "common/minimal_page.rs"]
mod minimal_page;
#[path = "common/minimal_rect.rs"]
mod minimal_rect;
#[path = "common/px.rs"]
mod px;
#[path = "common/pxv_doc_with.rs"]
mod pxv_doc_with;
#[path = "common/token_ref.rs"]
mod token_ref;

use bounded_page_rect_at::{bounded_page, rect_at};
use codes::codes;
use color_token::color_token;
use has_code::has_code;
use minimal_page::minimal_page;
use minimal_rect::minimal_rect;
use pxv_doc_with::{doc_with, pxv};
use token_ref::token_ref;
use zenith_core::format::format_document;
use zenith_core::{
    FrameNode, GroupNode, KdlAdapter, KdlSource, Node, RectNode, Severity, validate,
};

#[path = "validate_containers/frame.rs"]
mod frame;
#[path = "validate_containers/group.rs"]
mod group;
#[path = "validate_containers/semantic.rs"]
mod semantic;
