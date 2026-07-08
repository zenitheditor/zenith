mod common;
use common::*;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

// ── Text node with token-resolved fill/font/size → DrawGlyphRun ───────

#[test]
fn text_node_token_resolved_compiles_to_draw_glyph_run() {
    // A page with a text node whose fill, font-family, and font-size all
    // reference tokens.  Shaping uses the bundled Noto Sans provider.
    let src = r##"zenith version=1 {
  project id="proj.tx1" name="TX1"
  tokens format="zenith-token-v1" {
token id="color.ink"     type="color"      value="#111827"
token id="font.body"     type="fontFamily" value="Noto Sans"
token id="size.body"     type="dimension"  value=(px)24
  }
  styles {}
  document id="doc.tx1" title="TX1" {
page id="page.tx1" w=(px)400 h=(px)200 {
  text id="label.tx1" x=(px)10 y=(px)20 w=(px)380 h=(px)40 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
    span "Hello Zenith"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    // No shaping errors expected.
    let unshaped: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "scene.text_unshaped")
        .collect();
    assert!(
        unshaped.is_empty(),
        "no text_unshaped diagnostics expected; got: {:?}",
        result.diagnostics
    );

    // Commands: PushClip, DrawGlyphRun, PopClip.
    let cmds = &result.scene.commands;
    assert_eq!(cmds.len(), 3, "expected 3 commands; got: {:?}", cmds);
    assert!(matches!(cmds[0], SceneCommand::PushClip { .. }));
    assert!(matches!(cmds[2], SceneCommand::PopClip));

    match &cmds[1] {
        SceneCommand::DrawGlyphRun {
            x,
            y,
            font_id,
            font_size,
            color,
            source_node_id,
            glyphs,
            ..
        } => {
            // x is the text-box origin x.
            assert_eq!(*x, 10.0, "x must be text-box origin (10px)");
            // y is baseline = text_y + ascent; ascent > 0, so y > 20.0.
            assert!(*y > 20.0, "baseline y must be > text_y (20px); got {}", y);
            // font_id must be the stable Noto Sans id.
            assert_eq!(
                font_id, "noto-sans-400-normal",
                "font_id must be noto-sans-400-normal"
            );
            assert_eq!(*font_size, 24.0, "font_size must be 24px");
            // Fill color: #111827 → r=0x11=17, g=0x18=24, b=0x27=39.
            assert_eq!(color.r, 0x11, "color.r must be 0x11");
            assert_eq!(color.g, 0x18, "color.g must be 0x18");
            assert_eq!(color.b, 0x27, "color.b must be 0x27");
            assert_eq!(color.a, 255, "color.a must be 255 (opaque)");
            assert_eq!(source_node_id.as_deref(), Some("label.tx1"));
            // Glyph run must be non-empty.
            assert!(
                !glyphs.is_empty(),
                "glyphs must be non-empty for 'Hello Zenith'"
            );
        }
        other => panic!("expected DrawGlyphRun, got {other:?}"),
    }
}

// ── Span vertical-align="super" → smaller font + raised baseline ──────

#[test]
fn span_vertical_align_super_renders_smaller_and_raised() {
    // A text node with a baseline span followed by a superscript span. The
    // superscript run must shape at a REDUCED font size (0.65 × 24 = 15.6) and
    // sit ABOVE the baseline span's baseline.
    let src = r##"zenith version=1 {
  project id="proj.va" name="VA"
  tokens format="zenith-token-v1" {
token id="size.body" type="dimension" value=(px)24
  }
  styles {}
  document id="doc.va" title="VA" {
page id="page.va" w=(px)400 h=(px)200 {
  text id="t.va" x=(px)10 y=(px)20 w=(px)380 h=(px)60 font-size=(token)"size.body" {
    span "x"
    span "2" vertical-align="super"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let runs: Vec<(f64, f32)> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun { y, font_size, .. } => Some((*y, *font_size)),
            _ => None,
        })
        .collect();
    assert_eq!(
        runs.len(),
        2,
        "expected two glyph runs (baseline + super); got {:?}",
        runs
    );

    let (base_y, base_fs) = runs[0];
    let (super_y, super_fs) = runs[1];

    // Baseline span uses the full node font size (24).
    assert_eq!(base_fs, 24.0, "baseline span must render at full 24px");
    // Superscript span uses the reduced size (0.65 × 24 = 15.6).
    assert!(
        super_fs < base_fs,
        "superscript font_size ({super_fs}) must be < node font_size ({base_fs})"
    );
    assert!(
        (super_fs - 15.6).abs() < 0.01,
        "superscript font_size must be 0.65 × 24 = 15.6; got {super_fs}"
    );
    // Superscript baseline is raised (smaller y = higher on the page).
    assert!(
        super_y < base_y,
        "superscript baseline y ({super_y}) must be above the baseline span's y ({base_y})"
    );
}

/// A plain text node (no vertical-align anywhere) must compile to a
/// byte-identical command stream relative to a second run — proving the
/// vertical-align machinery does not perturb the no-vertical-align path.
#[test]
fn plain_text_byte_identical_with_vertical_align_feature() {
    let src = r##"zenith version=1 {
  project id="proj.pi" name="PI"
  tokens format="zenith-token-v1" {
token id="size.body" type="dimension" value=(px)24
  }
  styles {}
  document id="doc.pi" title="PI" {
page id="page.pi" w=(px)400 h=(px)200 {
  text id="t.pi" x=(px)10 y=(px)20 w=(px)380 h=(px)60 font-size=(token)"size.body" {
    span "Hello Zenith"
  }
}
  }
}
"##;
    let doc = parse(src);
    let a = compile(&doc, &default_provider());
    let b = compile(&doc, &default_provider());

    let ja = serde_json::to_string(&a.scene).expect("scene a json");
    let jb = serde_json::to_string(&b.scene).expect("scene b json");
    assert_eq!(
        ja, jb,
        "two compiles must be byte-identical (deterministic)"
    );

    // The plain span must shape at the full node size with a normal baseline.
    let run = a
        .scene
        .commands
        .iter()
        .find_map(|c| match c {
            SceneCommand::DrawGlyphRun { y, font_size, .. } => Some((*y, *font_size)),
            _ => None,
        })
        .expect("a plain text node must emit a DrawGlyphRun");
    assert_eq!(run.1, 24.0, "plain span must render at full 24px");
    assert!(run.0 > 20.0, "plain baseline y must be text_y + ascent");
}

// ── All-primary span stays a single DrawGlyphRun (fallback byte-identity) ──

#[test]
fn all_primary_text_emits_single_draw_glyph_run() {
    // With per-glyph font fallback wired into compilation, a span whose every
    // character is covered by the primary face MUST still compile to exactly
    // one DrawGlyphRun — and the command stream must be stable across two
    // compiles (deterministic, byte-identical to the pre-fallback output).
    let src = r##"zenith version=1 {
  project id="proj.bi" name="BI"
  styles {}
  document id="doc.bi" title="BI" {
page id="page.bi" w=(px)400 h=(px)200 {
  text id="label.bi" x=(px)10 y=(px)20 w=(px)380 h=(px)40 {
    span "Hello Zenith 123!"
  }
}
  }
}
"##;
    let doc = parse(src);
    let a = compile(&doc, &default_provider());
    let b = compile(&doc, &default_provider());

    // Deterministic: identical command streams across two compiles.
    assert_eq!(
        a.scene.commands, b.scene.commands,
        "all-primary compilation must be deterministic / byte-identical"
    );

    // Exactly one DrawGlyphRun for the all-primary span (no fragmentation).
    let glyph_runs = a
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }))
        .count();
    assert_eq!(
        glyph_runs, 1,
        "an all-primary span must emit exactly one DrawGlyphRun; got {glyph_runs}"
    );
}

