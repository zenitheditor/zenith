//! The node/op/token catalog listing surfaces of `zenith schema`.

use zenith_core::schema as core_schema;
use zenith_tx::schema as tx_schema;

use crate::commands::serialize_pretty;
use crate::json_types::{
    SchemaAttr, SchemaNodeContent, SchemaNodeDetail, SchemaNodeEntry, SchemaNodeOutput,
    SchemaNodesOutput, SchemaOpDetail, SchemaOpEntry, SchemaOpFieldEntry, SchemaOpOutput,
    SchemaOpsOutput, SchemaOverviewOutput, SchemaTokenDetail, SchemaTokenEntry, SchemaTokenOutput,
    SchemaTokensOutput,
};

use super::common::format_attr_table;

/// Bare `zenith schema`: short overview with counts and drill-in hints.
///
/// Returns `(stdout, exit_code)`.
pub fn overview(json: bool) -> (String, u8) {
    let node_count = core_schema::node_kinds().len();
    let op_count = tx_schema::op_names().len();
    let token_type_count = core_schema::token_types().len();

    if json {
        let out = SchemaOverviewOutput {
            schema: "zenith-schema-v1",
            node_kinds: node_count,
            tx_ops: op_count,
            token_types: token_type_count,
        };
        (serialize_pretty(&out), 0)
    } else {
        let diag_count = core_schema::diagnostic_codes().len();
        let text = format!(
            "Zenith schema — {node_count} node kinds, {op_count} tx ops, \
             {token_type_count} token types, 8 non-node surfaces, \
             {diag_count} diagnostic codes\n\n\
             Drill in:\n  \
             zenith schema nodes              # list all node kinds\n  \
             zenith schema node <kind>        # attributes for one kind\n  \
             zenith schema ops                # list all tx ops\n  \
             zenith schema op <name>          # fields + example for one op\n  \
             zenith schema tokens             # list all token types\n  \
             zenith schema token <type>       # value form + children + example for one type\n  \
             zenith schema page               # page declaration attributes\n  \
             zenith schema asset              # asset declaration attributes\n  \
             zenith schema document           # document root attributes\n  \
             zenith schema ports              # ports block + port entry structure\n  \
             zenith schema variant            # variants block + override entry structure\n  \
             zenith schema diagnostics        # diagnostic-policy verbs + codes\n  \
             zenith schema brand              # brand-contract block (allowed colors/fonts/weights)\n  \
             zenith schema block              # block role declaration: vocab, props, scopes\n\n\
             Attribute types, required-ness, and valid values are enforced by \
             `zenith validate`."
        );
        (text, 0)
    }
}

/// `zenith schema nodes`: list all node kinds with their summaries.
///
/// Returns `(stdout, exit_code)`.
pub fn nodes(json: bool) -> (String, u8) {
    let kinds = core_schema::node_kinds();

    if json {
        let entries: Vec<SchemaNodeEntry> = kinds
            .iter()
            .map(|&kind| SchemaNodeEntry {
                kind: kind.to_owned(),
                // node_summary is always Some for every kind in node_kinds().
                summary: core_schema::node_summary(kind).unwrap_or("").to_owned(),
            })
            .collect();
        let out = SchemaNodesOutput {
            schema: "zenith-schema-v1",
            nodes: entries,
        };
        (serialize_pretty(&out), 0)
    } else {
        let mut text = String::from("node kinds:\n");
        for &kind in kinds {
            let summary = core_schema::node_summary(kind).unwrap_or("");
            text.push_str(&format!("  {kind:<12}  {summary}\n"));
        }
        (text.trim_end().to_owned(), 0)
    }
}

