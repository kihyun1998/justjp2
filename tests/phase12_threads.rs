/// Phase 12.3: Multithreading tests.

use justjp2::{decode, encode, Component, EncodeParams, Image};
use justjp2::types::CodecFormat;
use justjp2::j2k::j2k_encode_tiled;
use justjp2::tcd::{TcdComponent, TcdParams};

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
            Component { data: r, width, height, precision: 8, signed: false, dx: 1, dy: 1 },
            Component { data: g, width, height, precision: 8, signed: false, dx: 1, dy: 1 },
            Component { data: b, width, height, precision: 8, signed: false, dx: 1, dy: 1 },
        ],
    }
}

#[test]
fn parallel_encode_produces_same_result() {
    // Encoding the same image twice should produce identical output,
    // because rayon parallelism is deterministic for the same input order.
    let img = make_gray_image(64, 64);
    let params = EncodeParams {
        lossless: true,
        num_decomp_levels: 2,
        format: CodecFormat::J2k,
        ..Default::default()
    };

    let bytes1 = encode(&img, &params).expect("first encode");
    let bytes2 = encode(&img, &params).expect("second encode");
    assert_eq!(bytes1, bytes2, "two encodes of the same image should produce identical output");

    // Also verify the output decodes correctly.
    let decoded = decode(&bytes1).expect("decode");
    assert_eq!(decoded.width, 64);
    assert_eq!(decoded.height, 64);
    assert_eq!(decoded.components.len(), 1);
}

#[test]
fn parallel_multi_tile() {
    // Encode a larger image with multiple tiles using j2k_encode_tiled.
    // With a 128x128 image and 64x64 tiles, we get 4 tiles that can be
    // encoded in parallel.
    let width = 128u32;
    let height = 128u32;
    let size = (width * height) as usize;
    let mut data = vec![0i32; size];
    for i in 0..size {
        data[i] = (i % 256) as i32;
    }

    let comp_info = vec![TcdComponent {
        width,
        height,
        precision: 8,
        signed: false,
        dx: 1,
        dy: 1,
    }];

    let params = TcdParams {
        num_res: 3,
        cblk_w: 64,
        cblk_h: 64,
        reversible: true,
        num_layers: 1,
        use_mct: false,
        reduce: 0,
        max_bytes: None,
    };

    let components = vec![data];

    // Encode with 64x64 tiles (4 tiles)
    let bytes = j2k_encode_tiled(&components, &comp_info, &params, 64, 64)
        .expect("tiled encode should succeed");

    // Decode and verify
    let decoded = decode(&bytes).expect("decode tiled");
    assert_eq!(decoded.width, 128);
    assert_eq!(decoded.height, 128);
    assert_eq!(decoded.components.len(), 1);
    assert_eq!(decoded.components[0].data.len(), size);
}

#[test]
fn parallel_rgb_roundtrip() {
    // Verify that parallel encoding of RGB images works correctly.
    let img = make_rgb_image(64, 64);
    let params = EncodeParams {
        lossless: true,
        num_decomp_levels: 2,
        format: CodecFormat::J2k,
        ..Default::default()
    };

    let bytes = encode(&img, &params).expect("encode RGB");
    let decoded = decode(&bytes).expect("decode RGB");

    assert_eq!(decoded.components.len(), 3);
    for ci in 0..3 {
        assert_eq!(decoded.components[ci].data.len(), 64 * 64);
    }
}

#[test]
fn thread_count_check() {
    // Verify that rayon is operational by checking the thread pool.
    let pool = rayon::ThreadPoolBuilder::new().build().unwrap();
    let thread_count = pool.current_num_threads();
    assert!(
        thread_count >= 1,
        "rayon should have at least 1 thread, got {thread_count}"
    );
}
