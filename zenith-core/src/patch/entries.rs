//! Entry diff: set, insert, or remove the arguments and properties that
//! differ between a node's canonical before and after forms.
//!
//! Each changed entry takes its canonical after text (`key=value` or the
//! bare argument). Unchanged entries keep their source bytes.

use std::collections::BTreeMap;

use kdl::{KdlEntry, KdlNode};

use super::diff::{Differ, entry_range, name_end};
use super::error::{PatchError, PatchErrorCode};
use super::text::Edit;

/// The node's arguments, in order.
fn args(node: &KdlNode) -> Vec<&KdlEntry> {
    node.entries()
        .iter()
        .filter(|e| e.name().is_none())
        .collect()
}

/// The node's properties as `(name, entry)`, in order.
fn props(node: &KdlNode) -> Vec<(&str, &KdlEntry)> {
    node.entries()
        .iter()
        .filter_map(|e| e.name().map(|n| (n.value(), e)))
        .collect()
}

impl Differ<'_> {
    /// Diff the entries of one aligned node triple.
    pub(super) fn diff_entries(
        &mut self,
        u: &KdlNode,
        b: &KdlNode,
        a: &KdlNode,
    ) -> Result<(), PatchError> {
        self.diff_args(u, b, a)?;
        self.diff_props(u, b, a)
    }

    fn diff_args(&mut self, u: &KdlNode, b: &KdlNode, a: &KdlNode) -> Result<(), PatchError> {
        let (ua, ba, aa) = (args(u), args(b), args(a));
        let b_texts = ba
            .iter()
            .map(|e| self.before_text(entry_range(e)))
            .collect::<Result<Vec<_>, _>>()?;
        let a_texts = aa
            .iter()
            .map(|e| self.after_text(entry_range(e)))
            .collect::<Result<Vec<_>, _>>()?;
        if b_texts == a_texts {
            return Ok(());
        }
        if ua.len() != ba.len() {
            return Err(PatchError::new(
                PatchErrorCode::UnalignedSource,
                format!(
                    "node `{}` has {} argument(s) in the source and {} in the canonical form",
                    u.name().value(),
                    ua.len(),
                    ba.len()
                ),
            ));
        }
        for (i, at) in a_texts.iter().enumerate() {
            if let (Some(ue), Some(bt)) = (ua.get(i), b_texts.get(i))
                && bt != at
            {
                let (s, e) = entry_range(ue);
                self.edits.push(Edit::replace(s, e, *at));
            }
        }
        if let Some(extra) = a_texts.get(ua.len()..)
            && !extra.is_empty()
        {
            let at = ua.last().map_or_else(|| name_end(u), |e| entry_range(e).1);
            let text: String = extra.iter().map(|t| format!(" {t}")).collect();
            self.edits.push(Edit::insert(at, text));
        }
        for ue in ua.iter().skip(a_texts.len()) {
            self.remove_entry(ue);
        }
        Ok(())
    }

    fn diff_props(&mut self, u: &KdlNode, b: &KdlNode, a: &KdlNode) -> Result<(), PatchError> {
        let (up, bp, ap) = (props(u), props(b), props(a));
        let b_map: BTreeMap<&str, &KdlEntry> = bp.iter().copied().collect();
        let a_map: BTreeMap<&str, &KdlEntry> = ap.iter().copied().collect();
        for (idx, (name, ae)) in ap.iter().enumerate() {
            let at = self.after_text(entry_range(ae))?;
            if let Some(be) = b_map.get(name)
                && self.before_text(entry_range(be))? == at
            {
                continue;
            }
            self.set_prop(u, &up, &ap, idx, at)?;
        }
        for (name, _) in &bp {
            if !a_map.contains_key(name) {
                self.remove_prop(u, &up, name)?;
            }
        }
        Ok(())
    }

    /// Set property `ap[idx]` to canonical text `text` on source node `u`.
    fn set_prop(
        &mut self,
        u: &KdlNode,
        up: &[(&str, &KdlEntry)],
        ap: &[(&str, &KdlEntry)],
        idx: usize,
        text: &str,
    ) -> Result<(), PatchError> {
        let Some((name, _)) = ap.get(idx) else {
            return Ok(());
        };
        match unique_prop(u, up, name)? {
            Some(ue) => {
                let (s, e) = entry_range(ue);
                self.edits.push(Edit::replace(s, e, text));
            }
            None => {
                let at = insertion_point(u, up, ap.get(..idx).unwrap_or(&[]));
                self.edits.push(Edit::insert(at, format!(" {text}")));
            }
        }
        Ok(())
    }

    fn remove_prop(
        &mut self,
        u: &KdlNode,
        up: &[(&str, &KdlEntry)],
        name: &str,
    ) -> Result<(), PatchError> {
        if let Some(ue) = unique_prop(u, up, name)? {
            self.remove_entry(ue);
        }
        Ok(())
    }

    /// Remove an entry with the spaces before it.
    fn remove_entry(&mut self, entry: &KdlEntry) {
        let (s, e) = entry_range(entry);
        let head = self.texts.src.get(..s).unwrap_or("");
        let ws = head.len() - head.trim_end_matches([' ', '\t']).len();
        self.edits.push(Edit::replace(s - ws, e, ""));
    }
}

