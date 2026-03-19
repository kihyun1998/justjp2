/// Discrete Wavelet Transform for JPEG 2000
///
/// Implements both the 5/3 reversible (Le Gall) transform for lossless coding
/// and the 9/7 irreversible (CDF 9/7) transform for lossy coding.

// ---------------------------------------------------------------------------
// 5/3 Reversible (Le Gall) — integer, lossless
// ---------------------------------------------------------------------------

/// Mirror-extend index into range [0, len).
#[inline]
fn mirror(mut i: isize, len: usize) -> usize {
    let n = len as isize;
    if n == 1 {
        return 0;
    }
    // Reflect into [0, 2*(n-1))
    if i < 0 {
        i = -i;
    }
    let period = 2 * (n - 1);
    i %= period;
    if i >= n {
        i = period - i;
    }
    i as usize
}

/// 1D forward 5/3 DWT (analysis).
///
/// Input:  `data[0..N]` — the signal.
/// Output: `data[0..half_len]` = low (L), `data[half_len..N]` = high (H), in-place.
///
/// Length 1: passthrough. Length 2: simple case.
pub fn dwt53_forward_1d(data: &mut [i32]) {
    let n = data.len();
    if n <= 1 {
        return;
    }

    // Number of low-pass (even) and high-pass (odd) samples
    let n_low = (n + 1) / 2;
    let n_high = n / 2;

    // Work on a copy so we can deinterleave
    let src = data.to_vec();

    // Even indices (low) and odd indices (high)
    let mut low: Vec<i32> = (0..n_low).map(|i| src[2 * i]).collect();
    let mut high: Vec<i32> = (0..n_high).map(|i| src[2 * i + 1]).collect();

    // Predict step: high[i] -= floor((even[i] + even[i+1]) / 2)
    for i in 0..n_high {
        let left = low[i];
        let right_idx = i + 1;
        let right = if right_idx < n_low {
            low[right_idx]
        } else {
            // Mirror: for the last high sample when n is even, mirror back
            low[mirror((right_idx) as isize, n_low)]
        };
        high[i] -= (left + right) >> 1;
    }

    // Update step: low[i] += floor((high[i-1] + high[i] + 2) / 4)
    for i in 0..n_low {
        let left = if i == 0 {
            high[mirror(-1_isize, n_high.max(1))]
        } else {
            high[i - 1]
        };
        let right = if i < n_high {
            high[i]
        } else {
            // Mirror
            high[mirror(i as isize, n_high.max(1))]
        };
        low[i] += (left + right + 2) >> 2;
    }

    // Pack [low | high]
    data[..n_low].copy_from_slice(&low);
    data[n_low..].copy_from_slice(&high);
}

/// 1D inverse 5/3 DWT (synthesis).
///
/// Input:  `data[0..half_len]` = low, `data[half_len..N]` = high.
/// Output: `data[0..N]` — reconstructed signal, in-place.
pub fn dwt53_inverse_1d(data: &mut [i32]) {
    let n = data.len();
    if n <= 1 {
        return;
    }

    let n_low = (n + 1) / 2;
    let n_high = n / 2;

    let mut low = data[..n_low].to_vec();
    let mut high = data[n_low..].to_vec();

    // Undo update: low[i] -= floor((high[i-1] + high[i] + 2) / 4)
    for i in 0..n_low {
        let left = if i == 0 {
            high[mirror(-1_isize, n_high.max(1))]
        } else {
            high[i - 1]
        };
        let right = if i < n_high {
            high[i]
        } else {
            high[mirror(i as isize, n_high.max(1))]
        };
        low[i] -= (left + right + 2) >> 2;
    }

    // Undo predict: high[i] += floor((even[i] + even[i+1]) / 2)
    for i in 0..n_high {
        let left = low[i];
        let right_idx = i + 1;
        let right = if right_idx < n_low {
            low[right_idx]
        } else {
            low[mirror(right_idx as isize, n_low)]
        };
        high[i] += (left + right) >> 1;
    }

    // Interleave back
    for i in 0..n_low {
        data[2 * i] = low[i];
    }
    for i in 0..n_high {
        data[2 * i + 1] = high[i];
    }
}

