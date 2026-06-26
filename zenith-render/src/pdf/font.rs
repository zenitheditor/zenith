//! CIDFont embedding for selectable / searchable PDF text.
//!
//! The PDF backend renders a [`SceneCommand::DrawGlyphRun`] with `selectable =
//! true` (the default) as REAL text: an `Identity-H` composite (`Type0`) font
//! whose descendant `CIDFontType2` carries the (optionally subsetted) font
//! program plus a `ToUnicode` CMap, so the text can be selected, copied,
//! searched, and indexed. A run with `selectable = false` is drawn as filled
//! glyph outlines instead (see [`super::content`]) and never reaches this module.
//!
//! Flow (driven by [`super::document`]):
//! 1. [`collect_usage`] walks every page's selectable glyph runs and records, per
//!    `font_id`, the set of glyph ids used and the Unicode text each maps back to.
//! 2. [`build_plan`] resolves each font's bytes, optionally subsets to the used
//!    glyphs (pure-Rust `subsetter`, no C deps), and produces a [`FontPlan`].
//! 3. The content translator emits each glyph as its CID via [`FontPlan::cid_of`].
//! 4. [`write_font`] materializes the five PDF objects per embedded font.
//!
//! A font that fails to resolve, parse, or subset is simply absent from the
//! [`FontPlan`]; the content translator falls back to outlines for its runs, so
//! the page still renders.

use std::collections::BTreeMap;

use pdf_writer::types::{CidFontType, FontFlags, SystemInfo};
use pdf_writer::{Filter, Finish, Name, Pdf, Rect, Ref, Str};
use zenith_core::FontProvider;
use zenith_scene::{Scene, SceneCommand};

/// Number of PDF objects written per embedded font: the `Type0` dict, the
/// descendant `CIDFont` dict, the font descriptor, the font-program stream, and
/// the `ToUnicode` CMap stream.
pub(super) const REFS_PER_FONT: i32 = 5;

/// Per-font glyph usage gathered from the scenes: every glyph id referenced by a
/// selectable run, mapped to the source Unicode text it represents (empty when no
/// source mapping was carried — the glyph is still embedded for rendering).
type Usage = BTreeMap<String, BTreeMap<u16, String>>;

/// One font ready to embed: the (possibly subsetted) program bytes, the
/// original-glyph-id → CID remap used in the content stream, the glyph→Unicode
/// map for `ToUnicode`, per-glyph advance widths, and the descriptor metrics.
pub(super) struct EmbeddedFont {
    /// Font program bytes to embed (subset output, or the raw face when not
    /// subsetting).
    bytes: Vec<u8>,
    /// `true` when `bytes` is a CFF/OpenType program (→ `FontFile3` / `OpenType`),
    /// `false` for a TrueType `glyf` program (→ `FontFile2`).
    is_cff: bool,
    /// Original glyph id → CID used in the content stream. With subsetting the CID
    /// is the compacted subset glyph id; without, CID == glyph id.
    gid_to_cid: BTreeMap<u16, u16>,
    /// CID → Unicode text, for the `ToUnicode` CMap (omits empty mappings).
    cid_to_unicode: BTreeMap<u16, String>,
    /// CID → advance width in font units.
    cid_widths: BTreeMap<u16, u16>,
    units_per_em: u16,
    ascent: i16,
    descent: i16,
    cap_height: i16,
    italic_angle: f32,
}

/// The document-wide set of embedded fonts plus a `font_id → index` lookup. The
/// index is both the position in [`FontPlan::fonts`] and the PDF resource id used
/// to form the resource name `f<index>` and the object-ref block.
pub(super) struct FontPlan {
    pub(super) fonts: Vec<EmbeddedFont>,
    index: BTreeMap<String, usize>,
}

impl FontPlan {
    /// Resolve `(resource_index, cid)` for `glyph_id` in `font_id`, or `None` when
    /// the font is not embedded (→ the caller draws the glyph as an outline).
    pub(super) fn cid_of(&self, font_id: &str, glyph_id: u16) -> Option<(usize, u16)> {
        let idx = *self.index.get(font_id)?;
        let cid = *self.fonts.get(idx)?.gid_to_cid.get(&glyph_id)?;
        Some((idx, cid))
    }

    /// The resource index for `font_id`, if embedded.
    pub(super) fn resource_index(&self, font_id: &str) -> Option<usize> {
        self.index.get(font_id).copied()
    }
}

