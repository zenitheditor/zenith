use zenith_core::{AssetProvider, FontProvider};
use zenith_scene::ir::path_segments_bbox;
use zenith_scene::{BlendMode, FillRule, Paint, SceneCommand, StrokeAlign};

use super::geometry::{self, BoxRect, finite};
use super::paint::Stroke;
use crate::RenderError;

pub(super) struct Writer {
    pub(super) body: String,
    pub(super) defs: String,
    width: f64,
    height: f64,
    counter: u64,
}

impl Writer {
    pub(super) fn new(width: f64, height: f64) -> Self {
        Self {
            body: String::new(),
            defs: String::new(),
            width,
            height,
            counter: 0,
        }
    }

    pub(super) fn finish(self) -> String {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\"><defs>{}</defs>{}</svg>",
            self.width, self.height, self.width, self.height, self.defs, self.body
        )
    }

    pub(super) fn id(&mut self, prefix: &str) -> String {
        let id = format!("{prefix}{}", self.counter);
        self.counter += 1;
        id
    }

    pub(super) fn clip(&mut self, path: &str, rule: FillRule) -> String {
        let id = self.id("clip");
        self.defs.push_str(&format!("<clipPath id=\"{id}\" clipPathUnits=\"userSpaceOnUse\"><path d=\"{path}\" clip-rule=\"{}\"/></clipPath>", geometry::rule(rule)));
        id
    }

    fn fill(
        &mut self,
        path: String,
        paint: &Paint,
        bounds: BoxRect,
        rule: FillRule,
    ) -> Result<(), RenderError> {
        let attrs = self.paint(paint, bounds)?;
        if !path.is_empty() {
            self.body.push_str(&format!(
                "<path d=\"{path}\" {attrs} fill-rule=\"{}\"/>",
                geometry::rule(rule)
            ));
        }
        Ok(())
    }

    fn stroke(
        &mut self,
        path: String,
        mut stroke: Stroke,
        align: StrokeAlign,
        rule: FillRule,
        bounds: BoxRect,
    ) -> Result<(), RenderError> {
        let clipping = match align {
            StrokeAlign::Center => String::new(),
            StrokeAlign::Inside => {
                stroke.width *= 2.0;
                let id = self.clip(&path, rule);
                format!(" clip-path=\"url(#{id})\"")
            }
            StrokeAlign::Outside => {
                stroke.width *= 2.0;
                let id = self.id("mask");
                // Opaque black removes the interior independently of stroke alpha.
                let (x, y, w, h) = bounds;
                let padding = stroke.width * stroke.miter.unwrap_or(4.0).max(1.0);
                let (mx, my, mw, mh) = (
                    x - padding,
                    y - padding,
                    w + padding * 2.0,
                    h + padding * 2.0,
                );
                finite(&[mx, my, mw, mh])?;
                self.defs.push_str(&format!("<mask id=\"{id}\" maskUnits=\"userSpaceOnUse\" x=\"{mx}\" y=\"{my}\" width=\"{mw}\" height=\"{mh}\" style=\"mask-type:luminance\"><rect x=\"{mx}\" y=\"{my}\" width=\"{mw}\" height=\"{mh}\" fill=\"white\"/><path d=\"{path}\" fill=\"black\" fill-rule=\"{}\"/></mask>", geometry::rule(rule)));
                format!(" mask=\"url(#{id})\"")
            }
        };
        let attrs = stroke.attributes()?;
        if !path.is_empty() && stroke.width > 0.0 {
            self.body
                .push_str(&format!("<path d=\"{path}\" {attrs}{clipping}/>"));
        }
        Ok(())
    }

    pub(super) fn check_command<'a>(
        &self,
        command: &'a SceneCommand,
        fonts: &dyn FontProvider,
        assets: &dyn AssetProvider,
    ) -> Result<Option<super::assets::SvgCapture<'a>>, RenderError> {
        let mut writer = Self::new(self.width, self.height);
        if matches!(
            command,
            SceneCommand::DrawImage { .. } | SceneCommand::DrawSvgAsset { .. }
        ) {
            writer.image(command, fonts, assets)
        } else {
            writer.command(command, fonts, assets)?;
            Ok(None)
        }
    }

    pub(super) fn command(
        &mut self,
        command: &SceneCommand,
        fonts: &dyn FontProvider,
        assets: &dyn AssetProvider,
    ) -> Result<(), RenderError> {
        match command {
            SceneCommand::FillRect { x, y, w, h, paint } => {
                let bounds = (*x, *y, *w, *h);
                let path = geometry::rect(bounds)?;
                self.fill(
                    if *w > 0.0 && *h > 0.0 {
                        path
                    } else {
                        String::new()
                    },
                    paint,
                    bounds,
                    FillRule::NonZero,
                )
            }
            SceneCommand::StrokeRect {
                x,
                y,
                w,
                h,
                color,
                stroke_width,
                stroke_dash,
                stroke_gap,
                stroke_linecap,
            } => {
                let path = geometry::rect((*x, *y, *w, *h))?;
                let stroke = Stroke {
                    dash: *stroke_dash,
                    gap: *stroke_gap,
                    cap: *stroke_linecap,
                    ..Stroke::new(*color, *stroke_width)
                };
                self.stroke(
                    if *w > 0.0 && *h > 0.0 {
                        path
                    } else {
                        String::new()
                    },
                    stroke,
                    StrokeAlign::Center,
                    FillRule::NonZero,
                    (0.0, 0.0, 0.0, 0.0),
                )
            }
            SceneCommand::FillRoundedRect {
                x,
                y,
                w,
                h,
                radius,
                radii,
                paint,
            } => {
                finite(&[*radius])?;
                let bounds = (*x, *y, *w, *h);
                self.fill(
                    geometry::rounded(bounds, radii.unwrap_or([*radius; 4]))?,
                    paint,
                    bounds,
                    FillRule::NonZero,
                )
            }
            SceneCommand::StrokeRoundedRect {
                x,
                y,
                w,
                h,
                radius,
                radii,
                color,
                stroke_width,
                stroke_dash,
                stroke_gap,
                stroke_linecap,
            } => {
                finite(&[*radius])?;
                let path = geometry::rounded((*x, *y, *w, *h), radii.unwrap_or([*radius; 4]))?;
                let stroke = Stroke {
                    dash: *stroke_dash,
                    gap: *stroke_gap,
                    cap: *stroke_linecap,
                    ..Stroke::new(*color, *stroke_width)
                };
                self.stroke(
                    path,
                    stroke,
                    StrokeAlign::Center,
                    FillRule::NonZero,
                    (0.0, 0.0, 0.0, 0.0),
                )
            }
            SceneCommand::FillEllipse {
                x,
                y,
                w,
                h,
                paint,
                rx,
                ry,
            } => {
                let bounds = (*x, *y, *w, *h);
                self.fill(
                    geometry::ellipse(bounds, *rx, *ry)?,
                    paint,
                    bounds,
                    FillRule::NonZero,
                )
            }
            SceneCommand::StrokeEllipse {
                x,
                y,
                w,
                h,
                color,
                stroke_width,
                stroke_dash,
                stroke_gap,
                stroke_linecap,
                rx,
                ry,
            } => {
                let path = geometry::ellipse((*x, *y, *w, *h), *rx, *ry)?;
                let stroke = Stroke {
                    dash: *stroke_dash,
                    gap: *stroke_gap,
                    cap: *stroke_linecap,
                    ..Stroke::new(*color, *stroke_width)
                };
                self.stroke(
                    path,
                    stroke,
                    StrokeAlign::Center,
                    FillRule::NonZero,
                    (0.0, 0.0, 0.0, 0.0),
                )
            }
            SceneCommand::StrokeLine {
                x1,
                y1,
                x2,
                y2,
                color,
                stroke_width,
                stroke_dash,
                stroke_gap,
                stroke_linecap,
            } => {
                finite(&[*x1, *y1, *x2, *y2])?;
                let stroke = Stroke {
                    dash: *stroke_dash,
                    gap: *stroke_gap,
                    cap: *stroke_linecap,
                    ..Stroke::new(*color, *stroke_width)
                };
                self.stroke(
                    format!("M{x1} {y1}L{x2} {y2}"),
                    stroke,
                    StrokeAlign::Center,
                    FillRule::NonZero,
                    (0.0, 0.0, 0.0, 0.0),
                )
            }
            SceneCommand::FillPolygon {
                points,
                paint,
                fill_rule,
            } => self.fill(
                geometry::polygon(points, true)?,
                paint,
                geometry::points_box(points),
                *fill_rule,
            ),
            SceneCommand::StrokePolyline {
                points,
                color,
                stroke_width,
                closed,
                align,
                clip_fill_rule,
            } => {
                let path = geometry::polygon(points, *closed)?;
                let align = if *closed { *align } else { StrokeAlign::Center };
                self.stroke(
                    path,
                    Stroke::new(*color, *stroke_width),
                    align,
                    *clip_fill_rule,
                    geometry::points_box(points),
                )
            }
            SceneCommand::FillPath {
                segments,
                paint,
                fill_rule,
            } => self.fill(
                geometry::path(segments)?,
                paint,
                path_segments_bbox(segments).unwrap_or((0.0, 0.0, 0.0, 0.0)),
                *fill_rule,
            ),
            SceneCommand::StrokePath {
                segments,
                color,
                stroke_width,
                closed,
                align,
                clip_fill_rule,
                stroke_linejoin,
                stroke_linecap,
                stroke_miter_limit,
            } => {
                let path = geometry::path(segments)?;
                let stroke = Stroke {
                    join: *stroke_linejoin,
                    cap: *stroke_linecap,
                    miter: *stroke_miter_limit,
                    ..Stroke::new(*color, *stroke_width)
                };
                let align = if *closed { *align } else { StrokeAlign::Center };
                self.stroke(
                    path,
                    stroke,
                    align,
                    *clip_fill_rule,
                    path_segments_bbox(segments).unwrap_or((0.0, 0.0, 0.0, 0.0)),
                )
            }
            SceneCommand::DrawImage { .. } | SceneCommand::DrawSvgAsset { .. } => {
                self.image(command, fonts, assets).map(|_| ())
            }
            SceneCommand::DrawGlyphRun { .. } => self.text(command, fonts),
            SceneCommand::PushClip { x, y, w, h } => {
                let path = geometry::rect((*x, *y, *w, *h))?;
                let id = self.clip(
                    if *w > 0.0 && *h > 0.0 { &path } else { "" },
                    FillRule::NonZero,
                );
                self.body.push_str(&format!("<g clip-path=\"url(#{id})\">"));
                Ok(())
            }
            SceneCommand::PushClipRoundedRect { x, y, w, h, radius } => {
                let path = geometry::rounded((*x, *y, *w, *h), [*radius; 4])?;
                let id = self.clip(&path, FillRule::NonZero);
                self.body.push_str(&format!("<g clip-path=\"url(#{id})\">"));
                Ok(())
            }
            SceneCommand::PushLayer {
                opacity,
                blend_mode,
            } => {
                finite(&[*opacity])?;
                let blend = match blend_mode {
                    None | Some(BlendMode::Normal) => String::new(),
                    Some(mode) => format!(" style=\"mix-blend-mode:{}\"", mode.as_kebab()),
                };
                self.body.push_str(&format!(
                    "<g opacity=\"{}\"{blend}>",
                    opacity.clamp(0.0, 1.0)
                ));
                Ok(())
            }
            SceneCommand::PushTransform { angle_deg, cx, cy } => {
                finite(&[*angle_deg, *cx, *cy])?;
                self.body
                    .push_str(&format!("<g transform=\"rotate({angle_deg} {cx} {cy})\">"));
                Ok(())
            }
            SceneCommand::PushScaleTranslate { sx, sy, tx, ty } => {
                finite(&[*sx, *sy, *tx, *ty])?;
                self.body.push_str(&format!(
                    "<g transform=\"matrix({sx} 0 0 {sy} {tx} {ty})\">"
                ));
                Ok(())
            }
            SceneCommand::PushTransformMatrix { a, b, c, d, e, f } => {
                finite(&[*a, *b, *c, *d, *e, *f])?;
                self.body.push_str(&format!(
                    "<g transform=\"matrix({a} {b} {c} {d} {e} {f})\">"
                ));
                Ok(())
            }
            SceneCommand::PopClip | SceneCommand::PopLayer | SceneCommand::PopTransform => {
                self.body.push_str("</g>");
                Ok(())
            }
            SceneCommand::BeginShadow { .. }
            | SceneCommand::EndShadow
            | SceneCommand::BeginBlur { .. }
            | SceneCommand::EndBlur
            | SceneCommand::BeginFilter { .. }
            | SceneCommand::EndFilter
            | SceneCommand::BeginMask { .. }
            | SceneCommand::EndMask => super::effects::check(command),
        }
    }
}
