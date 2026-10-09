//! Source patching over the commented examples, one op at a time.
//!
//! For each op the test runs the transaction, patches the example source,
//! and checks the patch:
//! - every `//` comment survives,
//! - every byte outside the touched nodes is unchanged,
//! - the patched text parses to the transaction's after document.
//!
//! Property-level ops patch in place. Structural ops (add, remove, duplicate,
//! move, group, ungroup, reparent, pages, tokens, styles, masters, assets)
//! patch in place too: only the comments attached to a removed node go.
//! The cases that still fall back are listed with their reason code.

use std::collections::BTreeMap;

use zenith_core::{
    Document, KdlAdapter, KdlSource, Node, PatchErrorCode, Span, patch_source, strip_spans,
    try_patch_source,
};
use zenith_tx::{Transaction, TxStatus, run_transaction};

/// What the patch of one op must do.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Expect {
    /// Patch in place.
    Exact,
    /// Patch in place by inserting exactly this text and nothing else.
    Inserts(&'static str),
    /// Patch a structural edit in place. Lines outside the nodes `ids` stay
    /// byte-identical, and exactly the comments in `lost` go.
    Structural {
        ids: &'static [&'static str],
        lost: &'static [&'static str],
    },
    /// Fall back to canonical text for this reason.
    Fallback(PatchErrorCode),
}

struct Case {
    op: &'static str,
    expect: Expect,
    /// Ids the op edits beyond `affected_node_ids` (pages, tokens, styles).
    extra: &'static [&'static str],
}

const fn exact(op: &'static str) -> Case {
    Case {
        op,
        expect: Expect::Exact,
        extra: &[],
    }
}

const fn exact_on(op: &'static str, extra: &'static [&'static str]) -> Case {
    Case {
        op,
        expect: Expect::Exact,
        extra,
    }
}

const fn inserts(op: &'static str, text: &'static str) -> Case {
    Case {
        op,
        expect: Expect::Inserts(text),
        extra: &[],
    }
}

const fn structural(
    op: &'static str,
    ids: &'static [&'static str],
    lost: &'static [&'static str],
) -> Case {
    Case {
        op,
        expect: Expect::Structural { ids, lost },
        extra: &[],
    }
}

const fn fallback(op: &'static str, code: PatchErrorCode) -> Case {
    Case {
        op,
        expect: Expect::Fallback(code),
        extra: &[],
    }
}

