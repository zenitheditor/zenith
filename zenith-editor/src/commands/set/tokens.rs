//! Raw inspector values as tokens. Zenith binds visual properties to
//! design tokens (a raw literal is `token.raw_visual_literal`), so a raw
//! value binds the existing token that holds it, or a new token the same
//! transaction creates.

use std::collections::BTreeSet;

use serde_json::Value;
use zenith_core::{Document, ResolvedValue, dim_to_px, resolve_tokens};
use zenith_tx::Op;

use super::params::TokenOrValue;
use crate::error::EditorError;

/// A raw value of one token type.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Raw {
    /// Canonical lowercase `#rrggbb` or `#rrggbbaa` (alpha `ff` dropped).
    Color(String),
    /// A size in px.
    Px(f64),
    /// A font family name.
    Family(String),
    /// A font weight, 100 to 900.
    Weight(f64),
}

/// The token type a property takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Color,
    Dimension,
    Family,
    Weight,
}

/// Tokens chosen or created by one `node.set`: new tokens are reused when
/// two fields ask for the same value.
pub(super) struct Tokens<'d> {
    doc: &'d Document,
    taken: BTreeSet<String>,
    created: Vec<(Raw, String)>,
    /// The `create_token` ops, in order; they run before the setters.
    pub(super) ops: Vec<Op>,
}

impl<'d> Tokens<'d> {
    pub(super) fn new(doc: &'d Document) -> Self {
        Self {
            doc,
            taken: doc.tokens.tokens.iter().map(|t| t.id.clone()).collect(),
            created: Vec::new(),
            ops: Vec::new(),
        }
    }

    /// The token id `field` binds for `input`: the id itself, or the token
    /// that holds the raw value (created when none does). `property` names
    /// a new dimension token (`size.<property>.<n>`).
    ///
    /// # Errors
    ///
    /// `editor.invalid_params` for a raw value of the wrong shape.
    pub(super) fn bind(
        &mut self,
        field: &str,
        property: &str,
        kind: Kind,
        input: &TokenOrValue,
    ) -> Result<String, EditorError> {
        let value = match input {
            TokenOrValue::Token(id) => return Ok(id.clone()),
            TokenOrValue::Value { value } => value,
        };
        let raw = parse(field, kind, value)?;
        if let Some(id) = self.existing(&raw) {
            return Ok(id);
        }
        if let Some((_, id)) = self.created.iter().find(|(r, _)| *r == raw) {
            return Ok(id.clone());
        }
        let base = base_id(property, &raw);
        let mut id = base.clone();
        let mut n = 2_u32;
        while self.taken.contains(&id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        self.taken.insert(id.clone());
        self.created.push((raw.clone(), id.clone()));
        let (token_type, value) = literal(&raw);
        self.ops.push(Op::CreateToken {
            id: id.clone(),
            token_type: token_type.to_owned(),
            value,
            set: None,
            layers: Vec::new(),
            filter_ops: Vec::new(),
            stops: Vec::new(),
            angle: None,
            radial: None,
            center_x: None,
            center_y: None,
            radius: None,
            shape: None,
            feather: None,
            invert: None,
        });
        Ok(id)
    }

    /// The first token (by id) whose resolved value equals `raw`.
    fn existing(&self, raw: &Raw) -> Option<String> {
        resolve_tokens(&self.doc.tokens)
            .resolved
            .into_iter()
            .find(|(_, t)| same(raw, &t.value))
            .map(|(id, _)| id)
    }
}

/// `true` when the resolved token value `v` is the raw value `raw`.
fn same(raw: &Raw, v: &ResolvedValue) -> bool {
    match raw {
        Raw::Color(hex) => matches!(
            v,
            ResolvedValue::Color(other) if normalize_hex(other).as_ref() == Some(hex)
        ),
        Raw::Px(px) => matches!(
            v,
            ResolvedValue::Dimension(d)
                if dim_to_px(d.value, &d.unit).is_some_and(|o| (o - px).abs() < 1e-9)
        ),
        Raw::Family(f) => matches!(v, ResolvedValue::FontFamily(other) if other == f),
        Raw::Weight(w) => matches!(
            v,
            ResolvedValue::FontWeight(other) if (f64::from(*other) - w).abs() < 1e-9
        ),
    }
}

/// Parse the raw `value` of `field` for a property of token type `kind`.
fn parse(field: &str, kind: Kind, value: &Value) -> Result<Raw, EditorError> {
    let bad = |what: &str| {
        EditorError::new(
            "editor.invalid_params",
            format!("{field}: value must be {what}, got {value}"),
        )
    };
    match kind {
        Kind::Color => value
            .as_str()
            .and_then(normalize_hex)
            .map(Raw::Color)
            .ok_or_else(|| bad("a hex color #rgb, #rrggbb, or #rrggbbaa")),
        Kind::Dimension => value
            .as_f64()
            .filter(|v| v.is_finite() && *v >= 0.0)
            .map(Raw::Px)
            .ok_or_else(|| bad("a size in px, 0 or more")),
        Kind::Family => value
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| Raw::Family(s.to_owned()))
            .ok_or_else(|| bad("a font family name")),
        Kind::Weight => value
            .as_f64()
            .filter(|v| v.fract() == 0.0 && (100.0..=900.0).contains(v))
            .map(Raw::Weight)
            .ok_or_else(|| bad("a whole font weight from 100 to 900")),
    }
}

