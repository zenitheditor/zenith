mod common;
use common::parse;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

/// Collect each `DrawGlyphRun`'s baseline y, de-duplicated into distinct lines
/// (keyed by rounded y to absorb sub-pixel jitter), in ascending order.
fn line_baselines(src: &str) -> Vec<f64> {
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let mut ys: std::collections::BTreeMap<i64, f64> = std::collections::BTreeMap::new();
    for c in &result.scene.commands {
        if let SceneCommand::DrawGlyphRun { y, .. } = c {
            ys.entry((y * 100.0).round() as i64).or_insert(*y);
        }
    }
    ys.into_values().collect()
}

const MULTI: &str = r##"zenith version=1 {
  project id="proj.mb" name="MB"
  tokens format="zenith-token-v1" {
    token id="fs" type="dimension" value=(px)28
  }
  styles {}
  document id="doc.mb" title="MB" {
page id="page.mb" w=(px)400 h=(px)200 {
  text id="text.mb" x=(px)20 y=(px)40 w=(px)360 h=(px)120 font-size=(token)"fs" {
    span "line one\nline two"
  }
}
  }
}
"##;

const SINGLE: &str = r##"zenith version=1 {
  project id="proj.mb" name="MB"
  tokens format="zenith-token-v1" {
    token id="fs" type="dimension" value=(px)28
  }
  styles {}
  document id="doc.mb" title="MB" {
page id="page.mb" w=(px)400 h=(px)200 {
  text id="text.mb" x=(px)20 y=(px)40 w=(px)360 h=(px)120 font-size=(token)"fs" {
    span "line one line two"
  }
}
  }
}
"##;

/// A literal `\n` inside span text is a MANDATORY line break: the two halves
/// land on two distinct baselines, the second below the first. (Before the fix
/// the newline reached the shaper whole and rendered as a single-line .notdef
/// tofu box.)
#[test]
fn newline_in_span_forces_a_hard_line_break() {
    let lines = line_baselines(MULTI);
    assert_eq!(
        lines.len(),
        2,
        "span text with one '\\n' must lay out on exactly two baselines, got {lines:?}"
    );
    assert!(
        lines[1] > lines[0],
        "the second line must sit below the first: {lines:?}"
    );
}

/// The same text without a break still fits one line — the mandatory-break
/// routing does not pull a plain fitting node onto the wrap path.
#[test]
fn no_newline_stays_single_line() {
    let lines = line_baselines(SINGLE);
    assert_eq!(
        lines.len(),
        1,
        "fitting text with no '\\n' must stay on a single baseline, got {lines:?}"
    );
}
