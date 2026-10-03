//! Write planned fixes into a KDL document.

use kdl::{KdlDocument, KdlEntry, KdlIdentifier, KdlValue};

use super::locate::{Slot, entry_at_mut, node_at_mut};
use super::mint::insert_tokens;
use super::plan::{AppliedFix, Change, Plan};

/// Apply `plan` to `doc` and return the records of the fixes that landed.
///
/// Entry edits run first, then token insertion, so every planned path stays
/// valid. A site that no longer resolves is skipped.
pub(super) fn apply(doc: &mut KdlDocument, plan: Plan) -> Vec<AppliedFix> {
    let mut applied = Vec::new();
    for fix in plan.fixes {
        if let Slot::NewProperty(property) = &fix.site.slot {
            let Change::Value { ty, value } = fix.change else {
                continue;
            };
            let Some(node) = node_at_mut(doc, &fix.site.node) else {
                continue;
            };
            let mut entry = KdlEntry::new_prop(property.as_str(), value);
            if let Some(ty) = ty {
                entry.set_ty(KdlIdentifier::from(ty));
            }
            node.push(entry);
            applied.push(fix.record);
            continue;
        }
        if let Change::NodeName(name) = &fix.change {
            let Some(node) = node_at_mut(doc, &fix.site.node) else {
                continue;
            };
            node.set_name(KdlIdentifier::from(name.as_str()));
            node.clear_format();
            applied.push(fix.record);
            continue;
        }
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
            Change::Value { ty, value } => match (ty, entry.name().cloned()) {
                (Some(ty), _) => {
                    entry.set_value(value);
                    entry.set_ty(KdlIdentifier::from(ty));
                }
                // No annotation: a fresh entry drops the old one.
                (None, Some(name)) => *entry = KdlEntry::new_prop(name, value),
                (None, None) => *entry = KdlEntry::new(value),
            },
            // Handled above: a node-name change has no entry.
            Change::NodeName(_) => continue,
        }
        entry.clear_format();
        applied.push(fix.record);
    }
    if !plan.minted.is_empty() && !insert_tokens(doc, &plan.minted) {
        return Vec::new();
    }
    applied
}
