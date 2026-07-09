//! `zenith library search` — ranked lookup across every resolved pack.
//!
//! Matching and ordering live in the `rank` submodule; this module owns the
//! command surface:
//! options, the result cap, and human/JSON formatting.

mod rank;

pub use rank::Filter;

use crate::commands::serialize_pretty;
use crate::library::{ItemKind, LibraryPack};

/// Default number of results shown. A bundled icon library holds ~1745 items, so
/// an uncapped ranked list is as unreadable as an unranked one.
pub const DEFAULT_LIMIT: usize = 25;

/// JSON shape for `library search --json`.
#[derive(Debug, serde::Serialize)]
struct LibrarySearchOutput<'a> {
    schema: &'static str,
    query: &'a str,
    /// Results that matched, before [`SearchOptions::limit`] was applied.
    total_matches: usize,
    results: Vec<SearchResultJson<'a>>,
}

/// A single search result in `library search --json`.
#[derive(Debug, serde::Serialize)]
struct SearchResultJson<'a> {
    package: &'a str,
    item: &'a str,
    kind: &'static str,
    source: &'static str,
    format: &'static str,
    license: Option<&'a str>,
    tags: &'a [String],
    categories: &'a [String],
    to_use: String,
}

/// Options for [`search`].
#[derive(Debug, Clone, Copy)]
pub struct SearchOptions<'a> {
    /// Narrowing filters applied before ranking.
    pub filter: Filter<'a>,
    /// Maximum results to return. `0` means no cap.
    pub limit: usize,
    /// Emit JSON instead of human-readable text.
    pub json: bool,
}

impl Default for SearchOptions<'_> {
    fn default() -> Self {
        Self {
            filter: Filter::default(),
            limit: DEFAULT_LIMIT,
            json: false,
        }
    }
}

/// Search resolved `packs` for library items matching `query`.
///
/// Results are ranked by BM25 over item id, tags, pack id, kind, and license
/// (see the `rank` submodule), then truncated to `options.limit`. Ordering is fully
/// deterministic, so the same query always prints the same bytes.
pub fn search(packs: &[LibraryPack], query: &str, options: SearchOptions<'_>) -> String {
    let ranked = rank::rank(packs, query, &options.filter);
    let total = ranked.len();
    let shown: &[rank::Scored<'_>] = if options.limit == 0 {
        &ranked
    } else {
        &ranked[..ranked.len().min(options.limit)]
    };

    if options.json {
        let out = LibrarySearchOutput {
            schema: "zenith-library-search-v1",
            query,
            total_matches: total,
            results: shown
                .iter()
                .map(|result| SearchResultJson {
                    package: result.pack.id.as_str(),
                    item: result.item.id.as_str(),
                    kind: result.item.kind.label(),
                    source: result.pack.source.label(),
                    format: result.pack.format.label(),
                    license: result.pack.license.as_deref(),
                    tags: &result.item.tags,
                    categories: &result.item.categories,
                    to_use: add_command(&result.pack.id, &result.item.id, result.item.kind),
                })
                .collect(),
        };
        serialize_pretty(&out)
    } else {
        format_human(query, shown, total)
    }
}

fn format_human(query: &str, results: &[rank::Scored<'_>], total: usize) -> String {
    if results.is_empty() {
        return format!("no library items matched \"{query}\"");
    }

    let mut lines = vec![format!("library search \"{query}\"")];
    for result in results {
        let license = result
            .pack
            .license
            .as_deref()
            .map(|value| format!(" license={value}"))
            .unwrap_or_default();
        let tags = if result.item.tags.is_empty() {
            String::new()
        } else {
            format!(" tags={}", result.item.tags.join(","))
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

    let hidden = total.saturating_sub(results.len());
    if hidden > 0 {
        lines.push(format!(
            "… {hidden} more match; narrow the query, or raise --limit"
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::resolve_packs;

    fn human(packs: &[LibraryPack], query: &str) -> String {
        search(packs, query, SearchOptions::default())
    }

    #[test]
    fn search_device_finds_lucide_device_icons() {
        let packs = resolve_packs(None);
        let out = human(&packs, "device");
        assert!(out.contains("@zenith/icons-lucide#monitor"), "got: {out}");
        assert!(
            out.contains("@zenith/icons-lucide#smartphone"),
            "got: {out}"
        );
        assert!(out.contains("license=ISC AND MIT"), "got: {out}");
        assert!(
            out.contains("--page <page-id> --at X,Y"),
            "component add command: {out}"
        );
    }

    #[test]
    fn search_cloud_json_includes_tags_license_and_to_use() {
        let packs = resolve_packs(None);
        let out = search(
            &packs,
            "cloud",
            SearchOptions {
                json: true,
                ..SearchOptions::default()
            },
        );
        let value: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(value["schema"], "zenith-library-search-v1");
        let results = value["results"].as_array().expect("results array");
        let cloud = results
            .iter()
            .find(|item| item["item"] == "cloud")
            .expect("cloud result");
        assert_eq!(cloud["package"], "@zenith/icons-lucide");
        assert_eq!(cloud["kind"], "component");
        assert_eq!(cloud["format"], "svg");
        assert_eq!(cloud["license"], "ISC AND MIT");
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
        let out = human(&packs, "noir");
        assert!(out.contains("@zenith/filters#noir (token)"), "got: {out}");
        assert!(
            out.contains("zenith library add @zenith/filters#noir --into <doc.zen>"),
            "got: {out}"
        );
    }

    #[test]
    fn search_empty_or_unmatched_query_reports_no_results() {
        let packs = resolve_packs(None);
        assert_eq!(human(&packs, ""), "no library items matched \"\"");
        assert_eq!(
            human(&packs, "zzzz-not-an-icon"),
            "no library items matched \"zzzz-not-an-icon\""
        );
    }

    #[test]
    fn results_are_capped_and_the_remainder_is_reported() {
        let packs = resolve_packs(None);
        // `arrow` matches far more than the default cap.
        let out = human(&packs, "arrow");
        let shown = out.matches("  add: ").count();
        assert_eq!(shown, DEFAULT_LIMIT);
        assert!(out.contains("more match; narrow the query"), "got: {out}");
    }

    #[test]
    fn limit_zero_shows_everything_and_reports_no_remainder() {
        let packs = resolve_packs(None);
        let out = search(
            &packs,
            "arrow",
            SearchOptions {
                limit: 0,
                ..SearchOptions::default()
            },
        );
        assert!(
            !out.contains("more match"),
            "got tail: {}",
            &out[out.len() - 80..]
        );
    }

    #[test]
    fn json_reports_total_matches_before_the_cap() {
        let packs = resolve_packs(None);
        let out = search(
            &packs,
            "arrow",
            SearchOptions {
                json: true,
                ..SearchOptions::default()
            },
        );
        let value: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        let total = value["total_matches"].as_u64().expect("total_matches");
        let shown = value["results"].as_array().expect("results").len();
        assert_eq!(shown, DEFAULT_LIMIT);
        assert!(
            total > shown as u64,
            "total {total} should exceed shown {shown}"
        );
    }
}
