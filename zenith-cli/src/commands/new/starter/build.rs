//! Build a starter document for a theme and a page size.
//!
//! The page size resolves like `zenith new` (`--format`, `--landscape`,
//! `--width`, `--height`), with the starter's natural size as the base when
//! no `--format` is given. The page count always comes from the starter.
//!
//! The layout fits any page through one factor,
//! `s = min(width / natural width, height / natural height)`:
//!
//! - The theme tokens (`zenith new --theme`, or the starter's default theme)
//!   are copied in. `size.*` is rescaled so `size.body` is the starter's
//!   natural body times `s`, and `radius.*` by the short side, through the
//!   shared print-scale rules.
//! - The starter's own `space.*` and `layout.*` tokens hold natural px values
//!   and scale by `s`. The page box tokens are derived from the final page:
//!   `space.margin` (scaled margin), `layout.width` and `layout.height` (the
//!   page minus two margins), and `layout.footer.y` (bottom margin edge
//!   minus `layout.footer.h`).
//! - Every page takes the final size and the scaled margin on all four sides.

use std::path::Path;

use zenith_core::ast::{Token, TokenLiteral, TokenValue, Unit};
use zenith_core::{Dimension, Document, KdlAdapter, KdlSource as _, Project};
use zenith_session::StorePaths;

use super::super::page::{PageSpec, PaperFormat};
use super::super::print_scale;
use super::super::scaffold::{
    NewErr, NewResult, prepare_target, resolve_store, slug_for, write_new,
};
use super::kind::Starter;

/// The size flags `zenith new --starter` accepts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StarterSize {
    /// `--format`: the base size instead of the natural size.
    pub format: Option<PaperFormat>,
    /// `--width`: overrides the base width.
    pub width: Option<u32>,
    /// `--height`: overrides the base height.
    pub height: Option<u32>,
    /// `--landscape`: the wider base side is the width.
    pub landscape: bool,
}

/// The page of `starter` under `size`. The page count is the starter's.
///
/// # Errors
///
/// A zero width or height (exit 2).
pub fn starter_page(starter: Starter, size: StarterSize) -> Result<PageSpec, NewErr> {
    let natural = starter.natural();
    let (base_w, base_h) = size
        .format
        .map_or((natural.width, natural.height), PaperFormat::portrait_px);
    // Landscape is the wide orientation of the base: a starter that is
    // already wide stays as it is.
    let (base_w, base_h) = if size.landscape {
        (base_w.max(base_h), base_w.min(base_h))
    } else {
        (base_w, base_h)
    };
    let width = size.width.unwrap_or(base_w);
    let height = size.height.unwrap_or(base_h);
    if width == 0 || height == 0 {
        return Err(NewErr {
            message: format!("page dimensions must be positive; got {width}x{height}"),
            exit_code: 2,
        });
    }
    let custom = size.format.is_some() || size.width.is_some() || size.height.is_some();
    Ok(PageSpec {
        width,
        height,
        pages: natural.pages,
        format: if custom { size.format } else { natural.format },
    })
}

/// Write `starter` at `path`, resolving the real data directory for history.
///
/// `name` titles the document and slugs its project and document ids; without
/// it the title is "Untitled" and the ids come from the file name, as in
/// `zenith new`.
/// `theme` names a theme pack; `None` applies the starter's default theme.
///
/// # Errors
///
/// A directory or existing file at `path`, an unknown theme, a zero page
/// size, or a write error (exit 2).
pub fn run_starter(
    path: &Path,
    name: Option<&str>,
    starter: Starter,
    theme: Option<&str>,
    size: StarterSize,
) -> Result<NewResult, NewErr> {
    run_starter_in(&resolve_store()?, path, name, starter, theme, size)
}

/// Same as [`run_starter`] but with an explicit store root (used by tests).
///
/// # Errors
///
/// As [`run_starter`].
pub fn run_starter_in(
    paths: &StorePaths,
    path: &Path,
    name: Option<&str>,
    starter: Starter,
    theme: Option<&str>,
    size: StarterSize,
) -> Result<NewResult, NewErr> {
    let target = prepare_target(path)?;
    // As `zenith new`: the title is the name or "Untitled", the ids come from
    // the name or the file stem. A starter never keeps its sample identity.
    let rename = Some((name.unwrap_or("Untitled"), slug_for(name, &target)));
    let bytes = build(starter, rename, theme, size, target.parent())?;
    write_new(paths, &target, &bytes)
}

