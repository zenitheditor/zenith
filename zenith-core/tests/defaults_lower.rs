//! Integration tests for `defaults::lower`: the per-property cascade, label
//! text-style cascade, table header styles, content pairing, and the
//! no-defaults path.

use std::collections::BTreeMap;

use zenith_core::{
    Document, KdlAdapter, KdlSource, Node, PropertyValue, ResolvedToken, Style, defaults,
    resolve_tokens,
};

const TOKENS: &str = r##"
    token id="c.attr" type="color" value="#010101"
    token id="c.node" type="color" value="#020202"
    token id="c.page" type="color" value="#030303"
    token id="c.doc" type="color" value="#040404"
    token id="r.page" type="dimension" value=(px)6
    token id="r.doc" type="dimension" value=(px)9
    token id="w.doc" type="dimension" value=(px)3
    token id="fs.node" type="dimension" value=(px)11
    token id="fs.page" type="dimension" value=(px)12
    token id="fs.doc" type="dimension" value=(px)13
    token id="fw.page" type="fontWeight" value=600
    token id="fw.doc" type="fontWeight" value=700
    token id="ls.doc" type="dimension" value=(px)1
"##;

/// A document with `tokens` (plus [`TOKENS`]), `styles`, document `top`
/// (a `defaults` block), page attributes `page_attrs`, and `page_body`.
fn src(tokens: &str, styles: &str, top: &str, page_attrs: &str, page_body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="p" name="P"
  tokens format="zenith-token-v1" {{
{TOKENS}
{tokens}
  }}
  styles {{
{styles}
  }}
  {top}
  document id="d" title="D" {{
    page id="pg" w=(px)400 h=(px)400 {page_attrs} {{
      {page_body}
    }}
  }}
}}
"##
    )
}

fn parse(source: &str) -> Document {
    KdlAdapter
        .parse(source.as_bytes())
        .expect("test document must parse")
}

fn resolved(doc: &Document) -> BTreeMap<String, ResolvedToken> {
    resolve_tokens(&doc.tokens).resolved
}

fn lowered(source: &str) -> Document {
    lowered_with_aliases(source).document
}

fn lowered_with_aliases(source: &str) -> defaults::Lowered {
    let doc = parse(source);
    defaults::lower(&doc, &resolved(&doc)).expect("a defaults block lowers")
}

fn tok(id: &str) -> Option<PropertyValue> {
    Some(PropertyValue::TokenRef(id.to_owned()))
}

fn page_node<'a>(doc: &'a Document, id: &str) -> &'a Node {
    find(&doc.body.pages[0].children, id).expect("node exists")
}

fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Node> {
    for node in nodes {
        if node.id() == Some(id) {
            return Some(node);
        }
        if let Some(found) = node.children().and_then(|c| find(c, id)) {
            return Some(found);
        }
        if let Node::Table(t) = node {
            for row in &t.rows {
                for cell in &row.cells {
                    if let Some(found) = find(&cell.children, id) {
                        return Some(found);
                    }
                }
            }
        }
    }
    None
}

fn style<'a>(doc: &'a Document, id: &str) -> &'a Style {
    doc.styles
        .styles
        .iter()
        .find(|s| s.id == id)
        .expect("style exists")
}

const CASCADE_STYLES: &str = r#"
    style id="n" { fill (token)"c.node" }
    style id="p" { fill (token)"c.page"; radius (token)"r.page" }
    style id="d" { fill (token)"c.doc"; radius (token)"r.doc"; stroke (token)"c.doc"; stroke-width (token)"w.doc" }
"#;

#[test]
fn no_defaults_block_returns_none() {
    let doc = parse(&src(
        "",
        CASCADE_STYLES,
        "",
        "",
        r#"rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 style="n""#,
    ));
    assert!(defaults::lower(&doc, &resolved(&doc)).is_none());
}

#[test]
fn per_property_cascade_attr_style_page_doc() {
    let doc = lowered(&src(
        "",
        CASCADE_STYLES,
        r#"defaults { rect style="d" }"#,
        "",
        r#"defaults { rect style="p" }
      rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 style="n" stroke=(token)"c.attr""#,
    ));
    let Node::Rect(r) = page_node(&doc, "r") else {
        panic!("rect");
    };
    assert_eq!(r.fill, None, "the node style fill wins, so no attribute");
    assert_eq!(r.stroke, tok("c.attr"), "the node attribute wins");
    assert_eq!(r.radius, tok("r.page"), "page default beats doc default");
    assert_eq!(r.stroke_width, tok("w.doc"), "doc default fills the rest");
    assert_eq!(r.style.as_deref(), Some("n"), "the node style id is kept");
}

