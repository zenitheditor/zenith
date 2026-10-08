//! Shared registered-font SVG parsing and placement transforms.
use super::svg::SvgPlacement;
use resvg::usvg::tiny_skia_path::Point;
use resvg::usvg::{self, TreeParsing, TreeTextToPath};
use zenith_core::FontProvider;
use zenith_scene::{FitMode, SvgStyle};

pub(super) fn font_database(fonts: &dyn FontProvider) -> usvg::fontdb::Database {
    let mut database = usvg::fontdb::Database::new();
    database.set_sans_serif_family("Noto Sans");
    database.set_serif_family("Noto Sans");
    database.set_monospace_family("Noto Sans Mono");
    for face in fonts.all_faces() {
        database.load_font_data(face.bytes.to_vec());
    }
    database
}

pub(super) fn parse(
    bytes: &[u8],
    style: Option<SvgStyle>,
    database: &usvg::fontdb::Database,
) -> Option<usvg::Tree> {
    let options = usvg::Options {
        font_family: "Noto Sans".to_owned(),
        ..Default::default()
    };
    let styled = crate::svg_style::styled_svg_bytes(bytes, style);
    let mut tree = usvg::Tree::from_data(&styled, &options).ok()?;
    tree.convert_text(database);
    Some(tree)
}

/// A 2-D affine map `(x, y) → (a·x + c·y + e, b·x + d·y + f)`, in scene units.
#[derive(Clone, Copy)]
pub(super) struct Affine {
    pub(super) a: f64,
    pub(super) b: f64,
    pub(super) c: f64,
    pub(super) d: f64,
    e: f64,
    f: f64,
}

impl Affine {
    /// A pure scale + translate (no rotation/skew).
    pub(super) fn scale_translate(sx: f64, sy: f64, tx: f64, ty: f64) -> Self {
        Affine {
            a: sx,
            b: 0.0,
            c: 0.0,
            d: sy,
            e: tx,
            f: ty,
        }
    }

    /// Convert a `usvg`/`tiny_skia` transform to an [`Affine`].
    pub(super) fn from_usvg(t: usvg::Transform) -> Self {
        Affine {
            a: f64::from(t.sx),
            b: f64::from(t.ky),
            c: f64::from(t.kx),
            d: f64::from(t.sy),
            e: f64::from(t.tx),
            f: f64::from(t.ty),
        }
    }

    /// `self ∘ inner`: apply `inner` first, then `self`.
    pub(super) fn then(self, inner: Affine) -> Affine {
        Affine {
            a: self.a * inner.a + self.c * inner.b,
            b: self.b * inner.a + self.d * inner.b,
            c: self.a * inner.c + self.c * inner.d,
            d: self.b * inner.c + self.d * inner.d,
            e: self.a * inner.e + self.c * inner.f + self.e,
            f: self.b * inner.e + self.d * inner.f + self.f,
        }
    }

    /// Map a point.
    pub(super) fn map(self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    /// Map a `tiny_skia` point.
    pub(super) fn map_pt(self, p: Point) -> (f64, f64) {
        self.map(f64::from(p.x), f64::from(p.y))
    }

    pub(super) fn conformal(self) -> bool {
        let u = self.a * self.a + self.b * self.b;
        let v = self.c * self.c + self.d * self.d;
        let tolerance = u.max(v) * 1e-6;
        u.is_finite()
            && v.is_finite()
            && u > 0.0
            && v > 0.0
            && (u - v).abs() <= tolerance
            && (self.a * self.c + self.b * self.d).abs() <= tolerance
    }

    /// Average linear scale factor `√|det|`, used to scale stroke widths.
    pub(super) fn avg_scale(self) -> f64 {
        (self.a * self.d - self.b * self.c).abs().sqrt()
    }
}

pub(super) fn placement_transform(tree: &usvg::Tree, place: SvgPlacement<'_>) -> Option<Affine> {
    let SvgPlacement {
        x,
        y,
        w,
        h,
        fit,
        pos_x,
        pos_y,
        ..
    } = place;
    let (svw, svh) = (f64::from(tree.size.width()), f64::from(tree.size.height()));
    if !(svw > 0.0 && svh > 0.0) {
        return None;
    }

    // Fit transform: SVG viewBox box [0,0,svw,svh] → placement box, preserving
    // aspect per `fit` and `object-position`. Identical math to `emit_image`.
    let (sx, sy, tx, ty) = match fit {
        FitMode::Stretch => (w / svw, h / svh, x, y),
        FitMode::Contain => {
            let s = (w / svw).min(h / svh);
            (
                s,
                s,
                x + (w - svw * s) * pos_x / 100.0,
                y + (h - svh * s) * pos_y / 100.0,
            )
        }
        FitMode::Cover => {
            let s = (w / svw).max(h / svh);
            (
                s,
                s,
                x - (svw * s - w) * pos_x / 100.0,
                y - (svh * s - h) * pos_y / 100.0,
            )
        }
        FitMode::None => (
            1.0,
            1.0,
            x - (svw - w) * pos_x / 100.0,
            y - (svh - h) * pos_y / 100.0,
        ),
    };
    if !(sx.is_finite()
        && sy.is_finite()
        && tx.is_finite()
        && ty.is_finite()
        && sx > 0.0
        && sy > 0.0)
    {
        return None;
    }
    Some(Affine::scale_translate(sx, sy, tx, ty))
}
