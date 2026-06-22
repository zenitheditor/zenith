//! Shared leaf emitters reused across the node writers: the inline `span`
//! line (text/shape/footnote/override children) and the `point` line
//! (polygon/polyline vertices).

use crate::ast::{Point, TextSpan};

use crate::format::writer::{
    escape_kdl_string, indent, write_opt_bool, write_opt_dimension, write_opt_property_value,
    write_opt_str,
};

pub(super) fn write_span(span: &TextSpan, out: &mut String, depth: usize) {
    indent(out, depth);
    out.push_str("span \"");
    out.push_str(&escape_kdl_string(&span.text));
    out.push('"');

    // Inline props: fill, font-weight, italic, underline, strikethrough,
    // vertical-align.
    write_opt_property_value(out, "fill", &span.fill);
    write_opt_property_value(out, "font-weight", &span.font_weight);
    write_opt_bool(out, "italic", &span.italic);
    write_opt_bool(out, "underline", &span.underline);
    write_opt_bool(out, "strikethrough", &span.strikethrough);
    write_opt_str(out, "vertical-align", &span.vertical_align);
    write_opt_str(out, "footnote-ref", &span.footnote_ref);

    out.push('\n');
}

/// Emit a `point x=(unit)N y=(unit)N` line for each vertex in the list.
///
/// The block is always emitted (even for zero points) to maintain a consistent
/// brace-block style, mirroring how `write_text` always emits its `{ … }`.
pub(super) fn write_points(points: &[Point], out: &mut String, depth: usize) {
    for pt in points {
        indent(out, depth);
        out.push_str("point");
        write_opt_dimension(out, "x", &pt.x);
        write_opt_dimension(out, "y", &pt.y);
        out.push('\n');
    }
}
