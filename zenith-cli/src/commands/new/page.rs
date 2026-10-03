//! Page geometry for `zenith new`: named paper formats, explicit dimensions,
//! orientation, and page count.
//!
//! Dimensions are resolved to document pixels. Paper formats use the CSS
//! reference of 96 px per inch (millimetre sizes converted at 96/25.4 px/mm and
//! rounded to the nearest pixel), so `--format a4` yields the same physical
//! proportions a print workflow expects while staying in the `(px)` unit the
//! page node speaks natively.

use clap::ValueEnum;

/// A standard paper format selectable via `zenith new --format`.
///
/// Each variant resolves to a portrait pixel size at 96 dpi via
/// [`PaperFormat::portrait_px`]; `--landscape` swaps the axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PaperFormat {
    /// ISO A3 — 297×420 mm (1123×1587 px).
    A3,
    /// ISO A4 — 210×297 mm (794×1123 px).
    A4,
    /// ISO A5 — 148×210 mm (559×794 px).
    A5,
    /// ISO B4 — 250×353 mm (945×1334 px).
    B4,
    /// ISO B5 — 176×250 mm (665×945 px).
    B5,
    /// US Letter — 8.5×11 in (816×1056 px).
    Letter,
    /// US Legal — 8.5×14 in (816×1344 px).
    Legal,
    /// US Tabloid — 11×17 in (1056×1632 px).
    Tabloid,
    /// 1080×1080 square (social graphic) — the default page.
    Square,
}

impl PaperFormat {
    /// Portrait dimensions `(width, height)` in document pixels at 96 dpi.
    pub fn portrait_px(self) -> (u32, u32) {
        match self {
            Self::A3 => (1123, 1587),
            Self::A4 => (794, 1123),
            Self::A5 => (559, 794),
            Self::B4 => (945, 1334),
            Self::B5 => (665, 945),
            Self::Letter => (816, 1056),
            Self::Legal => (816, 1344),
            Self::Tabloid => (1056, 1632),
            Self::Square => (1080, 1080),
        }
    }

    /// `true` for a print paper format. `Square` is a screen canvas.
    pub fn is_print(self) -> bool {
        match self {
            Self::A3
            | Self::A4
            | Self::A5
            | Self::B4
            | Self::B5
            | Self::Letter
            | Self::Legal
            | Self::Tabloid => true,
            Self::Square => false,
        }
    }
}

/// Resolved page geometry for a scaffolded document: a per-page pixel size and a
/// page count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageSpec {
    /// Page width in document pixels.
    pub width: u32,
    /// Page height in document pixels.
    pub height: u32,
    /// Number of pages to scaffold (≥ 1).
    pub pages: u32,
    /// The `--format` the size came from, or `None` for no format. A print
    /// format turns on the theme print scale (`print_scale`).
    pub format: Option<PaperFormat>,
}

/// The default page: a single 1080×1080 square.
///
/// This is deliberately the pre-existing hard-coded size so that `zenith new`
/// with no geometry flags scaffolds a byte-identical document to before.
pub const DEFAULT_PAGE: PageSpec = PageSpec {
    width: 1080,
    height: 1080,
    pages: 1,
    format: None,
};

/// Resolve CLI geometry inputs into a concrete [`PageSpec`].
///
/// Precedence: a `--format` (or the 1080×1080 square default) sets the base
/// size; `--landscape` swaps its axes; an explicit `--width`/`--height` then
/// overrides that axis. With no inputs at all this returns [`DEFAULT_PAGE`].
///
/// Returns `Err` with a human-readable message when a resulting dimension or the
/// page count is zero. (Clap already range-limits these, so this is a
/// defence-in-depth guard for direct callers such as tests.)
pub fn resolve_page(
    format: Option<PaperFormat>,
    width: Option<u32>,
    height: Option<u32>,
    landscape: bool,
    pages: u32,
) -> Result<PageSpec, String> {
    let (base_w, base_h) = format.map_or((DEFAULT_PAGE.width, DEFAULT_PAGE.height), |f| {
        f.portrait_px()
    });
    let (base_w, base_h) = if landscape {
        (base_h, base_w)
    } else {
        (base_w, base_h)
    };

    let width = width.unwrap_or(base_w);
    let height = height.unwrap_or(base_h);

    if width == 0 || height == 0 {
        return Err(format!(
            "page dimensions must be positive; got {width}x{height}"
        ));
    }
    if pages == 0 {
        return Err("a document must have at least one page".to_string());
    }

    Ok(PageSpec {
        width,
        height,
        pages,
        format,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_inputs_yields_default_square() {
        assert_eq!(
            resolve_page(None, None, None, false, 1).unwrap(),
            DEFAULT_PAGE
        );
    }

    #[test]
    fn format_sets_portrait_dimensions() {
        let p = resolve_page(Some(PaperFormat::A4), None, None, false, 1).unwrap();
        assert_eq!((p.width, p.height), (794, 1123));
    }

    #[test]
    fn landscape_swaps_axes() {
        let p = resolve_page(Some(PaperFormat::A4), None, None, true, 1).unwrap();
        assert_eq!((p.width, p.height), (1123, 794));
    }

    #[test]
    fn explicit_dimensions_override_format_per_axis() {
        // Width overridden, height inherited from A4 portrait.
        let p = resolve_page(Some(PaperFormat::A4), Some(500), None, false, 1).unwrap();
        assert_eq!((p.width, p.height), (500, 1123));
    }

    #[test]
    fn explicit_dimensions_without_format() {
        let p = resolve_page(None, Some(640), Some(480), false, 2).unwrap();
        assert_eq!((p.width, p.height, p.pages), (640, 480, 2));
    }

    #[test]
    fn zero_dimension_is_rejected() {
        assert!(resolve_page(None, Some(0), Some(100), false, 1).is_err());
        assert!(resolve_page(None, Some(100), Some(0), false, 1).is_err());
    }

    #[test]
    fn zero_pages_is_rejected() {
        assert!(resolve_page(None, None, None, false, 0).is_err());
    }

    #[test]
    fn format_is_carried_on_the_spec() {
        let p = resolve_page(Some(PaperFormat::Letter), None, None, true, 1).unwrap();
        assert_eq!(p.format, Some(PaperFormat::Letter));
        let p = resolve_page(None, Some(640), Some(480), false, 1).unwrap();
        assert_eq!(p.format, None);
    }

    #[test]
    fn only_square_is_not_print() {
        assert!(!PaperFormat::Square.is_print());
        for f in [
            PaperFormat::A3,
            PaperFormat::A4,
            PaperFormat::A5,
            PaperFormat::B4,
            PaperFormat::B5,
            PaperFormat::Letter,
            PaperFormat::Legal,
            PaperFormat::Tabloid,
        ] {
            assert!(f.is_print(), "{f:?} is a print format");
        }
    }

    #[test]
    fn letter_is_816_by_1056() {
        assert_eq!(PaperFormat::Letter.portrait_px(), (816, 1056));
    }
}
