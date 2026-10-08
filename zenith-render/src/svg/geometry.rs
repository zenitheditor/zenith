use crate::RenderError;
use zenith_scene::{FillRule, ir::PathSegment};

pub(super) type BoxRect = (f64, f64, f64, f64);

pub(super) fn finite(values: &[f64]) -> Result<(), RenderError> {
    if values
        .iter()
        .all(|value| value.is_finite() && (*value as f32).is_finite())
    {
        Ok(())
    } else {
        Err(RenderError::new(format!(
            "invalid SVG numeric coordinates {values:?}"
        )))
    }
}

pub(super) fn rule(rule: FillRule) -> &'static str {
    match rule {
        FillRule::NonZero => "nonzero",
        FillRule::EvenOdd => "evenodd",
    }
}

pub(super) fn rect((x, y, w, h): BoxRect) -> Result<String, RenderError> {
    finite(&[x, y, w, h, x + w, y + h])?;
    Ok(format!("M{x} {y}H{}V{}H{x}Z", x + w, y + h))
}

pub(super) fn rounded(box_rect: BoxRect, radii: [f64; 4]) -> Result<String, RenderError> {
    finite(&[box_rect.0, box_rect.1, box_rect.2, box_rect.3])?;
    finite(&radii)?;
    let (x, y, w, h) = box_rect;
    if w <= 0.0 || h <= 0.0 {
        return Ok(String::new());
    }
    let path = crate::tiny_skia::build_rounded_rect_path(
        x as f32,
        y as f32,
        w as f32,
        h as f32,
        radii.map(|r| r as f32),
    )
    .ok_or_else(|| RenderError::new("invalid SVG rounded rectangle"))?;
    Ok(tiny_path(&path))
}

pub(super) fn ellipse(
    box_rect: BoxRect,
    rx: Option<f64>,
    ry: Option<f64>,
) -> Result<String, RenderError> {
    let (x, y, w, h) = box_rect;
    let (rx, ry) = (rx.unwrap_or(w / 2.0), ry.unwrap_or(h / 2.0));
    finite(&[x, y, w, h, rx, ry])?;
    if w <= 0.0 || h <= 0.0 || rx <= 0.0 || ry <= 0.0 {
        return Ok(String::new());
    }
    let rect = tiny_skia::Rect::from_xywh(
        (x + w / 2.0 - rx) as f32,
        (y + h / 2.0 - ry) as f32,
        (rx * 2.0) as f32,
        (ry * 2.0) as f32,
    )
    .ok_or_else(|| RenderError::new("invalid SVG ellipse"))?;
    let path = tiny_skia::PathBuilder::from_oval(rect)
        .ok_or_else(|| RenderError::new("invalid SVG ellipse path"))?;
    Ok(tiny_path(&path))
}

pub(super) fn tiny_path(path: &tiny_skia::Path) -> String {
    let mut result = String::new();
    for segment in path.segments() {
        match segment {
            tiny_skia::PathSegment::MoveTo(p) => result.push_str(&format!("M{} {}", p.x, p.y)),
            tiny_skia::PathSegment::LineTo(p) => result.push_str(&format!("L{} {}", p.x, p.y)),
            tiny_skia::PathSegment::QuadTo(a, b) => {
                result.push_str(&format!("Q{} {} {} {}", a.x, a.y, b.x, b.y))
            }
            tiny_skia::PathSegment::CubicTo(a, b, c) => {
                result.push_str(&format!("C{} {} {} {} {} {}", a.x, a.y, b.x, b.y, c.x, c.y))
            }
            tiny_skia::PathSegment::Close => result.push('Z'),
        }
    }
    result
}

pub(super) fn polygon(points: &[f64], closed: bool) -> Result<String, RenderError> {
    finite(points)?;
    if !points.len().is_multiple_of(2) {
        return Err(RenderError::new("SVG polygon has an unmatched coordinate"));
    }
    let mut result = String::new();
    for (index, [x, y]) in points.as_chunks::<2>().0.iter().enumerate() {
        result.push_str(&format!("{}{x} {y}", if index == 0 { 'M' } else { 'L' }));
    }
    if closed && !result.is_empty() {
        result.push('Z');
    }
    Ok(result)
}

pub(super) fn path(segments: &[PathSegment]) -> Result<String, RenderError> {
    let mut result = String::new();
    let mut open = false;
    for segment in segments {
        match segment {
            PathSegment::MoveTo { x, y } => {
                finite(&[*x, *y])?;
                result.push_str(&format!("M{x} {y}"));
                open = true;
            }
            PathSegment::LineTo { x, y } => {
                if !open {
                    return Err(RenderError::new("SVG line lacks a path origin"));
                }
                finite(&[*x, *y])?;
                result.push_str(&format!("L{x} {y}"));
            }
            PathSegment::CubicTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                if !open {
                    return Err(RenderError::new("SVG cubic lacks a path origin"));
                }
                finite(&[*x1, *y1, *x2, *y2, *x, *y])?;
                result.push_str(&format!("C{x1} {y1} {x2} {y2} {x} {y}"));
            }
            PathSegment::Close => {
                if !open {
                    return Err(RenderError::new("SVG close lacks a path origin"));
                }
                result.push('Z');
                open = false;
            }
        }
    }
    Ok(result)
}

pub(super) fn points_box(points: &[f64]) -> BoxRect {
    let mut bounds: Option<(f64, f64, f64, f64)> = None;
    for [x, y] in points.as_chunks::<2>().0 {
        bounds = Some(match bounds {
            None => (*x, *y, *x, *y),
            Some((a, b, c, d)) => (a.min(*x), b.min(*y), c.max(*x), d.max(*y)),
        });
    }
    bounds.map_or((0.0, 0.0, 0.0, 0.0), |(a, b, c, d)| (a, b, c - a, d - b))
}
