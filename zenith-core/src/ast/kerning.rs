//! Authored manual kerning data shared by text-bearing node AST structs.

use crate::ast::value::PropertyValue;

/// One authored manual kerning pair adjustment on a text-bearing node.
#[derive(Debug, Clone, PartialEq)]
pub struct KerningPair {
    pub left: String,
    pub right: String,
    pub by: PropertyValue,
}
