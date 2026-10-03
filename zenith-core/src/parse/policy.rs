//! Standalone parsers for config blocks (`diagnostics { … }`, `brand { … }`).
//!
//! A Zenith config file (global or local) is a small KDL document whose
//! meaningful nodes are a top-level `diagnostics { … }` block and/or a
//! top-level `brand { … }` block, written exactly like their in-document
//! counterparts:
//!
//! ```text
//! diagnostics {
//!     allow "layout.off_canvas"
//!     deny  "font.local"
//!     warn  "image.upscale"
//! }
//!
//! brand {
//!     colors "#0b1f33" "#ffffff"
//!     fonts  "Noto Sans"
//!     weights 400 700
//! }
//! ```
//!
//! This is NOT a full `.zen` document — there is no `zenith` root node, no
//! `project`, no `tokens`. Only the `diagnostics` and `brand` blocks are read;
//! any other top-level node, and any unknown child inside either block, is a
//! hard [`ParseError`] with a did-you-mean. A source with no matching node
//! (including an empty source) yields the respective default (empty policy /
//! empty contract), which is an identity pass.

use crate::ast::brand::BrandContract;
use crate::ast::policy::DiagnosticPolicy;
use crate::error::{ParseError, ParseErrorCode};
use crate::parse::transform::{
    check_config_document, transform_brand_contract, transform_diagnostic_policy,
};

/// Map a KDL syntax error to a [`ParseError`], keeping the first diagnostic span.
fn config_kdl_error(e: kdl::KdlError) -> ParseError {
    let message = format!("config KDL parse error: {e}");
    let span = e.diagnostics.first().map(|d| {
        let start = d.span.offset();
        crate::ast::Span {
            start,
            end: start + d.span.len(),
        }
    });
    ParseError::at(span, ParseErrorCode::InvalidKdl, message)
}

/// Parse a standalone `diagnostics { … }` KDL config block from raw bytes.
///
/// The bytes are decoded and parsed as KDL using the same UTF-8-then-KDL path
/// as the document parser. The first top-level `diagnostics` node is delegated
/// to the shared `transform_diagnostic_policy` transform. An unknown top-level
/// node or an unknown child inside `diagnostics`/`brand` is an error. A missing
/// `diagnostics` node returns [`DiagnosticPolicy::default`].
///
/// # Errors
///
/// Returns a [`ParseError`] if the bytes are not valid UTF-8, are not valid
/// KDL, if a top-level node or block child is unknown, or if a recognized
/// `allow`/`deny`/`warn` entry is missing its diagnostic-code string argument.
pub fn parse_diagnostic_policy(source: &[u8]) -> Result<DiagnosticPolicy, ParseError> {
    // Step 1: validate UTF-8 (same contract as `KdlAdapter::parse`).
    let text = std::str::from_utf8(source).map_err(|e| {
        ParseError::spanless(
            ParseErrorCode::NotUtf8,
            format!("config source is not valid UTF-8: {e}"),
        )
    })?;

    // Step 2: parse KDL.
    let kdl_doc: kdl::KdlDocument = text.parse().map_err(config_kdl_error)?;

    // Step 3: reject unknown config content, then locate the first top-level
    // `diagnostics` node and transform it. Absent → empty policy (identity pass).
    check_config_document(&kdl_doc)?;
    match kdl_doc
        .nodes()
        .iter()
        .find(|n| n.name().value() == "diagnostics")
    {
        Some(node) => transform_diagnostic_policy(node),
        None => Ok(DiagnosticPolicy::default()),
    }
}

