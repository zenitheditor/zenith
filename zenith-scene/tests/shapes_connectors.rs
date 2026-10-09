mod common;
use common::parse;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::{SceneCommand, StrokeAlign};

/// Collect the first `StrokePolyline`'s flat points, or panic.
fn first_stroke_polyline_points(cmds: &[SceneCommand]) -> Vec<f64> {
    cmds.iter()
        .find_map(|c| match c {
            SceneCommand::StrokePolyline { points, .. } => Some(points.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected a StrokePolyline; got: {cmds:?}"))
}

/// Collect the first OPEN (`closed: false`) `StrokePolyline`'s flat points — the
/// connector's stroke, distinct from a target polygon's own closed stroke.
fn open_stroke_polyline_points(cmds: &[SceneCommand]) -> Vec<f64> {
    cmds.iter()
        .find_map(|c| match c {
            SceneCommand::StrokePolyline {
                points,
                closed: false,
                ..
            } => Some(points.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected an open StrokePolyline; got: {cmds:?}"))
}

/// Collect every `FillPolygon`'s flat points, in command order.
fn all_fill_polygon_points(cmds: &[SceneCommand]) -> Vec<Vec<f64>> {
    cmds.iter()
        .filter_map(|c| match c {
            SceneCommand::FillPolygon { points, .. } => Some(points.clone()),
            _ => None,
        })
        .collect()
}

/// Assert no segment of a flat `[x0,y0,x1,y1,…]` polyline passes through the
/// strict interior of the box `(x, y, w, h)`. Mirrors the router's own crossing
/// check but operates on the un-inflated box.
fn assert_polyline_misses_box(pts: &[f64], boxr: (f64, f64, f64, f64)) {
    const E: f64 = 1e-6;
    let (l, t, r, bot) = (boxr.0, boxr.1, boxr.0 + boxr.2, boxr.1 + boxr.3);
    let n = pts.len() / 2;
    for i in 0..n.saturating_sub(1) {
        let x0 = pts[i * 2];
        let y0 = pts[i * 2 + 1];
        let x1 = pts[(i + 1) * 2];
        let y1 = pts[(i + 1) * 2 + 1];
        if (y0 - y1).abs() <= E {
            if y0 > t + E && y0 < bot - E {
                let (xa, xb) = if x0 < x1 { (x0, x1) } else { (x1, x0) };
                let ov_lo = xa.max(l);
                let ov_hi = xb.min(r);
                assert!(
                    ov_lo + E >= ov_hi - E,
                    "horizontal segment {:?}->{:?} crosses interior of {boxr:?}; pts {pts:?}",
                    (x0, y0),
                    (x1, y1)
                );
            }
        } else if (x0 - x1).abs() <= E && x0 > l + E && x0 < r - E {
            let (ya, yb) = if y0 < y1 { (y0, y1) } else { (y1, y0) };
            let ov_lo = ya.max(t);
            let ov_hi = yb.min(bot);
            assert!(
                ov_lo + E >= ov_hi - E,
                "vertical segment {:?}->{:?} crosses interior of {boxr:?}; pts {pts:?}",
                (x0, y0),
                (x1, y1)
            );
        }
    }
}

/// Two crossing connectors: a HORIZONTAL one (a→b) and a VERTICAL one (c→d),
/// meeting at (320, 160). All auto anchors.
fn crossing_connectors_src(page_props: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.lj" name="LJ"
  tokens format="zenith-token-v1" {{
token id="color.line" type="color" value="#1e3a8a"
  }}
  styles {{}}
  document id="doc.lj" title="LJ" {{
page id="page.lj" w=(px)640 h=(px)360 {page_props} {{
  rect id="a" x=(px)40 y=(px)140 w=(px)80 h=(px)40 stroke=(token)"color.line"
  rect id="b" x=(px)520 y=(px)140 w=(px)80 h=(px)40 stroke=(token)"color.line"
  rect id="c" x=(px)300 y=(px)20 w=(px)40 h=(px)40 stroke=(token)"color.line"
  rect id="d" x=(px)300 y=(px)300 w=(px)40 h=(px)40 stroke=(token)"color.line"
  connector id="ch" from="a" to="b" stroke=(token)"color.line"
  connector id="cv" from="c" to="d" stroke=(token)"color.line"
}}
  }}
}}
"##
    )
}

/// Collect every connector `StrokePolyline`'s points (those with an even number
/// of coords, in emission order). All four rect strokes here are closed
/// outlines; connectors are open-center polylines, so filter on `closed:false`
/// AND a 2-point-or-more open path that is NOT a rect outline. Simplest robust
/// filter: open, center-aligned strokes whose first point matches a connector
/// endpoint. We just collect all open center strokes.
fn open_center_strokes(cmds: &[SceneCommand]) -> Vec<Vec<f64>> {
    cmds.iter()
        .filter_map(|c| match c {
            SceneCommand::StrokePolyline {
                points,
                closed: false,
                align: StrokeAlign::Center,
                ..
            } => Some(points.clone()),
            _ => None,
        })
        .collect()
}

/// Same crossing geometry as `crossing_connectors_src`, but the VERTICAL
/// connector (`cv`) lives inside a translate-only `group` (no x/y → zero
/// translation, no rotation, so no `PushTransform`/`PushClip` bracket moves it).
/// Its `StrokePolyline` is therefore page-absolute and crosses the horizontal
/// connector exactly as before — proving NESTED connectors now participate in
/// line-jumps. The horizontal connector (`ch`) stays a direct page child.
fn nested_crossing_connectors_src(page_props: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.lj" name="LJ"
  tokens format="zenith-token-v1" {{
token id="color.line" type="color" value="#1e3a8a"
  }}
  styles {{}}
  document id="doc.lj" title="LJ" {{
page id="page.lj" w=(px)640 h=(px)360 {page_props} {{
  rect id="a" x=(px)40 y=(px)140 w=(px)80 h=(px)40 stroke=(token)"color.line"
  rect id="b" x=(px)520 y=(px)140 w=(px)80 h=(px)40 stroke=(token)"color.line"
  rect id="c" x=(px)300 y=(px)20 w=(px)40 h=(px)40 stroke=(token)"color.line"
  rect id="d" x=(px)300 y=(px)300 w=(px)40 h=(px)40 stroke=(token)"color.line"
  connector id="ch" from="a" to="b" stroke=(token)"color.line"
  group id="g" {{
    connector id="cv" from="c" to="d" stroke=(token)"color.line"
  }}
}}
  }}
}}
"##
    )
}

fn has_diag(result: &zenith_scene::CompileResult, code: &str) -> bool {
    result.diagnostics.iter().any(|d| d.code == code)
}

#[path = "shapes_connectors/avoid.rs"]
mod avoid;
#[path = "shapes_connectors/exact_outline.rs"]
mod exact_outline;
#[path = "shapes_connectors/label.rs"]
mod label;
#[path = "shapes_connectors/line_jumps.rs"]
mod line_jumps;
#[path = "shapes_connectors/markers.rs"]
mod markers;
#[path = "shapes_connectors/orthogonal.rs"]
mod orthogonal;
#[path = "shapes_connectors/straight.rs"]
mod straight;
#[path = "shapes_connectors/unresolved.rs"]
mod unresolved;
#[path = "shapes_connectors/unsupported.rs"]
mod unsupported;
