//! Span stripping: clear every source span so two parses compare by content.
//!
//! A `Document` parsed from two different texts carries different byte spans
//! even when the content is equal. [`strip_spans`] clears every
//! `source_span` in the tree. Equality on the stripped trees is content
//! equality. The source patcher and the format round-trip tests use it.

use super::defaults::DefaultsBlock;
use super::document::{Document, Page, PortDef};
use super::node::Node;

/// Return `doc` with every source span cleared.
pub fn strip_spans(mut doc: Document) -> Document {
    doc.assets.source_span = None;
    for decl in &mut doc.assets.assets {
        decl.source_span = None;
    }
    for token in &mut doc.tokens.tokens {
        token.source_span = None;
    }
    doc.styles.source_span = None;
    for style in &mut doc.styles.styles {
        style.source_span = None;
    }
    strip_defaults(&mut doc.defaults);
    for comp in &mut doc.components {
        comp.source_span = None;
        strip_ports(&mut comp.ports);
        for node in &mut comp.children {
            strip_node_spans(node);
        }
    }
    for master in &mut doc.masters {
        master.source_span = None;
        for node in &mut master.children {
            strip_node_spans(node);
        }
    }
    for library in &mut doc.libraries {
        library.source_span = None;
    }
    for import in &mut doc.imports {
        import.source_span = None;
        for token_map in &mut import.token_maps {
            token_map.source_span = None;
        }
    }
    for action in &mut doc.actions {
        action.source_span = None;
    }
    for section in &mut doc.sections {
        section.source_span = None;
    }
    for prov in &mut doc.provenance {
        prov.source_span = None;
    }
    for variant in &mut doc.variants {
        variant.source_span = None;
        for ov in &mut variant.overrides {
            ov.source_span = None;
        }
    }
    for recipe in &mut doc.recipes {
        recipe.source_span = None;
        for param in &mut recipe.params {
            param.source_span = None;
        }
    }
    for entry in &mut doc.diagnostic_policy.entries {
        entry.source_span = None;
    }
    doc.brand_contract.source_span = None;
    for child in &mut doc.unsupported_children {
        child.source_span = None;
    }
    for page in &mut doc.body.pages {
        strip_page(page);
    }
    doc
}

/// Clear the source span of `node` and of every node and record below it.
pub fn strip_node_spans(node: &mut Node) {
    match node {
        Node::Rect(r) => r.source_span = None,
        Node::Ellipse(e) => e.source_span = None,
        Node::Line(l) => l.source_span = None,
        Node::Text(t) => t.source_span = None,
        Node::Code(c) => c.source_span = None,
        Node::Frame(f) => {
            f.source_span = None;
            for child in &mut f.children {
                strip_node_spans(child);
            }
        }
        Node::Group(g) => {
            g.source_span = None;
            for region in &mut g.protected_regions {
                region.source_span = None;
            }
            for child in &mut g.children {
                strip_node_spans(child);
            }
        }
        Node::Image(i) => i.source_span = None,
        Node::Polygon(p) => p.source_span = None,
        Node::Polyline(p) => p.source_span = None,
        Node::Path(p) => p.source_span = None,
        Node::Instance(i) => {
            i.source_span = None;
            for ov in &mut i.overrides {
                ov.source_span = None;
            }
        }
        Node::Field(f) => f.source_span = None,
        Node::Toc(t) => t.source_span = None,
        Node::Footnote(f) => f.source_span = None,
        Node::Table(t) => {
            t.source_span = None;
            for col in &mut t.columns {
                col.source_span = None;
            }
            for row in &mut t.rows {
                row.source_span = None;
                for cell in &mut row.cells {
                    cell.source_span = None;
                    for child in &mut cell.children {
                        strip_node_spans(child);
                    }
                }
            }
        }
        Node::Shape(s) => s.source_span = None,
        Node::Connector(c) => c.source_span = None,
        Node::Pattern(p) => {
            p.source_span = None;
            strip_node_spans(&mut p.motif);
        }
        Node::Chart(c) => c.source_span = None,
        Node::Light(l) => l.source_span = None,
        Node::Mesh(m) => m.source_span = None,
        Node::Unknown(u) => {
            u.source_span = None;
            for child in &mut u.children {
                strip_node_spans(child);
            }
        }
    }
}

fn strip_page(page: &mut Page) {
    page.source_span = None;
    for zone in &mut page.safe_zones {
        zone.source_span = None;
    }
    for fold in &mut page.folds {
        fold.source_span = None;
    }
    for guide in &mut page.construction.guides {
        guide.source_span = None;
    }
    strip_ports(&mut page.ports);
    strip_defaults(&mut page.defaults);
    for node in &mut page.children {
        strip_node_spans(node);
    }
}

fn strip_ports(ports: &mut [PortDef]) {
    for port in ports {
        port.source_span = None;
    }
}

fn strip_defaults(block: &mut DefaultsBlock) {
    block.source_span = None;
    for entry in block.entries.values_mut() {
        entry.source_span = None;
    }
    for rejected in &mut block.rejected {
        rejected.entry.source_span = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{KdlAdapter, KdlSource};

    const DOC: &str = r##"zenith version=1 {
  tokens format="zenith-token-v1" {
    token id="c" type="color" value="#112233"
  }
  styles {}
  defaults {
    rect style="s"
  }
  document id="d" {
    page id="p" w=(px)100 h=(px)100 {
      frame id="f" x=(px)0 y=(px)0 w=(px)50 h=(px)50 {
        rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"c"
      }
    }
  }
}
"##;

    #[test]
    fn spans_differ_until_stripped() {
        let a = KdlAdapter.parse(DOC.as_bytes()).expect("parse");
        let shifted = format!("\n\n{DOC}");
        let b = KdlAdapter.parse(shifted.as_bytes()).expect("parse");
        assert_ne!(a, b, "a shifted source must change the spans");
        assert_eq!(strip_spans(a), strip_spans(b));
    }

    #[test]
    fn nested_node_span_is_cleared() {
        let doc = strip_spans(KdlAdapter.parse(DOC.as_bytes()).expect("parse"));
        let frame = &doc.body.pages[0].children[0];
        assert_eq!(frame.source_span(), None);
        let inner = frame.children().expect("frame children");
        assert_eq!(inner[0].source_span(), None);
        assert_eq!(doc.defaults.source_span, None);
    }
}
