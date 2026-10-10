//! The label and chart-text ink the compile-stage contrast pass judges.
//!
//! A `shape` or `connector` label is laid out by the scene compiler, so its
//! real position exists only after the page compiles: the shape's padded,
//! aligned content box, or the midpoint of the routed connector. Chart
//! strings are the same. This pass reads the drawn glyph runs of each label
//! (`<owner>/label`) and each chart string (`<chart>/<role>/<index>`) and
//! measures their ink box. [`zenith_core::page_contrast_checks`] samples the
//! backdrop over it.

use std::collections::BTreeMap;

use zenith_core::{ChartTextInk, LabelInk};

use crate::ir::SceneCommand;

use super::super::chart::parse_chart_source;
use super::super::text::{ShapeEnv, ink_bounds};

/// Suffix of a synthesized label text id.
const LABEL_SUFFIX: &str = "/label";

/// The drawn runs of one label.
#[derive(Default)]
struct Runs {
    commands: Vec<SceneCommand>,
    colors: Vec<(u8, u8, u8)>,
    font_size: f64,
    /// The single rotation every run draws under, in scene px.
    rotation: Option<(f64, f64, f64)>,
    /// A run draws under a transform the check cannot model.
    unmodeled: bool,
}

/// The transform a run of the command stream draws under.
#[derive(Clone, Copy)]
enum Transform {
    Rotate(f64, f64, f64),
    Other,
}

/// The drawn runs of `commands` grouped by `key` of their source id. A run
/// whose source `key` maps to `None` is skipped.
fn collect_runs(
    commands: &[SceneCommand],
    key: impl Fn(&str) -> Option<String>,
) -> BTreeMap<String, Runs> {
    let mut stack: Vec<Transform> = Vec::new();
    let mut groups: BTreeMap<String, Runs> = BTreeMap::new();
    for command in commands {
        match command {
            SceneCommand::PushTransform { angle_deg, cx, cy } => {
                stack.push(Transform::Rotate(*angle_deg, *cx, *cy));
            }
            SceneCommand::PushScaleTranslate { .. } | SceneCommand::PushTransformMatrix { .. } => {
                stack.push(Transform::Other);
            }
            SceneCommand::PopTransform => {
                stack.pop();
            }
            SceneCommand::DrawGlyphRun {
                color,
                font_size,
                source_node_id,
                ..
            } => {
                let Some(group) = source_node_id.as_deref().and_then(&key) else {
                    continue;
                };
                let runs = groups.entry(group).or_default();
                let rotation = match stack.as_slice() {
                    [] => None,
                    [Transform::Rotate(a, cx, cy)] => Some((*a, *cx, *cy)),
                    [Transform::Other] | [_, _, ..] => {
                        runs.unmodeled = true;
                        None
                    }
                };
                if runs.commands.is_empty() {
                    runs.rotation = rotation;
                } else if runs.rotation != rotation {
                    runs.unmodeled = true;
                }
                runs.commands.push(command.clone());
                let rgb = (color.r, color.g, color.b);
                if color.a > 0 && !runs.colors.contains(&rgb) {
                    runs.colors.push(rgb);
                }
                runs.font_size = runs.font_size.max(f64::from(*font_size));
            }
            SceneCommand::FillRect { .. }
            | SceneCommand::StrokeRect { .. }
            | SceneCommand::FillRoundedRect { .. }
            | SceneCommand::StrokeRoundedRect { .. }
            | SceneCommand::FillEllipse { .. }
            | SceneCommand::StrokeEllipse { .. }
            | SceneCommand::StrokeLine { .. }
            | SceneCommand::FillPolygon { .. }
            | SceneCommand::StrokePolyline { .. }
            | SceneCommand::FillPath { .. }
            | SceneCommand::StrokePath { .. }
            | SceneCommand::DrawImage { .. }
            | SceneCommand::DrawSvgAsset { .. }
            | SceneCommand::PushClip { .. }
            | SceneCommand::PushClipRoundedRect { .. }
            | SceneCommand::PopClip
            | SceneCommand::PushLayer { .. }
            | SceneCommand::PopLayer
            | SceneCommand::BeginShadow { .. }
            | SceneCommand::EndShadow
            | SceneCommand::BeginBlur { .. }
            | SceneCommand::EndBlur
            | SceneCommand::BeginFilter { .. }
            | SceneCommand::EndFilter
            | SceneCommand::BeginMask { .. }
            | SceneCommand::EndMask => {}
        }
    }
    groups
}

/// The measured ink of `runs` in page px, or `None` when they draw under a
/// transform the check cannot model, in no visible colour, or no ink.
fn measure(runs: Runs, bleed: f64, env: ShapeEnv<'_>) -> Option<LabelInk> {
    if runs.unmodeled || runs.colors.is_empty() {
        return None;
    }
    let ink = ink_bounds(&runs.commands, env)?;
    Some(LabelInk {
        x: ink.left - bleed,
        y: ink.top - bleed,
        w: (ink.right - ink.left).max(0.0),
        h: (ink.bottom - ink.top).max(0.0),
        rotation: runs.rotation.map(|(a, cx, cy)| (a, cx - bleed, cy - bleed)),
        colors: runs.colors,
        font_size_px: runs.font_size,
    })
}

/// The measured ink of every label drawn into `commands`, by owner id, in
/// page px. `bleed` is the scene offset of the trim box. A label drawn under
/// a transform the check cannot model, or in no visible colour, is left out.
pub(in crate::compile) fn label_inks(
    commands: &[SceneCommand],
    bleed: f64,
    env: ShapeEnv<'_>,
) -> BTreeMap<String, LabelInk> {
    collect_runs(commands, |s| {
        s.strip_suffix(LABEL_SUFFIX).map(str::to_owned)
    })
    .into_iter()
    .filter_map(|(owner, runs)| Some((owner, measure(runs, bleed, env)?)))
    .collect()
}

/// The measured ink of every chart string drawn into `commands`, grouped by
/// chart id, in source-id order. Roles the contrast check does not judge
/// (value labels inside a mark) are left out, as are strings the check
/// cannot model (see [`label_inks`]).
pub(in crate::compile) fn chart_inks(
    commands: &[SceneCommand],
    bleed: f64,
    env: ShapeEnv<'_>,
) -> BTreeMap<String, Vec<ChartTextInk>> {
    let judged = |s: &str| {
        parse_chart_source(s)
            .filter(|(_, role, _)| role.contrast_judged())
            .map(|_| s.to_owned())
    };
    let mut out: BTreeMap<String, Vec<ChartTextInk>> = BTreeMap::new();
    for (source, runs) in collect_runs(commands, judged) {
        let Some((chart, role, _)) = parse_chart_source(&source) else {
            continue;
        };
        let Some(ink) = measure(runs, bleed, env) else {
            continue;
        };
        out.entry(chart.to_owned()).or_default().push(ChartTextInk {
            role: role.as_str().to_owned(),
            weight: role.weight(),
            ink,
        });
    }
    out
}
