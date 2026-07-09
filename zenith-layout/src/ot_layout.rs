//! Read-only OpenType layout feature and alternate discovery.
//!
//! Enumerates GSUB/GPOS feature tags and GSUB `AlternateSubstitution` glyphs
//! from raw font bytes via `rustybuzz::ttf_parser`. This is table exposure only
//! — it does not change shaping or authoring semantics.

use std::collections::{BTreeMap, BTreeSet};

use rustybuzz::ttf_parser;
use rustybuzz::ttf_parser::gsub::SubstitutionSubtable;
use rustybuzz::ttf_parser::{GlyphId, Tag};

use crate::error::LayoutError;

/// Honest limits of [`list_glyph_alternates`]: only type-3 alternate subtables.
pub const GLYPH_ALTERNATES_LIMITS: &str = "\
Only GSUB AlternateSubstitution (lookup type 3) coverage is enumerated. \
Single/Multiple/Ligature/Context/Chaining substitutions, feature-tag binding, \
and stylistic-set accessibility are not expanded.";

/// One OpenType layout feature tag and the tables that advertise it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureEntry {
    /// Four-character OpenType feature tag (e.g. `"liga"`, `"kern"`, `"ss01"`).
    pub tag: String,
    /// Tables that list this tag: `"GSUB"` and/or `"GPOS"`, sorted.
    pub tables: Vec<&'static str>,
}

/// Sorted layout-feature inventory for one face.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureList {
    /// Feature tags sorted lexicographically, each recording which tables list it.
    pub features: Vec<FeatureEntry>,
    /// True when the face has a classic `kern` table (independent of GPOS `kern`).
    pub has_kern_table: bool,
}

/// GSUB AlternateSubstitution glyphs reachable for one character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphAlternates {
    /// The codepoint that was queried.
    pub codepoint: char,
    /// cmap glyph index for `codepoint`, when present.
    pub glyph_index: Option<u16>,
    /// Alternate glyph IDs from type-3 subtables covering that glyph (sorted, unique).
    pub alternate_glyph_ids: Vec<u16>,
    /// Human-readable description of enumeration limits (always present).
    pub limits: String,
}

/// Enumerate GSUB + GPOS feature tags and whether a classic `kern` table exists.
///
/// Tags are collected into a deterministic [`BTreeMap`] keyed by tag string, then
/// emitted sorted. A tag present in both tables lists both in `tables`.
///
/// # Errors
///
/// Returns [`LayoutError`] when the bytes cannot be parsed as a font face.
pub fn list_layout_features(
    font_bytes: &[u8],
    face_index: u32,
) -> Result<FeatureList, LayoutError> {
    let face = parse_face(font_bytes, face_index)?;

    let tables = face.tables();
    let mut by_tag: BTreeMap<String, BTreeSet<&'static str>> = BTreeMap::new();

    if let Some(gsub) = tables.gsub {
        for feature in gsub.features {
            by_tag
                .entry(tag_string(feature.tag))
                .or_default()
                .insert("GSUB");
        }
    }

    if let Some(gpos) = tables.gpos {
        for feature in gpos.features {
            by_tag
                .entry(tag_string(feature.tag))
                .or_default()
                .insert("GPOS");
        }
    }

    let features = by_tag
        .into_iter()
        .map(|(tag, tables)| FeatureEntry {
            tag,
            tables: tables.into_iter().collect(),
        })
        .collect();

    Ok(FeatureList {
        features,
        has_kern_table: tables.kern.is_some(),
    })
}

/// List GSUB `AlternateSubstitution` (lookup type 3) glyphs for `ch`.
///
/// When the character has no cmap entry, `glyph_index` is `None` and
/// `alternate_glyph_ids` is empty. `limits` always describes the honest scope.
///
/// # Errors
///
/// Returns [`LayoutError`] when the bytes cannot be parsed as a font face.
pub fn list_glyph_alternates(
    font_bytes: &[u8],
    face_index: u32,
    ch: char,
) -> Result<GlyphAlternates, LayoutError> {
    let face = parse_face(font_bytes, face_index)?;

    // ttf-parser may map unmapped codepoints to glyph 0 (.notdef). Treat that
    // the same as no cmap entry for discovery purposes.
    let glyph_id = match face.glyph_index(ch) {
        Some(gid) if gid.0 != 0 => gid,
        Some(_) | None => {
            return Ok(GlyphAlternates {
                codepoint: ch,
                glyph_index: None,
                alternate_glyph_ids: Vec::new(),
                limits: GLYPH_ALTERNATES_LIMITS.to_owned(),
            });
        }
    };

    let mut alts: BTreeSet<u16> = BTreeSet::new();

    if let Some(gsub) = face.tables().gsub {
        collect_alternate_glyphs(&gsub, glyph_id, &mut alts);
    }

    Ok(GlyphAlternates {
        codepoint: ch,
        glyph_index: Some(glyph_id.0),
        alternate_glyph_ids: alts.into_iter().collect(),
        limits: GLYPH_ALTERNATES_LIMITS.to_owned(),
    })
}

