//! `THREE_RECTS_DOC`, shared by the `align_distribute` and `set_ops` binaries.

/// Three sibling rects at different x positions (10, 50, 90) on a 400×300
/// page; all have the same width (80px).
pub const THREE_RECTS_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)10 y=(px)20 w=(px)80 h=(px)50
      rect id="r2" x=(px)50 y=(px)60 w=(px)80 h=(px)50
      rect id="r3" x=(px)90 y=(px)100 w=(px)80 h=(px)50
    }
  }
}"##;