/// The canonical bytes of `starter` with `theme` (or its default theme) on
/// the page `size` gives, before a `doc-id` is stamped. Themes resolve from
/// the embedded presets only.
///
/// # Errors
///
/// An unknown theme or a zero page size (exit 2).
pub fn starter_document(
    starter: Starter,
    theme: Option<&str>,
    size: StarterSize,
) -> Result<Vec<u8>, NewErr> {
    build(starter, None, theme, size, None)
}

/// Resolve the page and the theme pack (a project pack under `project_dir`
/// shadows a preset), then [`materialize`].
fn build(
    starter: Starter,
    rename: Option<(&str, String)>,
    theme: Option<&str>,
    size: StarterSize,
    project_dir: Option<&Path>,
) -> Result<Vec<u8>, NewErr> {
    let page = starter_page(starter, size)?;
    let theme_name = theme.unwrap_or(starter.default_theme());
    let theme_doc =
        crate::library::resolve_theme_pack(project_dir, theme_name).map_err(|message| NewErr {
            message,
            exit_code: 2,
        })?;
    materialize(starter, rename, &theme_doc, page)
}

/// Append `own` to `base`. An entry of `own` replaces the entry of `base` with
/// the same id: the layout of a starter depends on its own tokens and styles,
/// and a theme pack (a project pack can define any id) must not leave two.
fn merge_by_id<T>(base: &mut Vec<T>, own: Vec<T>, id: impl Fn(&T) -> &str) {
    let ids: Vec<String> = own.iter().map(|item| id(item).to_owned()).collect();
    base.retain(|item| !ids.iter().any(|taken| taken == id(item)));
    base.extend(own);
}

/// The canonical bytes of `starter` on `page` with the tokens, styles, and
/// defaults of `theme`, retitled when `rename` holds a display name and its
/// id slug.
fn materialize(
    starter: Starter,
    rename: Option<(&str, String)>,
    theme: &Document,
    page: PageSpec,
) -> Result<Vec<u8>, NewErr> {
    let internal = |what: &str, message: String| NewErr {
        message: format!(
            "internal: starter '{}' failed to {what}: {message}",
            starter.kind()
        ),
        exit_code: 2,
    };
    let mut doc = KdlAdapter
        .parse(starter.source().as_bytes())
        .map_err(|e| internal("parse", e.message))?;

    if let Some((name, slug)) = rename {
        let author = doc.project.take().and_then(|p| p.author);
        doc.project = Some(Project {
            id: format!("proj.{slug}"),
            name: name.to_owned(),
            author,
        });
        doc.body.id = format!("doc.{slug}");
        doc.body.title = Some(name.to_owned());
    }

    let natural = starter.natural();
    let fit = (f64::from(page.width) / f64::from(natural.width))
        .min(f64::from(page.height) / f64::from(natural.height));
    let margin = (natural.margin_px * fit).round();

    let mut tokens = theme.tokens.clone();
    print_scale::apply_body(&mut tokens, natural.body_px * fit, page.width, page.height);
    let mut own = std::mem::take(&mut doc.tokens.tokens);
    fit_layout_tokens(&mut own, fit, margin, page);
    merge_by_id(&mut tokens.tokens, own, |t| &t.id);
    doc.tokens = tokens;

    let own_styles = std::mem::take(&mut doc.styles.styles);
    doc.styles = theme.styles.clone();
    merge_by_id(&mut doc.styles.styles, own_styles, |s| &s.id);
    doc.defaults = theme.defaults.clone();

    let px = |value: f64| Dimension {
        value,
        unit: Unit::Px,
    };
    for p in &mut doc.body.pages {
        p.width = px(f64::from(page.width));
        p.height = px(f64::from(page.height));
        p.margin_top = Some(px(margin));
        p.margin_bottom = Some(px(margin));
        p.margin_inner = Some(px(margin));
        p.margin_outer = Some(px(margin));
    }

    KdlAdapter
        .format(&doc)
        .map_err(|e| internal("format", e.message))
}

