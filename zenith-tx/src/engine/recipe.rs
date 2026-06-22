//! Recipe op application: [`apply_create_recipe`], [`apply_update_recipe`],
//! and [`apply_delete_recipe`].

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, Document, RecipeDef};

use super::record_affected;

/// The scalar fields shared by `CreateRecipe` and `UpdateRecipe`, bundled into
/// one `Copy` borrow struct so the apply functions stay within the argument
/// budget. `None` for an optional field means that field is absent on the recipe.
#[derive(Clone, Copy)]
pub(super) struct RecipeScalars<'a> {
    pub(super) id: &'a str,
    pub(super) kind: &'a str,
    pub(super) seed: Option<i64>,
    pub(super) generator: Option<&'a str>,
    pub(super) bounds: Option<&'a str>,
    pub(super) detached: Option<bool>,
}

// ── CreateRecipe ──────────────────────────────────────────────────────────────

/// Append a new [`RecipeDef`] to `doc.recipes`.
///
/// Eagerly rejects with `tx.duplicate_id` if a recipe with `id` already
/// exists. On success pushes the new recipe (with empty `params`, `palette`,
/// `expanded`, and `unknown_props`; `source_span: None`) and records `id` in
/// `affected`.
pub(super) fn apply_create_recipe(
    s: RecipeScalars<'_>,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    // Eager duplicate-id check.
    if doc.recipes.iter().any(|r| r.id == s.id) {
        diagnostics.push(Diagnostic::error(
            "tx.duplicate_id",
            format!("create_recipe: a recipe with id {:?} already exists", s.id),
            None,
            Some(s.id.to_owned()),
        ));
        return;
    }

    doc.recipes.push(RecipeDef {
        id: s.id.to_owned(),
        kind: s.kind.to_owned(),
        seed: s.seed,
        generator: s.generator.map(str::to_owned),
        bounds: s.bounds.map(str::to_owned),
        detached: s.detached,
        params: Vec::new(),
        palette: Vec::new(),
        expanded: Vec::new(),
        source_span: None,
        unknown_props: BTreeMap::new(),
    });

    record_affected(s.id, affected);
}

// ── UpdateRecipe ──────────────────────────────────────────────────────────────

/// Replace the scalar fields of an existing recipe, preserving `params`,
/// `palette`, `expanded`, and `unknown_props`.
///
/// Rejects with `tx.unknown_recipe` if no recipe with `id` exists. On success
/// mutates the recipe in place (preserving its position in the Vec) and
/// records `id` in `affected`.
pub(super) fn apply_update_recipe(
    s: RecipeScalars<'_>,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    // Find the index first (shared borrow), then mutate.
    let Some(idx) = doc.recipes.iter().position(|r| r.id == s.id) else {
        diagnostics.push(Diagnostic::error(
            "tx.unknown_recipe",
            format!("update_recipe: no recipe with id {:?} exists", s.id),
            None,
            Some(s.id.to_owned()),
        ));
        return;
    };

    // SAFETY: idx came from .position() on the same Vec with no intervening
    // mutation; .get_mut() is used to satisfy the no-unchecked-index rule.
    if let Some(recipe) = doc.recipes.get_mut(idx) {
        recipe.kind = s.kind.to_owned();
        recipe.seed = s.seed;
        recipe.generator = s.generator.map(str::to_owned);
        recipe.bounds = s.bounds.map(str::to_owned);
        recipe.detached = s.detached;
        // params, palette, expanded, unknown_props, source_span are PRESERVED.
    }

    record_affected(s.id, affected);
}

// ── DeleteRecipe ──────────────────────────────────────────────────────────────

/// Remove the recipe with `id` from `doc.recipes`.
///
/// Rejects with `tx.unknown_recipe` if no recipe with `id` exists. On success
/// removes the entry and records `id` in `affected`.
pub(super) fn apply_delete_recipe(
    id: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let Some(idx) = doc.recipes.iter().position(|r| r.id == id) else {
        diagnostics.push(Diagnostic::error(
            "tx.unknown_recipe",
            format!("delete_recipe: no recipe with id {:?} exists", id),
            None,
            Some(id.to_owned()),
        ));
        return;
    };

    doc.recipes.remove(idx);

    record_affected(id, affected);
}
