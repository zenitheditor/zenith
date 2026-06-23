//! The `tiny-skia` raster backend: the [`RasterBackend`] implementation and the
//! command-dispatch render loop.
//!
//! The loop owns the clip / transform stacks, the effect-capture stack, and the
//! compositing-layer stack. Structural and capture commands (clip, transform,
//! layer, and the blur/shadow/filter/mask brackets) are handled inline here —
//! they mutate those stacks and `continue`. Drawing commands resolve the active
//! target pixmap and delegate to [`draw_command`](super::commands::draw_command),
//! which routes each to a handler in the [`draw`](super::draw) submodules.

use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};
use zenith_core::{AssetProvider, FontProvider};
use zenith_scene::{
    BlendMode as IrBlendMode, FilterSpec, MaskSpec, Scene, SceneCommand, ShadowSpec,
};

use super::commands::{DrawCtx, draw_command};
use super::filter::apply_filters;
use super::mask::attenuate_by_mask;
use super::paths::intersect_rects;
use super::pixels::{f64_to_px, premultiplied_to_straight};
use super::shadow::{composite_shadows, gaussian_blur_premul};
use crate::backend::{RasterBackend, RasterImage};
use crate::error::RenderError;

// ── TinySkiaBackend ───────────────────────────────────────────────────────────

/// CPU rasterization backend backed by the `tiny-skia` library.
///
/// Determinism guarantees:
/// - Anti-aliasing is disabled for rect fills → integer-aligned rects produce
///   exact, reproducible pixels with no sub-pixel variance.
/// - Anti-aliasing is enabled for glyph fills — glyph edges are curved and
///   require AA for legible output. tiny-skia AA is pure-software and
///   deterministic on the same machine (no GPU, no random numbers).
/// - No `HashMap`, no random numbers, no timestamps.
/// - PNG encoding via `tiny_skia::Pixmap::encode_png` writes no timestamps.
pub struct TinySkiaBackend;

/// Map a scene-IR [`IrBlendMode`] to the `tiny_skia::BlendMode` used when a
/// compositing layer is painted back onto its parent.
///
/// `None` and `Some(Normal)` both yield `SourceOver` — plain compositing — so a
/// layer with no blend (or an explicit `normal`) composites byte-identically to
/// having no layer at all. Every other variant maps to the tiny-skia operator of
/// the same name. Exhaustive over `IrBlendMode`.
fn map_blend_mode(b: Option<IrBlendMode>) -> tiny_skia::BlendMode {
    use tiny_skia::BlendMode as Tk;
    match b {
        None | Some(IrBlendMode::Normal) => Tk::SourceOver,
        Some(IrBlendMode::Multiply) => Tk::Multiply,
        Some(IrBlendMode::Screen) => Tk::Screen,
        Some(IrBlendMode::Overlay) => Tk::Overlay,
        Some(IrBlendMode::Darken) => Tk::Darken,
        Some(IrBlendMode::Lighten) => Tk::Lighten,
        Some(IrBlendMode::ColorDodge) => Tk::ColorDodge,
        Some(IrBlendMode::ColorBurn) => Tk::ColorBurn,
        Some(IrBlendMode::HardLight) => Tk::HardLight,
        Some(IrBlendMode::SoftLight) => Tk::SoftLight,
        Some(IrBlendMode::Difference) => Tk::Difference,
        Some(IrBlendMode::Exclusion) => Tk::Exclusion,
    }
}

// The effect type associated with an active offscreen capture. Either a
// shadow (blurred shadow layers composited behind the crisp ink) or a
// Gaussian blur (the ink itself blurred in place) or a color filter.
enum CaptureEffect {
    Shadow(Vec<ShadowSpec>),
    Blur(f64),
    Filter(Vec<FilterSpec>),
    Mask(MaskSpec),
}

// One entry of the effect-capture stack. Effect captures (blur/shadow/
// filter) nest: each Begin* pushes a layer, each End* pops it and
// composites the captured ink onto the target below.
struct CaptureLayer {
    /// The offscreen ink buffer. `None` when allocation failed — draws
    /// fall through to the target below and the matching End* skips
    /// compositing (keeps Begin*/End* balanced so nesting stays correct).
    pm: Option<Pixmap>,
    effect: CaptureEffect,
}

// Resolve the current draw / composite target, innermost-first: the
// topmost effect-capture entry that holds a buffer, else the top blend
// layer, else the base canvas. With an empty `capture_stack` this is
// exactly the old `layer_stack.last() else base` target.
fn current_target<'a>(
    capture_stack: &'a mut [CaptureLayer],
    layer_stack: &'a mut [(Pixmap, f32, tiny_skia::BlendMode)],
    base: &'a mut Pixmap,
) -> &'a mut Pixmap {
    if let Some(layer) = capture_stack.iter_mut().rev().find(|l| l.pm.is_some()) {
        // safe: just checked is_some
        if let Some(pm) = layer.pm.as_mut() {
            return pm;
        }
    }
    if let Some((pm, _, _)) = layer_stack.last_mut() {
        return pm;
    }
    base
}

