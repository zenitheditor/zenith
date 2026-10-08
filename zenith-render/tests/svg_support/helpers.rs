use resvg::usvg::TreeParsing;
use zenith_render::RasterImage;

pub fn rasterize(bytes: &[u8]) -> RasterImage {
    let tree = resvg::usvg::Tree::from_data(bytes, &resvg::usvg::Options::default())
        .expect("SVG parses without external resources");
    let width = tree.size.width().ceil() as u32;
    let height = tree.size.height().ceil() as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height).expect("SVG dimensions");
    resvg::Tree::from_usvg(&tree).render(
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let rgba = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    RasterImage {
        width,
        height,
        rgba,
    }
}

pub fn assert_pixels_close(actual: &RasterImage, expected: &RasterImage, mean_limit: f64) {
    assert_eq!(
        (actual.width, actual.height),
        (expected.width, expected.height)
    );
    // Compare premultiplied channels so transparent edge colors do not dominate.
    let total: u64 = actual
        .rgba
        .chunks_exact(4)
        .zip(expected.rgba.chunks_exact(4))
        .map(|(a, b)| {
            let alpha = u64::from(a[3].abs_diff(b[3]));
            alpha
                + (0..3)
                    .map(|index| {
                        let ac = u16::from(a[index]) * u16::from(a[3]) / 255;
                        let bc = u16::from(b[index]) * u16::from(b[3]) / 255;
                        u64::from(ac.abs_diff(bc))
                    })
                    .sum::<u64>()
        })
        .sum();
    let mean = total as f64 / actual.rgba.len() as f64;
    assert!(
        mean <= mean_limit,
        "mean premultiplied channel difference {mean} exceeds {mean_limit}"
    );
}

pub fn data_url_bytes(svg: &[u8], mime: &str) -> Vec<u8> {
    let text = std::str::from_utf8(svg).expect("UTF-8 SVG");
    let prefix = format!("data:{mime};base64,");
    let start = text.find(&prefix).expect("embedded data URL") + prefix.len();
    let end = text[start..]
        .find(['\"', '\''])
        .expect("data URL terminator")
        + start;
    let mut buffer = 0_u32;
    let mut bits = 0_u32;
    let mut png = Vec::new();
    for byte in text[start..end].bytes().take_while(|byte| *byte != b'=') {
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("invalid base64 character"),
        };
        buffer = (buffer << 6) | u32::from(digit);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            png.push((buffer >> bits) as u8);
        }
    }
    png
}

pub fn embedded_png(svg: &[u8]) -> RasterImage {
    let png = data_url_bytes(svg, "image/png");
    let mut reader = png::Decoder::new(std::io::Cursor::new(png))
        .read_info()
        .expect("PNG header");
    let mut rgba = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut rgba).expect("PNG pixels");
    assert_eq!(info.color_type, png::ColorType::Rgba);
    rgba.truncate(info.buffer_size());
    RasterImage {
        width: info.width,
        height: info.height,
        rgba,
    }
}
