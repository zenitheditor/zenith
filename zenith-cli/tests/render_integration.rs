//! Integration tests: read fixture files from the workspace root `examples/`
//! directory and render them to PNG, asserting valid, non-empty output and
//! byte-identical determinism across two back-to-back renders.

use zenith_cli::commands::render::to_png;

#[test]
fn rect_zen_renders_to_valid_png() {
    // `CARGO_MANIFEST_DIR` is the zenith-cli crate root.  `examples/rect.zen`
    // lives one level up in the workspace root.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let fixture = std::path::Path::new(manifest_dir)
        .join("..")
        .join("examples")
        .join("rect.zen");

    let src = std::fs::read_to_string(&fixture)
        .unwrap_or_else(|e| panic!("could not read {}: {}", fixture.display(), e));

    let png = to_png(&src)
        .unwrap_or_else(|e| panic!("render failed (exit {}): {}", e.exit_code, e.message));

    // Must be non-empty.
    assert!(!png.is_empty(), "PNG output must not be empty");

    // Must start with the PNG magic bytes.
    assert!(
        png.len() >= 8,
        "PNG must have at least 8 bytes; got {}",
        png.len()
    );
    assert_eq!(
        &png[0..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        "first 8 bytes must be the PNG signature"
    );

    // Determinism: two renders must be byte-identical.
    let png2 = to_png(&src)
        .unwrap_or_else(|e| panic!("second render failed (exit {}): {}", e.exit_code, e.message));
    assert_eq!(
        png, png2,
        "two renders of rect.zen must produce identical bytes"
    );
}

#[test]
fn line_zen_renders_to_valid_png() {
    // `examples/line.zen` lives one level up from the zenith-cli crate root.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let fixture = std::path::Path::new(manifest_dir)
        .join("..")
        .join("examples")
        .join("line.zen");

    let src = std::fs::read_to_string(&fixture)
        .unwrap_or_else(|e| panic!("could not read {}: {}", fixture.display(), e));

    let png = to_png(&src)
        .unwrap_or_else(|e| panic!("render failed (exit {}): {}", e.exit_code, e.message));

    // Must be non-empty.
    assert!(!png.is_empty(), "PNG output must not be empty");

    // Must start with the PNG magic bytes.
    assert!(
        png.len() >= 8,
        "PNG must have at least 8 bytes; got {}",
        png.len()
    );
    assert_eq!(
        &png[0..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        "first 8 bytes must be the PNG signature"
    );

    // Determinism: two renders must be byte-identical.
    let png2 = to_png(&src)
        .unwrap_or_else(|e| panic!("second render failed (exit {}): {}", e.exit_code, e.message));
    assert_eq!(
        png, png2,
        "two renders of line.zen must produce identical bytes"
    );
}

#[test]
fn ellipse_zen_renders_to_valid_png() {
    // `examples/ellipse.zen` lives one level up from the zenith-cli crate root.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let fixture = std::path::Path::new(manifest_dir)
        .join("..")
        .join("examples")
        .join("ellipse.zen");

    let src = std::fs::read_to_string(&fixture)
        .unwrap_or_else(|e| panic!("could not read {}: {}", fixture.display(), e));

    let png = to_png(&src)
        .unwrap_or_else(|e| panic!("render failed (exit {}): {}", e.exit_code, e.message));

    // Must be non-empty.
    assert!(!png.is_empty(), "PNG output must not be empty");

    // Must start with the PNG magic bytes.
    assert!(
        png.len() >= 8,
        "PNG must have at least 8 bytes; got {}",
        png.len()
    );
    assert_eq!(
        &png[0..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        "first 8 bytes must be the PNG signature"
    );

    // Determinism: two renders must be byte-identical.
    let png2 = to_png(&src)
        .unwrap_or_else(|e| panic!("second render failed (exit {}): {}", e.exit_code, e.message));
    assert_eq!(
        png, png2,
        "two renders of ellipse.zen must produce identical bytes"
    );
}
