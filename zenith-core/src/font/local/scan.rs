//! Local/system font scanning.
//!
//! Reads font files from a caller-supplied list of directories and extracts the
//! family / weight / style of every face, so the CLI can register machine-local
//! fonts as a LAST-RESORT resolution source (after bundled + project fonts).
//!
//! The scan returns metadata only. It reads each file once and drops the bytes.
//! The caller reads the bytes of the few faces it registers.
//!
//! ## Determinism boundary
//!
//! This module reads font FILES but does NOT enumerate OS font locations: the
//! caller passes the directory list, and the optional index path. OS-directory
//! and cache-directory discovery live in the CLI.
//!
//! Output is deterministic for a given set of directory contents: files are
//! sorted by path, and the returned entries are sorted by
//! `(family, weight, style, path, index)`. The index never changes the output,
//! only the work done to produce it.
//!
//! No `unwrap`/`expect`/`panic!`: every IO or parse failure is skipped.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ttf_parser::name_id;

use super::index::{self, FaceMeta, Index, IndexFile};
use crate::font::FontStyle;

/// Upper bound on faces probed inside a single font collection (`.ttc`). Guards
/// against a malformed collection header advertising an unbounded face count.
const MAX_COLLECTION_FACES: u32 = 64;

/// Maximum subdirectory depth walked under each font root. Font directories nest
/// only a few levels; this cap bounds the walk and terminates symlink cycles.
const MAX_SCAN_DEPTH: u32 = 8;

/// Metadata of a single local/system font face found by [`scan_font_dirs`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFontEntry {
    /// Path to the font file on disk, as walked from the scanned directories.
    pub path: PathBuf,
    /// Typographic family name (e.g. `"Inter"`), preferring name ID 16 over 1.
    pub family: String,
    /// Numeric weight (e.g. 400, 700).
    pub weight: u16,
    /// Normal or italic style.
    pub style: FontStyle,
    /// Face index within the file (0 for single-face files; >0 for `.ttc`).
    pub index: u32,
}

