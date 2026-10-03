//! Style and `defaults` planning for `zenith theme apply`.
//!
//! A theme pack carries named styles and a document `defaults` block next to
//! its tokens. `theme apply` adds what the target document lacks and never
//! rewrites what it already has:
//! - a theme style whose id is absent from the document becomes an
//!   `Op::CreateStyle`; an existing id is skipped ([`BlockSkipReason::Exists`]);
//! - a theme document-level `defaults` entry whose kind the document block
//!   lacks becomes an `Op::SetDefault`; an existing kind is skipped.
//!
//! A style or entry that leaves the transaction invalid is skipped with
//! the reason, never emitted: the tx engine rejects the whole transaction on
//! one bad op.

use std::collections::{BTreeMap, BTreeSet};

use zenith_core::{DefaultsEntry, DefaultsKind, Document, PropertyValue, Style, style_enum_values};
use zenith_tx::Op;

/// Why a theme style or `defaults` entry was left out of the transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockSkipReason {
    /// The document already has a style with this id, or a document
    /// `defaults` entry for this kind.
    Exists,
    /// A style property value has no op form (a literal dimension, a data
    /// reference, or a literal on a key that takes a token).
    Unencodable,
    /// The style references a token the document will not have with the
    /// theme's type after the apply (a skipped or undeclared token).
    TokenUnavailable,
    /// The entry names a style the document will not have after the apply.
    StyleUnavailable,
}

impl BlockSkipReason {
    /// A short, stable label for human/JSON output.
    pub fn label(&self) -> &'static str {
        match self {
            BlockSkipReason::Exists => "exists",
            BlockSkipReason::Unencodable => "unencodable",
            BlockSkipReason::TokenUnavailable => "token_unavailable",
            BlockSkipReason::StyleUnavailable => "style_unavailable",
        }
    }
}

/// A theme style or `defaults` entry left out of the transaction.
#[derive(Debug, Clone)]
pub struct SkippedBlockItem {
    /// The style id, or the `defaults` node kind.
    pub id: String,
    /// Why it was skipped.
    pub reason: BlockSkipReason,
}

/// The style and `defaults` part of a `theme apply` transaction.
#[derive(Debug, Default)]
pub(super) struct BlockPlan {
    /// `CreateStyle` ops, then `SetDefault` ops.
    pub(super) ops: Vec<Op>,
    /// Ids of styles created.
    pub(super) added_styles: Vec<String>,
    /// Theme styles left untouched.
    pub(super) skipped_styles: Vec<SkippedBlockItem>,
    /// Kinds whose document `defaults` entry is created.
    pub(super) added_defaults: Vec<String>,
    /// Theme `defaults` entries left untouched.
    pub(super) skipped_defaults: Vec<SkippedBlockItem>,
}

/// Plan the style and `defaults` ops that bring `theme`'s styles and document
/// `defaults` entries into `doc`.
///
/// `tokens_after` holds every token id the document carries with the theme's
/// type once the token ops of the same transaction apply.
pub(super) fn plan_blocks(
    doc: &Document,
    theme: &Document,
    tokens_after: &BTreeSet<&str>,
) -> BlockPlan {
    let mut plan = BlockPlan::default();
    let doc_styles: BTreeSet<&str> = doc.styles.styles.iter().map(|s| s.id.as_str()).collect();
    let mut styles_after = doc_styles.clone();

    for style in &theme.styles.styles {
        if doc_styles.contains(style.id.as_str()) {
            plan.skipped_styles
                .push(skip(&style.id, BlockSkipReason::Exists));
            continue;
        }
        match encode_style(style, tokens_after) {
            Ok(properties) => {
                plan.ops.push(Op::CreateStyle {
                    id: style.id.clone(),
                    properties,
                });
                plan.added_styles.push(style.id.clone());
                styles_after.insert(style.id.as_str());
            }
            Err(reason) => plan.skipped_styles.push(skip(&style.id, reason)),
        }
    }

    for (kind, entry) in &theme.defaults.entries {
        let name = kind.name();
        if doc.defaults.get(*kind).is_some() {
            plan.skipped_defaults
                .push(skip(name, BlockSkipReason::Exists));
            continue;
        }
        if !entry_styles_exist(*kind, entry, &styles_after) {
            plan.skipped_defaults
                .push(skip(name, BlockSkipReason::StyleUnavailable));
            continue;
        }
        let text_style = entry
            .text_style
            .clone()
            .filter(|_| kind.accepts_text_style())
            .map(Some);
        plan.ops.push(Op::SetDefault {
            page: None,
            kind: name.to_owned(),
            style: entry.style.clone(),
            text_style,
        });
        plan.added_defaults.push(name.to_owned());
    }

    plan
}

fn skip(id: &str, reason: BlockSkipReason) -> SkippedBlockItem {
    SkippedBlockItem {
        id: id.to_owned(),
        reason,
    }
}

/// Whether every style `entry` names exists once the style ops apply.
fn entry_styles_exist(kind: DefaultsKind, entry: &DefaultsEntry, styles: &BTreeSet<&str>) -> bool {
    let label_ok = match (&entry.text_style, kind.accepts_text_style()) {
        (Some(id), true) => styles.contains(id.as_str()),
        (Some(_), false) | (None, true) | (None, false) => true,
    };
    label_ok && styles.contains(entry.style.as_str())
}