/// Walk every selectable glyph run in `scenes` and accumulate per-font glyph
/// usage with Unicode mappings, in deterministic (`BTreeMap`) order.
pub(super) fn collect_usage(scenes: &[Scene]) -> Usage {
    let mut usage: Usage = BTreeMap::new();
    for scene in scenes {
        for cmd in &scene.commands {
            let SceneCommand::DrawGlyphRun {
                font_id,
                selectable,
                glyphs,
                ..
            } = cmd
            else {
                continue;
            };
            if !*selectable {
                continue;
            }
            let entry = usage.entry(font_id.clone()).or_default();
            for g in glyphs {
                let slot = entry.entry(g.glyph_id).or_default();
                // First non-empty mapping wins; keep a present-but-empty entry so
                // the glyph is still embedded (it renders) even without a mapping.
                if slot.is_empty() && !g.text.is_empty() {
                    *slot = g.text.clone();
                }
            }
        }
    }
    usage
}

/// Build the embedding plan from collected `usage`. When `subset` is true each
/// font is compacted to just its used glyphs; otherwise the whole face is
/// embedded with an identity glyph→CID map. Fonts that cannot be resolved,
/// parsed, or subsetted are skipped (their runs fall back to outlines).
pub(super) fn build_plan(usage: &Usage, fonts: &dyn FontProvider, subset: bool) -> FontPlan {
    let mut plan = FontPlan {
        fonts: Vec::new(),
        index: BTreeMap::new(),
    };
    for (font_id, glyphs) in usage {
        let Some(font_data) = fonts.by_id(font_id) else {
            continue;
        };
        let Some(embedded) = build_font(&font_data.bytes, font_data.index, glyphs, subset) else {
            continue;
        };
        let idx = plan.fonts.len();
        plan.fonts.push(embedded);
        plan.index.insert(font_id.clone(), idx);
    }
    plan
}

/// Build one [`EmbeddedFont`] from raw face bytes and the glyph ids it must
/// cover. Returns `None` if the face fails to parse or (when subsetting) the
/// subset fails.
fn build_font(
    bytes: &[u8],
    index: u32,
    glyphs: &BTreeMap<u16, String>,
    subset: bool,
) -> Option<EmbeddedFont> {
    let face = ttf_parser::Face::parse(bytes, index).ok()?;
    let units_per_em = face.units_per_em();
    if units_per_em == 0 {
        return None;
    }
    let is_cff = face.tables().cff.is_some();

    // Build the glyph→CID remap and the embedded program.
    let (program, gid_to_cid) = if subset {
        let mut remapper = subsetter::GlyphRemapper::new();
        for &gid in glyphs.keys() {
            remapper.remap(gid);
        }
        let subset_bytes = subsetter::subset(bytes, index, &remapper).ok()?;
        let mut map = BTreeMap::new();
        for &gid in glyphs.keys() {
            if let Some(cid) = remapper.get(gid) {
                map.insert(gid, cid);
            }
        }
        (subset_bytes, map)
    } else {
        // Identity: CID == glyph id, embed the whole face.
        let map: BTreeMap<u16, u16> = glyphs.keys().map(|&gid| (gid, gid)).collect();
        (bytes.to_vec(), map)
    };

    // CID-keyed Unicode and width maps (advances come from the ORIGINAL face).
    let mut cid_to_unicode = BTreeMap::new();
    let mut cid_widths = BTreeMap::new();
    for (&gid, text) in glyphs {
        let Some(&cid) = gid_to_cid.get(&gid) else {
            continue;
        };
        if !text.is_empty() {
            cid_to_unicode.insert(cid, text.clone());
        }
        let advance = face
            .glyph_hor_advance(ttf_parser::GlyphId(gid))
            .unwrap_or(0);
        cid_widths.insert(cid, advance);
    }

    Some(EmbeddedFont {
        bytes: program,
        is_cff,
        gid_to_cid,
        cid_to_unicode,
        cid_widths,
        units_per_em,
        ascent: face.ascender(),
        descent: face.descender(),
        cap_height: face.capital_height().unwrap_or_else(|| face.ascender()),
        italic_angle: face.italic_angle(),
    })
}

/// The five object refs backing one embedded font, derived from a contiguous
/// block so the whole document shares one sequential id space.
pub(super) struct FontRefs {
    type0: Ref,
    cid: Ref,
    descriptor: Ref,
    program: Ref,
    to_unicode: Ref,
}

impl FontRefs {
    /// The `Type0` font dict ref — the object a page's `/Font` entry points at.
    pub(super) fn type0_ref(&self) -> Ref {
        self.type0
    }
}

/// Compute the ref block for the `idx`-th font starting at `base`.
pub(super) fn font_refs_at(base: i32, idx: usize) -> FontRefs {
    let b = base + (idx as i32) * REFS_PER_FONT;
    FontRefs {
        type0: Ref::new(b),
        cid: Ref::new(b + 1),
        descriptor: Ref::new(b + 2),
        program: Ref::new(b + 3),
        to_unicode: Ref::new(b + 4),
    }
}

