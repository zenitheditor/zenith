//! The chain distributor: pour a shaped source across member boxes (inline
//! and markdown-block paths), plus the widow/orphan straddle adjustment.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, FontProvider, ResolvedToken, Style, TextNode};
use zenith_layout::{RustybuzzEngine, TextDirection};

use crate::compile::markdown_resolve::MdBlockMap;
use crate::compile::text::{
    BlockStyleEnv, ChainSourceShape, HyphenationContext, Line, LineDecoration, LineStyle,
    NodeShape, ShapeEnv, WordToken, en_us_hyphenator, flatten_lines_to_tokens, pack_lines,
    resolve_kerning_pairs, shape_source_blocks, shape_words,
};

use super::style::{resolve_chain_base_style, resolve_chain_style};
use super::types::{ChainAssignment, ChainAssignments, Member};

/// The document-wide style lookups threaded into [`distribute_chains`], bundled
/// so the distributor edge stays under the argument lint. `md_blocks` is the
/// parsed-markdown side-channel keyed by node id: a chain whose source id is
/// present here flows as BLOCKS; every other chain stays on the inline path.
#[derive(Clone, Copy)]
pub(super) struct ChainDocStyles<'a> {
    pub(super) resolved: &'a BTreeMap<String, ResolvedToken>,
    pub(super) style_map: &'a BTreeMap<&'a str, &'a Style>,
    pub(super) doc_block_styles: &'a [zenith_core::BlockStyle],
    pub(super) md_blocks: &'a MdBlockMap,
}

/// Shared distributor: shape each chain's source once and pour its words greedily
/// across the chain's ordered members. Used by [`resolve_chains_document`]; kept
/// scope-agnostic so the collection scope (one page vs. the whole document) is
/// the ONLY thing that differs between call sites.
pub(super) fn distribute_chains(
    members: &BTreeMap<String, Vec<Member>>,
    source: &BTreeMap<String, &TextNode>,
    source_page_styles: &BTreeMap<String, &[zenith_core::BlockStyle]>,
    doc_styles: ChainDocStyles,
    fonts: &dyn FontProvider,
    engine: &RustybuzzEngine,
    diagnostics: &mut Vec<Diagnostic>,
) -> ChainAssignments {
    let resolved = doc_styles.resolved;
    let style_map = doc_styles.style_map;
    let mut assignments: ChainAssignments = BTreeMap::new();

    for (chain_id, chain_members) in members {
        // A chain with no span-bearing source emits nothing.
        let Some(src) = source.get(chain_id) else {
            continue;
        };

        // Source writing direction drives RTL shaping for the whole chain (the
        // per-member emit re-reads each member's own direction for line layout).
        let direction = match src.direction.as_deref() {
            Some("rtl") => TextDirection::Rtl,
            _ => TextDirection::Ltr,
        };

        // ── BLOCK PATH ────────────────────────────────────────────────────
        // When the source id is in the parsed-markdown side-channel, this chain
        // flows as BLOCKS (headings styled, paragraphs spaced) across members.
        // Every other chain (and a markdown source that parsed to no blocks)
        // takes the historical inline path below — byte-identical.
        if let Some(blocks) = doc_styles.md_blocks.get(&src.id)
            && !blocks.is_empty()
        {
            distribute_block_chain(
                BlockChainInput {
                    src,
                    blocks: blocks.as_slice(),
                    chain_members: chain_members.as_slice(),
                    page_block_styles: source_page_styles.get(chain_id).copied().unwrap_or(&[]),
                    doc_styles,
                    direction,
                    fonts,
                    engine,
                },
                diagnostics,
                &mut assignments,
            );
            continue;
        }

        // Shape the source spans ONCE into word tokens with the shared style.
        let (families, font_size, base_weight, letter_spacing_px, spans) =
            resolve_chain_style(src, resolved, style_map, fonts, diagnostics);
        let kerning_pairs = resolve_kerning_pairs(&src.kerning_pairs, resolved);
        let (tokens, metrics) = shape_words(
            &spans,
            &families,
            NodeShape {
                font_size,
                base_weight,
                letter_spacing_px,
                kerning_pairs: &kerning_pairs,
                direction,
            },
            ShapeEnv { engine, fonts },
            diagnostics,
            &src.id,
            src.source_span,
        );

        // Opt-in hyphenation for the whole chain, read from the source node.
        // Absent → `None` → packing + flattening are byte-identical to before.
        let hyph_ctx = if src.hyphenate == Some(true) {
            en_us_hyphenator().map(|dict| HyphenationContext {
                dict: Some(dict),
                engine,
                fonts,
                families: &families,
                hyphen: "-",
                direction,
                // Chain-member break-word is a documented v0 follow-up (like the
                // chain drop-cap/runaround deferrals); the chain path keeps the
                // existing hyphenation-only behavior, byte-identical to before.
                break_word: false,
            })
        } else {
            None
        };

        // Widow/orphan minimum, read from the chain source node. `None` or a
        // value < 2 leaves the greedy height-cut unadjusted (byte-identical).
        let widow_orphan = src.widow_orphan.filter(|&n| n >= 2);

        // Distribute tokens across the members' boxes in order.
        let mut remaining = tokens;
        let last_member = chain_members.len().saturating_sub(1);
        for (mi, member) in chain_members.iter().enumerate() {
            // Greedy-wrap the REMAINING words to THIS box's width.
            let mut lines = pack_lines(
                remaining,
                member.w,
                metrics.space_advance,
                hyph_ctx.as_ref(),
                metrics.line_height,
            );

            if mi == last_member {
                // Last box: keep everything that remains (it may overflow; the
                // member's own overflow handling rides in compile_text). The
                // `remaining` queue is not read again after this iteration.
                assignments.insert(
                    member.id.clone(),
                    ChainAssignment {
                        lines,
                        metrics,
                        is_last_member: true,
                    },
                );
                break;
            }

            // How many leading lines fit this box height: include lines while
            // their cumulative `height_px` does not exceed `member.h`. When all
            // heights are the uniform `metrics.line_height` this is identical to
            // `floor(member.h / line_height)` (the previous formula) — both count
            // the same number of lines at every boundary. A zero-height box yields
            // 0 so content cascades into the next box, matching the prior guard.
            let max_lines = {
                let mut cum = 0.0_f64;
                let mut count = 0usize;
                for l in &lines {
                    cum += l.height_px;
                    if cum > member.h {
                        break;
                    }
                    count += 1;
                }
                count
            };
            let mut take = max_lines.min(lines.len());

            // Widow/orphan adjustment: if the greedy cut splits a paragraph
            // across this boundary, pull lines DOWN into the next box so neither
            // side is left with fewer than N lines of that paragraph.
            if let Some(n) = widow_orphan {
                take = adjust_for_widow_orphan(&lines, take, n as usize);
            }

            // Lines beyond `take` carry their words into the next box. Rebuild
            // the remaining token queue from the overflow lines (flatten back
            // into a single word stream so the next box re-wraps to its width),
            // merging any hyphenation fragments back into whole words.
            let overflow_lines = lines.split_off(take);
            remaining = flatten_lines_to_tokens(overflow_lines, hyph_ctx.as_ref());

            assignments.insert(
                member.id.clone(),
                ChainAssignment {
                    lines,
                    metrics,
                    is_last_member: false,
                },
            );
        }
    }

    assignments
}

