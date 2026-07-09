use crate::commands::serialize_pretty;
use crate::library::LibraryPack;

/// JSON shape for `library list --json`.
#[derive(Debug, serde::Serialize)]
struct LibraryListOutput<'a> {
    schema: &'static str,
    packs: Vec<PackJson<'a>>,
}

/// A single pack entry in the `--json` output.
#[derive(Debug, serde::Serialize)]
struct PackJson<'a> {
    id: &'a str,
    version: Option<&'a str>,
    source: &'static str,
    format: &'static str,
    license: Option<&'a str>,
    token_count: usize,
    /// Every item the pack exports. Unlike the human listing, JSON is never
    /// truncated: machine consumers want the whole set.
    items: Vec<PackItemJson<'a>>,
}

/// A single exported item in the `--json` output: its id and kind.
#[derive(Debug, serde::Serialize)]
struct PackItemJson<'a> {
    id: &'a str,
    kind: &'static str,
}

/// Render the resolved `packs` for `library list`.
///
/// Packs are expected pre-sorted by id (see [`crate::library::resolve_packs`]);
/// item order is preserved from the pack's component order.
///
/// - Human (default): one header line per pack
///   (`<id>  <version>  [preset|project]`, with a trailing `(tokens: N)` when
///   the pack's token block is non-empty) followed by indented `#<item>` lines,
///   truncated at `MAX_LISTED_ITEMS` — an icon library exports ~1745 items,
///   and dumping them all is not a listing.
/// - `--json`: a `{"schema":"zenith-library-v1","packs":[…]}` document; each
///   pack entry carries `token_count` alongside its exported `items`, never
///   truncated.
pub fn list(packs: &[LibraryPack], json: bool) -> String {
    if json {
        let out = LibraryListOutput {
            schema: "zenith-library-v1",
            packs: packs
                .iter()
                .map(|p| PackJson {
                    id: &p.id,
                    version: p.version.as_deref(),
                    source: p.source.label(),
                    format: p.format.label(),
                    license: p.license.as_deref(),
                    token_count: p.token_count,
                    items: p
                        .items
                        .iter()
                        .map(|it| PackItemJson {
                            id: it.id.as_str(),
                            kind: it.kind.label(),
                        })
                        .collect(),
                })
                .collect(),
        };
        serialize_pretty(&out)
    } else {
        format_human(packs)
    }
}

/// How many items a pack may list before the human output truncates them.
///
/// A bundled SVG icon library exports ~1745 items; printing them turns
/// `library list` into 1783 lines of scroll. Discovery at that scale is
/// `library search`'s job, so the listing points there instead.
pub const MAX_LISTED_ITEMS: usize = 10;

