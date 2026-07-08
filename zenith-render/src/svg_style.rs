//! Render-time SVG style parameterization.
//!
//! This never mutates source asset bytes. Callers pass the returned bytes to
//! `usvg`; locked hash verification still applies to the original asset file.

use std::borrow::Cow;

use zenith_scene::{Color, SvgStyle};

pub(crate) fn styled_svg_bytes<'a>(bytes: &'a [u8], style: Option<SvgStyle>) -> Cow<'a, [u8]> {
    let Some(style) = style else {
        return Cow::Borrowed(bytes);
    };
    let Ok(mut svg) = String::from_utf8(bytes.to_vec()) else {
        return Cow::Borrowed(bytes);
    };
    let mut changed = false;

    if let Some(stroke) = style.stroke {
        let color = color_hex(stroke);
        changed |= replace_current_color_attr(&mut svg, "stroke", &color);
    }

    if let Some(fill) = style.fill {
        changed |= replace_current_color_attr(&mut svg, "fill", &color_hex(fill));
    }
    if let Some(width) = style.stroke_width {
        changed |= replace_attr_value(&mut svg, "stroke-width", &format_stroke_width(width));
    }

    if changed {
        Cow::Owned(svg.into_bytes())
    } else {
        Cow::Borrowed(bytes)
    }
}

fn color_hex(color: Color) -> String {
    if color.a == 255 {
        format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
    } else {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            color.r, color.g, color.b, color.a
        )
    }
}

fn format_stroke_width(width: f64) -> String {
    let clamped = width.max(0.0);
    if clamped.fract() == 0.0 {
        format!("{clamped:.0}")
    } else {
        let mut s = format!("{clamped:.6}");
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
        s
    }
}

fn replace_current_color_attr(svg: &mut String, attr: &str, value: &str) -> bool {
    replace_attr_if(svg, attr, value, |old| {
        old.trim().eq_ignore_ascii_case("currentColor")
    })
}

fn replace_attr_value(svg: &mut String, attr: &str, value: &str) -> bool {
    replace_attr_if(svg, attr, value, |_| true)
}

fn replace_attr_if<F>(svg: &mut String, attr: &str, value: &str, should_replace: F) -> bool
where
    F: Fn(&str) -> bool,
{
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg.as_str();
    let needle = format!("{attr}=");
    let mut changed = false;

    while let Some(pos) = rest.find(&needle) {
        let (before, after_needle) = rest.split_at(pos);
        out.push_str(before);
        out.push_str(&needle);
        let after_value_prefix = &after_needle[needle.len()..];
        let mut chars = after_value_prefix.chars();
        let Some(quote) = chars.next() else {
            rest = after_value_prefix;
            continue;
        };
        if quote != '"' && quote != '\'' {
            rest = after_value_prefix;
            continue;
        }
        let value_start = quote.len_utf8();
        let value_rest = &after_value_prefix[value_start..];
        let Some(end) = value_rest.find(quote) else {
            out.push_str(after_value_prefix);
            rest = "";
            break;
        };
        let old = &value_rest[..end];
        out.push(quote);
        if should_replace(old) {
            out.push_str(value);
            changed = true;
        } else {
            out.push_str(old);
        }
        out.push(quote);
        rest = &value_rest[end + quote.len_utf8()..];
    }

    out.push_str(rest);
    *svg = out;
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_lucide_current_color_and_stroke_width() {
        let src =
            br##"<svg fill="none" stroke="currentColor" stroke-width="2"><path d=""/></svg>"##;
        let out = styled_svg_bytes(
            src,
            Some(SvgStyle {
                stroke: Some(Color::srgb(18, 52, 86, 255)),
                fill: Some(Color::srgb(200, 10, 20, 255)),
                stroke_width: Some(3.5),
            }),
        );
        let text = String::from_utf8(out.into_owned()).expect("utf8");
        assert!(text.contains(r##"stroke="#123456""##), "{text}");
        assert!(text.contains(r##"fill="none""##), "{text}");
        assert!(text.contains(r##"stroke-width="3.5""##), "{text}");
    }

    #[test]
    fn preserves_non_current_color_paint() {
        let src = br##"<svg><path fill="url(#g)" stroke="#000"/></svg>"##;
        let out = styled_svg_bytes(
            src,
            Some(SvgStyle {
                stroke: Some(Color::srgb(255, 0, 0, 255)),
                fill: Some(Color::srgb(0, 255, 0, 255)),
                stroke_width: None,
            }),
        );
        let text = String::from_utf8(out.into_owned()).expect("utf8");
        assert!(text.contains(r##"fill="url(#g)""##), "{text}");
        assert!(text.contains(r##"stroke="#000""##), "{text}");
    }

    #[test]
    fn returns_borrowed_when_style_changes_nothing() {
        let src = br##"<svg><path d=""/></svg>"##;
        let out = styled_svg_bytes(
            src,
            Some(SvgStyle {
                stroke: Some(Color::srgb(18, 52, 86, 255)),
                fill: None,
                stroke_width: Some(2.0),
            }),
        );
        assert!(matches!(out, Cow::Borrowed(bytes) if bytes == src));
    }
}