#[test]
fn node_without_style_takes_page_then_doc_defaults() {
    let doc = lowered(&src(
        "",
        CASCADE_STYLES,
        r#"defaults { rect style="d" }"#,
        "",
        r#"defaults { rect style="p" }
      rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10"#,
    ));
    let Node::Rect(r) = page_node(&doc, "r") else {
        panic!("rect");
    };
    assert_eq!(r.fill, tok("c.page"));
    assert_eq!(r.radius, tok("r.page"));
    assert_eq!(r.stroke, tok("c.doc"));
    assert_eq!(r.style, None);
}

#[test]
fn shape_label_text_style_cascades_per_property() {
    let styles = r#"
    style id="nts" { font-size (token)"fs.node" }
    style id="pts" { font-size (token)"fs.page"; font-weight (token)"fw.page" }
    style id="dts" { font-weight (token)"fw.doc"; letter-spacing (token)"ls.doc" }
    style id="box" { radius (token)"r.doc" }
"#;
    let doc = lowered(&src(
        "",
        styles,
        r#"defaults { shape style="box" text-style="dts" }"#,
        "",
        r#"defaults { shape style="box" text-style="pts" }
      shape id="s" x=(px)0 y=(px)0 w=(px)100 h=(px)40 text-style="nts" { span "Go" }"#,
    ));
    let Node::Shape(s) = page_node(&doc, "s") else {
        panic!("shape");
    };
    assert_eq!(s.radius, tok("r.doc"));
    let id = s.text_style.as_deref().expect("label style");
    assert!(
        id.starts_with("defaults:label:"),
        "merged label style: {id}"
    );
    let merged = style(&doc, id);
    assert_eq!(merged.properties.get("font-size"), tok("fs.node").as_ref());
    assert_eq!(
        merged.properties.get("font-weight"),
        tok("fw.page").as_ref()
    );
    assert_eq!(
        merged.properties.get("letter-spacing"),
        tok("ls.doc").as_ref()
    );
}

#[test]
fn shape_label_without_own_style_takes_the_single_default_id() {
    let doc = lowered(&src(
        "",
        r#"style id="lbl" { font-size (token)"fs.doc" }"#,
        r#"defaults { shape style="lbl" text-style="lbl" }"#,
        "",
        r#"shape id="s" x=(px)0 y=(px)0 w=(px)100 h=(px)40 { span "Go" }"#,
    ));
    let Node::Shape(s) = page_node(&doc, "s") else {
        panic!("shape");
    };
    assert_eq!(s.text_style.as_deref(), Some("lbl"));
}

#[test]
fn table_header_style_stays_the_header_text_style() {
    let styles = r#"
    style id="hs" { fill (token)"c.node" }
    style id="body" { fill (token)"c.doc"; font-size (token)"fs.doc" }
"#;
    let doc = lowered(&src(
        "",
        styles,
        r#"defaults { text style="body" }"#,
        "",
        r#"table id="t" x=(px)0 y=(px)0 w=(px)200 h=(px)80 header-rows=1 header-style="hs" {
        column width=(px)200
        row { cell { text id="h" { span "Head" } } }
        row { cell { text id="b" { span "Body" } } }
      }"#,
    ));
    let Node::Text(h) = page_node(&doc, "h") else {
        panic!("text");
    };
    assert_eq!(h.fill, None, "the header style fill wins");
    assert_eq!(h.font_size, tok("fs.doc"));
    assert_eq!(h.style, None, "the scene still injects header_style");
    let Node::Text(b) = page_node(&doc, "b") else {
        panic!("text");
    };
    assert_eq!(b.fill, tok("c.doc"));
}

const PAIR_TOKENS: &str = r##"
    token id="color.primary" type="color" value="#605dff"
    token id="color.primary.content" type="color" value="#ffffff"
    token id="color.base.200" type="color" value="#e8e8e8"
    token id="color.base.content" type="color" value="#1a1a1a"
    token id="color.dark" type="color" value="#101010"
    token id="grad.a" type="gradient" angle=(deg)90 {
      stop offset=0.0 color=(token)"color.dark"
      stop offset=1.0 color=(token)"color.base.200"
    }
