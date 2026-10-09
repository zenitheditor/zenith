//! `TEXT_DOC`, shared by the `add_remove`, `set_ops`, and `transaction` binaries.

/// Minimal valid document with a `text` node (align `start`) and a `rect`.
pub const TEXT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      text id="label" x=(px)10 y=(px)10 w=(px)200 h=(px)40 align="start" {
        span "Hello"
      }
    }
  }
}"##;
