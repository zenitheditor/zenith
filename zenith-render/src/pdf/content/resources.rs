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
    /// Emit all colors, shadings, and raster images in DeviceCMYK when `true`.
    pub(in crate::pdf) cmyk: bool,
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
    /// Active 2D affine transform stack (`[a, b, c, d, e, f]`) matching PDF
    /// `PushTransform` / `PushScaleTranslate` / `PushTransformMatrix` /
    /// `PopTransform` scopes. Empty means identity (no active transform).
    pub(in crate::pdf) ctm_stack: Vec<[f64; 6]>,
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

    /// Compose `local` (`[a, b, c, d, e, f]`) onto the active transform stack.
    pub(in crate::pdf) fn push_ctm(&mut self, local: [f64; 6]) {
        let next = match self.ctm_stack.last().copied() {
            Some(o) => [
                o[0] * local[0] + o[2] * local[1],
                o[1] * local[0] + o[3] * local[1],
                o[0] * local[2] + o[2] * local[3],
                o[1] * local[2] + o[3] * local[3],
                o[0] * local[4] + o[2] * local[5] + o[4],
                o[1] * local[4] + o[3] * local[5] + o[5],
            ],
            None => local,
        };
        self.ctm_stack.push(next);
    }

    /// Pop the most recently pushed transform scope.
    pub(in crate::pdf) fn pop_ctm(&mut self) {
        self.ctm_stack.pop();
    }

    /// Map a scene-space axis-aligned rectangle through the active transform
    /// stack, returning its page-space bounding box. When no transform is
    /// active, returns the input coordinates untouched (bit-identical).
    pub(in crate::pdf) fn transform_rect(
        &self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
    ) -> (f64, f64, f64, f64) {
        let Some(m) = self.ctm_stack.last().copied() else {
            return (x0, y0, x1, y1);
        };
        let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for (px, py) in corners {
            let tx = m[0] * px + m[2] * py + m[4];
            let ty = m[1] * px + m[3] * py + m[5];
            min_x = min_x.min(tx);
            min_y = min_y.min(ty);
            max_x = max_x.max(tx);
            max_y = max_y.max(ty);
        }
        (min_x, min_y, max_x, max_y)
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
