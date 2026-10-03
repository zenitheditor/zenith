//! Shaping and emission of chart strings, shared by every chart kind.
//!
//! One [`ChartText`] carries the chart look and the shaping context. Each
//! string is shaped at its role's size and weight, and each drawn run carries
//! the role source id (see `role`), so the page lint can attribute it.

use zenith_core::{Diagnostic, FontStyle};
use zenith_layout::{ShapeRequest, TextDirection, TextLayoutEngine, ZenithGlyphRun};

use crate::ir::{Color, SceneCommand};

use super::super::NodeCtx;
use super::super::text::run_to_scene_glyphs;
use super::look::ChartLook;
use super::role::ChartTextRole;

/// The shaping context of one chart's strings.
#[derive(Clone, Copy)]
pub(super) struct ChartText<'a> {
    pub(super) cx: NodeCtx<'a>,
    pub(super) look: &'a ChartLook,
    pub(super) chart_id: &'a str,
}

/// One shaped string, ready to place.
pub(super) struct Shaped {
    runs: Vec<ZenithGlyphRun>,
    /// Total advance width in px.
    pub(super) advance: f64,
    /// Ascent of the first run in px.
    pub(super) ascent: f64,
    role: ChartTextRole,
}

impl Shaped {
    /// The same string drawn as `role`. Only a role of the same size and
    /// weight keeps the shaping valid: `Value` and `ValueInside` swap.
    pub(super) fn with_role(self, role: ChartTextRole) -> Shaped {
        Shaped { role, ..self }
    }
}

impl ChartText<'_> {
    /// The base size in px.
    pub(super) fn base(&self) -> f64 {
        self.look.base
    }

    /// The font size of `role` in px.
    pub(super) fn size(&self, role: ChartTextRole) -> f64 {
        self.look.base * role.scale()
    }

    fn request<'r>(&'r self, text: &'r str, role: ChartTextRole) -> ShapeRequest<'r> {
        ShapeRequest {
            text,
            families: &self.look.families,
            weight: role.weight(),
            style: FontStyle::Normal,
            font_size: self.size(role) as f32,
            direction: TextDirection::Ltr,
            features: &[],
            kerning_pairs: &[],
            letter_spacing_px: 0.0,
        }
    }

    /// Shape `text` as `role`. A shaping error pushes `scene.text_unshaped`
    /// and returns `None`.
    pub(super) fn shape(
        &self,
        text: &str,
        role: ChartTextRole,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Shaped> {
        let req = self.request(text, role);
        match self.cx.engine.shape_with_fallback(&req, self.cx.fonts) {
            Ok(result) => {
                let advance = result.runs.iter().map(|r| f64::from(r.advance_width)).sum();
                let ascent = result
                    .runs
                    .first()
                    .map_or(self.size(role) * 0.75, |r| f64::from(r.ascent));
                Some(Shaped {
                    runs: result.runs,
                    advance,
                    ascent,
                    role,
                })
            }
            Err(e) => {
                diagnostics.push(Diagnostic::advisory(
                    "scene.text_unshaped",
                    format!(
                        "chart '{}' {} text '{}' could not be shaped: {}; check the chart style font-family",
                        self.chart_id,
                        role.as_str(),
                        text,
                        e.message
                    ),
                    None,
                    Some(self.chart_id.to_owned()),
                ));
                None
            }
        }
    }

    /// The advance width of `text` as `role`; `0.0` when it does not shape.
    pub(super) fn advance(&self, text: &str, role: ChartTextRole) -> f64 {
        let req = self.request(text, role);
        self.cx
            .engine
            .shape_with_fallback(&req, self.cx.fonts)
            .map_or(0.0, |r| {
                r.runs.iter().map(|run| f64::from(run.advance_width)).sum()
            })
    }

    /// Emit `shaped` with its left edge at `x` and its baseline at
    /// `baseline`, as string `index` of its role.
    pub(super) fn emit(
        &self,
        shaped: Shaped,
        (x, baseline): (f64, f64),
        color: Color,
        index: usize,
        commands: &mut Vec<SceneCommand>,
    ) {
        let source = shaped.role.source_id(self.chart_id, index);
        let mut pen = x;
        for run in shaped.runs {
            let advance = f64::from(run.advance_width);
            let glyphs = run_to_scene_glyphs(&run);
            commands.push(SceneCommand::DrawGlyphRun {
                x: pen,
                y: baseline,
                font_id: run.font_id,
                font_size: run.font_size,
                color,
                stroke_color: None,
                stroke_width: None,
                link: None,
                selectable: true,
                source_node_id: Some(source.clone()),
                glyphs,
            });
            pen += advance;
        }
    }
}
