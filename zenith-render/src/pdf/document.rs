//! Top-level PDF document assembly: page boxes, object-id allocation, resource
//! materialization, and the deterministic trailer.

use pdf_writer::types::{ActionType, AnnotationType, FunctionShadingType, OutputIntentSubtype};
use pdf_writer::{Filter, Finish, Pdf, Rect as PdfRect, Ref, Str, TextStr};
use zenith_core::{AssetProvider, FontProvider, PX_TO_PT};
use zenith_scene::Scene;

use super::content::{
    ALPHA_PREFIX, FONT_PREFIX, IMAGE_PREFIX, LinkAnnot, PageResources, SHADING_PREFIX, name,
    translate_strict, translate_with,
};
use super::font;
use super::gradient::{GradientGeometry, PdfGradient};

/// Options controlling PDF emission.
#[derive(Clone, Copy)]
pub struct PdfOptions {
    /// Subset embedded fonts to used glyphs (`true`, default) or embed whole fonts (`false`).
    /// Native text remains selectable and searchable. Rasterized text loses these capabilities.
    pub subset: bool,
    /// Emit all colors, gradients, and images in DeviceCMYK (`true`) instead of DeviceRGB (`false`).
    pub cmyk: bool,
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            subset: true,
            cmyk: false,
        }
    }
}

/// Render `scene` to a deterministic vector PDF (a single page).
///
/// `fonts` resolves glyph outlines for any `DrawGlyphRun`; `assets` resolves
/// raster bytes for any `DrawImage`. The output carries print box metadata
/// (MediaBox / TrimBox / BleedBox / CropBox) and native DeviceCMYK colors for
/// CMYK-origin tokens. Identical input yields byte-identical output: no
/// timestamps, no document id, ordered iteration throughout.
///
/// Capture errors retain compatibility vector emission, which can lose effect appearance.
/// Use [`super::render_pdf_report`] for strict capture and resource errors.
///
/// Mirrors the shape of [`crate::render_png`] (`scene`, `fonts`, `assets`).
///
/// This is a thin single-page wrapper over [`render_pdf_multi`]; the
/// one-scene path through that function yields byte-identical output to a
/// historical single-page implementation (catalog=1, pages=2, page=3,
/// content=4, resources from 5).
#[must_use]
pub fn render_pdf(scene: &Scene, fonts: &dyn FontProvider, assets: &dyn AssetProvider) -> Vec<u8> {
    render_pdf_multi(std::slice::from_ref(scene), fonts, assets)
}

/// Like [`render_pdf`] but with explicit [`PdfOptions`] (e.g. font subsetting).
#[must_use]
pub fn render_pdf_with(
    scene: &Scene,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    options: PdfOptions,
) -> Vec<u8> {
    render_pdf_multi_with(std::slice::from_ref(scene), fonts, assets, options)
}

/// Render `scenes` (one per document page, in order) to a single deterministic
/// multi-page vector PDF, sharing one sequential object-id space.
///
/// Object ids are allocated by an ordered walk: catalog=1, page-tree=2, then
/// for each scene in order its page dict, content stream, and resource objects
/// (ExtGStates, gradient shadings + functions, images + SMasks) from one shared
/// monotonic counter starting at 3. With a single scene this reproduces the
/// historical single-page numbering exactly, so additive multi-page support is
/// byte-identical when only one page is present.
///
/// Print box metadata, DeviceCMYK colors, and full determinism (no timestamps,
/// no document id, ordered iteration throughout) match [`render_pdf`].
#[must_use]
pub fn render_pdf_multi(
    scenes: &[Scene],
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
) -> Vec<u8> {
    render_pdf_multi_with(scenes, fonts, assets, PdfOptions::default())
}

/// Like [`render_pdf_multi`] but with explicit [`PdfOptions`].
///
/// Allocation order: `catalog=1`, `page_tree=2`, then a shared **font block**
/// (`REFS_PER_FONT` ids per embedded font), then per-page (page dict, content
/// stream, link-annotation dicts, then resource objects). A document with no
/// selectable text embeds no fonts and has no links, so the font block is empty
/// and the id stream + bytes are identical to the historical output (the
/// additive invariant).
#[must_use]
pub fn render_pdf_multi_with(
    scenes: &[Scene],
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    options: PdfOptions,
) -> Vec<u8> {
    assemble(scenes, fonts, assets, options, None, 1.0, None).unwrap_or_default()
}

