//! Render-equivalence test for converted native Lucide icons (Doc-22 Phase 4:
//! "a converted icon renders byte-visually equivalent within tolerance to the
//! source SVG").
//!
//! For each icon we rasterize two images at the same size, color and stroke
//! style and compare them:
//!
//! 1. the SOURCE SVG bytes (checked in under `assets/libraries/icons/lucide/`),
//!    rasterized with resvg — the exact engine the render backend uses; and
//! 2. the CONVERTED native paths produced by
//!    [`zenith_producers::svg_to_native_paths`] (the same call the native
//!    Lucide pack generator uses), stroked with tiny-skia — the engine resvg
//!    itself renders through.
//!
//! Because both rasters are produced by the *same* tiny-skia engine over the
//! *same* usvg-normalized geometry, a correct conversion can differ only by
//! sub-pixel anti-aliasing along stroke edges. A real conversion defect — a
//! dropped subpath, mirrored/offset output, a wrong cap/join — changes whole
//! runs of ink pixels and is caught by the tolerances below. The tolerances are
//! chosen up front from that reasoning, NOT tuned to make the test pass.

use resvg::usvg::{self, TreeParsing};
use tiny_skia::{
    Color, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform as SkTransform,
};
use zenith_core::{Node, PathNode, PropertyValue};
use zenith_producers::{SvgNativeOptions, svg_to_native_paths};

/// Output raster edge length in device pixels.
const SIZE: u32 = 96;
/// Lucide icons author against a 24×24 viewBox.
const VIEWBOX: f64 = 24.0;
/// Device-space scale from viewBox units to output pixels.
const SCALE: f64 = SIZE as f64 / VIEWBOX;
/// Lucide's authored stroke width, in viewBox units.
const STROKE_WIDTH_VIEWBOX: f64 = 2.0;

// ── Tolerances (chosen up front; see module docs) ──────────────────────────
//
// `visually equivalent within tolerance` means: identical geometry, differing
// only by edge anti-aliasing. Ink covers only ~10–20% of the canvas, so even
// generous AA disagreement along every stroke edge stays well under these:
//
//  * mean absolute per-channel difference over ALL pixels ≤ 6 (of 255), and
//  * at most 4% of pixels differ by more than 32 (of 255) on any channel.
//
// A dropped/mirrored/offset subpath or wrong cap/join moves whole ink regions
// and blows past both bounds.
const MEAN_ABS_MAX: f64 = 6.0;
const FRAC_OVER_DELTA_MAX: f64 = 0.04;
const DELTA: u8 = 32;

