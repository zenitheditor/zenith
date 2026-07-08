//! Assembly of the complete diagnostic catalog and code lookup.
//!
//! The catalog is authored across the `codes_a`/`codes_b`/`codes_c` group
//! modules for file-size hygiene; this module concatenates them, in group
//! order, into the single [`DIAGNOSTIC_CODES`] slice the rest of the engine
//! consumes.

use super::entry::DiagnosticCodeInfo;
use super::{codes_a, codes_b, codes_c};

const LEN_A: usize = codes_a::CODES.len();
const LEN_B: usize = codes_b::CODES.len();
const LEN_C: usize = codes_c::CODES.len();
const TOTAL: usize = LEN_A + LEN_B + LEN_C;

/// Concatenate the three catalog groups into one fixed array, preserving group
/// order (`a`, then `b`, then `c`) so output determinism is unchanged.
const fn assemble() -> [DiagnosticCodeInfo; TOTAL] {
    let mut out = [codes_a::CODES[0]; TOTAL];
    let mut i = 0;

    let mut j = 0;
    while j < LEN_A {
        out[i] = codes_a::CODES[j];
        i += 1;
        j += 1;
    }
    let mut j = 0;
    while j < LEN_B {
        out[i] = codes_b::CODES[j];
        i += 1;
        j += 1;
    }
    let mut j = 0;
    while j < LEN_C {
        out[i] = codes_c::CODES[j];
        i += 1;
        j += 1;
    }
    out
}

const DIAGNOSTIC_CODES_ARR: [DiagnosticCodeInfo; TOTAL] = assemble();

/// The complete catalog of diagnostic codes the engine can emit.
///
/// Sorted into namespace groups for deterministic output. The `#[cfg(test)]`
/// drift guard below asserts the list is non-empty and that every code is unique.
///
/// NOTE: any new diagnostic code MUST be added to one of the `codes_*` group
/// modules (see the crate module docs).
pub const DIAGNOSTIC_CODES: &[DiagnosticCodeInfo] = &DIAGNOSTIC_CODES_ARR;