#[test]
fn scene_json_draw_glyph_run_op_and_font_id_no_bytes() {
    let src = r##"zenith version=1 {
  project id="proj.tx3" name="TX3"
  tokens format="zenith-token-v1" {
token id="color.ink" type="color"      value="#333333"
token id="font.body" type="fontFamily" value="Noto Sans"
token id="size.body" type="dimension"  value=(px)18
  }
  styles {}
  document id="doc.tx3" title="TX3" {
page id="page.tx3" w=(px)300 h=(px)100 {
  text id="label.tx3" x=(px)0 y=(px)0 w=(px)300 h=(px)50 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
    span "Hi"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let j1 = result.scene.to_json().expect("serialize 1");
    let j2 = result.scene.to_json().expect("serialize 2");

    // Must contain the op tag.
    assert!(
        j1.contains(r#""op": "DrawGlyphRun""#),
        "JSON must contain DrawGlyphRun op; snippet: {}",
        &j1[..j1.len().min(500)]
    );
    // Must contain the font_id string.
    assert!(
        j1.contains("noto-sans-400-normal"),
        "JSON must contain font_id; snippet: {}",
        &j1[..j1.len().min(500)]
    );
    // Must NOT contain a large byte array (no font bytes in IR).
    // Large byte arrays appear as `[1, 2, 3, ...]` with > ~50 numbers.
    // A simple heuristic: no run of more than 10 consecutive numbers separated by ", ".
    // We check that the JSON does not contain "bytes" as a key.
    assert!(
        !j1.contains(r#""bytes""#),
        "JSON must not contain a 'bytes' field; font bytes must not appear in the IR"
    );
    // Determinism: two serializations must be identical.
    assert_eq!(j1, j2, "two serializations must be identical (determinism)");
}

// ── Unresolvable font → font.unresolved advisory + fallback render ──────

#[test]
fn unresolvable_font_family_falls_back_and_emits_advisory() {
    let src = r##"zenith version=1 {
  project id="proj.tx4" name="TX4"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.tx4" title="TX4" {
page id="page.tx4" w=(px)200 h=(px)100 {
  text id="label.tx4" x=(px)0 y=(px)0 w=(px)200 h=(px)50 fill="#000000" font-family="Nonexistent" {
    span "test"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    // Exactly one font.unresolved advisory naming the node and the missing family.
    let unresolved: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "font.unresolved")
        .collect();
    assert_eq!(
        unresolved.len(),
        1,
        "expected 1 font.unresolved advisory; got: {:?}",
        result.diagnostics
    );
    assert!(
        unresolved[0].message.contains("label.tx4")
            && unresolved[0].message.contains("Nonexistent"),
        "advisory must name the node and the missing family; got: {:?}",
        unresolved[0]
    );

    // Text must STILL render via the fallback face — DrawGlyphRun present.
    let glyph_cmds: Vec<_> = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }))
        .collect();
    assert!(
        !glyph_cmds.is_empty(),
        "text must render in the fallback face, not be dropped; got: {:?}",
        result.scene.commands
    );
}

/// A text node inheriting font-size and fill from a style → correct DrawGlyphRun.
#[test]
fn text_inherits_font_from_style() {
    let src = r##"zenith version=1 {
  project id="proj.sc3" name="SC3"
  tokens format="zenith-token-v1" {
token id="color.ink" type="color" value="#111827"
token id="size.title" type="dimension" value=(px)32
  }
  styles {
style id="style.title" {
  fill (token)"color.ink"
  font-size (token)"size.title"
}
  }
  document id="doc.sc3" title="SC3" {
page id="page.sc3" w=(px)640 h=(px)360 {
  text id="text.sc3" x=(px)10 y=(px)20 w=(px)400 h=(px)50 style="style.title" {
    span "Hello"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let unshaped: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "scene.text_unshaped")
        .collect();
    assert!(
        unshaped.is_empty(),
        "no text_unshaped diagnostics expected; got: {:?}",
        result.diagnostics
    );

    let cmds = &result.scene.commands;
    match cmds
        .iter()
        .find(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }))
    {
        Some(SceneCommand::DrawGlyphRun {
            font_size, color, ..
        }) => {
            assert_eq!(*font_size, 32.0, "font_size must be 32px from style");
            assert_eq!(
                color.r, 0x11,
                "fill must come from style (color.ink r=0x11)"
            );
        }
        _ => panic!("expected DrawGlyphRun from style cascade"),
    }
}

#[test]
fn text_node_font_weight_selects_bold_face() {
    // Helper: extract the first DrawGlyphRun's font_id from a compiled doc.
    fn first_run_font_id(src: &str) -> String {
        let doc = parse(src);
        let result = compile(&doc, &default_provider());
        result
            .scene
            .commands
            .iter()
            .find_map(|c| match c {
                SceneCommand::DrawGlyphRun { font_id, .. } => Some(font_id.clone()),
                _ => None,
            })
            .expect("a DrawGlyphRun must exist")
    }

    // Bold: font-weight=(token)"weight.bold" → fontWeight 700 → bold face.
    let bold_src = r##"zenith version=1 {
  project id="proj.fw" name="FW"
  tokens format="zenith-token-v1" {
token id="weight.bold" type="fontWeight" value=700
  }
  styles {}
  document id="doc.fw" title="FW" {
page id="page.fw" w=(px)400 h=(px)200 {
  text id="text.bold" x=(px)10 y=(px)20 w=(px)380 h=(px)40 font-weight=(token)"weight.bold" { span "Bold" }
}
  }
}
"##;
    let bold_font_id = first_run_font_id(bold_src);
    assert!(
        bold_font_id.contains("noto-sans-700"),
        "font-weight 700 must select the bold face; got font_id {bold_font_id}"
    );

    // Regular: no font-weight → the default (400) face.
    let regular_src = r##"zenith version=1 {
  project id="proj.fw" name="FW"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.fw" title="FW" {
page id="page.fw" w=(px)400 h=(px)200 {
  text id="text.reg" x=(px)10 y=(px)20 w=(px)380 h=(px)40 { span "Regular" }
}
  }
}
"##;
    let regular_font_id = first_run_font_id(regular_src);
    assert!(
        regular_font_id.contains("noto-sans-400") && !regular_font_id.contains("700"),
        "absent font-weight must select the regular (400) face; got font_id {regular_font_id}"
    );
}

