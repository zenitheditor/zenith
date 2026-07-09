//! Integration tests for `zenith library search` — ranking, filters, and caps.

use std::process::Command;

fn search(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .arg("library")
        .arg("search")
        .args(args)
        .output()
        .expect("run zenith");
    assert!(
        output.status.success(),
        "zenith library search {args:?} failed"
    );
    String::from_utf8(output.stdout).expect("stdout utf8")
}

fn search_json(args: &[&str]) -> serde_json::Value {
    let mut args = args.to_vec();
    args.push("--json");
    serde_json::from_str(&search(&args)).expect("json")
}

/// The first `@pkg#item` line of human output.
fn top_hit(stdout: &str) -> &str {
    stdout
        .lines()
        .find(|l| l.starts_with('@'))
        .and_then(|l| l.split(' ').next())
        .expect("at least one result")
}

#[test]
fn library_search_device_finds_lucide_icon_human() {
    let stdout = search(&["device"]);
    assert!(
        stdout.contains("@zenith/icons-lucide#monitor"),
        "got: {stdout}"
    );
    assert!(stdout.contains("license=ISC AND MIT"), "got: {stdout}");
    assert!(
        stdout.contains("zenith library add @zenith/icons-lucide#monitor"),
        "got: {stdout}"
    );
}

#[test]
fn library_search_cloud_json_reports_license_tags_and_categories() {
    let value = search_json(&["cloud"]);
    assert_eq!(value["schema"], "zenith-library-search-v1");
    let results = value["results"].as_array().expect("results");
    let cloud = results
        .iter()
        .find(|r| r["package"] == "@zenith/icons-lucide" && r["item"] == "cloud")
        .unwrap_or_else(|| panic!("cloud result: {results:?}"));
    assert_eq!(cloud["license"], "ISC AND MIT");
    assert_eq!(cloud["format"], "svg");
    assert!(cloud["tags"].as_array().is_some_and(|t| !t.is_empty()));
    assert!(
        cloud["categories"]
            .as_array()
            .is_some_and(|c| !c.is_empty())
    );
}

/// An icon NAMED for the query outranks one that merely contains it.
#[test]
fn library_search_ranks_the_exact_name_first() {
    assert_eq!(top_hit(&search(&["play"])), "@zenith/icons-lucide#play");
    assert_eq!(top_hit(&search(&["house"])), "@zenith/icons-lucide#house");
}

/// An ALIAS carries near-name authority: `home` is an alias of `house`, but only
/// a tag of `lamp`.
#[test]
fn library_search_ranks_an_alias_above_a_tag() {
    assert_eq!(top_hit(&search(&["home"])), "@zenith/icons-lucide#house");
    // `sync` is a Zenith-local alias of upstream `refresh-cw`.
    assert_eq!(
        top_hit(&search(&["sync"])),
        "@zenith/icons-lucide#refresh-cw"
    );
    // Upstream-recorded renames stay findable under their old names.
    assert_eq!(
        top_hit(&search(&["upload-cloud"])),
        "@zenith/icons-lucide#cloud-upload"
    );
}

/// `play` must not match inside `airplay`, nor inside `monitor`'s `display` tag.
#[test]
fn library_search_does_not_match_inside_words() {
    let stdout = search(&["play", "--limit", "0"]);
    assert!(!stdout.contains("#airplay "), "got: {stdout}");
    assert!(!stdout.contains("#monitor "), "got: {stdout}");
    assert!(stdout.contains("#circle-play "), "got: {stdout}");
}

/// Every query term must match: a nonsense query returns nothing, even though
/// its `not` / `icon` fragments appear in hundreds of tags.
#[test]
fn library_search_requires_every_query_term() {
    let stdout = search(&["zzzz-not-an-icon"]);
    assert_eq!(
        stdout.trim(),
        "no library items matched \"zzzz-not-an-icon\""
    );
}

#[test]
fn library_search_caps_results_and_reports_the_remainder() {
    let stdout = search(&["arrow"]);
    assert!(
        stdout.contains("more match; narrow the query"),
        "got: {stdout}"
    );

    let value = search_json(&["arrow"]);
    let total = value["total_matches"].as_u64().expect("total_matches");
    let shown = value["results"].as_array().expect("results").len();
    assert!(total > shown as u64, "total {total} vs shown {shown}");

    // `--limit 0` lifts the cap entirely.
    let value = search_json(&["arrow", "--limit", "0"]);
    let all = value["results"].as_array().expect("results").len();
    assert_eq!(all as u64, total);
}

#[test]
fn library_search_filters_by_category_kind_and_pack() {
    let value = search_json(&["arrow", "--category", "navigation", "--limit", "0"]);
    for result in value["results"].as_array().expect("results") {
        let cats = result["categories"].as_array().expect("categories");
        assert!(
            cats.iter().any(|c| c == "navigation"),
            "result outside the filtered category: {result:?}"
        );
    }

    let value = search_json(&["noir", "--kind", "token"]);
    let results = value["results"].as_array().expect("results");
    assert!(!results.is_empty());
    for result in results {
        assert_eq!(result["kind"], "token");
    }

    let value = search_json(&["noir", "--pack", "@zenith/icons-lucide"]);
    assert!(value["results"].as_array().expect("results").is_empty());
}

/// `.zen` pack items are searched by id, alongside SVG-library icons.
#[test]
fn library_search_still_matches_zen_pack_items() {
    let stdout = search(&["noir"]);
    assert!(
        stdout.contains("@zenith/filters#noir (token)"),
        "got: {stdout}"
    );
}
