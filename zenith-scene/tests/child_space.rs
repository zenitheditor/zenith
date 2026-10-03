//! Container child spaces in compile: page- and zone-anchored nodes stay
//! page-absolute inside a translated group, and a group's symmetry center and
//! a mesh's vanishing point read in the same space as their siblings.

mod common;
use common::*;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

fn doc(body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="p" name="P"
  tokens format="zenith-token-v1" {{
    token id="color.r" type="color" value="#ff0000"
    token id="color.grid" type="color" value="#203040"
  }}
  styles {{}}
  document id="d" title="D" {{
    page id="pg" w=(px)400 h=(px)300 {{
      safe-zone id="sz" type="required" x=(px)100 y=(px)50 w=(px)200 h=(px)100
      {body}
    }}
  }}
}}"##
    )
}

fn rects(body: &str) -> Vec<(f64, f64, f64, f64)> {
    let result = compile(&parse(&doc(body)), &default_provider());
    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    fill_rects(&result)
}

fn commands(body: &str) -> Vec<SceneCommand> {
    let result = compile(&parse(&doc(body)), &default_provider());
    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    result.scene.commands
}

#[test]
fn page_anchor_inside_group_stays_page_absolute() {
    // Page 400x300, rect 40x30 bottom-right: (360, 270) wherever it nests.
    let rect = r##"rect id="r" anchor="bottom-right" w=(px)40 h=(px)30 fill=(token)"color.r""##;
    let root = rects(rect);
    let grouped = rects(&format!(r#"group id="g" x=(px)70 y=(px)45 {{ {rect} }}"#));
    let nested = rects(&format!(
        r#"group id="g1" x=(px)10 y=(px)5 {{ group id="g2" x=(px)20 y=(px)15 {{ {rect} }} }}"#
    ));
    assert_eq!(root, vec![(360.0, 270.0, 40.0, 30.0)]);
    assert_eq!(grouped, root);
    assert_eq!(nested, root);
}

#[test]
fn zone_anchor_inside_group_stays_page_absolute() {
    // Zone (100, 50, 200, 100), rect 40x30 centered: (180, 85).
    let rect =
        r##"rect id="r" anchor="center" anchor-zone="sz" w=(px)40 h=(px)30 fill=(token)"color.r""##;
    let root = rects(rect);
    let grouped = rects(&format!(r#"group id="g" x=(px)70 y=(px)45 {{ {rect} }}"#));
    assert_eq!(root, vec![(180.0, 85.0, 40.0, 30.0)]);
    assert_eq!(grouped, root);
}

#[test]
fn zone_anchor_inside_frame_stays_page_absolute() {
    let rect =
        r##"rect id="r" anchor="center" anchor-zone="sz" w=(px)40 h=(px)30 fill=(token)"color.r""##;
    let framed = rects(&format!(
        r#"frame id="f" x=(px)10 y=(px)10 w=(px)380 h=(px)280 {{ {rect} }}"#
    ));
    assert_eq!(framed, vec![(180.0, 85.0, 40.0, 30.0)]);
}

fn transforms(cmds: &[SceneCommand]) -> Vec<(f64, f64, f64)> {
    cmds.iter()
        .filter_map(|cmd| match cmd {
            SceneCommand::PushTransform { angle_deg, cx, cy } => Some((*angle_deg, *cx, *cy)),
            _ => None,
        })
        .collect()
}

#[test]
fn symmetry_center_reads_in_group_child_space() {
    // The center (50, 50) sits in the group's child space, like the seed
    // rect: a group at (30, 20) pivots its copies on page (80, 70).
    let cmds = commands(
        r##"group id="g" x=(px)30 y=(px)20 symmetry-count=4 symmetry-cx=(px)50 symmetry-cy=(px)50 {
        rect id="seed" x=(px)40 y=(px)10 w=(px)20 h=(px)20 fill=(token)"color.r"
      }"##,
    );
    assert_eq!(
        transforms(&cmds),
        vec![(90.0, 80.0, 70.0), (180.0, 80.0, 70.0), (270.0, 80.0, 70.0)]
    );
}

#[test]
fn symmetry_center_at_page_root_is_unchanged() {
    let cmds = commands(
        r##"group id="g" symmetry-count=4 symmetry-cx=(px)50 symmetry-cy=(px)50 {
        rect id="seed" x=(px)40 y=(px)10 w=(px)20 h=(px)20 fill=(token)"color.r"
      }"##,
    );
    assert_eq!(
        transforms(&cmds),
        vec![(90.0, 50.0, 50.0), (180.0, 50.0, 50.0), (270.0, 50.0, 50.0)]
    );
}

fn lines(cmds: &[SceneCommand]) -> Vec<(f64, f64, f64, f64)> {
    cmds.iter()
        .filter_map(|cmd| match cmd {
            SceneCommand::StrokeLine { x1, y1, x2, y2, .. } => Some((*x1, *y1, *x2, *y2)),
            _ => None,
        })
        .collect()
}

#[test]
fn mesh_vanishing_point_reads_in_parent_space() {
    // A mesh in a group at (100, 50) draws the same lines as the mesh moved
    // by (100, 50) at the page root, vanishing point included.
    let mesh = |x: f64, y: f64, vx: f64, vy: f64| {
        format!(
            r#"mesh id="m" kind="perspective" x=(px){x} y=(px){y} w=(px)100 h=(px)80 rows=2 columns=2 vanishing-x=(px){vx} vanishing-y=(px){vy} stroke=(token)"color.grid" stroke-width=(px)1"#
        )
    };
    let grouped = commands(&format!(
        r#"group id="g" x=(px)100 y=(px)50 {{ {} }}"#,
        mesh(10.0, 120.0, 60.0, 0.0)
    ));
    let root = commands(&mesh(110.0, 170.0, 160.0, 50.0));
    let grouped_lines = lines(&grouped);
    assert!(!grouped_lines.is_empty());
    assert_eq!(grouped_lines, lines(&root));
}

#[test]
fn mesh_default_vanishing_point_follows_the_box() {
    // Without an authored vanishing point the default is the box origin, in
    // device space, inside a group or at the root.
    let mesh = |x: f64, y: f64| {
        format!(
            r#"mesh id="m" kind="perspective" x=(px){x} y=(px){y} w=(px)100 h=(px)80 rows=2 columns=2 stroke=(token)"color.grid" stroke-width=(px)1"#
        )
    };
    let grouped = commands(&format!(
        r#"group id="g" x=(px)100 y=(px)50 {{ {} }}"#,
        mesh(10.0, 120.0)
    ));
    let root = commands(&mesh(110.0, 170.0));
    assert_eq!(lines(&grouped), lines(&root));
}