/// Parse a standalone `brand { … }` KDL config block from raw bytes.
///
/// The bytes are decoded and parsed as KDL using the same UTF-8-then-KDL path
/// as the document parser. The first top-level `brand` node is delegated to
/// the shared `transform_brand_contract` transform. An unknown top-level node
/// or an unknown child inside `diagnostics`/`brand` is an error. A missing
/// `brand` node returns [`BrandContract::default`].
///
/// # Errors
///
/// Returns a [`ParseError`] if the bytes are not valid UTF-8, are not valid
/// KDL, if a top-level node or block child is unknown, or if a recognized
/// category entry (`colors`, `fonts`, `weights`) contains a value of the wrong
/// type (e.g. a non-string color or an out-of-range weight integer).
pub fn parse_brand_contract(source: &[u8]) -> Result<BrandContract, ParseError> {
    // Step 1: validate UTF-8 (same contract as `KdlAdapter::parse`).
    let text = std::str::from_utf8(source).map_err(|e| {
        ParseError::spanless(
            ParseErrorCode::NotUtf8,
            format!("config source is not valid UTF-8: {e}"),
        )
    })?;

    // Step 2: parse KDL.
    let kdl_doc: kdl::KdlDocument = text.parse().map_err(config_kdl_error)?;

    // Step 3: reject unknown config content, then locate the first top-level
    // `brand` node and transform it. Absent → empty contract (identity pass).
    check_config_document(&kdl_doc)?;
    match kdl_doc.nodes().iter().find(|n| n.name().value() == "brand") {
        Some(node) => transform_brand_contract(node),
        None => Ok(BrandContract::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::policy::PolicyVerb;

    #[test]
    fn parses_allow_deny_warn_block() {
        let src = br#"diagnostics {
            allow "layout.off_canvas" "bg.glow" "bg.rim"
            deny  "font.local"
            warn  "image.upscale"
        }"#;
        let policy = parse_diagnostic_policy(src).expect("must parse");
        assert_eq!(policy.entries.len(), 3);
        assert_eq!(
            policy.verb_for("layout.off_canvas", Some("bg.glow")),
            Some(&PolicyVerb::Allow)
        );
        assert_eq!(
            policy.verb_for("layout.off_canvas", Some("bg.rim")),
            Some(&PolicyVerb::Allow)
        );
        assert_eq!(policy.verb_for("layout.off_canvas", Some("shape.1")), None);
        assert_eq!(policy.verb_for("font.local", None), Some(&PolicyVerb::Deny));
        assert_eq!(
            policy.verb_for("image.upscale", None),
            Some(&PolicyVerb::Warn)
        );
    }

    #[test]
    fn empty_source_is_default_policy() {
        let policy = parse_diagnostic_policy(b"").expect("empty must parse");
        assert!(policy.entries.is_empty());
    }

    #[test]
    fn no_diagnostics_node_is_default_policy() {
        // A config with only a `brand` block has no `diagnostics` → empty policy.
        let src = br##"brand {
            colors "#ffffff"
        }"##;
        let policy = parse_diagnostic_policy(src).expect("must parse");
        assert!(policy.entries.is_empty());
    }

    #[test]
    fn unknown_top_level_node_is_error() {
        let src = br#"diagnostic {
            allow "layout.off_canvas"
        }"#;
        let err = parse_diagnostic_policy(src).expect_err("unknown root must fail");
        assert_eq!(err.code, ParseErrorCode::UnexpectedNode);
        assert!(
            err.message.contains("did you mean 'diagnostics'?"),
            "{}",
            err.message
        );
        let err = parse_brand_contract(src).expect_err("unknown root must fail");
        assert_eq!(err.code, ParseErrorCode::UnexpectedNode);
    }

    #[test]
    fn unknown_policy_verb_is_error() {
        let src = br#"diagnostics {
            alow "layout.off_canvas"
        }"#;
        let err = parse_diagnostic_policy(src).expect_err("unknown verb must fail");
        assert_eq!(err.code, ParseErrorCode::UnexpectedNode);
        assert!(
            err.message.contains("did you mean 'allow'?"),
            "{}",
            err.message
        );
    }

    #[test]
    fn unknown_brand_child_is_error() {
        let src = br##"brand {
            color "#ffffff"
        }"##;
        let err = parse_brand_contract(src).expect_err("unknown brand child must fail");
        assert_eq!(err.code, ParseErrorCode::UnexpectedNode);
        assert!(
            err.message.contains("did you mean 'colors'?"),
            "{}",
            err.message
        );
    }

    #[test]
    fn malformed_kdl_is_error() {
        let src = b"diagnostics {{{ not valid kdl";
        let err = parse_diagnostic_policy(src).expect_err("must fail");
        assert_eq!(err.code, ParseErrorCode::InvalidKdl);
    }

    #[test]
    fn entry_missing_code_is_error() {
        // `deny` with no quoted code string is a hard parse error.
        let src = br#"diagnostics {
            deny
        }"#;
        let err = parse_diagnostic_policy(src).expect_err("missing code must fail");
        assert_eq!(err.code, ParseErrorCode::InvalidPropertyValue);
    }

    #[test]
    fn subject_argument_must_be_string() {
        let src = br#"diagnostics {
            allow "layout.off_canvas" 1
        }"#;
        let err = parse_diagnostic_policy(src).expect_err("invalid subject must fail");
        assert_eq!(err.code, ParseErrorCode::InvalidPropertyValue);
    }

    #[test]
    fn subject_property_is_rejected() {
        let src = br#"diagnostics {
            allow "layout.off_canvas" subject="bg.glow"
        }"#;
        let err = parse_diagnostic_policy(src).expect_err("subject property must fail");
        assert_eq!(err.code, ParseErrorCode::InvalidPropertyValue);
    }

    #[test]
    fn last_wins_across_entries() {
        let src = br#"diagnostics {
            deny "image.upscale"
            warn "image.upscale"
        }"#;
        let policy = parse_diagnostic_policy(src).expect("must parse");
        assert_eq!(
            policy.verb_for("image.upscale", None),
            Some(&PolicyVerb::Warn)
        );
    }

    // ── parse_brand_contract ─────────────────────────────────────────────────

    #[test]
    fn brand_contract_parses_all_categories() {
        let src = br##"brand {
            colors "#0b1f33" "#ffffff"
            fonts  "Noto Sans" "Roboto"
            weights 400 700
        }"##;
        let contract = parse_brand_contract(src).expect("must parse");
        assert_eq!(
            contract.allowed_colors,
            Some(vec!["#0b1f33".to_owned(), "#ffffff".to_owned()])
        );
        assert_eq!(
            contract.allowed_fonts,
            Some(vec!["Noto Sans".to_owned(), "Roboto".to_owned()])
        );
        assert_eq!(contract.allowed_weights, Some(vec![400u32, 700u32]));
    }

    #[test]
    fn brand_contract_absent_node_is_default() {
        let contract = parse_brand_contract(b"").expect("empty must parse");
        assert!(contract.is_empty(), "absent brand node must yield default");
    }

    #[test]
    fn brand_contract_no_brand_node_is_default() {
        let src = br#"diagnostics {
            allow "token.unused"
        }"#;
        let contract = parse_brand_contract(src).expect("must parse");
        assert!(
            contract.is_empty(),
            "source with only diagnostics node must yield default brand contract"
        );
    }

    #[test]
    fn brand_contract_malformed_kdl_is_error() {
        let src = b"brand {{{ not valid kdl";
        let err = parse_brand_contract(src).expect_err("must fail");
        assert_eq!(err.code, ParseErrorCode::InvalidKdl);
    }

    #[test]
    fn brand_contract_partial_categories_only_colors() {
        let src = br##"brand {
            colors "#ff0000"
        }"##;
        let contract = parse_brand_contract(src).expect("must parse");
        assert_eq!(contract.allowed_colors, Some(vec!["#ff0000".to_owned()]));
        assert!(
            contract.allowed_fonts.is_none(),
            "absent fonts must remain None (unconstrained)"
        );
        assert!(
            contract.allowed_weights.is_none(),
            "absent weights must remain None (unconstrained)"
        );
    }
}