/// Scan each directory in `dirs` for font files and return the metadata of every
/// readable face. No font bytes are retained.
///
/// Directories that do not exist or cannot be read are skipped. Files whose
/// extension is `ttf`, `otf`, or `ttc` (case-insensitive) are collected and
/// sorted by path. For a collection, faces are probed from index 0 upward until
/// parsing fails or `MAX_COLLECTION_FACES` is reached. Faces with no readable
/// family name are skipped.
///
/// When `index_path` is `Some`, the scan reads the metadata index there and
/// re-parses only files whose `(size, mtime)` changed, plus new files. Removed
/// files are dropped from the index. A missing or corrupt index is rebuilt. A
/// failed index write is ignored.
///
/// The returned `Vec` is sorted by `(family, weight, style, path, index)`.
#[must_use]
pub fn scan_font_dirs(dirs: &[PathBuf], index_path: Option<&Path>) -> Vec<LocalFontEntry> {
    let files = collect_font_files(dirs);
    let old: Index = index_path.map(index::load).unwrap_or_default();
    let mut fresh = Index::new();
    for path in files {
        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let size = meta.len();
        let mtime_ns = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        let file = match old.get(&path) {
            Some(f) if f.size == size && f.mtime_ns == mtime_ns => f.clone(),
            _ => {
                let bytes = match std::fs::read(&path) {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                IndexFile {
                    size,
                    mtime_ns,
                    faces: parse_faces(&bytes),
                }
            }
        };
        fresh.insert(path, file);
    }

    if let Some(p) = index_path
        && index::render(&fresh) != index::render(&old)
    {
        index::store(p, &fresh);
    }

    let mut entries: Vec<LocalFontEntry> = Vec::new();
    for (path, file) in fresh {
        for face in file.faces {
            entries.push(LocalFontEntry {
                path: path.clone(),
                family: face.family,
                weight: face.weight,
                style: face.style,
                index: face.index,
            });
        }
    }
    entries.sort_by(|a, b| {
        a.family
            .cmp(&b.family)
            .then(a.weight.cmp(&b.weight))
            .then(a.style.cmp(&b.style))
            .then(a.path.cmp(&b.path))
            .then(a.index.cmp(&b.index))
    });
    entries
}

/// Keep only the entries whose family is in `wanted`. Comparison is
/// case-insensitive, the same as `BytesFontProvider::resolve`. Order is kept.
#[must_use]
pub fn filter_wanted_families(
    entries: Vec<LocalFontEntry>,
    wanted: &BTreeSet<String>,
) -> Vec<LocalFontEntry> {
    let wanted_lower: BTreeSet<String> = wanted.iter().map(|w| w.to_lowercase()).collect();
    entries
        .into_iter()
        .filter(|e| wanted_lower.contains(&e.family.to_lowercase()))
        .collect()
}

/// Walk `dirs` and return every font file path, sorted and deduplicated.
fn collect_font_files(dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    // A depth-capped worklist bounds the walk and terminates on symlink cycles
    // without a visited set.
    let mut worklist: Vec<(PathBuf, u32)> = dirs.iter().map(|d| (d.clone(), 0u32)).collect();
    while let Some((dir, depth)) = worklist.pop() {
        let read = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for entry in read {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            // `file_type()` from the DirEntry avoids a `stat` per entry.
            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            let path = entry.path();
            if is_dir {
                if depth < MAX_SCAN_DEPTH {
                    worklist.push((path, depth + 1));
                }
            } else if has_font_extension(&path) {
                files.push(path);
            }
        }
    }
    files.sort();
    files.dedup();
    files
}

/// Parse every face in `bytes`. An unparseable file yields no faces.
fn parse_faces(bytes: &[u8]) -> Vec<FaceMeta> {
    let mut faces = Vec::new();
    for index in 0..MAX_COLLECTION_FACES {
        let face = match ttf_parser::Face::parse(bytes, index) {
            Ok(f) => f,
            // A failed parse at index 0 means the file is unreadable. At a
            // higher index it means the collection has no further faces.
            Err(_) => break,
        };
        let family = match best_family_name(&face) {
            Some(f) => f,
            None => continue,
        };
        let style = if face.is_italic() {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        };
        faces.push(FaceMeta {
            index,
            family,
            weight: face.weight().to_number(),
            style,
        });
    }
    faces
}

/// True when `path` has a `ttf`, `otf`, or `ttc` extension (case-insensitive).
fn has_font_extension(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => {
            let ext = ext.to_ascii_lowercase();
            ext == "ttf" || ext == "otf" || ext == "ttc"
        }
        None => false,
    }
}

