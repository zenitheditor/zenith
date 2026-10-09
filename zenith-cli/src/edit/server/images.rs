//! [`ImageCache`]: recent PNG renders, addressed by their SHA-256.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

/// Most images kept.
const MAX_ENTRIES: usize = 32;
/// Most PNG bytes kept.
const MAX_BYTES: usize = 128 * 1024 * 1024;

/// A bounded cache of PNGs keyed by lowercase hex SHA-256. The oldest entry
/// goes first when a bound is passed.
#[derive(Debug, Default)]
pub(crate) struct ImageCache {
    order: VecDeque<String>,
    map: BTreeMap<String, Arc<Vec<u8>>>,
    bytes: usize,
}

impl ImageCache {
    /// Store `png` under `sha256`.
    pub(crate) fn insert(&mut self, sha256: String, png: Vec<u8>) {
        if self.map.contains_key(&sha256) {
            return;
        }
        self.bytes += png.len();
        self.order.push_back(sha256.clone());
        self.map.insert(sha256, Arc::new(png));
        while self.order.len() > MAX_ENTRIES || (self.bytes > MAX_BYTES && self.order.len() > 1) {
            let Some(old) = self.order.pop_front() else {
                break;
            };
            if let Some(png) = self.map.remove(&old) {
                self.bytes -= png.len();
            }
        }
    }

    /// The PNG stored under `sha256`.
    pub(crate) fn get(&self, sha256: &str) -> Option<Arc<Vec<u8>>> {
        self.map.get(sha256).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oldest_entries_go_first() {
        let mut c = ImageCache::default();
        for i in 0..(MAX_ENTRIES + 2) {
            c.insert(format!("{i}"), vec![0; 4]);
        }
        assert!(c.get("0").is_none());
        assert!(c.get("1").is_none());
        assert!(c.get("2").is_some());
        assert_eq!(c.bytes, MAX_ENTRIES * 4);
    }
}
