//! Capability selection over the styled, outlined SVG tree.
use super::svg::SvgPlacement;
use super::svg_parse::{self, Affine};
use resvg::usvg::{self, NodeKind, Paint};
use zenith_core::{AssetKind, AssetProvider, FontProvider};
use zenith_scene::{FitMode, SceneCommand};

#[derive(Default)]
pub(super) struct SvgCapabilities {
    database: Option<usvg::fontdb::Database>,
}

impl SvgCapabilities {
    pub(super) fn requires_raster(
        &mut self,
        command: &SceneCommand,
        fonts: &dyn FontProvider,
        assets: &dyn AssetProvider,
    ) -> bool {
        let SceneCommand::DrawImage {
            x,
            y,
            w,
            h,
            asset_id,
            fit,
            pos_x,
            pos_y,
            opacity,
            clip_shape,
            svg_style,
            ..
        } = command
        else {
            return false;
        };
        let Some(asset) = assets.by_id(asset_id) else {
            return false;
        };
        if asset.kind != AssetKind::Svg {
            return false;
        }
        let database = self
            .database
            .get_or_insert_with(|| svg_parse::font_database(fonts));
        let Some(tree) = svg_parse::parse(&asset.bytes, *svg_style, database) else {
            return false;
        };
        let place = SvgPlacement {
            x: *x,
            y: *y,
            w: *w,
            h: *h,
            fit: *fit,
            pos_x: *pos_x,
            pos_y: *pos_y,
            opacity: *opacity,
            clip_shape,
            svg_style: *svg_style,
        };
        let Some(transform) = svg_parse::placement_transform(&tree, place) else {
            return false;
        };
        let view_box =
            usvg::utils::view_box_to_transform(tree.view_box.rect, tree.view_box.aspect, tree.size);
        view_box != usvg::Transform::default()
            || *opacity < 1.0
            || *fit == FitMode::None
            || unsupported_node(&tree.root, transform)
    }
}

fn unsupported_node(node: &usvg::Node, parent: Affine) -> bool {
    let kind = node.borrow();
    let transform = parent.then(Affine::from_usvg(kind.transform()));
    match &*kind {
        NodeKind::Group(group) => {
            group.should_isolate()
                || node
                    .children()
                    .any(|child| unsupported_node(&child, transform))
        }
        NodeKind::Path(path) => {
            if path.visibility != usvg::Visibility::Visible {
                return false;
            }
            path.paint_order == usvg::PaintOrder::StrokeAndFill
                || path
                    .fill
                    .as_ref()
                    .is_some_and(|fill| unsupported_fill(&fill.paint, path, transform))
                || path.stroke.as_ref().is_some_and(|stroke| {
                    !matches!(stroke.paint, Paint::Color(_))
                        || stroke.linejoin == usvg::LineJoin::MiterClip
                        || !transform.conformal()
                        || unsupported_stroke_numbers(stroke, transform)
                })
        }
        NodeKind::Image(_) | NodeKind::Text(_) => true,
    }
}

fn unsupported_fill(paint: &Paint, path: &usvg::Path, transform: Affine) -> bool {
    match paint {
        Paint::Color(_) => false,
        Paint::RadialGradient(_) | Paint::Pattern(_) => true,
        Paint::LinearGradient(gradient) => {
            let valid_stops = gradient.stops.len() >= 2
                && gradient.stops.first().is_some_and(|stop| stop.offset.get() == 0.0)
                && gradient.stops.last().is_some_and(|stop| stop.offset.get() == 1.0)
                && gradient.stops.iter().all(|stop| stop.opacity == usvg::Opacity::ONE)
                && gradient.stops.windows(2).all(|stops| matches!(stops, [left, right] if left.offset.get() < right.offset.get()));
            let bounds = path.data.bounds();
            let gradient_map = match gradient.units {
                usvg::Units::UserSpaceOnUse => transform,
                usvg::Units::ObjectBoundingBox => transform.then(Affine::scale_translate(
                    f64::from(bounds.width()),
                    f64::from(bounds.height()),
                    0.0,
                    0.0,
                )),
            };
            let axis_aligned = (gradient.x1 == gradient.x2 || gradient.y1 == gradient.y2)
                && gradient_map.b == 0.0
                && gradient_map.c == 0.0;
            let invalid_endpoints = super::svg::resolve_linear(gradient, path, transform)
                .is_none_or(|gradient| match gradient.geometry {
                    super::gradient::GradientGeometry::Axial([x0, y0, x1, y1]) => {
                        ![x0, y0, x1, y1].iter().all(|value| value.is_finite())
                            || (x0 == x1 && y0 == y1)
                    }
                    super::gradient::GradientGeometry::Radial(_) => true,
                });
            invalid_endpoints
                || (gradient.units == usvg::Units::ObjectBoundingBox && path.text_bbox.is_some())
                || !valid_stops
                || gradient.spread_method != usvg::SpreadMethod::Pad
                || gradient.transform != usvg::Transform::default()
                || !(gradient_map.conformal() || axis_aligned)
        }
    }
}

fn representable(value: f64) -> bool {
    value.is_finite() && value.abs() <= f64::from(f32::MAX)
}

fn unsupported_stroke_numbers(stroke: &usvg::Stroke, transform: Affine) -> bool {
    let scale = transform.avg_scale();
    if !representable(f64::from(stroke.width.get()) * scale) {
        return true;
    }
    let Some(dashes) = &stroke.dasharray else {
        return false;
    };
    let period = dashes.iter().map(|dash| f64::from(*dash)).sum::<f64>()
        * if dashes.len() % 2 == 1 { 2.0 } else { 1.0 };
    period <= 0.0
        || !period.is_finite()
        || !stroke.dashoffset.is_finite()
        || dashes
            .iter()
            .any(|dash| *dash < 0.0 || !representable(f64::from(*dash) * scale))
        || !representable(f64::from(stroke.dashoffset).rem_euclid(period) * scale)
}