/// Human-readable listing.
fn format_human(packs: &[LibraryPack]) -> String {
    if packs.is_empty() {
        return "no libraries found".to_owned();
    }
    let mut lines = Vec::new();
    for pack in packs {
        let version = pack.version.as_deref().unwrap_or("-");
        let tokens_suffix = if pack.token_count > 0 {
            format!("  (tokens: {})", pack.token_count)
        } else {
            String::new()
        };
        lines.push(format!(
            "{}  {}  [{}]{}",
            pack.id,
            version,
            pack.source.label(),
            tokens_suffix
        ));
        for item in pack.items.iter().take(MAX_LISTED_ITEMS) {
            lines.push(format!("  #{} ({})", item.id, item.kind.label()));
        }
        let hidden = pack.items.len().saturating_sub(MAX_LISTED_ITEMS);
        if hidden > 0 {
            lines.push(format!(
                "  … {hidden} more; search them with `zenith library search <query> --pack {}`",
                pack.id
            ));
        }
    }
    lines.push(String::new());
    lines.push(
        "Run `zenith library show <package>#<item>` to inspect any item in detail.".to_owned(),
    );
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{PackFormat, PackSource, resolve_packs};

    // ── `library list` hint ────────────────────────────────────────────────────

    #[test]
    fn list_human_includes_show_hint() {
        let packs = resolve_packs(None);
        let out = list(&packs, false);
        assert!(
            out.contains("zenith library show"),
            "list output must mention show: {}",
            out
        );
    }

    // ── `library list` tests ───────────────────────────────────────────────────

    #[test]
    fn human_lists_flowchart_with_items() {
        let packs = resolve_packs(None);
        let out = list(&packs, false);
        assert!(out.contains("@zenith/flowchart"), "got: {}", out);
        assert!(out.contains("[preset]"), "got: {}", out);
        assert!(out.contains("#process (component)"), "got: {}", out);
        assert!(out.contains("#decision (component)"), "got: {}", out);
        assert!(out.contains("#terminator (component)"), "got: {}", out);
    }

    #[test]
    fn human_lists_filters_with_token_items() {
        let packs = resolve_packs(None);
        let out = list(&packs, false);
        assert!(out.contains("@zenith/filters"), "got: {}", out);
        assert!(out.contains("#noir (token)"), "got: {}", out);
    }

    #[test]
    fn human_and_json_list_lucide_icon_pack() {
        let packs = resolve_packs(None);

        // Human output truncates: the pack exports ~1745 items, and the listing
        // points at `library search` rather than printing them all.
        let human = list(&packs, false);
        assert!(human.contains("@zenith/icons-lucide"), "got: {}", human);
        let listed = human
            .lines()
            .skip_while(|l| !l.starts_with("@zenith/icons-lucide"))
            .skip(1)
            .take_while(|l| l.starts_with("  #"))
            .count();
        assert_eq!(listed, MAX_LISTED_ITEMS);
        assert!(
            human.contains("more; search them with `zenith library search"),
            "got: {}",
            human
        );

        // JSON is never truncated: machine consumers want the whole set.
        let json = list(&packs, true);
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let packs_json = value["packs"].as_array().expect("packs array");
        let lucide = packs_json
            .iter()
            .find(|p| p["id"] == "@zenith/icons-lucide")
            .expect("lucide pack present");
        assert_eq!(lucide["version"], "1.23.0");
        assert_eq!(lucide["format"], "svg");
        assert_eq!(lucide["license"], "ISC AND MIT");
        let item_ids: Vec<&str> = lucide["items"]
            .as_array()
            .expect("items array")
            .iter()
            .filter_map(|item| item["id"].as_str())
            .collect();
        assert!(item_ids.len() > 1700, "got {} items", item_ids.len());
        for id in ["monitor", "database", "house", "cloud-download"] {
            assert!(item_ids.contains(&id), "missing {id}");
        }
        // Renamed away upstream; these are aliases now, not ids.
        for id in ["sync", "download-cloud", "upload-cloud"] {
            assert!(!item_ids.contains(&id), "{id} must not be an id");
        }
    }

    #[test]
    fn json_is_parseable_and_contains_flowchart() {
        let packs = resolve_packs(None);
        let out = list(&packs, true);
        let value: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(value["schema"], "zenith-library-v1");
        let packs_json = value["packs"].as_array().expect("packs array");
        let flow = packs_json
            .iter()
            .find(|p| p["id"] == "@zenith/flowchart")
            .expect("flowchart pack present");
        assert_eq!(flow["version"], "1.0.0");
        assert_eq!(flow["source"], "preset");
        let items = flow["items"].as_array().expect("items array");
        let ids: Vec<&str> = items.iter().filter_map(|v| v["id"].as_str()).collect();
        assert_eq!(ids, vec!["process", "decision", "terminator"]);
        assert!(
            items.iter().all(|v| v["kind"] == "component"),
            "all flowchart items are components"
        );
    }

    #[test]
    fn empty_packs_human_message() {
        let out = list(&[], false);
        assert_eq!(out, "no libraries found");
    }

    #[test]
    fn human_and_json_list_zero_item_theme_pack_header_only() {
        let packs = resolve_packs(None);

        let human = list(&packs, false);
        let header_line = human
            .lines()
            .find(|line| line.contains("@zenith/theme.cobalt"))
            .expect("theme.cobalt header line present");
        assert!(header_line.contains("[preset]"), "got: {}", header_line);
        let next_line = human
            .lines()
            .skip_while(|line| !line.contains("@zenith/theme.cobalt"))
            .nth(1)
            .unwrap_or("");
        assert!(
            !next_line.trim_start().starts_with('#'),
            "zero-item theme pack must have no item lines; next line: {}",
            next_line
        );

        let json = list(&packs, true);
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let packs_json = value["packs"].as_array().expect("packs array");
        let theme = packs_json
            .iter()
            .find(|p| p["id"] == "@zenith/theme.cobalt")
            .expect("theme.cobalt pack present in JSON");
        let items = theme["items"].as_array().expect("items array");
        assert!(
            items.is_empty(),
            "theme.cobalt must export no items; got: {:?}",
            items
        );
    }

    // ── `library list` token-count indicator ───────────────────────────────────

    #[test]
    fn human_and_json_show_token_count_for_pack_with_tokens() {
        // theme.cobalt has zero exportable ITEMS but a full token set — the
        // `(tokens: N)` indicator surfaces that whole set as discoverable.
        let packs = resolve_packs(None);

        let theme_pack = packs
            .iter()
            .find(|p| p.id == "@zenith/theme.cobalt")
            .expect("theme.cobalt pack resolved");
        assert!(
            theme_pack.token_count > 0,
            "theme.cobalt must carry tokens for this test to be meaningful"
        );

        let human = list(&packs, false);
        let header_line = human
            .lines()
            .find(|line| line.contains("@zenith/theme.cobalt"))
            .unwrap_or_else(|| panic!("theme.cobalt header line present; got: {}", human));
        assert!(
            header_line.contains(&format!("(tokens: {})", theme_pack.token_count)),
            "got: {}",
            header_line
        );

        let json = list(&packs, true);
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let packs_json = value["packs"].as_array().expect("packs array");
        let theme = packs_json
            .iter()
            .find(|p| p["id"] == "@zenith/theme.cobalt")
            .expect("theme.cobalt pack present in JSON");
        assert_eq!(theme["token_count"], theme_pack.token_count as u64);
    }

    #[test]
    fn human_and_json_no_token_indicator_for_tokens_free_pack() {
        // brand-kit declares no tokens block at all.
        let packs = resolve_packs(None);

        let brand_kit = packs
            .iter()
            .find(|p| p.id == "@zenith/brand-kit")
            .expect("brand-kit pack resolved");
        assert_eq!(
            brand_kit.token_count, 0,
            "brand-kit must be tokens-free for this test to be meaningful"
        );

        let human = list(&packs, false);
        let header_line = human
            .lines()
            .find(|line| line.contains("@zenith/brand-kit"))
            .expect("brand-kit header line present");
        assert!(
            !header_line.contains("(tokens:"),
            "tokens-free pack must show no token indicator: {}",
            header_line
        );

        let json = list(&packs, true);
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let packs_json = value["packs"].as_array().expect("packs array");
        let brand = packs_json
            .iter()
            .find(|p| p["id"] == "@zenith/brand-kit")
            .expect("brand-kit pack present in JSON");
        assert_eq!(brand["token_count"], 0);
    }

    #[test]
    fn version_falls_back_to_dash() {
        let pack = LibraryPack {
            id: "@x/y".to_owned(),
            version: None,
            source: PackSource::Preset,
            format: PackFormat::Zen,
            license: None,
            items: vec![],
            token_count: 0,
        };
        let out = list(std::slice::from_ref(&pack), false);
        assert!(out.contains("@x/y  -  [preset]"), "got: {}", out);
    }
}
