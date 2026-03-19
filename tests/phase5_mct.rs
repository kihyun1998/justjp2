use justjp2::mct::*;

#[test]
fn rct_encode_white() {
    let mut c0 = vec![255i32]; // R
    let mut c1 = vec![255i32]; // G
    let mut c2 = vec![255i32]; // B
    rct_forward(&mut c0, &mut c1, &mut c2);
    // Y = floor((255 + 510 + 255) / 4) = floor(1020/4) = 255
    assert_eq!(c0[0], 255, "Y for white");
    // Cb = B - G = 0
    assert_eq!(c1[0], 0, "Cb for white");
    // Cr = R - G = 0
    assert_eq!(c2[0], 0, "Cr for white");
}

#[test]
fn rct_encode_red() {
    let mut c0 = vec![255i32]; // R
    let mut c1 = vec![0i32];   // G
    let mut c2 = vec![0i32];   // B
    rct_forward(&mut c0, &mut c1, &mut c2);
    // Y = floor((255 + 0 + 0) / 4) = 63
    assert_eq!(c0[0], 63, "Y for red");
    // Cb = B - G = 0
    assert_eq!(c1[0], 0, "Cb for red");
    // Cr = R - G = 255
    assert_eq!(c2[0], 255, "Cr for red");
}

#[test]
fn rct_roundtrip() {
    let r = vec![128i32, 64, 200, 0, 255];
    let g = vec![100i32, 32, 180, 50, 128];
    let b = vec![50i32, 16, 220, 100, 64];
    let mut c0 = r.clone();
    let mut c1 = g.clone();
    let mut c2 = b.clone();
    rct_forward(&mut c0, &mut c1, &mut c2);
    rct_inverse(&mut c0, &mut c1, &mut c2);
    assert_eq!(c0, r, "R channel mismatch");
    assert_eq!(c1, g, "G channel mismatch");
    assert_eq!(c2, b, "B channel mismatch");
}

#[test]
fn rct_roundtrip_random() {
    // Deterministic "random" data
    let n = 256;
    let mut r = Vec::with_capacity(n);
    let mut g = Vec::with_capacity(n);
    let mut b = Vec::with_capacity(n);
    let mut seed: u32 = 12345;
    for _ in 0..n {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        r.push(((seed >> 16) % 256) as i32);
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        g.push(((seed >> 16) % 256) as i32);
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        b.push(((seed >> 16) % 256) as i32);
    }
    let orig_r = r.clone();
    let orig_g = g.clone();
    let orig_b = b.clone();
    rct_forward(&mut r, &mut g, &mut b);
    rct_inverse(&mut r, &mut g, &mut b);
    assert_eq!(r, orig_r);
    assert_eq!(g, orig_g);
    assert_eq!(b, orig_b);
}

#[test]
fn ict_roundtrip() {
    let r = vec![128.0f32, 64.0, 200.0, 0.0, 255.0];
    let g = vec![100.0f32, 32.0, 180.0, 50.0, 128.0];
    let b = vec![50.0f32, 16.0, 220.0, 100.0, 64.0];
    let mut c0 = r.clone();
    let mut c1 = g.clone();
    let mut c2 = b.clone();
    ict_forward(&mut c0, &mut c1, &mut c2);
    ict_inverse(&mut c0, &mut c1, &mut c2);
    for i in 0..r.len() {
        assert!(
            (c0[i] - r[i]).abs() < 0.01,
            "R[{i}]: got {}, expected {}",
            c0[i],
            r[i]
        );
        assert!(
            (c1[i] - g[i]).abs() < 0.01,
            "G[{i}]: got {}, expected {}",
            c1[i],
            g[i]
        );
        assert!(
            (c2[i] - b[i]).abs() < 0.01,
            "B[{i}]: got {}, expected {}",
            c2[i],
            b[i]
        );
    }
}

#[test]
fn ict_precision() {
    // Single pixel, check forward+inverse error < 0.5
    let mut c0 = vec![200.0f32];
    let mut c1 = vec![100.0f32];
    let mut c2 = vec![50.0f32];
    let orig = (c0[0], c1[0], c2[0]);
    ict_forward(&mut c0, &mut c1, &mut c2);
    ict_inverse(&mut c0, &mut c1, &mut c2);
    assert!(
        (c0[0] - orig.0).abs() < 0.5,
        "R error too large: {}",
        (c0[0] - orig.0).abs()
    );
    assert!(
        (c1[0] - orig.1).abs() < 0.5,
        "G error too large: {}",
        (c1[0] - orig.1).abs()
    );
    assert!(
        (c2[0] - orig.2).abs() < 0.5,
        "B error too large: {}",
        (c2[0] - orig.2).abs()
    );
}

#[test]
fn mct_1000_samples() {
    let n = 1000;
    let mut seed: u32 = 99999;
    let mut r = Vec::with_capacity(n);
    let mut g = Vec::with_capacity(n);
    let mut b = Vec::with_capacity(n);
    for _ in 0..n {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        r.push(((seed >> 16) % 256) as i32);
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        g.push(((seed >> 16) % 256) as i32);
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        b.push(((seed >> 16) % 256) as i32);
    }
    let orig_r = r.clone();
    let orig_g = g.clone();
    let orig_b = b.clone();

    // RCT roundtrip on large data
    rct_forward(&mut r, &mut g, &mut b);
    rct_inverse(&mut r, &mut g, &mut b);
    assert_eq!(r, orig_r);
    assert_eq!(g, orig_g);
    assert_eq!(b, orig_b);
}
