//! The embedded editor page: files under `assets/editor/`, compiled into
//! the binary by `build.rs`. Lookups match a path exactly, so no request
//! reaches the disk and `..` cannot escape.

include!(concat!(env!("OUT_DIR"), "/editor_assets.rs"));

/// One embedded page file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Asset {
    pub(crate) bytes: &'static [u8],
    pub(crate) content_type: &'static str,
    /// The quoted entity tag of the bytes, for `ETag` and `If-None-Match`.
    pub(crate) etag: &'static str,
}

/// The asset at request path `path`. `/` names `index.html`.
pub(crate) fn lookup(path: &str) -> Option<Asset> {
    let rel = match path.strip_prefix('/')? {
        "" => "index.html",
        rel => rel,
    };
    EDITOR_ASSETS
        .iter()
        .find(|(name, _, _)| *name == rel)
        .map(|(name, bytes, etag)| Asset {
            bytes,
            content_type: content_type(name),
            etag,
        })
}

/// `true` when `path` serves the page itself (`/` or `/index.html`).
pub(crate) fn is_page(path: &str) -> bool {
    matches!(path, "/" | "/index.html")
}

/// `true` when an `If-None-Match` value names `etag` (`*` or one entry of
/// a comma list; weak tags compare by their opaque part).
pub(crate) fn matches_etag(if_none_match: &str, etag: &str) -> bool {
    if_none_match.split(',').map(str::trim).any(|tag| {
        let tag = tag.strip_prefix("W/").unwrap_or(tag);
        tag == "*" || tag == etag
    })
}

/// The content type of an asset, by extension.
fn content_type(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map_or("", |(_, e)| e);
    match ext {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "wasm" => "application/wasm",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use super::*;

    fn disk_files(root: &Path, dir: &Path, out: &mut BTreeMap<String, PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                disk_files(root, &path, out);
            } else {
                let rel = path.strip_prefix(root).expect("under root");
                out.insert(rel.to_string_lossy().replace('\\', "/"), path.clone());
            }
        }
    }

    #[test]
    fn index_serves_at_root_and_traversal_finds_nothing() {
        let asset = lookup("/").expect("index");
        assert!(asset.content_type.starts_with("text/html"));
        assert!(!asset.bytes.is_empty());
        for path in [
            "/../Cargo.toml",
            "/%2e%2e/Cargo.toml",
            "/assets",
            "//index.html",
            "/vendor",
            "x",
        ] {
            assert!(lookup(path).is_none(), "{path}");
        }
    }

    #[test]
    fn table_holds_every_file_on_disk_sorted_with_its_bytes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/editor");
        let mut disk = BTreeMap::new();
        disk_files(&root, &root, &mut disk);
        let names: Vec<&str> = EDITOR_ASSETS.iter().map(|(n, _, _)| *n).collect();
        let want: Vec<&str> = disk.keys().map(String::as_str).collect();
        assert_eq!(names, want, "table is complete and sorted by path");
        for (name, bytes, etag) in EDITOR_ASSETS {
            let on_disk = std::fs::read(&disk[*name]).expect("read asset");
            assert_eq!(*bytes, on_disk.as_slice(), "{name}");
            assert!(etag.starts_with(&format!("\"{}-", bytes.len())), "{name}");
        }
        let tags: std::collections::BTreeSet<&str> =
            EDITOR_ASSETS.iter().map(|(_, _, t)| *t).collect();
        assert_eq!(
            tags.len(),
            EDITOR_ASSETS.len(),
            "distinct files, distinct tags"
        );
        assert!(names.iter().any(|n| n.contains('/')), "nested files embed");
    }

    #[test]
    fn page_files_get_their_content_types() {
        for (name, want) in [
            ("a.html", "text/html; charset=utf-8"),
            ("js/main.js", "text/javascript; charset=utf-8"),
            ("x.mjs", "text/javascript; charset=utf-8"),
            ("css/a.css", "text/css; charset=utf-8"),
            ("i.svg", "image/svg+xml"),
            ("d.json", "application/json; charset=utf-8"),
            ("m.wasm", "application/wasm"),
            ("p.png", "image/png"),
            ("f.woff2", "font/woff2"),
            ("vendor/LICENSE-codemirror.txt", "text/plain; charset=utf-8"),
            ("vendor/MANIFEST", "application/octet-stream"),
        ] {
            assert_eq!(content_type(name), want, "{name}");
        }
        for (name, _, _) in EDITOR_ASSETS {
            if name.ends_with(".js") || name.ends_with(".css") || name.ends_with(".html") {
                assert!(lookup(&format!("/{name}")).is_some(), "{name}");
            }
        }
    }

    #[test]
    fn if_none_match_compares_tags() {
        let tag = "\"10-00000000000000ff\"";
        assert!(matches_etag(tag, tag));
        assert!(matches_etag(&format!("W/{tag}"), tag));
        assert!(matches_etag(&format!("\"x\", {tag}"), tag));
        assert!(matches_etag("*", tag));
        assert!(!matches_etag("\"10-0\"", tag));
    }
}
