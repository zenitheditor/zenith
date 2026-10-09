//! Where a node's style comes from: its own `style`, else the page
//! `defaults` entry for its kind, else the document `defaults` entry.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Value, json};
use zenith_core::{DefaultsKind, Document, Node, PropertyValue, ResolvedToken};

use super::attrs::resolved_json;
use crate::doc::tree::{Located, Root};

/// The style a node draws with.
#[derive(Debug, Serialize)]
pub(crate) struct StyleSource {
    /// `node` (its own `style`), `page_defaults`, or `document_defaults`.
    pub(crate) source: &'static str,
    /// The style id.
    pub(crate) id: String,
    /// `false` when no style with that id is defined.
    pub(crate) defined: bool,
    /// The style's properties: name → `{value, token?, resolved?}`.
    pub(crate) properties: BTreeMap<String, Value>,
}

/// The style source of the node at `located`, or `None` when it uses no
/// style.
pub(crate) fn style_source(
    doc: &Document,
    located: &Located<'_>,
    tokens: &BTreeMap<String, ResolvedToken>,
) -> Option<StyleSource> {
    let node: &Node = located.node;
    let (source, id) = match node.style_ref() {
        Some(id) => ("node", id.to_owned()),
        None => {
            let kind = DefaultsKind::from_name(node.kind_str())?;
            let page_entry = match located.root {
                Root::Page(i) => doc.body.pages.get(i).and_then(|p| p.defaults.get(kind)),
                Root::Master(_) => None,
            };
            match page_entry {
                Some(entry) => ("page_defaults", entry.style.clone()),
                None => ("document_defaults", doc.defaults.get(kind)?.style.clone()),
            }
        }
    };
    let style = doc.styles.styles.iter().find(|s| s.id == id);
    let properties = style
        .map(|s| {
            s.properties
                .iter()
                .map(|(k, v)| (k.clone(), property_json(v, tokens)))
                .collect()
        })
        .unwrap_or_default();
    Some(StyleSource {
        source,
        id,
        defined: style.is_some(),
        properties,
    })
}

fn property_json(v: &PropertyValue, tokens: &BTreeMap<String, ResolvedToken>) -> Value {
    match v {
        PropertyValue::TokenRef(id) => json!({
            "token": id,
            "resolved": tokens.get(id).map(|t| resolved_json(&t.value)),
        }),
        PropertyValue::Literal(s) => json!({ "value": s }),
        PropertyValue::Dimension(d) => json!({ "value": d.to_kdl_string() }),
        PropertyValue::DataRef(s) => json!({ "data": s }),
    }
}
