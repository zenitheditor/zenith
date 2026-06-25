mod common;
use common::*;
use zenith_core::default_provider;
use zenith_scene::{compile, compile_page};

#[test]
fn chain_two_box_distributes_overflow_to_second_box() {
    // box1 is short (height 80) so only ~2 lines fit at 24px; the rest of the
    // ~40-word article must flow into box2 (placed far below at y=1000).
    let src = r##"zenith version=1 {
  project id="proj.ch2" name="CH2"
  tokens format="zenith-token-v1" {
token id="color.ink" type="color"      value="#111827"
token id="font.body" type="fontFamily" value="Noto Sans"
token id="size.body" type="dimension"  value=(px)24
  }
  styles {}
  document id="doc.ch2" title="CH2" {
page id="page.ch2" w=(px)600 h=(px)1400 {
  text id="box1" x=(px)10 y=(px)0 w=(px)300 h=(px)80 chain="article" fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
    span "Alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima mike november oscar papa quebec romeo sierra tango uniform victor whiskey xray yankee zulu one two three four five six seven eight nine ten eleven twelve"
  }
  text id="box2" x=(px)10 y=(px)1000 w=(px)300 h=(px)380 chain="article" fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let cmds = &result.scene.commands;

    // box1 baselines live near y in [0, 500); box2 near y in [1000, 1400).
    let box1_runs = glyph_runs_in_y(cmds, 0.0, 500.0);
    let box2_runs = glyph_runs_in_y(cmds, 1000.0, 1400.0);

    assert!(box1_runs > 0, "box1 must draw text; got {box1_runs}");
    assert!(
        box2_runs > 0,
        "box2 must receive the continuation; got {box2_runs}"
    );
    // box1 is height 80 at 24px line height → at most ~3 lines; the article has
    // far more words than fit, so box2 must carry strictly more runs than box1.
    assert!(
        box2_runs > box1_runs,
        "continuation box2 ({box2_runs}) must carry more than box1 ({box1_runs})"
    );

    // Determinism: a second compile yields byte-identical commands.
    let result2 = compile(&doc, &default_provider());
    assert_eq!(
        result.scene.commands, result2.scene.commands,
        "chain compile must be deterministic"
    );
}

/// A chain whose content overflows even the LAST box, when that last member
/// declares `overflow="fit"`, must raise a `text.fit_failed` Error on the last
/// member.
#[test]
fn chain_last_box_overflow_fit_fails() {
    // Both boxes are tiny (height 40 → 1 line each) but the article needs many
    // lines; the last box (overflow="fit") cannot hold its remainder.
    let src = r##"zenith version=1 {
  project id="proj.chf" name="CHF"
  tokens format="zenith-token-v1" {
token id="color.ink" type="color"      value="#111827"
token id="font.body" type="fontFamily" value="Noto Sans"
token id="size.body" type="dimension"  value=(px)24
  }
  styles {}
  document id="doc.chf" title="CHF" {
page id="page.chf" w=(px)600 h=(px)1400 {
  text id="cbox1" x=(px)10 y=(px)0 w=(px)300 h=(px)40 chain="article" fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
    span "Alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima mike november oscar papa quebec romeo sierra tango uniform victor whiskey"
  }
  text id="cbox2" x=(px)10 y=(px)1000 w=(px)300 h=(px)40 overflow="fit" chain="article" fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let fit_failed: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "text.fit_failed")
        .collect();
    assert_eq!(
        fit_failed.len(),
        1,
        "exactly one text.fit_failed expected on the last member; got: {:?}",
        result.diagnostics
    );
    assert_eq!(
        fit_failed[0].subject_id.as_deref(),
        Some("cbox2"),
        "the fit failure must name the last chain member"
    );
}

/// A non-chain text node must compile to a command stream identical to one
/// produced when no chain machinery is involved — the chain pre-pass on a
/// chain-free page is a no-op.
#[test]
fn non_chain_text_unaffected_by_chain_prepass() {
    let src = r##"zenith version=1 {
  project id="proj.nc" name="NC"
  tokens format="zenith-token-v1" {
token id="color.ink" type="color"      value="#111827"
token id="font.body" type="fontFamily" value="Noto Sans"
token id="size.body" type="dimension"  value=(px)24
  }
  styles {}
  document id="doc.nc" title="NC" {
page id="page.nc" w=(px)600 h=(px)200 {
  text id="plain" x=(px)10 y=(px)20 w=(px)580 h=(px)40 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
    span "Hello Zenith"
  }
}
  }
}
"##;
    let doc = parse(src);
    let r1 = compile(&doc, &default_provider());
    let r2 = compile(&doc, &default_provider());
    assert_eq!(
        r1.scene.commands, r2.scene.commands,
        "non-chain compile must be deterministic and unaffected"
    );
    // Expect PushClip, DrawGlyphRun, PopClip (the single-line fast path).
    assert_eq!(
        r1.scene.commands.len(),
        3,
        "non-chain single-line text must emit exactly 3 commands; got {:?}",
        r1.scene.commands
    );
}

