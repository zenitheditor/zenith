//! Editor gesture ops: `nudge_geometry`, `set_anchor`, `nudge_anchor_gap`,
//! `detach_anchor`, `nudge_line_points`, and the `scale` mode of
//! `transform_path_anchors`. Each test sends the op as JSON, as the editor
//! does. Compiled page boxes from `zenith-scene` check visual positions.

use std::collections::BTreeMap;

use zenith_core::{Document, KdlAdapter, KdlSource, default_provider};
use zenith_scene::{DocumentPrep, PageCompiler};
use zenith_tx::{Op, Transaction, TxResult, TxStatus, run_transaction};

#[path = "gestures/anchor.rs"]
mod anchor;
#[path = "gestures/line.rs"]
mod line;
#[path = "gestures/nudge_geometry.rs"]
mod nudge_geometry;
#[path = "gestures/path_scale.rs"]
mod path_scale;

/// A one-page document around `body`, with a color and two dimension
/// tokens.
fn doc(body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj" name="T"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
    token id="space.x" type="dimension" value=(px)40
    token id="space.pt" type="dimension" value=(pt)12
  }}
  styles {{ }}
  document id="doc1" title="T" {{
    page id="pg" w=(px)480 h=(px)360 {{
      {body}
    }}
  }}
}}"##
    )
}

fn parse(src: &str) -> Document {
    KdlAdapter
        .parse(src.as_bytes())
        .unwrap_or_else(|e| panic!("parse: {}\n{src}", e.message))
}

/// Run the ops in `ops_json` (a JSON array body) against `src`.
fn run(src: &str, ops_json: &str) -> TxResult {
    run_tx(src, &format!(r#"{{"ops":[{ops_json}]}}"#))
}

/// Run a full transaction JSON against `src`.
fn run_tx(src: &str, tx_json: &str) -> TxResult {
    let tx = Transaction::from_json(tx_json).unwrap_or_else(|e| panic!("{}", e.message));
    run_transaction(&parse(src), &tx).expect("run_transaction must not error")
}

/// Run one typed op against `src`.
fn run_op(src: &str, op: Op) -> TxResult {
    let tx = Transaction {
        ops: vec![op],
        permissions: Default::default(),
    };
    run_transaction(&parse(src), &tx).expect("run_transaction must not error")
}

fn accepted(r: &TxResult) {
    assert!(
        matches!(
            r.status,
            TxStatus::Accepted | TxStatus::AcceptedWithWarnings
        ),
        "expected accepted; got {:?}: {:?}",
        r.status,
        r.diagnostics
    );
}

/// Assert `r` is rejected with `code`, and return that diagnostic's message.
fn rejected(r: &TxResult, code: &str) -> String {
    assert_eq!(r.status, TxStatus::Rejected, "{:?}", r.diagnostics);
    assert_eq!(r.source_after, r.source_before);
    r.diagnostics
        .iter()
        .find(|d| d.code == code)
        .map(|d| d.message.clone())
        .unwrap_or_else(|| panic!("no {code} diagnostic in {:?}", r.diagnostics))
}

/// The source line that declares node `id`.
fn line_of<'a>(source: &'a str, id: &str) -> &'a str {
    let needle = format!("id=\"{id}\"");
    source
        .lines()
        .find(|l| l.contains(&needle))
        .unwrap_or_else(|| panic!("no line for {id}:\n{source}"))
}

/// Compiled page-space boxes `(x, y, w, h)` of page 0.
fn boxes(source: &str) -> BTreeMap<String, (f64, f64, f64, f64)> {
    let doc = parse(source);
    let prep = DocumentPrep::new(&doc, None, None);
    let fonts = default_provider();
    PageCompiler::new(&prep, &fonts)
        .compiled_boxes(0)
        .into_iter()
        .map(|(k, b)| (k, (b.rect.x, b.rect.y, b.rect.w, b.rect.h)))
        .collect()
}

fn box_of(source: &str, id: &str) -> (f64, f64, f64, f64) {
    *boxes(source)
        .get(id)
        .unwrap_or_else(|| panic!("no compiled box for {id}"))
}
