//! Translate one variant `override` into transaction ops.
//!
//! Each override becomes ops in this order: `set_visible`, `set_geometry`,
//! `set_fill`, `replace_text`, `set_node_token` (font-size), `set_layout`.
//! The layout ops re-run auto-layout at the variant size.

use zenith_core::{Dimension, PropertyValue, SizeKeyword, VariantOverride, dim_to_px};
use zenith_tx::{LayoutDim, LayoutEdit, Op, OpSpan, SizeInput};

/// The ops for `ov`, in the order the module doc lists.
///
/// Returns an error string when a value cannot become an op: a `font-size`
/// that is not a token ref, or a layout dimension that is a bare string or
/// a data ref. Validation reports these values first, so the error is a
/// fallback for an unvalidated document.
pub(super) fn override_ops(ov: &VariantOverride) -> Result<Vec<Op>, String> {
    let mut ops: Vec<Op> = Vec::new();
    if let Some(visible) = ov.visible {
        ops.push(Op::SetVisible {
            node: ov.node.clone(),
            visible,
        });
    }
    let w = size_input(ov.w.as_ref(), ov.w_keyword);
    let h = size_input(ov.h.as_ref(), ov.h_keyword);
    if ov.x.is_some() || ov.y.is_some() || w.is_some() || h.is_some() {
        ops.push(Op::SetGeometry {
            node: ov.node.clone(),
            x: ov.x.as_ref().and_then(px).map(Some),
            y: ov.y.as_ref().and_then(px).map(Some),
            w: w.map(Some),
            h: h.map(Some),
            rotate: None,
        });
    }
    if let Some(fill) = &ov.fill {
        ops.push(Op::SetFill {
            node: ov.node.clone(),
            fill: property_value_to_fill_str(fill),
        });
    }
    if let Some(text) = &ov.text {
        ops.push(Op::ReplaceText {
            node: ov.node.clone(),
            spans: vec![OpSpan {
                text: text.clone(),
                fill: None,
                font_weight: None,
                italic: None,
                underline: None,
                strikethrough: None,
                vertical_align: None,
                footnote_ref: None,
            }],
        });
    }
    if let Some(font_size) = &ov.font_size {
        let PropertyValue::TokenRef(token) = font_size else {
            return Err(format!(
                "override for node '{}': font-size must be a dimension token ref such as \
                 (token)\"size.title\"",
                ov.node
            ));
        };
        ops.push(Op::SetNodeToken {
            node: ov.node.clone(),
            property: "font-size".to_owned(),
            token: Some(token.clone()),
        });
    }
    if ov.sets_frame_layout() {
        ops.push(Op::SetLayout(Box::new(layout_edit(ov)?)));
    }
    Ok(ops)
}

/// A dimension in document pixels, or `None` for a unit without a px value.
fn px(d: &Dimension) -> Option<f64> {
    dim_to_px(d.value, &d.unit)
}

/// The `set_geometry` size for an override axis: the keyword when set, else
/// the px value.
fn size_input(value: Option<&Dimension>, keyword: Option<SizeKeyword>) -> Option<SizeInput> {
    match keyword {
        Some(k) => Some(SizeInput::Keyword(k.as_str().to_owned())),
        None => value.and_then(px).map(SizeInput::Px),
    }
}

/// The `set_layout` body for the frame fields of `ov`.
fn layout_edit(ov: &VariantOverride) -> Result<LayoutEdit, String> {
    let c = &ov.container;
    let dim = |prop: &str, value: &Option<PropertyValue>| -> Result<_, String> {
        value
            .as_ref()
            .map(|pv| layout_dim(&ov.node, prop, pv).map(Some))
            .transpose()
    };
    Ok(LayoutEdit {
        node: ov.node.clone(),
        layout: ov.layout.as_ref().map(|k| Some(k.as_str().to_owned())),
        gap: dim("gap", &c.gap)?,
        wrap_gap: dim("wrap-gap", &c.wrap_gap)?,
        padding: dim("padding", &c.padding)?,
        padding_x: dim("padding-x", &c.padding_x)?,
        padding_y: dim("padding-y", &c.padding_y)?,
        padding_top: dim("padding-top", &c.padding_top)?,
        padding_right: dim("padding-right", &c.padding_right)?,
        padding_bottom: dim("padding-bottom", &c.padding_bottom)?,
        padding_left: dim("padding-left", &c.padding_left)?,
        justify: c.justify.as_ref().map(|j| Some(j.as_str().to_owned())),
        align: c.align.as_ref().map(|a| Some(a.as_str().to_owned())),
        wrap: c.wrap.map(Some),
        ..LayoutEdit::default()
    })
}