/// 2D forward 5/3 DWT with multi-level support.
///
/// Data is stored in row-major order with the given `width` and `height`.
/// Each level transforms the current LL subband; the first level operates on the full image.
pub fn dwt53_forward_2d(data: &mut [i32], width: usize, height: usize, levels: usize) {
    let mut w = width;
    let mut h = height;
    for _ in 0..levels {
        if w <= 1 && h <= 1 {
            break;
        }
        // Rows
        for row in 0..h {
            let mut row_buf: Vec<i32> = (0..w).map(|c| data[row * width + c]).collect();
            dwt53_forward_1d(&mut row_buf);
            for c in 0..w {
                data[row * width + c] = row_buf[c];
            }
        }
        // Columns
        for col in 0..w {
            let mut col_buf: Vec<i32> = (0..h).map(|r| data[r * width + col]).collect();
            dwt53_forward_1d(&mut col_buf);
            for r in 0..h {
                data[r * width + col] = col_buf[r];
            }
        }
        w = (w + 1) / 2;
        h = (h + 1) / 2;
    }
}

/// 2D inverse 5/3 DWT with multi-level support.
pub fn dwt53_inverse_2d(data: &mut [i32], width: usize, height: usize, levels: usize) {
    // Collect subband sizes for each level
    let mut sizes: Vec<(usize, usize)> = Vec::new();
    let mut w = width;
    let mut h = height;
    for _ in 0..levels {
        if w <= 1 && h <= 1 {
            break;
        }
        sizes.push((w, h));
        w = (w + 1) / 2;
        h = (h + 1) / 2;
    }

    // Inverse in reverse order
    for &(w, h) in sizes.iter().rev() {
        // Columns
        for col in 0..w {
            let mut col_buf: Vec<i32> = (0..h).map(|r| data[r * width + col]).collect();
            dwt53_inverse_1d(&mut col_buf);
            for r in 0..h {
                data[r * width + col] = col_buf[r];
            }
        }
        // Rows
        for row in 0..h {
            let mut row_buf: Vec<i32> = (0..w).map(|c| data[row * width + c]).collect();
            dwt53_inverse_1d(&mut row_buf);
            for c in 0..w {
                data[row * width + c] = row_buf[c];
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 9/7 Irreversible (CDF 9/7) — float
// ---------------------------------------------------------------------------

const ALPHA: f64 = -1.586134342;
const BETA: f64 = -0.052980118;
const GAMMA: f64 = 0.882911075;
const DELTA: f64 = 0.443506852;
const K: f64 = 1.230174105;
const INV_K: f64 = 1.0 / 1.230174105;

/// Mirror-extend for f64 arrays.
#[inline]
fn mirror_f(i: isize, len: usize) -> usize {
    mirror(i, len)
}

/// 1D forward 9/7 DWT (analysis).
pub fn dwt97_forward_1d(data: &mut [f64]) {
    let n = data.len();
    if n <= 1 {
        return;
    }

    let n_low = (n + 1) / 2;
    let n_high = n / 2;

    let src = data.to_vec();
    let mut low: Vec<f64> = (0..n_low).map(|i| src[2 * i]).collect();
    let mut high: Vec<f64> = (0..n_high).map(|i| src[2 * i + 1]).collect();

    // Step 1: ALPHA predict
    for i in 0..n_high {
        let left = low[i];
        let right = low[mirror_f(i as isize + 1, n_low)];
        high[i] += ALPHA * (left + right);
    }

    // Step 2: BETA update
    for i in 0..n_low {
        let left = high[mirror_f(i as isize - 1, n_high.max(1))];
        let right = high[mirror_f(i as isize, n_high.max(1))];
        low[i] += BETA * (left + right);
    }

    // Step 3: GAMMA predict
    for i in 0..n_high {
        let left = low[i];
        let right = low[mirror_f(i as isize + 1, n_low)];
        high[i] += GAMMA * (left + right);
    }

    // Step 4: DELTA update
    for i in 0..n_low {
        let left = high[mirror_f(i as isize - 1, n_high.max(1))];
        let right = high[mirror_f(i as isize, n_high.max(1))];
        low[i] += DELTA * (left + right);
    }

    // Scale
    for v in low.iter_mut() {
        *v *= INV_K;
    }
    for v in high.iter_mut() {
        *v *= K;
    }

    data[..n_low].copy_from_slice(&low);
    data[n_low..].copy_from_slice(&high);
}

/// 1D inverse 9/7 DWT (synthesis).
pub fn dwt97_inverse_1d(data: &mut [f64]) {
    let n = data.len();
    if n <= 1 {
        return;
    }

    let n_low = (n + 1) / 2;
    let n_high = n / 2;

    let mut low = data[..n_low].to_vec();
    let mut high = data[n_low..].to_vec();

    // Undo scale
    for v in low.iter_mut() {
        *v *= K;
    }
    for v in high.iter_mut() {
        *v *= INV_K;
    }

    // Undo step 4: DELTA update
    for i in 0..n_low {
        let left = high[mirror_f(i as isize - 1, n_high.max(1))];
        let right = high[mirror_f(i as isize, n_high.max(1))];
        low[i] -= DELTA * (left + right);
    }

    // Undo step 3: GAMMA predict
    for i in 0..n_high {
        let left = low[i];
        let right = low[mirror_f(i as isize + 1, n_low)];
        high[i] -= GAMMA * (left + right);
    }

    // Undo step 2: BETA update
    for i in 0..n_low {
        let left = high[mirror_f(i as isize - 1, n_high.max(1))];
        let right = high[mirror_f(i as isize, n_high.max(1))];
        low[i] -= BETA * (left + right);
    }

    // Undo step 1: ALPHA predict
    for i in 0..n_high {
        let left = low[i];
        let right = low[mirror_f(i as isize + 1, n_low)];
        high[i] -= ALPHA * (left + right);
    }

    // Interleave
    for i in 0..n_low {
        data[2 * i] = low[i];
    }
    for i in 0..n_high {
        data[2 * i + 1] = high[i];
    }
}

/// 2D forward 9/7 DWT with multi-level support.
pub fn dwt97_forward_2d(data: &mut [f64], width: usize, height: usize, levels: usize) {
    let mut w = width;
    let mut h = height;
    for _ in 0..levels {
        if w <= 1 && h <= 1 {
            break;
        }
        // Rows
        for row in 0..h {
            let mut row_buf: Vec<f64> = (0..w).map(|c| data[row * width + c]).collect();
            dwt97_forward_1d(&mut row_buf);
            for c in 0..w {
                data[row * width + c] = row_buf[c];
            }
        }
        // Columns
        for col in 0..w {
            let mut col_buf: Vec<f64> = (0..h).map(|r| data[r * width + col]).collect();
            dwt97_forward_1d(&mut col_buf);
            for r in 0..h {
                data[r * width + col] = col_buf[r];
            }
        }
        w = (w + 1) / 2;
        h = (h + 1) / 2;
    }
}

/// 2D inverse 9/7 DWT with multi-level support.
pub fn dwt97_inverse_2d(data: &mut [f64], width: usize, height: usize, levels: usize) {
    let mut sizes: Vec<(usize, usize)> = Vec::new();
    let mut w = width;
    let mut h = height;
    for _ in 0..levels {
        if w <= 1 && h <= 1 {
            break;
        }
        sizes.push((w, h));
        w = (w + 1) / 2;
        h = (h + 1) / 2;
    }

    for &(w, h) in sizes.iter().rev() {
        // Columns
        for col in 0..w {
            let mut col_buf: Vec<f64> = (0..h).map(|r| data[r * width + col]).collect();
            dwt97_inverse_1d(&mut col_buf);
            for r in 0..h {
                data[r * width + col] = col_buf[r];
            }
        }
        // Rows
        for row in 0..h {
            let mut row_buf: Vec<f64> = (0..w).map(|c| data[row * width + c]).collect();
            dwt97_inverse_1d(&mut row_buf);
            for c in 0..w {
                data[row * width + c] = row_buf[c];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_basic() {
        assert_eq!(mirror(-1, 4), 1);
        assert_eq!(mirror(4, 4), 2);
        assert_eq!(mirror(0, 1), 0);
        assert_eq!(mirror(3, 4), 3);
    }

    #[test]
    fn dwt53_roundtrip_simple() {
        let original = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let mut data = original.clone();
        dwt53_forward_1d(&mut data);
        dwt53_inverse_1d(&mut data);
        assert_eq!(data, original);
    }
}