/// Distribute a CHAINED markdown source across its members as styled BLOCKS.
///
/// Each [`MdBlock`] is shaped once (per-block font/size/fill via the shared
/// cascade) into a descriptor; this distributor then packs each block's tokens to
/// each member's OWN width (members differ) and tags every resulting [`Line`] with
/// that block's per-line style + height, so a heading and body paragraph keep
/// their own ascent/size while sharing one galley. Inter-block spacing is folded
/// into the LAST line of the previous block's `height_px`; the very first block's
/// space-before is suppressed (no gap at the galley top). When a block boundary
/// lands at a member-box bottom the trailing gap simply ends that box (v1).
///
/// Overflow beyond the last member rides the EXISTING chain overflow path: the
/// last member keeps all remaining lines and `chain_member` raises the existing
/// `text.fit_failed` diagnostic under `overflow="fit"`, so the "add a chained
/// box" guidance persists until the article fits.
struct BlockChainInput<'a> {
    src: &'a TextNode,
    blocks: &'a [zenith_core::MdBlock],
    chain_members: &'a [Member],
    page_block_styles: &'a [zenith_core::BlockStyle],
    doc_styles: ChainDocStyles<'a>,
    direction: TextDirection,
    fonts: &'a dyn FontProvider,
    engine: &'a RustybuzzEngine<'a>,
}

