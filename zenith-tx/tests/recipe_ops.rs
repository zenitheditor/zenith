//! Integration tests for the `create_recipe`, `update_recipe`, and
//! `delete_recipe` transaction ops.

mod common;
use common::*;
use zenith_tx::{Op, Permissions, Transaction, TxStatus, run_transaction};

// ── Fixture ───────────────────────────────────────────────────────────────────

/// Minimal valid document with one existing recipe (kind "scatter", seed 7,
/// with a param and a palette entry) so update/delete tests have a target.
const RECIPE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.accent" type="color" value="#3b82f6"
  }
  styles { }
  recipes {
    recipe id="recipe.a" kind="scatter" seed=7 {
      param name="count" value=10
      palette token="color.accent"
      expanded node="r1"
    }
  }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.accent"
    }
  }
}"##;

/// Minimal document with NO recipes block — used for create-from-empty tests.
const EMPTY_RECIPE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}"##;

// ── create_recipe: new id → Accepted ─────────────────────────────────────────

/// (a) create_recipe with a fresh id is accepted; the new recipe appears in the
/// re-parsed document with the right scalars and empty params/palette/expanded.
#[test]
fn create_recipe_accepted() {
    let doc = parse(EMPTY_RECIPE_DOC);
    let initial_count = doc.recipes.len();

    let tx = Transaction {
        ops: vec![Op::CreateRecipe {
            id: "recipe.new".to_owned(),
            kind: "aurora".to_owned(),
            seed: Some(42),
            generator: Some("aurora@1".to_owned()),
            bounds: Some("pg1".to_owned()),
            detached: Some(false),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "expected Accepted; diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        result.affected_node_ids,
        vec!["recipe.new".to_owned()],
        "affected must contain the new recipe id"
    );
    assert!(
        result.source_after.contains("id=\"recipe.new\""),
        "source_after must contain the new recipe id; got:\n{}",
        result.source_after
    );
    assert!(
        result.source_after.contains("kind=\"aurora\""),
        "source_after must contain the kind; got:\n{}",
        result.source_after
    );
    assert!(
        result.source_after.contains("seed=42"),
        "source_after must contain the seed; got:\n{}",
        result.source_after
    );

    let after_doc = parse(&result.source_after);
    assert_eq!(
        after_doc.recipes.len(),
        initial_count + 1,
        "recipe count should increase by 1"
    );
    let recipe = after_doc
        .recipes
        .iter()
        .find(|r| r.id == "recipe.new")
        .expect("new recipe must exist");
    assert_eq!(recipe.kind, "aurora");
    assert_eq!(recipe.seed, Some(42));
    assert_eq!(recipe.generator.as_deref(), Some("aurora@1"));
    assert_eq!(recipe.bounds.as_deref(), Some("pg1"));
    assert_eq!(recipe.detached, Some(false));
    assert!(recipe.params.is_empty(), "params must be empty on create");
    assert!(recipe.palette.is_empty(), "palette must be empty on create");
    assert!(
        recipe.expanded.is_empty(),
        "expanded must be empty on create"
    );
}

/// (b) create_recipe with only the required fields (no optional scalars) is
/// accepted; optional fields are absent/None in the result.
#[test]
fn create_recipe_minimal_accepted() {
    let doc = parse(EMPTY_RECIPE_DOC);

    let tx = Transaction {
        ops: vec![Op::CreateRecipe {
            id: "recipe.min".to_owned(),
            kind: "scatter".to_owned(),
            seed: None,
            generator: None,
            bounds: None,
            detached: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "expected Accepted; diagnostics: {:?}",
        result.diagnostics
    );

    let after_doc = parse(&result.source_after);
    let recipe = after_doc
        .recipes
        .iter()
        .find(|r| r.id == "recipe.min")
        .expect("recipe.min must exist");
    assert_eq!(recipe.seed, None);
    assert_eq!(recipe.generator, None);
    assert_eq!(recipe.bounds, None);
    assert_eq!(recipe.detached, None);
}

// ── create_recipe: duplicate id → Rejected ───────────────────────────────────

/// (c) create_recipe with an id that already exists → Rejected (tx.duplicate_id).
#[test]
fn create_recipe_duplicate_id_rejected() {
    let doc = parse(RECIPE_DOC);
    // RECIPE_DOC already has "recipe.a".
    let tx = Transaction {
        ops: vec![Op::CreateRecipe {
            id: "recipe.a".to_owned(),
            kind: "scatter".to_owned(),
            seed: None,
            generator: None,
            bounds: None,
            detached: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Rejected,
        "expected Rejected; diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.duplicate_id"),
        "expected tx.duplicate_id; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── update_recipe: scalar fields replaced, params preserved ──────────────────

/// (d) update_recipe changes scalar fields while preserving params/palette/expanded.
#[test]
fn update_recipe_scalars_preserved_params() {
    let doc = parse(RECIPE_DOC);
    // recipe.a has kind="scatter", seed=7, one param, one palette, one expanded.

    let tx = Transaction {
        ops: vec![Op::UpdateRecipe {
            id: "recipe.a".to_owned(),
            kind: "aurora".to_owned(),
            seed: Some(99),
            generator: Some("aurora@2".to_owned()),
            bounds: None,
            detached: Some(true),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "expected Accepted; diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        result.affected_node_ids,
        vec!["recipe.a".to_owned()],
        "affected must contain the updated recipe id"
    );

    let after_doc = parse(&result.source_after);
    let recipe = after_doc
        .recipes
        .iter()
        .find(|r| r.id == "recipe.a")
        .expect("recipe.a must still exist");

    // Scalar fields updated.
    assert_eq!(recipe.kind, "aurora", "kind must be updated");
    assert_eq!(recipe.seed, Some(99), "seed must be updated");
    assert_eq!(
        recipe.generator.as_deref(),
        Some("aurora@2"),
        "generator must be updated"
    );
    assert_eq!(recipe.bounds, None, "bounds None must clear the field");
    assert_eq!(recipe.detached, Some(true), "detached must be updated");

    // Preserved fields.
    assert_eq!(recipe.params.len(), 1, "params must be preserved (1 entry)");
    assert_eq!(
        recipe.palette.len(),
        1,
        "palette must be preserved (1 entry)"
    );
    assert_eq!(
        recipe.expanded.len(),
        1,
        "expanded must be preserved (1 entry)"
    );
}

/// (e) update_recipe with detached None→Some(true) ("detach" semantics).
#[test]
fn update_recipe_detach() {
    let doc = parse(RECIPE_DOC);

    let tx = Transaction {
        ops: vec![Op::UpdateRecipe {
            id: "recipe.a".to_owned(),
            kind: "scatter".to_owned(),
            seed: Some(7),
            generator: None,
            bounds: None,
            detached: Some(true),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "expected Accepted; diagnostics: {:?}",
        result.diagnostics
    );

    let after_doc = parse(&result.source_after);
    let recipe = after_doc
        .recipes
        .iter()
        .find(|r| r.id == "recipe.a")
        .expect("recipe.a must still exist");
    assert_eq!(recipe.detached, Some(true), "detached must be Some(true)");
    assert!(
        result.source_after.contains("detached=#true"),
        "source_after must contain detached=#true; got:\n{}",
        result.source_after
    );
}

// ── update_recipe: unknown id → Rejected ─────────────────────────────────────

/// (f) update_recipe on a non-existent id → Rejected (tx.unknown_recipe).
#[test]
fn update_recipe_unknown_id_rejected() {
    let doc = parse(RECIPE_DOC);
    let tx = Transaction {
        ops: vec![Op::UpdateRecipe {
            id: "recipe.does_not_exist".to_owned(),
            kind: "scatter".to_owned(),
            seed: None,
            generator: None,
            bounds: None,
            detached: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Rejected,
        "expected Rejected; diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unknown_recipe"),
        "expected tx.unknown_recipe; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── delete_recipe: removes entry ─────────────────────────────────────────────

/// (g) delete_recipe removes the recipe from the document.
#[test]
fn delete_recipe_accepted() {
    let doc = parse(RECIPE_DOC);
    let initial_count = doc.recipes.len();

    let tx = Transaction {
        ops: vec![Op::DeleteRecipe {
            id: "recipe.a".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "expected Accepted; diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        result.affected_node_ids,
        vec!["recipe.a".to_owned()],
        "affected must contain the deleted recipe id"
    );

    let after_doc = parse(&result.source_after);
    assert_eq!(
        after_doc.recipes.len(),
        initial_count - 1,
        "recipe count should decrease by 1"
    );
    assert!(
        after_doc.recipes.iter().all(|r| r.id != "recipe.a"),
        "recipe.a must not exist after delete"
    );
}

// ── delete_recipe: unknown id → Rejected ─────────────────────────────────────

/// (h) delete_recipe on a non-existent id → Rejected (tx.unknown_recipe).
#[test]
fn delete_recipe_unknown_id_rejected() {
    let doc = parse(RECIPE_DOC);
    let tx = Transaction {
        ops: vec![Op::DeleteRecipe {
            id: "recipe.does_not_exist".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Rejected,
        "expected Rejected; diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unknown_recipe"),
        "expected tx.unknown_recipe; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── determinism / order preservation ─────────────────────────────────────────

/// (i) Create two recipes; update the first; assert the order is preserved
/// (first-created is still first in the result).
#[test]
fn create_two_update_first_order_preserved() {
    let doc = parse(EMPTY_RECIPE_DOC);

    // Create recipe.x then recipe.y.
    let tx = Transaction {
        ops: vec![
            Op::CreateRecipe {
                id: "recipe.x".to_owned(),
                kind: "scatter".to_owned(),
                seed: Some(1),
                generator: None,
                bounds: None,
                detached: None,
            },
            Op::CreateRecipe {
                id: "recipe.y".to_owned(),
                kind: "aurora".to_owned(),
                seed: Some(2),
                generator: None,
                bounds: None,
                detached: None,
            },
        ],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "create two: {:?}",
        result.diagnostics
    );

    // Now update recipe.x (first).
    let doc2 = parse(&result.source_after);
    let tx2 = Transaction {
        ops: vec![Op::UpdateRecipe {
            id: "recipe.x".to_owned(),
            kind: "scatter".to_owned(),
            seed: Some(99),
            generator: None,
            bounds: None,
            detached: Some(true),
        }],
        permissions: Permissions::default(),
    };
    let result2 = run_transaction(&doc2, &tx2).expect("run_transaction should not error");
    assert_eq!(
        result2.status,
        TxStatus::Accepted,
        "update first: {:?}",
        result2.diagnostics
    );

    let after_doc = parse(&result2.source_after);
    assert_eq!(after_doc.recipes.len(), 2, "must still have two recipes");
    // Order: recipe.x first, recipe.y second.
    assert_eq!(
        after_doc.recipes[0].id, "recipe.x",
        "recipe.x must remain first"
    );
    assert_eq!(
        after_doc.recipes[1].id, "recipe.y",
        "recipe.y must remain second"
    );
    // Updated value.
    assert_eq!(
        after_doc.recipes[0].seed,
        Some(99),
        "seed of recipe.x must be updated"
    );
    assert_eq!(
        after_doc.recipes[0].detached,
        Some(true),
        "detached of recipe.x must be updated"
    );
    // recipe.y untouched.
    assert_eq!(
        after_doc.recipes[1].seed,
        Some(2),
        "recipe.y seed must be unchanged"
    );
}
