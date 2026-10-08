//! Deterministic three-box Gaussian approximation over premultiplied RGBA8.
//!
//! Box sizes follow Ivan Kuchin's "Fastest Gaussian Blur" 3-box method
//! (<http://blog.ivank.net/fastest-gaussian-blur.html>). Each box is a
//! horizontal pass followed by a vertical pass. Edges extend the border pixel.
//! A pixel is `(sum + window / 2) / window` over its clamped window, in `u32`.
//! Output is byte-identical across runs and machines.

use tiny_skia::Pixmap;

/// Largest box window that divides by multiply-shift. Wider boxes use `/`.
///
/// Exactness proof: let `d` be the window and `m = ceil(2^32 / d)`, so
/// `m * d = 2^32 + e` with `0 <= e < d`. For `n = q * d + r` with `r < d`,
/// `n * m / 2^32 = q + (r + n * e / 2^32) / d`. The floor equals `q` when
/// `n * e < 2^32`. Here `n <= 255 * d + d / 2 < 256 * d` and `e < d`, so
/// `n * e < 256 * d^2`, which is at most `2^32` for `d <= 4096`. The test
/// `mul_shift_matches_division_for_every_window` checks every case exhaustively.
const MUL_SHIFT_MAX_WINDOW: u32 = 1023;

/// Reusable buffers for `gaussian_blur_premul`. Reuse avoids an allocation
/// per blur when several layers blur in a row.
#[derive(Debug, Default)]
pub(super) struct BlurScratch {
    /// Horizontal-pass output, one pixmap's worth of bytes.
    pixels: Vec<u8>,
    /// Vertical running sums, one per byte of a row.
    sums: Vec<u32>,
    /// `true` marks a horizontal-pass input row that is all zero.
    zero_rows: Vec<bool>,
}

/// True when `sigma` selects a real blur. NaN, infinite and non-positive values
/// leave the pixmap unchanged.
fn blur_active(sigma: f64) -> bool {
    sigma.is_finite() && sigma > 0.0
}

/// Box radius for a box of width `w`.
fn box_radius(w: u32) -> u32 {
    (w.max(1) - 1) / 2
}

/// Total horizontal (and vertical) reach of `gaussian_blur_premul` for `sigma`.
///
/// This is the sum of the three box radii. A pixel further than this from all
/// ink stays zero after the blur. Saturates instead of overflowing.
pub(super) fn blur_reach(sigma: f64) -> u32 {
    if !blur_active(sigma) {
        return 0;
    }
    boxes_for_gauss(sigma)
        .iter()
        .fold(0u32, |acc, &w| acc.saturating_add(box_radius(w)))
}

/// Compute the three box-blur sizes that approximate a Gaussian of the given
/// `sigma` with `n == 3` passes (Kuchin's method).
///
/// Returns three odd box widths; the box radius for a width `w` is `(w - 1) / 2`.
fn boxes_for_gauss(sigma: f64) -> [u32; 3] {
    const N: f64 = 3.0;
    if sigma <= 0.0 {
        return [1, 1, 1];
    }
    let w_ideal = ((12.0 * sigma * sigma / N) + 1.0).sqrt();
    let mut wl = w_ideal.floor() as i64;
    if wl % 2 == 0 {
        wl -= 1;
    }
    if wl < 1 {
        wl = 1;
    }
    let wu = wl + 2;
    let wl_f = wl as f64;
    let m_ideal =
        (12.0 * sigma * sigma - N * wl_f * wl_f - 4.0 * N * wl_f - 3.0 * N) / (-4.0 * wl_f - 4.0);
    let m = m_ideal.round() as i64;
    let mut out = [0u32; 3];
    for (i, slot) in out.iter_mut().enumerate() {
        let w = if (i as i64) < m { wl } else { wu };
        *slot = w.max(1) as u32;
    }
    out
}