/// Validate that `bytes` carries a standard ISO 15076-1 CMYK ICC profile header
/// (at least 128 bytes, `acsp` signature at offset 36, `CMYK` color space at offset 16).
fn validate_cmyk_icc(bytes: &[u8]) -> Result<(), crate::RenderError> {
    if bytes.len() < 128 {
        return Err(crate::RenderError::new(
            "invalid ICC profile: file is shorter than the 128-byte ICC header",
        ));
    }
    if bytes.get(36..40) != Some(b"acsp") {
        return Err(crate::RenderError::new(
            "invalid ICC profile: missing 'acsp' signature in header",
        ));
    }
    if bytes.get(16..20) != Some(b"CMYK") {
        return Err(crate::RenderError::new(
            "invalid ICC profile: expected a 4-channel CMYK color space profile",
        ));
    }
    Ok(())
}

pub(super) fn assemble(
    scenes: &[Scene],
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    options: PdfOptions,
    plans: Option<&[Vec<super::report::PlannedRegion>]>,
    raster_scale: f64,
    icc_profile: Option<&[u8]>,
) -> Result<Vec<u8>, crate::RenderError> {
    if options.cmyk
        && let Some(raw_icc) = icc_profile
    {
        validate_cmyk_icc(raw_icc)?;
    }

    let mut pdf = Pdf::new();

    let catalog_id = Ref::new(1);
    let page_tree_id = Ref::new(2);

    // Build the document-wide font plan from every selectable glyph run, then
    // reserve its object-id block immediately after the catalog + page tree.
    let usage = font::collect_usage(scenes);
    let font_plan = font::build_plan(&usage, fonts, options.subset);
    let font_base: i32 = 3;
    let mut next: i32 = font_base + (font_plan.fonts.len() as i32) * font::REFS_PER_FONT;
    let mut alloc = || {
        let r = Ref::new(next);
        next += 1;
        r
    };

    // Translate each scene and reserve all of its object ids in order, so the
    // page-tree's /Kids can list every page id before any page body is written.
    let mut pages: Vec<PreparedPage<'_>> = Vec::with_capacity(scenes.len());
    for (index, scene) in scenes.iter().enumerate() {
        let translated = if let Some(plans) = plans {
            let regions = plans
                .get(index)
                .ok_or_else(|| crate::RenderError::new("missing PDF page capture plan"))?;
            translate_strict(
                scene,
                fonts,
                assets,
                &font_plan,
                regions,
                raster_scale,
                options.cmyk,
            )
            .map_err(|error| crate::RenderError::new(format!("PDF page {}: {error}", index + 1)))?
        } else {
            translate_with(scene, fonts, assets, &font_plan, options.cmyk)
        };
        pages.push(prepare_page(scene, translated, &mut alloc));
    }

    let icc_id = if options.cmyk && icc_profile.is_some() {
        Some(alloc())
    } else {
        None
    };

    // ── Catalog + page tree ──────────────────────────────────────────────
    let mut catalog = pdf.catalog(catalog_id);
    catalog.pages(page_tree_id);
    if options.cmyk {
        let mut intents = catalog.output_intents();
        let mut intent = intents.push();
        intent.subtype(OutputIntentSubtype::PDFX);
        intent.output_condition(TextStr(
            "Offset printing, ISO 12647-2:2004/Amd 1, coated art paper",
        ));
        intent.output_condition_identifier(TextStr("FOGRA39"));
        intent.registry_name(TextStr("http://www.color.org"));
        intent.info(TextStr("ISO Coated v2 300% (ECI)"));
        if let Some(id) = icc_id {
            intent.dest_output_profile(id);
        }
        intent.finish();
        intents.finish();
    }
    catalog.finish();
    pdf.pages(page_tree_id)
        .kids(pages.iter().map(|p| p.page_id))
        .count(pages.len() as i32);

    // ── Embedded fonts (shared across pages) ─────────────────────────────
    for (idx, font) in font_plan.fonts.iter().enumerate() {
        let refs = font::font_refs_at(font_base, idx);
        font::write_font(&mut pdf, font, &refs);
    }

    // ── Per-page bodies ──────────────────────────────────────────────────
    for prepared in pages {
        write_prepared_page(&mut pdf, page_tree_id, prepared);
    }

    // ── Optional PDF/X OutputIntent ICC profile stream ───────────────────
    if let (Some(id), Some(raw_icc)) = (icc_id, icc_profile) {
        let flate = miniz_oxide::deflate::compress_to_vec_zlib(raw_icc, 6);
        pdf.icc_profile(id, &flate).n(4).filter(Filter::FlateDecode);
    }

    Ok(pdf.finish())
}