/// The single source entry for property `name`, `None` when absent. A
/// repeated property is ambiguous.
fn unique_prop<'u>(
    u: &KdlNode,
    up: &[(&str, &'u KdlEntry)],
    name: &str,
) -> Result<Option<&'u KdlEntry>, PatchError> {
    let mut found = up.iter().filter(|(n, _)| *n == name).map(|(_, e)| *e);
    let first = found.next();
    if found.next().is_some() {
        return Err(PatchError::new(
            PatchErrorCode::UnalignedSource,
            format!(
                "node `{}` repeats property `{name}` in the source",
                u.name().value()
            ),
        ));
    }
    Ok(first)
}

/// Where a new property goes: after the source copy of the nearest earlier
/// canonical property, else after the last argument, else after the name.
fn insertion_point(u: &KdlNode, up: &[(&str, &KdlEntry)], earlier: &[(&str, &KdlEntry)]) -> usize {
    for (name, _) in earlier.iter().rev() {
        let mut found = up.iter().filter(|(n, _)| n == name);
        if let (Some((_, e)), None) = (found.next(), found.next()) {
            return entry_range(e).1;
        }
    }
    args(u)
        .last()
        .map_or_else(|| name_end(u), |e| entry_range(e).1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kdl::KdlDocument;

    fn first(src: &str) -> KdlNode {
        let doc: KdlDocument = src.parse().expect("kdl");
        doc.nodes()[0].clone()
    }

    #[test]
    fn args_and_props_split() {
        let n = first("span \"a\" b=1 \"c\"");
        assert_eq!(args(&n).len(), 2);
        let p = props(&n);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].0, "b");
    }

    #[test]
    fn insertion_point_follows_nearest_earlier_property() {
        let src = "rect id=\"r\" x=1 w=2";
        let u = first(src);
        let canon = first("rect id=\"r\" x=1 y=5 w=2");
        let up = props(&u);
        let ap = props(&canon);
        // `y` comes after `x` canonically.
        let at = insertion_point(&u, &up, &ap[..2]);
        assert_eq!(&src[..at], "rect id=\"r\" x=1");
    }

    #[test]
    fn insertion_point_without_props_uses_last_argument() {
        let src = "span \"a\"";
        let u = first(src);
        assert_eq!(insertion_point(&u, &props(&u), &[]), src.len());
    }

    #[test]
    fn repeated_property_is_ambiguous() {
        let u = first("rect x=1 x=2");
        let err = unique_prop(&u, &props(&u), "x").expect_err("dup");
        assert_eq!(err.code, PatchErrorCode::UnalignedSource);
    }
}
