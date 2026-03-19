use justjp2::dwt::*;

// ---------------------------------------------------------------------------
// 1D 5/3 tests
// ---------------------------------------------------------------------------

#[test]
fn dwt53_encode_decode_4() {
    let original = vec![10, 20, 30, 40];
    let mut data = original.clone();
    dwt53_forward_1d(&mut data);
    // Verify it's actually transformed
    assert_ne!(data, original);
    dwt53_inverse_1d(&mut data);
    assert_eq!(data, original);
}

#[test]
fn dwt53_encode_decode_8() {
    let original = vec![1, 4, 9, 16, 25, 36, 49, 64];
    let mut data = original.clone();
    dwt53_forward_1d(&mut data);
    assert_ne!(data, original);
    dwt53_inverse_1d(&mut data);
    assert_eq!(data, original);
}

#[test]
fn dwt53_encode_decode_odd() {
    let original = vec![5, 10, 15, 20, 25];
    let mut data = original.clone();
    dwt53_forward_1d(&mut data);
    dwt53_inverse_1d(&mut data);
    assert_eq!(data, original);
}

#[test]
fn dwt53_single_sample() {
    let mut data = vec![42];
    dwt53_forward_1d(&mut data);
    assert_eq!(data, vec![42]);
    dwt53_inverse_1d(&mut data);
    assert_eq!(data, vec![42]);
}

#[test]
fn dwt53_two_samples() {
    let original = vec![100, 200];
    let mut data = original.clone();
    dwt53_forward_1d(&mut data);
    dwt53_inverse_1d(&mut data);
    assert_eq!(data, original);
}

// ---------------------------------------------------------------------------
// 1D 9/7 tests
// ---------------------------------------------------------------------------

#[test]
fn dwt97_encode_decode_8() {
    let original: Vec<f64> = vec![1.0, 4.0, 9.0, 16.0, 25.0, 36.0, 49.0, 64.0];
    let mut data = original.clone();
    dwt97_forward_1d(&mut data);
    dwt97_inverse_1d(&mut data);
    for (a, b) in data.iter().zip(original.iter()) {
        assert!(
            (a - b).abs() < 1e-10,
            "mismatch: got {a}, expected {b}"
        );
    }
}

#[test]
fn dwt97_encode_decode_odd() {
    let original: Vec<f64> = vec![5.0, 10.0, 15.0, 20.0, 25.0];
    let mut data = original.clone();
    dwt97_forward_1d(&mut data);
    dwt97_inverse_1d(&mut data);
    for (a, b) in data.iter().zip(original.iter()) {
        assert!(
            (a - b).abs() < 1e-10,
            "mismatch: got {a}, expected {b}"
        );
    }
}

#[test]
fn dwt97_precision() {
    // Verify forward+inverse error is extremely small
    let original: Vec<f64> = (0..16).map(|i| (i as f64) * 3.7 + 0.5).collect();
    let mut data = original.clone();
    dwt97_forward_1d(&mut data);
    dwt97_inverse_1d(&mut data);
    let max_err: f64 = data
        .iter()
        .zip(original.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f64, f64::max);
    assert!(max_err < 1e-6, "max error {max_err} exceeds 1e-6");
}

// ---------------------------------------------------------------------------
// 2D 5/3 tests
// ---------------------------------------------------------------------------

#[test]
fn dwt53_2d_4x4_roundtrip() {
    let original: Vec<i32> = (1..=16).collect();
    let mut data = original.clone();
    dwt53_forward_2d(&mut data, 4, 4, 1);
    assert_ne!(data, original);
    dwt53_inverse_2d(&mut data, 4, 4, 1);
    assert_eq!(data, original);
}

#[test]
fn dwt53_2d_8x8_roundtrip() {
    let original: Vec<i32> = (0..64).map(|i| i * 3 + 7).collect();
    let mut data = original.clone();
    dwt53_forward_2d(&mut data, 8, 8, 1);
    dwt53_inverse_2d(&mut data, 8, 8, 1);
    assert_eq!(data, original);
}

#[test]
fn dwt53_2d_non_square() {
    // 8 columns x 4 rows
    let original: Vec<i32> = (0..32).map(|i| i * 5 - 10).collect();
    let mut data = original.clone();
    dwt53_forward_2d(&mut data, 8, 4, 1);
    dwt53_inverse_2d(&mut data, 8, 4, 1);
    assert_eq!(data, original);
}

#[test]
fn dwt53_2d_multi_level() {
    // 8x8 with 3 levels of decomposition
    let original: Vec<i32> = (0..64).map(|i| (i * 7 + 3) % 256).collect();
    let mut data = original.clone();
    dwt53_forward_2d(&mut data, 8, 8, 3);
    dwt53_inverse_2d(&mut data, 8, 8, 3);
    assert_eq!(data, original);
}

#[test]
fn dwt53_2d_single_level() {
    let original: Vec<i32> = (0..16).collect();
    let mut data = original.clone();
    dwt53_forward_2d(&mut data, 4, 4, 1);
    dwt53_inverse_2d(&mut data, 4, 4, 1);
    assert_eq!(data, original);
}

// ---------------------------------------------------------------------------
// 2D 9/7 tests
// ---------------------------------------------------------------------------

#[test]
fn dwt97_2d_8x8_roundtrip() {
    let original: Vec<f64> = (0..64).map(|i| i as f64 * 2.5).collect();
    let mut data = original.clone();
    dwt97_forward_2d(&mut data, 8, 8, 1);
    dwt97_inverse_2d(&mut data, 8, 8, 1);
    let max_err: f64 = data
        .iter()
        .zip(original.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f64, f64::max);
    assert!(max_err < 1e-8, "max error {max_err} exceeds 1e-8");
}

#[test]
fn dwt97_2d_multi_level() {
    let original: Vec<f64> = (0..64).map(|i| i as f64 * 1.3 + 0.7).collect();
    let mut data = original.clone();
    dwt97_forward_2d(&mut data, 8, 8, 3);
    dwt97_inverse_2d(&mut data, 8, 8, 3);
    let max_err: f64 = data
        .iter()
        .zip(original.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f64, f64::max);
    assert!(max_err < 1e-6, "max error {max_err} exceeds 1e-6");
}

// ---------------------------------------------------------------------------
// Additional edge cases for lossless guarantee
// ---------------------------------------------------------------------------

#[test]
fn dwt53_roundtrip_all_zeros() {
    let original = vec![0i32; 16];
    let mut data = original.clone();
    dwt53_forward_1d(&mut data);
    dwt53_inverse_1d(&mut data);
    assert_eq!(data, original);
}

#[test]
fn dwt53_roundtrip_negative_values() {
    let original = vec![-100, 50, -25, 75, -10, 30, -60, 90];
    let mut data = original.clone();
    dwt53_forward_1d(&mut data);
    dwt53_inverse_1d(&mut data);
    assert_eq!(data, original);
}

#[test]
fn dwt53_roundtrip_length_3() {
    let original = vec![10, 20, 30];
    let mut data = original.clone();
    dwt53_forward_1d(&mut data);
    dwt53_inverse_1d(&mut data);
    assert_eq!(data, original);
}

#[test]
fn dwt53_2d_multi_level_non_power_of_two() {
    // 6x6 with 2 levels
    let w = 6;
    let h = 6;
    let original: Vec<i32> = (0..(w * h) as i32).collect();
    let mut data = original.clone();
    dwt53_forward_2d(&mut data, w, h, 2);
    dwt53_inverse_2d(&mut data, w, h, 2);
    assert_eq!(data, original);
}
