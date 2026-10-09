//! `transform_path_anchors` mode `scale`: resize a path about an origin.

use super::*;

const PATHS: &str = r#"path id="p" closed=#true fill=(token)"color.k" {
        anchor x=(px)10 y=(px)10 out-x=(px)20 out-y=(px)10
        anchor x=(px)30 y=(px)10 in-x=(px)20 in-y=(px)10
        anchor x=(px)30 y=(px)30
      }"#;

fn scale(node: &str, sx: f64, sy: f64, cx: f64, cy: f64) -> String {
    format!(
        r#"{{"op":"transform_path_anchors","node":"{node}","transform":{{"mode":"scale","sx":{sx},"sy":{sy},"cx":{cx},"cy":{cy}}}}}"#
    )
}

#[test]
fn scales_anchors_and_handles_about_the_origin() {
    let src = doc(PATHS);
    let r = run(&src, &scale("p", 2.0, 0.5, 10.0, 10.0));
    accepted(&r);
    assert_eq!(r.affected_node_ids, vec!["p".to_owned()]);
    let after = &r.source_after;
    assert!(
        after.contains("anchor x=(px)10 y=(px)10 out-x=(px)30 out-y=(px)10"),
        "{after}"
    );
    assert!(
        after.contains("anchor x=(px)50 y=(px)10 in-x=(px)30 in-y=(px)10"),
        "{after}"
    );
    assert!(after.contains("anchor x=(px)50 y=(px)20"), "{after}");
}

#[test]
fn negative_factor_mirrors_and_zero_is_rejected() {
    let src = doc(PATHS);
    let r = run(&src, &scale("p", -1.0, 1.0, 30.0, 0.0));
    accepted(&r);
    assert!(
        r.source_after.contains("anchor x=(px)50 y=(px)10"),
        "{}",
        r.source_after
    );
    let r = run(&src, &scale("p", 0.0, 1.0, 0.0, 0.0));
    let message = rejected(&r, "tx.invalid_geometry");
    assert!(message.contains("scale factor"), "{message}");
}

#[test]
fn locked_and_unknown_paths_are_rejected() {
    let src = doc(&PATHS.replacen(
        r#"path id="p" closed=#true"#,
        r#"path id="p" closed=#true locked=#true"#,
        1,
    ));
    rejected(&run(&src, &scale("p", 2.0, 2.0, 0.0, 0.0)), "node.locked");
    rejected(
        &run(&src, &scale("nope", 2.0, 2.0, 0.0, 0.0)),
        "tx.unknown_node",
    );
}
