//! Shared helpers for zenith-scene integration tests.
//! Every helper that was in `compile/tests.rs` and is not `#[test]` lives here.
//!
//! `tests/common/mod.rs` is compiled into EVERY integration-test binary, but each
//! binary only exercises a subset of these helpers — so the unused ones trip
//! `dead_code`/`unused_imports` in the binaries that don't call them. This is the
//! canonical shared-test-helper situation (see the Rust book, "Submodules in
//! Integration Tests"): the helpers are genuinely used across the suite, so the
//! per-binary false positives are suppressed here rather than fragmenting the
//! helpers across files.
#![allow(dead_code, unused_imports)]

pub use zenith_core::{Document, KdlAdapter, KdlSource, default_provider};
pub use zenith_scene::ir::{Color, FitMode, ImageClip, Paint, SceneCommand};
pub use zenith_scene::{CompileResult, compile, compile_page};

// ── Helper to parse a .zen source string ──────────────────────────────

pub fn parse(src: &str) -> Document {
    KdlAdapter
        .parse(src.as_bytes())
        .expect("test document must parse")
}

// ── Helper: collect every FillRect command's (x, y, w, h) in emission order.

pub fn fill_rects(result: &CompileResult) -> Vec<(f64, f64, f64, f64)> {
    result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect { x, y, w, h, .. } => Some((*x, *y, *w, *h)),
            _ => None,
        })
        .collect()
}

/// The solid-`FillRect` color red-channel values present in a scene.
pub fn fill_reds(result: &CompileResult) -> Vec<u8> {
    result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect {
                paint: Paint::Solid { color },
                ..
            } => Some(color.r),
            _ => None,
        })
        .collect()
}

/// Collect every `DrawGlyphRun` (x, color, font_id) in source order.
pub fn glyph_runs(src: &str) -> Vec<(f64, Color, String)> {
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun {
                x, color, font_id, ..
            } => Some((*x, *color, font_id.clone())),
            _ => None,
        })
        .collect()
}

/// Helper: compile a single-span text node with the given align and w,
/// return the x of the sole DrawGlyphRun.
pub fn text_align_run_x(align: Option<&str>, node_x: f64, node_w: Option<f64>) -> f64 {
    let w_attr = node_w.map_or(String::new(), |w| format!(" w=(px){w}"));
    let align_attr = align.map_or(String::new(), |a| format!(" align=\"{a}\""));
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.al" name="AL"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="doc.al" title="AL" {{
page id="page.al" w=(px)800 h=(px)400 {{
  text id="text.al" x=(px){node_x} y=(px)20{w_attr}{align_attr} {{
    span "Hello"
  }}
}}
  }}
}}
"##
    );
    let doc = parse(&src);
    let result = compile(&doc, &default_provider());
    result
        .scene
        .commands
        .iter()
        .find_map(|c| {
            if let SceneCommand::DrawGlyphRun { x, .. } = c {
                Some(*x)
            } else {
                None
            }
        })
        .expect("a DrawGlyphRun must be emitted")
}

/// Helper: collect (x, y) of every DrawGlyphRun emitted for a single
/// text node with the given box width, align, and span text.
pub fn wrap_runs(node_x: f64, box_w: f64, align: &str, span: &str) -> Vec<(f64, f64)> {
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.wr" name="WR"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="doc.wr" title="WR" {{
page id="page.wr" w=(px)1000 h=(px)600 {{
  text id="text.wr" x=(px){node_x} y=(px)20 w=(px){box_w} align="{align}" {{
    span "{span}"
  }}
}}
  }}
}}
"##
    );
    let doc = parse(&src);
    let result = compile(&doc, &default_provider());
    result
        .scene
        .commands
        .iter()
        .filter_map(|c| {
            if let SceneCommand::DrawGlyphRun { x, y, .. } = c {
                Some((*x, *y))
            } else {
                None
            }
        })
        .collect()
}

/// Count the `DrawGlyphRun` commands whose baseline `y` falls in `[lo, hi)`.
/// Used to attribute glyph runs to a particular chain member's box.
pub fn glyph_runs_in_y(cmds: &[SceneCommand], lo: f64, hi: f64) -> usize {
    cmds.iter()
        .filter(|c| match c {
            SceneCommand::DrawGlyphRun { y, .. } => *y >= lo && *y < hi,
            _ => false,
        })
        .count()
}

