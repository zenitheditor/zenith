//! Pure logic for `zenith fmt`.
//!
//! The public entry point [`run`] operates entirely on in-memory source text;
//! the caller is responsible for reading the original file and writing the
//! formatted result back to disk.

use sha2::{Digest, Sha256};
use zenith_core::{KdlAdapter, KdlSource};

use crate::commands::serialize_pretty;
use crate::json_types::FmtOutput;

// ── Result type ───────────────────────────────────────────────────────────────

/// Error type for `fmt`.
#[derive(Debug)]
pub struct FmtErr {
    /// Human-readable message.
    pub message: String,
    /// Exit code: 2 for parse or format errors.
    pub exit_code: u8,
}

/// The outcome of a successful fmt run.
#[derive(Debug)]
pub struct FmtResult {
    /// The canonical formatted bytes to write back to disk.
    pub formatted: Vec<u8>,
    /// Whether the formatted bytes differ from the original source.
    pub changed: bool,
    /// Hex-encoded hash of the formatted content (stable, deterministic).
    pub hash: String,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Parse `src`, format it canonically, and return the result.
///
/// Returns `Err(FmtErr)` on parse or format failure.  On success returns
/// [`FmtResult`] with the formatted bytes, a `changed` flag, and a content hash.
pub fn run(src: &str) -> Result<FmtResult, FmtErr> {
    // Parse ─────────────────────────────────────────────────────────────────
    let doc = KdlAdapter.parse(src.as_bytes()).map_err(|e| FmtErr {
        message: format!("error[parse.error]: {}", e.message),
        exit_code: 2,
    })?;

    // Format ─────────────────────────────────────────────────────────────────
    let formatted = KdlAdapter.format(&doc).map_err(|e| FmtErr {
        message: format!("error[fmt.format_failed]: {}", e.message),
        exit_code: 2,
    })?;

    let changed = formatted != src.as_bytes();
    let hash = hex_hash(&formatted);

    Ok(FmtResult {
        formatted,
        changed,
        hash,
    })
}

/// Render the fmt result for stdout.
///
/// If `json` is true emits a JSON object; otherwise a one-line human message.
pub fn render_stdout(result: &FmtResult, json: bool) -> String {
    if json {
        let out = FmtOutput {
            schema: "zenith-fmt-v1",
            changed: result.changed,
            hash: result.hash.clone(),
        };
        serialize_pretty(&out)
    } else if result.changed {
        format!("formatted (hash: {})", result.hash)
    } else {
        format!("already canonical (hash: {})", result.hash)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Compute the lowercase hex-encoded SHA-256 of `bytes`.
///
/// SHA-256 is stable across toolchain versions and platforms, so the reported
/// content hash is reproducible (unlike `DefaultHasher`) and consistent with the
/// content-addressing model used elsewhere in the project.
pub(crate) fn hex_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// A valid `.zen` source used as input to the formatter tests.
    ///
    /// This does NOT need to be byte-for-byte canonical — idempotency is the
    /// critical property verified by `fmt_is_idempotent`.
    const FMT_INPUT: &str = r##"zenith version=1 {
  project id="proj.f" name="Fmt Test"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#f8fafc"
  }
  styles {
  }
  document id="doc.f" title="Fmt Test" {
    page id="page.f" w=(px)320 h=(px)200 {
      rect id="rect.f" x=(px)0 y=(px)0 w=(px)320 h=(px)200 fill=(token)"color.bg"
    }
  }
}
"##;

    #[test]
    fn already_formatted_doc_reports_not_changed() {
        // First fmt produces the canonical form.
        let first = run(FMT_INPUT).expect("must succeed");
        // Second fmt on the canonical form must report changed=false.
        let canonical = std::str::from_utf8(&first.formatted).expect("utf8");
        let second = run(canonical).expect("second run");
        assert!(
            !second.changed,
            "fmt on already-canonical doc must report changed=false"
        );
    }

    #[test]
    fn fmt_is_idempotent() {
        let first = run(FMT_INPUT).expect("first fmt");
        let second = run(std::str::from_utf8(&first.formatted).expect("utf8")).expect("second fmt");
        assert_eq!(
            first.formatted, second.formatted,
            "fmt must be idempotent: fmt(fmt(x)) == fmt(x)"
        );
    }

    #[test]
    fn parse_error_returns_err() {
        let result = run("not valid kdl {{{");
        assert!(result.is_err(), "parse error must return Err");
        assert_eq!(result.unwrap_err().exit_code, 2);
    }

    #[test]
    fn hash_is_stable() {
        let r1 = run(FMT_INPUT).expect("r1");
        let r2 = run(FMT_INPUT).expect("r2");
        assert_eq!(r1.hash, r2.hash, "hash must be stable across runs");
    }
}
