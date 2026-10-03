//! Rotated overflowing text clips to its rotated box, not to the box's
//! axis-aligned bounding box.

use zenith_cli::commands::render::to_png;

/// A 400×400 page. A 200×40 text box centered at (200, 200), rotated 30°,
/// holds far more text than fits, so the wrapped lines overflow below the box.
const ROTATED_OVERFLOW_DOC: &str = r##"zenith version=1 {
  project id="proj.rotclip" name="Rotated clip"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.ink" type="color" value="#000000"
    token id="font.body" type="fontFamily" value="Noto Sans"
    token id="size.body" type="dimension" value=(px)24
  }
  styles {}
  document id="doc.rotclip" title="Rotated clip" {
    page id="page.rotclip" w=(px)400 h=(px)400 background=(token)"color.bg" {
      text id="text.rot" x=(px)100 y=(px)180 w=(px)200 h=(px)40 rotate=(deg)30 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
        span "Overflowing rotated text keeps wrapping onto many more lines than the box can hold at this size"
      }
    }
  }
}
"##;

#[test]
fn rotated_overflowing_text_clips_to_its_rotated_box() {
    let artifact = to_png(ROTATED_OVERFLOW_DOC, 1).expect("render");
    let pm = tiny_skia::Pixmap::decode_png(&artifact.png).expect("decode png");
    let (w, h) = (pm.width(), pm.height());
    assert_eq!((w, h), (400, 400));
    let (sin, cos) = 30.0_f64.to_radians().sin_cos();
    let mut ink_inside = 0usize;
    for y in 0..h {
        for x in 0..w {
            let px = pm.pixel(x, y).expect("pixel");
            let is_bg = px.red() == 255 && px.green() == 255 && px.blue() == 255;
            // Box-local coordinates of the pixel center (inverse rotation
            // about the box center).
            let (dx, dy) = (f64::from(x) + 0.5 - 200.0, f64::from(y) + 0.5 - 200.0);
            let u = dx * cos + dy * sin;
            let v = -dx * sin + dy * cos;
            let margin = 1.5;
            let outside = u.abs() > 100.0 + margin || v.abs() > 20.0 + margin;
            if outside {
                assert!(
                    is_bg,
                    "ink outside the rotated box at ({x}, {y}), local ({u:.1}, {v:.1})"
                );
            } else if !is_bg {
                ink_inside += 1;
            }
        }
    }
    assert!(
        ink_inside > 50,
        "text draws inside the box ({ink_inside} ink px)"
    );
}