/// One scene translated to its content stream and resources, with every object
/// id already reserved from the shared allocator (so id ordering is fixed
/// before any object body is emitted).
struct PreparedPage<'a> {
    page_id: Ref,
    content_id: Ref,
    content: Vec<u8>,
    scene: &'a Scene,
    res: PageResources,
    /// One ref per link annotation, in `res.links` order.
    annot_ids: Vec<Ref>,
    alpha_ids: Vec<Ref>,
    gradient_refs: Vec<GradientRefs>,
    image_refs: Vec<ImageRefs>,
}

/// Translate one scene and reserve all of its object ids from `alloc` in the
/// fixed order: page dict, content stream, then resources (ExtGStates, gradient
/// shadings each with their function/subfunctions, images each with optional
/// SMask). Matches the historical single-page allocation order.
fn prepare_page<'a>(
    scene: &'a Scene,
    translated: (pdf_writer::Content, PageResources),
    alloc: &mut impl FnMut() -> Ref,
) -> PreparedPage<'a> {
    let page_id = alloc();
    let content_id = alloc();

    // Translate the scene to a content stream + the resources it references.
    let (content, res) = translated;

    // Reserve one ref per link annotation (in res.links order), before the
    // resource refs, so the page's /Annots array can list them.
    let annot_ids: Vec<Ref> = res.links.iter().map(|_| alloc()).collect();

    // Allocate refs for every resource up front so the page's resource dict can
    // reference them. Order is fixed: ExtGStates, then gradient shadings (each
    // with its function), then images (each with optional SMask).
    let alpha_ids: Vec<Ref> = res.alphas.iter().map(|_| alloc()).collect();
    let gradient_refs: Vec<GradientRefs> = res
        .gradients
        .iter()
        .map(|g| {
            let shading = alloc();
            let function = alloc();
            // A multi-stop gradient (> 2 stops) needs one exponential
            // subfunction per segment, stitched together. Allocate those refs
            // here so the whole document uses one clean sequential id space.
            let seg_count = g.stops.len().saturating_sub(1);
            let sub_functions = if g.stops.len() > 2 {
                (0..seg_count).map(|_| alloc()).collect()
            } else {
                Vec::new()
            };
            GradientRefs {
                shading,
                function,
                sub_functions,
            }
        })
        .collect();
    let image_refs: Vec<ImageRefs> = res
        .images
        .iter()
        .map(|img| ImageRefs {
            image: alloc(),
            smask: if img.alpha_flate.is_some() {
                Some(alloc())
            } else {
                None
            },
        })
        .collect();

    PreparedPage {
        page_id,
        content_id,
        content: content.finish().into_vec(),
        scene,
        res,
        annot_ids,
        alpha_ids,
        gradient_refs,
        image_refs,
    }
}

