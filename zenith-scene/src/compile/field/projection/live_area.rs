//! Page live-area computation that mirrors the validator's margin formula.

/// Compute a page's live area `(x, y, w, h)` in AUTHORED coordinates, mirroring
/// the validator's `margin.rs` formula.
///
/// Returns `None` unless all four EFFECTIVE margins (`inner`/`outer`/`top`/
/// `bottom`) resolve to pixels — the same all-or-nothing gate the validator uses.
///
/// Each side's effective margin is the page's own value when set, else the
/// document-level default ([`zenith_core::Document::effective_margins`]) — the
/// single source of truth for the document→page margin cascade. With no document
/// margins set this reads exactly the page's own values, so the default-off path
/// is byte-identical.
///
/// LTR book — recto (odd, 1-based): `live_x = margin_inner`; verso (even) with
/// `mirror_margins`: `live_x = margin_outer`; otherwise `live_x = margin_inner`.
/// RTL book (`rtl == true`): the parity is MIRRORED — recto with `mirror_margins`
/// → `live_x = margin_outer` (binding on the right), verso → `live_x =
/// margin_inner`. `live_y = margin_top`, `live_w = page_w - inner - outer`,
/// `live_h = page_h - top - bottom`.
pub(in crate::compile) fn compute_live_area(
    doc: &zenith_core::Document,
    page: &zenith_core::Page,
    page_w: f64,
    page_h: f64,
    is_recto: bool,
    mirror_margins: bool,
    rtl: bool,
) -> Option<(f64, f64, f64, f64)> {
    use zenith_core::dim_to_px;
    let (inner_opt, outer_opt, top_opt, bottom_opt) = doc.effective_margins(page);
    let inner_dim = inner_opt.as_ref()?;
    let outer_dim = outer_opt.as_ref()?;
    let top_dim = top_opt.as_ref()?;
    let bottom_dim = bottom_opt.as_ref()?;

    let inner = dim_to_px(inner_dim.value, &inner_dim.unit)?;
    let outer = dim_to_px(outer_dim.value, &outer_dim.unit)?;
    let top = dim_to_px(top_dim.value, &top_dim.unit)?;
    let bottom = dim_to_px(bottom_dim.value, &bottom_dim.unit)?;

    // Inner (binding) is on the RIGHT for verso under LTR, and for recto under
    // RTL (the spread is mirrored). When it is on the right, OUTER insets the
    // left edge.
    let inner_on_right = if rtl { is_recto } else { !is_recto };
    let left_inset = if mirror_margins && inner_on_right {
        outer
    } else {
        inner
    };

    Some((
        left_inset,
        top,
        page_w - inner - outer,
        page_h - top - bottom,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::util::px;
    use zenith_core::Document;
    use zenith_core::Page;

    fn margined_page() -> Page {
        Page {
            id: "p".to_owned(),
            name: None,
            width: px(1200.0),
            height: px(1900.0),
            background: None,
            bleed: None,
            margin_inner: Some(px(160.0)),
            margin_outer: Some(px(100.0)),
            margin_top: Some(px(80.0)),
            margin_bottom: Some(px(80.0)),
            baseline_grid: None,
            line_jumps: None,
            source: None,
            fit: None,
            parity: None,
            master: None,
            safe_zones: Vec::new(),
            folds: Vec::new(),
            construction: zenith_core::ConstructionBlock::default(),
            ports: Vec::new(),
            block_styles: Vec::new(),
            children: Vec::new(),
            source_span: None,
        }
    }

    fn bare_page() -> Page {
        let mut p = margined_page();
        p.margin_inner = None;
        p.margin_outer = None;
        p.margin_top = None;
        p.margin_bottom = None;
        p
    }

    /// A document with no margins set — the default-off cascade reads page values
    /// verbatim, so `compute_live_area(&bare_doc(), page, …)` matches the
    /// pre-cascade behavior of reading `page.margin_*` directly.
    fn bare_doc() -> Document {
        use zenith_core::{KdlAdapter, KdlSource};
        // Parse the minimal valid document; all doc margins are None.
        KdlAdapter
            .parse(b"zenith version=1 { document id=\"d\" { } }")
            .expect("minimal test document must parse")
    }

    #[test]
    fn live_area_recto_uses_inner_margin() {
        // LTR recto (is_recto = true): left inset = margin_inner = 160.
        let la = compute_live_area(
            &bare_doc(),
            &margined_page(),
            1200.0,
            1900.0,
            true,
            true,
            false,
        );
        assert_eq!(la, Some((160.0, 80.0, 940.0, 1740.0)));
    }

    #[test]
    fn live_area_verso_mirrors_to_outer_margin() {
        // LTR verso (is_recto = false) with mirror: left inset = margin_outer = 100.
        let la = compute_live_area(
            &bare_doc(),
            &margined_page(),
            1200.0,
            1900.0,
            false,
            true,
            false,
        );
        assert_eq!(la, Some((100.0, 80.0, 940.0, 1740.0)));
    }

    #[test]
    fn live_area_unmirrored_verso_keeps_inner() {
        // Without mirroring, verso still uses inner as the left inset.
        let la = compute_live_area(
            &bare_doc(),
            &margined_page(),
            1200.0,
            1900.0,
            false,
            false,
            false,
        );
        assert_eq!(la, Some((160.0, 80.0, 940.0, 1740.0)));
    }

    #[test]
    fn live_area_rtl_recto_mirrors_to_outer_margin() {
        // RTL recto: binding on the RIGHT, so left inset = margin_outer = 100
        // (the mirror of the LTR recto). Width/top/bottom unchanged.
        let la = compute_live_area(
            &bare_doc(),
            &margined_page(),
            1200.0,
            1900.0,
            true,
            true,
            true,
        );
        assert_eq!(la, Some((100.0, 80.0, 940.0, 1740.0)));
    }

    #[test]
    fn live_area_rtl_verso_uses_inner_margin() {
        // RTL verso: binding on the LEFT, so left inset = margin_inner = 160.
        let la = compute_live_area(
            &bare_doc(),
            &margined_page(),
            1200.0,
            1900.0,
            false,
            true,
            true,
        );
        assert_eq!(la, Some((160.0, 80.0, 940.0, 1740.0)));
    }

    #[test]
    fn live_area_rtl_unmirrored_keeps_inner() {
        // Without mirroring, RTL recto still uses inner as the left inset.
        let la = compute_live_area(
            &bare_doc(),
            &margined_page(),
            1200.0,
            1900.0,
            true,
            false,
            true,
        );
        assert_eq!(la, Some((160.0, 80.0, 940.0, 1740.0)));
    }

    #[test]
    fn live_area_requires_all_four_margins() {
        let mut page = margined_page();
        page.margin_bottom = None;
        assert_eq!(
            compute_live_area(&bare_doc(), &page, 1200.0, 1900.0, true, true, false),
            None,
            "an incomplete margin set yields no live area"
        );
    }

    #[test]
    fn live_area_cascades_doc_margins_to_bare_page() {
        // Doc sets all four margins; the page declares none → the page inherits
        // the doc defaults and a live area is computed (LTR recto).
        let mut doc = bare_doc();
        doc.margin_inner = Some(px(160.0));
        doc.margin_outer = Some(px(100.0));
        doc.margin_top = Some(px(80.0));
        doc.margin_bottom = Some(px(80.0));
        let la = compute_live_area(&doc, &bare_page(), 1200.0, 1900.0, true, true, false);
        assert_eq!(la, Some((160.0, 80.0, 940.0, 1740.0)));
    }

    #[test]
    fn live_area_page_inner_overrides_doc_default() {
        // Doc sets all four; the page overrides only inner → page inner (200) +
        // doc outer/top/bottom. LTR recto uses inner as the left inset.
        let mut doc = bare_doc();
        doc.margin_inner = Some(px(160.0));
        doc.margin_outer = Some(px(100.0));
        doc.margin_top = Some(px(80.0));
        doc.margin_bottom = Some(px(80.0));
        let mut page = bare_page();
        page.margin_inner = Some(px(200.0));
        let la = compute_live_area(&doc, &page, 1200.0, 1900.0, true, true, false);
        // left inset = page inner = 200; width = 1200 - 200 - 100 = 900.
        assert_eq!(la, Some((200.0, 80.0, 900.0, 1740.0)));
    }
}
