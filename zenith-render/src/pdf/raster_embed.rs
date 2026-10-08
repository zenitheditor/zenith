//! Raster embedding for complete compositing scopes.
//!
//! Effects, opacity layers, and crossed scopes include their enclosing state.
//! Non-normal blends include the full page backdrop. Integer-pixel crops retain
//! their scene offsets and dimensions under the page transform.

use pdf_writer::Content;
use zenith_core::{AssetProvider, FontProvider};
use zenith_scene::{Scene, SceneCommand};

use super::content::{IMAGE_PREFIX, PageResources, emit_command, name};
use super::font::FontPlan;
use super::image::decoded_image_from_straight_rgba;

/// Rasterize a complete structural range and embed its integer-pixel crop.
/// Enclosing transforms, clips, and layers live inside `sub_commands`.
/// The caller emits the image under the page transform alone.
/// Raster errors retain all commands through the vector emitter.
pub(super) fn embed_rasterized_region(
    content: &mut Content,
    res: &mut PageResources,
    sub_commands: &[SceneCommand],
    page: (f64, f64),
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    font_plan: &FontPlan,
) {
    if embed_strict(content, res, sub_commands, page, fonts, assets, 1.0).is_err() {
        for c in sub_commands {
            emit_command(content, res, c, page, fonts, assets, font_plan);
        }
    }
}

pub(super) fn embed_strict(
    content: &mut Content,
    res: &mut PageResources,
    sub_commands: &[SceneCommand],
    page: (f64, f64),
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    raster_scale: f64,
) -> Result<(), crate::RenderError> {
    let (pw, ph) = page;
    crate::raster_capture::check_capture(page, raster_scale)?;
    let mut sub_scene = Scene::new(pw, ph);
    sub_scene.commands = sub_commands.to_vec();
    let img = crate::render::render_image_scaled(&sub_scene, raster_scale, fonts, assets)?;
    drop(sub_scene);
    crop_and_embed(content, res, &img.rgba, img.width, img.height, raster_scale)
}

/// Crop a rendered straight-alpha RGBA buffer to its tight opaque bounding box
/// and embed that crop as an image XObject placed back at its scene position.
///
/// Used by [`embed_rasterized_region`]. A fully transparent (or zero-sized /
/// malformed) buffer embeds nothing.
fn crop_and_embed(
    content: &mut Content,
    res: &mut PageResources,
    rgba: &[u8],
    iw: u32,
    ih: u32,
    raster_scale: f64,
) -> Result<(), crate::RenderError> {
    // Defensive: the buffer must be exactly iw*ih*4 bytes for the row math below.
    let expected = match (iw as usize)
        .checked_mul(ih as usize)
        .and_then(|n| n.checked_mul(4))
    {
        Some(n) => n,
        None => return Ok(()),
    };
    if iw == 0 || ih == 0 || rgba.len() != expected {
        return Ok(());
    }
    let stride = iw as usize * 4;

    // 4. Scan for the tight opaque bounding box (alpha byte > 0). All-transparent
    //    ⇒ nothing to draw.
    let mut min_x = iw;
    let mut min_y = ih;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut found = false;
    for (y, row) in rgba.chunks_exact(stride).enumerate() {
        for (x, [_, _, _, alpha]) in row.as_chunks::<4>().0.iter().enumerate() {
            if *alpha > 0 {
                found = true;
                let (xu, yu) = (x as u32, y as u32);
                if xu < min_x {
                    min_x = xu;
                }
                if yu < min_y {
                    min_y = yu;
                }
                if xu > max_x {
                    max_x = xu;
                }
                if yu > max_y {
                    max_y = yu;
                }
            }
        }
    }
    if !found {
        return Ok(());
    }

    // 5. Crop to (cw, ch) at offset (ox, oy) by copying rows.
    let ox = min_x;
    let oy = min_y;
    let cw = max_x - min_x + 1;
    let ch = max_y - min_y + 1;
    let crop_stride = cw as usize * 4;
    let mut cropped = Vec::with_capacity(crop_stride * ch as usize);
    for y in oy..=max_y {
        let row_start = y as usize * stride + ox as usize * 4;
        let row_end = row_start + crop_stride;
        match rgba.get(row_start..row_end) {
            Some(slice) => cropped.extend_from_slice(slice),
            None => return Ok(()), // bounds guard: never index out of range
        }
    }

    // 6. Encode the crop as an image XObject.
    let Some(decoded) = decoded_image_from_straight_rgba(&cropped, cw, ch) else {
        return Ok(());
    };
    let placement = [
        f64::from(cw) / raster_scale,
        0.0,
        0.0,
        -f64::from(ch) / raster_scale,
        f64::from(ox) / raster_scale,
        f64::from(oy + ch) / raster_scale,
    ];
    if placement
        .iter()
        .any(|value| !value.is_finite() || value.abs() > f64::from(f32::MAX))
    {
        return Err(crate::RenderError::new(format!(
            "PDF raster capture placement exceeds supported coordinates at scale {raster_scale}; increase raster capture scale"
        )));
    }
    let id = res.images.len();
    res.images.push(decoded);

    // 7. Place it: the crop's top-left maps to scene (ox, oy) and its pixel size
    //    is (cw, ch). The outer page CTM already flips y, so an image y-up unit
    //    square maps via [cw 0 0 -ch ox oy+ch] — identical pattern to emit_image.
    content.save_state();
    let mut placement = placement.map(|value| value as f32);
    if raster_scale == 1.0 {
        // Retain the legacy default-scale addition order.
        if let Some(bottom) = placement.last_mut() {
            *bottom = oy as f32 + ch as f32;
        }
    }
    content.transform(placement);
    content.x_object(name(IMAGE_PREFIX, id).as_name());
    content.restore_state();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cropped_offsets_and_dimensions_use_requested_scale() {
        let rgba = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 200, 30, 60, 255];
        for scale in [1.5, 2.0, 0.5] {
            let mut content = Content::new();
            let mut res = PageResources::default();
            crop_and_embed(&mut content, &mut res, &rgba, 2, 2, scale).unwrap();
            assert_eq!((res.images[0].width, res.images[0].height), (1, 1));
            let text = String::from_utf8(content.finish().into_vec()).unwrap();
            assert!(
                text.contains(&format!(
                    "{} 0 0 -{} {} {} cm",
                    (1.0 / scale) as f32,
                    (1.0 / scale) as f32,
                    (1.0 / scale) as f32,
                    (2.0 / scale) as f32
                )),
                "{text}"
            );
        }
    }
}
