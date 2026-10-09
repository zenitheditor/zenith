mod common;
use common::parse;
#[path = "common/mixed_code_docs.rs"]
mod mixed_code_docs;
#[path = "common/text_doc.rs"]
mod text_doc;
#[path = "common/two_rect_doc.rs"]
mod two_rect_doc;
use mixed_code_docs::{CODE_DOC, MIXED_DOC};
use text_doc::TEXT_DOC;
use two_rect_doc::TWO_RECT_DOC;

/// Page with a group that contains two rects.
const ADD_GROUP_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)320 h=(px)200 {
      group id="grp1" {
        rect id="g.a" x=(px)0 y=(px)0 w=(px)50 h=(px)50
        rect id="g.b" x=(px)0 y=(px)0 w=(px)50 h=(px)50
      }
    }
  }
}"##;
use zenith_tx::op::{OpPathAnchor, OpPathSubpath};
use zenith_tx::{Op, OpSpan, Permissions, Position, Transaction, TxStatus, run_transaction};

#[path = "add_remove/addnode.rs"]
mod addnode;
#[path = "add_remove/duplicatenode.rs"]
mod duplicatenode;
#[path = "add_remove/duplicatepage.rs"]
mod duplicatepage;
#[path = "add_remove/opacity.rs"]
mod opacity;
#[path = "add_remove/removenode.rs"]
mod removenode;
#[path = "add_remove/replacetext.rs"]
mod replacetext;
