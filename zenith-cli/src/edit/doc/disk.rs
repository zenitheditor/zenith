//! What the document looks like on disk: file stamps and text reads.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A file's modification time and length. A change in either means the file
/// changed. Equal stamps on a recently written file get a content check,
/// since some filesystems keep mtime in whole seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Stamp {
    /// Modification time, in milliseconds since the Unix epoch.
    pub(crate) mtime_ms: u64,
    /// Length in bytes.
    pub(crate) len: u64,
}

/// How long after its mtime a file still gets a content check on every
/// poll.
const RECENT: Duration = Duration::from_secs(3);

impl Stamp {
    /// `true` when the file changed less than [`RECENT`] ago (or its mtime is
    /// in the future).
    pub(crate) fn recent(&self) -> bool {
        let now = now_ms();
        self.mtime_ms.saturating_add(duration_ms(RECENT)) >= now
    }
}

/// The file at a path, as one poll sees it.
pub(crate) enum DiskRead {
    /// The file holds UTF-8 text.
    Text(String, Stamp),
    /// No file exists at the path.
    Missing,
    /// The file exists but cannot be read as UTF-8 text.
    Unreadable(String),
}

/// The stamp of `path`, or `None` when it does not exist or has no
/// metadata.
pub(crate) fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, duration_ms);
    Some(Stamp {
        mtime_ms,
        len: meta.len(),
    })
}

/// Read `path` as text, with its stamp.
pub(crate) fn read(path: &Path) -> DiskRead {
    let before = stamp(path);
    match std::fs::read(path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => {
                let len = text.len() as u64;
                let stamp = before.map_or(Stamp { mtime_ms: 0, len }, |s| Stamp { len, ..s });
                DiskRead::Text(text, stamp)
            }
            Err(_) => DiskRead::Unreadable(format!(
                "'{}' is not valid UTF-8; save it as UTF-8 text",
                path.display()
            )),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => DiskRead::Missing,
        Err(e) => DiskRead::Unreadable(format!("cannot read '{}': {e}", path.display())),
    }
}

/// Milliseconds since the Unix epoch now.
pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, duration_ms)
}

fn duration_ms(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_reports_text_missing_and_binary() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("a.zen");
        assert!(matches!(read(&path), DiskRead::Missing));
        std::fs::write(&path, "abc").expect("write");
        match read(&path) {
            DiskRead::Text(text, stamp) => {
                assert_eq!(text, "abc");
                assert_eq!(stamp.len, 3);
                assert!(stamp.recent());
            }
            DiskRead::Missing | DiskRead::Unreadable(_) => panic!("expected text"),
        }
        std::fs::write(&path, [0xff, 0xfe]).expect("write");
        assert!(matches!(read(&path), DiskRead::Unreadable(_)));
    }
}
