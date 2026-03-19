use justjp2::quantize::*;

#[test]
fn no_quantization_passthrough() {
    // With guard_bits=0, values should be unchanged
    let mut coeffs = vec![100, -200, 300, 0, -50];
    let original = coeffs.clone();
    no_quantize(&mut coeffs, 0);
    assert_eq!(coeffs, original);

    // With guard_bits=2, values should be right-shifted by 2
    let mut coeffs = vec![100, -200, 300, 0, -52];
    no_quantize(&mut coeffs, 2);
    assert_eq!(coeffs, vec![25, -50, 75, 0, -13]);
}

#[test]
fn scalar_quantize_dequantize() {
    // Forward then inverse quantization should approximately recover original values
    let original = vec![1000, -2000, 500, -750, 0, 3000];
    let mut coeffs = original.clone();

    let ss = StepSize {
        exponent: 10,
        mantissa: 512,
    };
    let guard_bits = 2;

    quantize_band(&mut coeffs, &ss, guard_bits);

    // After quantization, values should be smaller (divided by step)
    // Zero should remain zero
    assert_eq!(coeffs[4], 0);

    // Now dequantize
    dequantize_band(&mut coeffs, &ss, guard_bits);

    // Values should be approximately restored (not exact due to quantization loss)
    // But the signs should be preserved
    for i in 0..original.len() {
        if original[i] == 0 {
            assert_eq!(coeffs[i], 0);
        } else if original[i] > 0 {
            assert!(coeffs[i] > 0, "Sign should be preserved for positive values");
        } else {
            assert!(coeffs[i] < 0, "Sign should be preserved for negative values");
        }
    }
}

#[test]
fn stepsize_encode_decode() {
    // Test various exponent/mantissa combinations
    let test_cases = [
        (0u8, 0u16),
        (31, 2047),       // max values
        (15, 1024),
        (8, 256),
        (1, 1),
    ];

    for (exp, mant) in &test_cases {
        let ss = StepSize {
            exponent: *exp,
            mantissa: *mant,
        };
        let encoded = ss.to_u16();
        let decoded = StepSize::from_u16(encoded);
        assert_eq!(
            ss, decoded,
            "Roundtrip failed for exponent={}, mantissa={}",
            exp, mant
        );
    }
}

#[test]
fn calc_stepsizes_5levels() {
    // 5 resolution levels -> 3*5 - 2 = 13 bands
    // Band layout: LL, HL0, LH0, HH0, HL1, LH1, HH1, HL2, LH2, HH2, HL3, LH3, HH3
    let stepsizes = calc_stepsizes(5, 8, false);
    assert_eq!(stepsizes.len(), 13);

    // All step sizes should have valid exponent (5 bits) and mantissa (11 bits)
    for ss in &stepsizes {
        assert!(ss.exponent < 32, "Exponent should fit in 5 bits");
        assert!(ss.mantissa < 2048, "Mantissa should fit in 11 bits");
    }

    // For reversible (5/3)
    let stepsizes_rev = calc_stepsizes(5, 8, true);
    assert_eq!(stepsizes_rev.len(), 13);
}

#[test]
fn guard_bits_effect() {
    let mut coeffs_g0 = vec![1024, -512, 256];
    let mut coeffs_g2 = vec![1024, -512, 256];

    let ss = StepSize {
        exponent: 10,
        mantissa: 0,
    };

    quantize_band(&mut coeffs_g0, &ss, 0);
    quantize_band(&mut coeffs_g2, &ss, 2);

    // With more guard bits, the effective step size is smaller (exponent - guard_bits),
    // so quantized values should be larger (less division).
    // guard_bits=0: step = 2^10 = 1024, so 1024/1024 = 1
    // guard_bits=2: step = 2^8 = 256, so 1024/256 = 4
    assert_eq!(coeffs_g0[0], 1);
    assert_eq!(coeffs_g2[0], 4);
}

#[test]
fn zero_coefficient() {
    let mut coeffs = vec![0];
    let ss = StepSize {
        exponent: 10,
        mantissa: 512,
    };

    // Forward quantization
    quantize_band(&mut coeffs, &ss, 2);
    assert_eq!(coeffs[0], 0);

    // Inverse dequantization
    dequantize_band(&mut coeffs, &ss, 2);
    assert_eq!(coeffs[0], 0);

    // No quantization
    no_quantize(&mut coeffs, 3);
    assert_eq!(coeffs[0], 0);
}

#[test]
fn dwt_norms() {
    // Verify 5/3 norm table values match openjpeg
    assert!((dwt_getnorm(0, 0) - 1.000).abs() < 1e-6);
    assert!((dwt_getnorm(1, 0) - 1.500).abs() < 1e-6);
    assert!((dwt_getnorm(0, 1) - 1.038).abs() < 1e-6);
    assert!((dwt_getnorm(0, 3) - 0.7186).abs() < 1e-6);

    // Verify 9/7 norm table values match openjpeg
    assert!((dwt_getnorm_real(0, 0) - 1.000).abs() < 1e-6);
    assert!((dwt_getnorm_real(1, 0) - 1.965).abs() < 1e-6);
    assert!((dwt_getnorm_real(0, 1) - 2.022).abs() < 1e-6);
    assert!((dwt_getnorm_real(0, 3) - 2.080).abs() < 1e-6);

    // HL and LH should be identical for 5/3
    assert_eq!(dwt_getnorm(3, 1), dwt_getnorm(3, 2));

    // HL and LH should be identical for 9/7
    assert_eq!(dwt_getnorm_real(3, 1), dwt_getnorm_real(3, 2));

    // Level clamping should work (levels beyond table bounds)
    let _norm_high = dwt_getnorm(20, 0); // Should not panic
    let _norm_high = dwt_getnorm_real(20, 1); // Should not panic
}

#[test]
fn mct_norms() {
    // Reversible MCT norms
    assert!((mct_getnorm(0) - 1.732).abs() < 1e-6);
    assert!((mct_getnorm(1) - 0.8292).abs() < 1e-6);
    assert!((mct_getnorm(2) - 0.8292).abs() < 1e-6);

    // Irreversible MCT norms
    assert!((mct_getnorm_real(0) - 1.732).abs() < 1e-6);
    assert!((mct_getnorm_real(1) - 1.805).abs() < 1e-6);
    assert!((mct_getnorm_real(2) - 1.573).abs() < 1e-6);
}
