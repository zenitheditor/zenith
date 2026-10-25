//! Font embedding license: the OS/2 `fsType` bits of a face.
//!
//! The PDF backend embeds each face it draws as selectable text. The
//! `fsType` field states what a document may embed. A face with no OS/2
//! table, or one that fails to parse, reads as [`FontEmbedding::Allowed`]:
//! the parse error belongs to the font loader, not to this check.

use ttf_parser::Permissions;

/// What the `fsType` bits of a face allow a document to embed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontEmbedding {
    /// Installable, editable, or preview-and-print embedding.
    Allowed,
    /// Embedding is allowed, but only as the whole face (`fsType` bit 8).
    NoSubsetting,
    /// Restricted license (`fsType` 0x0002): no embedding at all.
    Restricted,
    /// Only bitmaps may embed (`fsType` bit 9). A PDF embeds outlines, so
    /// this face cannot embed either.
    BitmapOnly,
}

impl FontEmbedding {
    /// `true` when a PDF cannot embed the face as text.
    #[must_use]
    pub fn forbids_embedding(self) -> bool {
        match self {
            FontEmbedding::Allowed | FontEmbedding::NoSubsetting => false,
            FontEmbedding::Restricted | FontEmbedding::BitmapOnly => true,
        }
    }

    /// The license in a few words, for diagnostic messages.
    #[must_use]
    pub fn describe(self) -> &'static str {
        match self {
            FontEmbedding::Allowed => "embedding allowed",
            FontEmbedding::NoSubsetting => "embedding allowed without subsetting",
            FontEmbedding::Restricted => "restricted license (fsType 0x0002)",
            FontEmbedding::BitmapOnly => "bitmap-only embedding (fsType bit 9)",
        }
    }
}

/// The embedding license of face `index` in `bytes`.
#[must_use]
pub fn font_embedding(bytes: &[u8], index: u32) -> FontEmbedding {
    let Ok(face) = ttf_parser::Face::parse(bytes, index) else {
        return FontEmbedding::Allowed;
    };
    // The face-level `ttf-parser` helpers read a missing OS/2 table as "not
    // allowed". Read the table itself so a face without one stays allowed.
    let Some(os2) = face.tables().os2 else {
        return FontEmbedding::Allowed;
    };
    match os2.permissions() {
        Some(Permissions::Restricted) => return FontEmbedding::Restricted,
        Some(Permissions::Installable | Permissions::PreviewAndPrint | Permissions::Editable)
        | None => {}
    }
    if !os2.is_outline_embedding_allowed() {
        return FontEmbedding::BitmapOnly;
    }
    if !os2.is_subsetting_allowed() {
        return FontEmbedding::NoSubsetting;
    }
    FontEmbedding::Allowed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::bundled_faces;

    /// Byte offset of `fsType` inside the OS/2 table.
    const FS_TYPE_OFFSET: usize = 8;

    /// `bytes` with the OS/2 `fsType` field set to `fs_type`. The table
    /// checksum goes stale, which `ttf-parser` does not check.
    fn with_fs_type(bytes: &[u8], fs_type: u16) -> Vec<u8> {
        let mut out = bytes.to_vec();
        let num_tables = u16::from_be_bytes([out[4], out[5]]) as usize;
        for i in 0..num_tables {
            let rec = 12 + i * 16;
            if &out[rec..rec + 4] == b"OS/2" {
                let off =
                    u32::from_be_bytes([out[rec + 8], out[rec + 9], out[rec + 10], out[rec + 11]])
                        as usize;
                let at = off + FS_TYPE_OFFSET;
                out[at..at + 2].copy_from_slice(&fs_type.to_be_bytes());
                return out;
            }
        }
        panic!("face has no OS/2 table");
    }

    fn sample_face() -> &'static [u8] {
        bundled_faces()
            .iter()
            .find_map(|f| f.bytes)
            .expect("a bundled face is present")
    }

    #[test]
    fn every_bundled_face_is_embeddable() {
        for face in bundled_faces() {
            if let Some(bytes) = face.bytes {
                let license = font_embedding(bytes, 0);
                assert!(
                    !license.forbids_embedding(),
                    "bundled face {} reads {license:?}",
                    face.file
                );
            }
        }
    }

    #[test]
    fn restricted_bit_forbids_embedding() {
        let bytes = with_fs_type(sample_face(), 0x0002);
        assert_eq!(font_embedding(&bytes, 0), FontEmbedding::Restricted);
        assert!(FontEmbedding::Restricted.forbids_embedding());
    }

    #[test]
    fn bitmap_only_bit_forbids_embedding() {
        let bytes = with_fs_type(sample_face(), 0x0200);
        assert_eq!(font_embedding(&bytes, 0), FontEmbedding::BitmapOnly);
    }

    #[test]
    fn no_subsetting_bit_still_embeds() {
        let bytes = with_fs_type(sample_face(), 0x0100);
        assert_eq!(font_embedding(&bytes, 0), FontEmbedding::NoSubsetting);
        assert!(!FontEmbedding::NoSubsetting.forbids_embedding());
    }

    #[test]
    fn installable_and_unparsable_faces_are_allowed() {
        let bytes = with_fs_type(sample_face(), 0x0000);
        assert_eq!(font_embedding(&bytes, 0), FontEmbedding::Allowed);
        assert_eq!(font_embedding(b"not a font", 0), FontEmbedding::Allowed);
    }
}
