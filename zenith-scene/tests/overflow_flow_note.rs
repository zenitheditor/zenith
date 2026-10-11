//! A `text.overflow` on an in-flow child of a row or column frame says that
//! resizing the box along the frame's main axis moves the siblings after it.
//! A cross-axis remedy and a node outside a flow stay unchanged.

mod common;
use common::parse;
use zenith_core::default_provider;
use zenith_scene::compile;

fn overflow_message(children: &str) -> String {
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.fn" name="FN"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
    token id="size.big" type="dimension" value=(px)44
  }}
  styles {{}}
  document id="doc.fn" title="FN" {{
    page id="p" w=(px)400 h=(px)400 {{
{children}
    }}
  }}
}}
"##
    );
    let result = compile(&parse(&src), &default_provider());
    result
        .diagnostics
        .iter()
        .find(|d| d.code == "text.overflow")
        .unwrap_or_else(|| panic!("no text.overflow in {:?}", result.diagnostics))
        .message
        .clone()
}

const LONG: &str = "A much longer headline that needs three lines";

#[test]
fn a_stack_child_gets_the_sibling_note() {
    let msg = overflow_message(&format!(
        r#"      frame id="col" x=(px)0 y=(px)0 w=(px)200 h=(px)300 layout="column" {{
        text id="t" w=(px)200 h=(px)30 fill=(token)"color.k" font-size=(token)"size.big" {{ span "{LONG}" }}
        rect id="after" w=(px)50 h=(px)20 fill=(token)"color.k"
      }}"#
    ));
    assert!(
        msg.contains("in the flow of column frame 'col'") && msg.contains("moves the siblings"),
        "{msg}"
    );
}

#[test]
fn a_row_child_that_needs_width_names_the_row() {
    let msg = overflow_message(
        r#"      frame id="r" x=(px)0 y=(px)0 w=(px)300 h=(px)300 layout="row" {
        text id="t" w=(px)60 h=(px)60 fill=(token)"color.k" font-size=(token)"size.big" { span "Unbreakableheadlineword" }
        rect id="after" w=(px)50 h=(px)20 fill=(token)"color.k"
      }"#,
    );
    assert!(msg.contains("set w=(px)"), "{msg}");
    assert!(
        msg.contains("row frame 'r'") && msg.contains("a wider box moves the siblings"),
        "{msg}"
    );
}

#[test]
fn a_remedy_on_the_cross_axis_gets_no_note() {
    let row = overflow_message(&format!(
        r#"      frame id="r" x=(px)0 y=(px)0 w=(px)300 h=(px)300 layout="row" {{
        text id="t" w=(px)100 h=(px)30 fill=(token)"color.k" font-size=(token)"size.big" {{ span "{LONG}" }}
      }}"#
    ));
    assert!(
        row.contains("set h=(px)") && !row.contains("in the flow"),
        "{row}"
    );
    let column = overflow_message(
        r#"      frame id="col" x=(px)0 y=(px)0 w=(px)200 h=(px)300 layout="column" {
        text id="t" w=(px)60 h=(px)60 fill=(token)"color.k" font-size=(token)"size.big" { span "Unbreakableheadlineword" }
      }"#,
    );
    assert!(
        column.contains("set w=(px)") && !column.contains("in the flow"),
        "{column}"
    );
}

#[test]
fn no_note_outside_a_flow() {
    let absolute_page = overflow_message(&format!(
        r#"      text id="t" x=(px)0 y=(px)0 w=(px)200 h=(px)30 fill=(token)"color.k" font-size=(token)"size.big" {{ span "{LONG}" }}"#
    ));
    assert!(!absolute_page.contains("in the flow"), "{absolute_page}");
    let absolute_child = overflow_message(&format!(
        r#"      frame id="col" x=(px)0 y=(px)0 w=(px)200 h=(px)300 layout="column" {{
        text id="t" x=(px)0 y=(px)0 w=(px)200 h=(px)30 position="absolute" fill=(token)"color.k" font-size=(token)"size.big" {{ span "{LONG}" }}
      }}"#
    ));
    assert!(!absolute_child.contains("in the flow"), "{absolute_child}");
}
