//! Every op that computes geometry rejects a result that is not finite or
//! beyond ±2^53 px with `tx.value_out_of_range`, instead of writing a
//! saturated or non-KDL number.

use super::*;
use zenith_tx::{OpPoint, SizeInput};

const SHAPES: &str = r#"rect id="r" x=(px)10 y=(px)20 w=(px)100 h=(px)50 fill=(token)"color.k"
      rect id="b" anchor-sibling="r" anchor-edge="below" anchor-gap=(px)4 w=(px)10 h=(px)10 fill=(token)"color.k"
      line id="l" x1=(px)0 y1=(px)0 x2=(px)10 y2=(px)10 stroke=(token)"color.k"
      polygon id="poly" fill=(token)"color.k" {
        point x=(px)0 y=(px)0
        point x=(px)10 y=(px)0
        point x=(px)10 y=(px)10
      }
      path id="p" closed=#true fill=(token)"color.k" {
        anchor x=(px)10 y=(px)10
        anchor x=(px)30 y=(px)10
        anchor x=(px)30 y=(px)30
      }"#;

const HUGE: [f64; 2] = [1e19, -1e19];

#[test]
fn nudges_past_the_range_are_rejected() {
    let src = doc(SHAPES);
    for d in HUGE {
        let r = run(
            &src,
            &format!(r#"{{"op":"nudge_geometry","node":"r","dx":{d}}}"#),
        );
        let message = rejected(&r, "tx.value_out_of_range");
        assert!(
            message.contains("nudge_geometry") && message.contains("x of node \"r\""),
            "{message}"
        );
        let r = run(
            &src,
            &format!(r#"{{"op":"nudge_line_points","node":"l","dx1":{d}}}"#),
        );
        rejected(&r, "tx.value_out_of_range");
        let r = run(
            &src,
            &format!(r#"{{"op":"nudge_anchor_gap","node":"b","dy":{d}}}"#),
        );
        rejected(&r, "tx.value_out_of_range");
    }
}

#[test]
fn non_finite_and_huge_set_values_are_rejected() {
    let src = doc(SHAPES);
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e19, -1e19] {
        let r = run_op(
            &src,
            Op::SetGeometry {
                node: "r".to_owned(),
                x: Some(Some(v)),
                y: None,
                w: Some(Some(SizeInput::Px(40.0))),
                h: None,
                rotate: None,
            },
        );
        assert_eq!(r.status, TxStatus::Rejected, "{v}: {:?}", r.diagnostics);
        assert!(!r.source_after.contains("9223372036854775807"));
        let r = run_op(
            &src,
            Op::SetPoints {
                node: "poly".to_owned(),
                points: vec![
                    OpPoint { x: 0.0, y: 0.0 },
                    OpPoint { x: v, y: 0.0 },
                    OpPoint { x: 1.0, y: 1.0 },
                ],
            },
        );
        assert_eq!(r.status, TxStatus::Rejected, "{v}: {:?}", r.diagnostics);
    }
}

#[test]
fn a_path_scale_past_the_range_is_rejected() {
    let src = doc(SHAPES);
    let r = run(
        &src,
        r#"{"op":"transform_path_anchors","node":"p","transform":{"mode":"scale","sx":1e300,"sy":1e300,"cx":0,"cy":0}}"#,
    );
    rejected(&r, "tx.value_out_of_range");
}

#[test]
fn values_inside_the_range_write_exactly() {
    let src = doc(SHAPES);
    let r = run(
        &src,
        r#"{"op":"nudge_geometry","node":"r","dx":4503599627370486}"#,
    );
    accepted(&r);
    assert!(
        line_of(&r.source_after, "r").contains("x=(px)4503599627370496"),
        "{}",
        line_of(&r.source_after, "r")
    );
}

#[test]
fn an_authored_out_of_range_value_does_not_block_other_edits() {
    let src = doc(r#"rect id="r" x=(px)1e300 y=(px)20 w=(px)100 h=(px)50 fill=(token)"color.k""#);
    let r = run(&src, r#"{"op":"nudge_geometry","node":"r","dy":1}"#);
    assert!(
        r.diagnostics
            .iter()
            .all(|d| d.code != "tx.value_out_of_range"),
        "{:?}",
        r.diagnostics
    );
}
