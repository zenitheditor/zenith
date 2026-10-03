//! Scene effect of `defaults` lowering: label pairing and style radius,
//! imported-document scoping, instance overrides, and byte identity of
//! documents whose defaults change nothing.

mod common;

use common::*;
use zenith_core::{DefaultsEntry, DefaultsKind, Node, Style, defaults, resolve_tokens};
use zenith_scene::{ImportGraph, compile_page_with_imports};

fn glyph_colors(result: &CompileResult) -> Vec<(u8, u8, u8)> {
    result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun { color, .. } => Some((color.r, color.g, color.b)),
            _ => None,
        })
        .collect()
}

fn solid_rects(result: &CompileResult) -> Vec<(u8, u8, u8)> {
    result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect {
                paint: Paint::Solid { color },
                ..
            } => Some((color.r, color.g, color.b)),
            _ => None,
        })
        .collect()
}

/// The repro: a cobalt-like theme with `shape style="control"
/// text-style="label"` and `text style="body"` defaults.
const THEME: &str = r##"zenith version=1 {
  project id="proj.theme" name="Theme"
  tokens format="zenith-token-v1" {
    token id="color.base.100" type="color" value="#ffffff"
    token id="color.base.content" type="color" value="#1f2937"
    token id="color.primary" type="color" value="#605dff"
    token id="color.primary.content" type="color" value="#edf1fe"
    token id="radius.field" type="dimension" value=(px)8
    token id="size.label" type="dimension" value=(px)16
    token id="size.body" type="dimension" value=(px)14
  }
  styles {
    style id="control" { fill (token)"color.primary"; radius (token)"radius.field" }
    style id="label" { font-size (token)"size.label" }
    style id="body" { font-size (token)"size.body" }
  }
  defaults {
    shape style="control" text-style="label"
    text style="body"
  }
  document id="doc.theme" title="Theme" {
    page id="page.theme" w=(px)400 h=(px)200 background=(token)"color.base.100" {
      shape id="btn" x=(px)20 y=(px)20 w=(px)160 h=(px)48 { span "Go" }
      text id="t" x=(px)20 y=(px)100 w=(px)300 h=(px)30 { span "Body" }
    }
  }
}
"##;

#[test]
fn repro_shape_label_pairs_and_radius_comes_from_style() {
    let result = compile(&parse(THEME), &default_provider());
    let rounded: Vec<f64> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRoundedRect { radius, paint, .. } => {
                assert!(matches!(paint, Paint::Solid { color } if (color.r, color.g, color.b) == (0x60, 0x5d, 0xff)));
                Some(*radius)
            }
            _ => None,
        })
        .collect();
    assert_eq!(rounded, vec![8.0], "the control style paints the shape");
    let colors = glyph_colors(&result);
    assert_eq!(
        colors,
        vec![(0xed, 0xf1, 0xfe), (0x1f, 0x29, 0x37)],
        "label pairs with color.primary, body text with color.base.100"
    );
    let sizes: Vec<f32> = result
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun { font_size, .. } => Some(*font_size),
            _ => None,
        })
        .collect();
    assert_eq!(sizes, vec![16.0, 14.0]);
}

#[test]
fn imported_documents_lower_with_their_own_defaults() {
    let imported = parse(
        r##"zenith version=1 {
  project id="proj.lib" name="Lib"
  tokens format="zenith-token-v1" {
    token id="color.lib" type="color" value="#00ff00"
  }
  styles {
    style id="lib.box" { fill (token)"color.lib" }
  }
  defaults { rect style="lib.box" }
  components {
    component id="card" {
      rect id="bg" x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }
  }
  document id="doc.lib" title="Lib" {
    page id="page.lib" w=(px)10 h=(px)10 {}
  }
}
"##,
    );
    let host = parse(
        r##"zenith version=1 {
  project id="proj.host" name="Host"
  tokens format="zenith-token-v1" {
    token id="color.host" type="color" value="#ff0000"
  }
  styles {
    style id="host.box" { fill (token)"color.host" }
  }
  defaults { rect style="host.box" }
  document id="doc.host" title="Host" {
    page id="page.host" w=(px)100 h=(px)80 {
      rect id="own" x=(px)50 y=(px)0 w=(px)10 h=(px)10
      instance id="inst" source="library#component.card" x=(px)5 y=(px)7
    }
  }
}
"##,
    );
    let imports = ImportGraph::new().with_document("library", &imported);
    let result = compile_page_with_imports(&host, &default_provider(), 0, None, &imports);
    assert_eq!(
        solid_rects(&result),
        vec![(0xff, 0, 0), (0, 0xff, 0)],
        "host rect takes host defaults, imported rect its own"
    );
}

