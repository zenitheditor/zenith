//! Image numerics follow the resolved asset kind and effective crop.

use super::values::Check;
use crate::RenderError;
use std::collections::BTreeMap;
pub(in crate::pdf) type RasterDimensions = BTreeMap<String, Option<(u32, u32)>>;
use zenith_core::{AssetKind, AssetProvider};
use zenith_scene::SceneCommand;

pub(in crate::pdf) fn check_image(
    command: &SceneCommand,
    page: usize,
    index: usize,
    assets: &dyn AssetProvider,
    dimensions: &mut RasterDimensions,
) -> Result<(), RenderError> {
    let SceneCommand::DrawImage {
        x,
        y,
        w,
        h,
        asset_id,
        fit,
        pos_x,
        pos_y,
        src_rect,
        svg_style,
        ..
    } = command
    else {
        return Ok(());
    };
    let Some(asset) = assets.by_id(asset_id) else {
        return Ok(());
    };
    let check = Check {
        page,
        command: index,
    };
    match asset.kind {
        AssetKind::Image => {
            if let Some(rect) = src_rect {
                for (field, value) in [("x", rect.x), ("y", rect.y), ("w", rect.w), ("h", rect.h)] {
                    check.finite(&format!("image.src_rect.{field}"), value)?;
                }
            }
            let intrinsic = match dimensions.get(asset_id) {
                Some(dimensions) => *dimensions,
                None => {
                    let intrinsic = crate::tiny_skia::decode_raster_to_pixmap(&asset.bytes)
                        .map(|pixmap| (pixmap.width(), pixmap.height()));
                    dimensions.insert(asset_id.clone(), intrinsic);
                    intrinsic
                }
            };
            let Some(mut source) = intrinsic else {
                return Ok(());
            };
            if let Some(crop) = src_rect {
                let Some(rect) = crate::tiny_skia::crop_raster_rect(source, crop) else {
                    return Ok(());
                };
                source = (rect.width(), rect.height());
            }
            let (sw, sh) = (f64::from(source.0), f64::from(source.1));
            let (sx, sy, tx, ty) = crate::pdf::content::image::fit_transform(
                (*x, *y, *w, *h),
                (sw, sh),
                *fit,
                (*pos_x, *pos_y),
            );
            for (field, value) in [
                ("sx", sx),
                ("sy", sy),
                ("tx", tx),
                ("ty", ty),
                ("width", sw * sx),
                ("height", sh * sy),
            ] {
                check.coordinate(&format!("image.fit.{field}"), value)?;
            }
            check.coordinate(
                "image.fit.translation_y.f32",
                f64::from(ty as f32 + (sh * sy) as f32),
            )?;
        }
        AssetKind::Svg => {
            if let Some(style) = svg_style {
                if let Some(color) = style.fill {
                    check.color("svg_style.fill", &color)?;
                }
                if let Some(color) = style.stroke {
                    check.color("svg_style.stroke", &color)?;
                }
                if let Some(width) = style.stroke_width {
                    check.finite("svg_style.stroke_width", width)?;
                }
            }
        }
        AssetKind::Font | AssetKind::Unknown(_) => {}
    }
    Ok(())
}
