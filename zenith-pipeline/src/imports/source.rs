//! Import-source string parsing: `import#component.id` / `import#page.id`.

/// A parsed `source="import#target"` reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportSource<'a> {
    /// `import#component.<id>`.
    Component {
        /// The import id.
        import_id: &'a str,
        /// The component id inside the import.
        component_id: &'a str,
    },
    /// `import#page.<id>`.
    Page {
        /// The import id.
        import_id: &'a str,
        /// The page id inside the import.
        page_id: &'a str,
    },
    /// A well-formed reference to a target kind other than component or page.
    UnsupportedTarget,
    /// Not of the form `import#target`.
    Invalid,
}

/// Parse an import `source` reference.
#[must_use]
pub fn parse_import_source(source: &str) -> ImportSource<'_> {
    let Some((import_id, target)) = source.split_once('#') else {
        return ImportSource::Invalid;
    };
    if import_id.is_empty() || target.is_empty() || target.contains('#') {
        return ImportSource::Invalid;
    }

    if let Some(component_id) = target.strip_prefix("component.") {
        if component_id.is_empty() {
            return ImportSource::Invalid;
        }
        return ImportSource::Component {
            import_id,
            component_id,
        };
    }

    if let Some(page_id) = target.strip_prefix("page.") {
        if page_id.is_empty() {
            return ImportSource::Invalid;
        }
        return ImportSource::Page { import_id, page_id };
    }

    ImportSource::UnsupportedTarget
}