/// `#rgb`, `#rgba`, `#rrggbb`, or `#rrggbbaa` as lowercase `#rrggbb` /
/// `#rrggbbaa`, an opaque alpha (`ff`) dropped. `None` for anything else.
pub(super) fn normalize_hex(s: &str) -> Option<String> {
    let digits = s.trim().strip_prefix('#')?;
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let lower = digits.to_ascii_lowercase();
    let full: String = match lower.len() {
        3 | 4 => lower.chars().flat_map(|c| [c, c]).collect(),
        6 | 8 => lower,
        _ => return None,
    };
    let opaque = full.len() == 8 && full.ends_with("ff");
    let kept = if opaque {
        full.get(..6)?
    } else {
        full.as_str()
    };
    Some(format!("#{kept}"))
}

/// The id a new token for `raw` starts from.
fn base_id(property: &str, raw: &Raw) -> String {
    let num = |v: f64| {
        let text = if v.fract() == 0.0 {
            format!("{v:.0}")
        } else {
            format!("{v}")
        };
        text.replace('.', "-")
    };
    match raw {
        Raw::Color(hex) => format!("color.custom.{}", hex.trim_start_matches('#')),
        Raw::Px(v) => format!("size.{property}.{}", num(*v)),
        Raw::Family(f) => {
            let slug: String = f
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() {
                        c.to_ascii_lowercase()
                    } else {
                        '-'
                    }
                })
                .collect();
            let slug = slug.trim_matches('-').to_owned();
            format!("font.{}", if slug.is_empty() { "family" } else { &slug })
        }
        Raw::Weight(w) => format!("weight.{}", num(*w)),
    }
}

/// The `create_token` type and value of `raw`.
fn literal(raw: &Raw) -> (&'static str, String) {
    match raw {
        Raw::Color(hex) => ("color", hex.clone()),
        Raw::Px(v) => ("dimension", format!("(px){v}")),
        Raw::Family(f) => ("fontFamily", f.clone()),
        Raw::Weight(w) => ("fontWeight", format!("{w}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_forms_normalize() {
        assert_eq!(normalize_hex("#ABC").as_deref(), Some("#aabbcc"));
        assert_eq!(normalize_hex("#aabbccff").as_deref(), Some("#aabbcc"));
        assert_eq!(normalize_hex("#11223380").as_deref(), Some("#11223380"));
        assert_eq!(normalize_hex("#1234").as_deref(), Some("#11223344"));
        assert_eq!(normalize_hex("112233"), None);
        assert_eq!(normalize_hex("#12345"), None);
        assert_eq!(normalize_hex("#gg0000"), None);
    }

    #[test]
    fn new_ids_read_like_their_values() {
        assert_eq!(base_id("radius", &Raw::Px(4.0)), "size.radius.4");
        assert_eq!(base_id("font-size", &Raw::Px(13.5)), "size.font-size.13-5");
        assert_eq!(
            base_id("fill", &Raw::Color("#ff000080".into())),
            "color.custom.ff000080"
        );
        assert_eq!(
            base_id("font-family", &Raw::Family("Noto Serif".into())),
            "font.noto-serif"
        );
        assert_eq!(base_id("font-weight", &Raw::Weight(700.0)), "weight.700");
        assert_eq!(literal(&Raw::Px(2.5)), ("dimension", "(px)2.5".to_owned()));
    }
}
