/// Phase 12.4: SIMD optimization tests

use justjp2::mct;
use justjp2::simd;

#[test]
fn has_sse2_check() {
    let result = simd::has_sse2();
    // On x86_64 (which CI and most dev machines are), SSE2 is always true.
    // On other architectures it may be false. Either way, no panic.
    #[cfg(target_arch = "x86_64")]
    assert!(result);
    #[cfg(not(target_arch = "x86_64"))]
    let _ = result; // just check it doesn't panic
}

#[test]
fn has_avx2_check() {
    // Just verify it doesn't panic; result depends on hardware.
    let _ = simd::has_avx2();
}

#[test]
fn rct_forward_batch_matches() {
    // Compare batch version against scalar mct::rct_forward
    let r_orig: Vec<i32> = (0..17).map(|i| i * 10 + 5).collect();
    let g_orig: Vec<i32> = (0..17).map(|i| i * 7 + 20).collect();
    let b_orig: Vec<i32> = (0..17).map(|i| i * 13 + 3).collect();

    // Scalar reference
    let mut r_ref = r_orig.clone();
    let mut g_ref = g_orig.clone();
    let mut b_ref = b_orig.clone();
    mct::rct_forward(&mut r_ref, &mut g_ref, &mut b_ref);

    // Batch version
    let mut r_batch = r_orig.clone();
    let mut g_batch = g_orig.clone();
    let mut b_batch = b_orig.clone();
    simd::rct_forward_batch(&mut r_batch, &mut g_batch, &mut b_batch);

    assert_eq!(r_batch, r_ref, "Y channel mismatch");
    assert_eq!(g_batch, g_ref, "Cb channel mismatch");
    assert_eq!(b_batch, b_ref, "Cr channel mismatch");
}

#[test]
fn rct_forward_batch_matches_large() {
    // Larger test with exact multiples of 4 and non-multiples
    for size in [0, 1, 3, 4, 7, 8, 15, 16, 100, 1024, 1025] {
        let r_orig: Vec<i32> = (0..size).map(|i| (i * 31 + 7) % 256).collect();
        let g_orig: Vec<i32> = (0..size).map(|i| (i * 17 + 13) % 256).collect();
        let b_orig: Vec<i32> = (0..size).map(|i| (i * 41 + 3) % 256).collect();

        let mut r_ref = r_orig.clone();
        let mut g_ref = g_orig.clone();
        let mut b_ref = b_orig.clone();
        mct::rct_forward(&mut r_ref, &mut g_ref, &mut b_ref);

        let mut r_batch = r_orig.clone();
        let mut g_batch = g_orig.clone();
        let mut b_batch = b_orig.clone();
        simd::rct_forward_batch(&mut r_batch, &mut g_batch, &mut b_batch);

        assert_eq!(r_batch, r_ref, "Y mismatch at size {size}");
        assert_eq!(g_batch, g_ref, "Cb mismatch at size {size}");
        assert_eq!(b_batch, b_ref, "Cr mismatch at size {size}");
    }
}

#[test]
fn rct_inverse_batch_matches() {
    // Forward then inverse should round-trip
    let r_orig: Vec<i32> = (0..17).map(|i| i * 10 + 5).collect();
    let g_orig: Vec<i32> = (0..17).map(|i| i * 7 + 20).collect();
    let b_orig: Vec<i32> = (0..17).map(|i| i * 13 + 3).collect();

    // Apply forward transform first
    let mut r_fwd = r_orig.clone();
    let mut g_fwd = g_orig.clone();
    let mut b_fwd = b_orig.clone();
    mct::rct_forward(&mut r_fwd, &mut g_fwd, &mut b_fwd);

    // Scalar inverse
    let mut r_ref = r_fwd.clone();
    let mut g_ref = g_fwd.clone();
    let mut b_ref = b_fwd.clone();
    mct::rct_inverse(&mut r_ref, &mut g_ref, &mut b_ref);

    // Batch inverse
    let mut r_batch = r_fwd.clone();
    let mut g_batch = g_fwd.clone();
    let mut b_batch = b_fwd.clone();
    simd::rct_inverse_batch(&mut r_batch, &mut g_batch, &mut b_batch);

    assert_eq!(r_batch, r_ref, "R channel mismatch after inverse");
    assert_eq!(g_batch, g_ref, "G channel mismatch after inverse");
    assert_eq!(b_batch, b_ref, "B channel mismatch after inverse");
}