/// `zenith schema node <kind>`: detail for one node kind.
///
/// Returns `(stdout, exit_code)`. On unknown kind, exit_code is 1 and stdout
/// contains the error message (suitable for printing via the normal `println!`
/// path so the caller need not special-case stderr).
pub fn node_detail(kind: &str, json: bool) -> (String, u8) {
    let summary = match core_schema::node_summary(kind) {
        Some(s) => s,
        None => {
            let valid = core_schema::node_kinds().join(", ");
            // Provide a targeted hint for the two most-common near-misses.
            let hint = match kind {
                "override" | "variant" => {
                    "\nhint: 'override' and 'variant' are not node kinds — \
                     see `zenith schema variant` for the variants block and override entry."
                }
                _ => "",
            };
            let msg = format!("error: unknown node kind '{kind}'\nvalid kinds: {valid}{hint}");
            return (msg, 1);
        }
    };

    let attrs: Vec<SchemaAttr> = core_schema::node_attributes(kind)
        .iter()
        .map(|&a| SchemaAttr {
            name: a.to_owned(),
            ty: core_schema::attribute_type_for_kind(kind, a).to_owned(),
        })
        .collect();

    let content_desc = core_schema::node_content(kind);
    let example = core_schema::node_example(kind);

    if json {
        let content = content_desc.map(|d| SchemaNodeContent {
            description: d.description.to_owned(),
            example: d.example.to_owned(),
        });
        let out = SchemaNodeOutput {
            schema: "zenith-schema-v1",
            node: SchemaNodeDetail {
                kind: kind.to_owned(),
                summary: summary.to_owned(),
                attributes: attrs,
                example: example.map(str::to_owned),
                content,
            },
        };
        (serialize_pretty(&out), 0)
    } else {
        let mut text = format!("{kind}: {summary}\n");
        if attrs.is_empty() {
            text.push_str("  (no fixed attribute list)\n");
        } else {
            text.push_str("Attributes:\n");
            text.push_str(&format_attr_table(&attrs));
        }
        if let Some(d) = content_desc {
            text.push_str("\nContent:\n");
            text.push_str(&format!("  {}\n", d.description));
            text.push_str("  Example:\n");
            for line in d.example.lines() {
                text.push_str(&format!("    {line}\n"));
            }
        }
        if let Some(ex) = example {
            text.push_str("\nExample:\n");
            for line in ex.lines() {
                text.push_str(&format!("  {line}\n"));
            }
        }
        text.push_str(
            "\nNote: required-ness and full valid values are enforced by\n\
             `zenith validate` (the authoritative diagnostic loop).",
        );
        (text.trim_end().to_owned(), 0)
    }
}

/// `zenith schema ops`: list all tx ops with their summaries.
///
/// Returns `(stdout, exit_code)`.
pub fn ops(json: bool) -> (String, u8) {
    let names = tx_schema::op_names();

    if json {
        let entries: Vec<SchemaOpEntry> = names
            .iter()
            .map(|&name| SchemaOpEntry {
                op: name.to_owned(),
                summary: tx_schema::op_summary(name).unwrap_or("").to_owned(),
            })
            .collect();
        let out = SchemaOpsOutput {
            schema: "zenith-schema-v1",
            ops: entries,
        };
        (serialize_pretty(&out), 0)
    } else {
        let mut text = String::from("tx ops:\n");
        for &name in names {
            let summary = tx_schema::op_summary(name).unwrap_or("");
            text.push_str(&format!("  {name:<24}  {summary}\n"));
        }
        (text.trim_end().to_owned(), 0)
    }
}