#[test]
fn instance_override_beats_lowered_defaults() {
    let src = r##"zenith version=1 {
  project id="proj.ov" name="Ov"
  tokens format="zenith-token-v1" {
    token id="color.default" type="color" value="#123456"
    token id="color.override" type="color" value="#abcdef"
  }
  styles {
    style id="box" { fill (token)"color.default" }
  }
  defaults { rect style="box" }
  components {
    component id="card" {
      rect id="bg" x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }
  }
  document id="doc.ov" title="Ov" {
    page id="page.ov" w=(px)100 h=(px)80 {
      instance id="a" component="card" x=(px)0 y=(px)0
      instance id="b" component="card" x=(px)20 y=(px)0 {
        override ref="bg" fill=(token)"color.override"
      }
    }
  }
}
"##;
    let result = compile(&parse(src), &default_provider());
    assert_eq!(
        solid_rects(&result),
        vec![(0x12, 0x34, 0x56), (0xab, 0xcd, 0xef)]
    );
}

#[test]
fn examples_without_defaults_skip_lowering() {
    for (name, doc) in examples() {
        let resolved = resolve_tokens(&doc.tokens).resolved;
        assert!(
            defaults::lower(&doc, &resolved).is_none(),
            "{name}: no defaults block, so no lowered clone"
        );
    }
}

/// A `defaults` row naming a style with no properties lowers to the same
/// scene as no block, for every example without content tokens (which would
/// pair text).
#[test]
fn inert_defaults_keep_every_example_byte_identical() {
    let mut checked = 0;
    for (name, doc) in examples() {
        if doc.tokens.tokens.iter().any(|t| t.id.ends_with(".content")) {
            continue;
        }
        let mut with = doc.clone();
        with.styles.styles.push(Style {
            id: "defaults.inert".to_owned(),
            properties: Default::default(),
            unknown_props: Default::default(),
            source_span: None,
        });
        for kind in DefaultsKind::ALL {
            with.defaults.entries.insert(
                *kind,
                DefaultsEntry {
                    style: "defaults.inert".to_owned(),
                    text_style: kind
                        .accepts_text_style()
                        .then(|| "defaults.inert".to_owned()),
                    unknown_props: Default::default(),
                    source_span: None,
                },
            );
        }
        let fonts = default_provider();
        for page in 0..doc.body.pages.len() {
            let plain = compile_page(&doc, &fonts, page, None);
            let lowered = compile_page(&with, &fonts, page, None);
            assert_eq!(
                serde_json::to_string(&plain.scene).expect("scene json"),
                serde_json::to_string(&lowered.scene).expect("scene json"),
                "{name}: page {page} scene changed"
            );
        }
        checked += 1;
    }
    assert!(checked > 0, "at least one example is checked");
}

fn examples() -> Vec<(String, Document)> {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .expect("examples directory reads")
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "zen"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let src = std::fs::read_to_string(&path).expect("example reads");
            (path.display().to_string(), parse(&src))
        })
        .collect()
}

/// A page with `body` on a white background and labelled-shape tokens.
fn label_doc(tokens: &str, styles: &str, top: &str, body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.lbl" name="Lbl"
  tokens format="zenith-token-v1" {{
    token id="color.page" type="color" value="#ffffff"
    token id="color.primary" type="color" value="#605dff"
    token id="color.ink" type="color" value="#111111"
    token id="size.bw" type="dimension" value=(px)2
{tokens}
  }}
  styles {{
{styles}
  }}
  {top}
  document id="doc.lbl" title="Lbl" {{
    page id="page.lbl" w=(px)400 h=(px)200 background=(token)"color.page" {{
      {body}
    }}
  }}
}}
"##
    )
}

