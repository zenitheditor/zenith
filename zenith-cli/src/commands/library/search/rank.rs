//! Deterministic BM25 ranking over library pack items.
//!
//! With ~1745 bundled icons, a boolean "does the text contain the query?" test
//! is useless twice over: it returns hundreds of items in arbitrary order, and
//! it matches WITHIN words — `play` matched `airplay` and `monitor` (whose tag
//! is `display`). Ranking replaces it.
//!
//! Each item is a small document over weighted fields:
//!
//! | field    | weight | why |
//! |----------|--------|-----|
//! | item id  | 3.0    | an icon NAMED `play` must outrank one merely tagged `play` |
//! | aliases  | 2.5    | an alternate NAME (`home` for `house`) is nearly as strong as the name |
//! | tags     | 1.0    | merely RELATED words (`living`, `residence`) |
//! | pack id  | 0.5    | `search lucide` should surface that pack's items |
//! | kind     | 0.5    | `search token` is a meaningful query |
//! | license  | 0.5    | `search ISC` is a meaningful query |
//!
//! Separating aliases from tags is what makes `home` return `house` first rather
//! than `lamp` (which is merely TAGGED `home`), and `sync` return `refresh-cw`
//! rather than `folder-sync`. Collapsing the two loses that ordering entirely.
//!
//! `categories` are deliberately NOT indexed: a category like `shapes` applies
//! to hundreds of icons and would swamp scoring. They filter instead.
//!
//! An exact hit on an item's id, or on one of its aliases, earns a large bonus,
//! so the icon a user literally named is pinned to the top.
//!
//! Query terms are combined with AND: an item must match EVERY term to be a
//! result. BM25's usual OR-semantics are wrong for a command-line lookup —
//! `zzzz-not-an-icon` would return hundreds of icons on the strength of `not`
//! and `icon` alone. AND makes a query that names nothing return nothing.
//!
//! Scoring is standard BM25 with `k1 = 1.2`, `b = 0.75`, over the weighted term
//! frequencies (a BM25F-style single-pass approximation). A query term also
//! matches corpus terms it PREFIXES, once it is at least [`MIN_PREFIX_LEN`]
//! characters, so `hous` finds `house`; each query term contributes the best
//! score among the terms it expands to, rather than the sum, so a term does not
//! earn more merely by having many expansions.
//!
//! Determinism: scores are compared with [`f64::total_cmp`] and ties broken on
//! `(pack id, item id)`, so identical inputs always produce identical output.

use std::collections::BTreeMap;

use crate::library::{ItemKind, LibraryPack, PackItem};

/// BM25 term-frequency saturation.
const K1: f64 = 1.2;
/// BM25 length normalization.
const B: f64 = 0.75;
/// Shortest query term allowed to match by prefix, so `a` does not match all.
const MIN_PREFIX_LEN: usize = 3;
/// Score added when the query names an item id exactly. Large enough to pin the
/// exact hit above any accumulation of partial matches.
const EXACT_ID_BONUS: f64 = 1000.0;
/// Score added when the query names one of an item's aliases exactly. Below
/// [`EXACT_ID_BONUS`], so a real id always beats another item's alias.
const EXACT_ALIAS_BONUS: f64 = 500.0;

const W_ID: f64 = 3.0;
const W_ALIAS: f64 = 2.5;
const W_TAG: f64 = 1.0;
const W_PACK: f64 = 0.5;
const W_KIND: f64 = 0.5;
const W_LICENSE: f64 = 0.5;

/// Split text into lowercase alphanumeric terms. `arrow-right-left` → `["arrow",
/// "right", "left"]`, so a `-`-separated name is searchable by any of its parts.
pub fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Filters applied before ranking. A filter narrows the candidate set; it never
/// affects a surviving item's score.
#[derive(Debug, Default, Clone, Copy)]
pub struct Filter<'a> {
    /// Keep only items carrying this category (exact, lowercase).
    pub category: Option<&'a str>,
    /// Keep only items of this kind.
    pub kind: Option<ItemKind>,
    /// Keep only items from this exact pack id.
    pub pack: Option<&'a str>,
}