/// Encode a theme style as a `CreateStyle` property map: key → token id, or
/// key → enum literal for `align` / `v-align`.
fn encode_style(
    style: &Style,
    tokens_after: &BTreeSet<&str>,
) -> Result<BTreeMap<String, String>, BlockSkipReason> {
    let mut out = BTreeMap::new();
    for (key, value) in &style.properties {
        let encoded = match value {
            PropertyValue::TokenRef(id) => {
                if !tokens_after.contains(id.as_str()) {
                    return Err(BlockSkipReason::TokenUnavailable);
                }
                id.clone()
            }
            PropertyValue::Literal(lit) if style_enum_values(key).is_some() => lit.clone(),
            PropertyValue::Literal(_) | PropertyValue::Dimension(_) | PropertyValue::DataRef(_) => {
                return Err(BlockSkipReason::Unencodable);
            }
        };
        out.insert(key.clone(), encoded);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource as _};

    fn parse(src: &str) -> Document {
        KdlAdapter.parse(src.as_bytes()).expect("fixture parses")
    }

    const THEME: &str = r##"zenith version=1 {
  tokens format="zenith-token-v1" {
    token id="c" type="color" value="#111111"
    token id="r" type="dimension" value=(px)4
  }
  styles {
    style id="body" { fill (token)"c" }
    style id="box" { radius (token)"r"; align "center" }
  }
  defaults {
    shape style="box" text-style="body"
    text style="body"
  }
  document id="d" { page id="p" w=(px)10 h=(px)10 {} }
}"##;

    fn tokens(ids: &[&'static str]) -> BTreeSet<&'static str> {
        ids.iter().copied().collect()
    }

    #[test]
    fn empty_doc_gets_every_style_and_default() {
        let theme = parse(THEME);
        let doc =
            parse(r#"zenith version=1 { document id="d" { page id="p" w=(px)10 h=(px)10 {} } }"#);
        let plan = plan_blocks(&doc, &theme, &tokens(&["c", "r"]));
        assert_eq!(plan.added_styles, ["body", "box"]);
        assert_eq!(plan.added_defaults, ["shape", "text"]);
        assert!(plan.skipped_styles.is_empty());
        assert!(plan.skipped_defaults.is_empty());
        assert_eq!(plan.ops.len(), 4);
        assert!(matches!(
            &plan.ops[1],
            Op::CreateStyle { id, properties }
                if id == "box"
                    && properties.get("align").map(String::as_str) == Some("center")
                    && properties.get("radius").map(String::as_str) == Some("r")
        ));
        assert!(matches!(
            &plan.ops[2],
            Op::SetDefault { page: None, kind, style, text_style: Some(Some(ts)) }
                if kind == "shape" && style == "box" && ts == "body"
        ));
    }

    #[test]
    fn existing_style_and_kind_are_skipped() {
        let theme = parse(THEME);
        let doc = parse(
            r#"zenith version=1 {
  styles { style id="body" { } }
  defaults { text style="body" }
  document id="d" { page id="p" w=(px)10 h=(px)10 {} }
}"#,
        );
        let plan = plan_blocks(&doc, &theme, &tokens(&["c", "r"]));
        assert_eq!(plan.added_styles, ["box"]);
        assert_eq!(plan.skipped_styles.len(), 1);
        assert_eq!(plan.skipped_styles[0].id, "body");
        assert_eq!(plan.skipped_styles[0].reason, BlockSkipReason::Exists);
        assert_eq!(plan.added_defaults, ["shape"]);
        assert_eq!(plan.skipped_defaults[0].id, "text");
        assert_eq!(plan.skipped_defaults[0].reason, BlockSkipReason::Exists);
    }

    #[test]
    fn unavailable_token_skips_style_and_dependent_defaults() {
        let theme = parse(THEME);
        let doc =
            parse(r#"zenith version=1 { document id="d" { page id="p" w=(px)10 h=(px)10 {} } }"#);
        let plan = plan_blocks(&doc, &theme, &tokens(&["r"]));
        assert_eq!(plan.added_styles, ["box"]);
        assert_eq!(plan.skipped_styles[0].id, "body");
        assert_eq!(
            plan.skipped_styles[0].reason,
            BlockSkipReason::TokenUnavailable
        );
        assert!(plan.added_defaults.is_empty());
        let reasons: Vec<_> = plan
            .skipped_defaults
            .iter()
            .map(|s| (s.id.as_str(), s.reason))
            .collect();
        assert_eq!(
            reasons,
            [
                ("shape", BlockSkipReason::StyleUnavailable),
                ("text", BlockSkipReason::StyleUnavailable)
            ]
        );
    }

    #[test]
    fn literal_dimension_is_unencodable() {
        let theme = parse(
            r#"zenith version=1 {
  styles { style id="s" { radius (px)4 } }
  document id="d" { page id="p" w=(px)10 h=(px)10 {} }
}"#,
        );
        let doc =
            parse(r#"zenith version=1 { document id="d" { page id="p" w=(px)10 h=(px)10 {} } }"#);
        assert!(matches!(
            theme.styles.styles[0].properties.get("radius"),
            Some(PropertyValue::Dimension(_))
        ));
        let plan = plan_blocks(&doc, &theme, &tokens(&[]));
        assert!(plan.ops.is_empty());
        assert_eq!(plan.skipped_styles[0].reason, BlockSkipReason::Unencodable);
    }
}