#[test]
fn text_font_features_reach_shaper() {
    fn first_glyph_ids(src: &str) -> Vec<u16> {
        let doc = parse(src);
        let result = compile(&doc, &default_provider());
        assert!(
            result
                .diagnostics
                .iter()
                .all(|d| d.code != "scene.text_unshaped"),
            "text must shape without scene.text_unshaped diagnostics: {:?}",
            result.diagnostics
        );
        result
            .scene
            .commands
            .iter()
            .find_map(|c| match c {
                SceneCommand::DrawGlyphRun { glyphs, .. } => {
                    Some(glyphs.iter().map(|g| g.glyph_id).collect::<Vec<_>>())
                }
                _ => None,
            })
            .expect("a DrawGlyphRun must exist")
    }

    let default_src = r##"zenith version=1 {
  project id="proj.features" name="Features"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.features" title="Features" {
page id="page.features" w=(px)400 h=(px)200 {
  text id="text.default" x=(px)10 y=(px)20 w=(px)380 h=(px)80 { span "fi" }
}
  }
}
"##;
    let disabled_src = r##"zenith version=1 {
  project id="proj.features" name="Features"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.features" title="Features" {
page id="page.features" w=(px)400 h=(px)200 {
  text id="text.disabled" x=(px)10 y=(px)20 w=(px)380 h=(px)80 { span "fi" font-features="liga=0" }
}
  }
}
"##;

    let default_glyphs = first_glyph_ids(default_src);
    let disabled_glyphs = first_glyph_ids(disabled_src);
    assert_ne!(
        default_glyphs, disabled_glyphs,
        "liga=0 should change the shaped glyph sequence for 'fi'"
    );
}

