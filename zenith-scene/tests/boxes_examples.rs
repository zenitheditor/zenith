//! `compile_page_with_boxes` over every `examples/*.zen`: one pass gives the
//! scene and diagnostics of `compile_page` and the boxes of
//! `compiled_boxes`, and every box is well formed.

use std::path::PathBuf;

use zenith_core::{KdlAdapter, KdlSource, default_provider};
use zenith_scene::{CompiledBox, DocumentPrep, PageCompiler};

fn examples() -> Vec<PathBuf> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../examples");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(root)
        .expect("examples directory")
        .map(|e| e.expect("directory entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "zen"))
        .collect();
    paths.sort();
    paths
}

/// The bounds of the drawn box.
fn corner_bounds(b: &CompiledBox) -> (f64, f64, f64, f64) {
    let c = b.corners();
    let xs = c.map(|p| p.0);
    let ys = c.map(|p| p.1);
    let min = |v: [f64; 4]| v.iter().copied().fold(f64::INFINITY, f64::min);
    let max = |v: [f64; 4]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (min(xs), min(ys), max(xs), max(ys))
}

#[test]
fn one_pass_matches_compile_page_and_compiled_boxes_on_every_example() {
    let fonts = default_provider();
    let mut pages = 0;
    for path in examples() {
        let src = std::fs::read(&path).expect("example reads");
        let doc = KdlAdapter.parse(&src).expect("example parses");
        let prep = DocumentPrep::new(&doc, None, None);
        let compiler = PageCompiler::new(&prep, &fonts);
        for index in 0..compiler.page_count() {
            pages += 1;
            let name = format!("{} page {index}", path.display());
            let (result, boxes) = compiler.compile_page_with_boxes(index, true);
            let plain = compiler.compile_page(index);
            assert_eq!(
                result.scene.to_json().expect("scene JSON"),
                plain.scene.to_json().expect("scene JSON"),
                "{name}"
            );
            assert_eq!(result.diagnostics, plain.diagnostics, "{name}");
            assert_eq!(boxes, compiler.compiled_boxes(index), "{name}");

            let mut ranks: Vec<usize> = boxes.values().map(|b| b.paint_order).collect();
            ranks.sort_unstable();
            ranks.dedup();
            assert_eq!(ranks.len(), boxes.len(), "{name}: paint ranks repeat");
            for (id, b) in &boxes {
                assert!(
                    b.command_index <= result.scene.commands.len(),
                    "{name} {id}"
                );
                let (x0, y0, x1, y1) = corner_bounds(b);
                assert!(
                    [x0, y0, x1, y1].iter().all(|v| v.is_finite()),
                    "{name} {id}"
                );
                // With no transform anywhere, the drawn box is `rect`.
                if b.world == zenith_scene::Affine2::IDENTITY && b.rotate.is_none() {
                    let r = b.rect;
                    let same = (x0 - r.x).abs() < 1e-6
                        && (y0 - r.y).abs() < 1e-6
                        && (x1 - (r.x + r.w)).abs() < 1e-6
                        && (y1 - (r.y + r.h)).abs() < 1e-6;
                    assert!(same, "{name} {id}: {b:?}");
                }
            }
        }
    }
    assert!(pages >= 35, "{pages} pages");
}
