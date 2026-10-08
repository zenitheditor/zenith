use miniz_oxide::inflate::decompress_to_vec_zlib;
use zenith_render::RasterImage;

pub fn assert_image_planes(pdf: &[u8], raster: &RasterImage) {
    let mut rgb = None;
    let mut alpha = None;
    let marker = b"/Subtype /Image";
    for (index, _) in pdf
        .windows(marker.len())
        .enumerate()
        .filter(|(_, window)| *window == marker)
    {
        let dict_start = pdf[..index].windows(2).rposition(|w| w == b"<<").unwrap();
        let tail = &pdf[dict_start..];
        let start = tail.windows(7).position(|w| w == b"stream\n").unwrap();
        let dict = std::str::from_utf8(&tail[..start]).unwrap();
        let length: usize = dict
            .split("/Length ")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let width: usize = dict
            .split("/Width ")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let height: usize = dict
            .split("/Height ")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let data = decompress_to_vec_zlib(&tail[start + 7..start + 7 + length]).unwrap();
        if dict.contains("/DeviceRGB") {
            rgb = Some((width, height, data));
        } else if dict.contains("/DeviceGray") {
            alpha = Some(data);
        }
    }
    let pixels: Vec<_> = raster
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(_, p)| p[3] > 0)
        .collect();
    let min_x = pixels
        .iter()
        .map(|(i, _)| i % raster.width as usize)
        .min()
        .unwrap();
    let max_x = pixels
        .iter()
        .map(|(i, _)| i % raster.width as usize)
        .max()
        .unwrap();
    let min_y = pixels
        .iter()
        .map(|(i, _)| i / raster.width as usize)
        .min()
        .unwrap();
    let max_y = pixels
        .iter()
        .map(|(i, _)| i / raster.width as usize)
        .max()
        .unwrap();
    let mut expected_rgb = Vec::new();
    let mut expected_alpha = Vec::new();
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let pixel = &raster.rgba[(y * raster.width as usize + x) * 4..][..4];
            expected_rgb.extend_from_slice(&pixel[..3]);
            expected_alpha.push(pixel[3]);
        }
    }
    let (width, height, rgb) = rgb.unwrap();
    assert_eq!((width, height), (max_x - min_x + 1, max_y - min_y + 1));
    assert_eq!(rgb, expected_rgb);
    assert_eq!(
        alpha.unwrap_or_else(|| vec![255; width * height]),
        expected_alpha
    );
}
