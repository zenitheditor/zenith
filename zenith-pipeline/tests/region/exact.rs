//! A region render equals the same window cut from the full-page render,
//! byte for byte, on every page of every example. Its one-pass PNG equals
//! the PNG of its image.
//!
//! Exceptions: none. tiny-skia makes two surface-dependent choices (path
//! clipping and shader pixel centers). The region render runs such draws on a
//! scratch buffer that reproduces the full render (see the `zenith-render`
//! `region` docs). [`EXCEPTIONS`] stays empty: a new mismatch fails the test
//! instead of joining a list.
//!
//! Pages too large to render whole (no reference exists) skip the scratch
//! buffers. The render crate's unit tests bound that mode's difference.

use std::path::Path;

use zenith_pipeline::render::{PageSelection, compile_pages};
use zenith_pipeline::{FsConfig, Host, MemFs, PolicyFlags, RenderOptions, SourceFs};
use zenith_render::{
    DeviceRect, MAX_REGION_PIXELS, MAX_REGION_SIDE, RasterImage, encode_png, render_image_scaled,
    render_region_image, render_region_png,
};

use super::disk::{DiskFs, EXAMPLES, example_documents, mem_copy_of};

/// Output scales under test.
const SCALES: [f64; 9] = [0.25, 0.5, 0.8, 1.0, 1.5, 2.0, 3.0, 5.0, 8.0];

/// Documents allowed to differ: none.
const EXCEPTIONS: &[&str] = &[];

/// Windows of a `w x h` device page: origin, interior, far corner, edge
/// strips, a single pixel, an odd window, and the whole page.
fn windows(w: u32, h: u32) -> Vec<DeviceRect> {
    let rect = |x: u32, y: u32, rw: u32, rh: u32| {
        let x = x.min(w - 1);
        let y = y.min(h - 1);
        DeviceRect {
            x,
            y,
            width: rw.clamp(1, w - x),
            height: rh.clamp(1, h - y),
        }
    };
    vec![
        rect(0, 0, w / 2, h / 2),
        rect(w / 3, h / 4, w / 3, h / 3),
        rect(w - w / 3, h - h / 4, w / 3, h / 4),
        rect(w / 5, 0, w / 2, 3),
        rect(0, h / 3, 2, h / 2),
        rect(w / 2 + 1, h / 2 + 1, 1, 1),
        rect(17, 29, 101, 77),
        rect(0, 0, w, h),
    ]
}

/// `rect` cut from `full`.
fn cut(full: &RasterImage, rect: DeviceRect) -> Vec<u8> {
    let mut out = Vec::new();
    for row in rect.y..rect.y + rect.height {
        let start = ((row * full.width + rect.x) * 4) as usize;
        out.extend_from_slice(&full.rgba[start..start + (rect.width * 4) as usize]);
    }
    out
}

/// True when a full render of this size is a feasible reference.
fn feasible(image: &RasterImage) -> bool {
    image.width <= MAX_REGION_SIDE
        && image.height <= MAX_REGION_SIDE
        && u64::from(image.width) * u64::from(image.height) <= MAX_REGION_PIXELS
}

/// The examples, held in memory.
fn examples() -> MemFs {
    mem_copy_of(Path::new(EXAMPLES))
}