/// Emit one prepared page's object bodies (page dict, content stream, resource
/// objects) using its pre-reserved ids. `/Parent` of the page dict is the
/// shared page-tree id.
fn write_prepared_page(pdf: &mut Pdf, page_tree_id: Ref, prepared: PreparedPage<'_>) {
    let PreparedPage {
        page_id,
        content_id,
        content,
        scene,
        res,
        annot_ids,
        alpha_ids,
        gradient_refs,
        image_refs,
    } = prepared;

    // ── Page dict + boxes + resource dict ────────────────────────────────
    write_page(
        pdf,
        PageWrite {
            page_id,
            page_tree_id,
            content_id,
            scene,
            res: &res,
            annot_ids: &annot_ids,
            alpha_ids: &alpha_ids,
            gradient_refs: &gradient_refs,
            image_refs: &image_refs,
        },
    );

    // ── Content stream ───────────────────────────────────────────────────
    pdf.stream(content_id, &content);

    // ── Link annotations ─────────────────────────────────────────────────
    write_link_annotations(pdf, scene, &res.links, &annot_ids);

    // ── Resource objects ─────────────────────────────────────────────────
    write_alpha_states(pdf, &res, &alpha_ids);
    write_gradients(pdf, &res, &gradient_refs);
    write_images(pdf, &res, &image_refs);
}

/// Convert a scene px value to PDF points.
///
/// Page boxes and annotation rectangles sit outside the content stream.
/// The content stream `cm` does not reach them, so they scale here.
fn pt(px: f64) -> f32 {
    (px * PX_TO_PT) as f32
}

/// Convert a scene rect (top-left origin, y-down, px) to a PDF rect (bottom-left origin, y-up, pt).
fn scene_rect_pt(page_height: f64, x0: f64, y0: f64, x1: f64, y1: f64) -> PdfRect {
    PdfRect::new(pt(x0), pt(page_height - y1), pt(x1), pt(page_height - y0))
}

/// Write one `/Link` annotation per collected [`LinkAnnot`], converting the
/// scene-space rect (top-left origin, y-down, px) to PDF default user space
/// (bottom-left, y-up, pt) and attaching a URI action. Borders are suppressed so the link is
/// invisible (only the hit region is active), matching common web→PDF output.
fn write_link_annotations(pdf: &mut Pdf, scene: &Scene, links: &[LinkAnnot], annot_ids: &[Ref]) {
    for (link, id) in links.iter().zip(annot_ids) {
        let mut annot = pdf.annotation(*id);
        annot.subtype(AnnotationType::Link);
        annot.rect(scene_rect_pt(
            scene.height,
            link.x0,
            link.y0,
            link.x1,
            link.y1,
        ));
        // No visible border.
        annot.border(0.0, 0.0, 0.0, None);
        annot
            .action()
            .action_type(ActionType::Uri)
            .uri(Str(link.url.as_bytes()));
        annot.finish();
    }
}

/// Indirect references backing one native gradient: its shading dict and its
/// stitching/exponential color function.
struct GradientRefs {
    shading: Ref,
    function: Ref,
    /// One exponential subfunction ref per gradient segment, used only when the
    /// gradient has more than two stops (stitched via `function`).
    sub_functions: Vec<Ref>,
}

/// Indirect references backing one embedded image: the RGB image XObject and an
/// optional alpha SMask image XObject.
struct ImageRefs {
    image: Ref,
    smask: Option<Ref>,
}

/// Borrow/scalar context for [`write_page`], bundled into a `Copy` struct so
/// the function stays within the argument-count budget without an `#[allow]`.
/// `Ref` is `Copy` and the slice fields are shared borrows, so the whole struct
/// is `Copy`.
#[derive(Clone, Copy)]
struct PageWrite<'a> {
    page_id: Ref,
    page_tree_id: Ref,
    content_id: Ref,
    scene: &'a Scene,
    res: &'a PageResources,
    annot_ids: &'a [Ref],
    alpha_ids: &'a [Ref],
    gradient_refs: &'a [GradientRefs],
    image_refs: &'a [ImageRefs],
}

