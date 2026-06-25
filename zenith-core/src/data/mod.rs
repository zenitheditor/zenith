//! Runtime data-binding support.
//!
//! Provides [`DataContext`] (a flat `BTreeMap<String, String>` of named field
//! values) and the [`DataFormat`] / [`format_data_value`] formatter that turns
//! raw field strings into locale-styled display strings deterministically.

use std::collections::BTreeMap;

pub mod format;

pub use format::{DataFormat, format_data_value};

/// A flat map of named data fields available at scene-compile time.
///
/// Keys are dot-separated paths, e.g. `"revenue.total"`. Values are raw
/// strings; callers apply [`format_data_value`] to style them.
///
/// Uses [`BTreeMap`] for deterministic iteration order on the render path.
#[derive(Debug, Clone, Default)]
pub struct DataContext {
    /// The field map. Keyed by dotted path, value is the raw string.
    pub fields: BTreeMap<String, String>,
}

impl DataContext {
    /// Look up a field value by `path`.
    ///
    /// Returns `None` when the path is not present in this context.
    pub fn get(&self, path: &str) -> Option<&str> {
        self.fields.get(path).map(String::as_str)
    }
}