"##;

fn text_fill(doc: &Document, id: &str) -> Option<PropertyValue> {
    match page_node(doc, id) {
        Node::Text(t) => t.fill.clone(),
        _ => panic!("text"),
    }
}

#[test]
fn pairing_rule_one_page_background_content_token() {
    let doc = lowered(&src(
        PAIR_TOKENS,
        "",
        "defaults {}",
        r#"background=(token)"color.primary""#,
        r#"text id="t" x=(px)0 y=(px)0 w=(px)100 h=(px)20 { span "Hi" }"#,
    ));
    assert_eq!(text_fill(&doc, "t"), tok("color.primary.content"));
}

#[test]
fn pairing_rule_two_scale_step_and_frame_scope() {
    let doc = lowered(&src(
        PAIR_TOKENS,
        "",
        "defaults {}",
        r#"background=(token)"color.primary""#,
        r#"frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)100 fill=(token)"color.base.200" {
        text id="in" x=(px)0 y=(px)0 w=(px)100 h=(px)20 { span "In" }
      }
      text id="out" x=(px)0 y=(px)200 w=(px)100 h=(px)20 { span "Out" }"#,
    ));
    assert_eq!(text_fill(&doc, "in"), tok("color.base.content"));
    assert_eq!(text_fill(&doc, "out"), tok("color.primary.content"));
}

#[test]
fn pairing_rule_three_max_lc_with_smallest_id_tie_break() {
    let tokens = r##"
    token id="color.dark" type="color" value="#101010"
    token id="b.content" type="color" value="#ffffff"
    token id="a.content" type="color" value="#ffffff"
    token id="c.content" type="color" value="#202020"
"##;
    let doc = lowered(&src(
        tokens,
        "",
        "defaults {}",
        r#"background=(token)"color.dark""#,
        r#"text id="t" x=(px)0 y=(px)0 w=(px)100 h=(px)20 { span "Hi" }"#,
    ));
    assert_eq!(text_fill(&doc, "t"), tok("a.content"));
}

#[test]
fn pairing_inherits_through_a_gradient_frame() {
    let doc = lowered(&src(
        PAIR_TOKENS,
        "",
        "defaults {}",
        r#"background=(token)"color.primary""#,
        r#"frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)100 fill=(token)"grad.a" {
        text id="in" x=(px)0 y=(px)0 w=(px)100 h=(px)20 { span "In" }
      }"#,
    ));
    assert_eq!(text_fill(&doc, "in"), tok("color.primary.content"));
}

#[test]
fn pairing_sits_between_node_style_fill_and_default_fill() {
    let styles = r#"
    style id="own" { fill (token)"c.node" }
    style id="body" { fill (token)"c.doc" }
"#;
    let doc = lowered(&src(
        PAIR_TOKENS,
        styles,
        r#"defaults { text style="body" }"#,
        r#"background=(token)"color.primary""#,
        r#"text id="styled" x=(px)0 y=(px)0 w=(px)100 h=(px)20 style="own" { span "A" }
      text id="plain" x=(px)0 y=(px)30 w=(px)100 h=(px)20 { span "B" }
      text id="attr" x=(px)0 y=(px)60 w=(px)100 h=(px)20 fill=(token)"c.attr" { span "C" fill=(token)"c.page" }"#,
    ));
    assert_eq!(text_fill(&doc, "styled"), None, "node style fill wins");
    assert_eq!(text_fill(&doc, "plain"), tok("color.primary.content"));
    assert_eq!(text_fill(&doc, "attr"), tok("c.attr"));
    let Node::Text(t) = page_node(&doc, "attr") else {
        panic!("text");
    };
    assert_eq!(t.spans[0].fill, tok("c.page"), "span fill is untouched");
}

#[test]
fn page_block_alone_enables_pairing() {
    let doc = lowered(&src(
        PAIR_TOKENS,
        "",
        "",
        r#"background=(token)"color.primary""#,
        r#"defaults {}
      text id="t" x=(px)0 y=(px)0 w=(px)100 h=(px)20 { span "Hi" }"#,
    ));
    assert_eq!(text_fill(&doc, "t"), tok("color.primary.content"));
}

