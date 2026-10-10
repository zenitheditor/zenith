use super::geometry::{BoxRect, finite};
use super::writer::Writer;
use crate::RenderError;
use zenith_geometry::math;
use zenith_scene::{Color, LineCap, LineJoin, Paint};

pub(super) fn color(color: Color, attribute: &str) -> String {
    format!(
        "{attribute}=\"#{:02x}{:02x}{:02x}\" {attribute}-opacity=\"{}\"",
        color.r,
        color.g,
        color.b,
        f64::from(color.a) / 255.0
    )
}

impl Writer {
    pub(super) fn paint(&mut self, paint: &Paint, bounds: BoxRect) -> Result<String, RenderError> {
        match paint {
            Paint::Solid { color: c } => Ok(color(*c, "fill")),
            Paint::Gradient(gradient) => {
                finite(&[gradient.angle_deg])?;
                let (x, y, w, h) = bounds;
                let id = self.id("gradient");
                if gradient.stops.is_empty() {
                    return Err(RenderError::new("SVG gradient has no stops"));
                }
                let (element, attrs) = if gradient.radial {
                    let cx = x + w * gradient.center_x.unwrap_or(0.5);
                    let cy = y + h * gradient.center_y.unwrap_or(0.5);
                    let radius =
                        gradient.radius_frac.unwrap_or(1.0) * math::hypot(w / 2.0, h / 2.0);
                    finite(&[cx, cy, radius])?;
                    if radius <= 0.0 {
                        return Err(RenderError::new(
                            "SVG radial gradient radius must be positive",
                        ));
                    }
                    (
                        "radialGradient",
                        format!("cx=\"{cx}\" cy=\"{cy}\" r=\"{radius}\""),
                    )
                } else {
                    let theta = gradient.angle_deg.to_radians();
                    let (dx, dy) = (math::cos(theta), math::sin(theta));
                    let half = ((w * dx).abs() + (h * dy).abs()) / 2.0;
                    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
                    (
                        "linearGradient",
                        format!(
                            "x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"",
                            cx - dx * half,
                            cy - dy * half,
                            cx + dx * half,
                            cy + dy * half
                        ),
                    )
                };
                self.defs.push_str(&format!(
                    "<{element} id=\"{id}\" gradientUnits=\"userSpaceOnUse\" {attrs}>"
                ));
                for stop in &gradient.stops {
                    finite(&[stop.offset])?;
                    self.defs.push_str(&format!(
                        "<stop offset=\"{}\" {}/>",
                        stop.offset.clamp(0.0, 1.0),
                        color(stop.color, "stop-color")
                            .replace("stop-color-opacity", "stop-opacity")
                    ));
                }
                self.defs.push_str(&format!("</{element}>"));
                Ok(format!("fill=\"url(#{id})\""))
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Stroke {
    pub color: Color,
    pub width: f64,
    pub dash: Option<f64>,
    pub gap: Option<f64>,
    pub cap: Option<LineCap>,
    pub join: Option<LineJoin>,
    pub miter: Option<f64>,
}

impl Stroke {
    pub(super) fn new(color: Color, width: f64) -> Self {
        Self {
            color,
            width,
            dash: None,
            gap: None,
            cap: None,
            join: None,
            miter: None,
        }
    }
    pub(super) fn attributes(self) -> Result<String, RenderError> {
        finite(&[
            self.width,
            self.dash.unwrap_or(0.0),
            self.gap.unwrap_or(0.0),
            self.miter.unwrap_or(4.0),
        ])?;
        let cap = match self.cap.unwrap_or(LineCap::Butt) {
            LineCap::Butt => "butt",
            LineCap::Round => "round",
            LineCap::Square => "square",
        };
        let join = match self.join.unwrap_or(LineJoin::Miter) {
            LineJoin::Miter => "miter",
            LineJoin::Round => "round",
            LineJoin::Bevel => "bevel",
        };
        let mut attrs = format!(
            "fill=\"none\" {} stroke-width=\"{}\" stroke-linecap=\"{cap}\" stroke-linejoin=\"{join}\" stroke-miterlimit=\"{}\"",
            color(self.color, "stroke"),
            self.width.max(0.0),
            self.miter.unwrap_or(4.0).max(1.0)
        );
        if let Some(dash) = self.dash
            && dash > 0.0
        {
            attrs.push_str(&format!(
                " stroke-dasharray=\"{dash} {}\"",
                self.gap.unwrap_or(dash).max(0.0)
            ));
        }
        Ok(attrs)
    }
}