impl Filter<'_> {
    fn admits(&self, pack: &LibraryPack, item: &PackItem) -> bool {
        if let Some(id) = self.pack
            && pack.id != id
        {
            return false;
        }
        if let Some(kind) = self.kind
            && item.kind != kind
        {
            return false;
        }
        if let Some(category) = self.category
            && !item.categories.iter().any(|c| c == category)
        {
            return false;
        }
        true
    }
}

/// One ranked item, with the pack it came from.
#[derive(Debug)]
pub struct Scored<'a> {
    /// The pack the item belongs to.
    pub pack: &'a LibraryPack,
    /// The matched item.
    pub item: &'a PackItem,
    /// Its BM25 score; strictly positive for every returned result.
    pub score: f64,
}

/// One indexed item: its weighted term frequencies and its weighted length.
struct IndexedDoc<'a> {
    pack: &'a LibraryPack,
    item: &'a PackItem,
    /// Weighted term frequency, keyed by term.
    tf: BTreeMap<String, f64>,
    /// Weighted document length (sum of all weighted term frequencies).
    len: f64,
    /// The item id normalized to space-joined terms, for the exact-match bonus.
    id_key: String,
    /// Each alias normalized to space-joined terms, for the exact-match bonus.
    alias_keys: Vec<String>,
}

/// Add `text`'s terms to `tf`/`len` at the given field weight.
fn index_field(tf: &mut BTreeMap<String, f64>, len: &mut f64, text: &str, weight: f64) {
    for term in tokenize(text) {
        *tf.entry(term).or_insert(0.0) += weight;
        *len += weight;
    }
}

/// Index one item over its weighted fields.
fn index_doc<'a>(pack: &'a LibraryPack, item: &'a PackItem) -> IndexedDoc<'a> {
    let mut tf = BTreeMap::new();
    let mut len = 0.0;

    index_field(&mut tf, &mut len, &item.id, W_ID);
    for alias in &item.aliases {
        index_field(&mut tf, &mut len, alias, W_ALIAS);
    }
    for tag in &item.tags {
        index_field(&mut tf, &mut len, tag, W_TAG);
    }
    index_field(&mut tf, &mut len, &pack.id, W_PACK);
    index_field(&mut tf, &mut len, item.kind.label(), W_KIND);
    if let Some(license) = &pack.license {
        index_field(&mut tf, &mut len, license, W_LICENSE);
    }

    IndexedDoc {
        pack,
        item,
        tf,
        len,
        id_key: tokenize(&item.id).join(" "),
        alias_keys: item.aliases.iter().map(|a| tokenize(a).join(" ")).collect(),
    }
}

/// Inverse document frequency, in the BM25 (probabilistic, non-negative) form.
fn idf(n_docs: usize, doc_freq: usize) -> f64 {
    let n = n_docs as f64;
    let df = doc_freq as f64;
    (1.0 + (n - df + 0.5) / (df + 0.5)).ln()
}

/// The corpus terms a query term matches: itself, plus every term it prefixes
/// once it is long enough to be discriminating.
fn expansions<'a>(query_term: &str, df: &'a BTreeMap<String, usize>) -> Vec<&'a String> {
    if query_term.len() < MIN_PREFIX_LEN {
        return df
            .get_key_value(query_term)
            .map(|(t, _)| t)
            .into_iter()
            .collect();
    }
    df.keys().filter(|t| t.starts_with(query_term)).collect()
}

