//! The page/asset/document surface, variant, and diagnostics surfaces.

use zenith_core::schema as core_schema;

use crate::commands::serialize_pretty;
use crate::json_types::{
    SchemaAttr, SchemaDiagnosticCode, SchemaDiagnosticsOutput, SchemaOverridePropEntry,
    SchemaPortPropEntry, SchemaPortsOutput, SchemaSurfaceOutput, SchemaVariantOutput,
};

use super::common::format_attr_table;

/// Precedence note shown on the `schema diagnostics` surface.
const DIAGNOSTICS_PRECEDENCE: &str = "policy resolution is last-wins across global config, local config, in-file diagnostics, then CLI flags";

const DIAGNOSTICS_SYNTAX: &[&str] = &[
    "allow \"<code>\"",
    "allow \"<code>\" \"<subject-id>\"",
    "allow \"<code>\" \"<subject-id>\" \"<subject-id>\"",
    "deny \"<code>\"",
    "warn \"<code>\"",
];

pub fn page(json: bool) -> (String, u8) {
    surface_detail(
        "page",
        core_schema::page_summary(),
        core_schema::page_attributes(),
        json,
    )
}

/// `zenith schema asset`: summary + recognized attributes for an asset declaration.
///
/// Returns `(stdout, exit_code)`.
pub fn asset(json: bool) -> (String, u8) {
    surface_detail(
        "asset",
        core_schema::asset_summary(),
        core_schema::asset_attributes(),
        json,
    )
}

/// `zenith schema document`: summary + recognized attributes for the document root.
///
/// Returns `(stdout, exit_code)`.
pub fn document(json: bool) -> (String, u8) {
    surface_detail(
        "document",
        core_schema::document_summary(),
        core_schema::document_attributes(),
        json,
    )
}

/// Shared formatter for non-node surfaces (page / asset / document).
fn surface_detail(
    surface: &'static str,
    summary: &'static str,
    raw_attrs: Vec<&'static str>,
    json: bool,
) -> (String, u8) {
    let attrs: Vec<SchemaAttr> = raw_attrs
        .iter()
        .map(|&a| SchemaAttr {
            name: a.to_owned(),
            // Kind-aware so surface-specific overrides (e.g. page `fit`) win;
            // unmapped attributes fall back to the generic hint.
            ty: core_schema::attribute_type_for_kind(surface, a).to_owned(),
        })
        .collect();

    if json {
        let out = SchemaSurfaceOutput {
            schema: "zenith-schema-v1",
            surface,
            summary: summary.to_owned(),
            attributes: attrs,
        };
        (serialize_pretty(&out), 0)
    } else {
        let mut text = format!("{surface}: {summary}\n");
        if attrs.is_empty() {
            text.push_str("  (no fixed attribute list)\n");
        } else {
            text.push_str("Attributes:\n");
            text.push_str(&format_attr_table(&attrs));
        }
        text.push_str(
            "\nNote: required-ness and full valid values are enforced by\n\
             `zenith validate` (the authoritative diagnostic loop).",
        );
        (text.trim_end().to_owned(), 0)
    }
}

/// `zenith schema variant`: descriptor for the `variants` block and `override` entry.
///
/// Returns `(stdout, exit_code)`.
pub fn variant(json: bool) -> (String, u8) {
    let desc = core_schema::variant_descriptor();

    if json {
        let props: Vec<SchemaOverridePropEntry> = desc
            .override_props
            .iter()
            .map(|&(name, ty, required)| SchemaOverridePropEntry {
                name: name.to_owned(),
                ty: ty.to_owned(),
                required,
            })
            .collect();
        let out = SchemaVariantOutput {
            schema: "zenith-schema-v1",
            summary: desc.summary.to_owned(),
            block_structure: desc.block_structure.to_owned(),
            variant_node: desc.variant_node.to_owned(),
            override_entry: desc.override_entry.to_owned(),
            override_props: props,
            example: desc.example.to_owned(),
        };
        (serialize_pretty(&out), 0)
    } else {
        let mut text = format!("variant: {}\n", desc.summary);

        text.push_str(&format!("\nBlock structure:\n  {}\n", desc.block_structure));
        text.push_str(&format!(
            "\nvariant node:\n  {}\n",
            desc.variant_node.replace('\n', "\n  ")
        ));
        text.push_str(&format!(
            "\noverride entry:\n  {}\n",
            desc.override_entry.replace('\n', "\n  ")
        ));

        text.push_str("\nOverride properties:\n");
        let col_width = desc
            .override_props
            .iter()
            .map(|(n, _, _)| n.len())
            .max()
            .unwrap_or(0);
        for &(name, ty, required) in desc.override_props {
            let req = if required { ", required" } else { "" };
            text.push_str(&format!(
                "  {:<col_width$}  —  ({ty}{req})\n",
                name,
                col_width = col_width,
            ));
        }

        text.push_str(&format!(
            "\nExample:\n  {}",
            desc.example.replace('\n', "\n  ")
        ));
        (text, 0)
    }
}

