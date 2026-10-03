//! The one definition of what every theme ships besides its palette: the
//! `ui.*` styles, the document `defaults` rows, and the heading-weight token.
//!
//! `theme new` renders its KDL from these tables. The embedded packs carry
//! hand-edited copies, and a drift test compares each pack against
//! [`kit_document_source`].
//!
//! Style ids sit under `ui.` because style, token, and node ids share one
//! namespace: a bare `card` style collides with a node `id="card"`. `rect`,
//! `frame`, and `ellipse` take no default: they are backgrounds, and a card
//! opts in with `style="ui.card"`.

use std::fmt::Write as _;

/// A theme style: its id and `(property, token id)` pairs in canonical order.
#[derive(Debug, Clone, Copy)]
pub struct ThemeStyle {
    /// Style id (`ui.*`).
    pub id: &'static str,
    /// `(property key, token id)` pairs. Every value is a token reference.
    pub props: &'static [(&'static str, &'static str)],
}

/// One document `defaults` row.
#[derive(Debug, Clone, Copy)]
pub struct ThemeDefault {
    /// Node kind keyword (`connector`, `shape`, `text`).
    pub kind: &'static str,
    /// Default style id for the kind.
    pub style: &'static str,
    /// Default label style id (`shape` and `connector` only).
    pub text_style: Option<&'static str>,
}

/// Build the style table and the id list from one list, so they cannot diverge.
macro_rules! theme_styles {
    ($($id:literal => [$(($prop:literal, $token:literal)),+ $(,)?]),+ $(,)?) => {
        /// Every theme style, in canonical order.
        pub const THEME_STYLES: &[ThemeStyle] = &[
            $(ThemeStyle { id: $id, props: &[$(($prop, $token)),+] }),+
        ];
        /// Ids of [`THEME_STYLES`], in the same order.
        pub const THEME_STYLE_IDS: &[&str] = &[$($id),+];
    };
}

theme_styles! {
    "ui.body" => [
        ("fill", "color.base.content"),
        ("font-family", "font.body"),
        ("font-size", "size.body"),
    ],
    "ui.h1" => [
        ("font-family", "font.heading"),
        ("font-size", "size.h1"),
        ("font-weight", "font.weight.heading"),
    ],
    "ui.h2" => [
        ("font-family", "font.heading"),
        ("font-size", "size.h2"),
        ("font-weight", "font.weight.heading"),
    ],
    "ui.caption" => [
        ("font-family", "font.body"),
        ("font-size", "size.caption"),
    ],
    "ui.label" => [
        ("font-family", "font.body"),
        ("font-size", "size.body"),
    ],
    "ui.control" => [
        ("radius", "radius.field"),
        ("stroke-width", "border.width"),
    ],
    "ui.button" => [
        ("fill", "color.primary"),
        ("radius", "radius.field"),
    ],
    "ui.card" => [
        ("fill", "color.base.200"),
        ("radius", "radius.box"),
        ("stroke", "color.base.300"),
        ("stroke-width", "border.width"),
    ],
    "ui.connector" => [
        ("stroke", "color.base.content"),
        ("stroke-width", "border.width"),
    ],
}

/// Every document `defaults` row, in canonical (kind) order.
pub const THEME_DEFAULTS: &[ThemeDefault] = &[
    ThemeDefault {
        kind: "connector",
        style: "ui.connector",
        text_style: Some("ui.caption"),
    },
    ThemeDefault {
        kind: "shape",
        style: "ui.control",
        text_style: Some("ui.label"),
    },
    ThemeDefault {
        kind: "text",
        style: "ui.body",
        text_style: None,
    },
];

/// Id of the heading font-weight token the `ui.h1` and `ui.h2` styles read.
pub const HEADING_WEIGHT_TOKEN_ID: &str = "font.weight.heading";
/// Declared type of [`HEADING_WEIGHT_TOKEN_ID`].
pub const HEADING_WEIGHT_TOKEN_TYPE: &str = "fontWeight";
/// Value of [`HEADING_WEIGHT_TOKEN_ID`].
pub const HEADING_WEIGHT_TOKEN_VALUE: u16 = 700;

/// KDL for the `token` line of the heading-weight token (no indent, no newline).
pub(super) fn heading_weight_token_kdl() -> String {
    format!(
        "token id=\"{HEADING_WEIGHT_TOKEN_ID}\" type=\"{HEADING_WEIGHT_TOKEN_TYPE}\" value={HEADING_WEIGHT_TOKEN_VALUE}"
    )
}

/// KDL for the `styles` and `defaults` blocks, indented for a block that sits
/// directly under `zenith`.
pub(super) fn styles_and_defaults_kdl() -> String {
    let mut s = String::from("  styles {\n");
    for style in THEME_STYLES {
        let _ = write!(s, "    style id=\"{}\" {{", style.id);
        for (prop, token) in style.props {
            let _ = write!(s, " {prop} (token)\"{token}\";");
        }
        s.push_str(" }\n");
    }
    s.push_str("  }\n  defaults {\n");
    for row in THEME_DEFAULTS {
        let _ = write!(s, "    {} style=\"{}\"", row.kind, row.style);
        if let Some(text_style) = row.text_style {
            let _ = write!(s, " text-style=\"{text_style}\"");
        }
        s.push('\n');
    }
    s.push_str("  }\n");
    s
}

/// A minimal `.zen` document holding only the kit: the heading-weight token,
/// the styles, and the `defaults`. Parse it to get the reference a pack must
/// match.
pub fn kit_document_source() -> String {
    format!(
        "zenith version=1 {{\n  project id=\"theme.kit\" name=\"Kit\"\n  tokens format=\"zenith-token-v1\" {{\n    {}\n  }}\n{}  document id=\"doc.kit\" title=\"Kit\" {{\n    page id=\"pg\" w=(px)10 h=(px)10 {{}}\n  }}\n}}\n",
        heading_weight_token_kdl(),
        styles_and_defaults_kdl()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_match_table() {
        let ids: Vec<&str> = THEME_STYLES.iter().map(|s| s.id).collect();
        assert_eq!(ids, THEME_STYLE_IDS);
    }

    #[test]
    fn defaults_reference_known_styles() {
        for row in THEME_DEFAULTS {
            assert!(THEME_STYLE_IDS.contains(&row.style), "{}", row.style);
            if let Some(t) = row.text_style {
                assert!(THEME_STYLE_IDS.contains(&t), "{t}");
            }
        }
    }
}