#[test]
fn shape_label_pairs_with_the_shape_fill() {
    let styles = r#"
    style id="control" { fill (token)"color.primary"; radius (token)"r.doc" }
    style id="label" { font-size (token)"fs.doc" }
"#;
    let doc = lowered(&src(
        PAIR_TOKENS,
        styles,
        r#"defaults { shape style="control" text-style="label" }"#,
        "",
        r#"shape id="s" x=(px)0 y=(px)0 w=(px)100 h=(px)40 { span "Go" }"#,
    ));
    let Node::Shape(s) = page_node(&doc, "s") else {
        panic!("shape");
    };
    assert_eq!(s.fill, tok("color.primary"));
    assert_eq!(s.radius, tok("r.doc"));
    let merged = style(&doc, s.text_style.as_deref().expect("label style"));
    assert_eq!(
        merged.properties.get("fill"),
        tok("color.primary.content").as_ref()
    );
    assert_eq!(merged.properties.get("font-size"), tok("fs.doc").as_ref());
}

#[test]
fn explicit_attributes_written_by_overrides_win() {
    // A variant or instance override writes an explicit attribute; lowering
    // never replaces one.
    let doc = lowered(&src(
        "",
        CASCADE_STYLES,
        r#"defaults { rect style="d" }"#,
        "",
        r#"rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"c.attr""#,
    ));
    let Node::Rect(r) = page_node(&doc, "r") else {
        panic!("rect");
    };
    assert_eq!(r.fill, tok("c.attr"));
}

const CARD: &str = r#"components {
    component id="card" {
      rect id="bg" x=(px)0 y=(px)0 w=(px)10 h=(px)10
      text id="label" x=(px)0 y=(px)0 w=(px)10 h=(px)10 { span "Hi" }
    }
  }
  masters {
    master id="m" {
      rect id="mbg" x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }
  }"#;

fn component<'a>(doc: &'a Document, id: &str) -> &'a zenith_core::ComponentDef {
    doc.components
        .iter()
        .find(|c| c.id == id)
        .expect("component exists")
}

fn first_rect_fill(nodes: &[Node]) -> Option<PropertyValue> {
    nodes.iter().find_map(|n| match n {
        Node::Rect(r) => Some(r.fill.clone()),
        _ => None,
    })?
}

fn first_text_fill(nodes: &[Node]) -> Option<PropertyValue> {
    nodes.iter().find_map(|n| match n {
        Node::Text(t) => Some(t.fill.clone()),
        _ => None,
    })?
}

fn instance_component(doc: &Document, id: &str) -> String {
    match page_node(doc, id) {
        Node::Instance(i) => i.component.clone().expect("component"),
        _ => panic!("instance"),
    }
}

#[test]
fn authored_components_keep_document_defaults() {
    let doc = lowered(&src(
        "",
        CASCADE_STYLES,
        &format!("defaults {{ rect style=\"d\" }}\n  {CARD}"),
        "",
        r#"defaults { rect style="p" }"#,
    ));
    assert_eq!(
        first_rect_fill(&component(&doc, "card").children),
        tok("c.doc")
    );
}

#[test]
fn instances_lower_with_the_host_page_defaults_and_ambient() {
    let lowered = lowered_with_aliases(&src(
        PAIR_TOKENS,
        CASCADE_STYLES,
        &format!("defaults {{ rect style=\"d\" }}\n  {CARD}"),
        r#"background=(token)"color.primary""#,
        r#"defaults { rect style="p" }
      instance id="a" component="card" x=(px)0 y=(px)0
      frame id="f" x=(px)0 y=(px)100 w=(px)200 h=(px)100 fill=(token)"color.base.200" {
        instance id="b" component="card" x=(px)0 y=(px)0
      }"#,
    ));
    let doc = &lowered.document;
    let a = instance_component(doc, "a");
    assert_ne!(a, "card", "the instance points at a page copy");
    assert_eq!(lowered.aliases.authored_id(&a), "card");
    let copy = component(doc, &a);
    assert_eq!(
        first_rect_fill(&copy.children),
        tok("c.page"),
        "page beats doc"
    );
    assert_eq!(
        first_text_fill(&copy.children),
        tok("color.primary.content")
    );
    let b = instance_component(doc, "b");
    assert_ne!(a, b, "a different ambient pair is a different copy");
    assert_eq!(lowered.aliases.authored_id(&b), "card");
    assert_eq!(
        first_text_fill(&component(doc, &b).children),
        tok("color.base.content"),
        "an instance in a frame starts from the frame's pair"
    );
}

