/// Phase 12.2: Region of Interest (ROI) decoding tests.

use justjp2::{decode, decode_region, encode, Component, EncodeParams, Image};
use justjp2::types::CodecFormat;

/// Helper: create a simple grayscale test image with a known pattern.
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

fn encode_test_image(img: &Image) -> Vec<u8> {
    let params = EncodeParams {
        lossless: true,
        num_decomp_levels: 2,
        format: CodecFormat::J2k,
        ..Default::default()
    };
    encode(img, &params).expect("encode should succeed")
}

#[test]
fn decode_region_full() {
    // Region covering the full image should give the same result as decode.
    let img = make_gray_image(64, 64);
    let bytes = encode_test_image(&img);

    let full = decode(&bytes).expect("decode should succeed");
    let region = decode_region(&bytes, 0, 0, 64, 64).expect("decode_region should succeed");

    assert_eq!(region.width, full.width);
    assert_eq!(region.height, full.height);
    assert_eq!(region.components.len(), full.components.len());
    assert_eq!(region.components[0].data, full.components[0].data);
}

#[test]
fn decode_region_quadrant() {
    // Decode the top-left quarter of a 64x64 image.
    let img = make_gray_image(64, 64);
    let bytes = encode_test_image(&img);

    let region = decode_region(&bytes, 0, 0, 32, 32).expect("decode_region quadrant");
    assert_eq!(region.width, 32);
    assert_eq!(region.height, 32);
    assert_eq!(region.components[0].width, 32);
    assert_eq!(region.components[0].height, 32);
    assert_eq!(region.components[0].data.len(), 32 * 32);

    // Verify the region data matches the corresponding sub-region from a full decode.
    let full = decode(&bytes).expect("full decode");
    for y in 0..32u32 {
        for x in 0..32u32 {
            let region_val = region.components[0].data[(y * 32 + x) as usize];
            let full_val = full.components[0].data[(y * 64 + x) as usize];
            assert_eq!(
                region_val, full_val,
                "mismatch at ({x}, {y}): region={region_val}, full={full_val}"
            );
        }
    }
}

#[test]
fn decode_region_center() {
    // Decode a center region of a 64x64 image.
    let img = make_gray_image(64, 64);
    let bytes = encode_test_image(&img);

    let region = decode_region(&bytes, 16, 16, 48, 48).expect("decode_region center");
    assert_eq!(region.width, 32);
    assert_eq!(region.height, 32);
    assert_eq!(region.components[0].data.len(), 32 * 32);

    // Verify against full decode.
    let full = decode(&bytes).expect("full decode");
    for y in 0..32u32 {
        for x in 0..32u32 {
            let region_val = region.components[0].data[(y * 32 + x) as usize];
            let full_val = full.components[0].data[((y + 16) * 64 + (x + 16)) as usize];
            assert_eq!(
                region_val, full_val,
                "mismatch at ({x}, {y}): region={region_val}, full={full_val}"
            );
        }
    }
}

#[test]
fn decode_region_single_pixel() {
    // Decode a single pixel.
    let img = make_gray_image(64, 64);
    let bytes = encode_test_image(&img);

    let region = decode_region(&bytes, 10, 20, 11, 21).expect("decode_region single pixel");
    assert_eq!(region.width, 1);
    assert_eq!(region.height, 1);
    assert_eq!(region.components[0].data.len(), 1);

    // Verify the value matches the full decode.
    let full = decode(&bytes).expect("full decode");
    let expected = full.components[0].data[(20 * 64 + 10) as usize];
    assert_eq!(region.components[0].data[0], expected);
}

#[test]
fn decode_region_invalid_empty() {
    let img = make_gray_image(64, 64);
    let bytes = encode_test_image(&img);

    // x0 >= x1 should fail
    assert!(decode_region(&bytes, 10, 10, 10, 20).is_err());
    // y0 >= y1 should fail
    assert!(decode_region(&bytes, 10, 20, 20, 20).is_err());
}

#[test]
fn decode_region_out_of_bounds() {
    let img = make_gray_image(64, 64);
    let bytes = encode_test_image(&img);

    // Region extending beyond image bounds should fail
    assert!(decode_region(&bytes, 0, 0, 65, 64).is_err());
    assert!(decode_region(&bytes, 0, 0, 64, 65).is_err());
}
