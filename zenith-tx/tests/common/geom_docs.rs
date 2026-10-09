//! `RECT_GEOM_DOC` and `PATH_DOC`, shared by the `fill_rule_ops`, `path_ops`,
//! and `set_ops` binaries.

/// Rect at origin, 100×100. No tokens needed for geometry ops.
pub const RECT_GEOM_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="rect" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}"##;

/// Path with the minimum valid open anchor count.
pub const PATH_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="path1" {
        anchor x=(px)0 y=(px)0
        anchor x=(px)100 y=(px)0
      }
    }
  }
}"##;
