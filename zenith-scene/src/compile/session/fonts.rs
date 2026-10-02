//! Font provider handle that turns any `&F: FontProvider` into `&dyn FontProvider`.

use zenith_core::{FontData, FontProvider, FontStyle};

/// Borrowed provider of any type, sized so it coerces to `&dyn FontProvider`.
///
/// [`PageCompiler`](crate::compile::PageCompiler) stores `&F` so a `Sync` provider keeps
/// the compiler `Sync`. The node compilers take `&dyn FontProvider`. Every call
/// forwards unchanged, so resolved faces and their bytes stay identical.
pub(super) struct FontsRef<'a, F: ?Sized>(pub(super) &'a F);

impl<F: ?Sized + FontProvider> FontProvider for FontsRef<'_, F> {
    fn resolve(&self, families: &[String], weight: u16, style: FontStyle) -> Option<FontData> {
        self.0.resolve(families, weight, style)
    }

    fn by_id(&self, id: &str) -> Option<FontData> {
        self.0.by_id(id)
    }

    fn all_faces(&self) -> Vec<FontData> {
        self.0.all_faces()
    }
}
