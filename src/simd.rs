/// SIMD-optimized operations for DWT and MCT.
///
/// Provides batch-processing variants of core transform operations that
/// are structured for better auto-vectorization and cache utilization.
/// Falls back to scalar code on unsupported platforms.

/// Check if SSE2 is available at runtime.
///
/// On x86_64, SSE2 is always present (it is part of the baseline ISA).
/// On other architectures, returns false.
pub fn has_sse2() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        true // x86_64 always has SSE2
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// Check if AVX2 is available at runtime.
///
/// Uses `is_x86_feature_detected!` on x86_64; returns false elsewhere.
pub fn has_avx2() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        is_x86_feature_detected!("avx2")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// Batch RCT forward transform: processes samples in chunks of 4
/// for better auto-vectorization.
///
/// Semantically identical to `mct::rct_forward`:
/// ```text
/// Y  = floor((R + 2G + B) / 4)
/// Cb = B - G
/// Cr = R - G
/// ```
///
/// The chunked loop structure helps the compiler emit SIMD instructions
/// on platforms that support them.
#[inline]
pub fn rct_forward_batch(c0: &mut [i32], c1: &mut [i32], c2: &mut [i32]) {
    let n = c0.len().min(c1.len()).min(c2.len());
    let chunks = n / 4;
    let remainder = n % 4;

    // Process 4 elements at a time — structured for auto-vectorization
    for chunk in 0..chunks {
        let base = chunk * 4;
        // Load
        let r0 = c0[base];
        let r1 = c0[base + 1];
        let r2 = c0[base + 2];
        let r3 = c0[base + 3];
        let g0 = c1[base];
        let g1 = c1[base + 1];
        let g2 = c1[base + 2];
        let g3 = c1[base + 3];
        let b0 = c2[base];
        let b1 = c2[base + 1];
        let b2 = c2[base + 2];
        let b3 = c2[base + 3];

        // Y = floor((R + 2*G + B) / 4)
        c0[base] = (r0 + 2 * g0 + b0) >> 2;
        c0[base + 1] = (r1 + 2 * g1 + b1) >> 2;
        c0[base + 2] = (r2 + 2 * g2 + b2) >> 2;
        c0[base + 3] = (r3 + 2 * g3 + b3) >> 2;

        // Cb = B - G
        c1[base] = b0 - g0;
        c1[base + 1] = b1 - g1;
        c1[base + 2] = b2 - g2;
        c1[base + 3] = b3 - g3;

        // Cr = R - G
        c2[base] = r0 - g0;
        c2[base + 1] = r1 - g1;
        c2[base + 2] = r2 - g2;
        c2[base + 3] = r3 - g3;
    }

    // Scalar tail
    let tail_start = chunks * 4;
    for i in 0..remainder {
        let idx = tail_start + i;
        let r = c0[idx];
        let g = c1[idx];
        let b = c2[idx];
        c0[idx] = (r + 2 * g + b) >> 2;
        c1[idx] = b - g;
        c2[idx] = r - g;
    }
}

/// Batch RCT inverse transform: processes samples in chunks of 4
/// for better auto-vectorization.
///
/// Semantically identical to `mct::rct_inverse`:
/// ```text
/// G = Y - floor((Cb + Cr) / 4)
/// R = Cr + G
/// B = Cb + G
/// ```
#[inline]
pub fn rct_inverse_batch(c0: &mut [i32], c1: &mut [i32], c2: &mut [i32]) {
    let n = c0.len().min(c1.len()).min(c2.len());
    let chunks = n / 4;
    let remainder = n % 4;

    for chunk in 0..chunks {
        let base = chunk * 4;
        let y0 = c0[base];
        let y1 = c0[base + 1];
        let y2 = c0[base + 2];
        let y3 = c0[base + 3];
        let cb0 = c1[base];
        let cb1 = c1[base + 1];
        let cb2 = c1[base + 2];
        let cb3 = c1[base + 3];
        let cr0 = c2[base];
        let cr1 = c2[base + 1];
        let cr2 = c2[base + 2];
        let cr3 = c2[base + 3];

        let g0 = y0 - ((cb0 + cr0) >> 2);
        let g1 = y1 - ((cb1 + cr1) >> 2);
        let g2 = y2 - ((cb2 + cr2) >> 2);
        let g3 = y3 - ((cb3 + cr3) >> 2);

        c0[base] = cr0 + g0;
        c0[base + 1] = cr1 + g1;
        c0[base + 2] = cr2 + g2;
        c0[base + 3] = cr3 + g3;

        c1[base] = g0;
        c1[base + 1] = g1;
        c1[base + 2] = g2;
        c1[base + 3] = g3;

        c2[base] = cb0 + g0;
        c2[base + 1] = cb1 + g1;
        c2[base + 2] = cb2 + g2;
        c2[base + 3] = cb3 + g3;
    }

    let tail_start = chunks * 4;
    for i in 0..remainder {
        let idx = tail_start + i;
        let y = c0[idx];
        let cb = c1[idx];
        let cr = c2[idx];
        let g = y - ((cb + cr) >> 2);
        c0[idx] = cr + g;
        c1[idx] = g;
        c2[idx] = cb + g;
    }
}

