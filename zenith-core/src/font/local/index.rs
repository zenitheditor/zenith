//! On-disk metadata index for the local font scan.
//!
//! The index maps each font file to its `(size, mtime)` stamp and the faces
//! parsed from it. A later scan reuses an entry whose stamp still matches and
//! re-parses only changed or new files.
//!
//! Text format, one record per line, tab-separated, sorted by path:
//!
//! ```text
//! zenith-font-index v1
//! F<TAB>size<TAB>mtime_ns<TAB>path
//! S<TAB>face_index<TAB>weight<TAB>n|i<TAB>family
//! ```
//!
//! Each `S` line belongs to the nearest `F` line above it. Any malformed line
//! makes the whole index unreadable, and the caller then rebuilds it.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::font::FontStyle;

const HEADER: &str = "zenith-font-index v1";

/// Metadata of one face inside a font file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FaceMeta {
    pub index: u32,
    pub family: String,
    pub weight: u16,
    pub style: FontStyle,
}

/// Stamp and parsed faces of one font file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct IndexFile {
    pub size: u64,
    pub mtime_ns: u128,
    pub faces: Vec<FaceMeta>,
}

pub(super) type Index = BTreeMap<PathBuf, IndexFile>;

/// True when a record can round-trip through the line format.
fn is_cacheable(path: &Path, file: &IndexFile) -> bool {
    let plain = |s: &str| !s.contains(['\t', '\n', '\r']);
    path.to_str().is_some_and(plain) && file.faces.iter().all(|f| plain(&f.family))
}

/// Render `index` as text. Files that cannot round-trip are left out.
pub(super) fn render(index: &Index) -> String {
    let mut out = String::new();
    out.push_str(HEADER);
    out.push('\n');
    for (path, file) in index {
        let Some(path_str) = path.to_str() else {
            continue;
        };
        if !is_cacheable(path, file) {
            continue;
        }
        let _ = writeln!(out, "F\t{}\t{}\t{}", file.size, file.mtime_ns, path_str);
        for face in &file.faces {
            let style = match face.style {
                FontStyle::Normal => 'n',
                FontStyle::Italic => 'i',
            };
            let _ = writeln!(
                out,
                "S\t{}\t{}\t{}\t{}",
                face.index, face.weight, style, face.family
            );
        }
    }
    out
}

/// Parse index text. Returns `None` on a wrong header or any malformed line.
pub(super) fn parse(text: &str) -> Option<Index> {
    let mut lines = text.lines();
    if lines.next()? != HEADER {
        return None;
    }
    let mut index = Index::new();
    let mut current: Option<(PathBuf, IndexFile)> = None;
    for line in lines {
        let mut parts = line.splitn(5, '\t');
        match parts.next()? {
            "F" => {
                let mut f = line.splitn(4, '\t').skip(1);
                let size = f.next()?.parse().ok()?;
                let mtime_ns = f.next()?.parse().ok()?;
                let path = PathBuf::from(f.next()?);
                if let Some((p, file)) = current.take() {
                    index.insert(p, file);
                }
                current = Some((
                    path,
                    IndexFile {
                        size,
                        mtime_ns,
                        faces: Vec::new(),
                    },
                ));
            }
            "S" => {
                let face_index = parts.next()?.parse().ok()?;
                let weight = parts.next()?.parse().ok()?;
                let style = match parts.next()? {
                    "n" => FontStyle::Normal,
                    "i" => FontStyle::Italic,
                    _ => return None,
                };
                let family = parts.next()?.to_owned();
                let (_, file) = current.as_mut()?;
                file.faces.push(FaceMeta {
                    index: face_index,
                    family,
                    weight,
                    style,
                });
            }
            _ => return None,
        }
    }
    if let Some((p, file)) = current.take() {
        index.insert(p, file);
    }
    Some(index)
}

/// Read the index at `path`. A missing, unreadable, or corrupt file yields an
/// empty index.
pub(super) fn load(path: &Path) -> Index {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| parse(&t))
        .unwrap_or_default()
}

/// Write `index` to `path` through a temp file and a rename. Every error is
/// ignored: the index is an optimization.
pub(super) fn store(path: &Path, index: &Index) {
    if let Some(parent) = path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return;
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    if std::fs::write(&tmp, render(index)).is_err() {
        return;
    }
    if std::fs::rename(&tmp, path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Index {
        let mut index = Index::new();
        index.insert(
            PathBuf::from("/fonts/a.ttc"),
            IndexFile {
                size: 10,
                mtime_ns: 99,
                faces: vec![
                    FaceMeta {
                        index: 0,
                        family: "Alpha Sans".to_owned(),
                        weight: 400,
                        style: FontStyle::Normal,
                    },
                    FaceMeta {
                        index: 1,
                        family: "Alpha Sans".to_owned(),
                        weight: 700,
                        style: FontStyle::Italic,
                    },
                ],
            },
        );
        index.insert(
            PathBuf::from("/fonts/b.ttf"),
            IndexFile {
                size: 5,
                mtime_ns: 1,
                faces: Vec::new(),
            },
        );
        index
    }

    #[test]
    fn round_trips() {
        let index = sample();
        assert_eq!(parse(&render(&index)), Some(index));
    }

    #[test]
    fn render_is_deterministic() {
        assert_eq!(render(&sample()), render(&sample()));
    }

    #[test]
    fn rejects_bad_header_and_bad_lines() {
        assert_eq!(parse("nope\n"), None);
        assert_eq!(parse(&format!("{HEADER}\nX\tfoo\n")), None);
        assert_eq!(parse(&format!("{HEADER}\nF\tabc\t1\t/p\n")), None);
        // An `S` line before any `F` line is malformed.
        assert_eq!(parse(&format!("{HEADER}\nS\t0\t400\tn\tFam\n")), None);
    }

    #[test]
    fn uncacheable_records_are_left_out() {
        let mut index = sample();
        index.insert(
            PathBuf::from("/fonts/bad\tname.ttf"),
            IndexFile {
                size: 1,
                mtime_ns: 1,
                faces: Vec::new(),
            },
        );
        let parsed = parse(&render(&index));
        assert_eq!(parsed.map(|i| i.len()), Some(2));
    }
}
