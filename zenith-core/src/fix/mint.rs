//! Mint design tokens for raw literals that match no declared token.
//!
//! Ids are semantic and derive from the value alone, so the same value always
//! mints the same id and one minted token serves every node:
//!
//! - color → `color.custom.<hex>` (`#ffffff` → `color.custom.ffffff`)
//! - dimension → `<role>.<n>[unit]` (`size.96`, `radius.24`, `stroke.2`,
//!   `space.8`, `size.12pt`, `size.10_5`)
//! - font weight → `weight.<n>`
//! - font family → `font.<slug>` (`Playfair Display` → `font.playfair-display`)
//!
//! An id already declared with another value gets a `-2`, `-3`, … suffix.

use std::collections::{BTreeMap, BTreeSet};

use kdl::{KdlDocument, KdlEntry, KdlNode};

use crate::ast::value::Dimension;
use crate::suggest::{LiteralValue, common_dotted_prefix, dimension_role, unit_suffix};

use super::locate::{node_id, quote};

/// A token created by `zenith fix`.
#[derive(Debug, Clone, PartialEq)]
pub struct MintedToken {
    /// The new token id.
    pub id: String,
    /// The token type name (`color`, `dimension`, `fontWeight`, `fontFamily`).
    pub token_type: String,
    /// The value as `.zen` source text (`"#ffffff"`, `(px)96`, `700`).
    pub value: String,
}

/// Mints ids for one pass and remembers what it minted.
pub(super) struct Minter {
    /// Every token id declared in the document.
    declared: BTreeSet<String>,
    /// Minted id → value key, for reuse within the pass.
    by_key: BTreeMap<String, String>,
    /// Minted tokens in mint order.
    pub minted: Vec<MintedToken>,
}

impl Minter {
    pub(super) fn new(declared: BTreeSet<String>) -> Self {
        Self {
            declared,
            by_key: BTreeMap::new(),
            minted: Vec::new(),
        }
    }