/// Optimized DWT 5/3 predict step.
///
/// Computes: `high[i] -= (even[i] + even[i+1]) >> 1` for `i in 0..high.len()`.
///
/// `even` must have length >= `high.len()` (and ideally `high.len() + 1`
/// for the last element; the caller must handle boundary extension before
/// calling this function — i.e., `even` should already include any
/// mirrored boundary sample).
///
/// Processes in chunks of 4 for auto-vectorization.
#[inline]
pub fn dwt53_predict_batch(even: &[i32], high: &mut [i32]) {
    let n = high.len();
    if n == 0 {
        return;
    }
    assert!(
        even.len() > n,
        "even must have at least high.len()+1 elements (got even.len()={}, high.len()={})",
        even.len(),
        n,
    );

    let chunks = n / 4;
    let remainder = n % 4;

    for chunk in 0..chunks {
        let base = chunk * 4;
        let e0 = even[base];
        let e1 = even[base + 1];
        let e2 = even[base + 2];
        let e3 = even[base + 3];
        let e4 = even[base + 4];

        high[base] -= (e0 + e1) >> 1;
        high[base + 1] -= (e1 + e2) >> 1;
        high[base + 2] -= (e2 + e3) >> 1;
        high[base + 3] -= (e3 + e4) >> 1;
    }

    let tail_start = chunks * 4;
    for i in 0..remainder {
        let idx = tail_start + i;
        high[idx] -= (even[idx] + even[idx + 1]) >> 1;
    }
}

/// Optimized DWT 5/3 update step.
///
/// Computes: `low[i] += (high[i-1] + high[i] + 2) >> 2` for `i in 0..low.len()`.
///
/// `high_ext` must have length >= `low.len() + 1` and be pre-extended so that
/// `high_ext[0]` corresponds to `high[-1]` (the mirrored boundary sample) and
/// `high_ext[i+1]` corresponds to `high[i]`.
///
/// Processes in chunks of 4 for auto-vectorization.
#[inline]
pub fn dwt53_update_batch(high_ext: &[i32], low: &mut [i32]) {
    let n = low.len();
    if n == 0 {
        return;
    }
    assert!(
        high_ext.len() > n,
        "high_ext must have at least low.len()+1 elements (got high_ext.len()={}, low.len()={})",
        high_ext.len(),
        n,
    );

    let chunks = n / 4;
    let remainder = n % 4;

    for chunk in 0..chunks {
        let base = chunk * 4;
        let h0 = high_ext[base];
        let h1 = high_ext[base + 1];
        let h2 = high_ext[base + 2];
        let h3 = high_ext[base + 3];
        let h4 = high_ext[base + 4];

        low[base] += (h0 + h1 + 2) >> 2;
        low[base + 1] += (h1 + h2 + 2) >> 2;
        low[base + 2] += (h2 + h3 + 2) >> 2;
        low[base + 3] += (h3 + h4 + 2) >> 2;
    }

    let tail_start = chunks * 4;
    for i in 0..remainder {
        let idx = tail_start + i;
        low[idx] += (high_ext[idx] + high_ext[idx + 1] + 2) >> 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_detection() {
        // Just verify the functions don't panic
        let _ = has_sse2();
        let _ = has_avx2();
    }

    #[test]
    fn rct_forward_batch_small() {
        let mut c0 = vec![100i32];
        let mut c1 = vec![150i32];
        let mut c2 = vec![200i32];
        rct_forward_batch(&mut c0, &mut c1, &mut c2);
        // Y = (100 + 300 + 200) / 4 = 150
        assert_eq!(c0[0], 150);
        // Cb = 200 - 150 = 50
        assert_eq!(c1[0], 50);
        // Cr = 100 - 150 = -50
        assert_eq!(c2[0], -50);
    }
}
