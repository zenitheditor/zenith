//! Write planned fixes into a KDL document.

use kdl::{KdlDocument, KdlIdentifier, KdlValue};

use super::locate::entry_at_mut;
use super::mint::insert_tokens;
use super::plan::{AppliedFix, Change, Plan};

/// Apply `plan` to `doc` and return the records of the fixes that landed.
///
/// Entry edits run first, then token insertion, so every planned path stays
/// valid. A site that no longer resolves is skipped.
pub(super) fn apply(doc: &mut KdlDocument, plan: Plan) -> Vec<AppliedFix> {
    let mut applied = Vec::new();
    for fix in plan.fixes {
        let Some(entry) = entry_at_mut(doc, &fix.site) else {
            continue;
        };
        match fix.change {
            Change::TokenRef(id) => {
                entry.set_value(KdlValue::String(id));
                entry.set_ty(KdlIdentifier::from("token"));
            }
            Change::Text(text) => entry.set_value(KdlValue::String(text)),
            Change::Rename(name) => entry.set_name(Some(KdlIdentifier::from(name))),
        }
        entry.clear_format();
        applied.push(fix.record);
    }
    if !plan.minted.is_empty() && !insert_tokens(doc, &plan.minted) {
        return Vec::new();
    }
    applied
}
