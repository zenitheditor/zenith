//! Integration tests for master-page transaction ops.

mod common;
use common::parse;
use zenith_tx::{Op, Permissions, Transaction, TxStatus, run_transaction};

const BASE: &str = r##"zenith version=1 {
  project id="proj.m" name="M"
  tokens format="zenith-token-v1" {
    token id="color.fg" type="color" value="#0f172a"
    token id="font.body" type="fontFamily" value="Inter"
    token id="size.cap" type="dimension" value=(px)12
  }
  styles {}
  document id="doc.m" title="M" {
    page id="page.1" w=(px)800 h=(px)600 {
      text id="title" x=(px)40 y=(px)40 w=(px)400 h=(px)40 fill=(token)"color.fg" {
        span "Hello"
      }
    }
    page id="page.2" w=(px)800 h=(px)600 {
      text id="body" x=(px)40 y=(px)40 w=(px)400 h=(px)40 fill=(token)"color.fg" {
        span "World"
      }
    }
  }
}
"##;

#[test]
fn create_master_add_chrome_and_assign_pages() {
    let doc = parse(BASE);
    let tx = Transaction {
        ops: vec![
            Op::CreateMaster {
                id: "m.deck".to_owned(),
            },
            Op::AddNode {
                parent: "m.deck".to_owned(),
                position: Default::default(),
                source: r#"field id="folio" type="page-number" x=(px)700 y=(px)560 w=(px)80 h=(px)24 fill=(token)"color.fg" font-family=(token)"font.body" font-size=(token)"size.cap""#.to_owned(),
            },
            Op::SetPageMaster {
                page: "page.1".to_owned(),
                master: Some("m.deck".to_owned()),
            },
            Op::SetPageMaster {
                page: "page.2".to_owned(),
                master: Some("m.deck".to_owned()),
            },
        ],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result.source_after.contains("masters {"),
        "source must emit masters block:\n{}",
        result.source_after
    );
    assert!(result.source_after.contains("master id=\"m.deck\""));
    assert!(result.source_after.contains("field id=\"folio\""));
    assert!(
        result.source_after.contains("master=\"m.deck\""),
        "pages must reference master:\n{}",
        result.source_after
    );
}

#[test]
fn create_master_duplicate_id_rejected() {
    let doc = parse(BASE);
    let tx = Transaction {
        ops: vec![
            Op::CreateMaster {
                id: "m.deck".to_owned(),
            },
            Op::CreateMaster {
                id: "m.deck".to_owned(),
            },
        ],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.duplicate_id"),
        "got {:?}",
        result.diagnostics
    );
}

#[test]
fn set_page_master_unknown_master_rejected() {
    let doc = parse(BASE);
    let tx = Transaction {
        ops: vec![Op::SetPageMaster {
            page: "page.1".to_owned(),
            master: Some("m.missing".to_owned()),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unknown_master"),
        "got {:?}",
        result.diagnostics
    );
}

#[test]
fn delete_master_and_clear_page_master() {
    let src = r##"zenith version=1 {
  project id="proj.m" name="M"
  tokens format="zenith-token-v1" {
    token id="color.fg" type="color" value="#0f172a"
  }
  styles {}
  masters {
    master id="m.deck" {
      rect id="bar" x=(px)0 y=(px)0 w=(px)800 h=(px)4 fill=(token)"color.fg"
    }
  }
  document id="doc.m" title="M" {
    page id="page.1" w=(px)800 h=(px)600 master="m.deck" {
      text id="title" x=(px)40 y=(px)40 w=(px)400 h=(px)40 fill=(token)"color.fg" {
        span "Hello"
      }
    }
  }
}
"##;
    let doc = parse(src);
    let tx = Transaction {
        ops: vec![
            Op::SetPageMaster {
                page: "page.1".to_owned(),
                master: None,
            },
            Op::DeleteMaster {
                id: "m.deck".to_owned(),
            },
        ],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        !result.source_after.contains("master=\"m.deck\""),
        "page master attribute must be cleared:\n{}",
        result.source_after
    );
    assert!(
        !result.source_after.contains("master id=\"m.deck\""),
        "master def must be gone:\n{}",
        result.source_after
    );
}

#[test]
fn reorder_master_child_succeeds() {
    let src = r##"zenith version=1 {
  project id="proj.m" name="M"
  tokens format="zenith-token-v1" {
    token id="color.fg" type="color" value="#0f172a"
  }
  styles {}
  masters {
    master id="m.deck" {
      rect id="bar" x=(px)0 y=(px)0 w=(px)800 h=(px)4 fill=(token)"color.fg"
      field id="folio" type="page-number" x=(px)700 y=(px)560 w=(px)80 h=(px)24 fill=(token)"color.fg"
    }
  }
  document id="doc.m" title="M" {
    page id="page.1" w=(px)800 h=(px)600 master="m.deck" {
      text id="title" x=(px)40 y=(px)40 w=(px)400 h=(px)40 fill=(token)"color.fg" {
        span "Hello"
      }
    }
  }
}
"##;
    let doc = parse(src);
    let tx = Transaction {
        ops: vec![Op::MoveToFront {
            node: "bar".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert!(result.affected_node_ids.contains(&"bar".to_owned()));
    // bar should now appear after folio in the master children (to-front = last).
    let bar_pos = result
        .source_after
        .find("rect id=\"bar\"")
        .expect("bar present");
    let folio_pos = result
        .source_after
        .find("field id=\"folio\"")
        .expect("folio present");
    assert!(
        bar_pos > folio_pos,
        "bar should be after folio after MoveToFront; source:\n{}",
        result.source_after
    );
}
