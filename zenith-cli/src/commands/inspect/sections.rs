//! The document-level sections of the human `zenith inspect` outline:
//! assets, masters, components, and styles. With the page outline, they hold the
//! ids, styles, and token links an agent needs before a `tx` edit, so it
//! does not read the whole source.

use zenith_core::{Document, Node};

use super::document::{Resolved, render_node_human};
use super::tree::{build_node_entry, value_text};

/// The assets, masters, components, and styles sections, each after a
/// blank line. Empty when the document declares none of them.
pub(super) fn render_sections(doc: &Document, resolved: &Resolved) -> String {
    let mut out = String::new();
    if !doc.assets.assets.is_empty() {
        out.push_str("\n\nassets:");
        for asset in &doc.assets.assets {
            out.push_str(&format!("\n  {} {}", asset.id, asset.src));
        }
    }
    let masters: Vec<(&str, &[Node])> = doc
        .masters
        .iter()
        .map(|m| (m.id.as_str(), m.children.as_slice()))
        .collect();
    push_defs(&mut out, "masters", "master", &masters, resolved);
    let components: Vec<(&str, &[Node])> = doc
        .components
        .iter()
        .map(|c| (c.id.as_str(), c.children.as_slice()))
        .collect();
    push_defs(&mut out, "components", "component", &components, resolved);
    push_styles(&mut out, doc);
    out
}

/// `masters:` or `components:`, then one outline per definition. A
/// definition's boxes are authored, relative to the definition.
fn push_defs(
    out: &mut String,
    title: &str,
    kind: &str,
    defs: &[(&str, &[Node])],
    resolved: &Resolved,
) {
    if defs.is_empty() {
        return;
    }
    out.push_str(&format!("\n\n{title}:"));
    for (id, children) in defs {
        out.push_str(&format!("\n  {kind} {id}\n"));
        for child in *children {
            out.push_str(&render_node_human(&build_node_entry(child, resolved), 2));
        }
        truncate_newline(out);
    }
}

/// `styles:`, then one line per style: its id and each property, sorted by
/// key, as `key=value`.
fn push_styles(out: &mut String, doc: &Document) {
    if doc.styles.styles.is_empty() {
        return;
    }
    out.push_str("\n\nstyles:");
    for style in &doc.styles.styles {
        out.push_str(&format!("\n  {}", style.id));
        for (key, value) in &style.properties {
            if let Some(text) = value_text(value) {
                out.push_str(&format!(" {key}={text}"));
            }
        }
    }
}

fn truncate_newline(out: &mut String) {
    if out.ends_with('\n') {
        out.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource, resolve_tokens};

    const DOC: &str = r##"zenith version=1 {
  project id="proj.s" name="Sections"
  assets {
    asset id="asset.logo" kind="image" src="assets/logo.png"
  }
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#111111"
    token id="size.body" type="dimension" value=(px)14
  }
  styles {
    style id="body" {
      font-size (token)"size.body"
      fill (token)"color.ink"
    }
  }
  masters {
    master id="m.slide" {
      rect id="m.bar" x=(px)0 y=(px)0 w=(px)40 h=(px)4 fill=(token)"color.ink"
    }
  }
  components {
    component id="card" {
      text id="card.title" x=(px)8 y=(px)8 w=(px)100 h=(px)20 style="body" { span "Title" }
    }
  }
  document id="doc.s" title="Sections" {
    page id="p1" w=(px)200 h=(px)100 master="m.slide" {
      instance id="c1" component="card" x=(px)0 y=(px)0
    }
  }
}
"##;

    #[test]
    fn sections_list_assets_masters_components_and_styles() {
        let doc = KdlAdapter.parse(DOC.as_bytes()).unwrap();
        let resolved = resolve_tokens(&doc.tokens).resolved;
        let text = render_sections(&doc, &resolved);
        assert_eq!(
            text,
            "\n\nassets:\n  asset.logo assets/logo.png\
             \n\nmasters:\n  master m.slide\n    rect m.bar  box=0,0 40x4 fill=color.ink\
             \n\ncomponents:\n  component card\n    text card.title  box=8,8 100x20 style=body \"Title\"\
             \n\nstyles:\n  body fill=color.ink font-size=size.body"
        );
    }

    #[test]
    fn a_document_without_definitions_has_no_sections() {
        let src = r##"zenith version=1 {
  document id="d" {
    page id="p" w=(px)10 h=(px)10 {}
  }
}
"##;
        let doc = KdlAdapter.parse(src.as_bytes()).unwrap();
        let resolved = resolve_tokens(&doc.tokens).resolved;
        assert_eq!(render_sections(&doc, &resolved), "");
    }
}
