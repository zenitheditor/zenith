mod common;
use common::parse;
#[path = "common/geom_docs.rs"]
mod geom_docs;
#[path = "common/image_doc.rs"]
mod image_doc;
#[path = "common/line_doc.rs"]
mod line_doc;
#[path = "common/mixed_code_docs.rs"]
mod mixed_code_docs;
#[path = "common/text_doc.rs"]
mod text_doc;
#[path = "common/three_rects_doc.rs"]
mod three_rects_doc;
#[path = "common/two_rect_doc.rs"]
mod two_rect_doc;
use geom_docs::{PATH_DOC, RECT_GEOM_DOC};
use image_doc::IMAGE_DOC;
use line_doc::LINE_DOC;
use mixed_code_docs::{CODE_DOC, MIXED_DOC};
use text_doc::TEXT_DOC;
use three_rects_doc::THREE_RECTS_DOC;
use two_rect_doc::TWO_RECT_DOC;
use zenith_tx::op::OpPathAnchor;
use zenith_tx::{Op, OpPoint, Permissions, Transaction, TxStatus, run_transaction};

#[path = "set_ops/fill_stroke.rs"]
mod fill_stroke;
#[path = "set_ops/geometry.rs"]
mod geometry;
#[path = "set_ops/points_paths.rs"]
mod points_paths;
#[path = "set_ops/text_align.rs"]
mod text_align;
#[path = "set_ops/text_overflow.rs"]
mod text_overflow;
#[path = "set_ops/visibility.rs"]
mod visibility;