/// Source for a page with `bleed` and a token-filled full-bleed background plus
/// a single hero rect at authored origin.
pub fn bleed_doc_src(bleed_attr: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.bleed" name="Bleed"
  tokens format="zenith-token-v1" {{
    token id="color.bg" type="color" value="#102030"
    token id="color.hero" type="color" value="#ff8800"
  }}
  styles {{}}
  document id="doc.bleed" title="Bleed" {{
    page id="page.bleed" w=(px)400 h=(px)600{bleed_attr} background=(token)"color.bg" {{
      rect id="rect.hero" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.hero"
    }}
  }}
}}
"##
    )
}

/// Collect the `(x, y)` origin of every glyph run in a scene, in order.
pub fn glyph_run_origins(result: &CompileResult) -> Vec<(f64, f64)> {
    result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun { x, y, .. } => Some((*x, *y)),
            _ => None,
        })
        .collect()
}

/// Find the running-head run's `(x, glyph_count)` on a compiled page (the run
/// whose baseline sits just below y=80).
pub fn running_head_x_and_glyphs(r: &CompileResult) -> Option<(f64, usize)> {
    r.scene.commands.iter().find_map(|c| match c {
        SceneCommand::DrawGlyphRun { x, y, glyphs, .. } if *y > 80.0 && *y < 130.0 => {
            Some((*x, glyphs.len()))
        }
        _ => None,
    })
}

/// Build a wrapping body-paragraph document, optionally with `drop-cap-lines`.
/// The body text is long enough to overflow the box width (forcing the wrap
/// path). Returns the compiled scene's DrawGlyphRun list as `(x, y, font_size)`.
pub fn dropcap_runs(drop_cap_lines: Option<u32>, body: &str) -> Vec<(f64, f64, f32)> {
    let dc_attr = drop_cap_lines.map_or(String::new(), |n| format!(" drop-cap-lines={n}"));
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.dc" name="DC"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="doc.dc" title="DC" {{
page id="page.dc" w=(px)1800 h=(px)2700 {{
  text id="text.dc" x=(px)180 y=(px)600 w=(px)600 h=(px)1200 align="justify" font-size=(px)32{dc_attr} {{
    span "{body}"
  }}
}}
  }}
}}
"##
    );
    let doc = parse(&src);
    let result = compile(&doc, &default_provider());
    result
        .scene
        .commands
        .iter()
        .filter_map(|c| {
            if let SceneCommand::DrawGlyphRun {
                x, y, font_size, ..
            } = c
            {
                Some((*x, *y, *font_size))
            } else {
                None
            }
        })
        .collect()
}

/// Build a single-box wrapping paragraph with a narrow box, optionally with
/// `hyphenate=#true`. Returns the compiled scene's full command stream.
pub fn hyphenate_commands(hyphenate: bool, body: &str) -> Vec<SceneCommand> {
    let hy = if hyphenate { " hyphenate=#true" } else { "" };
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.hy" name="HY"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="doc.hy" title="HY" {{
page id="page.hy" w=(px)1200 h=(px)2000 {{
  text id="text.hy" x=(px)100 y=(px)100 w=(px)360 h=(px)1600 font-size=(px)40{hy} {{
    span "{body}"
  }}
}}
  }}
}}
"##
    );
    let doc = parse(&src);
    compile(&doc, &default_provider()).scene.commands
}

/// Count distinct glyph-run baseline y values (≈ the number of text lines).
pub fn distinct_line_count(cmds: &[SceneCommand]) -> usize {
    let mut ys: std::collections::BTreeSet<i64> = std::collections::BTreeSet::new();
    for c in cmds {
        if let SceneCommand::DrawGlyphRun { y, .. } = c {
            ys.insert((*y * 100.0) as i64);
        }
    }
    ys.len()
}

/// Count DrawGlyphRun commands.
pub fn glyph_run_count(cmds: &[SceneCommand]) -> usize {
    cmds.iter()
        .filter(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }))
        .count()
}