/// Blur a premultiplied RGBA8 `Pixmap` in place with three box passes.
///
/// Premultiplied blur is correct for source-over compositing. `scratch` holds
/// the working buffers. Its prior contents never affect the result.
pub(super) fn gaussian_blur_premul(pm: &mut Pixmap, sigma: f64, scratch: &mut BlurScratch) {
    // NaN-safe: the `> 0.0` test is false for NaN.
    if !blur_active(sigma) {
        return;
    }
    let width = pm.width() as usize;
    let height = pm.height() as usize;
    if width == 0 || height == 0 {
        return;
    }
    let boxes = boxes_for_gauss(sigma);
    let data = pm.data_mut();
    let expected = width * height * 4;
    if data.len() != expected {
        return; // defensive: unexpected layout → leave untouched
    }
    scratch.pixels.resize(expected, 0);
    scratch.zero_rows.resize(height, false);
    for w in boxes {
        let radius = box_radius(w);
        if radius == 0 {
            continue; // a 1-wide box is identity
        }
        let pass = Pass {
            width,
            height,
            radius: radius as usize,
        };
        // `radius < 2^31`, so the window fits in `u32`.
        let window = 2 * radius + 1;
        let done = if window <= MUL_SHIFT_MAX_WINDOW {
            box_blur(data, scratch, pass, MulShift::new(window))
        } else {
            box_blur(data, scratch, pass, Divide::new(window))
        };
        if done.is_none() {
            return;
        }
    }
}

/// Dimensions and radius of one box pass.
#[derive(Debug, Clone, Copy)]
struct Pass {
    width: usize,
    height: usize,
    radius: usize,
}

/// Maps a window sum to its output byte: `((sum + window / 2) / window).min(255)`.
trait Quotient: Copy {
    fn byte(self, sum: u32) -> u8;
}

/// Exact quotient by multiply-shift, for windows up to `MUL_SHIFT_MAX_WINDOW`.
#[derive(Debug, Clone, Copy)]
struct MulShift {
    half: u32,
    mul: u64,
}

impl MulShift {
    /// `mul = ceil(2^32 / window)`. `window` is odd, so `2^32` is never a multiple.
    fn new(window: u32) -> Self {
        Self {
            half: window / 2,
            mul: u64::from(u32::MAX / window) + 1,
        }
    }
}

impl Quotient for MulShift {
    #[inline(always)]
    fn byte(self, sum: u32) -> u8 {
        let q = (u64::from(sum.wrapping_add(self.half)) * self.mul) >> 32;
        q.min(255) as u8
    }
}

/// Exact quotient by hardware division, for windows above `MUL_SHIFT_MAX_WINDOW`.
#[derive(Debug, Clone, Copy)]
struct Divide {
    half: u32,
    window: u32,
}

impl Divide {
    fn new(window: u32) -> Self {
        Self {
            half: window / 2,
            window: window.max(1),
        }
    }
}

impl Quotient for Divide {
    #[inline(always)]
    fn byte(self, sum: u32) -> u8 {
        (sum.wrapping_add(self.half) / self.window).min(255) as u8
    }
}

/// One horizontal pass (`data` → scratch) then one vertical pass (scratch → `data`).
fn box_blur<Q: Quotient>(
    data: &mut [u8],
    scratch: &mut BlurScratch,
    pass: Pass,
    q: Q,
) -> Option<()> {
    let BlurScratch {
        pixels,
        sums,
        zero_rows,
    } = scratch;
    box_blur_h(data, pixels, zero_rows, pass, q);
    box_blur_v(pixels, data, sums, zero_rows, pass, q)
}

/// Window members for a window centred at index 0 with edge extension.
///
/// Yields `(index, weight)`: the window `[-radius, radius]` clamped to
/// `[0, last]`. Weights are `u32` and wrap like the running sums.
fn window_weights(radius: usize, last: usize) -> impl Iterator<Item = (usize, u32)> {
    let head = core::iter::once((0, (radius as u32).wrapping_add(1)));
    let body = (1..=radius.min(last)).map(|k| (k, 1));
    let tail = (radius > last).then(|| (last, (radius - last) as u32));
    head.chain(body).chain(tail)
}

/// `acc[i] += weight * src[i]`, wrapping.
fn add_scaled(acc: &mut [u32], src: &[u8], weight: u32) {
    for (a, &v) in acc.iter_mut().zip(src) {
        *a = a.wrapping_add(u32::from(v).wrapping_mul(weight));
    }
}

