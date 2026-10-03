//! Human and JSON renderers for a `zenith tx` outcome.

use zenith_tx::{TxResult, TxStatus};

use crate::commands::serialize_pretty;
use crate::json_types::{self, DiagnosticJson, TxOutputJson};

use super::boxes::{BoxDelta, fmt_box, moved};

/// The optional review parts of a tx outcome: the source diff and the
/// compiled box delta. `None` means the part is not shown.
#[derive(Debug, Default)]
pub struct TxView {
    /// Unified diff of the canonical source before and after.
    pub source_diff: Option<String>,
    /// Nodes whose compiled page box changed, sorted by id.
    pub boxes: Option<Vec<BoxDelta>>,
}

/// The status as shown in human output.
pub fn status_label(status: &TxStatus) -> &'static str {
    match status {
        TxStatus::Accepted => "accepted",
        TxStatus::AcceptedWithWarnings => "accepted (with warnings)",
        TxStatus::Rejected => "rejected",
    }
}

/// The status as written in JSON output.
pub fn status_json(status: &TxStatus) -> &'static str {
    match status {
        TxStatus::Accepted => "accepted",
        TxStatus::AcceptedWithWarnings => "accepted_with_warnings",
        TxStatus::Rejected => "rejected",
    }
}

/// Render a human-readable summary of the transaction result.
pub fn render_human(result: &TxResult, view: &TxView) -> String {
    let changed = result.source_before != result.source_after;

    let mut out = String::new();
    out.push_str(&format!("status: {}\n", status_label(&result.status)));
    out.push_str(&format!("changed: {}\n", changed));

    if result.affected_node_ids.is_empty() {
        out.push_str("affected: (none)\n");
    } else {
        out.push_str(&format!(
            "affected: {}\n",
            result.affected_node_ids.join(", ")
        ));
    }

    if result.diagnostics.is_empty() {
        out.push_str("diagnostics: (none)");
    } else {
        out.push_str("diagnostics:");
        for d in &result.diagnostics {
            let sev = json_types::severity_str(&d.severity);
            let subject = d
                .subject_id
                .as_deref()
                .map(|s| format!(" ({})", s))
                .unwrap_or_default();
            out.push_str(&format!(
                "\n  {}[{}]{}: {}",
                sev, d.code, subject, d.message
            ));
            if let Some(cause) = d.cause() {
                out.push_str(&format!("\n    cause: {cause}"));
            }
        }
    }

    if let Some(diff) = view.source_diff.as_deref().filter(|d| !d.is_empty()) {
        out.push('\n');
        out.push_str(diff.trim_end());
    }
    if let Some(boxes) = &view.boxes {
        if boxes.is_empty() {
            out.push_str("\nboxes: (none)");
        } else {
            out.push_str("\nboxes:");
            for d in boxes {
                out.push_str("\n  ");
                out.push_str(&box_line(d));
            }
        }
    }

    out
}

/// `moved card.3: (526.7,340 219x120) -> (48,590 698x120)`. `resized` when
/// only w/h changed; `added` / `removed` when one side has no box.
fn box_line(d: &BoxDelta) -> String {
    let id = &d.id;
    match (d.before, d.after) {
        (Some(a), Some(b)) => {
            let kind = if moved(a, b) { "moved" } else { "resized" };
            format!("{kind} {id}: {} -> {}", fmt_box(a), fmt_box(b))
        }
        (None, Some(b)) => format!("added {id}: {}", fmt_box(b)),
        (Some(a), None) => format!("removed {id}: {}", fmt_box(a)),
        (None, None) => format!("unchanged {id}"),
    }
}

/// Render a JSON summary of the transaction result.
pub(super) fn render_json(result: &TxResult, view: TxView) -> String {
    let changed = result.source_before != result.source_after;
    let out = TxOutputJson {
        schema: "zenith-tx-v1",
        status: status_json(&result.status).to_owned(),
        affected: result.affected_node_ids.clone(),
        diagnostics: result
            .diagnostics
            .iter()
            .map(DiagnosticJson::from)
            .collect(),
        changed,
        source_diff: view.source_diff,
        boxes: view.boxes,
    };
    serialize_pretty(&out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::inspect::NodeBox;

    fn nb(x: f64, y: f64, w: f64, h: f64) -> Option<NodeBox> {
        Some(NodeBox { x, y, w, h })
    }

    #[test]
    fn box_line_kinds() {
        let delta = |before, after| BoxDelta {
            id: "card.3".to_owned(),
            before,
            after,
        };
        assert_eq!(
            box_line(&delta(
                nb(526.7, 340.0, 219.0, 120.0),
                nb(48.0, 590.0, 698.0, 120.0)
            )),
            "moved card.3: (526.7,340 219x120) -> (48,590 698x120)"
        );
        assert_eq!(
            box_line(&delta(nb(0.0, 0.0, 1.0, 1.0), nb(0.0, 0.0, 2.0, 1.0))),
            "resized card.3: (0,0 1x1) -> (0,0 2x1)"
        );
        assert_eq!(
            box_line(&delta(None, nb(0.0, 0.0, 1.0, 1.0))),
            "added card.3: (0,0 1x1)"
        );
        assert_eq!(
            box_line(&delta(nb(0.0, 0.0, 1.0, 1.0), None)),
            "removed card.3: (0,0 1x1)"
        );
    }
}
