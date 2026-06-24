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
///   (`<id>  <version>  [preset|project]`) followed by indented `#<item>` lines.
/// - `--json`: a `{"schema":"zenith-library-v1","packs":[…]}` document.
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

/// Human-readable listing.
fn format_human(packs: &[LibraryPack]) -> String {
    if packs.is_empty() {
        return "no libraries found".to_owned();
    }
    let mut lines = Vec::new();
    for pack in packs {
        let version = pack.version.as_deref().unwrap_or("-");
        lines.push(format!(
            "{}  {}  [{}]",
            pack.id,
            version,
            pack.source.label()
        ));
        for item in &pack.items {
            lines.push(format!("  #{} ({})", item.id, item.kind.label()));
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
    use crate::library::{PackSource, resolve_packs};

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
    fn version_falls_back_to_dash() {
        let pack = LibraryPack {
            id: "@x/y".to_owned(),
            version: None,
            source: PackSource::Preset,
            items: vec![],
        };
        let out = list(std::slice::from_ref(&pack), false);
        assert!(out.contains("@x/y  -  [preset]"), "got: {}", out);
    }
}