/// Slide a four-channel sum: add `add`, then remove `sub`.
#[inline(always)]
fn slide(sum: &mut [u32; 4], add: &[u8], sub: &[u8]) {
    for ((s, &a), &b) in sum.iter_mut().zip(add).zip(sub) {
        *s = s.wrapping_add(u32::from(a)).wrapping_sub(u32::from(b));
    }
}

/// Write one output pixel from a four-channel sum.
#[inline(always)]
fn store<Q: Quotient>(out: &mut [u8], sum: &[u32; 4], q: Q) {
    for (o, &s) in out.iter_mut().zip(sum) {
        *o = q.byte(s);
    }
}

/// Horizontal box blur, row by row, all four channels at once.
///
/// Sets `zero_rows[y]` when source row `y` is all zero. Such a row blurs to
/// zero, because `window / 2 < window`.
fn box_blur_h<Q: Quotient>(src: &[u8], dst: &mut [u8], zero_rows: &mut [bool], pass: Pass, q: Q) {
    let stride = pass.width * 4;
    let rows = src
        .chunks_exact(stride)
        .zip(dst.chunks_exact_mut(stride))
        .zip(zero_rows.iter_mut());
    for ((src_row, dst_row), zero) in rows {
        *zero = src_row.iter().all(|&b| b == 0);
        if *zero {
            dst_row.fill(0);
        } else if pass.width <= pass.radius.saturating_mul(2)
            || blur_row_zoned(src_row, dst_row, pass.radius, q).is_none()
        {
            blur_row_clamped(src_row, dst_row, pass.radius, q);
        }
    }
}

/// Blur one row that is at least one window wide, in three zones.
///
/// Head: the removed pixel clamps to column 0. Middle: no clamps. Tail: the
/// added pixel clamps to the last column.
fn blur_row_zoned<Q: Quotient>(src: &[u8], dst: &mut [u8], radius: usize, q: Q) -> Option<()> {
    let r = radius;
    let w = src.len() / 4;
    let last = w.checked_sub(1)?;
    let mid_len = w.checked_sub(2 * r + 1)?;
    let first = src.get(..4)?;
    let end = src.get(last * 4..)?;
    let mut sum = [0u32; 4];
    for (k, weight) in window_weights(r, last) {
        add_scaled(&mut sum, src.get(k * 4..k * 4 + 4)?, weight);
    }
    let (head, rest) = dst.split_at_mut_checked(r * 4)?;
    let (mid, tail) = rest.split_at_mut_checked(mid_len * 4)?;

    let head_add = src.get((r + 1) * 4..(2 * r + 1) * 4)?;
    for (out, add) in head
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(head_add.as_chunks::<4>().0.iter())
    {
        store(out, &sum, q);
        slide(&mut sum, add, first);
    }

    let mid_add = src.get((2 * r + 1) * 4..)?;
    let mid_sub = src.get(..mid_len * 4)?;
    let mid_px = mid
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(mid_add.as_chunks::<4>().0.iter())
        .zip(mid_sub.as_chunks::<4>().0.iter());
    for ((out, add), sub) in mid_px {
        store(out, &sum, q);
        slide(&mut sum, add, sub);
    }

    let tail_sub = src.get(mid_len * 4..(w - r) * 4)?;
    for (out, sub) in tail
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(tail_sub.as_chunks::<4>().0.iter())
    {
        store(out, &sum, q);
        slide(&mut sum, end, sub);
    }
    Some(())
}

/// Blur one row with every index clamped. Handles rows narrower than a window.
fn blur_row_clamped<Q: Quotient>(src: &[u8], dst: &mut [u8], radius: usize, q: Q) {
    let last = (src.len() / 4).saturating_sub(1);
    let px = |k: usize| src.get(k * 4..k * 4 + 4).unwrap_or(&[]);
    let mut sum = [0u32; 4];
    for (k, weight) in window_weights(radius, last) {
        add_scaled(&mut sum, px(k), weight);
    }
    for (x, out) in dst.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        store(out, &sum, q);
        let add = px(x.saturating_add(radius).saturating_add(1).min(last));
        let sub = px(x.saturating_sub(radius));
        slide(&mut sum, add, sub);
    }
}

