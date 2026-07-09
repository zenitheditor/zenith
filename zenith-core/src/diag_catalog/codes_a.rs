//! Diagnostic catalog entries, group a. Part of the single catalog assembled
//! in `super::catalog`; ordering across groups is preserved on assembly.

use super::entry::{DiagnosticCodeInfo, info};
use crate::diagnostics::Severity;

/// Group a of the diagnostic-code catalog (see `super::catalog::DIAGNOSTIC_CODES`).
pub(super) const CODES: &[DiagnosticCodeInfo] = &[
    info(
        "anchor.cycle",
        Severity::Error,
        "Anchor parent chain forms a cycle.",
    ),
    info(
        "anchor.edge_without_sibling",
        Severity::Warning,
        "`anchor-edge` set without an `anchor-sibling`.",
    ),
    info(
        "anchor.gap_invalid_unit",
        Severity::Warning,
        "`anchor-gap` uses a non-pixel unit.",
    ),
    info(
        "anchor.parent_without_anchor",
        Severity::Warning,
        "`anchor-parent` set without an `anchor`.",
    ),
    info(
        "anchor.sibling_without_anchor",
        Severity::Warning,
        "`anchor-sibling` set without an `anchor`.",
    ),
    info(
        "anchor.unknown_edge",
        Severity::Error,
        "`anchor-edge` value is not a recognized edge.",
    ),
    info(
        "anchor.unknown_value",
        Severity::Error,
        "`anchor` value is not a recognized anchor point.",
    ),
    info(
        "anchor.unresolvable_parent",
        Severity::Error,
        "`anchor-parent` references an unknown node.",
    ),
    info(
        "anchor.unresolved_sibling",
        Severity::Error,
        "`anchor-sibling` references an unknown node.",
    ),
    info(
        "anchor.unresolved_zone",
        Severity::Error,
        "`anchor-zone` references an unknown zone.",
    ),
    info(
        "anchor.zone_without_anchor",
        Severity::Warning,
        "`anchor-zone` set without an `anchor`.",
    ),
    info(
        "asset.invalid_kind",
        Severity::Error,
        "`asset` kind is not a recognized asset kind.",
    ),
    info(
        "asset.invalid_src",
        Severity::Error,
        "`asset` src is empty or malformed.",
    ),
    info(
        "asset.missing",
        Severity::Error,
        "A declared asset file was not found on disk at render time.",
    ),
    info(
        "asset.unknown_property",
        Severity::Warning,
        "Unrecognized property on an `asset` declaration.",
    ),
    info(
        "asset.unknown_reference",
        Severity::Error,
        "Image references an undeclared asset id.",
    ),
    info(
        "baseline-grid.snap_failed",
        Severity::Warning,
        "A text column could not be snapped to the baseline grid.",
    ),
    info(
        "brand.color_off_palette",
        Severity::Warning,
        "A color token's value is not in the document brand palette.",
    ),
    info(
        "brand.font_not_allowed",
        Severity::Warning,
        "A font-family token's value is not in the approved brand font list.",
    ),
    info(
        "brand.weight_not_allowed",
        Severity::Warning,
        "A font-weight token's value is not in the approved brand weight list.",
    ),
    info(
        "component.unknown_override_target",
        Severity::Warning,
        "Instance override targets an unknown component child.",
    ),
    info(
        "component.unknown_reference",
        Severity::Error,
        "Instance references an undeclared component id.",
    ),
    info(
        "connector.invalid_anchor",
        Severity::Warning,
        "Connector anchor value is not recognized or a divided anchor is out of range.",
    ),
    info(
        "connector.invalid_marker",
        Severity::Warning,
        "Connector marker value is not recognized.",
    ),
    info(
        "connector.invalid_route",
        Severity::Warning,
        "Connector route value is not recognized.",
    ),
    info(
        "connector.missing_target",
        Severity::Warning,
        "Connector is missing a `from` or `to` target.",
    ),
    info(
        "connector.port_duplicate",
        Severity::Warning,
        "A node declares the same connector port id more than once.",
    ),
    info(
        "connector.port_invalid_target",
        Severity::Warning,
        "Connector port metadata references an unknown target node.",
    ),
    info(
        "connector.unknown_port",
        Severity::Warning,
        "Connector endpoint references an unknown port on a target node.",
    ),
    info(
        "connector.unknown_target",
        Severity::Warning,
        "Connector `from`/`to` references an unknown node.",
    ),
    info(
        "import.asset_missing",
        Severity::Error,
        "An imported document's declared asset file could not be resolved on disk.",
    ),
    info(
        "import.cycle",
        Severity::Error,
        "Composition import graph contains a cycle.",
    ),
    info(
        "import.hash_mismatch",
        Severity::Error,
        "Imported source bytes do not match the declared sha256.",
    ),
    info(
        "import.id_collision",
        Severity::Error,
        "An expanded imported node id collides with a host node id or another expanded id.",
    ),
    info(
        "import.invalid_kind",
        Severity::Error,
        "Import declaration uses an unsupported kind.",
    ),
    info(
        "import.missing",
        Severity::Error,
        "Imported `.zen` source could not be read.",
    ),
    info(
        "import.page_size_mismatch",
        Severity::Error,
        "Imported page target dimensions differ from the host page without an explicit fit.",
    ),
    info(
        "import.parse_error",
        Severity::Error,
        "Imported `.zen` source could not be parsed.",
    ),
    info(
        "import.token_conflict",
        Severity::Warning,
        "An import `token-map` target is not a resolved token in the host document.",
    ),
    info(
        "import.token_unresolved",
        Severity::Advisory,
        "An imported node references a token id absent from the import scope after mapping.",
    ),
    info(
        "import.unknown_reference",
        Severity::Error,
        "Composition source references an undeclared import or missing imported target.",
    ),
    info(
        "import.unsupported_target",
        Severity::Error,
        "Composition source target is malformed or unsupported in this context.",
    ),
    info(
        "instance.missing_reference",
        Severity::Error,
        "Instance is missing both component and source references.",
    ),
    info(
        "instance.multiple_references",
        Severity::Error,
        "Instance declares both component and source references.",
    ),
];