fn distribute_block_chain(
    input: BlockChainInput,
    diagnostics: &mut Vec<Diagnostic>,
    assignments: &mut ChainAssignments,
) {
    let BlockChainInput {
        src,
        blocks,
        chain_members,
        page_block_styles,
        doc_styles,
        direction,
        fonts,
        engine,
    } = input;

    // The chain source's base render style (families/size/weight) for the cascade
    // fallback. The returned spans are unused on the block path.
    // Use the base-style resolver (families/size/weight only) — the per-span
    // ResolvedSpan allocation is not needed on the block path.
    let (families, font_size, base_weight) = resolve_chain_base_style(
        src,
        doc_styles.resolved,
        doc_styles.style_map,
        fonts,
        diagnostics,
    );
    let kerning_pairs = resolve_kerning_pairs(&src.kerning_pairs, doc_styles.resolved);

    let descriptors = shape_source_blocks(
        src,
        blocks,
        ChainSourceShape {
            families: &families,
            node_font_size: font_size,
            base_weight,
            kerning_pairs: &kerning_pairs,
            direction,
        },
        BlockStyleEnv {
            resolved: doc_styles.resolved,
            page_block_styles,
            doc_block_styles: doc_styles.doc_block_styles,
        },
        ShapeEnv { engine, fonts },
        diagnostics,
    );

    // The chain's representative metrics = the FIRST block's metrics (used by the
    // baseline-grid snap + as the assignment-level fallback). Per-line style on
    // each Line carries the real per-block values for emit.
    let rep_metrics = descriptors.first().map(|d| d.metrics).unwrap_or_default();

    // Opt-in en-US hyphenation for prose blocks, read from the source node and
    // mirroring the inline chain path: absent → `None` → packing byte-identical.
    // Code blocks never hyphenate (they pass `None` regardless). Break-word stays
    // off, matching the inline chain path's documented behavior.
    let hyph_ctx = if src.hyphenate == Some(true) {
        en_us_hyphenator().map(|dict| HyphenationContext {
            dict: Some(dict),
            engine,
            fonts,
            families: &families,
            hyphen: "-",
            direction,
            break_word: false,
        })
    } else {
        None
    };

    // A FIFO of blocks awaiting placement. Each block's owned tokens are consumed
    // exactly once (no cloning): a straddling block re-queues its overflow tail at
    // the FRONT (re-wrapped to the next member's width). `style` carries the
    // per-line style/metrics/spacing; `is_spacer` marks a horizontal-rule gap.
    struct PendingBlock {
        index: usize,
        tokens: Vec<WordToken>,
        style: LineStyle,
        line_height: f64,
        space_advance: f64,
        space_after_px: f64,
        space_before_px: f64,
        is_spacer: bool,
        left_indent_px: f64,
        decoration: Option<LineDecoration>,
        /// Code blocks render raw — no hyphenation; prose blocks hyphenate when
        /// the source opts in (mirrors the single-box wrap path).
        hyphenate: bool,
    }
    let mut queue: std::collections::VecDeque<PendingBlock> = descriptors
        .into_iter()
        .enumerate()
        .map(|(index, d)| PendingBlock {
            index,
            // A code block (background decoration) renders raw; everything else
            // is prose eligible for hyphenation.
            hyphenate: !matches!(d.decoration, Some(LineDecoration::Background(_))),
            tokens: d.tokens,
            style: d.line_style,
            line_height: d.metrics.line_height,
            space_advance: d.metrics.space_advance,
            space_after_px: d.space_after_px,
            space_before_px: d.space_before_px,
            is_spacer: d.is_spacer,
            left_indent_px: d.left_indent_px,
            decoration: d.decoration,
        })
        .collect();

    let last_member = chain_members.len().saturating_sub(1);

    for (mi, member) in chain_members.iter().enumerate() {
        let mut member_lines: Vec<Line> = Vec::new();
        let mut used_h = 0.0_f64;
        let is_last = mi == last_member;
        // The block index of the previous line in THIS member, the gap-fold
        // target. `None` before any line in this member → no fold at the top.
        let mut prev_block_in_member: Option<usize> = None;
        // The `space_after` of the block that owns the previous line in THIS
        // member, captured when it was placed (its descriptor is consumed by then).
        let mut prev_space_after: f64 = 0.0;

        while let Some(block) = queue.pop_front() {
            // A spacer block (horizontal rule) is ONE empty line of the gap height
            // carrying the rule decoration (drawn centered in its band by emit).
            let mut block_lines: Vec<Line> = if block.is_spacer {
                vec![Line {
                    words: Vec::new(),
                    content_w: 0.0,
                    paragraph: block.index,
                    height_px: block.line_height,
                    line_style: Some(block.style),
                    left_indent_px: block.left_indent_px,
                    decoration: block.decoration,
                }]
            } else {
                // Prose blocks hyphenate when the source opts in; code blocks pass
                // `None` so their raw content is never split. The indent shrinks the
                // packing width so wrapped text stays inside the box (emit applies
                // the matching shift), mirroring the single-box indent slot.
                let pack_hyph = if block.hyphenate {
                    hyph_ctx.as_ref()
                } else {
                    None
                };
                let pack_w = (member.w - block.left_indent_px).max(0.0);
                let mut ls = pack_lines(
                    block.tokens,
                    pack_w,
                    block.space_advance,
                    pack_hyph,
                    block.line_height,
                );
                for l in &mut ls {
                    l.paragraph = block.index;
                    l.line_style = Some(block.style);
                    l.left_indent_px = block.left_indent_px;
                    l.decoration = block.decoration;
                }
                ls
            };

            // Fold the inter-block gap into the previous line of THIS member when
            // this is a NEW block (the prior member-top continuation gets no fold,
            // so the gap that ended the prior box is not double-counted). The gap
            // is `prev.space_after + this.space_before`.
            let gap = match prev_block_in_member {
                Some(prev_idx) if prev_idx != block.index => {
                    prev_space_after + block.space_before_px
                }
                _ => 0.0,
            };

            if is_last {
                // Last member keeps everything; overflow rides chain_member's own
                // `overflow="fit"` check + the assignment carries all leftover.
                if gap > 0.0
                    && let Some(prev_line) = member_lines.last_mut()
                {
                    prev_line.height_px += gap;
                }
                if block_lines.last().is_some() {
                    prev_block_in_member = Some(block.index);
                    prev_space_after = block.space_after_px;
                }
                member_lines.append(&mut block_lines);
                continue;
            }

            // Apply the gap to the previous line (folded into its height) before
            // measuring this block, so the gap counts against the box budget.
            if gap > 0.0
                && let Some(prev_line) = member_lines.last_mut()
            {
                prev_line.height_px += gap;
                used_h += gap;
            }

            // Place lines while the box height allows. A still-empty member always
            // takes at least the first line so content cannot stall.
            let mut placed = 0usize;
            for l in &block_lines {
                if used_h + l.height_px > member.h && !member_lines.is_empty() {
                    break;
                }
                used_h += l.height_px;
                placed += 1;
            }

            if placed == block_lines.len() {
                if block_lines.last().is_some() {
                    prev_block_in_member = Some(block.index);
                    prev_space_after = block.space_after_px;
                }
                member_lines.append(&mut block_lines);
                continue;
            }

            // The block straddles this member boundary: keep `placed` lines here,
            // re-queue the overflow tail (re-wrapped to the NEXT member's width).
            let kept: Vec<Line> = block_lines.drain(..placed).collect();
            member_lines.extend(kept);
            // Merge hyphen fragments back into whole words for prose tails so the
            // next member re-wraps cleanly; code tails carry `None` (never split).
            let tail_hyph = if block.hyphenate {
                hyph_ctx.as_ref()
            } else {
                None
            };
            let tail_tokens = flatten_lines_to_tokens(block_lines, tail_hyph);
            queue.push_front(PendingBlock {
                index: block.index,
                tokens: tail_tokens,
                style: block.style,
                line_height: block.line_height,
                space_advance: block.space_advance,
                space_after_px: block.space_after_px,
                // A continued tail carries NO space-before (the block already
                // started above); its space-after still applies after it ends.
                space_before_px: 0.0,
                is_spacer: false,
                left_indent_px: block.left_indent_px,
                decoration: block.decoration,
                hyphenate: block.hyphenate,
            });
            break;
        }

        assignments.insert(
            member.id.clone(),
            ChainAssignment {
                lines: member_lines,
                metrics: rep_metrics,
                is_last_member: is_last,
            },
        );

        if is_last {
            break;
        }
    }
}