/// The `set_layout` input for one layout dimension value.
fn layout_dim(node: &str, prop: &str, pv: &PropertyValue) -> Result<LayoutDim, String> {
    match pv {
        PropertyValue::Dimension(d) => Ok(LayoutDim::Text(d.to_kdl_string())),
        PropertyValue::TokenRef(id) => Ok(LayoutDim::Text(id.clone())),
        PropertyValue::Literal(_) | PropertyValue::DataRef(_) => Err(format!(
            "override for node '{node}': {prop} must be a dimension such as (px)24 or a \
             dimension token ref"
        )),
    }
}

/// Extract a string to pass to [`Op::SetFill`] from a [`PropertyValue`].
///
/// [`Op::SetFill`] takes a token id and stores it as a token ref. A literal
/// or dimension fill passes through as its raw string. Post-validation then
/// rejects it as `token.unknown_reference`, so that variant fails and no
/// corrupt document is written.
fn property_value_to_fill_str(pv: &PropertyValue) -> String {
    match pv {
        PropertyValue::TokenRef(id) => id.clone(),
        PropertyValue::Literal(s) => s.clone(),
        PropertyValue::Dimension(d) => d.to_kdl_string(),
        PropertyValue::DataRef(path) => path.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{Document, KdlAdapter, KdlSource};

    fn overrides(body: &str) -> Document {
        let src = format!(
            r##"zenith version=1 {{
  project id="p" name="P"
  tokens format="zenith-token-v1" {{
    token id="size.t" type="dimension" value=(px)40
  }}
  styles {{}}
  variants {{
    variant id="v" source="pg" w=(px)200 h=(px)100 {{
      {body}
    }}
  }}
  document id="d" title="D" {{
    page id="pg" w=(px)100 h=(px)100 {{
      frame id="f" x=(px)0 y=(px)0 w=(px)100 h=(px)100 layout="column" {{
        text id="t" w=(px)80 {{ span "a" }}
      }}
    }}
  }}
}}
"##
        );
        KdlAdapter.parse(src.as_bytes()).expect("fixture parses")
    }

    fn ops_for(body: &str) -> Result<Vec<Op>, String> {
        let doc = overrides(body);
        override_ops(&doc.variants[0].overrides[0])
    }

    #[test]
    fn layout_fields_become_one_set_layout_op() {
        let ops = ops_for(
            r#"override node="f" layout="row" gap=(token)"size.t" padding-x=(px)12 justify="center" align="start" wrap=#true"#,
        )
        .expect("ops build");
        assert_eq!(ops.len(), 1);
        let Op::SetLayout(edit) = &ops[0] else {
            panic!("expected set_layout, got {ops:?}");
        };
        assert_eq!(edit.node, "f");
        assert_eq!(edit.layout, Some(Some("row".to_owned())));
        assert_eq!(edit.gap, Some(Some(LayoutDim::Text("size.t".to_owned()))));
        assert_eq!(
            edit.padding_x,
            Some(Some(LayoutDim::Text("(px)12".to_owned())))
        );
        assert_eq!(edit.padding, None);
        assert_eq!(edit.justify, Some(Some("center".to_owned())));
        assert_eq!(edit.align, Some(Some("start".to_owned())));
        assert_eq!(edit.wrap, Some(Some(true)));
        assert_eq!(edit.clip, None);
    }

    #[test]
    fn font_size_token_becomes_set_node_token() {
        let ops = ops_for(r#"override node="t" font-size=(token)"size.t""#).expect("ops build");
        assert_eq!(
            ops,
            [Op::SetNodeToken {
                node: "t".to_owned(),
                property: "font-size".to_owned(),
                token: Some("size.t".to_owned()),
            }]
        );
    }

    #[test]
    fn font_size_literal_is_an_error() {
        let err = ops_for(r#"override node="t" font-size=(px)40"#).expect_err("literal fails");
        assert!(
            err.contains("font-size must be a dimension token ref"),
            "{err}"
        );
    }

    #[test]
    fn size_keyword_becomes_set_geometry_keyword() {
        let ops = ops_for(r#"override node="t" w="fill" h=(px)30"#).expect("ops build");
        let [Op::SetGeometry { w, h, x, y, .. }] = ops.as_slice() else {
            panic!("expected one set_geometry, got {ops:?}");
        };
        assert_eq!(w, &Some(Some(SizeInput::Keyword("fill".to_owned()))));
        assert_eq!(h, &Some(Some(SizeInput::Px(30.0))));
        assert_eq!((x, y), (&None, &None));
    }

    #[test]
    fn no_fields_build_no_ops() {
        assert_eq!(ops_for(r#"override node="t""#).expect("ops build"), []);
    }
}