/// `zenith schema op <name>`: full detail for one tx op (summary + fields + example).
///
/// Returns `(stdout, exit_code)`. On unknown name, exit_code is 1.
pub fn op_detail(name: &str, json: bool) -> (String, u8) {
    let summary = match tx_schema::op_summary(name) {
        Some(s) => s,
        None => {
            let valid = tx_schema::op_names().join(", ");
            let msg = format!("error: unknown op '{name}'\nvalid ops: {valid}");
            return (msg, 1);
        }
    };

    // op_fields and op_example are always Some when op_summary is Some
    // (enforced by the drift-guard tests in zenith-tx).
    let fields = tx_schema::op_fields(name).unwrap_or(&[]);
    let example = tx_schema::op_example(name).unwrap_or("");

    if json {
        let field_entries: Vec<SchemaOpFieldEntry> = fields
            .iter()
            .map(|f| SchemaOpFieldEntry {
                name: f.name.to_owned(),
                ty: f.ty.to_owned(),
                required: f.required,
            })
            .collect();
        let out = SchemaOpOutput {
            schema: "zenith-schema-v1",
            op: SchemaOpDetail {
                op: name.to_owned(),
                summary: summary.to_owned(),
                fields: field_entries,
                example: example.to_owned(),
            },
        };
        (serialize_pretty(&out), 0)
    } else {
        let mut text = format!("{name}: {summary}\n");
        if fields.is_empty() {
            text.push_str("\nFields: (none — this op carries no fields beyond the \"op\" tag)\n");
        } else {
            text.push_str("\nFields:\n");
            for f in fields {
                let req = if f.required { ", required" } else { "" };
                text.push_str(&format!("  {:<20}  ({}{req})\n", f.name, f.ty));
            }
        }
        text.push_str(&format!("\nExample:\n  {example}"));
        (text, 0)
    }
}

// ── Token type formatters ─────────────────────────────────────────────────────

/// `zenith schema tokens`: list all token types with their summaries.
///
/// Returns `(stdout, exit_code)`.
pub fn tokens(json: bool) -> (String, u8) {
    let types = core_schema::token_types();

    if json {
        let entries: Vec<SchemaTokenEntry> = types
            .iter()
            .map(|&ty| SchemaTokenEntry {
                ty: ty.to_owned(),
                // token_type_summary is always Some for every type in token_types().
                summary: core_schema::token_type_summary(ty).unwrap_or("").to_owned(),
            })
            .collect();
        let out = SchemaTokensOutput {
            schema: "zenith-schema-v1",
            token_types: entries,
        };
        (serialize_pretty(&out), 0)
    } else {
        let mut text = String::from("token types:\n");
        for &ty in types {
            let summary = core_schema::token_type_summary(ty).unwrap_or("");
            text.push_str(&format!("  {ty:<12}  {summary}\n"));
        }
        (text.trim_end().to_owned(), 0)
    }
}

/// `zenith schema token <type>`: full detail for one token type.
///
/// Returns `(stdout, exit_code)`. On unknown type, exit_code is 1 and stdout
/// contains the error message.
pub fn token_detail(ty: &str, json: bool) -> (String, u8) {
    let desc = match core_schema::token_type_descriptor(ty) {
        Some(d) => d,
        None => {
            let valid = core_schema::token_types().join(", ");
            let msg = format!("error: unknown token type '{ty}'\nvalid types: {valid}");
            return (msg, 1);
        }
    };

    if json {
        let out = SchemaTokenOutput {
            schema: "zenith-schema-v1",
            token: SchemaTokenDetail {
                ty: desc.type_name.to_owned(),
                summary: desc.summary.to_owned(),
                value_form: desc.value_form.to_owned(),
                child_nodes: desc.child_nodes.to_owned(),
                example: desc.example.to_owned(),
            },
        };
        (serialize_pretty(&out), 0)
    } else {
        let mut text = format!("{}: {}\n", desc.type_name, desc.summary);
        if !desc.value_form.is_empty() {
            text.push_str(&format!("\nValue form:\n  {}\n", desc.value_form));
        }
        if !desc.child_nodes.is_empty() {
            text.push_str(&format!("\nChild nodes:\n  {}\n", desc.child_nodes));
        }
        text.push_str(&format!(
            "\nExample:\n  {}",
            desc.example.replace('\n', "\n  ")
        ));
        text.push_str(&format!("\n\n{TOKEN_SET_NOTE}"));
        (text, 0)
    }
}

/// Every token — regardless of type — accepts an optional `set=` provenance
/// attribute, documented once here rather than duplicated per type.
const TOKEN_SET_NOTE: &str = "Every token type also accepts an optional `set=\"…\"` \
attribute: a free-form provenance id (e.g. a theme/pack id such as \
\"@zenith/theme.cobalt\") stamped by tooling. It is never resolved as a \
reference — only grouped and echoed (see token.set_partially_used in \
`zenith schema diagnostics`).";