/// Compile a single text node, optionally in tab-leader mode, and return its
/// command stream. The box is `x=100 y=100 w=600 h=400`, font-size 40.
pub fn tab_leader_commands(tab_leader: bool, body: &str) -> Vec<SceneCommand> {
    let tl = if tab_leader { " tab-leader=\".\"" } else { "" };
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.tl" name="TL"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="doc.tl" title="TL" {{
page id="page.tl" w=(px)1200 h=(px)900 {{
  text id="text.tl" x=(px)100 y=(px)100 w=(px)600 h=(px)400 font-size=(px)40{tl} {{
    span "{body}"
  }}
}}
  }}
}}
"##
    );
    let doc = parse(&src);
    compile(&doc, &default_provider()).scene.commands
}

/// Build a 2-page chain with widow/orphan toggle, returns
/// `(page0_line_count, page1_line_count)`.
pub fn widow_orphan_line_counts(widow_orphan: bool) -> (usize, usize) {
    let wo = if widow_orphan { " widow-orphan=2" } else { "" };
    let p1 = "alpha bravo charlie delta echo foxtrot golf hotel";
    let p2 = "victor whiskey xray yankee zulu aurora borealis cascade delta \
estuary fjord glacier harbor island jungle kelp lagoon marsh nimbus quill \
raven storm thicket umbra";
    let body = format!("{p1}\\n{p2}");
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.wo" name="WO"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="doc.wo" title="WO" {{
page id="page.a" w=(px)1200 h=(px)2000 {{
  text id="body.1" x=(px)100 y=(px)100 w=(px)500 h=(px)180 chain="ch" font-size=(px)40 overflow="visible"{wo} {{
    span "{body}"
  }}
}}
page id="page.b" w=(px)1200 h=(px)2000 {{
  text id="body.2" x=(px)100 y=(px)100 w=(px)500 h=(px)1200 chain="ch" font-size=(px)40 overflow="visible" {{
  }}
}}
  }}
}}
"##
    );
    let doc = parse(&src);
    let p0 = compile_page(&doc, &default_provider(), 0).scene.commands;
    let p1c = compile_page(&doc, &default_provider(), 1).scene.commands;
    (distinct_line_count(&p0), distinct_line_count(&p1c))
}

/// Build a doc with one multi-line wrapping text node for baseline-grid tests.
pub fn baseline_grid_doc(grid_attr: &str) -> Document {
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.bg" name="BG"
  tokens format="zenith-token-v1" {{
token id="font.body" type="fontFamily" value="Noto Sans"
  }}
  styles {{}}
  document id="doc.bg" title="BG" {{
page id="page.bg" w=(px)400 h=(px)600 {grid_attr} {{
  text id="col1" x=(px)10 y=(px)25 w=(px)150 h=(px)500 font-family=(token)"font.body" font-size=(px)18 {{
    span "The quick brown fox jumps over the lazy dog again and again across the line."
  }}
}}
  }}
}}
"##
    );
    parse(&src)
}

/// Baseline y of every emitted glyph run, in command order.
pub fn glyph_run_ys(cmds: &[SceneCommand]) -> Vec<f64> {
    cmds.iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun { y, .. } => Some(*y),
            _ => None,
        })
        .collect()
}

/// DISTINCT baseline y values (one per wrapped line), ascending.
pub fn distinct_line_ys(cmds: &[SceneCommand]) -> Vec<f64> {
    let mut ys = glyph_run_ys(cmds);
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ys.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    ys
}

/// Collect every `DrawGlyphRun` `(x, y)` in command order.
pub fn glyph_run_positions(cmds: &[SceneCommand]) -> Vec<(f64, f64)> {
    cmds.iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun { x, y, .. } => Some((*x, *y)),
            _ => None,
        })
        .collect()
}

/// Build a runaround page doc. `extra` is injected before the text node;
/// `exclusion_attr` is appended to the text line.
pub fn runaround_doc(extra: &str, exclusion_attr: &str) -> Document {
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.ra" name="RA"
  tokens format="zenith-token-v1" {{
  }}
  styles {{}}
  document id="doc.ra" title="RA" {{
    page id="page.ra" w=(px)600 h=(px)600 {{
      {extra}
      text id="body" x=(px)0 y=(px)0 w=(px)400 h=(px)560 font-size=(px)20 {exclusion_attr} {{
        span "The quick brown fox jumps over the lazy dog and then keeps running far beyond the box edge to force wrapping across many lines of body text here"
      }}
    }}
  }}
}}
"##
    );
    parse(&src)
}

