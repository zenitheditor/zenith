//! The error a source patch returns when it cannot produce exact text.

use std::fmt;

/// Why the patcher cannot edit the source text in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchErrorCode {
    /// The source text is not valid KDL.
    InvalidSource,
    /// The canonical formatter rejected one of the documents.
    FormatFailed,
    /// A changed node has no single counterpart in the source text.
    UnalignedSource,
    /// The source layout around an edit point has no exact in-place edit.
    UnsupportedLayout,
    /// The patched text is not valid Zenith source.
    ReparseFailed,
    /// The patched text parses to a document other than the target.
    VerifyMismatch,
}

impl PatchErrorCode {
    /// The stable code string, `patch.<snake_event>`.
    pub fn as_str(self) -> &'static str {
        match self {
            PatchErrorCode::InvalidSource => "patch.invalid_source",
            PatchErrorCode::FormatFailed => "patch.format_failed",
            PatchErrorCode::UnalignedSource => "patch.unaligned_source",
            PatchErrorCode::UnsupportedLayout => "patch.unsupported_layout",
            PatchErrorCode::ReparseFailed => "patch.reparse_failed",
            PatchErrorCode::VerifyMismatch => "patch.verify_mismatch",
        }
    }
}

/// A source patch that cannot run. The caller writes canonical text instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchError {
    /// The error category.
    pub code: PatchErrorCode,
    /// What blocked the patch.
    pub message: String,
}

impl PatchError {
    pub(super) fn new(code: PatchErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for PatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for PatchError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_use_the_patch_namespace() {
        let all = [
            PatchErrorCode::InvalidSource,
            PatchErrorCode::FormatFailed,
            PatchErrorCode::UnalignedSource,
            PatchErrorCode::UnsupportedLayout,
            PatchErrorCode::ReparseFailed,
            PatchErrorCode::VerifyMismatch,
        ];
        for code in all {
            let s = code.as_str();
            assert!(s.starts_with("patch."), "{s}");
            assert!(
                s["patch.".len()..]
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_'),
                "{s}"
            );
        }
    }

    #[test]
    fn display_names_the_code() {
        let e = PatchError::new(PatchErrorCode::UnsupportedLayout, "inline block");
        assert_eq!(e.to_string(), "patch.unsupported_layout: inline block");
    }
}