fn example(name: &str) -> String {
    let path = format!("{}/../examples/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

fn parse(src: &str) -> Document {
    KdlAdapter
        .parse(src.as_bytes())
        .unwrap_or_else(|e| panic!("parse: {}\n{src}", e.message))
}

fn collect_node_spans(nodes: &[Node], out: &mut BTreeMap<String, Span>) {
    for node in nodes {
        if let (Some(id), Some(span)) = (node.id(), node.source_span()) {
            out.insert(id.to_owned(), span);
        }
        if let Some(children) = node.children() {
            collect_node_spans(children, out);
        }
        if let Node::Pattern(p) = node {
            collect_node_spans(std::slice::from_ref(&*p.motif), out);
        }
    }
}

/// Source span of every id-bearing page, node, master, master node, asset,
/// token, and style.
fn id_spans(doc: &Document) -> BTreeMap<String, Span> {
    let mut out = BTreeMap::new();
    for page in &doc.body.pages {
        if let Some(span) = page.source_span {
            out.insert(page.id.clone(), span);
        }
        collect_node_spans(&page.children, &mut out);
    }
    for master in &doc.masters {
        if let Some(span) = master.source_span {
            out.insert(master.id.clone(), span);
        }
        collect_node_spans(&master.children, &mut out);
    }
    for asset in &doc.assets.assets {
        if let Some(span) = asset.source_span {
            out.insert(asset.id.clone(), span);
        }
    }
    for token in &doc.tokens.tokens {
        if let Some(span) = token.source_span {
            out.insert(token.id.clone(), span);
        }
    }
    for style in &doc.styles.styles {
        if let Some(span) = style.source_span {
            out.insert(style.id.clone(), span);
        }
    }
    out
}

/// `src` with the spans of `ids` replaced by a placeholder. Spans nested in
/// another masked span are covered by the outer one.
fn mask(src: &str, doc: &Document, ids: &[String]) -> String {
    let spans = id_spans(doc);
    let mut ranges: Vec<(usize, usize, &str)> = ids
        .iter()
        .filter_map(|id| spans.get(id).map(|s| (s.start, s.end, id.as_str())))
        .collect();
    ranges.sort();
    let mut out = String::new();
    let mut cursor = 0;
    for (start, end, id) in ranges {
        if start < cursor {
            continue;
        }
        out.push_str(&src[cursor..start]);
        out.push_str(&format!("<{id}>"));
        cursor = end;
    }
    out.push_str(&src[cursor..]);
    out
}

fn comments(src: &str) -> Vec<&str> {
    src.lines()
        .filter_map(|l| l.find("//").map(|i| &l[i..]))
        .collect()
}

/// `true` for a line that holds only a comment.
fn comment_line(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("//") || t.starts_with("/*") || t.starts_with('*') || t.ends_with("*/")
}

/// The non-blank lines of `src` outside the nodes `ids`. A node covers the
/// lines its span touches and the comment lines directly above it. An entry
/// `=name` covers the lines of an id-less container block such as `styles`
/// whose braces the edit opens or closes.
fn kept_lines<'s>(src: &'s str, doc: &Document, ids: &[&str]) -> Vec<&'s str> {
    let spans = id_spans(doc);
    let mut starts = vec![0];
    starts.extend(src.match_indices('\n').map(|(i, _)| i + 1));
    let lines: Vec<&str> = src.split('\n').collect();
    let mut covered = vec![false; lines.len()];
    for id in ids {
        let Some(span) = spans.get(*id) else {
            continue;
        };
        let first = starts.partition_point(|&s| s <= span.start) - 1;
        let last = starts.partition_point(|&s| s < span.end) - 1;
        for c in &mut covered[first..=last] {
            *c = true;
        }
        let mut above = first;
        while above > 0 && comment_line(lines[above - 1]) {
            above -= 1;
            covered[above] = true;
        }
    }
    let containers: Vec<&str> = ids.iter().filter_map(|i| i.strip_prefix('=')).collect();
    lines
        .iter()
        .zip(&covered)
        .filter(|(l, c)| {
            let t = l.trim();
            !**c && !t.is_empty() && !containers.iter().any(|n| t.starts_with(n)) && t != "}"
        })
        .map(|(l, _)| *l)
        .collect()
}

