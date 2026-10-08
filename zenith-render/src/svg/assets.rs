use super::{
    geometry::{self, BoxRect, finite},
    writer::Writer,
};
use crate::RenderError;
use resvg::usvg::{self, TreeParsing, TreeWriting};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use zenith_core::{AssetKind, AssetProvider, FontProvider};
use zenith_scene::{FitMode, ImageClip, SceneCommand, SrcRect, SvgStyle};

pub(super) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in bytes.chunks(3) {
        let mut iter = chunk.iter().copied();
        let a = iter.next().unwrap_or(0);
        let b = iter.next().unwrap_or(0);
        let c = iter.next().unwrap_or(0);
        let indices = [
            a >> 2,
            ((a & 3) << 4) | (b >> 4),
            ((b & 15) << 2) | (c >> 6),
            c & 63,
        ];
        for (index, value) in indices.into_iter().enumerate() {
            if index > chunk.len() {
                result.push('=');
            } else if let Some(character) = ALPHABET.get(usize::from(value)) {
                result.push(char::from(*character));
            }
        }
    }
    result
}

pub(super) fn image_element(bytes: &[u8], mime: &str, bounds: BoxRect, opacity: f64) -> String {
    let (x, y, w, h) = bounds;
    format!(
        "<image x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" preserveAspectRatio=\"none\" opacity=\"{opacity}\" xlink:href=\"data:{mime};base64,{}\"/>",
        base64(bytes)
    )
}

fn svg_options(rejected: Arc<AtomicBool>) -> usvg::Options {
    let string_rejected = rejected.clone();
    let data_rejected = rejected;
    usvg::Options {
        font_family: "Noto Sans".to_owned(),
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_string: Box::new(move |_, _| {
                string_rejected.store(true, Ordering::Relaxed);
                None
            }),
            resolve_data: Box::new(move |mime, bytes, _| {
                if mime == "image/svg+xml" {
                    let options = svg_options(data_rejected.clone());
                    match usvg::Tree::from_data(&bytes, &options) {
                        Ok(tree) => Some(usvg::ImageKind::SVG(tree)),
                        Err(_) => {
                            data_rejected.store(true, Ordering::Relaxed);
                            None
                        }
                    }
                } else if ["image/png", "image/jpeg", "image/jpg", "image/gif"].contains(&mime) {
                    if !valid_embedded_raster(mime, &bytes) {
                        data_rejected.store(true, Ordering::Relaxed);
                        return None;
                    }
                    usvg::ImageHrefResolver::default_data_resolver()(
                        mime,
                        bytes,
                        &svg_options(data_rejected.clone()),
                    )
                } else {
                    data_rejected.store(true, Ordering::Relaxed);
                    None
                }
            }),
        },
        ..Default::default()
    }
}

fn valid_embedded_raster(mime: &str, bytes: &[u8]) -> bool {
    match mime {
        "image/png" => {
            bytes.starts_with(b"\x89PNG\r\n\x1a\n")
                && crate::tiny_skia::decode_raster_to_pixmap(bytes).is_some()
        }
        "image/jpeg" | "image/jpg" => {
            bytes.starts_with(&[0xff, 0xd8, 0xff])
                && crate::tiny_skia::decode_raster_to_pixmap(bytes).is_some()
        }
        "image/gif" => {
            let mut options = gif::DecodeOptions::new();
            options.set_color_output(gif::ColorOutput::RGBA);
            let Ok(mut decoder) = options.read_info(std::io::Cursor::new(bytes)) else {
                return false;
            };
            // SVG renders the first animation frame, matching resvg's GIF decoder.
            matches!(decoder.read_next_frame(), Ok(Some(_)))
        }
        _ => false,
    }
}

fn normalize_svg(
    bytes: &[u8],
    style: Option<SvgStyle>,
    fonts: &dyn FontProvider,
) -> Result<(Vec<u8>, f64, f64), RenderError> {
    let tree = checked_svg(bytes, style, fonts)?;
    Ok((
        tree.to_string(&usvg::XmlOptions::default()).into_bytes(),
        f64::from(tree.size.width()),
        f64::from(tree.size.height()),
    ))
}

pub(crate) fn checked_svg(
    bytes: &[u8],
    style: Option<SvgStyle>,
    fonts: &dyn FontProvider,
) -> Result<usvg::Tree, RenderError> {
    let rejected = Arc::new(AtomicBool::new(false));
    let options = svg_options(rejected.clone());
    let bytes = crate::svg_style::styled_svg_bytes(bytes, style);
    let tree = usvg::Tree::from_data(&bytes, &options)
        .map_err(|error| RenderError::new(format!("invalid SVG asset: {error}")))?;
    if rejected.load(Ordering::Relaxed) {
        return Err(RenderError::new(
            "SVG asset contains unresolved external or unsupported image references",
        ));
    }
    let mut database = usvg::fontdb::Database::new();
    database.set_sans_serif_family("Noto Sans");
    database.set_serif_family("Noto Sans");
    database.set_monospace_family("Noto Sans Mono");
    for font in fonts.all_faces() {
        database.load_font_data(font.bytes.to_vec());
    }
    super::asset_text::outline(&tree.root, &database)?;
    Ok(tree)
}

