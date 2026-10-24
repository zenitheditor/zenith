//! `light` and `mesh` geometry with a unit that has no px value.
//!
//! The compiler skips the node and pushes `scene.unsupported_unit`, as it
//! does for `rect`. It never draws the node at x or y of 0.

mod common;
use common::parse;
use zenith_core::default_provider;
use zenith_scene::compile;

fn compile_body(body: &str) -> (usize, Vec<String>) {
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.eu" name="Eu"
  tokens format="zenith-token-v1" {{
    token id="c.l" type="color" value="#ffcc00"
  }}
  styles {{}}
  document id="doc.eu" title="Eu" {{
    page id="page.eu" w=(px)400 h=(px)200 {{
      {body}
    }}
  }}
}}
"##
    );
    let result = compile(&parse(&src), &default_provider());
    let advisories = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "scene.unsupported_unit")
        .map(|d| d.message.clone())
        .collect();
    (result.scene.commands.len(), advisories)
}

#[test]
fn light_with_pct_x_is_skipped_with_advisory() {
    let (cmds, advisories) =
        compile_body(r#"light id="l.a" x=(pct)85 y=(px)50 radius=(px)60 color=(token)"c.l""#);
    let (px_cmds, px_advisories) =
        compile_body(r#"light id="l.a" x=(px)85 y=(px)50 radius=(px)60 color=(token)"c.l""#);
    assert!(px_advisories.is_empty());
    assert!(
        cmds < px_cmds,
        "pct light must not draw: {cmds} vs {px_cmds}"
    );
    assert_eq!(advisories.len(), 1, "{advisories:?}");
    assert!(
        advisories[0].contains("light 'l.a' field 'x'"),
        "{advisories:?}"
    );
}

#[test]
fn light_with_pct_y_names_the_y_field() {
    let (_, advisories) =
        compile_body(r#"light id="l.b" x=(px)10 y=(pct)50 radius=(px)60 color=(token)"c.l""#);
    assert_eq!(advisories.len(), 1, "{advisories:?}");
    assert!(advisories[0].contains("field 'y'"), "{advisories:?}");
}

#[test]
fn mesh_with_pct_geometry_is_skipped_with_advisory() {
    let mesh = |x: &str| {
        format!(
            r#"mesh id="m.a" x={x} y=(px)10 w=(px)100 h=(px)80 rows=2 columns=2 stroke=(token)"c.l""#
        )
    };
    let (cmds, advisories) = compile_body(&mesh("(pct)20"));
    let (px_cmds, px_advisories) = compile_body(&mesh("(px)20"));
    assert!(px_advisories.is_empty());
    assert!(px_cmds > 0);
    assert!(
        cmds < px_cmds,
        "pct mesh must not draw: {cmds} vs {px_cmds}"
    );
    assert_eq!(advisories.len(), 1, "{advisories:?}");
    assert!(
        advisories[0].contains("mesh 'm.a' field 'x'"),
        "{advisories:?}"
    );
}

#[test]
fn light_and_mesh_without_x_y_are_unchanged() {
    let (cmds, advisories) = compile_body(r#"light id="l.c" radius=(px)60 color=(token)"c.l""#);
    assert!(advisories.is_empty());
    assert!(cmds > 0);
}