fn write_page(pdf: &mut Pdf, ctx: PageWrite<'_>) {
    let PageWrite {
        page_id,
        page_tree_id,
        content_id,
        scene,
        res,
        annot_ids,
        alpha_ids,
        gradient_refs,
        image_refs,
    } = ctx;
    // Page boxes are in points. The scene is in px, so each value scales by `PX_TO_PT`.
    let media = PdfRect::new(0.0, 0.0, pt(scene.width), pt(scene.height));

    let mut page = pdf.page(page_id);
    page.parent(page_tree_id);
    page.media_box(media);

    // Print boxes. When a trim box is present (bleed active), the trim rect is
    // converted from scene (top-left, y-down, px) coords to PDF (bottom-left, y-up, pt):
    // a scene rect [tx, ty, tw, th] becomes PDF [tx, H-(ty+th), tx+tw, H-ty] times `PX_TO_PT`.
    // BleedBox / CropBox = MediaBox (the canvas already includes the bleed).
    // With no trim, all four boxes equal the MediaBox.
    match scene.trim {
        Some(t) => {
            page.trim_box(scene_rect_pt(scene.height, t.x, t.y, t.x + t.w, t.y + t.h));
            page.bleed_box(media);
            page.crop_box(media);
        }
        None => {
            page.trim_box(media);
            page.bleed_box(media);
            page.crop_box(media);
        }
    }

    page.contents(content_id);

    // Link annotations (clickable hyperlinks). Absent → no /Annots key, so a
    // page without links is byte-identical to the historical output.
    if !annot_ids.is_empty() {
        page.annotations(annot_ids.iter().copied());
    }

    // Resource dictionary referencing every interned resource by its stable
    // `<prefix><index>` name.
    let mut resources = page.resources();
    if !res.font_indices.is_empty() {
        let mut fonts = resources.fonts();
        for &idx in &res.font_indices {
            let nm = name(FONT_PREFIX, idx);
            fonts.pair(nm.as_name(), font::font_refs_at(3, idx).type0_ref());
        }
        fonts.finish();
    }
    if !res.alphas.is_empty() {
        let mut gs = resources.ext_g_states();
        for (i, r) in alpha_ids.iter().enumerate() {
            let nm = name(ALPHA_PREFIX, i);
            gs.pair(nm.as_name(), *r);
        }
        gs.finish();
    }
    if !res.gradients.is_empty() {
        let mut sh = resources.shadings();
        for (i, gr) in gradient_refs.iter().enumerate() {
            let nm = name(SHADING_PREFIX, i);
            sh.pair(nm.as_name(), gr.shading);
        }
        sh.finish();
    }
    if !res.images.is_empty() {
        let mut xo = resources.x_objects();
        for (i, ir) in image_refs.iter().enumerate() {
            let nm = name(IMAGE_PREFIX, i);
            xo.pair(nm.as_name(), ir.image);
        }
        xo.finish();
    }
    resources.finish();
    page.finish();
}

/// Write one `/ExtGState` per interned alpha, carrying both `ca` (fill) and
/// `CA` (stroke) so a single state serves filled and stroked draws.
fn write_alpha_states(pdf: &mut Pdf, res: &PageResources, alpha_ids: &[Ref]) {
    for (a, r) in res.alphas.iter().zip(alpha_ids) {
        let factor = f32::from(*a) / 255.0;
        let mut gs = pdf.ext_graphics(*r);
        gs.non_stroking_alpha(factor);
        gs.stroking_alpha(factor);
        gs.finish();
    }
}

/// Write native axial or radial shading with DeviceRGB or DeviceCMYK color functions.
fn write_gradients(pdf: &mut Pdf, res: &PageResources, refs: &[GradientRefs]) {
    for (g, gr) in res.gradients.iter().zip(refs) {
        write_gradient_function(pdf, gr, g, res.cmyk);

        let mut shading = pdf.function_shading(gr.shading);
        match g.geometry {
            GradientGeometry::Axial(coords) => {
                shading.shading_type(FunctionShadingType::Axial);
                if res.cmyk {
                    shading.color_space().device_cmyk();
                } else {
                    shading.color_space().device_rgb();
                }
                shading.coords(coords);
            }
            GradientGeometry::Radial(coords) => {
                shading.shading_type(FunctionShadingType::Radial);
                if res.cmyk {
                    shading.color_space().device_cmyk();
                } else {
                    shading.color_space().device_rgb();
                }
                shading.coords(coords);
            }
        }
        shading.function(gr.function);
        // Clamp (don't extend) beyond the endpoints so the shading fills the
        // clipped shape with the edge colors, matching CSS `Pad` spread.
        shading.extend([true, true]);
        shading.finish();
    }
}