#[test]
fn every_example_region_equals_the_full_render() {
    let fs = examples();
    let config = FsConfig::new(&fs, None);
    let host = Host::new(&fs, &config);
    let flags = PolicyFlags::default();
    let opts = RenderOptions::new(&flags);
    let dir = Some(Path::new(EXAMPLES));
    let mut mismatches = Vec::new();
    let (mut cases, mut pages_seen) = (0usize, 0usize);
    for doc in example_documents() {
        let name = doc
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let src = std::fs::read_to_string(&doc).expect("read example");
        let compiled = compile_pages(host, &src, dir, PageSelection::All, opts, false)
            .unwrap_or_else(|e| panic!("{name} does not compile: {:?}", e.diagnostics));
        for page in &compiled.pages {
            pages_seen += 1;
            for scale in SCALES {
                let Ok(full) =
                    render_image_scaled(&page.scene, scale, &compiled.fonts, &compiled.assets)
                else {
                    continue;
                };
                if !feasible(&full) {
                    continue;
                }
                for rect in windows(full.width, full.height) {
                    cases += 1;
                    let part = render_region_image(
                        &page.scene,
                        scale,
                        rect,
                        &compiled.fonts,
                        &compiled.assets,
                    )
                    .unwrap_or_else(|e| panic!("{name} p{} s{scale} {rect:?}: {e}", page.page));
                    assert_eq!((part.width, part.height), (rect.width, rect.height));
                    // The one-pass PNG of a region equals encoding its image.
                    if scale == 0.5 || scale == 1.5 {
                        let png = render_region_png(
                            &page.scene,
                            scale,
                            rect,
                            &compiled.fonts,
                            &compiled.assets,
                        )
                        .unwrap_or_else(|e| panic!("{name} png {rect:?}: {e}"));
                        let encoded = encode_png(&part).expect("encode");
                        if png != encoded {
                            mismatches.push(format!(
                                "{name} page {} scale {scale} {rect:?}: region PNG differs from \
                                 the encoded region image",
                                page.page
                            ));
                        }
                    }
                    let want = cut(&full, rect);
                    let differ = part
                        .rgba
                        .chunks(4)
                        .zip(want.chunks(4))
                        .filter(|(a, b)| a != b)
                        .count();
                    if differ > 0 && !EXCEPTIONS.contains(&name.as_str()) {
                        mismatches.push(format!(
                            "{name} page {} scale {scale} {rect:?}: {differ} px differ",
                            page.page
                        ));
                    }
                }
            }
        }
    }
    assert!(pages_seen >= 35, "only {pages_seen} pages compiled");
    assert!(cases > 1000, "only {cases} cases ran");
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

/// The region pixels of `gradient.zen` page 1 at a fractional scale, through
/// `fs`.
fn gradient_region(fs: &dyn SourceFs) -> Vec<u8> {
    let config = FsConfig::new(fs, None);
    let host = Host::new(fs, &config);
    let flags = PolicyFlags::default();
    let opts = RenderOptions::new(&flags);
    let src = std::fs::read_to_string(Path::new(EXAMPLES).join("gradient.zen")).expect("read");
    let compiled = compile_pages(
        host,
        &src,
        Some(Path::new(EXAMPLES)),
        PageSelection::One(1),
        opts,
        false,
    )
    .expect("compile");
    let rect = DeviceRect {
        x: 211,
        y: 97,
        width: 333,
        height: 251,
    };
    render_region_image(
        &compiled.pages[0].scene,
        2.7,
        rect,
        &compiled.fonts,
        &compiled.assets,
    )
    .expect("region")
    .rgba
}

#[test]
fn region_renders_are_deterministic_across_hosts() {
    let memory = gradient_region(&examples());
    assert_eq!(memory, gradient_region(&examples()));
    assert_eq!(memory, gradient_region(&DiskFs));
}

/// A page as wide as the surface cap with a blur that reaches far past a
/// wide window: the pad is clamped to the page before the caps apply, so
/// the region still equals the full render (an earlier pad limit of
/// `(8191 - window) / 2` per side lost ink near the window edge).
#[test]
fn a_wide_window_on_a_cap_wide_page_keeps_the_full_blur_reach() {
    let src = r##"zenith version=1 {
  project id="proj.w" name="W"
  tokens format="zenith-token-v1" {
    token id="c.ink" type="color" value="#204080"
  }
  styles {}
  document id="doc.w" title="W" {
    page id="p" w=(px)8191 h=(px)2048 {
      rect id="left" x=(px)0 y=(px)400 w=(px)400 h=(px)1200 fill=(token)"c.ink" blur=(px)600
      rect id="right" x=(px)7800 y=(px)400 w=(px)391 h=(px)1200 fill=(token)"c.ink" blur=(px)600
    }
  }
}
"##;
    let fs = MemFs::new();
    let config = FsConfig::new(&fs, None);
    let host = Host::new(&fs, &config);
    let flags = PolicyFlags::default();
    let opts = RenderOptions::new(&flags);
    let compiled =
        compile_pages(host, src, None, PageSelection::All, opts, false).expect("compile");
    let page = &compiled.pages[0];
    let full = render_image_scaled(&page.scene, 1.0, &compiled.fonts, &compiled.assets)
        .expect("full render");
    assert!(feasible(&full));
    for rect in [
        DeviceRect {
            x: 1000,
            y: 0,
            width: 7000,
            height: 2048,
        },
        DeviceRect {
            x: 191,
            y: 100,
            width: 7000,
            height: 1800,
        },
    ] {
        let part = render_region_image(&page.scene, 1.0, rect, &compiled.fonts, &compiled.assets)
            .expect("region");
        let want = cut(&full, rect);
        let differ = part
            .rgba
            .chunks(4)
            .zip(want.chunks(4))
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(differ, 0, "{rect:?}: {differ} px differ");
    }
}

/// A 4096 px page of gradient ellipses: each crosses the window edge, so
/// each draw runs on a scratch buffer that starts at the page origin. The
/// windows equal the full render.
#[test]
fn gradient_draws_across_window_edges_equal_the_full_render() {
    let mut body = String::new();
    for i in 0..24u32 {
        let x = 300 + (i % 6) * 600;
        let y = 300 + (i / 6) * 900;
        body.push_str(&format!(
            "      ellipse id=\"e{i}\" x=(px){x} y=(px){y} w=(px)520 h=(px)700 fill=(token)\"g\" \
             rotate=(deg){}\n",
            i * 7
        ));
    }
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.g" name="G"
  tokens format="zenith-token-v1" {{
    token id="c.a" type="color" value="#1e3a8a"
    token id="c.b" type="color" value="#f97316"
    token id="g" type="gradient" angle=(deg)33 {{
      stop offset=0.0 color=(token)"c.a"
      stop offset=1.0 color=(token)"c.b"
    }}
  }}
  styles {{}}
  document id="doc.g" title="G" {{
    page id="p" w=(px)4096 h=(px)4096 {{
{body}    }}
  }}
}}
"##
    );
    let fs = MemFs::new();
    let config = FsConfig::new(&fs, None);
    let host = Host::new(&fs, &config);
    let flags = PolicyFlags::default();
    let opts = RenderOptions::new(&flags);
    let compiled =
        compile_pages(host, &src, None, PageSelection::All, opts, false).expect("compile");
    let page = &compiled.pages[0];
    let full = render_image_scaled(&page.scene, 1.0, &compiled.fonts, &compiled.assets)
        .expect("full render");
    for rect in [
        DeviceRect {
            x: 3300,
            y: 3200,
            width: 700,
            height: 800,
        },
        DeviceRect {
            x: 1111,
            y: 2222,
            width: 999,
            height: 777,
        },
        DeviceRect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        },
    ] {
        let part = render_region_image(&page.scene, 1.0, rect, &compiled.fonts, &compiled.assets)
            .expect("region");
        assert!(part.rgba == cut(&full, rect), "{rect:?} differs");
    }
}
