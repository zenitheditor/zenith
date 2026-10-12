//! The usvg font database for SVG assets, built from the registered faces.

use std::sync::Arc;

use resvg::usvg::fontdb::{Database, Source};
use zenith_core::FontProvider;

/// A font database over every face of `fonts`, in `all_faces()` order.
///
/// Each face shares the provider's bytes (`Arc`), so a database copies no
/// font data. Generic families map to the bundled Noto faces. No system
/// font is read.
pub(crate) fn font_database(fonts: &dyn FontProvider) -> Database {
    let mut database = Database::new();
    database.set_sans_serif_family("Noto Sans");
    database.set_serif_family("Noto Sans");
    database.set_monospace_family("Noto Sans Mono");
    for face in fonts.all_faces() {
        database.load_font_source(Source::Binary(Arc::new(face.bytes)));
    }
    database
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::default_provider;

    #[test]
    fn database_holds_one_face_per_registered_face() {
        let fonts = default_provider();
        let database = font_database(&fonts);
        assert_eq!(database.len(), fonts.all_faces().len());
    }
}