/// Read the checked-in source SVG bytes for `name`.
fn source_svg(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/libraries/icons/lucide")
        .join(format!("{name}.svg"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// A fresh white opaque `SIZE×SIZE` pixmap.
fn white_pixmap() -> Pixmap {
    let mut pm = Pixmap::new(SIZE, SIZE).expect("allocate pixmap");
    pm.fill(Color::WHITE);
    pm
}

/// Rasterize the SOURCE SVG at `SIZE×SIZE` on white, with `currentColor`
/// resolved to solid black — the reference image.
fn rasterize_source(name: &str) -> Vec<u8> {
    let bytes = source_svg(name);
    // Lucide strokes use `currentColor`; pin it to black so the reference has a
    // known stroke color (matching the native raster below).
    let svg = String::from_utf8(bytes).expect("svg is utf-8");
    let svg = svg.replace("currentColor", "#000000");

    let tree = usvg::Tree::from_data(svg.as_bytes(), &usvg::Options::default())
        .expect("source svg parses");
    let size = tree.size;
    let scale = SIZE as f32 / size.width();

    let mut pm = white_pixmap();
    let rtree = resvg::Tree::from_usvg(&tree);
    rtree.render(SkTransform::from_scale(scale, scale), &mut pm.as_mut());
    pm.data().to_vec()
}

/// Map a converter linecap string to a tiny-skia cap (default: butt).
fn line_cap(cap: Option<&str>) -> LineCap {
    match cap {
        Some("round") => LineCap::Round,
        Some("square") => LineCap::Square,
        _ => LineCap::Butt,
    }
}

/// Map a converter linejoin string to a tiny-skia join (default: miter).
fn line_join(join: Option<&str>) -> LineJoin {
    match join {
        Some("round") => LineJoin::Round,
        Some("bevel") => LineJoin::Bevel,
        _ => LineJoin::Miter,
    }
}

/// Device x of a viewBox coordinate held in a `Dimension`.
fn dev(d: Option<&zenith_core::Dimension>) -> Option<f32> {
    d.map(|d| (d.value * SCALE) as f32)
}

/// Build a tiny-skia path from a converted [`PathNode`], scaled to device
/// space, replicating zenith-geometry's anchor→segment rule exactly: a segment
/// is a line iff BOTH the departing `out` handle and the arriving `in` handle
/// are absent, otherwise a cubic whose controls fall back to the endpoints.
fn build_path(node: &PathNode) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    for subpath in &node.subpaths {
        let anchors = &subpath.anchors;
        if anchors.is_empty() {
            continue;
        }
        let closed = subpath.closed == Some(true);
        let pt = |a: &zenith_core::PathAnchor| -> Option<(f32, f32)> {
            Some((dev(a.x.as_ref())?, dev(a.y.as_ref())?))
        };
        let (sx, sy) = pt(&anchors[0])?;
        pb.move_to(sx, sy);

        let last = anchors.len() - 1;
        for i in 0..anchors.len() {
            let end_idx = if i < last {
                i + 1
            } else if closed {
                0
            } else {
                break;
            };
            let start = &anchors[i];
            let end = &anchors[end_idx];
            let (ex, ey) = pt(end)?;
            let out = match (dev(start.out_x.as_ref()), dev(start.out_y.as_ref())) {
                (Some(x), Some(y)) => Some((x, y)),
                _ => None,
            };
            let inn = match (dev(end.in_x.as_ref()), dev(end.in_y.as_ref())) {
                (Some(x), Some(y)) => Some((x, y)),
                _ => None,
            };
            match (out, inn) {
                (None, None) => pb.line_to(ex, ey),
                (out, inn) => {
                    let (c1x, c1y) =
                        out.unwrap_or((dev(start.x.as_ref())?, dev(start.y.as_ref())?));
                    let (c2x, c2y) = inn.unwrap_or((ex, ey));
                    pb.cubic_to(c1x, c1y, c2x, c2y, ex, ey);
                }
            }
        }
        if closed {
            pb.close();
        }
    }
    pb.finish()
}

/// Rasterize the CONVERTED native paths at `SIZE×SIZE` on white, black ink.
fn rasterize_native(name: &str) -> Vec<u8> {
    let bytes = source_svg(name);
    let options = SvgNativeOptions {
        id_prefix: "icon".to_owned(),
        stroke: Some(PropertyValue::Literal("#000000".to_owned())),
        fill: Some(PropertyValue::Literal("#000000".to_owned())),
        stroke_width: Some(PropertyValue::Literal("2px".to_owned())),
    };
    let nodes = svg_to_native_paths(&bytes, &options).expect("convert to native paths");
    assert!(!nodes.is_empty(), "{name}: converter produced no nodes");

    let mut pm = white_pixmap();
    let mut black = Paint::default();
    black.set_color(Color::BLACK);
    black.anti_alias = true;

    for node in &nodes {
        let Node::Path(path) = node else {
            panic!("{name}: converter produced a non-path node");
        };
        let Some(skpath) = build_path(path) else {
            panic!("{name}: path '{}' produced no geometry", path.id);
        };
        if path.fill.is_some() {
            pm.fill_path(
                &skpath,
                &black,
                tiny_skia::FillRule::Winding,
                SkTransform::identity(),
                None,
            );
        }
        if path.stroke.is_some() {
            let stroke = Stroke {
                width: (STROKE_WIDTH_VIEWBOX * SCALE) as f32,
                line_cap: line_cap(path.stroke_linecap.as_deref()),
                line_join: line_join(path.stroke_linejoin.as_deref()),
                miter_limit: 4.0,
                ..Default::default()
            };
            pm.stroke_path(&skpath, &black, &stroke, SkTransform::identity(), None);
        }
    }
    pm.data().to_vec()
}

/// Mean absolute per-channel difference (RGB) and the fraction of pixels that
/// differ by more than `DELTA` on any channel. Both rasters are opaque white
/// backgrounds, so alpha is uniform and ignored.
fn compare(a: &[u8], b: &[u8]) -> (f64, f64) {
    assert_eq!(a.len(), b.len());
    let pixels = a.len() / 4;
    let mut sum_abs: u64 = 0;
    let mut over: u64 = 0;
    for i in 0..pixels {
        let base = i * 4;
        let mut pixel_over = false;
        for c in 0..3 {
            let d = a[base + c].abs_diff(b[base + c]);
            sum_abs += u64::from(d);
            if d > DELTA {
                pixel_over = true;
            }
        }
        if pixel_over {
            over += 1;
        }
    }
    let mean_abs = sum_abs as f64 / (pixels as f64 * 3.0);
    let frac_over = over as f64 / pixels as f64;
    (mean_abs, frac_over)
}

/// Fraction of pixels that are not pure white (i.e. carry ink). Guards against
/// a false pass where both rasters are blank.
fn ink_fraction(data: &[u8]) -> f64 {
    let pixels = data.len() / 4;
    let mut ink = 0u64;
    for i in 0..pixels {
        let base = i * 4;
        if data[base] != 255 || data[base + 1] != 255 || data[base + 2] != 255 {
            ink += 1;
        }
    }
    ink as f64 / pixels as f64
}

fn assert_equivalent(name: &str) {
    let src = rasterize_source(name);
    let native = rasterize_native(name);
    let src_ink = ink_fraction(&src);
    let native_ink = ink_fraction(&native);
    eprintln!("icon '{name}': src_ink = {src_ink:.4}, native_ink = {native_ink:.4}");
    assert!(
        src_ink > 0.03,
        "icon '{name}': source raster is nearly blank ({src_ink:.4} ink) — nothing to compare"
    );
    assert!(
        native_ink > 0.03,
        "icon '{name}': native raster is nearly blank ({native_ink:.4} ink) — converter drew nothing"
    );
    let (mean_abs, frac_over) = compare(&src, &native);
    eprintln!(
        "icon '{name}': mean_abs_per_channel = {mean_abs:.4}, \
         frac_pixels_over_{DELTA} = {frac_over:.4}"
    );
    assert!(
        mean_abs <= MEAN_ABS_MAX,
        "icon '{name}': mean abs per-channel diff {mean_abs:.4} exceeds {MEAN_ABS_MAX} \
         — converted paths diverge from source SVG"
    );
    assert!(
        frac_over <= FRAC_OVER_DELTA_MAX,
        "icon '{name}': {:.2}% of pixels differ by >{DELTA} (limit {:.2}%) \
         — converted paths diverge from source SVG",
        frac_over * 100.0,
        FRAC_OVER_DELTA_MAX * 100.0
    );
}

#[test]
fn monitor_native_matches_source() {
    assert_equivalent("monitor");
}

#[test]
fn cloud_native_matches_source() {
    assert_equivalent("cloud");
}

#[test]
fn wifi_native_matches_source() {
    assert_equivalent("wifi");
}
