//! Intrinsic pixel size of image and SVG assets.
//!
//! Auto-layout sizes a hugging `image` from its asset's pixel size. Scene
//! compilation reads no bytes, so the caller reads the sizes here, from the
//! same decoders the raster backend draws with, and passes them to
//! `zenith_scene::DocumentPrep::with_image_sizes`.

use std::collections::BTreeMap;

use resvg::usvg::{self, TreeParsing};
use zenith_core::{AssetKind, AssetProvider};

/// The pixel size `(w, h)` of an asset, from its header (raster) or its
/// root size (SVG).
///
/// Raster images support PNG and JPEG, the formats the backend draws.
/// `None` for another format, malformed bytes, a font asset, or a zero size.
#[must_use]
pub fn asset_intrinsic_size(kind: &AssetKind, bytes: &[u8]) -> Option<(f64, f64)> {
    let (w, h) = match kind {
        AssetKind::Image => raster_size(bytes)?,
        AssetKind::Svg => {
            let tree = usvg::Tree::from_data(bytes, &usvg::Options::default()).ok()?;
            (f64::from(tree.size.width()), f64::from(tree.size.height()))
        }
        AssetKind::Font | AssetKind::Unknown(_) => return None,
    };
    (w > 0.0 && h > 0.0).then_some((w, h))
}

/// [`asset_intrinsic_size`] of every id in `ids` that `assets` resolves, by
/// id.
#[must_use]
pub fn asset_intrinsic_sizes<'a>(
    assets: &dyn AssetProvider,
    ids: impl IntoIterator<Item = &'a str>,
) -> BTreeMap<String, (f64, f64)> {
    let mut sizes = BTreeMap::new();
    for id in ids {
        if let Some(asset) = assets.by_id(id)
            && let Some(size) = asset_intrinsic_size(&asset.kind, &asset.bytes)
        {
            sizes.insert(id.to_owned(), size);
        }
    }
    sizes
}

/// The pixel size of a PNG or JPEG from its header.
fn raster_size(bytes: &[u8]) -> Option<(f64, f64)> {
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        let reader = png::Decoder::new(bytes).read_info().ok()?;
        let info = reader.info();
        return Some((f64::from(info.width), f64::from(info.height)));
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        let mut decoder = jpeg_decoder::Decoder::new(bytes);
        decoder.read_info().ok()?;
        let info = decoder.info()?;
        return Some((f64::from(info.width), f64::from(info.height)));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG: &[u8] =
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20"/></svg>"#;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, w, h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut writer = enc.write_header().expect("header");
            writer
                .write_image_data(&vec![0u8; (w * h * 4) as usize])
                .expect("data");
        }
        out
    }

    #[test]
    fn png_size_from_header() {
        assert_eq!(
            asset_intrinsic_size(&AssetKind::Image, &png(30, 12)),
            Some((30.0, 12.0))
        );
    }

    #[test]
    fn svg_size_from_root() {
        assert_eq!(
            asset_intrinsic_size(&AssetKind::Svg, SVG),
            Some((40.0, 20.0))
        );
    }

    #[test]
    fn unknown_bytes_and_fonts_have_no_size() {
        assert_eq!(asset_intrinsic_size(&AssetKind::Image, b"GIF89a"), None);
        assert_eq!(asset_intrinsic_size(&AssetKind::Font, SVG), None);
    }
}