/// Write the color function for `g`. With exactly two stops a single Type 2
/// exponential (linear) function is emitted at `gr.function`; with more stops a
/// Type 3 stitching function at `gr.function` combines one exponential
/// subfunction per segment (refs in `gr.sub_functions`).
fn write_gradient_function(pdf: &mut Pdf, gr: &GradientRefs, g: &PdfGradient, cmyk: bool) {
    // Two-stop (or defensively fewer): a single linear exponential function.
    if g.stops.len() <= 2 {
        let c0 = g.stops.first().map(|s| s.1).unwrap_or([0.0, 0.0, 0.0]);
        let c1 = g.stops.get(1).map(|s| s.1).unwrap_or(c0);
        write_linear_segment(pdf, gr.function, c0, c1, cmyk);
        return;
    }

    // > 2 stops: one linear exponential per segment, stitched together.
    for (k, sub) in gr.sub_functions.iter().enumerate() {
        let c0 = g.stops.get(k).map(|s| s.1).unwrap_or([0.0, 0.0, 0.0]);
        let c1 = g.stops.get(k + 1).map(|s| s.1).unwrap_or(c0);
        write_linear_segment(pdf, *sub, c0, c1, cmyk);
    }

    // Interior stop offsets become the stitching bounds; each subfunction's
    // input is encoded over [0, 1].
    let last = g.stops.len() - 1;
    let bounds: Vec<f32> = g
        .stops
        .get(1..last)
        .unwrap_or(&[])
        .iter()
        .map(|s| s.0)
        .collect();
    let mut encode: Vec<f32> = Vec::with_capacity(gr.sub_functions.len() * 2);
    for _ in &gr.sub_functions {
        encode.push(0.0);
        encode.push(1.0);
    }

    let mut stitch = pdf.stitching_function(gr.function);
    stitch.domain([0.0, 1.0]);
    if cmyk {
        stitch.range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
    } else {
        stitch.range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
    }
    stitch.functions(gr.sub_functions.iter().copied());
    stitch.bounds(bounds);
    stitch.encode(encode);
    stitch.finish();
}

/// Write a single Type 2 (exponential, `N = 1` linear) function in DeviceRGB or
/// DeviceCMYK mapping `[0, 1]` from color `c0` to `c1`.
fn write_linear_segment(pdf: &mut Pdf, id: Ref, c0: [f32; 3], c1: [f32; 3], cmyk: bool) {
    let mut f = pdf.exponential_function(id);
    f.domain([0.0, 1.0]);
    if cmyk {
        f.range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
        f.c0(super::color::rgb_to_cmyk(c0[0], c0[1], c0[2]));
        f.c1(super::color::rgb_to_cmyk(c1[0], c1[1], c1[2]));
    } else {
        f.range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
        f.c0(c0);
        f.c1(c1);
    }
    f.n(1.0);
    f.finish();
}

/// Write each image as a FlateDecode DeviceRGB or DeviceCMYK XObject, with an
/// optional FlateDecode DeviceGray SMask for transparency.
fn write_images(pdf: &mut Pdf, res: &PageResources, refs: &[ImageRefs]) {
    for (img, ir) in res.images.iter().zip(refs) {
        let w = img.width as i32;
        let h = img.height as i32;

        let mut xobj = pdf.image_xobject(ir.image, &img.rgb_flate);
        xobj.filter(Filter::FlateDecode);
        xobj.width(w);
        xobj.height(h);
        if img.cmyk {
            xobj.color_space().device_cmyk();
        } else {
            xobj.color_space().device_rgb();
        }
        xobj.bits_per_component(8);
        if let Some(smask) = ir.smask {
            xobj.s_mask(smask);
        }
        xobj.finish();

        if let (Some(smask), Some(alpha)) = (ir.smask, &img.alpha_flate) {
            let mut sm = pdf.image_xobject(smask, alpha);
            sm.filter(Filter::FlateDecode);
            sm.width(w);
            sm.height(h);
            sm.color_space().device_gray();
            sm.bits_per_component(8);
            sm.finish();
        }
    }
}
