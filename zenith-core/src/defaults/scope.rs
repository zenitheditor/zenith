//! The style layers one node reads, and the merged styles lowering adds.

use std::collections::BTreeMap;

use crate::ast::defaults::{DefaultsBlock, DefaultsEntry, DefaultsKind};
use crate::ast::style::Style;
use crate::ast::value::PropertyValue;

/// The defaults in force for one subtree: the page block (absent for
/// components and masters) over the document block.
#[derive(Clone, Copy)]
pub(super) struct Scope<'a> {
    pub(super) styles: &'a BTreeMap<&'a str, &'a Style>,
    pub(super) page: Option<&'a DefaultsBlock>,
    /// The page whose content is lowered: `None` for the document-scope
    /// lowering of components and masters.
    pub(super) page_id: Option<&'a str>,
    pub(super) doc: &'a DefaultsBlock,
    /// Whether content pairing runs (the document or the page has a block).
    pub(super) pairing: bool,
}

/// One named style in a cascade.
#[derive(Clone, Copy)]
pub(super) struct Layer<'a> {
    pub(super) id: &'a str,
    pub(super) style: &'a Style,
}

impl<'a> Layer<'a> {
    fn get(self, key: &str) -> Option<&'a PropertyValue> {
        self.style.properties.get(key)
    }
}

/// The per-property cascade of one node: its own style over the page default
/// over the document default. A layer whose id names no declared style is
/// absent (validation reports the id).
#[derive(Clone, Copy)]
pub(super) struct Cascade<'a> {
    pub(super) node: Option<Layer<'a>>,
    pub(super) page: Option<Layer<'a>>,
    pub(super) doc: Option<Layer<'a>>,
}

impl<'a> Scope<'a> {
    fn layer(&self, id: Option<&str>) -> Option<Layer<'a>> {
        self.styles
            .get_key_value(id?)
            .map(|(id, style)| Layer { id, style })
    }

    fn entry(block: Option<&'a DefaultsBlock>, kind: DefaultsKind) -> Option<&'a DefaultsEntry> {
        block?.get(kind)
    }

    /// The `style` cascade of a node of `kind` whose own style id is
    /// `node_style`.
    pub(super) fn cascade(&self, kind: DefaultsKind, node_style: Option<&str>) -> Cascade<'a> {
        Cascade {
            node: self.layer(node_style),
            page: self.layer(Self::entry(self.page, kind).map(|e| e.style.as_str())),
            doc: self.layer(Self::entry(Some(self.doc), kind).map(|e| e.style.as_str())),
        }
    }

    /// The label `text-style` cascade of a node of `kind` whose own
    /// `text-style` is `node_text_style`.
    pub(super) fn label_cascade(
        &self,
        kind: DefaultsKind,
        node_text_style: Option<&str>,
    ) -> Cascade<'a> {
        let text_style = |block| {
            Self::entry(block, kind).and_then(|e: &'a DefaultsEntry| e.text_style.as_deref())
        };
        Cascade {
            node: self.layer(node_text_style),
            page: self.layer(text_style(self.page)),
            doc: self.layer(text_style(Some(self.doc))),
        }
    }
}

impl<'a> Cascade<'a> {
    /// Whether the node's own style carries `key`.
    pub(super) fn node_has(&self, key: &str) -> bool {
        self.node.and_then(|l| l.get(key)).is_some()
    }

