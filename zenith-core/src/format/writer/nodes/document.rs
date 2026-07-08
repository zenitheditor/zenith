//! The `document` body, the `page` node, and its page-metadata children
//! (`safe-zone`, `fold`, `block`).

use crate::ast::{
    ConstructionBlock, ConstructionGuideDef, DocumentBody, Fold, Page, SafeZone, SafeZoneType,
};

use crate::format::writer::{
    fmt_dimension, indent, write_opt_dimension, write_opt_property_value, write_opt_str,
    write_opt_str_escaped,
};

use super::helpers::write_block_style;
use super::write_children_block;

// ---------------------------------------------------------------------------
// Document body
// ---------------------------------------------------------------------------

pub(in crate::format::writer) fn write_document_body(
    body: &DocumentBody,
    out: &mut String,
    depth: usize,
) {
    indent(out, depth);
    out.push_str("document");
    out.push_str(" id=\"");
    out.push_str(&body.id);
    out.push('"');
    write_opt_str(out, "title", &body.title);
    out.push_str(" {\n");

    // Block style decls at document scope emitted before pages.
    for bs in &body.block_styles {
        write_block_style(bs, out, depth + 1);
    }

    for page in &body.pages {
        write_page(page, out, depth + 1);
    }

    indent(out, depth);
    out.push_str("}\n");
}

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

fn write_page(page: &Page, out: &mut String, depth: usize) {
    indent(out, depth);
    out.push_str("page");
    // Canonical order: id, name, source, fit, w, h, background
    out.push_str(" id=\"");
    out.push_str(&page.id);
    out.push('"');
    write_opt_str(out, "name", &page.name);
    write_opt_str(out, "source", &page.source);
    write_opt_str(out, "fit", &page.fit);
    out.push_str(" w=");
    out.push_str(&fmt_dimension(&page.width));
    out.push_str(" h=");
    out.push_str(&fmt_dimension(&page.height));
    write_opt_property_value(out, "background", &page.background);
    write_opt_dimension(out, "bleed", &page.bleed);
    write_opt_dimension(out, "baseline-grid", &page.baseline_grid);
    write_opt_str(out, "line-jumps", &page.line_jumps);
    write_opt_dimension(out, "margin-inner", &page.margin_inner);
    write_opt_dimension(out, "margin-outer", &page.margin_outer);
    write_opt_dimension(out, "margin-top", &page.margin_top);
    write_opt_dimension(out, "margin-bottom", &page.margin_bottom);
    write_opt_str(out, "parity", &page.parity);
    write_opt_str(out, "master", &page.master);

    out.push_str(" {\n");
    // Safe-zones, folds, and construction guides are page metadata, emitted
    // before block decls and renderable children.
    for zone in &page.safe_zones {
        write_safe_zone(zone, out, depth + 1);
    }
    for fold in &page.folds {
        write_fold(fold, out, depth + 1);
    }
    write_construction_block(&page.construction, out, depth + 1);
    // Block style decls at page scope emitted after safe-zones/folds, before children.
    for bs in &page.block_styles {
        write_block_style(bs, out, depth + 1);
    }
    write_children_block(&page.children, out, depth);
    indent(out, depth);
    out.push_str("}\n");
}

/// Emit a single `safe-zone` line:
/// `safe-zone id="..." type="exclusion|required" x=(px)N y=(px)N w=(px)N h=(px)N label="..."`
/// (`label` is omitted when `None`).
fn write_safe_zone(zone: &SafeZone, out: &mut String, depth: usize) {
    indent(out, depth);
    out.push_str("safe-zone");
    out.push_str(" id=\"");
    out.push_str(&zone.id);
    out.push('"');
    out.push_str(" type=\"");
    out.push_str(match zone.zone_type {
        SafeZoneType::Exclusion => "exclusion",
        SafeZoneType::Required => "required",
    });
    out.push('"');
    out.push_str(" x=");
    out.push_str(&fmt_dimension(&zone.x));
    out.push_str(" y=");
    out.push_str(&fmt_dimension(&zone.y));
    out.push_str(" w=");
    out.push_str(&fmt_dimension(&zone.w));
    out.push_str(" h=");
    out.push_str(&fmt_dimension(&zone.h));
    write_opt_str_escaped(out, "label", &zone.label);
    out.push('\n');
}

/// Emit a single `fold` line:
/// `fold id="..." orientation="vertical|horizontal" position=(px)N`
/// (`position` is omitted when `None`).
fn write_fold(fold: &Fold, out: &mut String, depth: usize) {
    indent(out, depth);
    out.push_str("fold");
    out.push_str(" id=\"");
    out.push_str(&fold.id);
    out.push('"');
    out.push_str(" orientation=\"");
    out.push_str(&fold.orientation);
    out.push('"');
    write_opt_dimension(out, "position", &fold.position);
    out.push('\n');
}

fn write_construction_block(block: &ConstructionBlock, out: &mut String, depth: usize) {
    if block.guides.is_empty() {
        return;
    }

    indent(out, depth);
    out.push_str("construction {\n");
    for guide in &block.guides {
        write_construction_guide(guide, out, depth + 1);
    }
    indent(out, depth);
    out.push_str("}\n");
}

fn write_construction_guide(guide: &ConstructionGuideDef, out: &mut String, depth: usize) {
    indent(out, depth);
    out.push_str("guide");
    out.push_str(" id=\"");
    out.push_str(&guide.id);
    out.push('"');
    out.push_str(" type=\"");
    out.push_str(&guide.guide_type);
    out.push('"');
    write_opt_dimension(out, "x1", &guide.x1);
    write_opt_dimension(out, "y1", &guide.y1);
    write_opt_dimension(out, "x2", &guide.x2);
    write_opt_dimension(out, "y2", &guide.y2);
    write_opt_dimension(out, "cx", &guide.cx);
    write_opt_dimension(out, "cy", &guide.cy);
    write_opt_dimension(out, "r", &guide.r);
    write_opt_str_escaped(out, "label", &guide.label);
    out.push('\n');
}
