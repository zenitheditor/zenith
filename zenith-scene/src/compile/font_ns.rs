//! Font-provider wrapper that namespaces an imported subtree's font families.
//!
//! Imported `.zen` font assets are registered into the shared provider under a
//! namespaced family (`"{import_id}/{family}"`, see the CLI's
//! `register_project_fonts`) so a host font and an imported font that share a
//! real family name do not shadow or merge each other. When an imported subtree
//! is compiled, its text still requests the *plain* family name, so its font
//! resolution is routed through this wrapper: for each requested family it tries
//! the namespaced family first, then the plain family.

use zenith_core::{FontData, FontProvider, FontStyle};

/// Wraps a [`FontProvider`] so that an imported subtree's font families resolve
/// to the import-namespaced face first, then fall back to the plain family.
///
/// For a resolution request `[f1, f2, …]` the wrapper delegates the interleaved
/// priority list `[prefix/f1, f1, prefix/f2, f2, …]` to `inner`. This means:
/// - an imported face declared by the imported document (registered under
///   `prefix/f`) wins for its family, without shadowing the host's face of the
///   same real family name; and
/// - a family the imported document did NOT declare (bundled / system /
///   host-registered) still resolves via the plain-family fallback, and earlier
///   families in the list still outrank later ones.
///
/// `by_id` and `all_faces` delegate unchanged: face ids are already unique
/// across the flat provider, and PDF embedding enumerates every registered face.
pub(in crate::compile) struct NamespacedFontProvider<'a> {
    inner: &'a dyn FontProvider,
    prefix: String,
}

impl<'a> NamespacedFontProvider<'a> {
    /// Wrap `inner`, prefixing requested families with `prefix` (the import id).
    pub(in crate::compile) fn new(inner: &'a dyn FontProvider, prefix: &str) -> Self {
        Self {
            inner,
            prefix: prefix.to_owned(),
        }
    }
}

impl FontProvider for NamespacedFontProvider<'_> {
    fn resolve(&self, families: &[String], weight: u16, style: FontStyle) -> Option<FontData> {
        let mut ordered = Vec::with_capacity(families.len() * 2);
        for family in families {
            ordered.push(format!("{}/{}", self.prefix, family));
            ordered.push(family.clone());
        }
        self.inner.resolve(&ordered, weight, style)
    }

    fn by_id(&self, id: &str) -> Option<FontData> {
        self.inner.by_id(id)
    }

    fn all_faces(&self) -> Vec<FontData> {
        self.inner.all_faces()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use zenith_core::{BytesFontProvider, FontSource};

    use super::*;

    /// The interleaved priority list is `[p/f1, f1, p/f2, f2]`: for each family
    /// the namespaced face wins, but earlier families still outrank later ones.
    #[test]
    fn resolve_prefers_namespaced_face_then_plain_fallback() {
        let mut inner = BytesFontProvider::new();
        let host: Arc<[u8]> = Arc::from(vec![0xAAu8; 8].as_slice());
        let imported: Arc<[u8]> = Arc::from(vec![0xBBu8; 8].as_slice());
        // Host face under the real family "Brand"; imported face under "id/Brand".
        inner.register(
            "Brand",
            400,
            FontStyle::Normal,
            host,
            0,
            FontSource::Project,
        );
        let imported_id = inner.register(
            "id/Brand",
            400,
            FontStyle::Normal,
            imported,
            0,
            FontSource::Project,
        );

        let wrapper = NamespacedFontProvider::new(&inner, "id");
        let resolved = wrapper
            .resolve(&["Brand".to_owned()], 400, FontStyle::Normal)
            .expect("namespaced face resolves");
        // The namespaced face wins over the plain host face of the same name.
        assert_eq!(resolved.id, imported_id);
        assert_eq!(resolved.bytes[0], 0xBB);
    }

    /// A family the imported document did not declare still resolves via the
    /// plain-family fallback (the second entry of each interleaved pair).
    #[test]
    fn resolve_falls_back_to_undeclared_plain_family() {
        let mut inner = BytesFontProvider::new();
        let bytes: Arc<[u8]> = Arc::from(vec![0xCCu8; 8].as_slice());
        inner.register(
            "Shared",
            400,
            FontStyle::Normal,
            bytes,
            0,
            FontSource::Bundled,
        );

        let wrapper = NamespacedFontProvider::new(&inner, "id");
        let resolved = wrapper
            .resolve(&["Shared".to_owned()], 400, FontStyle::Normal)
            .expect("plain family resolves via fallback");
        assert_eq!(resolved.source, FontSource::Bundled);
        assert_eq!(resolved.bytes[0], 0xCC);
    }

    /// Earlier requested families outrank later ones even through the wrapper.
    #[test]
    fn resolve_orders_first_family_before_second() {
        let mut inner = BytesFontProvider::new();
        let first: Arc<[u8]> = Arc::from(vec![0x11u8; 8].as_slice());
        let second: Arc<[u8]> = Arc::from(vec![0x22u8; 8].as_slice());
        inner.register(
            "First",
            400,
            FontStyle::Normal,
            first,
            0,
            FontSource::Bundled,
        );
        inner.register(
            "Second",
            400,
            FontStyle::Normal,
            second,
            0,
            FontSource::Bundled,
        );

        let wrapper = NamespacedFontProvider::new(&inner, "id");
        let resolved = wrapper
            .resolve(
                &["First".to_owned(), "Second".to_owned()],
                400,
                FontStyle::Normal,
            )
            .expect("first family resolves");
        assert_eq!(resolved.bytes[0], 0x11);
    }
}