fn parse_face(font_bytes: &[u8], face_index: u32) -> Result<ttf_parser::Face<'_>, LayoutError> {
    ttf_parser::Face::parse(font_bytes, face_index)
        .map_err(|e| LayoutError::new(format!("font parse failed: {e:?}")))
}

/// Walk every GSUB lookup and collect type-3 alternate glyph IDs covering `glyph`.
fn collect_alternate_glyphs(
    gsub: &ttf_parser::opentype_layout::LayoutTable<'_>,
    glyph: GlyphId,
    alts: &mut BTreeSet<u16>,
) {
    for lookup_index in 0..gsub.lookups.len() {
        let Some(lookup) = gsub.lookups.get(lookup_index) else {
            continue;
        };
        for sub_index in 0..lookup.subtables.len() {
            let Some(sub) = lookup.subtables.get::<SubstitutionSubtable<'_>>(sub_index) else {
                continue;
            };
            let SubstitutionSubtable::Alternate(alt) = sub else {
                continue;
            };
            let Some(coverage_index) = alt.coverage.get(glyph) else {
                continue;
            };
            let Some(set) = alt.alternate_sets.get(coverage_index) else {
                continue;
            };
            for alt_index in 0..set.alternates.len() {
                if let Some(gid) = set.alternates.get(alt_index) {
                    alts.insert(gid.0);
                }
            }
        }
    }
}

fn tag_string(tag: Tag) -> String {
    let bytes = tag.to_bytes();
    // OpenType tags are four ASCII bytes; fall back to lossy only for invalid data.
    match std::str::from_utf8(&bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => format!("{tag}"),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::font::embedded;

    const REGULAR: &[u8] = embedded::NOTO_SANS_REGULAR;

    #[test]
    fn noto_sans_lists_layout_features_sorted() {
        let list = list_layout_features(REGULAR, 0).expect("Noto Sans must parse");
        // Tags must be lexicographically sorted.
        let tags: Vec<&str> = list.features.iter().map(|f| f.tag.as_str()).collect();
        let mut sorted = tags.clone();
        sorted.sort();
        assert_eq!(tags, sorted, "feature tags must be sorted");
        // Each entry's tables must be sorted and non-empty.
        for entry in &list.features {
            assert!(!entry.tables.is_empty(), "tag {} has no tables", entry.tag);
            let mut t = entry.tables.clone();
            t.sort();
            assert_eq!(entry.tables, t, "tables for {} must be sorted", entry.tag);
            for table in &entry.tables {
                assert!(
                    *table == "GSUB" || *table == "GPOS",
                    "unexpected table name {table}"
                );
            }
        }
        // Classic kern may or may not be present; field is always defined.
        let _ = list.has_kern_table;
    }

    #[test]
    fn noto_sans_features_are_not_all_empty_when_tables_exist() {
        let face = ttf_parser::Face::parse(REGULAR, 0).expect("parse");
        let has_layout = face.tables().gsub.is_some() || face.tables().gpos.is_some();
        let list = list_layout_features(REGULAR, 0).expect("list");
        if has_layout {
            // Noto Sans ships with OpenType layout; expect at least one tag.
            assert!(
                !list.features.is_empty(),
                "Noto Sans layout tables present but feature list empty"
            );
        }
    }

    #[test]
    fn invalid_bytes_features_return_err() {
        let result = list_layout_features(b"not a font", 0);
        assert!(result.is_err(), "invalid bytes must return Err");
        let msg = result.unwrap_err().message;
        assert!(
            msg.contains("font parse failed"),
            "error should mention parse failure, got: {msg}"
        );
    }

    #[test]
    fn noto_sans_alternates_for_a_has_glyph_index() {
        let alts = list_glyph_alternates(REGULAR, 0, 'A').expect("must parse");
        assert_eq!(alts.codepoint, 'A');
        assert!(
            alts.glyph_index.is_some(),
            "Noto Sans must map 'A' to a glyph"
        );
        assert!(
            !alts.limits.is_empty(),
            "limits string must always be present"
        );
        assert!(
            alts.limits.contains("AlternateSubstitution"),
            "limits must mention AlternateSubstitution"
        );
        // Alternate IDs (if any) must be sorted unique — property of BTreeSet collect.
        let mut sorted = alts.alternate_glyph_ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(alts.alternate_glyph_ids, sorted);
    }

    #[test]
    fn alternates_unknown_codepoint_has_no_glyph() {
        // U+FFFF is almost never mapped in Latin fonts.
        let alts = list_glyph_alternates(REGULAR, 0, '\u{FFFF}').expect("must parse");
        assert_eq!(alts.glyph_index, None);
        assert!(alts.alternate_glyph_ids.is_empty());
        assert!(!alts.limits.is_empty());
    }

    #[test]
    fn invalid_bytes_alternates_return_err() {
        let result = list_glyph_alternates(b"not a font", 0, 'A');
        assert!(result.is_err());
    }

    #[test]
    fn tag_string_round_trips_ascii() {
        let tag = Tag::from_bytes(b"liga");
        assert_eq!(tag_string(tag), "liga");
    }
}
