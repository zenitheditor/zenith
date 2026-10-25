//! `font.not_embeddable`: a face that draws selectable text forbids
//! embedding in its OS/2 `fsType` bits.
//!
//! The PDF backend embeds every face a selectable glyph run uses. A
//! restricted or bitmap-only face must not embed, so the PDF draws its runs
//! as outlines instead, and that text is not selectable. This check reports
//! it at validate and compile time, before any export. It runs on every
//! page, because any page can export to PDF. Each face reports once per
//! page, on the first node that draws with it.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, font_embedding};

use super::owner::{Owners, subject};
use super::run::PrintInput;
use crate::ir::SceneCommand;

/// Every `font.not_embeddable` of the page, in font id order.
pub(super) fn not_embeddable(input: &PrintInput<'_, '_>, owners: &Owners) -> Vec<Diagnostic> {
    // First selectable run of each face: font id → command index.
    let mut first: BTreeMap<&str, usize> = BTreeMap::new();
    for (index, command) in input.env.commands.iter().enumerate() {
        if let SceneCommand::DrawGlyphRun {
            font_id,
            selectable: true,
            ..
        } = command
        {
            first.entry(font_id.as_str()).or_insert(index);
        }
    }
    let mut out = Vec::new();
    for (font_id, index) in first {
        let Some(face) = input.env.shape.fonts.by_id(font_id) else {
            continue;
        };
        let license = font_embedding(&face.bytes, face.index);
        if !license.forbids_embedding() {
            continue;
        }
        let Some(command) = input.env.commands.get(index) else {
            continue;
        };
        let who = subject(input, owners, index, command);
        let message = format!(
            "font '{font_id}' used by {} has a {} — PDF export draws this text as outlines, \
             so it is not selectable. Use a face that allows embedding, or set \
             selectable=#false to accept outlines.",
            who.name,
            license.describe(),
        );
        out.push(Diagnostic::warning(
            "font.not_embeddable",
            message,
            who.span,
            Some(who.id.to_owned()),
        ));
    }
    out
}