/// `zenith schema ports`: descriptor for the `ports` block and `port` entry.
///
/// Returns `(stdout, exit_code)`.
pub fn ports(json: bool) -> (String, u8) {
    let desc = core_schema::ports_descriptor();

    if json {
        let props: Vec<SchemaPortPropEntry> = desc
            .port_props
            .iter()
            .map(|&(name, ty, required)| SchemaPortPropEntry {
                name: name.to_owned(),
                ty: ty.to_owned(),
                required,
            })
            .collect();
        let out = SchemaPortsOutput {
            schema: "zenith-schema-v1",
            summary: desc.summary.to_owned(),
            placement: desc.placement.to_owned(),
            block_structure: desc.block_structure.to_owned(),
            port_props: props,
            example: desc.example.to_owned(),
        };
        (serialize_pretty(&out), 0)
    } else {
        let mut text = format!("ports: {}\n", desc.summary);

        text.push_str(&format!("\nPlacement:\n  {}\n", desc.placement));
        text.push_str(&format!(
            "\nBlock structure:\n  {}\n",
            desc.block_structure.replace('\n', "\n  ")
        ));

        text.push_str("\nPort properties:\n");
        let col_width = desc
            .port_props
            .iter()
            .map(|(n, _, _)| n.len())
            .max()
            .unwrap_or(0);
        for &(name, ty, required) in desc.port_props {
            let req = if required { ", required" } else { "" };
            text.push_str(&format!(
                "  {:<col_width$}  —  ({ty}{req})\n",
                name,
                col_width = col_width,
            ));
        }

        text.push_str(&format!(
            "\nExample:\n  {}",
            desc.example.replace('\n', "\n  ")
        ));
        (text, 0)
    }
}

/// `zenith schema diagnostics`: the in-file diagnostic-policy verbs and the
/// governable diagnostic codes.
///
/// Returns `(stdout, exit_code)`.
pub fn diagnostics(json: bool) -> (String, u8) {
    let summary = core_schema::diagnostics_summary();
    let verbs = core_schema::diagnostics_verbs();
    let catalog = core_schema::diagnostic_codes();

    if json {
        let codes: Vec<SchemaDiagnosticCode> = catalog
            .iter()
            .map(|info| SchemaDiagnosticCode {
                code: info.code.to_owned(),
                severity: crate::json_types::severity_str(&info.severity).to_owned(),
                summary: info.summary.to_owned(),
                governable: info.is_governable(),
            })
            .collect();
        let out = SchemaDiagnosticsOutput {
            schema: "zenith-schema-v1",
            summary: summary.to_owned(),
            verbs: verbs.iter().map(|&v| v.to_owned()).collect(),
            syntax: DIAGNOSTICS_SYNTAX.iter().map(|&s| s.to_owned()).collect(),
            precedence: DIAGNOSTICS_PRECEDENCE,
            codes,
        };
        (serialize_pretty(&out), 0)
    } else {
        let mut text = format!("diagnostics: {summary}\n\n");

        text.push_str("Policy verbs (in a root `diagnostics { … }` block):\n");
        text.push_str("  allow \"<code>\"  —  suppress this advisory/warning\n");
        text.push_str(
            "  allow \"<code>\" \"<subject-id>\" [\"<subject-id>\" …]  —  suppress only listed subjects\n",
        );
        text.push_str("  deny  \"<code>\"  —  elevate to a blocking Error (CI gate)\n");
        text.push_str("  warn  \"<code>\"  —  force to a Warning\n\n");

        text.push_str("Examples:\n");
        text.push_str("  allow \"layout.off_canvas\"\n");
        text.push_str("  allow \"layout.off_canvas\" \"bg.glow\" \"bg.rim\"\n\n");

        text.push_str("Precedence: ");
        text.push_str(DIAGNOSTICS_PRECEDENCE);
        text.push_str("\n\n");

        // Governable codes (Warning/Advisory) — what a policy can actually adjust.
        text.push_str("Governable codes (code · severity · summary):\n");
        let governable: Vec<&_> = catalog.iter().filter(|i| i.is_governable()).collect();
        let col_width = governable.iter().map(|i| i.code.len()).max().unwrap_or(0);
        for info in &governable {
            text.push_str(&format!(
                "  {:<col_width$}  {:<9}  {}\n",
                info.code,
                crate::json_types::severity_str(&info.severity),
                info.summary,
                col_width = col_width,
            ));
        }

        text.push_str(
            "\nNote: integrity Errors cannot be suppressed or weakened — `allow`/`warn` on an \
             Error code is reported as `policy.ineffective_on_error`. Only governable \
             (Warning/Advisory) codes are listed above; for the COMPLETE catalog including the \
             always-enforced Error codes (e.g. `token.raw_visual_literal`), run \
             `zenith schema diagnostics --json`.",
        );
        (text.trim_end().to_owned(), 0)
    }
}