/// Build a doc with one overlong unbreakable token in a narrow box.
pub fn break_word_doc(attr: &str) -> Document {
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.bw" name="BW"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="doc.bw" title="BW" {{
page id="page.bw" w=(px)400 h=(px)400 {{
  text id="col.bw" x=(px)10 y=(px)20 w=(px)120 h=(px)300 {attr} {{
    span "https://very-long.example.com/some/very/deep/path/segment"
  }}
}}
  }}
}}
"##
    );
    parse(&src)
}

/// The distinct baseline-y values of the emitted glyph runs (one per line).
pub fn glyph_line_ys(result: &CompileResult) -> Vec<f64> {
    let mut ys: Vec<f64> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun { y, .. } => Some(*y),
            _ => None,
        })
        .collect();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    ys.dedup();
    ys
}

/// Collect the font_size of every DrawGlyphRun; returns the first run's size.
pub fn first_glyph_font_size(result: &CompileResult) -> Option<f32> {
    result.scene.commands.iter().find_map(|c| match c {
        SceneCommand::DrawGlyphRun { font_size, .. } => Some(*font_size),
        _ => None,
    })
}

// ── Shared constants ───────────────────────────────────────────────────

/// A two-page document. Page 1 has a full-bleed rect filled `#252525`
/// (r=0x25); page 2 a full-bleed rect filled `#dcdcdc` (r=0xdc). The page
/// fill color uniquely identifies which page was compiled.
pub const TWO_PAGE_DOC: &str = r##"zenith version=1 {
  project id="proj.mp" name="MP"
  tokens format="zenith-token-v1" {
token id="color.p1" type="color" value="#252525"
token id="color.p2" type="color" value="#dcdcdc"
  }
  styles {}
  document id="doc.mp" title="MP" {
page id="page.p1" w=(px)100 h=(px)100 {
  rect id="rect.p1" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.p1"
}
page id="page.p2" w=(px)200 h=(px)200 {
  rect id="rect.p2" x=(px)0 y=(px)0 w=(px)200 h=(px)200 fill=(token)"color.p2"
}
  }
}
"##;

/// Source with a `panel.master` component (bg rect + text label) instanced
/// three times at three x positions, each overriding the label text.
pub const COMPONENT_SRC: &str = r##"zenith version=1 {
  project id="proj.c" name="C"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#101010"
    token id="color.fg" type="color" value="#fafafa"
    token id="color.alt" type="color" value="#ff0000"
    token id="size.body" type="dimension" value=(pt)18
    token id="font.fam" type="fontFamily" value="Noto Sans"
  }
  styles {}
  components {
    component id="panel.master" {
      rect id="bg" x=(px)0 y=(px)0 w=(px)100 h=(px)60 fill=(token)"color.bg"
      text id="label" x=(px)5 y=(px)5 w=(px)90 h=(px)30 fill=(token)"color.fg" font-family=(token)"font.fam" font-size=(token)"size.body" {
        span "Default"
      }
    }
  }
  document id="doc.c" title="C" {
    page id="page.c" w=(px)640 h=(px)360 {
      instance id="inst.1" component="panel.master" x=(px)0 y=(px)0 {
        override ref="label" { span "Back" }
      }
      instance id="inst.2" component="panel.master" x=(px)200 y=(px)0 {
        override ref="label" fill=(token)"color.alt" { span "Center" }
      }
      instance id="inst.3" component="panel.master" x=(px)400 y=(px)0 {
        override ref="label" { span "Cover" }
      }
    }
  }
}
"##;