fn contrast_for(result: &CompileResult, subject: &str) -> Vec<String> {
    result
        .diagnostics
        .iter()
        .filter(|d| d.code.starts_with("contrast.") && d.subject_id.as_deref() == Some(subject))
        .map(|d| format!("{} {}", d.code, d.message))
        .collect()
}

const BUTTON: &str = r#"shape id="btn" x=(px)20 y=(px)20 w=(px)160 h=(px)48 fill=(token)"color.primary" { span "Go" }"#;

#[test]
fn black_label_on_primary_shape_is_reported_at_compile() {
    let result = compile(&parse(&label_doc("", "", "", BUTTON)), &default_provider());
    let found = contrast_for(&result, "btn");
    assert_eq!(found.len(), 1, "{:?}", result.diagnostics);
    assert!(
        found[0].starts_with("contrast.low shape 'btn' label"),
        "{found:?}"
    );
    assert!(found[0].contains("text-style"), "names the fix");
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.subject_id.as_deref() == Some("btn"))
        .expect("diagnostic");
    assert!(diag.span.is_some(), "points at the authored shape");
}

#[test]
fn label_contrast_adds_diagnostics_only() {
    let doc = parse(&label_doc("", "", "", BUTTON));
    let result = compile(&doc, &default_provider());
    let light = parse(&label_doc(
        r##"token id="color.on" type="color" value="#ffffff""##,
        r#"style id="lbl" { fill (token)"color.on" }"#,
        "",
        &BUTTON.replace("{ span", "text-style=\"lbl\" { span"),
    ));
    let passing = compile(&light, &default_provider());
    assert!(
        contrast_for(&passing, "btn").is_empty(),
        "{:?}",
        passing.diagnostics
    );
    assert!(!result.scene.commands.is_empty());
}

#[test]
fn defaults_pairing_fixes_the_label() {
    let result = compile(
        &parse(&label_doc(
            r##"token id="color.primary.content" type="color" value="#ffffff""##,
            "",
            "defaults {}",
            BUTTON,
        )),
        &default_provider(),
    );
    assert!(
        contrast_for(&result, "btn").is_empty(),
        "{:?}",
        result.diagnostics
    );
}

#[test]
fn label_ink_over_a_partly_covered_area_is_judged_where_it_draws() {
    // The label is top-aligned: it draws over the dark band, not the white
    // lower half that a centre sample would find.
    let body = r#"shape id="card" x=(px)20 y=(px)20 w=(px)200 h=(px)160 v-align="top" { span "Title" }
      rect id="band" x=(px)20 y=(px)20 w=(px)200 h=(px)40 fill=(token)"color.ink""#;
    let result = compile(&parse(&label_doc("", "", "", body)), &default_provider());
    // The band paints after the shape, so it is not under the label.
    assert!(
        contrast_for(&result, "card").is_empty(),
        "{:?}",
        result.diagnostics
    );
    let under = r#"rect id="band" x=(px)20 y=(px)20 w=(px)200 h=(px)40 fill=(token)"color.ink"
      shape id="card" x=(px)20 y=(px)20 w=(px)200 h=(px)160 v-align="top" { span "Title" }"#;
    let result = compile(&parse(&label_doc("", "", "", under)), &default_provider());
    let found = contrast_for(&result, "card");
    assert_eq!(found.len(), 1, "{:?}", result.diagnostics);
    assert!(found[0].starts_with("contrast.invisible"), "{found:?}");
}

#[test]
fn connector_label_is_judged_at_its_routed_midpoint() {
    let body = r#"rect id="dark" x=(px)0 y=(px)0 w=(px)400 h=(px)100 fill=(token)"color.ink"
      shape id="a" x=(px)10 y=(px)120 w=(px)60 h=(px)40
      shape id="b" x=(px)300 y=(px)120 w=(px)60 h=(px)40
      connector id="c" from="a" to="b" stroke=(token)"color.ink" stroke-width=(token)"size.bw" { span "Yes" }"#;
    let result = compile(&parse(&label_doc("", "", "", body)), &default_provider());
    assert!(
        contrast_for(&result, "c").is_empty(),
        "the label sits on the white page below the band: {:?}",
        result.diagnostics
    );
    let raised = body.replace("y=(px)120", "y=(px)30");
    let result = compile(&parse(&label_doc("", "", "", &raised)), &default_provider());
    let found = contrast_for(&result, "c");
    assert_eq!(found.len(), 1, "{:?}", result.diagnostics);
    assert!(found[0].contains("connector 'c' label"));
}

