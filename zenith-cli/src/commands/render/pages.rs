//! Page compile helpers shared by the render entry points.
//!
//! Every entry point builds one [`DocumentPrep`] and one [`PageCompiler`] per
//! invocation and compiles its pages from them. Multi-page paths map pages on
//! a bounded thread pool and collect the results in page order.

use rayon::prelude::*;
use zenith_core::{BytesFontProvider, Document};
use zenith_scene::{CompileResult, PageCompiler, append_construction_overlay};

use super::entry::RenderEntryOptions;

/// Upper bound on page worker threads.
const MAX_PAGE_THREADS: usize = 8;

/// Compile `page_index` from `compiler`, then append the construction overlay
/// when `opts` asks for it.
///
/// The overlay reads the page from `doc`, the caller's parsed document.
pub(super) fn compile_for_render(
    doc: &Document,
    compiler: &PageCompiler<'_, BytesFontProvider>,
    page_index: usize,
    opts: RenderEntryOptions<'_>,
) -> CompileResult {
    let mut compile_result = compiler.compile_page(page_index);
    if opts.construction_overlay
        && let Some(page) = doc.body.pages.get(page_index)
    {
        append_construction_overlay(&mut compile_result.scene, page);
    }
    compile_result
}

/// Run `f` for every page index in `0..page_count` and return the results in
/// page order.
///
/// Pages run on a scoped pool of `min(available_parallelism, 8, page_count)`
/// threads. The result vector is indexed by page, so output never depends on
/// thread scheduling. One page, one available thread, or a pool that fails to
/// start runs the pages in sequence on this thread with the same result.
pub(super) fn map_pages<T, F>(page_count: usize, f: F) -> Vec<T>
where
    T: Send,
    F: Fn(usize) -> T + Sync + Send,
{
    let threads = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
        .min(MAX_PAGE_THREADS)
        .min(page_count);
    if threads <= 1 {
        return (0..page_count).map(f).collect();
    }
    match rayon::ThreadPoolBuilder::new().num_threads(threads).build() {
        Ok(pool) => pool.install(|| (0..page_count).into_par_iter().map(&f).collect()),
        Err(_) => (0..page_count).map(f).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_pages_keeps_page_order() {
        let out = map_pages(64, |i| i * 3);
        let expected: Vec<usize> = (0..64).map(|i| i * 3).collect();
        assert_eq!(out, expected);
    }

    #[test]
    fn map_pages_handles_zero_and_one_page() {
        assert!(map_pages(0, |i| i).is_empty());
        assert_eq!(map_pages(1, |i| i + 7), vec![7]);
    }
}
