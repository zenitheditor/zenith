//! Every example renders and validates to the same bytes through `MemFs` as
//! through the disk host.

use std::path::Path;

use sha2::{Digest, Sha256};
use zenith_pipeline::render::{render_png_pages, render_scene_json, render_svg_pages};
use zenith_pipeline::{
    FsConfig, Host, PolicyFlags, RenderOptions, SourceFs, Validation, validate_source,
};

use super::disk::{DiskFs, EXAMPLES, example_documents, mem_copy_of};

/// One example's outputs, reduced to comparable text.
#[derive(Debug, PartialEq)]
struct Outputs {
    validate: String,
    pngs: String,
    svgs: String,
    scene: String,
}

fn outputs(fs: &dyn SourceFs, doc: &Path) -> Outputs {
    let config = FsConfig::new(fs, None);
    let host = Host::new(fs, &config);
    let src = std::fs::read_to_string(doc).expect("read example");
    let dir = Some(Path::new(EXAMPLES));
    let flags = PolicyFlags::default();
    let opts = RenderOptions::new(&flags);
    let Validation {
        diagnostics,
        files,
        exit_code,
    } = validate_source(host, &src, dir, &flags);
    let pngs = match render_png_pages(host, &src, dir, opts) {
        Ok(a) => format!("{:?} {:?} {:?}", hashes(&a.pages), a.sizes, a.diagnostics),
        Err(e) => format!("error {} {:?}", e.exit_code, e.diagnostics),
    };
    let svgs = match render_svg_pages(host, &src, dir, opts) {
        Ok(a) => {
            let svgs: Vec<Vec<u8>> = a.pages.into_iter().map(|p| p.svg).collect();
            format!("{:?} {:?}", hashes(&svgs), a.diagnostics)
        }
        Err(e) => format!("error {} {:?}", e.exit_code, e.diagnostics),
    };
    let scene = match render_scene_json(host, &src, dir, 1, opts) {
        Ok(a) => format!("{} {:?}", a.json, a.diagnostics),
        Err(e) => format!("error {} {:?}", e.exit_code, e.diagnostics),
    };
    Outputs {
        validate: format!("{exit_code} {diagnostics:?} {files:?}"),
        pngs,
        svgs,
        scene,
    }
}

/// The SHA-256 hex digest of each byte string.
fn hashes(items: &[Vec<u8>]) -> Vec<String> {
    items
        .iter()
        .map(|b| format!("{:x}", Sha256::digest(b)))
        .collect()
}

#[test]
fn every_example_matches_the_disk_host() {
    let mem = mem_copy_of(Path::new(EXAMPLES));
    let docs = example_documents();
    assert!(
        docs.len() >= 30,
        "expected the examples set, got {}",
        docs.len()
    );
    let mut rendered = 0;
    for doc in &docs {
        let on_disk = outputs(&DiskFs, doc);
        let in_memory = outputs(&mem, doc);
        assert_eq!(on_disk, in_memory, "{}", doc.display());
        if !on_disk.pngs.starts_with("error") {
            rendered += 1;
        }
    }
    assert_eq!(rendered, docs.len(), "every example renders");
}