/// Scale the starter's own literal px tokens by `fit`, then derive the page
/// box tokens from `margin` and `page`.
fn fit_layout_tokens(tokens: &mut [Token], fit: f64, margin: f64, page: PageSpec) {
    for token in tokens.iter_mut() {
        if let Some(value) = literal_px(token) {
            set_px(token, (value * fit).round().max(0.0));
        }
    }
    let scaled = |tokens: &[Token], id: &str| {
        tokens
            .iter()
            .find(|t| t.id == id)
            .and_then(literal_px)
            .unwrap_or(0.0)
    };
    let width = f64::from(page.width);
    let height = f64::from(page.height);
    let footer_y = height - margin - scaled(tokens, "layout.footer.h");
    for token in tokens.iter_mut() {
        let derived = match token.id.as_str() {
            "space.margin" => Some(margin),
            "layout.width" => Some(width - 2.0 * margin),
            "layout.height" => Some(height - 2.0 * margin),
            "layout.footer.y" => Some(footer_y),
            _ => None,
        };
        if let Some(value) = derived {
            set_px(token, value.max(0.0));
        }
    }
}

/// The value of a literal px dimension token.
fn literal_px(token: &Token) -> Option<f64> {
    match &token.value {
        TokenValue::Literal(TokenLiteral::Dimension(d)) if d.unit == Unit::Px => Some(d.value),
        TokenValue::Literal(_) | TokenValue::Reference { .. } => None,
    }
}

/// Write `value` px into `token`.
fn set_px(token: &mut Token, value: f64) {
    token.value = TokenValue::Literal(TokenLiteral::Dimension(Dimension {
        value,
        unit: Unit::Px,
    }));
}

#[cfg(test)]
mod tests {
    use clap::ValueEnum as _;
    use zenith_core::TokenBlock;

    use super::*;

    fn parse(bytes: &[u8]) -> Document {
        KdlAdapter.parse(bytes).expect("starter parses")
    }

    fn token_px(tokens: &TokenBlock, id: &str) -> f64 {
        tokens
            .tokens
            .iter()
            .find(|t| t.id == id)
            .and_then(literal_px)
            .unwrap_or_else(|| panic!("no px token {id}"))
    }

    fn build_doc(starter: Starter, theme: Option<&str>, size: StarterSize) -> Document {
        parse(&starter_document(starter, theme, size).expect("build"))
    }

    #[test]
    fn natural_page_is_the_starter_size() {
        for &starter in Starter::value_variants() {
            let page = starter_page(starter, StarterSize::default()).expect("page");
            let natural = starter.natural();
            assert_eq!(
                (page.width, page.height, page.pages, page.format),
                (natural.width, natural.height, natural.pages, natural.format)
            );
        }
    }

    #[test]
    fn a_starter_entry_replaces_a_theme_entry_with_the_same_id() {
        let mut base = vec![("space.md", 1), ("color.a", 2)];
        merge_by_id(&mut base, vec![("space.md", 9), ("layout.w", 3)], |e| e.0);
        assert_eq!(base, [("color.a", 2), ("space.md", 9), ("layout.w", 3)]);
    }

    #[test]
    fn size_flags_resolve_like_new() {
        let landscape = StarterSize {
            landscape: true,
            ..StarterSize::default()
        };
        let page = starter_page(Starter::Deck, landscape).expect("page");
        assert_eq!((page.width, page.height, page.pages), (1280, 720, 3));
        let portrait = starter_page(Starter::Poster, landscape).expect("page");
        assert!(
            portrait.width > portrait.height,
            "a portrait starter turns wide"
        );

        let a4 = StarterSize {
            format: Some(PaperFormat::A4),
            ..StarterSize::default()
        };
        let page = starter_page(Starter::Card, a4).expect("page");
        assert_eq!(
            (page.width, page.height, page.format),
            (794, 1123, Some(PaperFormat::A4))
        );

        let custom = StarterSize {
            width: Some(1000),
            ..StarterSize::default()
        };
        let page = starter_page(Starter::Poster, custom).expect("page");
        assert_eq!((page.width, page.height, page.format), (1000, 960, None));

        let zero = StarterSize {
            height: Some(0),
            ..StarterSize::default()
        };
        assert_eq!(
            starter_page(Starter::Flyer, zero)
                .map_err(|e| e.exit_code)
                .err(),
            Some(2)
        );
    }

    #[test]
    fn sources_hold_only_layout_tokens_and_no_doc_id() {
        for &starter in Starter::value_variants() {
            let doc = parse(starter.source().as_bytes());
            assert!(doc.doc_id.is_none(), "{}", starter.kind());
            for token in &doc.tokens.tokens {
                assert!(
                    token.id.starts_with("space.") || token.id.starts_with("layout."),
                    "{}: {} is not a layout token; use the theme contract",
                    starter.kind(),
                    token.id
                );
            }
        }
    }

