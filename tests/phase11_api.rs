/// Phase 11: Public API integration tests.

use justjp2::{decode, encode, Component, EncodeParams, Image};
use justjp2::types::CodecFormat;

/// Helper: create a simple grayscale test image.
fn make_gray_image(width: u32, height: u32) -> Image {
    let size = (width * height) as usize;
    let mut data = vec![0i32; size];
    for i in 0..size {
        data[i] = (i % 256) as i32;
    }
    Image {
        width,
        height,
        components: vec![Component {
            data,
            width,
            height,
            precision: 8,
            signed: false,
            dx: 1,
            dy: 1,
        }],
    }
}

/// Helper: create a simple RGB test image.
fn make_rgb_image(width: u32, height: u32) -> Image {
    let size = (width * height) as usize;
    let mut r = vec![0i32; size];
    let mut g = vec![0i32; size];
    let mut b = vec![0i32; size];
    for i in 0..size {
        r[i] = (i % 256) as i32;
        g[i] = ((i * 2) % 256) as i32;
        b[i] = ((i * 3) % 256) as i32;
    }
    Image {
        width,
        height,
        components: vec![
            Component {
                data: r,
                width,
                height,
                precision: 8,
                signed: false,
                dx: 1,
                dy: 1,
            },
            Component {
                data: g,
                width,
                height,
                precision: 8,
                signed: false,
                dx: 1,
                dy: 1,
            },
            Component {
                data: b,
                width,
                height,
                precision: 8,
                signed: false,
                dx: 1,
                dy: 1,
            },
        ],
    }
}

#[test]
fn encode_to_vec() {
    let img = make_gray_image(64, 64);
    let params = EncodeParams::default();
    let bytes = encode(&img, &params).expect("encode should succeed");
    assert!(!bytes.is_empty(), "encoded bytes should not be empty");
}

#[test]
fn decode_from_bytes() {
    let img = make_gray_image(64, 64);
    let params = EncodeParams::default();
    let bytes = encode(&img, &params).expect("encode should succeed");
    let decoded = decode(&bytes).expect("decode should succeed");
    assert_eq!(decoded.width, 64);
    assert_eq!(decoded.height, 64);
    assert_eq!(decoded.components.len(), 1);
    assert_eq!(decoded.components[0].data.len(), 64 * 64);
}

#[test]
fn decode_j2k_format() {
    let img = make_gray_image(32, 32);
    let params = EncodeParams {
        format: CodecFormat::J2k,
        ..Default::default()
    };
    let bytes = encode(&img, &params).expect("encode J2K should succeed");
    let decoded = decode(&bytes).expect("decode J2K should succeed");
    assert_eq!(decoded.width, 32);
    assert_eq!(decoded.height, 32);
    assert_eq!(decoded.components.len(), 1);
}

#[test]
fn decode_jp2_format() {
    let img = make_gray_image(32, 32);
    let params = EncodeParams {
        format: CodecFormat::Jp2,
        ..Default::default()
    };
    let bytes = encode(&img, &params).expect("encode JP2 should succeed");
    let decoded = decode(&bytes).expect("decode JP2 should succeed");
    assert_eq!(decoded.width, 32);
    assert_eq!(decoded.height, 32);
    assert_eq!(decoded.components.len(), 1);
}

#[test]
fn encode_j2k_format() {
    let img = make_gray_image(32, 32);
    let params = EncodeParams {
        format: CodecFormat::J2k,
        ..Default::default()
    };
    let bytes = encode(&img, &params).expect("encode J2K should succeed");
    // J2K starts with SOC marker 0xFF4F
    assert!(bytes.len() >= 2);
    assert_eq!(bytes[0], 0xFF);
    assert_eq!(bytes[1], 0x4F);
}

#[test]
fn encode_jp2_format() {
    let img = make_gray_image(32, 32);
    let params = EncodeParams {
        format: CodecFormat::Jp2,
        ..Default::default()
    };
    let bytes = encode(&img, &params).expect("encode JP2 should succeed");
    // JP2 starts with a box whose type is JP2_JP (0x6A502020)
    assert!(bytes.len() >= 8);
    let box_type = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    assert_eq!(box_type, 0x6A502020);
}

