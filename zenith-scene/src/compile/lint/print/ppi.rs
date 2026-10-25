//! `image.effective_ppi` and `image.effective_ppi_target`: a raster image on
//! a print page has too few source pixels per placed inch.
//!
//! One inch is 96 px (`zenith_core::PX_PER_INCH`). `dim_to_px` reads `1pt` as 96/72 px.
//! The `zenith new --format` paper sizes use the same rule (A4 is 794 px wide).
//! The PDF backend writes 1 px as 0.75 pt, so a PDF viewer measures the same inch.
//! The placed size is the node box in page px, every ancestor instance fit
//! scale applied. A `src-*` crop counts only the cropped source region. The
//! fit mode sets the source-to-box scale:
//!
//! - `stretch` (the default): each axis on its own. The lower axis counts.
//! - `contain`: the smaller axis ratio. `cover`: the larger one.
//! - `none`: the source draws 1:1, so only an ancestor scale changes it.
//!
//! Below 150 ppi is the `image.effective_ppi` Warning. From 150 to under
//! 300 ppi is the `image.effective_ppi_target` Advisory. Each code keeps one
//! severity, so a policy can govern the two bands apart.
//! SVG assets are vector, so they are exempt. An image whose asset size is
//! unknown is skipped.
//!
//! Relation to `image.upscale`: that check reports a raster drawn larger
//! than its source pixels, which is under 96 ppi. On a print page it is
//! skipped, and this check reports the same image with its ppi number.

use zenith_core::{AssetKind, Diagnostic, PX_PER_INCH};

use super::super::super::boxes::CompiledBox;
use super::super::ledger::{Entry, ImageFacts};
use super::fmt::fmt_num;
use super::run::PrintInput;
use crate::ir::FitMode;
use crate::layout::LayoutBox;

/// Below this effective resolution an image is a Warning.
pub(super) const WARN_PPI: f64 = 150.0;
/// Below this effective resolution an image is an Advisory.
pub(super) const ADVISE_PPI: f64 = 300.0;

/// What one measurement found.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Measure {
    /// Effective ppi: the lower axis.
    ppi: f64,
    /// The source region in px (the crop, when set).
    source: (f64, f64),
    /// The placed box in page px.
    placed: (f64, f64),
}

/// Every `image.effective_ppi` and `image.effective_ppi_target` of the
/// page, in ledger order.
pub(super) fn effective_ppi(input: &PrintInput<'_, '_>) -> Vec<Diagnostic> {
    input
        .ledger
        .entries
        .iter()
        .filter_map(|entry| judge(input, entry))
        .collect()
}

fn judge(input: &PrintInput<'_, '_>, entry: &Entry) -> Option<Diagnostic> {
    let facts = entry.image.as_ref()?;
    if !entry.visible {
        return None;
    }
    let drawn = input.boxes.get(&entry.id).filter(|b| !b.hidden)?;
    if !is_raster(input, &facts.asset) {
        return None;
    }
    let intrinsic = *input.env.image_sizes.get(&facts.asset)?;
    let m = measure(facts, drawn, intrinsic)?;
    let warn = m.ppi < WARN_PPI;
    if !warn && m.ppi >= ADVISE_PPI {
        return None;
    }
    let (threshold, label) = if warn {
        (WARN_PPI, "floor")
    } else {
        (ADVISE_PPI, "target")
    };
    let message = message(&entry.id, facts, intrinsic, m, threshold, label);
    let span = entry.span;
    let subject = Some(entry.id.clone());
    Some(if warn {
        Diagnostic::warning("image.effective_ppi", message, span, subject)
    } else {
        Diagnostic::advisory("image.effective_ppi_target", message, span, subject)
    })
}

/// `true` when `asset` names a raster `image` asset of the host document
/// or of the import its `<import-id>/` prefix names.
fn is_raster(input: &PrintInput<'_, '_>, asset: &str) -> bool {
    let host = input.env.paint.assets.iter().find(|d| d.id == asset);
    let kind = match host {
        Some(decl) => Some(&decl.kind),
        None => asset.split_once('/').and_then(|(import, id)| {
            input
                .env
                .imports
                .get(import)
                .and_then(|scope| scope.document.assets.assets.iter().find(|d| d.id == id))
                .map(|d| &d.kind)
        }),
    };
    match kind {
        Some(AssetKind::Image) => true,
        Some(AssetKind::Svg | AssetKind::Font | AssetKind::Unknown(_)) | None => false,
    }
}

/// The effective resolution of an image drawn in `drawn` from a source of
/// `intrinsic` px. `None` for a degenerate box or source.
fn measure(facts: &ImageFacts, drawn: &CompiledBox, intrinsic: (f64, f64)) -> Option<Measure> {
    let world = drawn.world;
    let scale = (world.a.hypot(world.b), world.c.hypot(world.d));
    let placed = (drawn.local.w * scale.0, drawn.local.h * scale.1);
    let source = source_region(facts.src, intrinsic)?;
    let fit = match facts.fit.as_deref() {
        Some("contain") => FitMode::Contain,
        Some("cover") => FitMode::Cover,
        Some("none") => FitMode::None,
        Some(_) | None => FitMode::Stretch,
    };
    let ppi = ppi_of(fit, source, placed, scale)?;
    Some(Measure {
        ppi,
        source,
        placed,
    })
}

