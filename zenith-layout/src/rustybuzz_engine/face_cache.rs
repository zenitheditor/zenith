//! Font bytes for one render and the faces parsed from them.
//!
//! [`FontFaceStore`] owns the bytes. [`FaceCache`] borrows a store and parses
//! each face at most once, on first use. Parsed faces borrow the store, so no
//! struct refers to itself and no `unsafe` is needed.

use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use zenith_core::{FontData, FontProvider};

use crate::error::LayoutError;

/// Error for a face whose bytes fail to parse.
pub(super) fn parse_error(data: &FontData) -> LayoutError {
    LayoutError::new(format!(
        "failed to parse font face for '{}' (index {})",
        data.id, data.index
    ))
}

/// Font bytes for one render, read once from a [`FontProvider`].
///
/// Create one store per render, then build a
/// [`RustybuzzEngine`](super::RustybuzzEngine) over it. The engine parses each
/// stored face at most once, however many runs it shapes.
///
/// The store holds `Arc` clones of the provider's bytes, so it is cheap to
/// build. A face the provider resolves later is served from the store only if
/// its id, bytes, and face index all match. Any other face is parsed per call,
/// so results never depend on the store.
#[derive(Debug, Clone)]
pub struct FontFaceStore {
    /// Faces in `provider.all_faces()` order. The position is the slot.
    faces: Vec<FontData>,
    /// Font id to slot. The first face with a given id wins.
    by_id: BTreeMap<String, usize>,
}

impl FontFaceStore {
    /// Read every face `provider` registers.
    #[must_use]
    pub fn new(provider: &dyn FontProvider) -> Self {
        Self::from_faces(provider.all_faces())
    }

    /// Build a store over `faces`, keeping their order as slot order.
    pub(super) fn from_faces(faces: Vec<FontData>) -> Self {
        let mut by_id = BTreeMap::new();
        for (slot, data) in faces.iter().enumerate() {
            by_id.entry(data.id.clone()).or_insert(slot);
        }
        Self { faces, by_id }
    }

    /// Number of stored faces.
    pub(super) fn face_count(&self) -> usize {
        self.faces.len()
    }

    /// Font data at `slot`.
    pub(super) fn data(&self, slot: usize) -> Option<&FontData> {
        self.faces.get(slot)
    }

    /// Slot that holds exactly `data`: same id, same bytes, same face index.
    ///
    /// Bytes compare by `Arc` identity, so the check is O(1).
    pub(super) fn slot_of(&self, data: &FontData) -> Option<usize> {
        let slot = *self.by_id.get(&data.id)?;
        let held = self.faces.get(slot)?;
        (Arc::ptr_eq(&held.bytes, &data.bytes) && held.index == data.index).then_some(slot)
    }
}

/// Lazily parsed `rustybuzz` faces over one [`FontFaceStore`].
///
/// Slot `i` caches the parse of store face `i`. A face that fails to parse
/// caches `None`, so a bad face is also tried only once.
pub(super) struct FaceCache<'s> {
    store: &'s FontFaceStore,
    slots: Vec<OnceCell<Option<rustybuzz::Face<'s>>>>,
}

impl<'s> FaceCache<'s> {
    /// Build an empty cache over `store`. Nothing parses yet.
    pub(super) fn new(store: &'s FontFaceStore) -> Self {
        let slots = store.faces.iter().map(|_| OnceCell::new()).collect();
        Self { store, slots }
    }

    /// The store this cache reads from.
    pub(super) fn store(&self) -> &'s FontFaceStore {
        self.store
    }

    /// Parsed face at `slot`, parsed on first use.
    ///
    /// Returns `None` when `slot` is out of range or the bytes fail to parse.
    pub(super) fn face(&self, slot: usize) -> Option<&rustybuzz::Face<'s>> {
        let store: &'s FontFaceStore = self.store;
        self.slots
            .get(slot)?
            .get_or_init(|| {
                let data = store.data(slot)?;
                rustybuzz::Face::from_slice(&data.bytes, data.index)
            })
            .as_ref()
    }

    /// Number of slots that hold a parse result.
    #[cfg(test)]
    pub(super) fn parsed_count(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.get().is_some())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use zenith_core::default_provider;

    use super::*;

    #[test]
    fn store_keeps_provider_order() {
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let ids: Vec<String> = provider.all_faces().into_iter().map(|d| d.id).collect();
        let stored: Vec<String> = (0..store.face_count())
            .filter_map(|slot| store.data(slot).map(|d| d.id.clone()))
            .collect();
        assert_eq!(stored, ids);
    }

    #[test]
    fn slot_of_requires_same_bytes() {
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let own = provider
            .by_id("noto-sans-400-normal")
            .expect("bundled face");
        assert!(store.slot_of(&own).is_some(), "same Arc must hit the store");

        // A second provider holds the same font in a different allocation.
        let other = default_provider()
            .by_id("noto-sans-400-normal")
            .expect("bundled face");
        assert!(
            store.slot_of(&other).is_none(),
            "different bytes must miss the store"
        );
    }

    #[test]
    fn face_parses_once_and_on_demand() {
        let provider = default_provider();
        let store = FontFaceStore::new(&provider);
        let cache = FaceCache::new(&store);
        assert_eq!(cache.parsed_count(), 0, "a new cache parses nothing");

        let first = cache.face(0).expect("bundled face parses");
        let second = cache.face(0).expect("bundled face parses");
        assert!(
            std::ptr::eq(first, second),
            "a second read reuses the parse"
        );
        assert_eq!(cache.parsed_count(), 1);
        assert!(
            cache.face(store.face_count()).is_none(),
            "out of range slot"
        );
        assert_eq!(cache.parsed_count(), 1);
    }

    #[test]
    fn unparsable_face_caches_none() {
        let bad: Arc<[u8]> = Arc::from(vec![0u8; 16].as_slice());
        let store = FontFaceStore::from_faces(vec![FontData {
            id: "bad".to_owned(),
            bytes: bad,
            index: 0,
            source: zenith_core::FontSource::Project,
        }]);
        let cache = FaceCache::new(&store);
        assert!(cache.face(0).is_none());
        assert_eq!(cache.parsed_count(), 1, "the failed parse is cached");
    }
}
