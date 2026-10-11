//! The starter kinds and the page each one is designed on.
//!
//! A starter source (`assets/starters/<kind>.zen`) holds the layout and a
//! few `space.*` / `layout.*` dimension tokens at their natural values. Every
//! colour, font, type size, radius, and style comes from the theme contract
//! (`color.*`, `font.*`, `size.*`, `radius.*`, `border.width`, `ui.*`), so the
//! source is completed by `build` and is not a standalone document.

use clap::ValueEnum;

use super::super::page::PaperFormat;

/// A finished layout that `zenith new --starter` writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Starter {
    /// A5 event flyer: hero, details, bullet list, call to action.
    Flyer,
    /// A4 letterhead with a sample letter, contact header, and footer.
    Letterhead,
    /// A4 handout: title, three headed bullet sections, a small table.
    Handout,
    /// 1280x720 deck: title slide, three-bullet slide, closing slide.
    Deck,
    /// 720x960 event poster: dominant title and a facts block.
    Poster,
    /// A4 menu: two sections of name, description, and price rows.
    Menu,
    /// 672x384 business card front: name, title, contact lines.
    Card,
    /// A4 report: three stat figures, a paragraph, a bar chart.
    Report,
}

/// The page a starter is designed on, and its type and margin there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Natural {
    /// Page width in px.
    pub width: u32,
    /// Page height in px.
    pub height: u32,
    /// The paper format of the natural size, if it is one.
    pub format: Option<PaperFormat>,
    /// Page count.
    pub pages: u32,
    /// `size.body` in px at the natural size.
    pub body_px: f64,
    /// Page margin in px at the natural size.
    pub margin_px: f64,
    /// The theme applied when `--theme` is absent.
    pub theme: &'static str,
}

impl Starter {
    /// The kind name as `--starter` spells it.
    pub fn kind(self) -> &'static str {
        match self {
            Self::Flyer => "flyer",
            Self::Letterhead => "letterhead",
            Self::Handout => "handout",
            Self::Deck => "deck",
            Self::Poster => "poster",
            Self::Menu => "menu",
            Self::Card => "card",
            Self::Report => "report",
        }
    }

    /// The embedded layout source, without theme tokens or a `doc-id`.
    pub fn source(self) -> &'static str {
        match self {
            Self::Flyer => include_str!("../../../../assets/starters/flyer.zen"),
            Self::Letterhead => include_str!("../../../../assets/starters/letterhead.zen"),
            Self::Handout => include_str!("../../../../assets/starters/handout.zen"),
            Self::Deck => include_str!("../../../../assets/starters/deck.zen"),
            Self::Poster => include_str!("../../../../assets/starters/poster.zen"),
            Self::Menu => include_str!("../../../../assets/starters/menu.zen"),
            Self::Card => include_str!("../../../../assets/starters/card.zen"),
            Self::Report => include_str!("../../../../assets/starters/report.zen"),
        }
    }

    /// The theme applied when `--theme` is absent.
    pub fn default_theme(self) -> &'static str {
        self.natural().theme
    }

    /// Natural page `(width, height)` in px.
    pub fn page_size(self) -> (u32, u32) {
        let n = self.natural();
        (n.width, n.height)
    }

    /// Number of pages in the starter.
    pub fn page_count(self) -> u32 {
        self.natural().pages
    }

    pub(super) fn natural(self) -> Natural {
        let a4 = |body_px, margin_px, theme| Natural {
            width: 794,
            height: 1123,
            format: Some(PaperFormat::A4),
            pages: 1,
            body_px,
            margin_px,
            theme,
        };
        match self {
            Self::Flyer => Natural {
                width: 559,
                height: 794,
                format: Some(PaperFormat::A5),
                pages: 1,
                body_px: 17.0,
                margin_px: 32.0,
                theme: "lagoon",
            },
            Self::Letterhead => a4(16.0, 56.0, "cobalt"),
            Self::Handout => a4(16.0, 56.0, "lagoon"),
            Self::Menu => a4(16.0, 64.0, "sorbet"),
            Self::Report => a4(16.0, 56.0, "cobalt"),
            Self::Deck => Natural {
                width: 1280,
                height: 720,
                format: None,
                pages: 3,
                body_px: 24.0,
                margin_px: 48.0,
                theme: "prism",
            },
            Self::Poster => Natural {
                width: 720,
                height: 960,
                format: None,
                pages: 1,
                body_px: 22.0,
                margin_px: 56.0,
                theme: "harbor",
            },
            Self::Card => Natural {
                width: 672,
                height: 384,
                format: None,
                pages: 1,
                body_px: 16.0,
                margin_px: 40.0,
                theme: "cobalt",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_matches_the_clap_value_name() {
        for &starter in Starter::value_variants() {
            let value = starter.to_possible_value().expect("visible value");
            assert_eq!(value.get_name(), starter.kind());
        }
    }

    #[test]
    fn the_help_text_states_the_natural_size() {
        for &starter in Starter::value_variants() {
            let n = starter.natural();
            let help = starter
                .to_possible_value()
                .and_then(|v| v.get_help().map(|h| h.to_string()))
                .expect("help text");
            let label = match n.format {
                Some(format) => format!("{format:?}").to_uppercase(),
                None => format!("{}x{}", n.width, n.height),
            };
            assert!(
                help.contains(&label),
                "{}: help '{help}' lacks '{label}'",
                starter.kind()
            );
        }
    }

    #[test]
    fn natural_format_matches_natural_size() {
        for &starter in Starter::value_variants() {
            let n = starter.natural();
            if let Some(format) = n.format {
                assert_eq!(
                    format.portrait_px(),
                    (n.width, n.height),
                    "{}",
                    starter.kind()
                );
            }
        }
    }
}