fn check(file: &str, src: &str, case: &Case) {
    let before = parse(src);
    let tx = Transaction::from_json(&format!(r#"{{"ops":[{}]}}"#, case.op))
        .unwrap_or_else(|e| panic!("{file}: {}: {e}", case.op));
    let result = run_transaction(&before, &tx).expect("run");
    assert_ne!(
        result.status,
        TxStatus::Rejected,
        "{file}: {} was rejected: {:?}",
        case.op,
        result.diagnostics
    );
    assert_ne!(
        result.source_before, result.source_after,
        "{file}: {} changed nothing",
        case.op
    );
    let after = &result.document_after;
    let patched = patch_source(src, &before, after).expect("patch_source");
    assert_eq!(
        strip_spans(parse(&patched.text)),
        strip_spans(after.clone()),
        "{file}: {}: patched text must parse to the after document",
        case.op
    );

    match case.expect {
        Expect::Fallback(code) => {
            assert!(
                patched.reformatted,
                "{file}: {} must fall back:\n{}",
                case.op, patched.text
            );
            assert_eq!(patched.text, result.source_after);
            let err = try_patch_source(src, &before, after).expect_err("fallback");
            assert_eq!(err.code, code, "{file}: {}: {err}", case.op);
        }
        Expect::Structural { ids, lost } => {
            if let Err(e) = try_patch_source(src, &before, after) {
                panic!("{file}: {} must patch in place: {e}", case.op);
            }
            assert!(!patched.reformatted);
            let mut want = comments(src);
            for gone in lost {
                let at = want
                    .iter()
                    .position(|c| c == gone)
                    .unwrap_or_else(|| panic!("{file}: no comment {gone:?}"));
                want.remove(at);
            }
            let mut got = comments(&patched.text);
            want.sort_unstable();
            got.sort_unstable();
            assert_eq!(got, want, "{file}: {} comments:\n{}", case.op, patched.text);
            let patched_doc = parse(&patched.text);
            assert_eq!(
                kept_lines(src, &before, ids),
                kept_lines(&patched.text, &patched_doc, ids),
                "{file}: {} changed lines outside {ids:?}:\n{}",
                case.op,
                patched.text
            );
        }
        Expect::Inserts(text) => {
            assert!(!patched.reformatted, "{file}: {}", case.op);
            assert_eq!(
                patched.text.replacen(text, "", 1),
                src,
                "{file}: {} must only insert {text:?}:\n{}",
                case.op,
                patched.text
            );
        }
        Expect::Exact => {
            if let Err(e) = try_patch_source(src, &before, after) {
                panic!("{file}: {} must patch in place: {e}", case.op);
            }
            assert!(!patched.reformatted);
            for comment in comments(src) {
                assert!(
                    patched.text.contains(comment),
                    "{file}: {} lost comment {comment:?}",
                    case.op
                );
            }
            let mut ids = result.affected_node_ids.clone();
            ids.extend(case.extra.iter().map(|s| (*s).to_owned()));
            let patched_doc = parse(&patched.text);
            assert_eq!(
                mask(src, &before, &ids),
                mask(&patched.text, &patched_doc, &ids),
                "{file}: {} changed bytes outside {ids:?}:\n{}",
                case.op,
                patched.text
            );
        }
    }
}

fn check_all(file: &str, cases: &[Case]) {
    let src = example(file);
    for case in cases {
        check(file, &src, case);
    }
}

#[test]
fn stack_example_ops() {
    check_all(
        "stack.zen",
        &[
            exact(r#"{"op":"set_geometry","node":"title","x":40}"#),
            exact(r#"{"op":"set_geometry","node":"card.a","w":420,"h":70}"#),
            exact(r#"{"op":"set_geometry","node":"accent","rotate":15}"#),
            exact(r#"{"op":"nudge_geometry","node":"title","dx":4,"dy":-2,"dw":10}"#),
            exact(r#"{"op":"nudge_geometry","node":"card.b","dw":-16,"dh":8}"#),
            exact(r#"{"op":"nudge_anchor_gap","node":"card.a","dy":6}"#),
            exact(r#"{"op":"nudge_anchor_gap","node":"accent","dx":-4}"#),
            exact(
                r#"{"op":"set_anchor","node":"card.c","anchor_sibling":"card.a","anchor_gap":"(pt)9"}"#,
            ),
            exact(r#"{"op":"set_anchor","node":"label.b","anchor":"center-left"}"#),
            exact(r#"{"op":"detach_anchor","node":"card.a"}"#),
            exact(r#"{"op":"detach_anchor","node":"label.c"}"#),
            exact(r#"{"op":"set_fill","node":"accent","fill":"color.title"}"#),
            exact(r#"{"op":"set_stroke","node":"card.b","stroke":"color.accent"}"#),
            exact(r#"{"op":"set_stroke_width","node":"card.c","stroke_width":"size.radius"}"#),
            exact(
                r#"{"op":"set_node_token","node":"card.a","property":"radius","token":"size.border"}"#,
            ),
            exact(r#"{"op":"set_node_token","node":"card.b","property":"radius","token":null}"#),
            exact(
                r#"{"op":"set_node_token","node":"title","property":"font-size","token":"size.body"}"#,
            ),
            exact(
                r#"{"op":"set_node_token","node":"label.a","property":"font_size","token":"size.heading"}"#,
            ),
            exact(r#"{"op":"set_span_text","node":"label.b","span":0,"text":"Renamed"}"#),
            exact(r#"{"op":"set_opacity","node":"accent","opacity":0.5}"#),
            exact(r#"{"op":"set_visible","node":"label.b","visible":false}"#),
            exact(r#"{"op":"set_locked","node":"card.c","locked":true}"#),
            exact(r#"{"op":"set_text_align","node":"label.a","align":"start"}"#),
            exact(r#"{"op":"set_text_direction","node":"title","direction":"rtl"}"#),
            exact(r#"{"op":"set_text_overflow","node_id":"label.c","overflow":"visible"}"#),
            exact(r#"{"op":"replace_text","node":"title","spans":[{"text":"Hello"}]}"#),
            exact(r#"{"op":"find_replace_text","find":"Featured","replace":"Top"}"#),
            exact_on(
                r##"{"op":"update_token_value","id":"color.accent","value":"#ff0000"}"##,
                &["color.accent"],
            ),
            exact_on(
                r#"{"op":"set_page_size","page":"page.stack","w":"(px)500","h":"(px)380"}"#,
                &["page.stack"],
            ),
            // Still falls back: a second span inside the inline
            // `{ span "…" }` block shares the line with the first.
            fallback(
                r#"{"op":"replace_text","node":"label.a","spans":[{"text":"A"},{"text":"B"}]}"#,
                PatchErrorCode::UnsupportedLayout,
            ),
            structural(
                r#"{"op":"remove_node","node":"label.c"}"#,
                &["label.c"],
                &[],
            ),
            structural(
                r#"{"op":"remove_node","node":"accent"}"#,
                &["accent"],
                &["// Accent rule — placed AFTER (right of) the title, vertically centered."],
            ),
            structural(r#"{"op":"move_to_front","node":"title"}"#, &["title"], &[]),
            structural(
                r#"{"op":"move_to_back","node":"label.c"}"#,
                &["label.c"],
                &[],
            ),
            structural(
                r#"{"op":"move_forward","node":"accent"}"#,
                &["accent", "card.a"],
                &[],
            ),
            structural(
                r#"{"op":"move_backward","node":"card.b"}"#,
                &["card.b", "label.a"],
                &[],
            ),
            structural(
                r#"{"op":"duplicate_node","node":"card.a","new_id":"card.d"}"#,
                &["card.d"],
                &[],
            ),
            // Every node is anchored to a sibling, so the group takes them all.
            structural(
                r#"{"op":"group","node_ids":["title","accent","card.a","label.a","card.b","label.b","card.c","label.c"],"group_id":"grp"}"#,
                &[
                    "grp", "title", "accent", "card.a", "label.a", "card.b", "label.b", "card.c",
                    "label.c",
                ],
                &[],
            ),
            structural(
                r##"{"op":"create_token","id":"color.new","type":"color","value":"#123456"}"##,
                &["color.new"],
                &[],
            ),
            structural(
                r#"{"op":"create_style","id":"s2","properties":{"fill":"color.card"}}"#,
                &["s2", "=styles"],
                &[],
            ),
            structural(
                r#"{"op":"add_page","id":"pg2","w":"(px)480","h":"(px)360"}"#,
                &["pg2"],
                &[],
            ),
            structural(
                r#"{"op":"duplicate_page","page":"page.stack","new_id":"page.copy","id_suffix":".v2"}"#,
                &["page.copy"],
                &[],
            ),
            // No `masters` block yet: the patch creates it.
            structural(
                r#"{"op":"create_master","id":"m.one"}"#,
                &["m.one", "=masters"],
                &[],
            ),
        ],
    );
}

#[test]
fn pattern_example_ops() {
    check_all(
        "pattern.zen",
        &[
            exact(r#"{"op":"set_geometry","node":"bg.dots","x":8,"y":8}"#),
            exact(r#"{"op":"set_geometry","node":"bg.stars","w":400,"h":300}"#),
            exact(r#"{"op":"set_fill","node":"sw.grid","fill":"color.star"}"#),
            exact(r#"{"op":"set_opacity","node":"bg.stars","opacity":0.8}"#),
            exact(r#"{"op":"set_visible","node":"lbl.scatter","visible":false}"#),
            exact(
                r#"{"op":"align_nodes","node_ids":["sw.grid","sw.scatter"],"align":"left","anchor":"(px)30"}"#,
            ),
            exact(r#"{"op":"replace_text","node":"lbl.grid","spans":[{"text":"grid"}]}"#),
            exact_on(
                r##"{"op":"update_token_value","id":"color.dot","value":"#00ff00"}"##,
                &["color.dot"],
            ),
            structural(
                r#"{"op":"remove_node","node":"sw.grid"}"#,
                &["sw.grid"],
                &[],
            ),
            structural(
                r#"{"op":"remove_node","node":"bg.stars"}"#,
                &["bg.stars"],
                &["// Scatter: exactly `count` instances at seed-derived pseudo-random positions."],
            ),
            structural(r#"{"op":"move_to_back","node":"title"}"#, &["title"], &[]),
            structural(
                r#"{"op":"move_to_front","node":"bg.dots"}"#,
                &["bg.dots"],
                &[],
            ),
            structural(
                r#"{"op":"duplicate_node","node":"bg.dots","new_id":"bg.dots2"}"#,
                &["bg.dots2"],
                &[],
            ),
            structural(
                r#"{"op":"group","node_ids":["sw.grid","lbl.grid"],"group_id":"legend.grid"}"#,
                &["legend.grid", "sw.grid", "lbl.grid"],
                &[],
            ),
        ],
    );
}

#[test]
fn connector_routing_example_ops() {
    check_all(
        "connector-routing.zen",
        &[
            exact(r#"{"op":"set_stroke","node":"e.flow","stroke":"c.cross"}"#),
            exact(r#"{"op":"set_stroke_width","node":"e.cross","stroke_width":"bw"}"#),
            exact(r#"{"op":"set_geometry","node":"n.block","x":300}"#),
            exact(r#"{"op":"nudge_geometry","node":"n.block","dx":12,"dy":-6}"#),
            exact(r#"{"op":"set_fill","node":"n.top","fill":"c.block"}"#),
            exact(r#"{"op":"set_opacity","node":"e.flow","opacity":0.7}"#),
            exact_on(
                r#"{"op":"set_style_property","style_id":"lbl","property":"fill","value":"c.flow"}"#,
                &["lbl"],
            ),
            exact_on(
                r##"{"op":"update_token_value","id":"c.bg","value":"#ffffff"}"##,
                &["c.bg"],
            ),
            structural(
                r#"{"op":"remove_node","node":"e.cross"}"#,
                &["e.cross"],
                &["// A second connector that crosses the flow; the page's line-jumps hops it."],
            ),
            structural(
                r#"{"op":"move_forward","node":"n.start"}"#,
                &["n.start", "n.end"],
                &[],
            ),
            structural(
                r#"{"op":"move_to_front","node":"e.flow"}"#,
                &["e.flow"],
                &[],
            ),
            structural(
                r#"{"op":"duplicate_node","node":"n.block","new_id":"n.block2"}"#,
                &["n.block2"],
                &[],
            ),
            structural(
                r##"{"op":"create_token","id":"c.new","type":"color","value":"#123456"}"##,
                &["c.new"],
                &[],
            ),
        ],
    );
}

/// A commented fixture with the node kinds the examples above lack: paths,
/// polygons, images, a layout frame, assets, masters, and defaults.
const RICH: &str = r##"zenith version=1 {
  project id="proj.rich" name="Rich"
  assets {
    asset id="img.a" kind="image" src="images/a.png"
    asset id="img.b" kind="image" src="images/b.png"
  }
  // Colours.
  tokens format="zenith-token-v1" {
    token id="c.a" type="color" value="#112233"
    token id="c.b" type="color" value="#445566"
  }
  styles {
    style id="box" {
      fill (token)"c.a" // box fill
    }
  }
  masters {
    master id="m.one" {
      rect id="bar" x=(px)0 y=(px)0 w=(px)800 h=(px)4 fill=(token)"c.b"
    }
  }
  document id="doc.rich" title="Rich" {
    // Page one.
    page id="pg" w=(px)800 h=(px)600 {
      // A path with four anchors.
      path id="p1" closed=#true fill=(token)"c.a" {
        anchor x=(px)0 y=(px)0 kind="corner" out-x=(px)10 out-y=(px)0 // start
        anchor x=(px)20 y=(px)0 kind="smooth" in-x=(px)10 in-y=(px)0
        anchor x=(px)0 y=(px)20
        anchor x=(px)10 y=(px)30
      }
      polygon id="poly" fill=(token)"c.b" {
        point x=(px)400 y=(px)0
        point x=(px)440 y=(px)0 // corner
        point x=(px)420 y=(px)30
      }
      image id="pic" asset="img.a" x=(px)500 y=(px)0 w=(px)100 h=(px)100
      // A hairline rule.
      line id="rule" x1=(px)0 y1=(pt)450 x2=(px)300 y2=(pt)450 stroke=(token)"c.b" // rule
      rect id="d1" x=(px)0 y=(px)300 w=(px)20 h=(px)20 // first
      rect id="d2" x=(px)50 y=(px)300 w=(px)20 h=(px)20
      rect id="d3" x=(px)200 y=(px)300 w=(px)20 h=(px)20
      frame id="chips" x=(px)0 y=(px)400 w=(px)300 h=(px)50 layout="column" {
        rect id="chip.a" w=(px)40 h=(px)20 fill=(token)"c.a"
      }

      // A pair.
      group id="pair" {
        rect id="g1" x=(px)600 y=(px)300 w=(px)10 h=(px)10 // g1
        rect id="g2" x=(px)620 y=(px)300 w=(px)10 h=(px)10
      }
    }
    // Page two.
    page id="pg.two" w=(px)800 h=(px)600 {
      rect id="two" x=(px)0 y=(px)0 w=(px)10 h=(px)10 // two
    }
  }
}
"##;

#[test]
fn rich_fixture_ops() {
    let cases = [
        exact(r#"{"op":"set_fill_rule","node":"p1","fill_rule":"evenodd"}"#),
        exact(
            r#"{"op":"set_points","node":"poly","points":[{"x":400,"y":0},{"x":450,"y":0},{"x":420,"y":30}]}"#,
        ),
        exact(
            r#"{"op":"set_points","node":"poly","points":[{"x":400,"y":0},{"x":440,"y":0},{"x":420,"y":30},{"x":400,"y":30}]}"#,
        ),
        exact(
            r#"{"op":"set_path_anchors","node":"p1","anchors":[{"x":0,"y":0,"kind":"corner","out_x":10,"out_y":0},{"x":20,"y":0,"kind":"smooth","in_x":10,"in_y":0},{"x":0,"y":20,"kind":"corner"}]}"#,
        ),
        exact(r#"{"op":"set_path_anchor_kind","node":"p1","anchor_index":0,"kind":"smooth"}"#),
        exact(r#"{"op":"move_path_anchor","node":"p1","anchor_index":1,"dx":5,"dy":5}"#),
        exact(
            r#"{"op":"move_path_handle","node":"p1","anchor_index":0,"handle":"out","dx":2,"dy":3}"#,
        ),
        exact(r#"{"op":"remove_path_anchor","node":"p1","anchor_index":3}"#),
        exact(r#"{"op":"insert_path_anchor","node":"p1","segment_index":0,"t":0.5}"#),
        exact(
            r#"{"op":"transform_path_anchors","node":"p1","transform":{"mode":"translate","dx":10,"dy":-4}}"#,
        ),
        exact(r#"{"op":"set_asset","node_id":"pic","asset_id":"img.b"}"#),
        exact(r#"{"op":"nudge_geometry","node":"pic","dx":-20,"dh":12}"#),
        exact(r#"{"op":"nudge_line_points","node":"rule","dx2":-40,"dy2":8}"#),
        exact(r#"{"op":"nudge_line_points","node":"rule","dx1":5,"dy1":5,"dx2":5,"dy2":5}"#),
        exact(
            r#"{"op":"transform_path_anchors","node":"p1","transform":{"mode":"scale","sx":2,"sy":1.5,"cx":0,"cy":0}}"#,
        ),
        exact(r#"{"op":"distribute_nodes","node_ids":["d1","d2","d3"],"axis":"horizontal"}"#),
        exact(r#"{"op":"align_to_edge","node":"d3","edge":"right","margin":24}"#),
        exact(r#"{"op":"set_layout","node":"chips","layout":"row","gap":8}"#),
        exact(r#"{"op":"set_geometry","node":"bar","h":6}"#),
        exact_on(
            r#"{"op":"set_style_property","style_id":"box","property":"fill","value":"c.b"}"#,
            &["box"],
        ),
        exact_on(
            r#"{"op":"set_page_master","page":"pg","master":"m.one"}"#,
            &["pg"],
        ),
        inserts(
            r#"{"op":"set_style_property","style_id":"box","property":"stroke","value":"c.b"}"#,
            "      stroke (token)\"c.b\"\n",
        ),
        inserts(
            r#"{"op":"set_default","kind":"shape","style":"box"}"#,
            "  defaults {\n    shape style=\"box\"\n  }\n",
        ),
        structural(
            r#"{"op":"add_node","parent":"pg","source":"rect id=\"new\" x=(px)1 y=(px)1 w=(px)2 h=(px)2"}"#,
            &["new"],
            &[],
        ),
        structural(
            r#"{"op":"add_path","parent":"pg","id":"p2","closed":true,"anchors":[{"x":0,"y":0,"kind":"corner"},{"x":9,"y":0,"kind":"corner"},{"x":9,"y":9,"kind":"corner"}]}"#,
            &["p2"],
            &[],
        ),
        structural(
            r#"{"op":"group","node_ids":["d1","d2"],"group_id":"grp"}"#,
            &["grp", "d1", "d2"],
            &[],
        ),
        structural(
            r#"{"op":"ungroup","group_id":"pair"}"#,
            &["pair", "g1", "g2"],
            &["// A pair."],
        ),
        structural(
            r#"{"op":"reparent","node":"d1","new_parent":"chips","position":{"at":"last"}}"#,
            &["d1", "chip.a"],
            &[],
        ),
        structural(
            r#"{"op":"reparent","node":"chip.a","new_parent":"pg","position":{"at":"last"}},{"op":"set_geometry","node":"chip.a","x":0,"y":500}"#,
            &["chips", "chip.a"],
            &[],
        ),
        structural(
            r#"{"op":"remove_node","node":"rule"}"#,
            &["rule"],
            &["// A hairline rule.", "// rule"],
        ),
        structural(
            r#"{"op":"remove_node","node":"p1"}"#,
            &["p1"],
            &["// A path with four anchors.", "// start"],
        ),
        structural(
            r#"{"op":"add_page","id":"pg3","w":"(px)800","h":"(px)600"}"#,
            &["pg3"],
            &[],
        ),
        structural(
            r#"{"op":"duplicate_page","page":"pg","new_id":"pg3","id_suffix":".v2"}"#,
            &["pg3"],
            &[],
        ),
        structural(
            r#"{"op":"delete_page","page":"pg.two"}"#,
            &["pg.two"],
            &["// Page two.", "// two"],
        ),
        structural(
            r#"{"op":"reorder_pages","order":["pg.two","pg"]}"#,
            &["pg.two"],
            &[],
        ),
        structural(
            r#"{"op":"create_style","id":"s2","properties":{"fill":"c.a"}}"#,
            &["s2"],
            &[],
        ),
        structural(
            r#"{"op":"delete_style","id":"box"}"#,
            &["box", "=styles"],
            &["// box fill"],
        ),
        structural(r#"{"op":"create_master","id":"m.two"}"#, &["m.two"], &[]),
        structural(
            r#"{"op":"delete_master","id":"m.one"}"#,
            &["m.one", "=masters"],
            &[],
        ),
        structural(
            r##"{"op":"create_token","id":"c.n","type":"color","value":"#000000"}"##,
            &["c.n"],
            &[],
        ),
        structural(
            r#"{"op":"add_asset","id":"img.c","kind":"image","src":"images/c.png"}"#,
            &["img.c"],
            &[],
        ),
        structural(r#"{"op":"move_backward","node":"d2"}"#, &["d1", "d2"], &[]),
        structural(
            r#"{"op":"duplicate_node","node":"pair","new_id":"pair.dup"}"#,
            &["pair.dup", "g1.dup", "g2.dup"],
            &[],
        ),
        structural(
            r#"{"op":"duplicate_node","node":"chips","new_id":"chips.dup"}"#,
            &["chips.dup", "chip.a.dup"],
            &[],
        ),
    ];
    for case in &cases {
        check("RICH", RICH, case);
    }
}

/// Frame, group, instance, and table with comments, anchors, a connector, and
/// a page port. Duplicating each patches in place and keeps every comment.
const BOXES: &str = r##"zenith version=1 {
  project id="proj.boxes" name="Boxes"
  tokens format="zenith-token-v1" {
    token id="c.a" type="color" value="#112233"
  }
  styles {}
  components {
    component id="badge" {
      rect id="face" x=(px)0 y=(px)0 w=(px)20 h=(px)20 fill=(token)"c.a"
    }
  }
  document id="doc.boxes" title="Boxes" {
    page id="pg" w=(px)800 h=(px)600 {
      rect id="outside" x=(px)0 y=(px)0 w=(px)40 h=(px)40 // anchor target
      // A frame with anchors and connectors.
      frame id="box" anchor-sibling="outside" anchor-edge="below" anchor-gap=(px)8 w=(px)200 h=(px)200 {
        rect id="a" x=(px)0 y=(px)0 w=(px)40 h=(px)40 // inside
        rect id="b" anchor-sibling="a" anchor-edge="below" anchor-gap=(px)8 w=(px)40 h=(px)40
        connector id="k.in" from="a" to="b"
        connector id="k.mixed" from="a" to="outside"
      }
      // A badge instance.
      instance id="inst" component="badge" x=(px)300 y=(px)100
      // A table.
      table id="tbl" x=(px)300 y=(px)200 w=(px)200 h=(px)100 {
        column width=(px)100
        row {
          cell {
            rect id="t1" x=(px)0 y=(px)0 w=(px)10 h=(px)10 // cell rect
            rect id="t2" anchor-sibling="t1" anchor-edge="after" w=(px)10 h=(px)10
          }
        }
      }
    }
  }
}
"##;

#[test]
fn container_duplicate_ops() {
    let cases = [
        structural(
            r#"{"op":"duplicate_node","node":"box","new_id":"box.dup"}"#,
            &["box.dup", "a.dup", "b.dup", "k.in.dup", "k.mixed.dup"],
            &[],
        ),
        structural(
            r#"{"op":"duplicate_node","node":"inst","new_id":"inst.dup"}"#,
            &["inst.dup"],
            &[],
        ),
        structural(
            r#"{"op":"duplicate_node","node":"tbl","new_id":"tbl.dup"}"#,
            &["tbl.dup", "t1.dup", "t2.dup"],
            &[],
        ),
    ];
    for case in &cases {
        check("BOXES", BOXES, case);
    }
}

/// A page port on a copied node gets a copy, patched in place with comments kept.
#[test]
fn container_duplicate_copies_page_ports_in_place() {
    let src = BOXES.replace(
        "      rect id=\"outside\"",
        "      ports {\n        port node=\"a\" id=\"p\" anchor=\"top-right\"\n      }\n      rect id=\"outside\"",
    );
    let before = parse(&src);
    let tx = Transaction::from_json(
        r#"{"ops":[{"op":"duplicate_node","node":"box","new_id":"box.dup"}]}"#,
    )
    .expect("tx");
    let result = run_transaction(&before, &tx).expect("run");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    let after = &result.document_after;
    if let Err(e) = try_patch_source(&src, &before, after) {
        panic!("must patch in place: {e}");
    }
    let patched = patch_source(&src, &before, after).expect("patch_source");
    assert!(!patched.reformatted);
    for comment in comments(&src) {
        assert!(patched.text.contains(comment), "lost {comment:?}");
    }
    assert!(
        patched
            .text
            .contains("port node=\"a.dup\" id=\"p\" anchor=\"top-right\"")
    );
    assert_eq!(
        strip_spans(parse(&patched.text)),
        strip_spans(after.clone())
    );
}