    /// The id for `lit` on property `prop`: a token minted earlier in this
    /// pass for the same value, else a new one. `None` when the value has no
    /// id form.
    pub(super) fn mint(&mut self, lit: &LiteralValue, prop: &str) -> Option<String> {
        let (base, token_type, value) = describe(lit, prop)?;
        let key = format!("{token_type}={value}");
        if let Some(id) = self.by_key.get(&key) {
            return Some(id.clone());
        }
        let mut id = base.clone();
        let mut n = 2;
        while self.declared.contains(&id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        self.declared.insert(id.clone());
        self.by_key.insert(key, id.clone());
        self.minted.push(MintedToken {
            id: id.clone(),
            token_type: token_type.to_owned(),
            value,
        });
        Some(id)
    }
}

/// `(base id, token type, value source text)` for a literal.
fn describe(lit: &LiteralValue, prop: &str) -> Option<(String, &'static str, String)> {
    match lit {
        LiteralValue::Color(hex) => Some((
            format!("color.custom.{}", hex.trim_start_matches('#')),
            "color",
            format!("\"{hex}\""),
        )),
        LiteralValue::Dimension(d) => {
            let prefix = dimension_role(prop).map_or("size", |r| r.mint);
            Some((
                format!("{prefix}.{}", dimension_slug(d)),
                "dimension",
                d.to_kdl_string(),
            ))
        }
        LiteralValue::FontWeight(w) => Some((format!("weight.{w}"), "fontWeight", w.to_string())),
        LiteralValue::FontFamily(name) => {
            let slug = slug(name);
            if slug.is_empty() {
                return None;
            }
            Some((format!("font.{slug}"), "fontFamily", quote(name)))
        }
    }
}

/// Id segment for a dimension: `96`, `10_5`, `neg4`, `12pt`.
fn dimension_slug(d: &Dimension) -> String {
    let kdl = d.to_kdl_string();
    let number = kdl.split_once(')').map_or(kdl.as_str(), |(_, n)| n);
    let number = match number.strip_prefix('-') {
        Some(rest) => format!("neg{rest}"),
        None => number.to_owned(),
    };
    format!("{}{}", number.replace('.', "_"), unit_suffix(&d.unit))
}

/// Lowercase ASCII slug: alphanumerics kept, every other run becomes `-`.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}

/// Build the `token` node for a minted token.
fn token_node(t: &MintedToken) -> Option<KdlNode> {
    // Parse the value through KDL so annotations and escapes stay exact.
    let snippet = format!("v value={}", t.value);
    let parsed: KdlDocument = snippet.parse().ok()?;
    let value_entry = parsed.nodes().first()?.entries().first()?.clone();
    let mut node = KdlNode::new("token");
    node.push(KdlEntry::new_prop("id", t.id.clone()));
    node.push(KdlEntry::new_prop("type", t.token_type.clone()));
    let mut value_entry = value_entry;
    value_entry.clear_format();
    node.push(value_entry);
    Some(node)
}

/// Insert minted tokens into the `tokens` block under the root node.
///
/// Each token goes after the last declared token that shares the most
/// leading id segments with it (at least one), else at the block end. The
/// block is created when the document has none. Returns `false` when the
/// document has no root node.
pub(super) fn insert_tokens(doc: &mut KdlDocument, minted: &[MintedToken]) -> bool {
    let Some(root) = doc.nodes_mut().first_mut() else {
        return false;
    };
    let children = root.ensure_children();
    let block_index = children
        .nodes()
        .iter()
        .position(|n| n.name().value() == "tokens");
    let block_index = match block_index {
        Some(i) => i,
        None => {
            let mut block = KdlNode::new("tokens");
            block.push(KdlEntry::new_prop("format", "zenith-token-v1"));
            children.nodes_mut().push(block);
            children.nodes().len() - 1
        }
    };
    let Some(block) = children.nodes_mut().get_mut(block_index) else {
        return false;
    };
    let tokens = block.ensure_children();
    for t in minted {
        let Some(node) = token_node(t) else {
            continue;
        };
        let mut best: Option<(usize, usize)> = None; // (index, shared)
        for (i, existing) in tokens.nodes().iter().enumerate() {
            if existing.name().value() != "token" {
                continue;
            }
            let shared = node_id(existing).map_or(0, |id| common_dotted_prefix(&t.id, id));
            if shared > 0 && best.is_none_or(|(_, s)| shared >= s) {
                best = Some((i, shared));
            }
        }
        let at = best.map_or(tokens.nodes().len(), |(i, _)| i + 1);
        tokens.nodes_mut().insert(at, node);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::value::Unit;

    fn px(v: f64) -> LiteralValue {
        LiteralValue::Dimension(Dimension {
            value: v,
            unit: Unit::Px,
        })
    }

    #[test]
    fn mint_ids_by_value_and_role() {
        let mut m = Minter::new(BTreeSet::new());
        let color = LiteralValue::color("#FFFFFF").expect("hex");
        assert_eq!(
            m.mint(&color, "fill").as_deref(),
            Some("color.custom.ffffff")
        );
        assert_eq!(m.mint(&px(96.0), "font-size").as_deref(), Some("size.96"));
        assert_eq!(m.mint(&px(24.0), "radius").as_deref(), Some("radius.24"));
        assert_eq!(
            m.mint(&px(2.0), "stroke-width").as_deref(),
            Some("stroke.2")
        );
        assert_eq!(m.mint(&px(10.5), "x-gap").as_deref(), Some("size.10_5"));
        assert_eq!(
            m.mint(&LiteralValue::FontWeight(700), "font-weight")
                .as_deref(),
            Some("weight.700")
        );
        let family = LiteralValue::font_family("Playfair  Display!").expect("name");
        assert_eq!(
            m.mint(&family, "font-family").as_deref(),
            Some("font.playfair-display")
        );
    }

    #[test]
    fn mint_reuses_same_value_and_dodges_taken_ids() {
        let declared = BTreeSet::from(["size.96".to_owned()]);
        let mut m = Minter::new(declared);
        assert_eq!(m.mint(&px(96.0), "font-size").as_deref(), Some("size.96-2"));
        assert_eq!(m.mint(&px(96.0), "font-size").as_deref(), Some("size.96-2"));
        assert_eq!(m.minted.len(), 1);
    }

    #[test]
    fn insert_places_token_after_its_group() {
        let src = r##"zenith {
  tokens format="zenith-token-v1" {
    token id="color.a" type="color" value="#000000"
    token id="size.h1" type="dimension" value=(px)64
  }
}"##;
        let mut doc: KdlDocument = src.parse().expect("kdl");
        let minted = vec![
            MintedToken {
                id: "color.custom.ffffff".into(),
                token_type: "color".into(),
                value: "\"#ffffff\"".into(),
            },
            MintedToken {
                id: "weight.700".into(),
                token_type: "fontWeight".into(),
                value: "700".into(),
            },
        ];
        assert!(insert_tokens(&mut doc, &minted));
        doc.autoformat();
        let out = doc.to_string();
        let a = out.find("color.a").expect("a");
        let c = out.find("color.custom.ffffff").expect("c");
        let s = out.find("size.h1").expect("s");
        let w = out.find("weight.700").expect("w");
        assert!(a < c && c < s && s < w, "{out}");
    }
}
