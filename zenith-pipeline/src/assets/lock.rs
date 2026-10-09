//! `--locked` hash verification of asset bytes.

use sha2::{Digest, Sha256};

use crate::error::PipelineError;

/// Check that `bytes` match the `sha256` declared on an asset.
///
/// `id` names the asset and `kind` is a short noun (`"asset"` or
/// `"font asset"`), both for the message.
///
/// # Errors
///
/// Exit code 2 when no hash is declared (`asset.sha256_missing`) or the hex
/// digest differs from the declared one, compared trimmed and
/// case-insensitively (`asset.sha256_mismatch`).
pub(crate) fn verify_locked_sha256(
    id: &str,
    kind: &str,
    sha256: Option<&str>,
    bytes: &[u8],
) -> Result<(), PipelineError> {
    let declared = sha256.ok_or_else(|| {
        PipelineError::new(
            "asset.sha256_missing",
            format!(
                "--locked: {kind} '{id}' has no declared sha256; add sha256=\"<hex>\" to it or \
                 drop --locked"
            ),
            2,
        )
    })?;
    let hex = format!("{:x}", Sha256::digest(bytes));
    if declared.trim().to_lowercase() != hex {
        return Err(PipelineError::new(
            "asset.sha256_mismatch",
            format!("--locked: {kind} '{id}' sha256 mismatch (declared {declared}, actual {hex})"),
            2,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_mismatched_hashes_are_errors() {
        let missing = verify_locked_sha256("a", "asset", None, b"x").expect_err("missing");
        assert_eq!(missing.diagnostics[0].code, "asset.sha256_missing");
        let wrong = verify_locked_sha256("a", "asset", Some("00"), b"x").expect_err("wrong");
        assert_eq!(wrong.diagnostics[0].code, "asset.sha256_mismatch");
        let hex = format!("{:x}", Sha256::digest(b"x"));
        let upper = format!(" {} ", hex.to_uppercase());
        assert!(verify_locked_sha256("a", "asset", Some(&upper), b"x").is_ok());
    }
}
