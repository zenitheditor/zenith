//! Parallel multi-page render is deterministic and matches page-by-page renders.
//!
//! `--all-pages` and the multi-page PDF compile pages on a thread pool. These
//! tests check that two runs give identical bytes and diagnostics, and that
//! each page equals the single-page render of that page, in page order.

use std::fs;
use std::process::Command;

use zenith_cli::commands::render::{
    RenderEntryOptions, to_pdf_all_pages_with_dir_options, to_png_all_pages_options,
    to_png_with_dir_options,
};
use zenith_cli::config::CliPolicyFlags;

const PAGE_COUNT: usize = 12;

/// A 12-page document. Each page has a distinct fill and a member of one
/// cross-page text chain. Every page carries a `(data)` reference. The shared
/// `data.no_context` advisory reports once per render, not once per page.
fn multi_page_src() -> String {
    let mut pages = String::new();
    let mut tokens = String::new();
    for i in 0..PAGE_COUNT {
        let shade = 20 + i * 15;
        tokens.push_str(&format!(
            "    token id=\"color.p{i}\" type=\"color\" value=\"#{shade:02x}4080\"\n"
        ));
        let source = if i == 0 {
            "span \"Alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima mike november oscar papa quebec romeo sierra tango uniform victor whiskey xray yankee zulu one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen\""
        } else {
            ""
        };
        pages.push_str(&format!(
            r##"    page id="page.{i}" w=(px)240 h=(px)160 {{
      rect id="rect.{i}" x=(px)0 y=(px)0 w=(px)240 h=(px)160 fill=(token)"color.p{i}"
      text id="chain.{i}" x=(px)10 y=(px)10 w=(px)220 h=(px)40 chain="article" fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {{
        {source}
      }}
      text id="label.{i}" x=(px)10 y=(px)120 w=(px)220 h=(px)30 fill=(token)"color.ink" {{
        span "page" data-ref="label"
      }}
    }}
"##
        ));
    }
    format!(
        r##"zenith version=1 {{
  project id="proj.ap" name="AP"
  tokens format="zenith-token-v1" {{
    token id="color.ink" type="color" value="#111827"
    token id="font.body" type="fontFamily" value="Noto Sans"
    token id="size.body" type="dimension" value=(px)14
{tokens}  }}
  styles {{}}
  document id="doc.ap" title="AP" {{
{pages}  }}
}}
"##
    )
}

fn opts(flags: &CliPolicyFlags) -> RenderEntryOptions<'_> {
    RenderEntryOptions {
        locked: false,
        subset: true,
        flags,
        data: None,
        construction_overlay: false,
        scale: 1.0,
    }
}

#[test]
fn all_pages_png_is_stable_and_matches_single_page_renders() {
    let src = multi_page_src();
    let flags = CliPolicyFlags::default();
    let first = to_png_all_pages_options(&src, None, opts(&flags)).expect("first all-pages run");
    let second = to_png_all_pages_options(&src, None, opts(&flags)).expect("second all-pages run");
    assert_eq!(first.pages.len(), PAGE_COUNT);
    assert_eq!(second.pages.len(), PAGE_COUNT);
    assert_eq!(
        first.diagnostics, second.diagnostics,
        "diagnostics differ across runs"
    );
    for (index, (a, b)) in first.pages.iter().zip(&second.pages).enumerate() {
        assert_eq!(a, b, "page {index}: PNG bytes differ across runs");
        let single = to_png_with_dir_options(&src, None, index + 1, opts(&flags))
            .expect("single-page render");
        assert_eq!(*a, single.png, "page {index}: PNG differs from single-page");
        for d in &single.diagnostics {
            assert!(
                first.diagnostics.contains(d),
                "page {index}: single-page diagnostic missing from all-pages: {d:?}"
            );
        }
    }
    let distinct: std::collections::BTreeSet<&Vec<u8>> = first.pages.iter().collect();
    assert_eq!(
        distinct.len(),
        PAGE_COUNT,
        "every page renders distinct bytes"
    );
}

#[test]
fn all_pages_pdf_is_stable_across_runs() {
    let src = multi_page_src();
    let flags = CliPolicyFlags::default();
    let first = to_pdf_all_pages_with_dir_options(&src, None, opts(&flags)).expect("first PDF");
    let second = to_pdf_all_pages_with_dir_options(&src, None, opts(&flags)).expect("second PDF");
    assert_eq!(first.pdf, second.pdf, "PDF bytes differ across runs");
    assert_eq!(first.diagnostics, second.diagnostics);
    let no_context = first
        .diagnostics
        .iter()
        .filter(|d| d.code == "data.no_context")
        .count();
    assert_eq!(no_context, 1, "the shared data.no_context reports once");
}

/// Run `zenith render <doc> --all-pages <dir> --json` and return stdout.
fn run_all_pages(doc: &std::path::Path, out: &std::path::Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .arg("render")
        .arg(doc)
        .arg("--all-pages")
        .arg(out)
        .arg("--json")
        .output()
        .expect("run zenith");
    assert!(
        output.status.success(),
        "render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout utf8")
}

#[test]
fn all_pages_cli_writes_identical_files_across_runs() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let doc = tmp.path().join("doc.zen");
    fs::write(&doc, multi_page_src()).expect("write doc");
    let out_a = tmp.path().join("a");
    let out_b = tmp.path().join("b");
    let json_a: serde_json::Value =
        serde_json::from_str(&run_all_pages(&doc, &out_a)).expect("run a JSON");
    let json_b: serde_json::Value =
        serde_json::from_str(&run_all_pages(&doc, &out_b)).expect("run b JSON");
    assert_eq!(
        json_a["diagnostics"], json_b["diagnostics"],
        "diagnostic JSON differs across runs"
    );

    let flags = CliPolicyFlags::default();
    for page in 1..=PAGE_COUNT {
        let name = format!("page-{page}.png");
        let a = fs::read(out_a.join(&name)).expect("page file in run a");
        let b = fs::read(out_b.join(&name)).expect("page file in run b");
        assert_eq!(a, b, "{name} differs across runs");
        let single = to_png_with_dir_options(
            &fs::read_to_string(&doc).expect("read doc"),
            Some(tmp.path()),
            page,
            opts(&flags),
        )
        .expect("single-page render");
        assert_eq!(a, single.png, "{name} differs from the single-page render");
    }
}
