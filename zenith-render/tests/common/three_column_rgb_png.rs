use std::sync::Arc;

/// Build a 3×3 PNG whose columns are pure RED (x=0), GREEN (x=1), BLUE (x=2).
///
/// Uses tiny-skia to compose the image rather than pulling in the `image`
/// crate or embedding a hand-crafted PNG byte string.
pub fn three_column_rgb_png() -> Arc<[u8]> {
    use tiny_skia::{Pixmap, PremultipliedColorU8};

    let mut pm = Pixmap::new(3, 3).expect("3x3 pixmap");
    // Fill each pixel using PremultipliedColorU8 (alpha=255 → premult == straight).
    let pixels = pm.pixels_mut();
    for row in 0..3_usize {
        for col in 0..3_usize {
            let idx = row * 3 + col;
            pixels[idx] = match col {
                0 => PremultipliedColorU8::from_rgba(255, 0, 0, 255).expect("red"),
                1 => PremultipliedColorU8::from_rgba(0, 255, 0, 255).expect("green"),
                _ => PremultipliedColorU8::from_rgba(0, 0, 255, 255).expect("blue"),
            };
        }
    }
    let png = pm.encode_png().expect("PNG encode must succeed");
    Arc::from(png.as_slice())
}
