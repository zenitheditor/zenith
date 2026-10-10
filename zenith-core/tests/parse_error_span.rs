//! A parse error for a missing required property carries the node span and
//! names the nearest ancestor that has an id.

use zenith_core::{KdlAdapter, KdlSource, ParseError};

fn parse_err(src: &str) -> ParseError {
    match KdlAdapter.parse(src.as_bytes()) {
        Ok(_) => panic!("expected a parse error for:\n{src}"),
        Err(e) => e,
    }
}

#[test]
fn idless_text_in_table_cell_has_span_and_names_table() {
    let src = r#"zenith version=1 {
  document id="doc" {
    page id="pg" w=(px)800 h=(px)600 {
      table id="schedule" x=(px)0 y=(px)0 w=(px)400 h=(px)200 {
        column width=(px)200
        row {
          cell {
            text {
              span "Name"
            }
          }
        }
      }
    }
  }
}
"#;
    let err = parse_err(src);
    assert!(
        err.message
            .contains("node `text` in cell of row of table \"schedule\""),
        "message must name the ancestors, got: {}",
        err.message
    );
    assert!(
        err.message
            .contains("is missing required string property `id`"),
        "message must name the missing property, got: {}",
        err.message
    );
    let Some(span) = err.span else {
        panic!("error has no span: {}", err.message);
    };
    let offending = src.get(span.start..span.end).unwrap_or("");
    assert!(
        offending.starts_with("text"),
        "span must cover the `text` node, got: {offending:?}"
    );
    let line = src[..span.start].matches('\n').count() + 1;
    assert_eq!(line, 8, "span must point at the `text` line");
}

#[test]
fn idless_node_in_frame_names_frame() {
    let src = r#"zenith version=1 {
  document id="doc" {
    page id="pg" w=(px)800 h=(px)600 {
      frame id="card" w=(px)100 h=(px)100 {
        rect x=(px)0 y=(px)0 w=(px)10 h=(px)10
      }
    }
  }
}
"#;
    let err = parse_err(src);
    assert!(
        err.message
            .contains("node `rect` in frame \"card\" is missing"),
        "got: {}",
        err.message
    );
    assert!(err.span.is_some(), "error must carry a span");
}

#[test]
fn idless_node_on_page_names_page() {
    let src = r#"zenith version=1 {
  document id="doc" {
    page id="pg" w=(px)800 h=(px)600 {
      rect x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }
  }
}
"#;
    let err = parse_err(src);
    assert!(
        err.message
            .contains("node `rect` in page \"pg\" is missing"),
        "got: {}",
        err.message
    );
    assert!(err.span.is_some(), "error must carry a span");
}

#[test]
fn missing_page_size_has_span() {
    let src = r#"zenith version=1 {
  document id="doc" {
    page id="pg" h=(px)600 {
    }
  }
}
"#;
    let err = parse_err(src);
    assert!(
        err.span.is_some(),
        "error must carry a span: {}",
        err.message
    );
}

#[test]
fn bad_token_value_has_span() {
    let src = r##"zenith version=1 {
  tokens format="zenith-token-v1" {
    token id="color.x" type="color"
  }
  document id="doc" {
    page id="pg" w=(px)800 h=(px)600 {}
  }
}
"##;
    let err = parse_err(src);
    assert!(
        err.span.is_some(),
        "error must carry a span: {}",
        err.message
    );
}

/// A KDL syntax error points at the line that holds the error, not at the
/// start of the document, for several malformed inputs.
#[test]
fn kdl_syntax_errors_point_at_the_offending_line() {
    let cases: [(&str, usize, &str); 4] = [
        (
            "zenith version=1 {\n  document id=\"d\" {\n    page id=\"p\" w=(px)10 h=(px)10 {\n      rect id=\"r\" x=(px)0 y=(px)\n    }\n  }\n}\n",
            4,
            "rect id=\"r\" x=(px)0 y=(px)",
        ),
        (
            "zenith version=1 {\n  document id=\"d\" {\n    page id=\"p\" w=\n  }\n}\n",
            3,
            "page id=\"p\" w=",
        ),
        (
            "zenith version=1 {\n  document id=\"d\" {\n    text id=\"t\" {\n      span \"open\n    }\n  }\n}\n",
            4,
            "\"o",
        ),
        ("zenith version=1 {\n  project id=\n", 2, "project id="),
    ];
    for (src, line, starts) in cases {
        let err = parse_err(src);
        let span = err
            .span
            .unwrap_or_else(|| panic!("no span: {}", err.message));
        let at = src[..span.start].matches('\n').count() + 1;
        assert_eq!(at, line, "{src:?}: {err:?}");
        assert!(
            src[span.start..].starts_with(starts),
            "{src:?}: span text {:?}",
            &src[span.start..span.end]
        );
        assert!(
            err.message.contains(&format!("line {line},")),
            "{}",
            err.message
        );
    }
}