/// The source region in px: the crop clamped to the source, or the whole
/// source. `None` when it is empty.
fn source_region(crop: Option<LayoutBox>, intrinsic: (f64, f64)) -> Option<(f64, f64)> {
    let (iw, ih) = intrinsic;
    let (w, h) = match crop {
        Some(c) => (
            (c.x + c.w).min(iw) - c.x.max(0.0),
            (c.y + c.h).min(ih) - c.y.max(0.0),
        ),
        None => (iw, ih),
    };
    (w > 0.0 && h > 0.0 && w.is_finite() && h.is_finite()).then_some((w, h))
}

/// Effective ppi of a `source` px region drawn into a `placed` px box under
/// `fit`. `scale` is the ancestor scale, read only by `fit="none"`.
fn ppi_of(fit: FitMode, source: (f64, f64), placed: (f64, f64), scale: (f64, f64)) -> Option<f64> {
    let rx = placed.0 / source.0;
    let ry = placed.1 / source.1;
    let (kx, ky) = match fit {
        FitMode::Stretch => (rx, ry),
        FitMode::Contain => (rx.min(ry), rx.min(ry)),
        FitMode::Cover => (rx.max(ry), rx.max(ry)),
        FitMode::None => scale,
    };
    let k = kx.max(ky);
    (k > 0.0 && k.is_finite()).then(|| PX_PER_INCH / k)
}

fn message(
    id: &str,
    facts: &ImageFacts,
    intrinsic: (f64, f64),
    m: Measure,
    threshold: f64,
    label: &str,
) -> String {
    let px = |(w, h): (f64, f64)| format!("{}×{}", fmt_num(w.round()), fmt_num(h.round()));
    let inches = |(w, h): (f64, f64)| {
        format!(
            "{}×{} in",
            fmt_num(w / PX_PER_INCH),
            fmt_num(h / PX_PER_INCH)
        )
    };
    let source = match facts.src {
        Some(_) => format!("{} px crop of a {} px source", px(m.source), px(intrinsic)),
        None => format!("{} px source", px(m.source)),
    };
    let fit = facts.fit.as_deref().unwrap_or("stretch");
    // Raising the source or shrinking the box by `gain` meets the threshold.
    let gain = threshold / m.ppi;
    let need = ((intrinsic.0 * gain).ceil(), (intrinsic.1 * gain).ceil());
    let fits = (m.placed.0 / gain, m.placed.1 / gain);
    let upscaled = if m.ppi < PX_PER_INCH {
        " Under 96 ppi the source is also upscaled. On a print page this replaces image.upscale."
    } else {
        ""
    };
    format!(
        "image '{id}' prints at {} ppi < {} ppi {label} ({source} in a {} box, fit={fit}) — \
         supply a source of at least {} px, or shrink the box to {}.{upscaled}",
        fmt_num(m.ppi.floor()),
        fmt_num(threshold),
        inches(m.placed),
        px(need),
        inches(fits),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNIT: (f64, f64) = (1.0, 1.0);

    #[test]
    fn stretch_reads_the_lower_axis() {
        // 960 px over 960 px wide is 96 ppi. 960 px over 480 px tall is 192.
        let ppi = ppi_of(FitMode::Stretch, (960.0, 960.0), (960.0, 480.0), UNIT);
        assert_eq!(ppi, Some(96.0));
    }

    #[test]
    fn contain_and_cover_use_one_scale() {
        let src = (1000.0, 500.0);
        let placed = (500.0, 500.0);
        // contain: min(0.5, 1) = 0.5 → 192 ppi. cover: max = 1 → 96 ppi.
        assert_eq!(ppi_of(FitMode::Contain, src, placed, UNIT), Some(192.0));
        assert_eq!(ppi_of(FitMode::Cover, src, placed, UNIT), Some(96.0));
    }

    #[test]
    fn fit_none_reads_only_the_ancestor_scale() {
        let ppi = ppi_of(FitMode::None, (100.0, 100.0), (50.0, 50.0), (0.5, 0.5));
        assert_eq!(ppi, Some(192.0));
    }

    #[test]
    fn crop_uses_the_visible_source_region() {
        let crop = LayoutBox {
            x: 100.0,
            y: 0.0,
            w: 300.0,
            h: 200.0,
        };
        assert_eq!(
            source_region(Some(crop), (1200.0, 800.0)),
            Some((300.0, 200.0))
        );
        // A crop past the source edge is clamped.
        let past = LayoutBox {
            x: 1100.0,
            y: 0.0,
            w: 300.0,
            h: 200.0,
        };
        assert_eq!(
            source_region(Some(past), (1200.0, 800.0)),
            Some((100.0, 200.0))
        );
        let outside = LayoutBox { x: 1300.0, ..past };
        assert_eq!(source_region(Some(outside), (1200.0, 800.0)), None);
    }

    #[test]
    fn degenerate_box_has_no_ppi() {
        assert_eq!(
            ppi_of(FitMode::Stretch, (10.0, 10.0), (0.0, 0.0), UNIT),
            None
        );
    }
}
