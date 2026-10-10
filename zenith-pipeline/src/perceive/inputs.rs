//! Metric inputs the document declares: bleed, margins, construction
//! guides, and top-level node boxes. No input is ever invented.

use std::collections::BTreeMap;

use zenith_core::{Dimension, Document, Page, PathAnchor, Unit, dim_to_px};
use zenith_geometry::{ConstructionGuide, Point2};
use zenith_scene::CompiledBox;

use super::raster::PixelRect;

/// Why `clearspace` does not apply: a margin side is missing.
pub(crate) const NO_MARGINS: &str = "the page and document do not declare all four margins; \
set margin-top, margin-bottom, margin-inner, and margin-outer";

/// Why `clearspace` does not apply: a margin does not resolve.
pub(crate) const BAD_MARGIN: &str = "a page margin is not a positive px or pt length; \
run zenith validate";

/// Why `grid_conformance` does not apply: no guide resolves.
pub(crate) const NO_GUIDES: &str = "the page declares no construction guide that resolves; \
add a construction block with segment or circle guides";

/// Why `grid_conformance` does not apply: no top-level node has a box.
pub(crate) const NO_NODES: &str = "the page has no top-level node with a compiled box";

/// Why `optical_balance` and `clearspace` do not apply: the page is flat.
pub(crate) const NO_INK: &str = "every pixel of the page is the same, so there is no ink";

/// The page's print bleed in px: 0 when absent, unresolvable, or not positive.
/// The same rule the compile applies.
fn page_bleed(page: &Page) -> f64 {
    page.bleed
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit))
        .filter(|px| px.is_finite() && *px > 0.0)
        .unwrap_or(0.0)
}

/// The trim box of a `width × height` scale-1 raster of `page`: the raster
/// less the rounded bleed on every side.
pub(crate) fn trim_rect(page: &Page, width: u32, height: u32) -> PixelRect {
    let bleed = page_bleed(page).round();
    let inset = if bleed >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        // `bleed` is finite, non-negative, and below u32::MAX here.
        bleed as u32
    };
    PixelRect {
        x: inset,
        y: inset,
        width: width.saturating_sub(inset.saturating_mul(2)),
        height: height.saturating_sub(inset.saturating_mul(2)),
    }
}

/// The smallest of the four effective page margins in px, or why there is
/// none. Every side must be declared, on the page or the document.
pub(crate) fn clearspace_required(doc: &Document, page: &Page) -> Result<f64, &'static str> {
    let (inner, outer, top, bottom) = doc.effective_margins(page);
    let mut required = f64::INFINITY;
    for side in [inner, outer, top, bottom] {
        let side = side.ok_or(NO_MARGINS)?;
        let px = dim_to_px(side.value, &side.unit)
            .filter(|px| px.is_finite() && *px > 0.0)
            .ok_or(BAD_MARGIN)?;
        required = required.min(px);
    }
    Ok(required)
}

/// The construction guides of `page` that resolve to finite, non-degenerate
/// geometry, in declaration order. Others are skipped, as the construction
/// overlay skips them.
pub(crate) fn construction_guides(page: &Page) -> Vec<ConstructionGuide> {
    let px = |d: &Option<Dimension>| d.as_ref().and_then(|d| dim_to_px(d.value, &d.unit));
    let mut guides = Vec::new();
    for guide in &page.construction.guides {
        let resolved = match guide.guide_type.as_str() {
            "segment" => match (px(&guide.x1), px(&guide.y1), px(&guide.x2), px(&guide.y2)) {
                (Some(x1), Some(y1), Some(x2), Some(y2)) => Point2::new(x1, y1)
                    .and_then(|start| Ok((start, Point2::new(x2, y2)?)))
                    .and_then(|(start, end)| ConstructionGuide::segment(start, end))
                    .ok(),
                _ => None,
            },
            "circle" => match (px(&guide.cx), px(&guide.cy), px(&guide.r)) {
                (Some(cx), Some(cy), Some(r)) => Point2::new(cx, cy)
                    .and_then(|center| ConstructionGuide::circle(center, r))
                    .ok(),
                _ => None,
            },
            _ => None,
        };
        guides.extend(resolved);
    }
    guides
}

/// One closed four-anchor contour per top-level child of `page` that has a
/// finite compiled box: the `visual` box (what the node paints, every
/// transform applied), in page px. Children keep document order.
pub(crate) fn node_contours(
    page: &Page,
    boxes: &BTreeMap<String, CompiledBox>,
) -> Vec<Vec<PathAnchor>> {
    let mut contours = Vec::new();
    for child in &page.children {
        let Some(compiled) = child.id().and_then(|id| boxes.get(id)) else {
            continue;
        };
        let b = compiled.visual;
        if !(b.x.is_finite() && b.y.is_finite() && b.w.is_finite() && b.h.is_finite()) {
            continue;
        }
        contours.push(vec![
            anchor(b.x, b.y),
            anchor(b.x + b.w, b.y),
            anchor(b.x + b.w, b.y + b.h),
            anchor(b.x, b.y + b.h),
        ]);
    }
    contours
}

