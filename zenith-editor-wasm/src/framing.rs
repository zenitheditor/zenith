//! Split the stdin bytes of one request: the JSON envelope, then the raw
//! font files the envelope lists.
//!
//! Stdin holds the envelope text. When the envelope lists `fonts`, one NUL
//! byte follows the text, then each listed file's bytes back to back, in
//! list order. JSON text never holds a raw NUL, so the first NUL ends the
//! envelope. Raw bytes skip the base64 and JSON cost of a large face: the
//! CJK fallback face alone is 5.6 MB.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::protocol::ErrorBody;

/// The byte between the envelope text and the font bytes.
const SEPARATOR: u8 = 0;

/// One font file that follows the envelope.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct FontPart {
    /// The bundled or fallback font file name, for example
    /// `NotoSansCJKsc-Fallback.otf`.
    pub(crate) file: String,
    /// The file's length in bytes.
    pub(crate) size: usize,
}

/// The envelope text of `input` and the bytes after the separator. The
/// tail is empty when `input` has no separator.
///
/// # Errors
///
/// `request.invalid_utf8` when the envelope text is not UTF-8.
pub(crate) fn split(input: &[u8]) -> Result<(&str, &[u8]), ErrorBody> {
    let mut pieces = input.splitn(2, |&b| b == SEPARATOR);
    let head = pieces.next().unwrap_or_default();
    let tail = pieces.next().unwrap_or_default();
    let text = std::str::from_utf8(head).map_err(|e| {
        ErrorBody::new(
            "request.invalid_utf8",
            format!(
                "the request envelope is not UTF-8 ({e}); send the JSON envelope as UTF-8, then one NUL byte before any font bytes"
            ),
        )
    })?;
    Ok((text, tail))
}

/// The font files in `tail`, keyed by file name, as `parts` lists them.
///
/// # Errors
///
/// `request.invalid_fonts` when `parts` names a file twice, or when the
/// listed sizes do not add up to the length of `tail`.
pub(crate) fn attached_fonts(
    parts: &[FontPart],
    tail: &[u8],
) -> Result<BTreeMap<String, Vec<u8>>, ErrorBody> {
    let mut out = BTreeMap::new();
    let mut rest = tail;
    for part in parts {
        let Some((bytes, after)) = rest.split_at_checked(part.size) else {
            return Err(size_mismatch(parts, tail.len()));
        };
        rest = after;
        if out.insert(part.file.clone(), bytes.to_vec()).is_some() {
            return Err(ErrorBody::new(
                "request.invalid_fonts",
                format!(
                    "fonts lists '{}' twice; list each font file once",
                    part.file
                ),
            ));
        }
    }
    if !rest.is_empty() {
        return Err(size_mismatch(parts, tail.len()));
    }
    Ok(out)
}

/// The `request.invalid_fonts` error for `parts` that do not cover the
/// `got` bytes after the envelope.
fn size_mismatch(parts: &[FontPart], got: usize) -> ErrorBody {
    let listed = parts
        .iter()
        .try_fold(0usize, |sum, part| sum.checked_add(part.size))
        .map_or_else(|| "more than usize::MAX".to_owned(), |n| n.to_string());
    ErrorBody::new(
        "request.invalid_fonts",
        format!(
            "fonts lists {listed} bytes, but {got} bytes follow the envelope; send one NUL byte after the envelope, then each listed file's bytes in list order"
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(file: &str, size: usize) -> FontPart {
        FontPart {
            file: file.to_owned(),
            size,
        }
    }

    #[test]
    fn no_separator_has_an_empty_tail() {
        let (text, tail) = split(br#"{"method":"ping"}"#).expect("split");
        assert_eq!(text, r#"{"method":"ping"}"#);
        assert!(tail.is_empty());
    }

    #[test]
    fn the_first_nul_ends_the_envelope() {
        let (text, tail) = split(b"{}\0ab\0c").expect("split");
        assert_eq!(text, "{}");
        assert_eq!(tail, b"ab\0c");
    }

    #[test]
    fn a_non_utf8_envelope_is_rejected() {
        let err = split(b"{\xff}").expect_err("not UTF-8");
        assert_eq!(err.code, "request.invalid_utf8");
    }

    #[test]
    fn parts_slice_the_tail_in_list_order() {
        let fonts = attached_fonts(&[part("b.ttf", 2), part("a.ttf", 3)], b"BBaaa").expect("fonts");
        assert_eq!(fonts.get("a.ttf").map(Vec::as_slice), Some(&b"aaa"[..]));
        assert_eq!(fonts.get("b.ttf").map(Vec::as_slice), Some(&b"BB"[..]));
    }

    #[test]
    fn sizes_must_add_up_to_the_tail() {
        let short = attached_fonts(&[part("a.ttf", 4)], b"abc").expect_err("short");
        assert_eq!(short.code, "request.invalid_fonts");
        assert!(
            short.message.contains("lists 4 bytes, but 3"),
            "{}",
            short.message
        );
        let extra = attached_fonts(&[], b"x").expect_err("unlisted bytes");
        assert_eq!(extra.code, "request.invalid_fonts");
        let huge = attached_fonts(&[part("a.ttf", usize::MAX), part("b.ttf", 1)], b"")
            .expect_err("overflow");
        assert_eq!(huge.code, "request.invalid_fonts");
    }

    #[test]
    fn a_file_listed_twice_is_rejected() {
        let err = attached_fonts(&[part("a.ttf", 1), part("a.ttf", 1)], b"xy").expect_err("twice");
        assert_eq!(err.code, "request.invalid_fonts");
        assert!(err.message.contains("'a.ttf' twice"), "{}", err.message);
    }
}