#[test]
fn rct_roundtrip_batch() {
    // Forward (batch) then inverse (batch) should round-trip
    let r_orig: Vec<i32> = (0..100).map(|i| (i * 31) % 256).collect();
    let g_orig: Vec<i32> = (0..100).map(|i| (i * 17) % 256).collect();
    let b_orig: Vec<i32> = (0..100).map(|i| (i * 41) % 256).collect();

    let mut r = r_orig.clone();
    let mut g = g_orig.clone();
    let mut b = b_orig.clone();

    simd::rct_forward_batch(&mut r, &mut g, &mut b);
    simd::rct_inverse_batch(&mut r, &mut g, &mut b);

    assert_eq!(r, r_orig);
    assert_eq!(g, g_orig);
    assert_eq!(b, b_orig);
}

#[test]
fn dwt53_predict_batch_matches() {
    // even = [10, 20, 30, 40, 50, 60, 70, 80, 90]  (9 elements)
    // high = [1, 2, 3, 4, 5, 6, 7, 8]                (8 elements)
    // predict: high[i] -= (even[i] + even[i+1]) >> 1

    let even = vec![10, 20, 30, 40, 50, 60, 70, 80, 90];
    let high_orig: Vec<i32> = vec![1, 2, 3, 4, 5, 6, 7, 8];

    // Scalar reference
    let mut high_ref = high_orig.clone();
    for i in 0..high_ref.len() {
        high_ref[i] -= (even[i] + even[i + 1]) >> 1;
    }

    // Batch version
    let mut high_batch = high_orig.clone();
    simd::dwt53_predict_batch(&even, &mut high_batch);

    assert_eq!(high_batch, high_ref);
}

#[test]
fn dwt53_predict_batch_various_sizes() {
    for n_high in [0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 100] {
        let even: Vec<i32> = (0..=(n_high as i32)).map(|i| i * 5 + 3).collect();
        let high_orig: Vec<i32> = (0..n_high).map(|i| (i as i32) * 3 + 1).collect();

        let mut high_ref = high_orig.clone();
        for i in 0..n_high {
            high_ref[i] -= (even[i] + even[i + 1]) >> 1;
        }

        let mut high_batch = high_orig.clone();
        simd::dwt53_predict_batch(&even, &mut high_batch);

        assert_eq!(high_batch, high_ref, "predict mismatch at n_high={n_high}");
    }
}

#[test]
fn dwt53_update_batch_matches() {
    // high_ext = [h[-1], h[0], h[1], h[2], h[3], h[4]]  (pre-extended)
    // low = [l0, l1, l2, l3, l4]
    // update: low[i] += (high_ext[i] + high_ext[i+1] + 2) >> 2

    let high_ext = vec![5, 10, 15, 20, 25, 30];
    let low_orig = vec![100, 200, 300, 400, 500];

    // Scalar reference
    let mut low_ref = low_orig.clone();
    for i in 0..low_ref.len() {
        low_ref[i] += (high_ext[i] + high_ext[i + 1] + 2) >> 2;
    }

    // Batch version
    let mut low_batch = low_orig.clone();
    simd::dwt53_update_batch(&high_ext, &mut low_batch);

    assert_eq!(low_batch, low_ref);
}

#[test]
fn dwt53_update_batch_various_sizes() {
    for n_low in [0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 100] {
        let high_ext: Vec<i32> = (0..=(n_low as i32)).map(|i| i * 7 + 2).collect();
        let low_orig: Vec<i32> = (0..n_low).map(|i| (i as i32) * 11 + 50).collect();

        let mut low_ref = low_orig.clone();
        for i in 0..n_low {
            low_ref[i] += (high_ext[i] + high_ext[i + 1] + 2) >> 2;
        }

        let mut low_batch = low_orig.clone();
        simd::dwt53_update_batch(&high_ext, &mut low_batch);

        assert_eq!(low_batch, low_ref, "update mismatch at n_low={n_low}");
    }
}
