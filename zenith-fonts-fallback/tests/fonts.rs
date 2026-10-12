//! The embedded fonts match the pinned digests, parse, and cover the
//! scripts the chain exists for.
#![cfg(feature = "embed")]

use sha2::{Digest, Sha256};
use zenith_fonts_fallback::fallback_faces;

fn bytes_of(file: &str) -> &'static [u8] {
    fallback_faces()
        .iter()
        .find(|f| f.file == file)
        .and_then(|f| f.bytes)
        .expect("embedded face")
}

#[test]
fn embedded_bytes_hash_to_the_pinned_sha256() {
    for face in fallback_faces() {
        let bytes = face.bytes.expect("embed feature embeds every face");
        let digest = format!("{:x}", Sha256::digest(bytes));
        assert_eq!(digest, face.sha256, "{}", face.file);
    }
}

#[test]
fn every_face_parses_with_its_family_and_weight() {
    for face in fallback_faces() {
        let bytes = face.bytes.expect("embedded");
        let parsed = ttf_parser::Face::parse(bytes, 0).expect(face.file);
        let family = parsed
            .names()
            .into_iter()
            .filter(|n| n.name_id == ttf_parser::name_id::FAMILY && n.is_unicode())
            .find_map(|n| n.to_string())
            .expect("family name");
        assert_eq!(family, face.family, "{}", face.file);
        assert_eq!(parsed.weight().to_number(), face.weight, "{}", face.file);
        assert!(!parsed.is_italic(), "{}", face.file);
    }
}

#[test]
fn chain_order_is_arabic_hebrew_cjk_emoji() {
    let families: Vec<&str> = fallback_faces().iter().map(|f| f.family).collect();
    assert_eq!(
        families,
        [
            "Noto Sans Arabic",
            "Noto Sans Hebrew",
            "Noto Sans CJK SC",
            "Noto Emoji"
        ]
    );
}

#[test]
fn each_face_covers_its_script() {
    let cases = [
        ("NotoSansArabic-Regular.ttf", "عربى"),
        ("NotoSansHebrew-Regular.ttf", "עברית"),
        ("NotoSansCJKsc-Fallback.otf", "你好안녕こんにちは。"),
        ("NotoEmoji-Regular.ttf", "😀"),
    ];
    for (file, text) in cases {
        let face = ttf_parser::Face::parse(bytes_of(file), 0).expect(file);
        for ch in text.chars() {
            assert!(face.glyph_index(ch).is_some(), "{file} lacks {ch}");
        }
    }
}