/// Look up a code's catalog entry, or `None` if the code is not in the catalog.
pub fn lookup(code: &str) -> Option<&'static DiagnosticCodeInfo> {
    DIAGNOSTIC_CODES.iter().find(|e| e.code == code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Severity;
    use std::collections::BTreeSet;

    #[test]
    fn catalog_is_nonempty() {
        assert!(
            !DIAGNOSTIC_CODES.is_empty(),
            "the diagnostic-code catalog must not be empty"
        );
    }

    #[test]
    fn catalog_codes_are_unique() {
        let mut seen: BTreeSet<&'static str> = BTreeSet::new();
        for entry in DIAGNOSTIC_CODES {
            assert!(
                !entry.code.is_empty(),
                "a catalog entry has an empty code string"
            );
            assert!(
                seen.insert(entry.code),
                "duplicate diagnostic code in the catalog: {}",
                entry.code,
            );
        }
    }

    #[test]
    fn governable_codes_are_warning_or_advisory() {
        for entry in DIAGNOSTIC_CODES {
            assert_eq!(
                entry.is_governable(),
                entry.severity != Severity::Error,
                "is_governable() must mirror (severity != Error) for {}",
                entry.code,
            );
        }
    }

    #[test]
    fn lookup_finds_known_and_misses_unknown() {
        assert!(lookup("layout.off_canvas").is_some());
        assert!(lookup("policy.unknown_code").is_some());
        assert!(lookup("not.a_real_code").is_none());
    }

    #[test]
    fn compile_font_diagnostics_are_catalogued_and_governable() {
        // The compile-stage font diagnostics (emitted by zenith-scene) must be in
        // the catalog so the diagnostic policy can govern them on the render path.
        let unresolved = lookup("font.unresolved").expect("font.unresolved must be catalogued");
        assert_eq!(unresolved.severity, Severity::Advisory);
        assert!(
            unresolved.is_governable(),
            "font.unresolved must be governable"
        );

        let glyph_missing =
            lookup("font.glyph_missing").expect("font.glyph_missing must be catalogued");
        assert_eq!(glyph_missing.severity, Severity::Warning);
        assert!(
            glyph_missing.is_governable(),
            "font.glyph_missing must be governable"
        );

        // `font.local` must be a governable Advisory so `deny "font.local"` can
        // gate CI renders that resolved a machine-local font.
        let local = lookup("font.local").expect("font.local must be catalogued");
        assert_eq!(local.severity, Severity::Advisory);
        assert!(local.is_governable(), "font.local must be governable");
    }

    #[test]
    fn scene_diagnostics_are_catalogued() {
        // Codes emitted by zenith-scene during compilation must be in the catalog
        // so that `diagnostics { … }` blocks can govern them.
        let cases: &[(&str, Severity)] = &[
            ("baseline-grid.snap_failed", Severity::Warning),
            ("footnote.body_overlap", Severity::Advisory),
            ("footnote.no_live_area", Severity::Advisory),
            ("scene.invalid_color", Severity::Advisory),
            ("scene.invalid_import_source", Severity::Advisory),
            ("scene.missing_geometry", Severity::Advisory),
            ("scene.no_pages", Severity::Advisory),
            ("scene.page_out_of_range", Severity::Advisory),
            ("scene.text_unshaped", Severity::Advisory),
            ("scene.unknown_component", Severity::Advisory),
            ("scene.unknown_import", Severity::Advisory),
            ("scene.unknown_import_component", Severity::Advisory),
            ("scene.unknown_import_page", Severity::Advisory),
            ("scene.unresolved_token", Severity::Advisory),
            ("scene.unsupported_import_source", Severity::Advisory),
            ("scene.unsupported_import_target", Severity::Advisory),
            ("scene.unsupported_node", Severity::Advisory),
            ("scene.unsupported_unit", Severity::Advisory),
            ("scene.wrong_token_type", Severity::Advisory),
            ("table.flow_overflow", Severity::Advisory),
            ("text.fit_failed", Severity::Error),
            ("text.forced_break", Severity::Warning),
            ("text.overflow", Severity::Warning),
        ];
        for (code, expected_severity) in cases {
            let entry =
                lookup(code).unwrap_or_else(|| panic!("{code} must be in the diagnostic catalog"));
            assert_eq!(
                entry.severity, *expected_severity,
                "{code} catalog severity mismatch"
            );
        }
    }

    #[test]
    fn import_diagnostics_are_catalogued() {
        let cases: &[(&str, Severity)] = &[
            ("import.cycle", Severity::Error),
            ("import.hash_mismatch", Severity::Error),
            ("import.invalid_kind", Severity::Error),
            ("import.missing", Severity::Error),
            ("import.page_size_mismatch", Severity::Error),
            ("import.parse_error", Severity::Error),
            ("import.unknown_reference", Severity::Error),
            ("import.unsupported_target", Severity::Error),
        ];
        for (code, expected_severity) in cases {
            let entry =
                lookup(code).unwrap_or_else(|| panic!("{code} must be in the diagnostic catalog"));
            assert_eq!(
                entry.severity, *expected_severity,
                "{code} catalog severity mismatch"
            );
        }
    }

    #[test]
    fn render_diagnostics_are_catalogued() {
        // Codes emitted by zenith-cli at render time must be in the catalog.
        let asset_missing =
            lookup("asset.missing").expect("asset.missing must be in the diagnostic catalog");
        assert_eq!(asset_missing.severity, Severity::Error);

        let text_src_missing =
            lookup("text.src_missing").expect("text.src_missing must be in the diagnostic catalog");
        assert_eq!(text_src_missing.severity, Severity::Error);
        assert!(
            !text_src_missing.is_governable(),
            "text.src_missing must not be governable (always-Error)"
        );

        let image_overflow =
            lookup("image.overflow").expect("image.overflow must be in the diagnostic catalog");
        assert_eq!(image_overflow.severity, Severity::Advisory);
        assert!(
            image_overflow.is_governable(),
            "image.overflow must be governable"
        );

        let image_upscale =
            lookup("image.upscale").expect("image.upscale must be in the diagnostic catalog");
        assert_eq!(image_upscale.severity, Severity::Advisory);
        assert!(
            image_upscale.is_governable(),
            "image.upscale must be governable"
        );
    }

    #[test]
    fn transaction_diagnostics_are_catalogued() {
        // Codes emitted by zenith-tx must be in the catalog so that policy
        // validation can recognise them as known codes.
        let known_tx_codes: &[(&str, Severity)] = &[
            ("node.locked", Severity::Error),
            ("tx.duplicate_id", Severity::Error),
            ("tx.geometry_unresolved", Severity::Warning),
            ("tx.invalid_geometry", Severity::Error),
            ("tx.invalid_geometry_tolerance", Severity::Error),
            ("tx.invalid_node_spec", Severity::Error),
            ("tx.invalid_parent", Severity::Error),
            ("tx.invalid_path_anchor", Severity::Error),
            ("tx.invalid_value", Severity::Error),
            ("tx.locked_skipped", Severity::Warning),
            ("tx.noop", Severity::Advisory),
            ("tx.not_a_pattern", Severity::Error),
            ("tx.out_of_range", Severity::Error),
            ("tx.pattern_not_expandable", Severity::Error),
            ("tx.pattern_unresolved_bounds", Severity::Error),
            ("tx.unknown_node", Severity::Error),
            ("tx.unknown_recipe", Severity::Error),
            ("tx.unknown_style", Severity::Error),
            ("tx.unknown_token", Severity::Error),
            ("tx.unsupported_closed_path", Severity::Error),
            ("tx.unsupported_path_handles", Severity::Error),
            ("tx.unsupported_property", Severity::Error),
            ("tx.wrong_node_type", Severity::Error),
        ];
        for (code, expected_severity) in known_tx_codes {
            let entry =
                lookup(code).unwrap_or_else(|| panic!("{code} must be in the diagnostic catalog"));
            assert_eq!(
                entry.severity, *expected_severity,
                "{code} catalog severity mismatch"
            );
        }
    }
}