/// A chain that spans THREE pages (one body box per page, all sharing
/// `chain="ch1"`, only page 1's box bears the article) must flow the article
/// across all three: the box on page 2 AND the box on page 3 each emit
/// continuation glyph runs (not empty). Two compiles per page are byte-identical.
#[test]
fn chain_spans_three_pages() {
    // Each page is 400 tall; the body box is short (h=120 → ~5 lines at 24px)
    // so the ~60-word article cannot fit one box and must cascade page→page.
    let src = r##"zenith version=1 {
  project id="proj.cp" name="CP"
  tokens format="zenith-token-v1" {
token id="color.ink" type="color"      value="#111827"
token id="font.body" type="fontFamily" value="Noto Sans"
token id="size.body" type="dimension"  value=(px)24
  }
  styles {}
  document id="doc.cp" title="CP" {
page id="p1" w=(px)600 h=(px)400 {
  text id="b1" x=(px)10 y=(px)10 w=(px)300 h=(px)120 chain="ch1" fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
    span "Alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima mike november oscar papa quebec romeo sierra tango uniform victor whiskey xray yankee zulu one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty"
  }
}
page id="p2" w=(px)600 h=(px)400 {
  text id="b2" x=(px)10 y=(px)10 w=(px)300 h=(px)120 chain="ch1" fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
  }
}
page id="p3" w=(px)600 h=(px)400 {
  text id="b3" x=(px)10 y=(px)10 w=(px)300 h=(px)600 chain="ch1" fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
  }
}
  }
}
"##;
    let doc = parse(src);
    let p1 = compile_page(&doc, &default_provider(), 0, None);
    let p2 = compile_page(&doc, &default_provider(), 1, None);
    let p3 = compile_page(&doc, &default_provider(), 2, None);

    let runs1 = glyph_runs_in_y(&p1.scene.commands, 0.0, 400.0);
    let runs2 = glyph_runs_in_y(&p2.scene.commands, 0.0, 400.0);
    let runs3 = glyph_runs_in_y(&p3.scene.commands, 0.0, 400.0);

    assert!(runs1 > 0, "page 1 box must draw text; got {runs1}");
    assert!(
        runs2 > 0,
        "page 2 box must receive the continuation; got {runs2}"
    );
    assert!(
        runs3 > 0,
        "page 3 box must receive the continuation tail; got {runs3}"
    );

    // Determinism: each page recompiles byte-identical.
    for idx in 0..3 {
        let a = compile_page(&doc, &default_provider(), idx, None);
        let b = compile_page(&doc, &default_provider(), idx, None);
        assert_eq!(
            a.scene.commands, b.scene.commands,
            "cross-page chain page {idx} must compile deterministically"
        );
    }
}