#[test]
fn text_letter_spacing_reaches_scene_glyph_positions() {
    fn glyph_dx(src: &str) -> Vec<f32> {
        let doc = parse(src);
        let result = compile(&doc, &default_provider());
        assert!(
            result
                .diagnostics
                .iter()
                .all(|d| d.code != "scene.text_unshaped"),
            "text must shape without scene.text_unshaped diagnostics: {:?}",
            result.diagnostics
        );
        result
            .scene
            .commands
            .iter()
            .find_map(|c| match c {
                SceneCommand::DrawGlyphRun { glyphs, .. } => {
                    Some(glyphs.iter().map(|g| g.dx).collect::<Vec<_>>())
                }
                _ => None,
            })
            .expect("a DrawGlyphRun must exist")
    }

    let base_src = r##"zenith version=1 {
  project id="proj.spacing" name="Spacing"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.spacing" title="Spacing" {
page id="page.spacing" w=(px)400 h=(px)200 {
  text id="text.base" x=(px)10 y=(px)20 w=(px)380 h=(px)80 { span "ABC" }
}
  }
}
"##;
    let spaced_src = r##"zenith version=1 {
  project id="proj.spacing" name="Spacing"
  tokens format="zenith-token-v1" {
    token id="size.track" type="dimension" value=(px)4
  }
  styles {}
  document id="doc.spacing" title="Spacing" {
page id="page.spacing" w=(px)400 h=(px)200 {
  text id="text.spaced" x=(px)10 y=(px)20 w=(px)380 h=(px)80 letter-spacing=(token)"size.track" { span "ABC" }
}
  }
}
"##;

    let base = glyph_dx(base_src);
    let spaced = glyph_dx(spaced_src);
    assert!(base.len() >= 3 && spaced.len() >= 3);
    assert!((spaced[1] - base[1] - 4.0).abs() < 0.001);
    assert!((spaced[2] - base[2] - 8.0).abs() < 0.001);
}

