//! The `defaults { … }` block transform (document and page scope).
//!
//! Each child row is `<kind> style="id" [text-style="id"]`. A row whose kind
//! is a [`DefaultsKind`] and is not yet declared lands in
//! [`DefaultsBlock::entries`]. Every other row lands in
//! [`DefaultsBlock::rejected`] with its reason, so validation reports it and
//! the formatter keeps it. Children of a row are recorded by the
//! `unknown_children` pass as `block.unknown_child`.

use kdl::KdlNode;

use crate::ast::{
    DEFAULTS_ENTRY_PROPS, DEFAULTS_UNSUPPORTED_KINDS, DefaultsBlock, DefaultsEntry, DefaultsKind,
    DefaultsRejection, RejectedDefaultsEntry,
};
use crate::error::{ParseError, ParseErrorCode};

use super::helpers::{
    collect_unknown_props, node_span, optional_string_prop, optional_string_prop_aliased,
};

/// Transform a `defaults` block node into a [`DefaultsBlock`].
pub(super) fn transform_defaults(node: &KdlNode) -> Result<DefaultsBlock, ParseError> {
    let mut block = DefaultsBlock {
        source_span: node_span(node),
        ..DefaultsBlock::default()
    };
    let Some(children) = node.children() else {
        return Ok(block);
    };
    for child in children.nodes() {
        let name = child.name().value();
        let entry = transform_entry(child)?;
        match DefaultsKind::from_name(name) {
            Some(kind) if block.entries.contains_key(&kind) => {
                block.rejected.push(RejectedDefaultsEntry {
                    name: name.to_owned(),
                    reason: DefaultsRejection::DuplicateKind(kind),
                    entry,
                });
            }
            Some(kind) => {
                block.entries.insert(kind, entry);
            }
            None => {
                let reason = if DEFAULTS_UNSUPPORTED_KINDS.contains(&name) {
                    DefaultsRejection::UnsupportedKind
                } else {
                    DefaultsRejection::UnknownKind
                };
                block.rejected.push(RejectedDefaultsEntry {
                    name: name.to_owned(),
                    reason,
                    entry,
                });
            }
        }
    }
    Ok(block)
}

/// Read one row's attributes. `style` is required.
fn transform_entry(node: &KdlNode) -> Result<DefaultsEntry, ParseError> {
    let source_span = node_span(node);
    let Some(style) = optional_string_prop(node, "style") else {
        let message = format!(
            "defaults row `{}` is missing the required string property `style`; \
             add style=\"<style id>\"",
            node.name().value()
        );
        return Err(match source_span {
            Some(span) => {
                ParseError::with_span(ParseErrorCode::InvalidPropertyValue, span, message)
            }
            None => ParseError::spanless(ParseErrorCode::InvalidPropertyValue, message),
        });
    };
    let text_style = optional_string_prop_aliased(node, "text-style", "text_style");
    let mut unknown_props = collect_unknown_props(node, DEFAULTS_ENTRY_PROPS);
    unknown_props.remove("text_style");
    Ok(DefaultsEntry {
        style: style.to_owned(),
        text_style: text_style.map(str::to_owned),
        unknown_props,
        source_span,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(src: &str) -> DefaultsBlock {
        let doc: kdl::KdlDocument = src.parse().expect("kdl");
        let node = doc.nodes().first().expect("defaults node");
        transform_defaults(node).expect("transform")
    }

    #[test]
    fn rows_split_into_entries_and_rejections() {
        let b = block(
            r#"defaults {
                text style="body"
                shape style="box" text-style="box.label"
                text style="again"
                instance style="x"
                txet style="y"
            }"#,
        );
        assert_eq!(b.entries.len(), 2);
        assert_eq!(
            b.get(DefaultsKind::Text).map(|e| e.style.as_str()),
            Some("body")
        );
        assert_eq!(
            b.get(DefaultsKind::Shape)
                .and_then(|e| e.text_style.as_deref()),
            Some("box.label")
        );
        let reasons: Vec<_> = b
            .rejected
            .iter()
            .map(|r| (r.name.as_str(), r.reason))
            .collect();
        assert_eq!(
            reasons,
            [
                ("text", DefaultsRejection::DuplicateKind(DefaultsKind::Text)),
                ("instance", DefaultsRejection::UnsupportedKind),
                ("txet", DefaultsRejection::UnknownKind),
            ]
        );
    }

    #[test]
    fn missing_style_is_a_parse_error() {
        let doc: kdl::KdlDocument = r#"defaults { text text-style="x" }"#.parse().expect("kdl");
        let node = doc.nodes().first().expect("node");
        let err = transform_defaults(node).expect_err("missing style");
        assert!(err.to_string().contains("style"), "{err}");
    }

    #[test]
    fn unknown_attributes_are_kept() {
        let b = block(r#"defaults { rect style="s" fill="x" }"#);
        let entry = b.get(DefaultsKind::Rect).expect("rect");
        assert!(entry.unknown_props.contains_key("fill"));
    }
}
