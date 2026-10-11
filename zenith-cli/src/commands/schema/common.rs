//! Shared formatting helper for the `zenith schema` command surfaces.

use crate::json_types::SchemaAttr;

/// Render an attribute list as an aligned `name — type` table.
pub(super) fn format_attr_table(attrs: &[SchemaAttr]) -> String {
    let col_width = attrs.iter().map(|a| a.name.len()).max().unwrap_or(0);

    let mut out = String::new();
    for attr in attrs {
        let default = attr
            .default
            .as_deref()
            .map(|d| format!(" (default: {d})"))
            .unwrap_or_default();
        out.push_str(&format!(
            "  {:<col_width$}  —  {}{default}\n",
            attr.name,
            attr.ty,
            col_width = col_width,
        ));
    }
    out
}

/// Side and axis suffixes of an attribute family such as `padding-x`.
const FAMILY_SUFFIXES: [&str; 6] = ["x", "y", "top", "right", "bottom", "left"];

/// Fold `padding-x`, `padding-top`, and the other members of an attribute
/// family into the `padding` row when their types match. The row's type then
/// ends with `(also padding-x, ...)`. Order and every other row stay as they
/// were.
pub(super) fn fold_families(attrs: Vec<SchemaAttr>) -> Vec<SchemaAttr> {
    let find = |name: &str| attrs.iter().find(|a| a.name == name);
    let members = |base: &SchemaAttr| -> Vec<String> {
        FAMILY_SUFFIXES
            .iter()
            .map(|suffix| format!("{}-{suffix}", base.name))
            .filter(|name| find(name).is_some_and(|m| m.ty == base.ty))
            .collect()
    };
    let is_member = |attr: &SchemaAttr| -> bool {
        FAMILY_SUFFIXES.iter().any(|suffix| {
            attr.name
                .strip_suffix(&format!("-{suffix}"))
                .and_then(find)
                .is_some_and(|base| base.ty == attr.ty)
        })
    };
    attrs
        .iter()
        .filter(|a| !is_member(a))
        .map(|a| {
            let also = members(a);
            if also.is_empty() {
                a.clone()
            } else {
                SchemaAttr {
                    name: a.name.clone(),
                    ty: format!("{} (also {})", a.ty, also.join(", ")),
                    default: a.default.clone(),
                }
            }
        })
        .collect()
}

/// `text` cut after the sentence that crosses `limit` chars, and whether it was
/// cut. Text within a third of `limit` over the limit stays whole.
pub(super) fn brief(text: &str, limit: usize) -> (String, bool) {
    if text.chars().count() <= limit + limit / 3 {
        return (text.to_owned(), false);
    }
    let mut out = String::new();
    for sentence in text.split_inclusive(". ") {
        out.push_str(sentence);
        if out.chars().count() >= limit {
            break;
        }
    }
    let cut = out.chars().count() < text.chars().count();
    (out.trim_end().to_owned(), cut)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attr(name: &str, ty: &str) -> SchemaAttr {
        SchemaAttr {
            name: name.to_owned(),
            ty: ty.to_owned(),
            default: None,
        }
    }

    #[test]
    fn a_family_folds_into_its_base_row() {
        let attrs = vec![
            attr("fill", "color"),
            attr("margin", "dim"),
            attr("margin-x", "dim"),
            attr("margin-top", "dim"),
            attr("w", "dim"),
        ];
        let folded = fold_families(attrs);
        let names: Vec<&str> = folded.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["fill", "margin", "w"]);
        assert_eq!(folded[1].ty, "dim (also margin-x, margin-top)");
    }

    #[test]
    fn a_member_with_another_type_keeps_its_row() {
        let attrs = vec![
            attr("anchor", "enum"),
            attr("anchor-gap", "px"),
            attr("x", "px"),
        ];
        assert_eq!(fold_families(attrs.clone()), attrs);
        let odd = vec![attr("margin", "dim"), attr("margin-x", "other")];
        assert_eq!(fold_families(odd.clone()), odd);
    }

    #[test]
    fn brief_cuts_at_a_sentence_and_keeps_short_text() {
        assert_eq!(brief("One. Two.", 100), ("One. Two.".to_owned(), false));
        let long = "First sentence here. Second sentence here. Third sentence here. Fourth one.";
        let (cut, was_cut) = brief(long, 20);
        assert!(was_cut);
        assert_eq!(cut, "First sentence here.");
    }
}