#[test]
fn instances_and_masters_take_the_host_page_defaults() {
    let src = r##"zenith version=1 {
  project id="proj.pg" name="Pg"
  tokens format="zenith-token-v1" {
    token id="color.doc" type="color" value="#111111"
    token id="color.page" type="color" value="#222222"
  }
  styles {
    style id="d" { fill (token)"color.doc" }
    style id="p" { fill (token)"color.page" }
  }
  defaults { rect style="d" }
  components {
    component id="card" {
      rect id="bg" x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }
  }
  masters {
    master id="m" {
      rect id="mbg" x=(px)50 y=(px)50 w=(px)10 h=(px)10
    }
  }
  document id="doc.pg" title="Pg" {
    page id="one" w=(px)100 h=(px)80 master="m" {
      defaults { rect style="p" }
      instance id="i" component="card" x=(px)0 y=(px)0
    }
    page id="two" w=(px)100 h=(px)80 master="m" {
      instance id="i2" component="card" x=(px)0 y=(px)0
    }
  }
}
"##;
    let doc = parse(src);
    let fonts = default_provider();
    let one = compile_page(&doc, &fonts, 0, None);
    assert_eq!(
        solid_rects(&one),
        vec![(0x22, 0x22, 0x22), (0x22, 0x22, 0x22)]
    );
    let two = compile_page(&doc, &fonts, 1, None);
    assert_eq!(
        solid_rects(&two),
        vec![(0x11, 0x11, 0x11), (0x11, 0x11, 0x11)]
    );
}

/// A page-defaulted component copy: its internal id never reaches the
/// diagnostics, scene JSON, or compiled boxes.
#[test]
fn page_component_copy_ids_never_leak() {
    let src = r##"zenith version=1 {
  project id="proj.leak" name="Leak"
  tokens format="zenith-token-v1" {
    token id="color.page" type="color" value="#222222"
  }
  styles {
    style id="p" { fill (token)"color.page" }
  }
  defaults { rect style="p" }
  components {
    component id="card" {
      rect id="bg" x=(px)0 y=(px)0 w=(px)10 h=(px)10
      instance id="inner" component="ghost" x=(px)0 y=(px)0
    }
  }
  masters {
    master id="m" {
      rect id="mbg" x=(px)50 y=(px)50 w=(px)10 h=(px)10
    }
  }
  document id="doc.leak" title="Leak" {
    page id="one" w=(px)100 h=(px)80 master="m" {
      defaults { rect style="p" }
      instance id="i" component="card" x=(px)0 y=(px)0
    }
  }
}
"##;
    let doc = parse(src);
    let fonts = default_provider();
    let prep = zenith_scene::DocumentPrep::new(&doc, None, None);
    let compiler = zenith_scene::PageCompiler::new(&prep, &fonts);
    let result = compiler.compile_page(0);

    let internal = |text: &str| text.contains("@defaults:");
    let ghost = result
        .diagnostics
        .iter()
        .find(|d| d.code == "scene.unknown_component")
        .expect("the copy's nested instance reports its missing component");
    assert!(ghost.message.contains("'ghost'"), "{}", ghost.message);
    for d in result
        .diagnostics
        .iter()
        .chain(compiler.document_diagnostics().iter())
    {
        assert!(!internal(&d.message), "{}", d.message);
        assert!(!d.subject_id.as_deref().is_some_and(internal), "{d:?}");
    }
    let json = serde_json::to_string(&result.scene).expect("scene json");
    assert!(!internal(&json), "scene JSON holds an internal id");
    for key in compiler.compiled_boxes(0).keys() {
        assert!(!internal(key), "box key {key}");
    }

    // The compiled document holds the copy; `authored_id` maps it back.
    let lowered = prep.document();
    let Some(Node::Instance(i)) = lowered.body.pages[0].children.first() else {
        panic!("instance");
    };
    let copy = i.component.as_deref().expect("component");
    assert_ne!(copy, "card");
    assert_eq!(prep.authored_id(copy), "card");
    let master = lowered.body.pages[0].master.as_deref().expect("master");
    assert_eq!(prep.authored_id(master), "m");
}