#[test]
fn code_letter_spacing_reaches_scene_glyph_positions() {
    fn glyph_dx(src: &str) -> Vec<f32> {
        let doc = parse(src);
        let result = compile(&doc, &default_provider());
        result
            .scene
            .commands
            .iter()
            .find_map(|c| match c {
                SceneCommand::DrawGlyphRun { glyphs, .. } => {
                    Some(glyphs.iter().map(|g| g.dx).collect::<Vec<_>>())
                }
                _ => None,
            })
            .expect("a DrawGlyphRun must exist")
    }

    let base_src = r##"zenith version=1 {
  project id="proj.code.spacing" name="Code Spacing"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.code.spacing" title="Code Spacing" {
page id="page.spacing" w=(px)400 h=(px)200 {
  code id="code.base" x=(px)10 y=(px)20 w=(px)380 h=(px)80 { content "ABC" }
}
  }
}
"##;
    let spaced_src = r##"zenith version=1 {
  project id="proj.code.spacing" name="Code Spacing"
  tokens format="zenith-token-v1" {
    token id="size.track" type="dimension" value=(px)3
  }
  styles {}
  document id="doc.code.spacing" title="Code Spacing" {
page id="page.spacing" w=(px)400 h=(px)200 {
  code id="code.spaced" x=(px)10 y=(px)20 w=(px)380 h=(px)80 letter-spacing=(token)"size.track" { content "ABC" }
}
  }
}
"##;

    let base = glyph_dx(base_src);
    let spaced = glyph_dx(spaced_src);
    assert!(base.len() >= 3 && spaced.len() >= 3);
    assert!((spaced[1] - base[1] - 3.0).abs() < 0.001);
    assert!((spaced[2] - base[2] - 6.0).abs() < 0.001);
}

/// Two spans with different fill tokens → two runs, distinct colors, the
/// second positioned to the right of the first.
#[test]
fn text_spans_render_with_per_span_fill_and_order() {
    let src = r##"zenith version=1 {
  project id="proj.ps" name="PS"
  tokens format="zenith-token-v1" {
token id="color.red" type="color" value="#ff0000"
token id="color.blue" type="color" value="#0000ff"
  }
  styles {}
  document id="doc.ps" title="PS" {
page id="page.ps" w=(px)400 h=(px)200 {
  text id="text.ps" x=(px)10 y=(px)20 w=(px)380 h=(px)40 {
    span "Red" fill=(token)"color.red"
    span "Blue" fill=(token)"color.blue"
  }
}
  }
}
"##;
    let runs = glyph_runs(src);
    assert_eq!(
        runs.len(),
        2,
        "expected two DrawGlyphRun; got {}",
        runs.len()
    );

    let (x0, c0, _) = &runs[0];
    let (x1, c1, _) = &runs[1];
    assert_eq!((c0.r, c0.g, c0.b), (0xff, 0x00, 0x00), "first span red");
    assert_eq!((c1.r, c1.g, c1.b), (0x00, 0x00, 0xff), "second span blue");
    assert!(
        x1 > x0,
        "second run x ({x1}) must be greater than first ({x0})"
    );
}

/// A bold second span → its run resolves to the 700 face while the first
/// (regular) span resolves to the 400 face.
#[test]
fn text_spans_render_with_per_span_weight() {
    let src = r##"zenith version=1 {
  project id="proj.pw" name="PW"
  tokens format="zenith-token-v1" {
token id="weight.bold" type="fontWeight" value=700
  }
  styles {}
  document id="doc.pw" title="PW" {
page id="page.pw" w=(px)400 h=(px)200 {
  text id="text.pw" x=(px)10 y=(px)20 w=(px)380 h=(px)40 {
    span "Reg"
    span "Bold" font-weight=(token)"weight.bold"
  }
}
  }
}
"##;
    let runs = glyph_runs(src);
    assert_eq!(
        runs.len(),
        2,
        "expected two DrawGlyphRun; got {}",
        runs.len()
    );
    assert!(
        runs[0].2.contains("noto-sans-400"),
        "first span must use the regular (400) face; got {}",
        runs[0].2
    );
    assert!(
        runs[1].2.contains("noto-sans-700"),
        "second span must use the bold (700) face; got {}",
        runs[1].2
    );
}

