//! The CLI's native host and an in-memory host render every example to the
//! same bytes and diagnostics: the browser editor and `zenith render` share
//! one pipeline.

use std::path::{Path, PathBuf};

use zenith_cli::commands::render::{to_png_all_pages_options, to_svg_all_pages_with_dir_options};
use zenith_cli::commands::validate;
use zenith_pipeline::render::{render_png_pages, render_svg_pages};
use zenith_pipeline::{FsConfig, Host, MemFs, PolicyFlags, RenderOptions, validate_source};

const EXAMPLES: &str = "../examples";

/// Every file under `dir`, recursively.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut work = vec![dir.to_path_buf()];
    while let Some(d) = work.pop() {
        for entry in std::fs::read_dir(&d).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                work.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out
}

#[test]
fn native_and_memory_hosts_render_examples_identically() {
    let mut mem = MemFs::new();
    for path in files_under(Path::new(EXAMPLES)) {
        mem.insert(&path, std::fs::read(&path).expect("read"));
    }
    let config = FsConfig::new(&mem, None);
    let host = Host::new(&mem, &config);
    let dir = Some(Path::new(EXAMPLES));
    let flags = PolicyFlags::default();
    let opts = RenderOptions::new(&flags);

    let mut docs: Vec<PathBuf> = files_under(Path::new(EXAMPLES))
        .into_iter()
        .filter(|p| p.parent() == dir && p.extension().is_some_and(|e| e == "zen"))
        .collect();
    docs.sort();
    assert!(docs.len() >= 30, "{}", docs.len());
    for doc in &docs {
        let src = std::fs::read_to_string(doc).expect("read example");
        let name = doc.display();

        let native = to_png_all_pages_options(&src, dir, opts).expect("native png");
        let memory = render_png_pages(host, &src, dir, opts).expect("memory png");
        assert_eq!(native.pages, memory.pages, "{name}: PNG bytes");
        assert_eq!(
            native.diagnostics, memory.diagnostics,
            "{name}: PNG diagnostics"
        );

        let native = to_svg_all_pages_with_dir_options(&src, dir, opts).expect("native svg");
        let memory = render_svg_pages(host, &src, dir, opts).expect("memory svg");
        let svgs = |a: &zenith_pipeline::SvgPagesArtifact| -> Vec<Vec<u8>> {
            a.pages.iter().map(|p| p.svg.clone()).collect()
        };
        assert_eq!(svgs(&native), svgs(&memory), "{name}: SVG bytes");

        let native = validate::collect(&src, dir, &flags);
        let memory = validate_source(host, &src, dir, &flags);
        assert_eq!(native.diagnostics, memory.diagnostics, "{name}: validate");
        assert_eq!(native.exit_code, memory.exit_code, "{name}: exit code");
    }
}