/// Return the best available family name from a face's name table.
///
/// Prefers name ID 16 (Typographic Family) over name ID 1 (Family). Mirrors the
/// strategy in `zenith-layout`'s `font_meta::best_family_name` so a local face
/// registers under the same family string a project asset would.
fn best_family_name(face: &ttf_parser::Face<'_>) -> Option<String> {
    let mut typo_family: Option<String> = None;
    let mut family: Option<String> = None;

    for name in face.names() {
        if name.name_id == name_id::TYPOGRAPHIC_FAMILY
            && typo_family.is_none()
            && let Some(s) = name.to_string()
        {
            typo_family = Some(s);
        } else if name.name_id == name_id::FAMILY
            && family.is_none()
            && let Some(s) = name.to_string()
        {
            family = Some(s);
        }
    }

    typo_family.or(family)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The workspace bundled-fonts directory, used as a real, committed fixture.
    fn bundled_fonts_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/fonts")
    }

    /// Create a unique empty temp directory for one test.
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "zenith-local-font-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn copy_bundled(name: &str, to: &Path) {
        std::fs::copy(bundled_fonts_dir().join(name), to).expect("copy fixture font");
    }

    #[test]
    fn scans_bundled_fonts_dir_for_noto_sans() {
        let entries = scan_font_dirs(&[bundled_fonts_dir()], None);
        assert!(
            entries.iter().any(|e| e.family.contains("Noto Sans")),
            "expected a 'Noto Sans' family, got: {:?}",
            entries.iter().map(|e| &e.family).collect::<Vec<_>>()
        );
        assert!(
            entries
                .iter()
                .any(|e| e.weight == 400 && e.style == FontStyle::Normal),
            "expected at least one 400/Normal face"
        );
    }

    #[test]
    fn extracts_weight_and_style_variants() {
        let entries = scan_font_dirs(&[bundled_fonts_dir()], None);
        assert!(entries.iter().any(|e| e.weight == 700));
        assert!(entries.iter().any(|e| e.style == FontStyle::Italic));
    }

    #[test]
    fn parse_faces_reads_metadata_from_bytes() {
        let bytes =
            std::fs::read(bundled_fonts_dir().join("NotoSans-BoldItalic.ttf")).expect("read");
        let faces = parse_faces(&bytes);
        assert_eq!(faces.len(), 1);
        assert_eq!(faces[0].family, "Noto Sans");
        assert_eq!(faces[0].weight, 700);
        assert_eq!(faces[0].style, FontStyle::Italic);
        assert_eq!(faces[0].index, 0);
        assert!(parse_faces(b"not a font").is_empty());
    }

    #[test]
    fn missing_dir_and_empty_list_yield_empty() {
        assert!(
            scan_font_dirs(&[PathBuf::from("/this/path/does/not/exist/zenith")], None).is_empty()
        );
        assert!(scan_font_dirs(&[], None).is_empty());
    }

    #[test]
    fn output_is_sorted_and_deterministic() {
        let a = scan_font_dirs(&[bundled_fonts_dir()], None);
        let b = scan_font_dirs(&[bundled_fonts_dir()], None);
        assert_eq!(a, b);
        let mut sorted = a.clone();
        sorted.sort_by(|x, y| {
            x.family
                .cmp(&y.family)
                .then(x.weight.cmp(&y.weight))
                .then(x.style.cmp(&y.style))
                .then(x.path.cmp(&y.path))
                .then(x.index.cmp(&y.index))
        });
        assert_eq!(a, sorted);
    }

    #[test]
    fn non_font_extensions_are_skipped() {
        for e in &scan_font_dirs(&[bundled_fonts_dir()], None) {
            assert!(has_font_extension(&e.path));
        }
    }

    #[test]
    fn wanted_filter_is_case_insensitive_and_exact() {
        let entries = scan_font_dirs(&[bundled_fonts_dir()], None);
        let wanted: BTreeSet<String> = ["NOTO serif".to_owned()].into();
        let kept = filter_wanted_families(entries.clone(), &wanted);
        assert!(!kept.is_empty());
        assert!(kept.iter().all(|e| e.family == "Noto Serif"));
        let none: BTreeSet<String> = ["Noto".to_owned()].into();
        assert!(filter_wanted_families(entries, &none).is_empty());
        assert!(filter_wanted_families(Vec::new(), &wanted).is_empty());
    }

    #[test]
    fn index_is_written_and_gives_same_entries() {
        let dir = temp_dir("roundtrip");
        let fonts = dir.join("fonts");
        std::fs::create_dir_all(&fonts).expect("mkdir");
        copy_bundled("NotoSans-Regular.ttf", &fonts.join("a.ttf"));
        let idx = dir.join("cache").join("index.txt");

        let first = scan_font_dirs(std::slice::from_ref(&fonts), Some(&idx));
        assert!(idx.exists(), "index must be written");
        let second = scan_font_dirs(std::slice::from_ref(&fonts), Some(&idx));
        assert_eq!(first, second);
        assert_eq!(first, scan_font_dirs(std::slice::from_ref(&fonts), None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn index_entry_is_trusted_while_stamp_matches() {
        let dir = temp_dir("trust");
        let fonts = dir.join("fonts");
        std::fs::create_dir_all(&fonts).expect("mkdir");
        let font = fonts.join("a.ttf");
        copy_bundled("NotoSans-Regular.ttf", &font);
        let idx = dir.join("index.txt");
        let _ = scan_font_dirs(std::slice::from_ref(&fonts), Some(&idx));

        // Rewrite the cached family. A matching stamp makes the scan return it.
        let text = std::fs::read_to_string(&idx).expect("read index");
        std::fs::write(&idx, text.replace("Noto Sans", "Cached Name")).expect("write index");
        let entries = scan_font_dirs(std::slice::from_ref(&fonts), Some(&idx));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].family, "Cached Name");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn changed_size_invalidates_entry() {
        let dir = temp_dir("stale");
        let fonts = dir.join("fonts");
        std::fs::create_dir_all(&fonts).expect("mkdir");
        let font = fonts.join("a.ttf");
        copy_bundled("NotoSans-Regular.ttf", &font);
        let idx = dir.join("index.txt");
        let first = scan_font_dirs(std::slice::from_ref(&fonts), Some(&idx));
        assert_eq!(first[0].family, "Noto Sans");

        // Different file, different size: the stamp no longer matches.
        copy_bundled("NotoSerif-Regular.ttf", &font);
        let second = scan_font_dirs(std::slice::from_ref(&fonts), Some(&idx));
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].family, "Noto Serif");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn changed_mtime_invalidates_entry() {
        let dir = temp_dir("mtime");
        let fonts = dir.join("fonts");
        std::fs::create_dir_all(&fonts).expect("mkdir");
        copy_bundled("NotoSans-Regular.ttf", &fonts.join("a.ttf"));
        let idx = dir.join("index.txt");
        let _ = scan_font_dirs(std::slice::from_ref(&fonts), Some(&idx));

        // Tamper the cached name and the cached mtime: the entry is re-parsed.
        let text = std::fs::read_to_string(&idx).expect("read index");
        let mut tampered = String::new();
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("F\t") {
                let mut f = rest.splitn(3, '\t');
                let size = f.next().unwrap_or("0");
                let _ = f.next();
                let path = f.next().unwrap_or("");
                tampered.push_str(&format!("F\t{size}\t12345\t{path}\n"));
            } else {
                tampered.push_str(&line.replace("Noto Sans", "Cached Name"));
                tampered.push('\n');
            }
        }
        std::fs::write(&idx, tampered).expect("write index");
        let entries = scan_font_dirs(std::slice::from_ref(&fonts), Some(&idx));
        assert_eq!(entries[0].family, "Noto Sans");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn removed_file_is_dropped_from_index() {
        let dir = temp_dir("removed");
        let fonts = dir.join("fonts");
        std::fs::create_dir_all(&fonts).expect("mkdir");
        copy_bundled("NotoSans-Regular.ttf", &fonts.join("a.ttf"));
        copy_bundled("NotoSerif-Regular.ttf", &fonts.join("b.ttf"));
        let idx = dir.join("index.txt");
        assert_eq!(
            scan_font_dirs(std::slice::from_ref(&fonts), Some(&idx)).len(),
            2
        );

        std::fs::remove_file(fonts.join("b.ttf")).expect("remove");
        assert_eq!(
            scan_font_dirs(std::slice::from_ref(&fonts), Some(&idx)).len(),
            1
        );
        let text = std::fs::read_to_string(&idx).expect("read index");
        assert!(!text.contains("b.ttf"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_index_is_ignored_and_rebuilt() {
        let dir = temp_dir("corrupt");
        let fonts = dir.join("fonts");
        std::fs::create_dir_all(&fonts).expect("mkdir");
        copy_bundled("NotoSans-Regular.ttf", &fonts.join("a.ttf"));
        let idx = dir.join("index.txt");
        std::fs::write(&idx, b"\xff\xfe garbage").expect("write garbage");

        let entries = scan_font_dirs(std::slice::from_ref(&fonts), Some(&idx));
        assert_eq!(entries.len(), 1);
        let text = std::fs::read_to_string(&idx).expect("rebuilt index is text");
        assert!(text.starts_with("zenith-font-index v1\n"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unwritable_index_path_is_ignored() {
        let dir = temp_dir("nowrite");
        let fonts = dir.join("fonts");
        std::fs::create_dir_all(&fonts).expect("mkdir");
        copy_bundled("NotoSans-Regular.ttf", &fonts.join("a.ttf"));
        // The parent of the index path is a regular file, so create_dir_all fails.
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, b"x").expect("write blocker");
        let idx = blocker.join("index.txt");
        let entries = scan_font_dirs(std::slice::from_ref(&fonts), Some(&idx));
        assert_eq!(entries.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
