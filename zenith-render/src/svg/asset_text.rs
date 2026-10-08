use crate::RenderError;
use resvg::usvg::{self, NodeExt, TextToPath};

/// Convert asset text through registered fonts and preserve errors instead of dropping ink.
pub(super) fn outline(
    root: &usvg::Node,
    database: &usvg::fontdb::Database,
) -> Result<(), RenderError> {
    let mut text_nodes = Vec::new();
    for node in root.descendants() {
        if matches!(*node.borrow(), usvg::NodeKind::Text(_)) {
            text_nodes.push(node.clone());
        }
        let mut result = Ok(());
        node.subroots(|subroot| {
            if result.is_ok() {
                result = outline(&subroot, database);
            }
        });
        result?;
    }
    for node in text_nodes {
        let replacement = if let usvg::NodeKind::Text(text) = &*node.borrow() {
            check_fonts(text, database)?;
            let parent = node.parent().ok_or_else(|| {
                RenderError::new(format!("SVG asset text {} lacks a parent", text.id))
            })?;
            let transform = parent.abs_transform().pre_concat(text.transform);
            let replacement = text.convert(database, transform);
            if replacement.is_none()
                && text.chunks.iter().any(|chunk| {
                    chunk
                        .text
                        .chars()
                        .any(|character| !character.is_whitespace())
                })
            {
                return Err(RenderError::new(format!(
                    "SVG asset text {} cannot resolve registered font outlines; register its fonts",
                    text.id
                )));
            }
            replacement
        } else {
            None
        };
        if let Some(replacement) = replacement {
            node.insert_after(replacement);
        }
        node.detach();
    }
    Ok(())
}

fn check_fonts(text: &usvg::Text, database: &usvg::fontdb::Database) -> Result<(), RenderError> {
    use usvg::fontdb::{Family, Query, Stretch, Style, Weight};
    for chunk in &text.chunks {
        for span in &chunk.spans {
            let content = chunk.text.get(span.start..span.end).ok_or_else(|| {
                RenderError::new(format!(
                    "SVG asset text {} has an invalid span range {}..{}; correct the asset",
                    text.id, span.start, span.end
                ))
            })?;
            if content.chars().all(char::is_whitespace) {
                continue;
            }
            let mut families: Vec<_> = span
                .font
                .families
                .iter()
                .map(|family| match family.as_str() {
                    "serif" => Family::Serif,
                    "sans-serif" => Family::SansSerif,
                    "cursive" => Family::Cursive,
                    "fantasy" => Family::Fantasy,
                    "monospace" => Family::Monospace,
                    _ => Family::Name(family),
                })
                .collect();
            // Match usvg's final generic-family fallback exactly.
            families.push(Family::Serif);
            let stretch = match span.font.stretch {
                usvg::FontStretch::UltraCondensed => Stretch::UltraCondensed,
                usvg::FontStretch::ExtraCondensed => Stretch::ExtraCondensed,
                usvg::FontStretch::Condensed => Stretch::Condensed,
                usvg::FontStretch::SemiCondensed => Stretch::SemiCondensed,
                usvg::FontStretch::Normal => Stretch::Normal,
                usvg::FontStretch::SemiExpanded => Stretch::SemiExpanded,
                usvg::FontStretch::Expanded => Stretch::Expanded,
                usvg::FontStretch::ExtraExpanded => Stretch::ExtraExpanded,
                usvg::FontStretch::UltraExpanded => Stretch::UltraExpanded,
            };
            let style = match span.font.style {
                usvg::FontStyle::Normal => Style::Normal,
                usvg::FontStyle::Italic => Style::Italic,
                usvg::FontStyle::Oblique => Style::Oblique,
            };
            let query = Query {
                families: &families,
                weight: Weight(span.font.weight),
                stretch,
                style,
            };
            let font=database.query(&query).ok_or_else(||RenderError::new(format!("SVG asset text {} cannot resolve font families {} for span {}..{}; register its fonts",text.id,span.font.families.join(", "),span.start,span.end)))?;
            let valid = database
                .with_face_data(font, |bytes, index| {
                    ttf_parser::Face::parse(bytes, index).is_ok()
                })
                .unwrap_or(false);
            if !valid {
                return Err(RenderError::new(format!(
                    "SVG asset text {} resolves an invalid font; register valid font bytes",
                    text.id
                )));
            }
        }
    }
    Ok(())
}