/// Adjust a greedy height-cut `take` (number of lines kept in THIS box, out of
/// `lines`) to honor a widow/orphan minimum of `n` lines per paragraph across
/// the box boundary. Returns the possibly-reduced `take`; lines are only ever
/// moved DOWN into the next box (the greedy flow cannot push lines up).
///
/// The boundary splits a paragraph when the last kept line and the first
/// overflow line share a paragraph index. In that case:
/// - `top_count` = trailing lines of that paragraph kept in THIS box;
/// - `bottom_count` = leading lines of that paragraph in the NEXT box.
///
/// To satisfy the WIDOW rule the next box must start with ≥ `n` lines of the
/// paragraph, so if `bottom_count < n` we move `n - bottom_count` lines down. To
/// satisfy the ORPHAN rule this box must keep ≥ `n` lines of the paragraph, so if
/// the move would leave `top_count < n` we instead move the WHOLE top chunk of
/// the paragraph down (the paragraph then starts cleanly in the next box).
///
/// Degenerate cases (documented): if the adjustment would empty THIS box
/// (`take` → 0) the cut is LEFT as-is, since an empty box is worse than a
/// widow/orphan; likewise a paragraph shorter than `2n` lines cannot satisfy the
/// rule on both sides and falls back to being moved whole (or left, if that
/// empties the box).
fn adjust_for_widow_orphan(lines: &[Line], take: usize, n: usize) -> usize {
    // No straddle when nothing is kept, nothing overflows, or the boundary lines
    // belong to different paragraphs.
    if take == 0 || take >= lines.len() {
        return take;
    }
    let (Some(last_kept), Some(first_over)) = (lines.get(take - 1), lines.get(take)) else {
        return take;
    };
    if last_kept.paragraph != first_over.paragraph {
        return take;
    }
    let para = last_kept.paragraph;

    // Trailing lines of `para` kept in this box.
    let top_count = lines[..take]
        .iter()
        .rev()
        .take_while(|l| l.paragraph == para)
        .count();
    // Leading lines of `para` in the next box.
    let bottom_count = lines[take..]
        .iter()
        .take_while(|l| l.paragraph == para)
        .count();

    let mut new_take = take;
    if bottom_count < n {
        let need = n - bottom_count;
        new_take = take.saturating_sub(need);
    }
    // If the (possible) move still leaves the top side with < n lines of the
    // paragraph, move the whole top chunk down so the paragraph starts fresh.
    let top_after = top_count.saturating_sub(take - new_take);
    if top_after < n {
        new_take = take.saturating_sub(top_count);
    }

    // Never empty this box; if the rule cannot be honored without doing so,
    // leave the greedy cut unchanged (degenerate case).
    if new_take >= 1 { new_take } else { take }
}