    /// The node's own style value for `key`.
    pub(super) fn node_value(&self, key: &str) -> Option<&'a PropertyValue> {
        self.node.and_then(|l| l.get(key))
    }

    /// The defaults value for `key`: the page default, else the document
    /// default. `None` when the node's own style carries `key` (it wins).
    pub(super) fn fallback(&self, key: &str) -> Option<&'a PropertyValue> {
        if self.node_has(key) {
            return None;
        }
        self.page
            .and_then(|l| l.get(key))
            .or_else(|| self.doc.and_then(|l| l.get(key)))
    }

    /// The enum literal of the defaults value for `key` (`align`, `v-align`).
    pub(super) fn fallback_enum(&self, key: &str) -> Option<String> {
        literal(self.fallback(key)?)
    }

    /// Whether either default layer exists.
    pub(super) fn has_defaults(&self) -> bool {
        self.page.is_some() || self.doc.is_some()
    }

    /// The distinct layer ids, node first.
    fn ids(&self) -> Vec<&'a str> {
        let mut ids: Vec<&'a str> = Vec::with_capacity(3);
        for layer in [self.node, self.page, self.doc].into_iter().flatten() {
            if !ids.contains(&layer.id) {
                ids.push(layer.id);
            }
        }
        ids
    }

    /// Every property of the cascade, merged per key (node over page over
    /// document). `skip_default` drops a key from the default layers only.
    fn merged(&self, skip_default: Option<&str>) -> BTreeMap<String, PropertyValue> {
        let mut props: BTreeMap<String, PropertyValue> = self
            .node
            .map(|l| l.style.properties.clone())
            .unwrap_or_default();
        for layer in [self.page, self.doc].into_iter().flatten() {
            for (key, value) in &layer.style.properties {
                if skip_default == Some(key.as_str()) {
                    continue;
                }
                props.entry(key.clone()).or_insert_with(|| value.clone());
            }
        }
        props
    }
}

/// The enum literal a style value holds, or `None` for any other form (the
/// scene ignores a non-literal enum value).
pub(super) fn literal(value: &PropertyValue) -> Option<String> {
    match value {
        PropertyValue::Literal(v) => Some(v.clone()),
        PropertyValue::TokenRef(_) | PropertyValue::Dimension(_) | PropertyValue::DataRef(_) => {
            None
        }
    }
}

/// Styles lowering adds to the document: one per distinct cascade that no
/// single declared style expresses. Keyed by id, so the output order is
/// deterministic and a repeated cascade shares one style.
#[derive(Default)]
pub(super) struct Synth {
    styles: BTreeMap<String, Style>,
}

/// Extra inputs of a merged style beyond its cascade.
#[derive(Clone, Copy, Default)]
pub(super) struct MergeExtras<'a> {
    /// A fill placed between the node layer and the default layers (the
    /// ambient content pair).
    pub(super) fill: Option<&'a PropertyValue>,
    /// A key the default layers do not contribute.
    pub(super) skip_default: Option<&'a str>,
}

impl Synth {
    /// The style id that expresses `cascade` (plus `extras`) as one style.
    ///
    /// A cascade of at most one layer and no extras is that layer's own id.
    /// Anything else becomes a merged style whose id names its inputs.
    pub(super) fn style_for(
        &mut self,
        role: &str,
        cascade: &Cascade<'_>,
        extras: MergeExtras<'_>,
    ) -> Option<String> {
        let ids = cascade.ids();
        let paired_fill = extras.fill.filter(|_| !cascade.node_has("fill"));
        if paired_fill.is_none() && extras.skip_default.is_none() && ids.len() <= 1 {
            return ids.first().map(|id| (*id).to_owned());
        }
        let mut props = cascade.merged(extras.skip_default);
        if let Some(fill) = paired_fill {
            props.insert("fill".to_owned(), fill.clone());
        }
        let layer_id = |l: Option<Layer<'_>>| l.map_or("-", |l| l.id).to_owned();
        let fill_id = match paired_fill {
            Some(PropertyValue::TokenRef(id)) => id.clone(),
            Some(
                PropertyValue::Literal(_) | PropertyValue::Dimension(_) | PropertyValue::DataRef(_),
            )
            | None => "-".to_owned(),
        };
        let id = format!(
            "defaults:{role}:{}:{}:{}:{fill_id}:{}",
            layer_id(cascade.node),
            layer_id(cascade.page),
            layer_id(cascade.doc),
            extras.skip_default.unwrap_or("-"),
        );
        self.styles.entry(id.clone()).or_insert_with(|| Style {
            id: id.clone(),
            properties: props,
            unknown_props: BTreeMap::new(),
            source_span: None,
        });
        Some(id)
    }

    /// The merged styles, in id order.
    pub(super) fn into_styles(self) -> impl Iterator<Item = Style> {
        self.styles.into_values()
    }
}
