//! Page-resource accumulator ([`PageResources`], [`LinkAnnot`]), the resource
//! name prefixes, and the stack-buffer resource-name builder ([`ResName`],
//! [`name`]).

use std::collections::BTreeSet;

use crate::pdf::gradient::PdfGradient;
use crate::pdf::image::DecodedImage;

/// Page-level resources accumulated during [`translate`](crate::pdf::content::translate), keyed for
/// deduplication and emitted in a deterministic order by the document writer.
#[derive(Default)]
pub(in crate::pdf) struct PageResources {
    /// Distinct fill/stroke alpha values (< 255), in first-seen order.
    /// Each becomes one `/ExtGState` with `ca` + `CA`. Index = resource id.
    pub(in crate::pdf) alphas: Vec<u8>,
    /// Native gradient shadings, in first-seen (draw) order. Index = resource id.
    pub(in crate::pdf) gradients: Vec<PdfGradient>,
    /// Decoded image XObjects, in first-seen order. Index = resource id.
    pub(in crate::pdf) images: Vec<DecodedImage>,
    /// Document-level font resource indices this page's content references (it
    /// emitted selectable text in each), used to build the page `/Font` dict.
    pub(in crate::pdf) font_indices: BTreeSet<usize>,
    /// Clickable link annotations collected from selectable linked glyph runs,
    /// in scene coordinates (top-left origin, y-down). Materialized as `/Link`
    /// annotations by the document writer.
    pub(in crate::pdf) links: Vec<LinkAnnot>,
}

/// A clickable link rectangle in scene coordinates (top-left origin, y-down) plus
/// its target URL, accumulated from a selectable `DrawGlyphRun` carrying a `link`.
pub(in crate::pdf) struct LinkAnnot {
    /// Left edge in scene px.
    pub(in crate::pdf) x0: f64,
    /// Top edge in scene px.
    pub(in crate::pdf) y0: f64,
    /// Right edge in scene px.
    pub(in crate::pdf) x1: f64,
    /// Bottom edge in scene px.
    pub(in crate::pdf) y1: f64,
    /// Target URL.
    pub(in crate::pdf) url: String,
}

impl PageResources {
    /// Intern an alpha byte, returning its stable `ExtGState` resource index.
    pub(in crate::pdf) fn intern_alpha(&mut self, a: u8) -> usize {
        if let Some(index) = self.alphas.iter().position(|alpha| *alpha == a) {
            return index;
        }
        let index = self.alphas.len();
        self.alphas.push(a);
        index
    }
}

/// The resource-name prefixes. Names are `<prefix><index>`, e.g. `ga2`, `sh0`,
/// `im1` — ASCII only, deterministic.
pub(in crate::pdf) const ALPHA_PREFIX: &str = "ga";
pub(in crate::pdf) const SHADING_PREFIX: &str = "sh";
pub(in crate::pdf) const IMAGE_PREFIX: &str = "im";
pub(in crate::pdf) const FONT_PREFIX: &str = "f";

/// A small owned resource-name buffer (`<prefix><index>`), kept on the stack to
/// avoid per-call heap churn while satisfying `pdf_writer::Name`'s borrow.
pub(in crate::pdf) struct ResName {
    buf: [u8; 24],
    len: usize,
}

impl ResName {
    pub(in crate::pdf) fn as_name(&self) -> pdf_writer::Name<'_> {
        pdf_writer::Name(&self.buf[..self.len])
    }
}

/// Build a deterministic ASCII resource name `<prefix><index>`.
pub(in crate::pdf) fn name(prefix: &str, index: usize) -> ResName {
    use std::io::Write;
    let mut buf = [0u8; 24];
    let mut cursor = std::io::Cursor::new(&mut buf[..]);
    // prefix is a short ASCII literal and index is a usize; the 24-byte buffer
    // is always large enough, so the writes cannot fail. If they ever did, the
    // name would be truncated to `cursor.position()` bytes — still valid ASCII.
    let _ = write!(cursor, "{prefix}{index}");
    let len = cursor.position() as usize;
    ResName { buf, len }
}
