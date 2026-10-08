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
    let (pw, ph) = page;
    let mut sub_scene = Scene::new(pw, ph);
    sub_scene.commands = sub_commands.to_vec();

    let img = match crate::render::render_image(&sub_scene, fonts, assets) {
        Ok(i) => i,
        Err(_) => {
            // Retain the region body when raster rendering returns an error.
            for c in sub_commands {
                emit_command(content, res, c, page, fonts, assets, font_plan);
            }
            return;
        }
    };
    drop(sub_scene);
    crop_and_embed(content, res, &img.rgba, img.width, img.height);
}

/// Crop a rendered straight-alpha RGBA buffer to its tight opaque bounding box
/// and embed that crop as an image XObject placed back at its scene position.
///
/// Used by [`embed_rasterized_region`]. A fully transparent (or zero-sized /
/// malformed) buffer embeds nothing.
fn crop_and_embed(content: &mut Content, res: &mut PageResources, rgba: &[u8], iw: u32, ih: u32) {
    // Defensive: the buffer must be exactly iw*ih*4 bytes for the row math below.
    let expected = match (iw as usize)
        .checked_mul(ih as usize)
        .and_then(|n| n.checked_mul(4))
    {
        Some(n) => n,
        None => return,
    };
    if iw == 0 || ih == 0 || rgba.len() != expected {
        return;
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
        for (x, px) in row.chunks_exact(4).enumerate() {
            if let [_, _, _, alpha] = px
                && *alpha > 0
            {
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
        return;
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
            None => return, // bounds guard: never index out of range
        }
    }

    // 6. Encode the crop as an image XObject.
    let Some(decoded) = decoded_image_from_straight_rgba(&cropped, cw, ch) else {
        return;
    };
    let id = res.images.len();
    res.images.push(decoded);

    // 7. Place it: the crop's top-left maps to scene (ox, oy) and its pixel size
    //    is (cw, ch). The outer page CTM already flips y, so an image y-up unit
    //    square maps via [cw 0 0 -ch ox oy+ch] — identical pattern to emit_image.
    content.save_state();
    content.transform([
        cw as f32,
        0.0,
        0.0,
        -(ch as f32),
        ox as f32,
        oy as f32 + ch as f32,
    ]);
    content.x_object(name(IMAGE_PREFIX, id).as_name());
    content.restore_state();
}