/// An italic span selects the italic face; a plain span stays upright.
#[test]
fn text_italic_span_selects_italic_face() {
    let src = r##"zenith version=1 {
  project id="proj.it" name="IT"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.it" title="IT" {
page id="page.it" w=(px)400 h=(px)200 {
  text id="text.it" x=(px)10 y=(px)20 w=(px)380 h=(px)40 {
    span "Up"
    span "Italic" italic=#true
  }
}
  }
}
"##;
    let runs = glyph_runs(src);
    assert_eq!(runs.len(), 2, "expected two runs; got {}", runs.len());
    assert!(
        !runs[0].2.contains("italic"),
        "first span must be upright; got {}",
        runs[0].2
    );
    assert!(
        runs[1].2.contains("italic"),
        "second span must use the italic face; got {}",
        runs[1].2
    );
}

/// Underline/strikethrough spans each emit one decoration `FillRect`; a
/// plain span emits none.
#[test]
fn text_span_decorations_emit_fill_rects() {
    let src = r##"zenith version=1 {
  project id="proj.dec" name="DEC"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.dec" title="DEC" {
page id="page.dec" w=(px)400 h=(px)200 {
  text id="text.dec" x=(px)10 y=(px)20 w=(px)380 h=(px)40 {
    span "plain"
    span "under" underline=#true
    span "strike" strikethrough=#true
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    let fill_rects = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
        .count();
    assert_eq!(
        fill_rects, 2,
        "one underline + one strikethrough → 2 decoration rects; got {fill_rects}"
    );
}

/// A single-span node emits exactly one run (non-breaking regression).
#[test]
fn text_single_span_emits_one_run() {
    let src = r##"zenith version=1 {
  project id="proj.ss" name="SS"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.ss" title="SS" {
page id="page.ss" w=(px)400 h=(px)200 {
  text id="text.ss" x=(px)10 y=(px)20 w=(px)380 h=(px)40 { span "Solo" }
}
  }
}
"##;
    let runs = glyph_runs(src);
    assert_eq!(runs.len(), 1, "single span must emit exactly one run");
}

/// An empty span between two non-empty spans is skipped (no run emitted),
/// yet positioning of the following span still accounts for the previous
/// span's advance — i.e. empty spans don't emit but don't break order.
#[test]
fn text_empty_span_is_skipped_without_breaking_order() {
    let src = r##"zenith version=1 {
  project id="proj.es" name="ES"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.es" title="ES" {
page id="page.es" w=(px)400 h=(px)200 {
  text id="text.es" x=(px)10 y=(px)20 w=(px)380 h=(px)40 {
    span "AAAA"
    span ""
    span "BBBB"
  }
}
  }
}
"##;
    let runs = glyph_runs(src);
    assert_eq!(
        runs.len(),
        2,
        "empty span must be skipped → two runs; got {}",
        runs.len()
    );
    let (x0, _, _) = &runs[0];
    let (x1, _, _) = &runs[1];
    assert!(
        x1 > x0,
        "third span x ({x1}) must follow the first span's advance ({x0})"
    );
}

/// A text node with a LITERAL `font-size=(px)20` must produce a
/// `DrawGlyphRun` whose `font_size` is 20.0.
#[test]
fn text_literal_font_size_resolves() {
    let src = r##"zenith version=1 {
  project id="proj.lfs" name="LFS"
  tokens format="zenith-token-v1" {
token id="color.text" type="color" value="#111827"
  }
  styles {}
  document id="doc.lfs" title="LFS" {
page id="page.lfs" w=(px)320 h=(px)200 {
  text id="text.lfs" x=(px)10 y=(px)10 w=(px)200 h=(px)50 fill=(token)"color.text" font-size=(px)20 {
    span "Hi"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());
    match result
        .scene
        .commands
        .iter()
        .find(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }))
    {
        Some(SceneCommand::DrawGlyphRun { font_size, .. }) => {
            assert_eq!(*font_size, 20.0, "literal font-size must resolve to 20px");
        }
        other => panic!("expected DrawGlyphRun, got {other:?}"),
    }
}
