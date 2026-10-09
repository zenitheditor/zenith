//! A node's authored attributes, read from its own source text: names,
//! values, units, and token bindings with their resolved values.

use std::collections::BTreeMap;

use kdl::{KdlDocument, KdlValue};
use serde::Serialize;
use serde_json::{Value, json};
use zenith_core::{PropertyValue, ResolvedToken, ResolvedValue, TokenType, Unit, dim_to_px};

/// One authored attribute.
#[derive(Debug, Serialize, PartialEq)]
pub(crate) struct Attribute {
    /// The attribute name; `#<i>` for the i-th positional argument.
    pub(crate) name: String,
    /// The value as written: string, number, bool, or null.
    pub(crate) value: Value,
    /// The type annotation: a unit (`px`, `pt`, `pct`, `deg`) or `token` /
    /// `data`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) unit: Option<String>,
    /// The token the attribute is bound to, with its resolved value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) token: Option<TokenBinding>,
    /// The value in px, for a px / pt dimension or a dimension token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) px: Option<f64>,
}

/// A token binding.
#[derive(Debug, Serialize, PartialEq)]
pub(crate) struct TokenBinding {
    /// The token id.
    pub(crate) id: String,
    /// The token type (`color`, `dimension`, …); absent when the token does
    /// not resolve.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) r#type: Option<String>,
    /// The resolved value; absent when the token does not resolve.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) value: Option<Value>,
}

/// The attributes of the first node in `source` (one node's own source
/// text), in source order. Children are not read. Empty when the text
/// does not parse on its own.
pub(crate) fn attributes(source: &str, tokens: &BTreeMap<String, ResolvedToken>) -> Vec<Attribute> {
    let Ok(doc) = source.parse::<KdlDocument>() else {
        return Vec::new();
    };
    let Some(node) = doc.nodes().first() else {
        return Vec::new();
    };
    let mut positional = 0;
    node.entries()
        .iter()
        .map(|entry| {
            let name = match entry.name() {
                Some(n) => n.value().to_owned(),
                None => {
                    positional += 1;
                    format!("#{}", positional - 1)
                }
            };
            let unit = entry.ty().map(|t| t.value().to_owned());
            let value = kdl_json(entry.value());
            let token = match (unit.as_deref(), entry.value()) {
                (Some("token"), KdlValue::String(id)) => Some(binding(id, tokens)),
                (Some(_) | None, _) => None,
            };
            let px = px_of(unit.as_deref(), entry.value(), token.as_ref(), tokens);
            Attribute {
                name,
                value,
                unit,
                token,
                px,
            }
        })
        .collect()
}

fn kdl_json(v: &KdlValue) -> Value {
    match v {
        KdlValue::String(s) => json!(s),
        KdlValue::Integer(i) => {
            i64::try_from(*i).map_or_else(|_| json!(i.to_string()), |i| json!(i))
        }
        KdlValue::Float(f) => json!(f),
        KdlValue::Bool(b) => json!(b),
        KdlValue::Null => Value::Null,
    }
}

fn number(v: &KdlValue) -> Option<f64> {
    match v {
        KdlValue::Integer(i) => i64::try_from(*i).ok().map(|i| i as f64),
        KdlValue::Float(f) => Some(*f),
        KdlValue::String(_) | KdlValue::Bool(_) | KdlValue::Null => None,
    }
}

fn px_of(
    unit: Option<&str>,
    v: &KdlValue,
    token: Option<&TokenBinding>,
    tokens: &BTreeMap<String, ResolvedToken>,
) -> Option<f64> {
    if token.is_some() {
        let KdlValue::String(id) = v else {
            return None;
        };
        return zenith_core::resolve_geometry_px(
            Some(&PropertyValue::TokenRef(id.clone())),
            tokens,
        );
    }
    let unit = Unit::from_annotation(unit?);
    dim_to_px(number(v)?, &unit)
}

fn binding(id: &str, tokens: &BTreeMap<String, ResolvedToken>) -> TokenBinding {
    let resolved = tokens.get(id);
    TokenBinding {
        id: id.to_owned(),
        r#type: resolved.map(|t| type_name(&t.token_type)),
        value: resolved.map(|t| resolved_json(&t.value)),
    }
}

/// The `.zen` spelling of a token type.
pub(crate) fn type_name(t: &TokenType) -> String {
    match t {
        TokenType::Color => "color".to_owned(),
        TokenType::Dimension => "dimension".to_owned(),
        TokenType::Number => "number".to_owned(),
        TokenType::FontFamily => "fontFamily".to_owned(),
        TokenType::FontWeight => "fontWeight".to_owned(),
        TokenType::Gradient => "gradient".to_owned(),
        TokenType::Shadow => "shadow".to_owned(),
        TokenType::Filter => "filter".to_owned(),
        TokenType::Mask => "mask".to_owned(),
        TokenType::Unknown(s) => s.clone(),
    }
}

/// A resolved token value as JSON: a hex string, a canonical dimension, a
/// number, a family name, a weight, or a short summary of a structured
/// value.
pub(crate) fn resolved_json(v: &ResolvedValue) -> Value {
    match v {
        ResolvedValue::Color(hex) => json!(hex),
        ResolvedValue::CmykColor { hex, c, m, y, k } => {
            json!({ "hex": hex, "cmyk": [c, m, y, k] })
        }
        ResolvedValue::Dimension(d) => json!(d.to_kdl_string()),
        ResolvedValue::Number(n) => json!(n),
        ResolvedValue::FontFamily(f) => json!(f),
        ResolvedValue::FontWeight(w) => json!(w),
        ResolvedValue::Gradient(g) => json!({
            "gradient": g.stops.iter().map(|(o, c)| json!([o, c])).collect::<Vec<_>>(),
            "angle": g.angle_deg,
        }),
        ResolvedValue::Shadow(s) => json!({ "shadow_layers": s.layers.len() }),
        ResolvedValue::Filter(f) => json!({ "filter_ops": f.ops.len() }),
        ResolvedValue::Mask(m) => json!({ "mask_feather": m.feather, "invert": m.invert }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlSource, TokenBlock, resolve_tokens};

    #[test]
    fn reads_units_tokens_and_positionals() {
        let src = r##"zenith version=1 {
  tokens format="zenith-token-v1" {
    token id="size.w" type="dimension" value=(px)40
    token id="c" type="color" value="#112233"
  }
  document id="d" { page id="p" w=(px)100 h=(px)100 { } }
}"##;
        let doc = zenith_core::KdlAdapter
            .parse(src.as_bytes())
            .expect("parse");
        let block: &TokenBlock = &doc.tokens;
        let tokens = resolve_tokens(block).resolved;
        let attrs = attributes(
            r#"rect id="r" x=(pt)9 w=(token)"size.w" fill=(token)"c" "pos" opacity=0.5"#,
            &tokens,
        );
        let by = |n: &str| attrs.iter().find(|a| a.name == n).expect(n);
        assert_eq!(by("x").px, Some(12.0));
        assert_eq!(by("x").unit.as_deref(), Some("pt"));
        assert_eq!(by("w").px, Some(40.0));
        let fill = by("fill").token.as_ref().expect("binding");
        assert_eq!(fill.value, Some(json!("#112233")));
        assert_eq!(fill.r#type.as_deref(), Some("color"));
        assert_eq!(by("#0").value, json!("pos"));
        assert_eq!(by("opacity").value, json!(0.5));
        assert!(attributes("{{{", &tokens).is_empty());
    }
}