fn raster(
    bytes: &[u8],
    crop: Option<&SrcRect>,
) -> Result<Option<(Vec<u8>, f64, f64)>, RenderError> {
    let mut image = crate::tiny_skia::decode_raster_to_pixmap(bytes)
        .ok_or_else(|| RenderError::new("invalid or unsupported SVG raster asset"))?;
    if let Some(crop) = crop {
        finite(&[crop.x, crop.y, crop.w, crop.h])?;
        let (w, h) = (f64::from(image.width()), f64::from(image.height()));
        let x = crop.x.clamp(0.0, w) as i32;
        let y = crop.y.clamp(0.0, h) as i32;
        let right = (crop.x + crop.w).clamp(0.0, w) as i32;
        let bottom = (crop.y + crop.h).clamp(0.0, h) as i32;
        if right <= x || bottom <= y {
            return Ok(None);
        }
        let rect = tiny_skia::IntRect::from_xywh(x, y, (right - x) as u32, (bottom - y) as u32)
            .ok_or_else(|| RenderError::new("invalid SVG image source crop"))?;
        image = image
            .as_ref()
            .clone_rect(rect)
            .ok_or_else(|| RenderError::new("SVG image source crop exceeds bounds"))?;
    }
    let bytes = image
        .encode_png()
        .map_err(|error| RenderError::new(format!("SVG raster asset encoding error: {error}")))?;
    Ok(Some((
        bytes,
        f64::from(image.width()),
        f64::from(image.height()),
    )))
}

pub(super) struct SvgCapture<'a> {
    pub(super) asset_id: &'a str,
    pub(super) intrinsic: (f64, f64),
    pub(super) destination: (f64, f64),
}

impl Writer {
    pub(super) fn image<'a>(
        &mut self,
        command: &'a SceneCommand,
        fonts: &dyn FontProvider,
        assets: &dyn AssetProvider,
    ) -> Result<Option<SvgCapture<'a>>, RenderError> {
        let (x, y, w, h, id, fit, pos_x, pos_y, opacity, clip, crop, style) =
            if let SceneCommand::DrawImage {
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
                src_rect,
                svg_style,
            } = command
            {
                (
                    *x,
                    *y,
                    *w,
                    *h,
                    asset_id,
                    *fit,
                    *pos_x,
                    *pos_y,
                    *opacity,
                    clip_shape.as_ref(),
                    src_rect.as_ref(),
                    *svg_style,
                )
            } else if let SceneCommand::DrawSvgAsset { x, y, w, h, asset } = command {
                (
                    *x,
                    *y,
                    *w,
                    *h,
                    asset,
                    FitMode::Stretch,
                    0.0,
                    0.0,
                    1.0,
                    None,
                    None,
                    None,
                )
            } else {
                return Err(RenderError::new(
                    "invalid SVG asset command; supply DrawImage or DrawSvgAsset",
                ));
            };
        let bounds = (x, y, w, h);
        finite(&[
            bounds.0, bounds.1, bounds.2, bounds.3, pos_x, pos_y, opacity,
        ])?;
        let asset = assets
            .by_id(id)
            .ok_or_else(|| RenderError::new(format!("unresolved SVG asset {id}")))?;
        let (bytes, sw, sh, mime) = match asset.kind {
            AssetKind::Image => {
                let Some((bytes, w, h)) = raster(&asset.bytes, crop)? else {
                    return Ok(None);
                };
                (bytes, w, h, "image/png")
            }
            AssetKind::Svg => {
                let (bytes, w, h) = normalize_svg(&asset.bytes, style, fonts)?;
                (bytes, w, h, "image/svg+xml")
            }
            AssetKind::Font | AssetKind::Unknown(_) => {
                return Err(RenderError::new(format!(
                    "unsupported SVG asset kind for {id}"
                )));
            }
        };
        let (x, y, w, h) = bounds;
        if w <= 0.0 || h <= 0.0 {
            return Ok(None);
        }
        let (rw, rh) = match fit {
            FitMode::Stretch => (w, h),
            FitMode::None => (sw, sh),
            FitMode::Contain => {
                let s = (w / sw).min(h / sh);
                (sw * s, sh * s)
            }
            FitMode::Cover => {
                let s = (w / sw).max(h / sh);
                (sw * s, sh * s)
            }
        };
        let (tx, ty) = (x + (w - rw) * pos_x / 100.0, y + (h - rh) * pos_y / 100.0);
        finite(&[rw, rh, tx, ty])?;
        let clip_path = match clip {
            None => geometry::rect(bounds)?,
            Some(ImageClip::Ellipse) => geometry::ellipse(bounds, None, None)?,
            Some(ImageClip::RoundedRect { radius }) => geometry::rounded(bounds, [*radius; 4])?,
        };
        let clip_id = self.clip(&clip_path, zenith_scene::FillRule::NonZero);
        self.body
            .push_str(&format!("<g clip-path=\"url(#{clip_id})\">"));
        self.body.push_str(&image_element(
            &bytes,
            mime,
            (tx, ty, rw, rh),
            opacity.clamp(0.0, 1.0),
        ));
        self.body.push_str("</g>");
        Ok(if mime == "image/svg+xml" {
            Some(SvgCapture {
                asset_id: id,
                intrinsic: (sw, sh),
                destination: (w, h),
            })
        } else {
            None
        })
    }
}
