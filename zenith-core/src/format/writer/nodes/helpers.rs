//! Shared leaf emitters reused across the node writers: the inline `span`
//! line (text/shape/footnote/override children), the `point` line
//! (polygon/polyline vertices), the `anchor` line (path anchors), and the
//! `block` role-style declaration.

use crate::ast::{BlockStyle, PathAnchor, PathSubpath, Point, TextSpan};
use crate::data::DataFormat;

use crate::format::writer::{
    escape_kdl_string, indent, write_opt_bool, write_opt_dimension, write_opt_property_value,
    write_opt_str, write_opt_str_escaped,
};

/// Emit a single `block role="…" …` declaration line (no child block — leaf
/// decl like `fold`). Canonical property order: role, font-family, font-size,
/// font-weight, fill, align, italic, space-before, space-after.
/// Fields absent from the declaration are silently skipped.
pub(super) fn write_block_style(bs: &BlockStyle, out: &mut String, depth: usize) {
    indent(out, depth);
    out.push_str("block role=\"");
    out.push_str(&bs.role);
    out.push('"');
    write_opt_property_value(out, "font-family", &bs.font_family);
    write_opt_property_value(out, "font-size", &bs.font_size);
    write_opt_property_value(out, "font-weight", &bs.font_weight);
    write_opt_property_value(out, "fill", &bs.fill);
    write_opt_str(out, "align", &bs.align);
    write_opt_bool(out, "italic", &bs.italic);
    write_opt_dimension(out, "space-before", &bs.space_before);
    write_opt_dimension(out, "space-after", &bs.space_after);
    out.push('\n');
}

pub(super) fn write_span(span: &TextSpan, out: &mut String, depth: usize) {
    indent(out, depth);
    out.push_str("span \"");
    out.push_str(&escape_kdl_string(&span.text));
    out.push('"');

    // Inline props: fill, font-weight, italic, underline, strikethrough,
    // highlight, code, link. Each is emitted only when present so a span
    // without any of them is byte-identical to before.
    write_opt_property_value(out, "fill", &span.fill);
    write_opt_property_value(out, "font-weight", &span.font_weight);
    write_opt_str(out, "font-features", &span.font_features);
    write_opt_bool(out, "italic", &span.italic);
    write_opt_bool(out, "underline", &span.underline);
    write_opt_bool(out, "strikethrough", &span.strikethrough);
    write_opt_property_value(out, "highlight", &span.highlight);
    write_opt_bool(out, "code", &span.code);
    write_opt_str(out, "link", &span.link);
    write_opt_str(out, "vertical-align", &span.vertical_align);
    write_opt_str(out, "footnote-ref", &span.footnote_ref);

    // Data-binding: emit `data-ref` and the resolved format attrs only when set,
    // so a span without data binding is byte-identical to before.
    write_opt_str(out, "data-ref", &span.data_ref);
    write_span_data_format(out, &span.data_format);

    out.push('\n');
}

/// Emit the `format` / `precision` / `locale` attributes for a span's optional
/// [`DataFormat`]. Emits nothing when `fmt` is `None`. Attribute order is
/// canonical (`format` then `precision` then `locale`) so round-trips are stable.
fn write_span_data_format(out: &mut String, fmt: &Option<DataFormat>) {
    let Some(fmt) = fmt else {
        return;
    };
    let (name, precision, locale): (&str, &Option<u8>, &Option<String>) = match fmt {
        DataFormat::Currency { locale, precision } => ("currency", precision, locale),
        DataFormat::Percent { precision } => ("percent", precision, &None),
        DataFormat::Number { precision } => ("number", precision, &None),
    };
    out.push_str(" format=\"");
    out.push_str(name);
    out.push('"');
    if let Some(p) = precision {
        out.push_str(" precision=");
        out.push_str(&p.to_string());
    }
    write_opt_str(out, "locale", locale);
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

/// Emit an `anchor x=(unit)N y=(unit)N ...` line for each path anchor.
///
/// The parent block is always emitted by the path writer; missing fields are
/// skipped so parse-time optionality remains lossless for invalid drafts.
pub(super) fn write_path_anchors(anchors: &[PathAnchor], out: &mut String, depth: usize) {
    for anchor in anchors {
        indent(out, depth);
        out.push_str("anchor");
        write_opt_dimension(out, "x", &anchor.x);
        write_opt_dimension(out, "y", &anchor.y);
        if let Some(kind) = &anchor.kind {
            write_opt_str_escaped(out, "kind", &Some(kind.kind_str().to_owned()));
        }
        write_opt_dimension(out, "in-x", &anchor.in_x);
        write_opt_dimension(out, "in-y", &anchor.in_y);
        write_opt_dimension(out, "out-x", &anchor.out_x);
        write_opt_dimension(out, "out-y", &anchor.out_y);
        out.push('\n');
    }
}

pub(super) fn write_path_subpaths(subpaths: &[PathSubpath], out: &mut String, depth: usize) {
    for subpath in subpaths {
        indent(out, depth);
        out.push_str("subpath");
        write_opt_bool(out, "closed", &subpath.closed);
        out.push_str(" {\n");
        write_path_anchors(&subpath.anchors, out, depth + 1);
        indent(out, depth);
        out.push_str("}\n");
    }
}