    #[test]
    fn natural_build_uses_the_starter_body_and_margin() {
        for &starter in Starter::value_variants() {
            let natural = starter.natural();
            let doc = build_doc(starter, None, StarterSize::default());
            assert_eq!(token_px(&doc.tokens, "size.body"), natural.body_px.round());
            assert_eq!(token_px(&doc.tokens, "space.margin"), natural.margin_px);
            assert_eq!(
                token_px(&doc.tokens, "layout.width"),
                f64::from(natural.width) - 2.0 * natural.margin_px
            );
            for page in &doc.body.pages {
                assert_eq!(
                    page.margin_top.as_ref().map(|d| d.value),
                    Some(natural.margin_px),
                    "{}",
                    starter.kind()
                );
            }
        }
    }

    #[test]
    fn a_larger_page_scales_type_space_and_page_box() {
        let square = StarterSize {
            width: Some(1080),
            height: Some(1080),
            ..StarterSize::default()
        };
        let doc = build_doc(Starter::Flyer, None, square);
        // fit = min(1080 / 559, 1080 / 794) = 1080 / 794.
        let fit = 1080.0 / 794.0;
        let margin = (32.0_f64 * fit).round();
        assert_eq!(token_px(&doc.tokens, "space.margin"), margin);
        assert_eq!(token_px(&doc.tokens, "layout.width"), 1080.0 - 2.0 * margin);
        assert_eq!(
            token_px(&doc.tokens, "layout.height"),
            1080.0 - 2.0 * margin
        );
        assert_eq!(token_px(&doc.tokens, "space.md"), (16.0_f64 * fit).round());
        assert_eq!(token_px(&doc.tokens, "size.body"), (17.0_f64 * fit).round());
        for page in &doc.body.pages {
            assert_eq!((page.width.value, page.height.value), (1080.0, 1080.0));
        }
    }

    #[test]
    fn theme_swaps_tokens_and_keeps_the_layout() {
        let lagoon = build_doc(Starter::Flyer, None, StarterSize::default());
        let cobalt = build_doc(Starter::Flyer, Some("cobalt"), StarterSize::default());
        let set = |doc: &Document, id: &str| {
            doc.tokens
                .tokens
                .iter()
                .find(|t| t.id == id)
                .and_then(|t| t.set.clone())
        };
        assert_eq!(
            set(&lagoon, "color.primary").as_deref(),
            Some("@zenith/theme.lagoon")
        );
        assert_eq!(
            set(&cobalt, "color.primary").as_deref(),
            Some("@zenith/theme.cobalt")
        );
        let page_ids = |doc: &Document| -> Vec<String> {
            doc.body.pages.iter().map(|p| p.id.clone()).collect()
        };
        assert_eq!(page_ids(&lagoon), page_ids(&cobalt));
        let own = |doc: &Document| -> Vec<(String, Option<f64>)> {
            doc.tokens
                .tokens
                .iter()
                .filter(|t| t.set.is_none())
                .map(|t| (t.id.clone(), literal_px(t)))
                .collect()
        };
        assert_eq!(
            own(&lagoon),
            own(&cobalt),
            "layout tokens do not depend on the theme"
        );
        assert!(cobalt.styles.styles.iter().any(|s| s.id == "ui.display"));
    }

    #[test]
    fn unknown_theme_is_an_error() {
        let err = starter_document(Starter::Card, Some("no-such-theme"), StarterSize::default())
            .expect_err("unknown theme");
        assert_eq!(err.exit_code, 2);
        assert!(err.message.contains("unknown theme"), "{}", err.message);
    }

    #[test]
    fn rename_sets_name_title_and_ids() {
        let theme = crate::library::resolve_theme_pack(None, "cobalt").expect("theme");
        let page = starter_page(Starter::Card, StarterSize::default()).expect("page");
        let bytes = materialize(
            Starter::Card,
            Some(("Studio Card", "studio-card".into())),
            &theme,
            page,
        )
        .expect("materialize");
        let doc = parse(&bytes);
        let project = doc.project.expect("project");
        assert_eq!(project.id, "proj.studio-card");
        assert_eq!(project.name, "Studio Card");
        assert_eq!(doc.body.id, "doc.studio-card");
        assert_eq!(doc.body.title.as_deref(), Some("Studio Card"));
    }
}