/// Rank `packs`' items against `query`, best first.
///
/// Returns only items that matched at least one query term. An empty query, or
/// one whose terms appear nowhere, yields an empty vector.
pub fn rank<'a>(packs: &'a [LibraryPack], query: &str, filter: &Filter<'_>) -> Vec<Scored<'a>> {
    let query_terms = tokenize(query);
    if query_terms.is_empty() {
        return Vec::new();
    }

    let docs: Vec<IndexedDoc<'a>> = packs
        .iter()
        .flat_map(|pack| {
            pack.items
                .iter()
                .filter(|item| filter.admits(pack, item))
                .map(move |item| index_doc(pack, item))
        })
        .collect();
    if docs.is_empty() {
        return Vec::new();
    }

    // Document frequency per term, and the average weighted document length.
    let mut df: BTreeMap<String, usize> = BTreeMap::new();
    for doc in &docs {
        for term in doc.tf.keys() {
            *df.entry(term.clone()).or_insert(0) += 1;
        }
    }
    let n_docs = docs.len();
    let total_len: f64 = docs.iter().map(|d| d.len).sum();
    let avg_len = total_len / n_docs as f64;

    // Precompute each query term's expansions and their idf.
    let expanded: Vec<Vec<(&String, f64)>> = query_terms
        .iter()
        .map(|q| {
            expansions(q, &df)
                .into_iter()
                .map(|term| {
                    let n = df.get(term).copied().unwrap_or(0);
                    (term, idf(n_docs, n))
                })
                .collect()
        })
        .collect();

    let query_key = query_terms.join(" ");

    let mut scored: Vec<Scored<'a>> = docs
        .iter()
        .filter_map(|doc| {
            let mut score = 0.0;
            for terms in &expanded {
                // Each query term contributes its BEST expansion, not the sum:
                // a term must not earn more just by prefixing many corpus terms.
                let mut best = 0.0_f64;
                for (term, term_idf) in terms {
                    let Some(&tf) = doc.tf.get(*term) else {
                        continue;
                    };
                    let denom = tf + K1 * (1.0 - B + B * doc.len / avg_len);
                    if denom <= 0.0 {
                        continue;
                    }
                    best = best.max(term_idf * (tf * (K1 + 1.0)) / denom);
                }
                // AND semantics: a term the item does not match disqualifies it.
                if best <= 0.0 {
                    return None;
                }
                score += best;
            }
            if score <= 0.0 {
                return None;
            }
            if doc.id_key == query_key {
                score += EXACT_ID_BONUS;
            } else if doc.alias_keys.contains(&query_key) {
                score += EXACT_ALIAS_BONUS;
            }
            Some(Scored {
                pack: doc.pack,
                item: doc.item,
                score,
            })
        })
        .collect();

    // Deterministic: score descending, then pack id, then item id.
    scored.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.pack.id.cmp(&b.pack.id))
            .then_with(|| a.item.id.cmp(&b.item.id))
    });
    scored
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::resolve_packs;

    fn ids(scored: &[Scored<'_>]) -> Vec<String> {
        scored.iter().map(|s| s.item.id.clone()).collect()
    }

    #[test]
    fn tokenize_splits_on_hyphens_and_lowercases() {
        assert_eq!(tokenize("Arrow-Right_Left"), ["arrow", "right", "left"]);
        assert_eq!(
            tokenize("@zenith/icons-lucide"),
            ["zenith", "icons", "lucide"]
        );
        assert!(tokenize("---").is_empty());
    }

    /// The regression this ranker exists for: `.contains()` ranked `airplay` and
    /// `monitor` (tag `display`) as matches for `play`.
    #[test]
    fn exact_id_outranks_substring_neighbours() {
        let packs = resolve_packs(None);
        let scored = rank(&packs, "play", &Filter::default());
        assert_eq!(scored[0].item.id, "play", "got: {:?}", &ids(&scored)[..5]);

        let ids = ids(&scored);
        assert!(
            !ids.contains(&"airplay".to_owned()),
            "`play` must not match inside `airplay`"
        );
        assert!(
            !ids.contains(&"monitor".to_owned()),
            "`play` must not match inside the `display` tag"
        );
        // A hyphenated name genuinely containing the term still matches.
        assert!(ids.contains(&"circle-play".to_owned()));
    }

    #[test]
    fn id_match_outranks_tag_match() {
        let packs = resolve_packs(None);
        let scored = rank(&packs, "house", &Filter::default());
        assert_eq!(scored[0].item.id, "house");
    }

    #[test]
    fn an_exact_alias_outranks_items_whose_id_merely_contains_the_term() {
        let packs = resolve_packs(None);
        // `sync` is not an id at lucide 1.23.0; it is an ALIAS of `refresh-cw`.
        // Without the alias/tag split, `folder-sync` and `cloud-sync` won here.
        let scored = rank(&packs, "sync", &Filter::default());
        assert_eq!(
            scored[0].item.id,
            "refresh-cw",
            "got: {:?}",
            &ids(&scored)[..4]
        );

        // Likewise the upstream-recorded rename.
        let scored = rank(&packs, "upload-cloud", &Filter::default());
        assert_eq!(scored[0].item.id, "cloud-upload");
    }

    /// `home` is an ALIAS of `house`, but only a TAG of `lamp` and `birdhouse`.
    /// Alias authority is what orders them correctly.
    #[test]
    fn an_alias_outranks_a_mere_tag() {
        let packs = resolve_packs(None);
        let scored = rank(&packs, "home", &Filter::default());
        assert_eq!(scored[0].item.id, "house", "got: {:?}", &ids(&scored)[..4]);
    }

    #[test]
    fn prefix_matches_once_long_enough() {
        let packs = resolve_packs(None);
        assert!(ids(&rank(&packs, "hous", &Filter::default())).contains(&"house".to_owned()));
        // Below MIN_PREFIX_LEN, only exact terms match.
        let short = rank(&packs, "ho", &Filter::default());
        assert!(!ids(&short).contains(&"house".to_owned()));
    }

    #[test]
    fn category_filter_narrows_without_reordering() {
        let packs = resolve_packs(None);
        let filter = Filter {
            category: Some("buildings"),
            ..Filter::default()
        };
        let scored = rank(&packs, "house", &filter);
        assert_eq!(scored[0].item.id, "house");
        assert!(
            scored
                .iter()
                .all(|s| s.item.categories.iter().any(|c| c == "buildings")),
            "every result must carry the filtered category"
        );
    }

    #[test]
    fn kind_and_pack_filters_narrow() {
        let packs = resolve_packs(None);
        let scored = rank(
            &packs,
            "noir",
            &Filter {
                kind: Some(ItemKind::Token),
                ..Filter::default()
            },
        );
        assert!(scored.iter().all(|s| s.item.kind == ItemKind::Token));
        assert_eq!(scored[0].item.id, "noir");

        let scored = rank(
            &packs,
            "noir",
            &Filter {
                pack: Some("@zenith/icons-lucide"),
                ..Filter::default()
            },
        );
        assert!(scored.is_empty(), "no lucide icon is named noir");
    }

    #[test]
    fn empty_and_unmatched_queries_yield_nothing() {
        let packs = resolve_packs(None);
        assert!(rank(&packs, "", &Filter::default()).is_empty());
        assert!(rank(&packs, "   ", &Filter::default()).is_empty());
        assert!(rank(&packs, "zzzznotanicon", &Filter::default()).is_empty());
    }

    /// AND semantics. Under OR, `zzzz-not-an-icon` returned hundreds of hits on
    /// the strength of its `not` and `icon` terms alone.
    #[test]
    fn every_query_term_must_match() {
        let packs = resolve_packs(None);
        assert!(rank(&packs, "zzzz-not-an-icon", &Filter::default()).is_empty());

        // A term that matches nothing disqualifies the whole query.
        assert!(!rank(&packs, "house", &Filter::default()).is_empty());
        assert!(rank(&packs, "house zzzznope", &Filter::default()).is_empty());

        // Multi-term queries still work when every term lands.
        let scored = rank(&packs, "arrow right", &Filter::default());
        assert!(
            scored
                .iter()
                .all(|s| s.item.id.contains("arrow") || !s.item.tags.is_empty()),
            "all results carry both terms somewhere"
        );
        assert!(!scored.is_empty());
    }

    #[test]
    fn ranking_is_deterministic() {
        let packs = resolve_packs(None);
        let a = ids(&rank(&packs, "cloud", &Filter::default()));
        let b = ids(&rank(&packs, "cloud", &Filter::default()));
        assert_eq!(a, b);
    }
}