#[test]
fn masters_lower_with_the_page_defaults() {
    let source = src(
        "",
        CASCADE_STYLES,
        &format!("defaults {{ rect style=\"d\" }}\n  {CARD}"),
        r#"master="m""#,
        r#"defaults { rect style="p" }"#,
    );
    let lowered = lowered_with_aliases(&source);
    let doc = &lowered.document;
    let copy_id = doc.body.pages[0].master.clone().expect("master");
    assert_ne!(copy_id, "m");
    assert_eq!(lowered.aliases.authored_id(&copy_id), "m");
    let copy = doc
        .masters
        .iter()
        .find(|m| m.id == copy_id)
        .expect("master copy");
    assert_eq!(first_rect_fill(&copy.children), tok("c.page"));
    let authored = doc.masters.iter().find(|m| m.id == "m").expect("master");
    assert_eq!(first_rect_fill(&authored.children), tok("c.doc"));
}

#[test]
fn copy_ids_never_collide_with_declared_ids() {
    let lowered = lowered_with_aliases(&src(
        "",
        CASCADE_STYLES,
        r#"defaults { rect style="d" }
  components {
    component id="card" {
      rect id="bg" x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }
    component id="card@defaults:pg:-" {
      rect id="bg" x=(px)0 y=(px)0 w=(px)20 h=(px)20
    }
  }"#,
        "",
        r#"instance id="i" component="card" x=(px)0 y=(px)0"#,
    ));
    let copy = instance_component(&lowered.document, "i");
    assert_ne!(copy, "card@defaults:pg:-", "a declared id is never reused");
    assert_eq!(lowered.aliases.authored_id(&copy), "card");
    let width = |id: &str| match &component(&lowered.document, id).children[0] {
        Node::Rect(r) => r.w.clone(),
        _ => panic!("rect"),
    };
    assert_eq!(width(&copy), width("card"), "the copy is of `card`");
}

#[test]
fn recursive_component_lowering_terminates() {
    let doc = lowered(&src(
        "",
        CASCADE_STYLES,
        r#"defaults { rect style="d" }
  components {
    component id="loop" {
      instance id="again" component="loop" x=(px)0 y=(px)0
    }
  }"#,
        "",
        r#"instance id="i" component="loop" x=(px)0 y=(px)0"#,
    ));
    let copy_id = instance_component(&doc, "i");
    let copy = component(&doc, &copy_id);
    let Node::Instance(inner) = &copy.children[0] else {
        panic!("instance");
    };
    assert_eq!(inner.component.as_deref(), Some(copy_id.as_str()));
}

/// A chart takes the ambient content pair as its text `fill`, the default
/// style's `stroke` / `stroke-width` as attributes, and the default style
/// itself for the style-only `font-family` / `font-size`.
#[test]
fn chart_takes_pair_fill_stroke_and_style_only_font_keys() {
    let doc = lowered(&src(
        PAIR_TOKENS,
        r#"style id="ui.chart" { fill (token)"c.doc"; font-size (token)"fs.doc"; stroke (token)"c.page"; stroke-width (token)"w.doc" }"#,
        r#"defaults {
    chart style="ui.chart"
  }"#,
        r#"background=(token)"color.primary""#,
        r#"chart id="c" kind="bar" x=(px)0 y=(px)0 w=(px)200 h=(px)100 {
        series 1.0 2.0
      }
      chart id="own" kind="bar" x=(px)0 y=(px)0 w=(px)200 h=(px)100 fill=(token)"c.attr" {
        series 1.0 2.0
      }"#,
    ));
    let Node::Chart(c) = page_node(&doc, "c") else {
        panic!("chart");
    };
    assert_eq!(c.fill, tok("color.primary.content"));
    assert_eq!(c.stroke, tok("c.page"));
    assert_eq!(c.stroke_width, tok("w.doc"));
    assert_eq!(c.style.as_deref(), Some("ui.chart"));
    let Node::Chart(own) = page_node(&doc, "own") else {
        panic!("chart");
    };
    assert_eq!(own.fill, tok("c.attr"));
}
