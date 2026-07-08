use crate::commands::serialize_pretty;
use crate::library::{ItemKind, LibraryPack};

/// JSON shape for `library search --json`.
#[derive(Debug, serde::Serialize)]
struct LibrarySearchOutput<'a> {
    schema: &'static str,
    query: &'a str,
    results: Vec<SearchResultJson<'a>>,
}

/// A single search result in `library search --json`.
#[derive(Debug, serde::Serialize)]
struct SearchResultJson<'a> {
    package: &'a str,
    item: &'a str,
    kind: &'static str,
    source: &'static str,
    license: Option<&'static str>,
    tags: &'static [&'static str],
    to_use: String,
}

#[derive(Debug)]
struct SearchResult<'a> {
    pack: &'a LibraryPack,
    item: &'a crate::library::PackItem,
    license: Option<&'static str>,
    tags: &'static [&'static str],
}

/// Search resolved `packs` for library items matching `query`.
///
/// Matching is deterministic and intentionally simple: package id, item id,
/// item kind, license, and known tag/alias text are lowercased and matched by
/// substring. Curated aliases are currently embedded only for the bundled
/// Lucide icon pack; project packs remain discoverable by package/item/kind.
pub fn search(packs: &[LibraryPack], query: &str, json: bool) -> String {
    let normalized_query = normalize_query(query);
    let results = search_results(packs, &normalized_query);

    if json {
        let out = LibrarySearchOutput {
            schema: "zenith-library-search-v1",
            query,
            results: results
                .iter()
                .map(|result| SearchResultJson {
                    package: result.pack.id.as_str(),
                    item: result.item.id.as_str(),
                    kind: result.item.kind.label(),
                    source: result.pack.source.label(),
                    license: result.license,
                    tags: result.tags,
                    to_use: add_command(&result.pack.id, &result.item.id, result.item.kind),
                })
                .collect(),
        };
        serialize_pretty(&out)
    } else {
        format_human(query, &results)
    }
}

fn search_results<'a>(packs: &'a [LibraryPack], normalized_query: &str) -> Vec<SearchResult<'a>> {
    if normalized_query.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();
    for pack in packs {
        for item in &pack.items {
            let license = item_license(&pack.id);
            let tags = item_tags(&pack.id, &item.id);
            if matches_item(pack, item, license, tags, normalized_query) {
                results.push(SearchResult {
                    pack,
                    item,
                    license,
                    tags,
                });
            }
        }
    }
    results
}

fn matches_item(
    pack: &LibraryPack,
    item: &crate::library::PackItem,
    license: Option<&str>,
    tags: &[&str],
    normalized_query: &str,
) -> bool {
    let kind = normalized_text(item.kind.label());
    normalized_text(&pack.id).contains(normalized_query)
        || normalized_text(&item.id).contains(normalized_query)
        || kind.contains(normalized_query)
        || license
            .map(normalized_text)
            .is_some_and(|text| text.contains(normalized_query))
        || tags
            .iter()
            .any(|tag| normalized_text(tag).contains(normalized_query))
}

