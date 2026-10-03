//! Structured [`FixHint`] builders. A hint is set only when exactly one fix
//! is safe, so `zenith fix` can apply it without judgment.

use crate::diagnostics::FixHint;

use super::distance::{token_fix_target, unique_nearest};

/// Rename hint for unknown property `name`: its unique nearest entry of
/// `known` within edit distance ≤ 2.
pub(crate) fn rename_property_fix(name: &str, known: &[&str]) -> Option<FixHint> {
    unique_nearest(name, known.iter().copied(), 2).map(|to| FixHint::RenameProperty {
        from: name.to_owned(),
        to: to.to_owned(),
    })
}

/// Value hint for enum `prop` = `value`: its unique nearest entry of `allowed`
/// within edit distance ≤ 2.
pub(crate) fn replace_value_fix(prop: &str, value: &str, allowed: &[&str]) -> Option<FixHint> {
    unique_nearest(value, allowed.iter().copied(), 2).map(|to| FixHint::ReplaceValue {
        property: prop.to_owned(),
        from: value.to_owned(),
        to: to.to_owned(),
    })
}

/// Token hint for an unknown reference `token_id` on `prop`: a declared
/// prefix, else the unique nearest id within edit distance ≤ 2, among
/// `candidates` (the ids of accepted type).
pub(crate) fn replace_token_ref_fix<'a>(
    prop: &str,
    token_id: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<FixHint> {
    token_fix_target(token_id, candidates).map(|to| FixHint::ReplaceTokenRef {
        property: prop.to_owned(),
        from: token_id.to_owned(),
        to: to.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_needs_unique_nearest() {
        let known = ["font-weight", "font-size"];
        assert_eq!(
            rename_property_fix("font-wieght", &known),
            Some(FixHint::RenameProperty {
                from: "font-wieght".into(),
                to: "font-weight".into()
            })
        );
        assert_eq!(rename_property_fix("ab", &["ac", "ad"]), None);
    }

    #[test]
    fn replace_value_needs_unique_nearest() {
        let allowed = ["cover", "contain"];
        assert!(replace_value_fix("fit", "covr", &allowed).is_some());
        assert_eq!(replace_value_fix("fit", "zzzzz", &allowed), None);
    }

    #[test]
    fn token_ref_prefers_declared_prefix_and_rejects_ties() {
        let ids = ["color.base.100", "color.base.200", "color.primary"];
        assert_eq!(
            replace_token_ref_fix("fill", "color.primary.500", ids),
            Some(FixHint::ReplaceTokenRef {
                property: "fill".into(),
                from: "color.primary.500".into(),
                to: "color.primary".into()
            })
        );
        assert_eq!(replace_token_ref_fix("fill", "color.base.900", ids), None);
    }
}