/// Write the five PDF objects for one embedded font.
pub(super) fn write_font(pdf: &mut Pdf, font: &EmbeddedFont, refs: &FontRefs) {
    let upem = f32::from(font.units_per_em);
    let to_thousand = |v: i16| f32::from(v) * 1000.0 / upem;

    // ── Type0 (composite) font ───────────────────────────────────────────────
    let mut t0 = pdf.type0_font(refs.type0);
    t0.base_font(Name(b"ZenithFont"));
    t0.encoding_predefined(Name(b"Identity-H"));
    t0.descendant_font(refs.cid);
    t0.to_unicode(refs.to_unicode);
    t0.finish();

    // ── Descendant CIDFontType2 ──────────────────────────────────────────────
    let mut cid = pdf.cid_font(refs.cid);
    cid.subtype(CidFontType::Type2);
    cid.base_font(Name(b"ZenithFont"));
    cid.system_info(SystemInfo {
        registry: Str(b"Adobe"),
        ordering: Str(b"Identity"),
        supplement: 0,
    });
    cid.font_descriptor(refs.descriptor);
    cid.default_width(1000.0);
    // CID == GID in the embedded program (subset is compacted, identity is raw).
    cid.cid_to_gid_map_predefined(Name(b"Identity"));
    {
        // One width per CID, in ascending CID order (BTreeMap iteration).
        let mut w = cid.widths();
        for (&c, &advance) in &font.cid_widths {
            let scaled = f32::from(advance) * 1000.0 / upem;
            w.consecutive(c, [scaled]);
        }
        w.finish();
    }
    cid.finish();

    // ── Font descriptor ──────────────────────────────────────────────────────
    let flags = if font.italic_angle != 0.0 {
        FontFlags::NON_SYMBOLIC | FontFlags::ITALIC
    } else {
        FontFlags::NON_SYMBOLIC
    };
    let mut desc = pdf.font_descriptor(refs.descriptor);
    desc.name(Name(b"ZenithFont"));
    desc.flags(flags);
    desc.bbox(Rect::new(
        0.0,
        to_thousand(font.descent),
        1000.0,
        to_thousand(font.ascent),
    ));
    desc.italic_angle(font.italic_angle);
    desc.ascent(to_thousand(font.ascent));
    desc.descent(to_thousand(font.descent));
    desc.cap_height(to_thousand(font.cap_height));
    // StemV has no reliable source in the face; a conventional mid value is used
    // (this hint only nudges substitute-font rendering, never the embedded glyphs).
    desc.stem_v(80.0);
    if font.is_cff {
        desc.font_file3(refs.program);
    } else {
        desc.font_file2(refs.program);
    }
    desc.finish();

    // ── Font-program stream (Flate-compressed) ───────────────────────────────
    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&font.bytes, 6);
    let mut stream = pdf.stream(refs.program, &compressed);
    stream.filter(Filter::FlateDecode);
    if font.is_cff {
        // A subsetted/raw OpenType-CFF program is embedded as an OpenType file.
        stream.pair(Name(b"Subtype"), Name(b"OpenType"));
    } else {
        // FontFile2 requires the uncompressed length in Length1.
        stream.pair(Name(b"Length1"), font.bytes.len() as i32);
    }
    stream.finish();

    // ── ToUnicode CMap stream ────────────────────────────────────────────────
    let cmap = build_tounicode_cmap(font);
    pdf.stream(refs.to_unicode, cmap.as_bytes()).finish();
}

/// Build a `ToUnicode` CMap mapping each CID to its source Unicode text, so PDF
/// readers can extract/search the rendered text. CIDs are written in ascending
/// order in `beginbfchar` blocks capped at 100 entries (the PDF limit).
fn build_tounicode_cmap(font: &EmbeddedFont) -> String {
    let mut entries: Vec<(u16, String)> = Vec::new();
    for (&cid, text) in &font.cid_to_unicode {
        let hex: String = text
            .chars()
            .flat_map(|c| {
                // UTF-16BE hex, so astral chars become a surrogate pair.
                let mut buf = [0u16; 2];
                c.encode_utf16(&mut buf)
                    .iter()
                    .map(|u| format!("{u:04X}"))
                    .collect::<Vec<_>>()
            })
            .collect();
        entries.push((cid, hex));
    }
    // entries already in ascending CID order (BTreeMap), but be explicit.
    entries.sort_by_key(|(cid, _)| *cid);

    let mut cmap = String::new();
    cmap.push_str("/CIDInit /ProcSet findresource begin\n");
    cmap.push_str("12 dict begin\nbegincmap\n");
    cmap.push_str("/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n");
    cmap.push_str("/CMapName /Adobe-Identity-UCS def\n");
    cmap.push_str("/CMapType 2 def\n");
    cmap.push_str("1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n");
    for chunk in entries.chunks(100) {
        cmap.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for (cid, hex) in chunk {
            cmap.push_str(&format!("<{cid:04X}> <{hex}>\n"));
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    cmap
}