fn anchor(x: f64, y: f64) -> PathAnchor {
    let px = |value| {
        Some(Dimension {
            value,
            unit: Unit::Px,
        })
    };
    PathAnchor {
        x: px(x),
        y: px(y),
        kind: None,
        in_x: None,
        in_y: None,
        out_x: None,
        out_y: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource};

    fn parse(page_attrs: &str, page_body: &str) -> Document {
        let src = format!(
            "zenith version=1 {{\n  document id=\"d\" {{\n    page id=\"p\" w=(px)100 h=(px)80 {page_attrs} {{\n{page_body}\n    }}\n  }}\n}}\n"
        );
        KdlAdapter.parse(src.as_bytes()).expect("parse")
    }

    #[test]
    fn trim_rect_removes_the_rounded_bleed() {
        let doc = parse("bleed=(px)9.6", "");
        let page = &doc.body.pages[0];
        assert_eq!(
            trim_rect(page, 120, 100),
            PixelRect {
                x: 10,
                y: 10,
                width: 100,
                height: 80
            }
        );
        let plain = parse("", "");
        assert_eq!(trim_rect(&plain.body.pages[0], 100, 80).x, 0);
    }

    #[test]
    fn clearspace_required_is_the_smallest_declared_margin() {
        let doc = parse(
            "margin-top=(px)12 margin-bottom=(px)8 margin-inner=(px)10 margin-outer=(pt)6",
            "",
        );
        assert_eq!(clearspace_required(&doc, &doc.body.pages[0]), Ok(8.0));
    }

    #[test]
    fn clearspace_required_needs_every_side() {
        let doc = parse("margin-top=(px)12", "");
        assert_eq!(
            clearspace_required(&doc, &doc.body.pages[0]),
            Err(NO_MARGINS)
        );
        let zero = parse(
            "margin-top=(px)0 margin-bottom=(px)8 margin-inner=(px)10 margin-outer=(px)6",
            "",
        );
        assert_eq!(
            clearspace_required(&zero, &zero.body.pages[0]),
            Err(BAD_MARGIN)
        );
    }

    #[test]
    fn construction_guides_skip_unresolved_and_degenerate_guides() {
        let doc = parse(
            "",
            r#"      construction {
        guide id="a" type="segment" x1=(px)0 y1=(px)0 x2=(px)100 y2=(px)0
        guide id="b" type="segment" x1=(px)5 y1=(px)5 x2=(px)5 y2=(px)5
        guide id="c" type="circle" cx=(px)50 cy=(px)40 r=(px)10
        guide id="d" type="circle" cx=(px)50 cy=(px)40
      }"#,
        );
        let guides = construction_guides(&doc.body.pages[0]);
        assert_eq!(guides.len(), 2);
        assert!(matches!(guides[0], ConstructionGuide::Segment { .. }));
        assert!(matches!(guides[1], ConstructionGuide::Circle { .. }));
    }

    #[test]
    fn node_contours_use_top_level_visual_boxes_in_document_order() {
        let doc = parse(
            "",
            r#"      rect id="b" x=(px)0 y=(px)0 w=(px)10 h=(px)10
      rect id="a" x=(px)20 y=(px)0 w=(px)10 h=(px)10"#,
        );
        let page = &doc.body.pages[0];
        let mut boxes = BTreeMap::new();
        for (id, x) in [("a", 20.0), ("b", 0.0)] {
            boxes.insert(id.to_owned(), compiled_box(x));
        }
        let contours = node_contours(page, &boxes);
        assert_eq!(contours.len(), 2);
        assert_eq!(contours[0][1].x.as_ref().map(|d| d.value), Some(10.0));
        assert_eq!(contours[1][0].x.as_ref().map(|d| d.value), Some(20.0));
    }

    fn compiled_box(x: f64) -> CompiledBox {
        let fonts = zenith_core::BytesFontProvider::new();
        let src = format!(
            "zenith version=1 {{\n  document id=\"d\" {{\n    page id=\"p\" w=(px)100 h=(px)80 {{\n      rect id=\"r\" x=(px){x} y=(px)0 w=(px)10 h=(px)10\n    }}\n  }}\n}}\n"
        );
        let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
        let prep = zenith_scene::DocumentPrep::new(&doc, None, None);
        let compiler = zenith_scene::PageCompiler::new(&prep, &fonts);
        compiler
            .compiled_boxes(0)
            .remove("r")
            .expect("compiled box")
    }
}
