//! The text slots of a document: every `text` node id with its current text.
//! `zenith new --starter` prints them so an agent fills the whole layout with
//! one batched `set_span_text` transaction, with no re-read of the file.

use crate::commands::inspect::{NodeEntry, PageEntry, clip_text};

/// One editable text node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// The node id.
    pub id: String,
    /// The node's text on one line, cut at 48 characters.
    pub text: String,
    /// How many spans the node has. A node with more than one span is edited
    /// with `replace_text`, not `set_span_text`.
    pub spans: usize,
}

fn collect(nodes: &[NodeEntry], out: &mut Vec<Slot>) {
    for node in nodes {
        if let Some(text) = &node.text {
            out.push(Slot {
                id: node.id.clone(),
                text: clip_text(text),
                spans: node.spans.unwrap_or(1),
            });
        }
        collect(&node.children, out);
    }
}

/// Every `text` node, in document order, across all pages of the inspect tree.
pub fn text_slots(pages: &[PageEntry]) -> Vec<Slot> {
    let mut out = Vec::new();
    for page in pages {
        collect(&page.children, &mut out);
    }
    out
}

/// The command that fills the single-span slots: one `set_span_text` per slot
/// with its real id, and `<new text>` where the text goes.
fn fill_command(slots: &[Slot], path: &str, limit: usize) -> String {
    let ops: Vec<serde_json::Value> = slots
        .iter()
        .take(limit)
        .filter(|slot| slot.spans <= 1)
        .map(|slot| {
            serde_json::json!({
                "op": "set_span_text",
                "node": slot.id,
                "span": 0,
                "text": "<new text>",
            })
        })
        .collect();
    format!(
        "zenith tx {path} --apply --ops '{}'",
        serde_json::Value::Array(ops)
    )
}

/// The slot listing `zenith new --starter` prints: one line per slot, at most
/// `limit` lines, and one ready command that edits every single-span slot.
pub fn slot_listing(slots: &[Slot], path: &str, limit: usize) -> String {
    let mut out = format!(
        "text slots (node id: current text). Fill them in one call, replacing each <new text> and dropping slots you keep. Slots marked [N spans] use replace_text:\n  {}\n",
        fill_command(slots, path, limit)
    );
    for slot in slots.iter().take(limit) {
        let extra = if slot.spans > 1 {
            format!(" [{} spans]", slot.spans)
        } else {
            String::new()
        };
        out.push_str(&format!("  {}: \"{}\"{extra}\n", slot.id, slot.text));
    }
    if slots.len() > limit {
        out.push_str(&format!(
            "  (+{} more: run `zenith inspect {path}`)\n",
            slots.len() - limit
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::inspect::build_doc_tree;
    use zenith_core::{KdlAdapter, KdlSource as _, resolve_tokens};

    fn pages(src: &str) -> Vec<PageEntry> {
        let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
        build_doc_tree(&doc.body.pages, &resolve_tokens(&doc.tokens).resolved)
    }

    const DOC: &str = r##"zenith version=1 {
  project id="p" name="P"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="d" title="T" {
    page id="pg" w=(px)400 h=(px)200 {
      frame id="f" x=(px)0 y=(px)0 w=(px)400 h=(px)200 layout="column" {
        text id="a" w="fill" h="hug" { span "First" }
        text id="b" w="fill" h="hug" {
          span "Second"
          span " more"
        }
      }
    }
  }
}
"##;

    #[test]
    fn slots_follow_document_order() {
        let slots = text_slots(&pages(DOC));
        let ids: Vec<&str> = slots.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);
        assert_eq!(slots[1].spans, 2);
        assert_eq!(slots[1].text, "Second more");
    }

    #[test]
    fn long_text_is_cut_and_the_listing_is_capped() {
        let long = "x".repeat(80);
        let src = DOC.replace("First", &long);
        let slots = text_slots(&pages(&src));
        assert!(slots[0].text.ends_with("...") && slots[0].text.chars().count() == 51);
        let listing = slot_listing(&slots, "t.zen", 1);
        assert!(
            listing.contains("a: \"") && !listing.contains("b: \""),
            "{listing}"
        );
        assert!(
            listing.contains("(+1 more: run `zenith inspect t.zen`)"),
            "{listing}"
        );
        let all = slot_listing(&slots, "t.zen", 5);
        assert!(all.contains("\"node\":\"a\""), "{all}");
        assert!(
            !all.contains("\"node\":\"b\""),
            "multi-span slots stay out of the command: {all}"
        );
        assert!(
            listing.contains("[2 spans]") || slot_listing(&slots, "t.zen", 5).contains("[2 spans]")
        );
    }
}