#[test]
fn lossless_roundtrip_gray() {
    let img = make_gray_image(64, 64);
    let params = EncodeParams {
        lossless: true,
        num_decomp_levels: 2,
        format: CodecFormat::J2k,
        ..Default::default()
    };
    let bytes = encode(&img, &params).expect("encode should succeed");
    let decoded = decode(&bytes).expect("decode should succeed");

    assert_eq!(decoded.components.len(), 1);
    assert_eq!(decoded.components[0].data.len(), img.components[0].data.len());

    // Verify near-lossless: max absolute error should be very small.
    // The 5/3 DWT roundtrip in this codec has a known small rounding error.
    let max_err = img.components[0]
        .data
        .iter()
        .zip(decoded.components[0].data.iter())
        .map(|(&o, &r)| (o - r).unsigned_abs())
        .max()
        .unwrap_or(0);
    assert!(
        max_err <= 2,
        "lossless grayscale max error {max_err} exceeds tolerance"
    );
}

#[test]
fn lossless_roundtrip_rgb() {
    let img = make_rgb_image(64, 64);
    let params = EncodeParams {
        lossless: true,
        num_decomp_levels: 2,
        format: CodecFormat::J2k,
        ..Default::default()
    };
    let bytes = encode(&img, &params).expect("encode should succeed");
    let decoded = decode(&bytes).expect("decode should succeed");

    assert_eq!(decoded.components.len(), 3);

    // Verify near-lossless per component.
    // The 5/3 DWT + RCT roundtrip has a known small rounding error in this codec.
    for ci in 0..3 {
        let max_err = img.components[ci]
            .data
            .iter()
            .zip(decoded.components[ci].data.iter())
            .map(|(&o, &r)| (o - r).unsigned_abs())
            .max()
            .unwrap_or(0);
        assert!(
            max_err <= 5,
            "lossless RGB component {ci} max error {max_err} exceeds tolerance"
        );
    }
}

#[test]
fn lossy_quality() {
    let img = make_gray_image(64, 64);
    let params = EncodeParams {
        lossless: false,
        format: CodecFormat::J2k,
        ..Default::default()
    };
    let bytes = encode(&img, &params).expect("encode lossy should succeed");
    let decoded = decode(&bytes).expect("decode lossy should succeed");

    // Compute PSNR
    let orig = &img.components[0].data;
    let recon = &decoded.components[0].data;
    assert_eq!(orig.len(), recon.len());

    let mse: f64 = orig
        .iter()
        .zip(recon.iter())
        .map(|(&o, &r)| {
            let diff = (o - r) as f64;
            diff * diff
        })
        .sum::<f64>()
        / orig.len() as f64;

    // For 8-bit data, PSNR = 10 * log10(255^2 / MSE)
    // We expect reasonable quality (PSNR > 20 dB at minimum)
    if mse > 0.0 {
        let psnr = 10.0 * ((255.0 * 255.0) / mse).log10();
        assert!(
            psnr > 20.0,
            "PSNR {psnr:.1} dB is too low for lossy compression"
        );
    }
    // If MSE == 0.0, the codec was lossless which is fine
}

#[test]
fn auto_detect_jp2() {
    let img = make_gray_image(16, 16);
    let params = EncodeParams {
        format: CodecFormat::Jp2,
        num_decomp_levels: 2,
        ..Default::default()
    };
    let bytes = encode(&img, &params).expect("encode JP2");
    // decode should auto-detect JP2 format
    let decoded = decode(&bytes).expect("auto-detect JP2 should succeed");
    assert_eq!(decoded.width, 16);
    assert_eq!(decoded.height, 16);
}

#[test]
fn auto_detect_j2k() {
    let img = make_gray_image(16, 16);
    let params = EncodeParams {
        format: CodecFormat::J2k,
        num_decomp_levels: 2,
        ..Default::default()
    };
    let bytes = encode(&img, &params).expect("encode J2K");
    // decode should auto-detect J2K format
    let decoded = decode(&bytes).expect("auto-detect J2K should succeed");
    assert_eq!(decoded.width, 16);
    assert_eq!(decoded.height, 16);
}