fn format_human(query: &str, results: &[SearchResult<'_>]) -> String {
    if results.is_empty() {
        return format!("no library items matched \"{query}\"");
    }

    let mut lines = vec![format!("library search \"{query}\"")];
    for result in results {
        let license = result
            .license
            .map(|value| format!(" license={value}"))
            .unwrap_or_default();
        let tags = if result.tags.is_empty() {
            String::new()
        } else {
            format!(" tags={}", result.tags.join(","))
        };
        lines.push(format!(
            "{}#{} ({}) [{}]{}{}",
            result.pack.id,
            result.item.id,
            result.item.kind.label(),
            result.pack.source.label(),
            license,
            tags
        ));
        lines.push(format!(
            "  add: {}",
            add_command(&result.pack.id, &result.item.id, result.item.kind)
        ));
    }
    lines.join("\n")
}

fn add_command(package: &str, item: &str, kind: ItemKind) -> String {
    let spec = format!("{package}#{item}");
    match kind {
        ItemKind::Component => {
            format!("zenith library add {spec} --into <doc.zen> --page <page-id> --at X,Y")
        }
        ItemKind::Token | ItemKind::Action => {
            format!("zenith library add {spec} --into <doc.zen>")
        }
    }
}

fn normalize_query(query: &str) -> String {
    normalized_text(query).trim().to_owned()
}

fn normalized_text(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn item_license(package: &str) -> Option<&'static str> {
    match package {
        "@zenith/icons-lucide" => Some("ISC"),
        _ => None,
    }
}

fn item_tags(package: &str, item: &str) -> &'static [&'static str] {
    match (package, item) {
        ("@zenith/icons-lucide", "monitor") => &[
            "device", "desktop", "display", "screen", "computer", "client",
        ],
        ("@zenith/icons-lucide", "smartphone") => &["device", "phone", "mobile", "client"],
        ("@zenith/icons-lucide", "tablet") => &["device", "mobile", "screen", "client"],
        ("@zenith/icons-lucide", "server") => &["server", "compute", "backend", "rack"],
        ("@zenith/icons-lucide", "database") => &["database", "data", "storage", "db"],
        ("@zenith/icons-lucide", "cloud") => &["cloud", "internet", "network", "service"],
        ("@zenith/icons-lucide", "hard-drive") => &["disk", "drive", "storage", "server"],
        ("@zenith/icons-lucide", "cpu") => &["chip", "processor", "compute", "hardware"],
        ("@zenith/icons-lucide", "network") => &["network", "nodes", "topology", "graph"],
        ("@zenith/icons-lucide", "wifi") => &["wifi", "wireless", "network", "signal"],
        ("@zenith/icons-lucide", "globe") => &["web", "internet", "world", "global"],
        ("@zenith/icons-lucide", "box") => &["package", "container", "artifact", "module"],
        ("@zenith/icons-lucide", "file") => &["file", "document", "page"],
        ("@zenith/icons-lucide", "folder") => &["folder", "directory", "files"],
        ("@zenith/icons-lucide", "lock") => &["lock", "secure", "security", "private"],
        ("@zenith/icons-lucide", "key") => &["key", "credential", "secret", "auth"],
        ("@zenith/icons-lucide", "search") => &["search", "find", "inspect", "query"],
        ("@zenith/icons-lucide", "settings") => &["settings", "config", "preferences", "gear"],
        ("@zenith/icons-lucide", "arrow-right-left") => {
            &["sync", "transfer", "exchange", "bidirectional", "arrows"]
        }
        ("@zenith/icons-lucide", "sync") => &["sync", "refresh", "reload", "loop"],
        ("@zenith/icons-lucide", "upload-cloud") => &["upload", "cloud", "import", "send"],
        ("@zenith/icons-lucide", "download-cloud") => &["download", "cloud", "export", "receive"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::resolve_packs;

    #[test]
    fn search_device_finds_lucide_device_icons() {
        let packs = resolve_packs(None);
        let out = search(&packs, "device", false);
        assert!(out.contains("@zenith/icons-lucide#monitor"), "got: {out}");
        assert!(
            out.contains("@zenith/icons-lucide#smartphone"),
            "got: {out}"
        );
        assert!(out.contains("license=ISC"), "got: {out}");
        assert!(
            out.contains("--page <page-id> --at X,Y"),
            "component add command: {out}"
        );
    }

    #[test]
    fn search_cloud_json_includes_tags_license_and_to_use() {
        let packs = resolve_packs(None);
        let out = search(&packs, "cloud", true);
        let value: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(value["schema"], "zenith-library-search-v1");
        let results = value["results"].as_array().expect("results array");
        let cloud = results
            .iter()
            .find(|item| item["item"] == "cloud")
            .expect("cloud result");
        assert_eq!(cloud["package"], "@zenith/icons-lucide");
        assert_eq!(cloud["kind"], "component");
        assert_eq!(cloud["license"], "ISC");
        assert!(
            cloud["tags"]
                .as_array()
                .expect("tags")
                .iter()
                .any(|tag| tag == "internet"),
            "tags: {}",
            cloud["tags"]
        );
        assert!(
            cloud["to_use"]
                .as_str()
                .expect("to_use")
                .contains("zenith library add @zenith/icons-lucide#cloud"),
            "to_use: {}",
            cloud["to_use"]
        );
    }

    #[test]
    fn search_still_matches_non_icon_item_ids() {
        let packs = resolve_packs(None);
        let out = search(&packs, "noir", false);
        assert!(out.contains("@zenith/filters#noir (token)"), "got: {out}");
        assert!(
            out.contains("zenith library add @zenith/filters#noir --into <doc.zen>"),
            "got: {out}"
        );
    }

    #[test]
    fn search_empty_or_unmatched_query_reports_no_results() {
        let packs = resolve_packs(None);
        assert_eq!(search(&packs, "", false), "no library items matched \"\"");
        assert_eq!(
            search(&packs, "zzzz-not-an-icon", false),
            "no library items matched \"zzzz-not-an-icon\""
        );
    }
}
