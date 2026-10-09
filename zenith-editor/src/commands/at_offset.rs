//! `select.at_offset {offset, select?}`: the node under a code-pane cursor.

use serde::Deserialize;
use serde_json::{Value, json};
use zenith_core::{Document, Node, Span};

use super::common::params;
use crate::ctx::Ctx;
use crate::doc::tree::child_lists;
use crate::error::EditorError;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AtParams {
    offset: usize,
    #[serde(default = "yes")]
    select: bool,
}

fn yes() -> bool {
    true
}

/// Where an offset falls: the innermost node with an id whose span holds
/// it, and the page or master that holds the offset.
#[derive(Debug, Default, PartialEq)]
struct Found<'d> {
    node: Option<&'d Node>,
    page: Option<usize>,
    master: Option<&'d str>,
}

/// The innermost node with an id whose source span holds byte `offset` of
/// the current text (`start <= offset <= end`, so a cursor right after a
/// node still names it).
///
/// With `select` (the default), a hit becomes the selection and a miss
/// clears it, and a page that holds the offset becomes the session page.
/// While the current text does not parse, the reply has `parsed: false`
/// and the session is unchanged: the cursor moves while the user types.
pub(crate) fn run(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: AtParams = params(ctx, raw)?;
    let text = ctx.session.text.clone();
    if p.offset > text.len() {
        return Err(EditorError::new(
            "editor.invalid_params",
            format!(
                "offset {} is past the end of the text ({} bytes); send a byte offset from 0 to {}",
                p.offset,
                text.len(),
                text.len()
            ),
        ));
    }
    let Ok(doc) = ctx.parse(&text) else {
        return Ok(json!({
            "parsed": false,
            "id": Value::Null,
            "page": Value::Null,
            "selection": ctx.session.selection,
        }));
    };
    let found = find(&doc, p.offset);
    let id = found.node.and_then(Node::id).map(str::to_owned);
    if p.select {
        ctx.session.selection = id.iter().cloned().collect();
        if let Some(page) = found.page {
            ctx.session.page = page + 1;
        }
    }
    let mut out = json!({
        "parsed": true,
        "id": id,
        "page": found.page.map(|i| i + 1),
        "selection": ctx.session.selection,
    });
    if let Some(obj) = out.as_object_mut() {
        if let Some(node) = found.node {
            obj.insert("kind".into(), Value::from(node.kind_str()));
        }
        if let Some(master) = found.master {
            obj.insert("master".into(), Value::from(master));
        }
    }
    Ok(out)
}

/// Locate byte `offset` in `doc`: pages first, then masters.
fn find(doc: &Document, offset: usize) -> Found<'_> {
    for (i, page) in doc.body.pages.iter().enumerate() {
        let node = innermost(&page.children, offset);
        if node.is_some() || holds(page.source_span, offset) {
            return Found {
                node,
                page: Some(i),
                master: None,
            };
        }
    }
    for master in &doc.masters {
        let node = innermost(&master.children, offset);
        if node.is_some() || holds(master.source_span, offset) {
            return Found {
                node,
                page: None,
                master: Some(master.id.as_str()),
            };
        }
    }
    Found::default()
}

/// The deepest node with an id under `nodes` whose span holds `offset`.
fn innermost(nodes: &[Node], offset: usize) -> Option<&Node> {
    for node in nodes {
        if !holds(node.source_span(), offset) {
            continue;
        }
        let inner = child_lists(node)
            .into_iter()
            .find_map(|list| innermost(list, offset));
        if inner.is_some() {
            return inner;
        }
        if node.id().is_some() {
            return Some(node);
        }
    }
    None
}

/// `true` when `span` holds `offset`, end included.
fn holds(span: Option<Span>, offset: usize) -> bool {
    span.is_some_and(|s| s.start <= offset && offset <= s.end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource};

    const SRC: &str = r#"zenith version=1 {
  document id="d" {
    page id="one" w=(px)100 h=(px)100 {
      group id="g" {
        rect id="a" x=(px)0 y=(px)0 w=(px)5 h=(px)5
      }
    }
    page id="two" w=(px)100 h=(px)100 {
      rect id="b" x=(px)0 y=(px)0 w=(px)5 h=(px)5
    }
  }
  masters {
    master id="m" {
      rect id="chrome" x=(px)0 y=(px)0 w=(px)5 h=(px)5
    }
  }
}
"#;

    fn at(needle: &str) -> usize {
        SRC.find(needle).expect("needle")
    }

    fn id_at(doc: &Document, offset: usize) -> Option<&str> {
        find(doc, offset).node.and_then(Node::id)
    }

    #[test]
    fn innermost_node_wins_and_end_is_inclusive() {
        let doc = KdlAdapter.parse(SRC.as_bytes()).expect("parse");
        assert_eq!(id_at(&doc, at("rect id=\"a\"") + 3), Some("a"));
        assert_eq!(id_at(&doc, at("group id")), Some("g"));
        let end_a = at("h=(px)5\n      }") + "h=(px)5".len();
        assert_eq!(id_at(&doc, end_a), Some("a"));
        assert_eq!(find(&doc, at("rect id=\"a\"")).page, Some(0));
    }

    #[test]
    fn page_without_node_and_master_content() {
        let doc = KdlAdapter.parse(SRC.as_bytes()).expect("parse");
        let page_two = find(&doc, at("page id=\"two\""));
        assert_eq!((page_two.node, page_two.page), (None, Some(1)));
        assert_eq!(id_at(&doc, at("rect id=\"b\"")), Some("b"));
        let chrome = find(&doc, at("rect id=\"chrome\""));
        assert_eq!(chrome.node.and_then(Node::id), Some("chrome"));
        assert_eq!((chrome.page, chrome.master), (None, Some("m")));
        assert_eq!(find(&doc, 0), Found::default());
    }
}
