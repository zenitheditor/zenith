//! The token scope and fit transform of expanded content: how a `group` that
//! stands in for an expanded `instance` draws its children.

use std::collections::BTreeMap;

use crate::ast::style::Style;
use crate::tokens::ResolvedToken;

/// The `w` / `h` fit transform an expanded subtree draws under. A local
/// point `(x, y)` lands at `origin + (tx + sx·x, ty + sy·y)`, where `origin`
/// is the page position of the parent's local `(0, 0)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScopeFit {
    pub sx: f64,
    pub sy: f64,
    pub tx: f64,
    pub ty: f64,
}

impl ScopeFit {
    /// `true` when the transform scales (a pure translation is not a scale).
    pub fn scales(&self) -> bool {
        self.sx != 1.0 || self.sy != 1.0
    }
}

/// The resolved tokens and styles of one document.
#[derive(Clone, Copy, Debug)]
pub struct ScopeTokens<'a> {
    pub resolved: &'a BTreeMap<String, ResolvedToken>,
    pub styles: &'a BTreeMap<&'a str, &'a Style>,
}

/// How the `group` of one id draws its children. The contrast walks apply it
/// to the group whose id is the key in [`ContentScopes`].
#[derive(Clone, Copy, Debug)]
pub struct ContentScope<'a> {
    /// The resolved tokens and styles of the subtree's own document (an
    /// imported component). `None`: the enclosing scope.
    pub tokens: Option<ScopeTokens<'a>>,
    /// The fit transform, in place of the group's `x` / `y` offset. `None`:
    /// the group offsets its children by its `x` / `y`.
    pub fit: Option<ScopeFit>,
}

/// The scoped groups of one page, by group id.
pub type ContentScopes<'a> = BTreeMap<String, ContentScope<'a>>;