#[cfg(test)]
mod widow_orphan_tests {
    use super::*;

    /// Build a line list from per-line paragraph indices (words/width are not
    /// read by `adjust_for_widow_orphan`).
    fn lines_with_paragraphs(paras: &[usize]) -> Vec<Line> {
        paras
            .iter()
            .map(|&p| Line {
                words: Vec::new(),
                content_w: 0.0,
                paragraph: p,
                height_px: 0.0,
                line_style: None,
                left_indent_px: 0.0,
                decoration: None,
            })
            .collect()
    }

    /// No straddle (the boundary lines belong to different paragraphs) → the cut
    /// is left exactly where the greedy fit put it.
    #[test]
    fn no_straddle_keeps_take() {
        // take=3: line 2 is paragraph 0, line 3 is paragraph 1 → no straddle.
        let lines = lines_with_paragraphs(&[0, 0, 0, 1, 1, 1]);
        assert_eq!(adjust_for_widow_orphan(&lines, 3, 2), 3);
    }

    /// Orphan: box 1 would keep a lone FIRST line of paragraph 1 (take=4 keeps
    /// [0,0,0,1]). With N=2 that single line is pulled down → take=3.
    #[test]
    fn orphan_single_first_line_pulled_down() {
        let lines = lines_with_paragraphs(&[0, 0, 0, 1, 1, 1]);
        assert_eq!(adjust_for_widow_orphan(&lines, 4, 2), 3);
    }

    /// Widow: the next box would start with a lone LAST line of paragraph 0
    /// (take=5 keeps [0,0,0,0,0], overflow [0,1,...] starts with 1 line of P0).
    /// With N=2 one line is pulled down so the next box starts with 2 lines of P0.
    #[test]
    fn widow_single_last_line_pulled_down() {
        let lines = lines_with_paragraphs(&[0, 0, 0, 0, 0, 0, 1, 1]);
        // take=5 → bottom_count(P0)=1 (line index 5), top_count=5. Pull 1 down.
        assert_eq!(adjust_for_widow_orphan(&lines, 5, 2), 4);
    }

    /// Both sides already satisfy N → no change.
    #[test]
    fn satisfied_both_sides_unchanged() {
        let lines = lines_with_paragraphs(&[0, 0, 0, 0]);
        // take=2: top=2 lines of P0, bottom=2 lines of P0 → fine.
        assert_eq!(adjust_for_widow_orphan(&lines, 2, 2), 2);
    }

    /// Degenerate: honoring the rule would empty the box → leave the cut as-is.
    #[test]
    fn degenerate_would_empty_box_left_as_is() {
        // Whole box is the tail of paragraph 1 (single line), next box continues
        // it. Pulling down would empty the box → unchanged.
        let lines = lines_with_paragraphs(&[1, 1, 1]);
        assert_eq!(adjust_for_widow_orphan(&lines, 1, 2), 1);
    }
}