/// A 4-page mirror-margin book whose master carries a running-head + a
/// page-number field; each page sets `master="m.body"` and has one body text.
pub const BOOK_SRC: &str = r##"zenith version=1 mirror-margins=#true {
  project id="proj.book" name="Book"
  tokens format="zenith-token-v1" {
token id="color.ink" type="color" value="#111111"
  }
  styles {}
  masters {
master id="m.body" {
  field id="rh" type="running-head" recto="Chapter One: A Long Recto Title" verso="Verso" y=(px)80 h=(px)40 fill=(token)"color.ink"
  field id="folio" type="page-number" y=(px)1820 h=(px)40 fill=(token)"color.ink"
}
  }
  document id="doc.book" title="Book" {
page id="p1" w=(px)1200 h=(px)1900 margin-inner=(px)160 margin-outer=(px)100 margin-top=(px)80 margin-bottom=(px)80 master="m.body" {
  text id="b1" x=(px)160 y=(px)200 w=(px)900 h=(px)40 fill=(token)"color.ink" { span "Body one" }
}
page id="p2" w=(px)1200 h=(px)1900 margin-inner=(px)160 margin-outer=(px)100 margin-top=(px)80 margin-bottom=(px)80 master="m.body" {
  text id="b2" x=(px)160 y=(px)200 w=(px)900 h=(px)40 fill=(token)"color.ink" { span "Body two" }
}
page id="p3" w=(px)1200 h=(px)1900 margin-inner=(px)160 margin-outer=(px)100 margin-top=(px)80 margin-bottom=(px)80 master="m.body" {
  text id="b3" x=(px)160 y=(px)200 w=(px)900 h=(px)40 fill=(token)"color.ink" { span "Body three" }
}
page id="p4" w=(px)1200 h=(px)1900 margin-inner=(px)160 margin-outer=(px)100 margin-top=(px)80 margin-bottom=(px)80 master="m.body" {
  text id="b4" x=(px)160 y=(px)200 w=(px)900 h=(px)40 fill=(token)"color.ink" { span "Body four" }
}
  }
}
"##;

/// The same book, but with `page-parity-start="verso"` so page 1 is a VERSO.
pub const BOOK_SRC_VERSO_START: &str = r##"zenith version=1 mirror-margins=#true page-parity-start="verso" {
  project id="proj.book" name="Book"
  tokens format="zenith-token-v1" {
token id="color.ink" type="color" value="#111111"
  }
  styles {}
  masters {
master id="m.body" {
  field id="rh" type="running-head" recto="Chapter One: A Long Recto Title" verso="Verso" y=(px)80 h=(px)40 fill=(token)"color.ink"
  field id="folio" type="page-number" y=(px)1820 h=(px)40 fill=(token)"color.ink"
}
  }
  document id="doc.book" title="Book" {
page id="p1" w=(px)1200 h=(px)1900 margin-inner=(px)160 margin-outer=(px)100 margin-top=(px)80 margin-bottom=(px)80 master="m.body" {
  text id="b1" x=(px)160 y=(px)200 w=(px)900 h=(px)40 fill=(token)"color.ink" { span "Body one" }
}
page id="p2" w=(px)1200 h=(px)1900 margin-inner=(px)160 margin-outer=(px)100 margin-top=(px)80 margin-bottom=(px)80 master="m.body" {
  text id="b2" x=(px)160 y=(px)200 w=(px)900 h=(px)40 fill=(token)"color.ink" { span "Body two" }
}
  }
}
"##;

/// A margined page carrying a body text node with a `footnote-ref` span
/// plus one page-level footnote.
pub const FOOTNOTE_ONE_SRC: &str = r##"zenith version=1 {
  project id="proj.fn1" name="FN1"
  tokens format="zenith-token-v1" {
  }
  styles {}
  document id="doc.fn1" title="FN1" {
page id="page.fn1" w=(px)600 h=(px)900 margin-inner=(px)60 margin-outer=(px)60 margin-top=(px)80 margin-bottom=(px)80 {
  text id="body" x=(px)60 y=(px)80 w=(px)480 h=(px)200 {
    span "Strong evidence" footnote-ref="fn.1"
    span " supports the claim."
  }
  footnote id="fn.1" {
    span "See also Chapter 4."
  }
}
  }
}
"##;

pub const DROPCAP_BODY: &str = "The quick brown fox jumps over the lazy dog and then \
continues running across the wide green meadow under a bright morning sky while \
birds sing and the river flows gently past the old stone bridge nearby downstream.";

pub const HYPH_BODY: &str = "extraordinarily complicated hyphenation demonstrates \
remarkable typographical sophistication consistently";
