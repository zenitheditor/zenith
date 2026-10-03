//! Content pairing: the text colour a structural fill implies.
//!
//! A document or page that carries a `defaults` block pairs text with the
//! fill it sits on. The ambient fill is structural only: the page
//! `background`, a frame's fill, a shape's fill (for its label), and a table's
//! cell, header, and table fills. For an ambient fill `F`:
//!
//! 1. `(token)"X"` where token `X.content` exists → `X.content`.
//! 2. `(token)"P.N"` where `N` is all digits and token `P.content` exists →
//!    `P.content`.
//! 3. `F` resolves to a solid colour → the declared colour token whose id ends
//!    `.content` with the largest APCA |Lc| against `F`. A tie takes the
//!    smallest id. No such token → no pairing.
//! 4. A gradient, an unresolved token, or no fill → the parent ambient.

use std::collections::BTreeMap;

use crate::ast::value::PropertyValue;
use crate::color::{apca_lc, parse_rgb};
use crate::tokens::ResolvedToken;

/// Suffix that marks a content (on-colour) token.
const CONTENT_SUFFIX: &str = ".content";

/// The pairing table of one document: its resolved tokens and the solid
/// colour of every `*.content` colour token, in id order.
pub(super) struct Pairing<'a> {
    resolved: &'a BTreeMap<String, ResolvedToken>,
    content: Vec<(&'a str, (u8, u8, u8))>,
}

/// What an ambient fill does to the ambient pair its children see.
#[derive(Debug, PartialEq)]
enum Outcome {
    /// The fill sets the pair (`None`: a solid fill with no content token).
    Set(Option<String>),
    /// The fill is not a solid structural colour: keep the parent pair.
    Inherit,
}

impl<'a> Pairing<'a> {
    /// Collect the `*.content` colour tokens of `resolved`.
    pub(super) fn new(resolved: &'a BTreeMap<String, ResolvedToken>) -> Self {
        let content = resolved
            .iter()
            .filter(|(id, _)| id.ends_with(CONTENT_SUFFIX))
            .filter_map(|(id, token)| {
                let rgb = parse_rgb(token.value.as_color_hex()?)?;
                Some((id.as_str(), rgb))
            })
            .collect();
        Self { resolved, content }
    }

    /// The pair children of a node with ambient fill `fill` see, given the
    /// `parent` pair.
    pub(super) fn child_pair(
        &self,
        fill: Option<&PropertyValue>,
        parent: Option<&PropertyValue>,
    ) -> Option<PropertyValue> {
        match self.outcome(fill) {
            Outcome::Set(pair) => pair.map(PropertyValue::TokenRef),
            Outcome::Inherit => parent.cloned(),
        }
    }

    fn outcome(&self, fill: Option<&PropertyValue>) -> Outcome {
        let Some(fill) = fill else {
            return Outcome::Inherit;
        };
        let rgb = match fill {
            PropertyValue::TokenRef(id) => {
                if let Some(pair) = self.named_pair(id) {
                    return Outcome::Set(Some(pair));
                }
                let Some(token) = self.resolved.get(id.as_str()) else {
                    return Outcome::Inherit;
                };
                match token.value.as_color_hex().and_then(parse_rgb) {
                    Some(rgb) => rgb,
                    None => return Outcome::Inherit,
                }
            }
            PropertyValue::Literal(raw) => match parse_rgb(raw) {
                Some(rgb) => rgb,
                None => return Outcome::Inherit,
            },
            PropertyValue::Dimension(_) | PropertyValue::DataRef(_) => return Outcome::Inherit,
        };
        Outcome::Set(self.best_content(rgb))
    }

    /// Rules 1 and 2: the content token named after the fill token.
    fn named_pair(&self, id: &str) -> Option<String> {
        let direct = format!("{id}{CONTENT_SUFFIX}");
        if self.resolved.contains_key(&direct) {
            return Some(direct);
        }
        let (parent, last) = id.rsplit_once('.')?;
        if parent.is_empty() || last.is_empty() || !last.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let scale = format!("{parent}{CONTENT_SUFFIX}");
        self.resolved.contains_key(&scale).then_some(scale)
    }

    /// Rule 3: the content token with the largest |Lc| against `bg`. The
    /// table is in id order and only a strictly larger |Lc| replaces the
    /// best, so a tie keeps the smallest id.
    fn best_content(&self, bg: (u8, u8, u8)) -> Option<String> {
        let mut best: Option<(&str, f64)> = None;
        for (id, rgb) in &self.content {
            let lc = apca_lc(*rgb, bg).abs();
            if best.is_none_or(|(_, b)| lc > b) {
                best = Some((id, lc));
            }
        }
        best.map(|(id, _)| id.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::token::TokenType;
    use crate::tokens::ResolvedValue;

    fn table(colors: &[(&str, &str)]) -> BTreeMap<String, ResolvedToken> {
        colors
            .iter()
            .map(|(id, hex)| {
                (
                    (*id).to_owned(),
                    ResolvedToken {
                        token_type: TokenType::Color,
                        value: ResolvedValue::Color((*hex).to_owned()),
                    },
                )
            })
            .collect()
    }

    fn tok(id: &str) -> PropertyValue {
        PropertyValue::TokenRef(id.to_owned())
    }

    #[test]
    fn rule_one_direct_content_token() {
        let t = table(&[
            ("color.primary", "#605dff"),
            ("color.primary.content", "#ffffff"),
        ]);
        let p = Pairing::new(&t);
        assert_eq!(
            p.child_pair(Some(&tok("color.primary")), None),
            Some(tok("color.primary.content"))
        );
    }

    #[test]
    fn rule_two_scale_step_uses_scale_content() {
        let t = table(&[
            ("color.base.200", "#e0e0e0"),
            ("color.base.content", "#111111"),
        ]);
        let p = Pairing::new(&t);
        assert_eq!(
            p.child_pair(Some(&tok("color.base.200")), None),
            Some(tok("color.base.content"))
        );
    }

    #[test]
    fn rule_two_needs_an_all_digit_step() {
        let t = table(&[
            ("color.base.x2", "#e0e0e0"),
            ("color.base.content", "#111111"),
        ]);
        let p = Pairing::new(&t);
        // Falls through to rule 3, which also finds base.content.
        assert_eq!(
            p.named_pair("color.base.x2"),
            None,
            "a non-digit last segment is not a scale step"
        );
    }

    #[test]
    fn rule_three_picks_max_lc_and_ties_take_smallest_id() {
        let t = table(&[
            ("bg", "#000000"),
            ("a.content", "#ffffff"),
            ("b.content", "#ffffff"),
            ("c.content", "#333333"),
        ]);
        let p = Pairing::new(&t);
        assert_eq!(p.child_pair(Some(&tok("bg")), None), Some(tok("a.content")));
    }

    #[test]
    fn rule_three_without_content_tokens_clears_the_pair() {
        let t = table(&[("bg", "#000000")]);
        let p = Pairing::new(&t);
        let parent = tok("x.content");
        assert_eq!(p.child_pair(Some(&tok("bg")), Some(&parent)), None);
    }

    #[test]
    fn rule_four_inherits_through_unresolved_and_absent_fills() {
        let t = table(&[("a.content", "#ffffff")]);
        let p = Pairing::new(&t);
        let parent = tok("a.content");
        assert_eq!(
            p.child_pair(Some(&tok("missing")), Some(&parent)),
            Some(parent.clone())
        );
        assert_eq!(p.child_pair(None, Some(&parent)), Some(parent));
    }
}