/// Vertical box blur, row-major, with one running sum per row byte.
///
/// `zero_rows[y]` set means source row `y` is all zero. Rows outside the
/// table count as non-zero. `live` is the window weight of rows not known to
/// be zero. At `live == 0` every sum is zero, so the output row is zero.
fn box_blur_v<Q: Quotient>(
    src: &[u8],
    dst: &mut [u8],
    sums: &mut Vec<u32>,
    zero_rows: &[bool],
    pass: Pass,
    q: Q,
) -> Option<()> {
    let r = pass.radius;
    let stride = pass.width * 4;
    let last = pass.height.checked_sub(1)?;
    let row = |k: usize| src.get(k * stride..(k + 1) * stride);
    let nonzero = |k: usize| !zero_rows.get(k).copied().unwrap_or(false);

    sums.clear();
    sums.resize(stride, 0);
    let mut live: u64 = 0;
    for (k, weight) in window_weights(r, last) {
        add_scaled(sums, row(k)?, weight);
        if nonzero(k) {
            live += u64::from(weight);
        }
    }

    for (y, out_row) in dst.chunks_exact_mut(stride).enumerate() {
        let add_y = y.saturating_add(r).saturating_add(1).min(last);
        let sub_y = y.saturating_sub(r);
        // Sliding changes the sums only when a non-zero row enters or leaves.
        let moves = y < last && add_y != sub_y && (nonzero(add_y) || nonzero(sub_y));
        if moves {
            let (add, sub) = (row(add_y)?, row(sub_y)?);
            if live == 0 {
                out_row.fill(0);
                for ((s, &a), &b) in sums.iter_mut().zip(add).zip(sub) {
                    *s = s.wrapping_add(u32::from(a)).wrapping_sub(u32::from(b));
                }
            } else {
                let cells = out_row.iter_mut().zip(sums.iter_mut()).zip(add).zip(sub);
                for (((o, s), &a), &b) in cells {
                    *o = q.byte(*s);
                    *s = s.wrapping_add(u32::from(a)).wrapping_sub(u32::from(b));
                }
            }
            live = live + u64::from(nonzero(add_y)) - u64::from(nonzero(sub_y));
        } else if live == 0 {
            out_row.fill(0);
        } else {
            for (o, &s) in out_row.iter_mut().zip(sums.iter()) {
                *o = q.byte(s);
            }
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The previous `gaussian_blur_premul`, kept verbatim as the byte reference.
    fn reference_gaussian_blur_premul(pm: &mut Pixmap, sigma: f64) {
        // Non-positive or non-finite sigma → no blur (NaN-safe: the `> 0.0` test is
        // false for NaN, so we return early).
        if !blur_active(sigma) {
            return;
        }
        let width = pm.width() as usize;
        let height = pm.height() as usize;
        if width == 0 || height == 0 {
            return;
        }
        let boxes = boxes_for_gauss(sigma);
        let data = pm.data_mut();
        let expected = width * height * 4;
        if data.len() != expected {
            return; // defensive: unexpected layout → leave untouched
        }
        let mut scratch = vec![0u8; expected];
        for w in boxes {
            let radius = box_radius(w) as usize;
            if radius == 0 {
                continue; // a 1-wide box is identity
            }
            // Horizontal: data → scratch.
            reference_box_blur_h(data, &mut scratch, width, height, radius);
            // Vertical: scratch → data.
            reference_box_blur_v(&scratch, data, width, height, radius);
        }
    }

    /// The previous `box_blur_h`, verbatim.
    fn reference_box_blur_h(
        src: &[u8],
        dst: &mut [u8],
        width: usize,
        height: usize,
        radius: usize,
    ) {
        let window = (2 * radius + 1) as u32;
        let last = width.saturating_sub(1);
        for y in 0..height {
            let row = y * width * 4;
            for c in 0..4 {
                // Initialize the running sum for the window CENTERED at x=0, i.e.
                // positions [-radius, radius] each clamped to [0, width-1] (edge
                // extension). Positions < 0 collapse onto column 0.
                let mut sum: u32 = 0;
                for i in 0..=(2 * radius) {
                    // Map loop index i∈[0,2r] to signed offset (i - radius), clamped.
                    let xx = (i.saturating_sub(radius)).min(last);
                    let v = src.get(row + xx * 4 + c).copied().unwrap_or(0);
                    sum += u32::from(v);
                }
                for x in 0..width {
                    if let Some(o) = dst.get_mut(row + x * 4 + c) {
                        *o = ((sum + window / 2) / window).min(255) as u8;
                    }
                    // Slide one column right: add pixel at x+radius+1 (clamped),
                    // drop the leftmost at x-radius (clamped to 0 via saturating_sub).
                    let add_x = (x + radius + 1).min(last);
                    let sub_x = x.saturating_sub(radius);
                    let add = src.get(row + add_x * 4 + c).copied().unwrap_or(0);
                    let sub = src.get(row + sub_x * 4 + c).copied().unwrap_or(0);
                    sum = sum + u32::from(add) - u32::from(sub);
                }
            }
        }
    }

    /// The previous `box_blur_v`, verbatim.
    fn reference_box_blur_v(
        src: &[u8],
        dst: &mut [u8],
        width: usize,
        height: usize,
        radius: usize,
    ) {
        let window = (2 * radius + 1) as u32;
        let stride = width * 4;
        let last = height.saturating_sub(1);
        for x in 0..width {
            let col = x * 4;
            for c in 0..4 {
                // Window centered at y=0 with edge extension (see box_blur_h).
                let mut sum: u32 = 0;
                for i in 0..=(2 * radius) {
                    let yy = (i.saturating_sub(radius)).min(last);
                    let v = src.get(col + yy * stride + c).copied().unwrap_or(0);
                    sum += u32::from(v);
                }
                for y in 0..height {
                    if let Some(o) = dst.get_mut(col + y * stride + c) {
                        *o = ((sum + window / 2) / window).min(255) as u8;
                    }
                    let add_y = (y + radius + 1).min(last);
                    let sub_y = y.saturating_sub(radius);
                    let add = src.get(col + add_y * stride + c).copied().unwrap_or(0);
                    let sub = src.get(col + sub_y * stride + c).copied().unwrap_or(0);
                    sum = sum + u32::from(add) - u32::from(sub);
                }
            }
        }
    }

    /// Fixed-seed xorshift32.
    struct Rng(u32);

    impl Rng {
        fn next(&mut self) -> u32 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            self.0 = x;
            x
        }
    }

    /// A premultiplied-valid pixmap (each channel <= alpha) mixing zero rows,
    /// sparse rows, faint rows and dense rows.
    fn random_pixmap(w: u32, h: u32, seed: u32) -> Pixmap {
        let mut pm = Pixmap::new(w, h).expect("alloc");
        let mut rng = Rng(seed.max(1));
        let stride = w as usize * 4;
        for row in pm.data_mut().chunks_exact_mut(stride) {
            let mode = rng.next() % 5;
            for px in row.as_chunks_mut::<4>().0 {
                let a = match mode {
                    0 => 0,
                    1 if !rng.next().is_multiple_of(7) => 0,
                    2 => rng.next() % 3,
                    _ => rng.next() % 256,
                };
                px[3] = a as u8;
                for c in px.iter_mut().take(3) {
                    *c = (rng.next() % (a + 1)) as u8;
                }
            }
        }
        pm
    }

    const SIZES: [(u32, u32); 16] = [
        (1, 1),
        (1, 9),
        (1, 120),
        (9, 1),
        (120, 1),
        (2, 2),
        (3, 50),
        (50, 3),
        (7, 81),
        (81, 7),
        (37, 23),
        (200, 3),
        (3, 200),
        (64, 48),
        (83, 83),
        (161, 5),
    ];

    /// One box pass of the new kernel against the reference H + V pass.
    fn assert_pass_matches(pm: &Pixmap, radius: usize, scratch: &mut BlurScratch) {
        let (w, h) = (pm.width() as usize, pm.height() as usize);
        let mut expected = pm.data().to_vec();
        let mut tmp = vec![0u8; expected.len()];
        reference_box_blur_h(&expected, &mut tmp, w, h, radius);
        reference_box_blur_v(&tmp, &mut expected, w, h, radius);

        let mut got = pm.data().to_vec();
        scratch.pixels.resize(got.len(), 0);
        scratch.zero_rows.resize(h, false);
        let pass = Pass {
            width: w,
            height: h,
            radius,
        };
        let window = (2 * radius + 1) as u32;
        let done = if window <= MUL_SHIFT_MAX_WINDOW {
            box_blur(&mut got, scratch, pass, MulShift::new(window))
        } else {
            box_blur(&mut got, scratch, pass, Divide::new(window))
        };
        assert!(done.is_some());
        assert!(got == expected, "box pass {w}x{h} radius {radius} differs");
    }

    #[test]
    fn box_pass_matches_reference_for_radii_1_to_40() {
        let mut scratch = BlurScratch::default();
        for (i, &(w, h)) in SIZES.iter().enumerate() {
            let pm = random_pixmap(w, h, 0x9E37_79B9 ^ i as u32);
            for radius in 1..=40 {
                assert_pass_matches(&pm, radius, &mut scratch);
            }
        }
    }

    #[test]
    fn box_pass_matches_reference_around_division_bound() {
        let mut scratch = BlurScratch::default();
        for &(w, h) in &[(1u32, 1u32), (3, 2), (5, 1100), (1100, 4)] {
            let pm = random_pixmap(w, h, w * 31 + h);
            for radius in [510, 511, 512, 513, 700] {
                assert_pass_matches(&pm, radius, &mut scratch);
            }
        }
    }

    #[test]
    fn gaussian_blur_matches_reference() {
        let sigmas = [
            0.3, 0.5, 0.8, 1.0, 1.3, 1.7, 2.0, 2.6, 3.3, 4.0, 5.0, 6.5, 8.0, 10.0, 12.5, 16.0,
            20.0, 25.0, 31.0, 40.0,
        ];
        // One scratch across every size and sigma: stale contents must not leak.
        let mut scratch = BlurScratch::default();
        for (i, &(w, h)) in SIZES.iter().enumerate() {
            let pm = random_pixmap(w, h, 0x1234_5678 ^ (i as u32 * 7919));
            for &sigma in &sigmas {
                let mut expected = pm.clone();
                reference_gaussian_blur_premul(&mut expected, sigma);
                let mut got = pm.clone();
                gaussian_blur_premul(&mut got, sigma, &mut scratch);
                assert!(
                    got.data() == expected.data(),
                    "gaussian {w}x{h} sigma {sigma} differs"
                );
            }
        }
    }

    #[test]
    fn inactive_sigma_leaves_pixmap_unchanged() {
        let pm = random_pixmap(13, 11, 77);
        let mut scratch = BlurScratch::default();
        for sigma in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let mut got = pm.clone();
            gaussian_blur_premul(&mut got, sigma, &mut scratch);
            assert_eq!(got.data(), pm.data());
        }
    }

    #[test]
    fn mul_shift_matches_division_for_every_window() {
        for window in (1..=MUL_SHIFT_MAX_WINDOW).step_by(2) {
            let fast = MulShift::new(window);
            let exact = Divide::new(window);
            for sum in 0..=255 * window {
                let want = ((sum + window / 2) / window).min(255) as u8;
                assert_eq!(fast.byte(sum), want, "window {window} sum {sum}");
                assert_eq!(exact.byte(sum), want, "window {window} sum {sum}");
            }
        }
    }

    #[test]
    fn blur_reach_sums_box_radii() {
        assert_eq!(blur_reach(0.0), 0);
        assert_eq!(blur_reach(f64::NAN), 0);
        // sigma 2.0 -> boxes [3, 3, 5] -> radii 1 + 1 + 2.
        assert_eq!(boxes_for_gauss(2.0), [3, 3, 5]);
        assert_eq!(blur_reach(2.0), 4);
    }
}
