//! The document-level `variants { … }` block writer.

use crate::ast::VariantDef;

use super::nodes::{write_box_dimension, write_layout_container};
use super::{
    fmt_dimension, fmt_unknown_property, indent, write_opt_bool, write_opt_dimension,
    write_opt_property_value, write_opt_str_escaped,
};

/// Emit the `variants { … }` block.
///
/// Stable position: after `provenance`, before `recipes`. The block is
/// emitted only when at least one variant is declared. A document without
/// variants keeps its canonical form. Each variant emits:
///
/// ```text
/// variant id="…" source="…" w=(px)N h=(px)N {
///   override node="…" visible=#false x=(px)N y=(px)N w=(px)N h=(px)N fill=… text="…" font-size=… layout="…" gap=… …
/// }
/// ```
///
/// Override props emit in this order: `visible`, `x`, `y`, `w`, `h`, `fill`,
/// `text`, `font-size`, `layout`, then the frame container attributes in
/// their frame order (`gap`, `wrap-gap`, `padding*`, `justify`, `align`,
/// `wrap`). A `None` prop emits nothing. A `w`/`h` keyword emits as
/// `w="fill"`. Unknown props follow the known ones in sorted key order. A
/// variant with no overrides still emits a brace block.
pub(super) fn write_variants_block(variants: &[VariantDef], out: &mut String, depth: usize) {
    if variants.is_empty() {
        return;
    }
    indent(out, depth);
    out.push_str("variants {\n");
    for def in variants {
        indent(out, depth + 1);
        out.push_str("variant id=\"");
        out.push_str(&def.id);
        out.push_str("\" source=\"");
        out.push_str(&def.source);
        out.push_str("\" w=");
        out.push_str(&fmt_dimension(&def.w));
        out.push_str(" h=");
        out.push_str(&fmt_dimension(&def.h));
        // Unknown props on the variant node itself, in sorted key order.
        for (key, prop) in &def.unknown_props {
            out.push(' ');
            out.push_str(key);
            out.push('=');
            out.push_str(&fmt_unknown_property(prop));
        }
        out.push_str(" {\n");
        for ov in &def.overrides {
            indent(out, depth + 2);
            out.push_str("override node=\"");
            out.push_str(&ov.node);
            out.push('"');
            write_opt_bool(out, "visible", &ov.visible);
            write_opt_dimension(out, "x", &ov.x);
            write_opt_dimension(out, "y", &ov.y);
            write_box_dimension(out, "w", &ov.w, ov.w_keyword);
            write_box_dimension(out, "h", &ov.h, ov.h_keyword);
            write_opt_property_value(out, "fill", &ov.fill);
            write_opt_str_escaped(out, "text", &ov.text);
            write_opt_property_value(out, "font-size", &ov.font_size);
            let layout = ov.layout.as_ref().map(|k| k.as_str().to_owned());
            write_opt_str_escaped(out, "layout", &layout);
            write_layout_container(out, &ov.container);
            // Unknown props on the override node, in sorted key order.
            for (key, prop) in &ov.unknown_props {
                out.push(' ');
                out.push_str(key);
                out.push('=');
                out.push_str(&fmt_unknown_property(prop));
            }
            out.push('\n');
        }
        indent(out, depth + 1);
        out.push_str("}\n");
    }
    indent(out, depth);
    out.push_str("}\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KdlAdapter, KdlSource};

    #[test]
    fn writes_override_keys_in_canonical_order() {
        let src = r##"zenith version=1 {
  project id="p" name="P"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  variants {
    variant id="v" source="pg" w=(px)10 h=(px)10 {
      override node="f" wrap=#true align="end" padding-y=(px)2 layout="row" font-size=(token)"s" h="hug" visible=#true
    }
  }
  document id="d" title="D" {
    page id="pg" w=(px)10 h=(px)10 {
    }
  }
}
"##;
        let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
        let mut out = String::new();
        write_variants_block(&doc.variants, &mut out, 0);
        assert!(
            out.contains(
                r#"override node="f" visible=#true h="hug" font-size=(token)"s" layout="row" padding-y=(px)2 align="end" wrap=#true"#
            ),
            "{out}"
        );
    }

    #[test]
    fn empty_variants_write_nothing() {
        let mut out = String::new();
        write_variants_block(&[], &mut out, 0);
        assert!(out.is_empty());
    }
}