impl RasterBackend for TinySkiaBackend {
    fn rasterize(
        &self,
        scene: &Scene,
        fonts: &dyn FontProvider,
        assets: &dyn AssetProvider,
    ) -> Result<RasterImage, RenderError> {
        let width = f64_to_px(scene.width, "width")?;
        let height = f64_to_px(scene.height, "height")?;

        let mut pixmap = Pixmap::new(width, height).ok_or_else(|| {
            RenderError::new(format!("failed to allocate pixmap ({width}×{height})"))
        })?;
        // Background starts fully transparent (0,0,0,0) — the deterministic default.

        // Clip stack: each entry is (x, y, x2, y2) in scene coordinates.
        // The outermost clip is the page rectangle.
        let page_clip = (0.0_f64, 0.0_f64, scene.width, scene.height);
        let mut clip_stack: Vec<(f64, f64, f64, f64)> = vec![page_clip];

        // Transform stack: the top entry is the current affine transform applied
        // to every draw. The base entry is identity, so unrotated scenes pass
        // `Transform::identity()` to every draw call (byte-identical to before).
        let mut transform_stack: Vec<Transform> = vec![Transform::identity()];

        // Lazily-built fontdb for SVG text→path conversion. Initialised at most
        // once per render, only when an SVG asset is actually drawn. Never loads
        // system fonts — only the registered faces from `fonts`.
        let mut svg_fontdb: Option<resvg::usvg::fontdb::Database> = None;

        // The effect-capture stack. The innermost active capture (topmost entry
        // with `Some(pm)`) is the current draw target; an empty stack means
        // draws target the top blend layer or the real canvas — byte-identical
        // to before this stack existed.
        let mut capture_stack: Vec<CaptureLayer> = Vec::new();

        // Active compositing layers. Each entry is a full-page offscreen pixmap
        // that buffers the ink of a blend-mode node (or its children), plus the
        // opacity and tiny-skia blend operator used to composite it back onto
        // its parent at the matching PopLayer. Empty in the common case — with
        // no layers active the draw target resolution is byte-identical to
        // before (the layer check below short-circuits on an empty Vec).
        let mut layer_stack: Vec<(Pixmap, f32, tiny_skia::BlendMode)> = Vec::new();

        for cmd in &scene.commands {
            // Hoist once per iteration. Push/pop arms mutate the stack and
            // never consume current_ts; draw arms read it and never mutate the
            // stack — so hoisting is behavior-identical to reading in each arm.
            let current_ts = *transform_stack.last().unwrap_or(&Transform::identity());

            // ── Structural / capture commands first ───────────────────────────
            // These never draw into a target pixmap; they mutate the clip /
            // transform stacks or open/close the shadow capture, then `continue`
            // so the drawing dispatch below is reached only by drawing commands.
            match cmd {
                SceneCommand::PushClip { x, y, w, h } => {
                    let new_rect = (*x, *y, x + w, y + h);
                    let current = *clip_stack.last().unwrap_or(&page_clip);
                    // Push the intersection so the stack always represents the
                    // effective clip at the current nesting depth.
                    let intersected =
                        intersect_rects(current, new_rect).unwrap_or((0.0, 0.0, 0.0, 0.0)); // empty → degenerate
                    clip_stack.push(intersected);
                    continue;
                }

                // Never pop below the page clip (index 0).
                SceneCommand::PopClip => {
                    if clip_stack.len() > 1 {
                        clip_stack.pop();
                    }
                    continue;
                }

                SceneCommand::PushTransform { angle_deg, cx, cy } => {
                    let rot = Transform::from_rotate_at(*angle_deg as f32, *cx as f32, *cy as f32);
                    transform_stack.push(current_ts.pre_concat(rot));
                    continue;
                }

                SceneCommand::PopTransform => {
                    if transform_stack.len() > 1 {
                        transform_stack.pop();
                    }
                    continue;
                }

                // Open an offscreen capture for shadowed ink. Always pushes a
                // capture layer so Begin/End stay balanced and captures nest.
                // On allocation failure `pm` is `None` — pushed anyway so the
                // ink draws crisp (no shadow) and the matching End* is balanced.
                SceneCommand::BeginShadow { shadows } => {
                    let pm = Pixmap::new(width, height);
                    capture_stack.push(CaptureLayer {
                        pm,
                        effect: CaptureEffect::Shadow(shadows.clone()),
                    });
                    continue;
                }

                // Close the active shadow capture: paint the blurred shadow
                // layers onto the target below this capture, then composite the
                // crisp ink. After the pop, `current_target` sees the stack
                // without this layer — the next capture, blend layer, or base.
                SceneCommand::EndShadow => {
                    if let Some(layer) = capture_stack.pop()
                        && let (Some(ink), CaptureEffect::Shadow(shadows)) =
                            (layer.pm, layer.effect)
                    {
                        let shadow_target =
                            current_target(&mut capture_stack, &mut layer_stack, &mut pixmap);
                        composite_shadows(shadow_target, &ink, &shadows, width, height);
                    }
                    continue;
                }

                // Open an offscreen capture for a Gaussian-blurred element.
                // Always pushes (nesting); `None` buffer on alloc failure draws
                // crisp and keeps Begin/End balanced.
                SceneCommand::BeginBlur { radius } => {
                    let pm = Pixmap::new(width, height);
                    capture_stack.push(CaptureLayer {
                        pm,
                        effect: CaptureEffect::Blur(*radius),
                    });
                    continue;
                }

                // Close the active blur capture: blur the ink in place, then
                // composite it onto the target below this capture.
                SceneCommand::EndBlur => {
                    if let Some(layer) = capture_stack.pop()
                        && let (Some(mut ink), CaptureEffect::Blur(sigma)) =
                            (layer.pm, layer.effect)
                    {
                        gaussian_blur_premul(&mut ink, sigma);
                        let blur_target =
                            current_target(&mut capture_stack, &mut layer_stack, &mut pixmap);
                        blur_target.draw_pixmap(
                            0,
                            0,
                            ink.as_ref(),
                            &PixmapPaint::default(),
                            Transform::identity(),
                            None,
                        );
                    }
                    continue;
                }

                // Open an offscreen capture for a color-filtered element. Always
                // pushes (nesting). An empty filter list — or allocation failure
                // — yields a `None` buffer: draws fall through (crisp, no
                // filter) and the matching EndFilter skips compositing, exactly
                // as the old empty-list/alloc-failure no-op did.
                SceneCommand::BeginFilter { filters } => {
                    let pm = if filters.is_empty() {
                        None
                    } else {
                        Pixmap::new(width, height)
                    };
                    capture_stack.push(CaptureLayer {
                        pm,
                        effect: CaptureEffect::Filter(filters.clone()),
                    });
                    continue;
                }

                // Close the active filter capture: transform the captured ink
                // in place, then composite it onto the target below this capture.
                SceneCommand::EndFilter => {
                    if let Some(layer) = capture_stack.pop()
                        && let (Some(mut ink), CaptureEffect::Filter(filters)) =
                            (layer.pm, layer.effect)
                    {
                        apply_filters(&mut ink, &filters);
                        let filter_target =
                            current_target(&mut capture_stack, &mut layer_stack, &mut pixmap);
                        filter_target.draw_pixmap(
                            0,
                            0,
                            ink.as_ref(),
                            &PixmapPaint::default(),
                            Transform::identity(),
                            None,
                        );
                    }
                    continue;
                }

                // Open an offscreen capture for a masked element. Always pushes
                // (nesting). On allocation failure `pm` is `None` — draws fall
                // through (unmasked) and the matching EndMask skips compositing,
                // keeping Begin/End balanced.
                SceneCommand::BeginMask { mask } => {
                    let pm = Pixmap::new(width, height);
                    capture_stack.push(CaptureLayer {
                        pm,
                        effect: CaptureEffect::Mask(*mask),
                    });
                    continue;
                }

                // Close the active mask capture: attenuate the captured ink by
                // the coverage field, then composite it onto the target below.
                SceneCommand::EndMask => {
                    if let Some(layer) = capture_stack.pop()
                        && let (Some(mut ink), CaptureEffect::Mask(spec)) = (layer.pm, layer.effect)
                    {
                        attenuate_by_mask(&mut ink, &spec);
                        let target =
                            current_target(&mut capture_stack, &mut layer_stack, &mut pixmap);
                        target.draw_pixmap(
                            0,
                            0,
                            ink.as_ref(),
                            &PixmapPaint::default(),
                            Transform::identity(),
                            None,
                        );
                    }
                    continue;
                }

                // Open a compositing layer: allocate a full-page offscreen pixmap
                // that the following draws (and any nested layers/shadows) paint
                // into, to be composited back at PopLayer. On allocation failure
                // we skip pushing — draws then fall through to the previous
                // target and paint source-over (degraded, never a crash).
                SceneCommand::PushLayer {
                    opacity,
                    blend_mode,
                } => {
                    if let Some(pm) = Pixmap::new(width, height) {
                        layer_stack.push((pm, *opacity as f32, map_blend_mode(*blend_mode)));
                    }
                    continue;
                }

                // Close the most-recent layer: composite its buffered ink onto
                // the NEW current target — the next layer down if one remains,
                // else the active shadow capture, else the canvas — using the
                // layer's opacity and blend operator.
                SceneCommand::PopLayer => {
                    if let Some((layer_pm, op, bm)) = layer_stack.pop() {
                        // After popping this blend layer, composite onto the new
                        // current target: the next blend layer down if any, else
                        // the innermost active capture, else the base canvas.
                        // `current_target` resolves capture-first, so when a
                        // capture is open and no blend layer remains it returns
                        // the capture pixmap — byte-identical to the old order.
                        let target_after_pop =
                            current_target(&mut capture_stack, &mut layer_stack, &mut pixmap);
                        target_after_pop.draw_pixmap(
                            0,
                            0,
                            layer_pm.as_ref(),
                            &PixmapPaint {
                                opacity: op.clamp(0.0, 1.0),
                                blend_mode: bm,
                                quality: FilterQuality::Nearest,
                            },
                            Transform::identity(),
                            None,
                        );
                    }
                    continue;
                }

                // Drawing commands: fall through to the dispatch below (no
                // `continue`). Listed explicitly so this structural match stays
                // exhaustive over `SceneCommand` — no wildcard arm.
                SceneCommand::FillRect { .. }
                | SceneCommand::StrokeRect { .. }
                | SceneCommand::FillRoundedRect { .. }
                | SceneCommand::StrokeRoundedRect { .. }
                | SceneCommand::FillEllipse { .. }
                | SceneCommand::StrokeEllipse { .. }
                | SceneCommand::StrokeLine { .. }
                | SceneCommand::FillPolygon { .. }
                | SceneCommand::StrokePolyline { .. }
                | SceneCommand::DrawImage { .. }
                | SceneCommand::DrawSvgAsset { .. }
                | SceneCommand::DrawGlyphRun { .. } => {}
            }

            // The active drawing target, innermost-first: the topmost effect
            // capture holding a buffer (capture ink is always the innermost draw
            // target), else the top compositing layer if any, else the real
            // canvas. Computed once per drawing command, after the structural
            // match above has run (so no borrow overlaps). With no capture and no
            // layer active this resolves to `&mut pixmap` exactly as before —
            // the no-layer path is byte-identical.
            let target: &mut Pixmap =
                current_target(&mut capture_stack, &mut layer_stack, &mut pixmap);

            let ctx = DrawCtx {
                current_ts,
                effective_clip: *clip_stack.last().unwrap_or(&page_clip),
                width,
                height,
            };
            draw_command(target, ctx, cmd, fonts, assets, &mut svg_fontdb);
        }

        // Convert tiny-skia's premultiplied RGBA8 to straight-alpha RGBA8.
        let raw = pixmap.data(); // &[u8], len = width*height*4, premul RGBA
        let mut rgba = Vec::with_capacity(raw.len());
        for chunk in raw.chunks_exact(4) {
            let (sr, sg, sb, sa) =
                premultiplied_to_straight(chunk[0], chunk[1], chunk[2], chunk[3]);
            rgba.push(sr);
            rgba.push(sg);
            rgba.push(sb);
            rgba.push(sa);
        }

        Ok(RasterImage {
            width,
            height,
            rgba,
        })
    }

    fn encode_png(&self, image: &RasterImage) -> Result<Vec<u8>, RenderError> {
        // Re-premultiply straight-alpha back to premultiplied for tiny-skia.
        let mut premul = Vec::with_capacity(image.rgba.len());
        for chunk in image.rgba.chunks_exact(4) {
            let (r, g, b, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
            if a == 0 {
                premul.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                let a_u16 = u16::from(a);
                let mul = |v: u8| -> u8 {
                    let result = (u16::from(v) * a_u16 + 127) / 255;
                    result.min(255) as u8
                };
                premul.push(mul(r));
                premul.push(mul(g));
                premul.push(mul(b));
                premul.push(a);
            }
        }

        let mut pixmap = Pixmap::new(image.width, image.height).ok_or_else(|| {
            RenderError::new(format!(
                "failed to allocate pixmap for encoding ({}×{})",
                image.width, image.height
            ))
        })?;

        let dst = pixmap.data_mut();
        if dst.len() != premul.len() {
            return Err(RenderError::new(
                "pixel buffer length mismatch during PNG encoding",
            ));
        }
        dst.copy_from_slice(&premul);

        pixmap
            .encode_png()
            .map_err(|e| RenderError::new(format!("PNG encoding failed: {e}")))
    }
}
