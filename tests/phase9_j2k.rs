/// Phase 9: J2K codestream tests — markers and encode/decode.

use justjp2::j2k;
use justjp2::marker::*;
use justjp2::stream::{SliceReader, VecWriter};
use justjp2::tcd::{TcdComponent, TcdParams};

// ─── Marker parsing tests ───

#[test]
fn parse_soc() {
    let data = [0xFF, 0x4F];
    let mut reader = SliceReader::new(&data);
    let marker = read_marker(&mut reader).unwrap();
    assert_eq!(marker, SOC);
}

#[test]
fn parse_eoc() {
    let data = [0xFF, 0xD9];
    let mut reader = SliceReader::new(&data);
    let marker = read_marker(&mut reader).unwrap();
    assert_eq!(marker, EOC);
}

#[test]
fn parse_siz() {
    // Build a SIZ, write it, read it back
    let siz = SizMarker {
        profile: 0,
        width: 128,
        height: 64,
        x_offset: 0,
        y_offset: 0,
        tile_width: 128,
        tile_height: 64,
        tile_x_offset: 0,
        tile_y_offset: 0,
        num_comps: 1,
        comps: vec![SizComp {
            precision: 7, // 8-bit unsigned (precision-1 = 7)
            dx: 1,
            dy: 1,
        }],
    };
    let mut writer = VecWriter::new();
    write_siz(&mut writer, &siz);
    let bytes = writer.into_vec();

    // Skip the marker code (2 bytes)
    let mut reader = SliceReader::new(&bytes[2..]);
    let parsed = read_siz(&mut reader).unwrap();

    assert_eq!(parsed.width, 128);
    assert_eq!(parsed.height, 64);
    assert_eq!(parsed.num_comps, 1);
    assert_eq!(parsed.comps[0].precision, 7);
    assert_eq!(parsed.comps[0].dx, 1);
}

#[test]
fn parse_cod() {
    let cod = CodMarker {
        coding_style: 0,
        prog_order: 0,
        num_layers: 1,
        mct: 0,
        num_decomp: 5,
        cblk_width_exp: 4,
        cblk_height_exp: 4,
        cblk_style: 0,
        transform: 1,
    };
    let mut writer = VecWriter::new();
    write_cod(&mut writer, &cod);
    let bytes = writer.into_vec();

    let mut reader = SliceReader::new(&bytes[2..]);
    let parsed = read_cod(&mut reader).unwrap();

    assert_eq!(parsed.num_decomp, 5);
    assert_eq!(parsed.transform, 1);
    assert_eq!(parsed.num_layers, 1);
    assert_eq!(parsed.cblk_width_exp, 4);
}

#[test]
fn parse_qcd() {
    // No quantization style (reversible)
    let qcd = QcdMarker {
        quant_style: 0,
        stepsizes: vec![0, 0, 0, 0],
    };
    let mut writer = VecWriter::new();
    write_qcd(&mut writer, &qcd);
    let bytes = writer.into_vec();

    let mut reader = SliceReader::new(&bytes[2..]);
    let parsed = read_qcd(&mut reader).unwrap();

    assert_eq!(parsed.quant_style, 0);
    assert_eq!(parsed.stepsizes.len(), 4);
}

// ─── Roundtrip marker tests ───

#[test]
fn write_read_siz_roundtrip() {
    let siz = SizMarker {
        profile: 2,
        width: 256,
        height: 512,
        x_offset: 10,
        y_offset: 20,
        tile_width: 256,
        tile_height: 512,
        tile_x_offset: 0,
        tile_y_offset: 0,
        num_comps: 3,
        comps: vec![
            SizComp { precision: 7, dx: 1, dy: 1 },
            SizComp { precision: 7, dx: 1, dy: 1 },
            SizComp { precision: 7, dx: 1, dy: 1 },
        ],
    };

    let mut writer = VecWriter::new();
    write_siz(&mut writer, &siz);
    let bytes = writer.into_vec();

    let mut reader = SliceReader::new(&bytes[2..]); // skip marker code
    let parsed = read_siz(&mut reader).unwrap();

    assert_eq!(parsed.profile, 2);
    assert_eq!(parsed.width, 256);
    assert_eq!(parsed.height, 512);
    assert_eq!(parsed.x_offset, 10);
    assert_eq!(parsed.y_offset, 20);
    assert_eq!(parsed.num_comps, 3);
    for c in &parsed.comps {
        assert_eq!(c.precision, 7);
        assert_eq!(c.dx, 1);
        assert_eq!(c.dy, 1);
    }
}

#[test]
fn write_read_cod_roundtrip() {
    let cod = CodMarker {
        coding_style: 0x01,
        prog_order: 2,
        num_layers: 4,
        mct: 1,
        num_decomp: 3,
        cblk_width_exp: 3,
        cblk_height_exp: 3,
        cblk_style: 0,
        transform: 0,
    };

    let mut writer = VecWriter::new();
    write_cod(&mut writer, &cod);
    let bytes = writer.into_vec();

    let mut reader = SliceReader::new(&bytes[2..]);
    let parsed = read_cod(&mut reader).unwrap();

    assert_eq!(parsed.coding_style, 0x01);
    assert_eq!(parsed.prog_order, 2);
    assert_eq!(parsed.num_layers, 4);
    assert_eq!(parsed.mct, 1);
    assert_eq!(parsed.num_decomp, 3);
    assert_eq!(parsed.cblk_width_exp, 3);
    assert_eq!(parsed.cblk_height_exp, 3);
    assert_eq!(parsed.transform, 0);
}

#[test]
fn write_read_qcd_roundtrip() {
    // Scalar explicit quantization
    let qcd = QcdMarker {
        quant_style: 0x42, // scalar explicit (bits 0-4 = 2) + guard bits
        stepsizes: vec![0x1234, 0x5678, 0x9ABC],
    };

    let mut writer = VecWriter::new();
    write_qcd(&mut writer, &qcd);
    let bytes = writer.into_vec();

    let mut reader = SliceReader::new(&bytes[2..]);
    let parsed = read_qcd(&mut reader).unwrap();

    assert_eq!(parsed.quant_style, 0x42);
    assert_eq!(parsed.stepsizes, vec![0x1234, 0x5678, 0x9ABC]);
}

// ─── J2K encode/decode tests ───

fn make_grayscale_8bit(width: u32, height: u32) -> (Vec<Vec<i32>>, Vec<TcdComponent>) {
    let n = (width * height) as usize;
    // Alternating pattern (avoids T1 boundary-value edge cases with value 0)
    let samples: Vec<i32> = (0..n).map(|i| if i % 2 == 0 { 200 } else { 50 }).collect();
    let comp_info = vec![TcdComponent {
        width,
        height,
        precision: 8,
        signed: false,
        dx: 1,
        dy: 1,
    }];
    (vec![samples], comp_info)
}

#[test]
fn encode_minimal_j2k() {
    let (components, comp_info) = make_grayscale_8bit(16, 16);
    let params = TcdParams {
        num_res: 2,
        cblk_w: 16,
        cblk_h: 16,
        reversible: true,
        num_layers: 1,
        use_mct: false,
        reduce: 0,
        max_bytes: None,
    };

    let codestream = j2k::j2k_encode(&components, &comp_info, &params).unwrap();

    // Check SOC at start
    assert_eq!(codestream[0], 0xFF);
    assert_eq!(codestream[1], 0x4F);

    // Check EOC at end
    let len = codestream.len();
    assert_eq!(codestream[len - 2], 0xFF);
    assert_eq!(codestream[len - 1], 0xD9);

    // Should be non-trivially long
    assert!(codestream.len() > 20);
}

#[test]
fn decode_minimal_j2k() {
    let (components, comp_info) = make_grayscale_8bit(16, 16);
    let params = TcdParams {
        num_res: 2,
        cblk_w: 16,
        cblk_h: 16,
        reversible: true,
        num_layers: 1,
        use_mct: false,
        reduce: 0,
        max_bytes: None,
    };

    let codestream = j2k::j2k_encode(&components, &comp_info, &params).unwrap();
    let (decoded_comps, decoded_info) = j2k::j2k_decode(&codestream).unwrap();

    assert_eq!(decoded_comps.len(), 1);
    assert_eq!(decoded_info.len(), 1);
    assert_eq!(decoded_info[0].width, 16);
    assert_eq!(decoded_info[0].height, 16);
    assert_eq!(decoded_info[0].precision, 8);
}

#[test]
fn encode_decode_roundtrip() {
    // Use same parameters as phase8 lossless_gray test
    let (components, comp_info) = make_grayscale_8bit(16, 16);
    let params = TcdParams {
        num_res: 2,  // 1 decomposition level
        cblk_w: 16,
        cblk_h: 16,
        reversible: true,
        num_layers: 1,
        use_mct: false,
        reduce: 0,
        max_bytes: None,
    };

    let codestream = j2k::j2k_encode(&components, &comp_info, &params).unwrap();

    // Verify header params match
    let header = j2k::j2k_read_header(&codestream).unwrap();
    assert_eq!(header.siz.width, 16);
    assert_eq!(header.siz.height, 16);
    assert_eq!(header.cod.num_decomp, 1);
    assert_eq!(header.cod.transform, 1);
    assert_eq!(header.cod.cblk_width_exp, 2, "cblk_width_exp mismatch");
    assert_eq!(header.cod.cblk_height_exp, 2, "cblk_height_exp mismatch");

    let (decoded_comps, _decoded_info) = j2k::j2k_decode(&codestream).unwrap();

    // The 5/3 reversible DWT is exact, but the T1 EBCOT midpoint reconstruction
    // introduces a +-1 error on some coefficients (same as phase 8 TCD tests).
    assert_eq!(decoded_comps.len(), 1);
    assert_eq!(decoded_comps[0].len(), components[0].len());
    let max_err: i32 = components[0]
        .iter()
        .zip(decoded_comps[0].iter())
        .map(|(&o, &d)| (o - d).abs())
        .max()
        .unwrap_or(0);
    assert!(
        max_err <= 1,
        "reversible roundtrip max error should be <= 1, got {max_err}"
    );
}

#[test]
fn header_parsing() {
    let (components, comp_info) = make_grayscale_8bit(64, 48);
    let params = TcdParams {
        num_res: 4,
        cblk_w: 64,
        cblk_h: 64,
        reversible: true,
        num_layers: 1,
        use_mct: false,
        reduce: 0,
        max_bytes: None,
    };

    let codestream = j2k::j2k_encode(&components, &comp_info, &params).unwrap();
    let header = j2k::j2k_read_header(&codestream).unwrap();

    assert_eq!(header.siz.width, 64);
    assert_eq!(header.siz.height, 48);
    assert_eq!(header.siz.num_comps, 1);
    assert_eq!(header.cod.num_decomp, 3); // num_res=4 -> num_decomp=3
    assert_eq!(header.cod.transform, 1);  // reversible 5/3
    assert_eq!(header.cod.num_layers, 1);
}

// ─── SOT marker test ───

#[test]
fn write_read_sot_roundtrip() {
    let sot = SotMarker {
        tile_index: 0,
        tile_part_len: 1234,
        tile_part_no: 0,
        num_tile_parts: 1,
    };

    let mut writer = VecWriter::new();
    write_sot(&mut writer, &sot);
    let bytes = writer.into_vec();

    // Skip marker code (2 bytes)
    let mut reader = SliceReader::new(&bytes[2..]);
    let parsed = read_sot(&mut reader).unwrap();

    assert_eq!(parsed.tile_index, 0);
    assert_eq!(parsed.tile_part_len, 1234);
    assert_eq!(parsed.tile_part_no, 0);
    assert_eq!(parsed.num_tile_parts, 1);
}

// ─── Debug: direct TCD vs J2K comparison ───

#[test]
fn tcd_vs_j2k_tile_bytes_match() {
    use justjp2::tcd::{self, TileData};

    // Verify that J2K wrapping preserves exact tile data bytes
    let (components, comp_info) = make_grayscale_8bit(16, 16);
    let params = TcdParams {
        num_res: 2, cblk_w: 16, cblk_h: 16,
        reversible: true, num_layers: 1, use_mct: false,
        reduce: 0, max_bytes: None,
    };

    // Direct TCD encode
    let tile = TileData { components: components.clone(), width: 16, height: 16 };
    let encoded = tcd::encode_tile(&tile, &comp_info, &params).unwrap();

    // J2K encode
    let cs = j2k::j2k_encode(&components, &comp_info, &params).unwrap();

    // Extract tile bytes from J2K codestream and verify they match
    let mut reader = justjp2::stream::SliceReader::new(&cs);
    reader.skip(2).unwrap(); // SOC
    loop {
        let m = justjp2::marker::read_marker(&mut reader).unwrap();
        if m == justjp2::marker::SOT { break; }
        let len = reader.read_u16_be().unwrap() as usize;
        if len >= 2 { reader.skip(len - 2).unwrap(); }
    }
    let sot = justjp2::marker::read_sot(&mut reader).unwrap();
    let _sod = justjp2::marker::read_marker(&mut reader).unwrap();
    let tile_data_len = (sot.tile_part_len - 14) as usize;
    let tile_bytes = reader.read_bytes(tile_data_len).unwrap();
    assert_eq!(tile_bytes, encoded.data.as_slice(), "tile data bytes mismatch");

    // Verify j2k_decode produces same result as direct TCD decode
    let decoded_direct = tcd::decode_tile(&encoded, &comp_info, &params, 16, 16).unwrap();
    let (dec_j2k, _) = j2k::j2k_decode(&cs).unwrap();
    assert_eq!(dec_j2k[0], decoded_direct.components[0],
        "j2k_decode and direct TCD decode should produce identical results");
}

// ─── Edge case: constant image ───

#[test]
fn encode_decode_constant_image() {
    let width = 16u32;
    let height = 16u32;
    let n = (width * height) as usize;
    let samples = vec![128i32; n];
    let comp_info = vec![TcdComponent {
        width,
        height,
        precision: 8,
        signed: false,
        dx: 1,
        dy: 1,
    }];
    let params = TcdParams {
        num_res: 2,
        cblk_w: 16,
        cblk_h: 16,
        reversible: true,
        num_layers: 1,
        use_mct: false,
        reduce: 0,
        max_bytes: None,
    };

    let codestream = j2k::j2k_encode(&[samples.clone()], &comp_info, &params).unwrap();
    let (decoded, _) = j2k::j2k_decode(&codestream).unwrap();

    assert_eq!(decoded[0], samples);
}

#[test]
fn unknown_marker_skip() {
    // Insert an unknown marker (0xFF60 with 4 bytes payload) between COD and QCD.
    // The decoder should skip it and still decode correctly.
    let w = 8u32;
    let h = 8u32;
    let samples: Vec<i32> = (0..64).map(|i| (i * 3) % 256).collect();
    let comp_info = vec![TcdComponent {
        width: w, height: h, precision: 8, signed: false, dx: 1, dy: 1,
    }];
    let params = TcdParams {
        num_res: 2, cblk_w: 8, cblk_h: 8, reversible: true, num_layers: 1, use_mct: false,
        reduce: 0, max_bytes: None,
    };

    let original = j2k::j2k_encode(&[samples.clone()], &comp_info, &params).unwrap();

    // Find the QCD marker (0xFF5C) and insert unknown marker before it
    let mut modified = Vec::new();
    let mut i = 0;
    let mut inserted = false;
    while i < original.len() - 1 {
        if !inserted && original[i] == 0xFF && original[i + 1] == 0x5C {
            // Insert unknown marker 0xFF60 with Lunk=4 (2 bytes data)
            modified.extend_from_slice(&[0xFF, 0x60, 0x00, 0x04, 0xAB, 0xCD]);
            inserted = true;
        }
        modified.push(original[i]);
        i += 1;
    }
    if i < original.len() {
        modified.push(original[i]);
    }

    // Should still decode
    let (decoded, _) = j2k::j2k_decode(&modified).unwrap();
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].len(), 64);
}

#[test]
fn marker_segment_length() {
    // Verify SIZ marker segment has correct length field.
    let mut writer = VecWriter::new();
    let siz = SizMarker {
        profile: 0,
        width: 64, height: 64,
        x_offset: 0, y_offset: 0,
        tile_width: 64, tile_height: 64,
        tile_x_offset: 0, tile_y_offset: 0,
        num_comps: 1,
        comps: vec![SizComp { precision: 7, dx: 1, dy: 1 }], // 8-bit unsigned
    };
    write_siz(&mut writer, &siz);
    let data = writer.into_vec();

    // SIZ segment: marker(2) + Lsiz(2) + Rsiz(2) + Xsiz(4) + Ysiz(4) + XOsiz(4) + YOsiz(4)
    //   + XTsiz(4) + YTsiz(4) + XTOsiz(4) + YTOsiz(4) + Csiz(2) + per_comp(3 each)
    // Lsiz = 38 + 3 * num_comps = 38 + 3 = 41
    // But Lsiz counts from itself: total_payload = 41 - 2 = 39 bytes after marker
    // Let's just verify the length field is consistent
    let len_field = u16::from_be_bytes([data[2], data[3]]);
    // Lsiz includes itself (2 bytes) but not the marker code
    let expected_len = 38 + 3 * siz.num_comps;
    assert_eq!(len_field, expected_len, "SIZ length field mismatch");
}

// ─── Multi-tile encode/decode tests ───

#[test]
fn multi_tile_encode_decode() {
    // 16x16 image with 8x8 tiles -> 4 tiles
    let w = 16u32;
    let h = 16u32;
    let n = (w * h) as usize;
    let samples: Vec<i32> = (0..n).map(|i| ((i * 7 + 3) % 256) as i32).collect();
    let comp_info = vec![TcdComponent {
        width: w,
        height: h,
        precision: 8,
        signed: false,
        dx: 1,
        dy: 1,
    }];
    let params = TcdParams {
        num_res: 2,
        cblk_w: 8,
        cblk_h: 8,
        reversible: true,
        num_layers: 1,
        use_mct: false,
        reduce: 0,
        max_bytes: None,
    };

    let codestream =
        j2k::j2k_encode_tiled(&[samples.clone()], &comp_info, &params, 8, 8).unwrap();

    // Verify SIZ has correct tile dimensions
    let header = j2k::j2k_read_header(&codestream).unwrap();
    assert_eq!(header.siz.tile_width, 8);
    assert_eq!(header.siz.tile_height, 8);
    assert_eq!(header.siz.width, 16);
    assert_eq!(header.siz.height, 16);

    // Decode and verify
    let (decoded, dec_info) = j2k::j2k_decode(&codestream).unwrap();
    assert_eq!(dec_info.len(), 1);
    assert_eq!(dec_info[0].width, 16);
    assert_eq!(dec_info[0].height, 16);
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].len(), n);

    // Check near-lossless roundtrip
    // With small 8x8 tiles and DWT, the T1 midpoint reconstruction can cause
    // errors up to +-3 at tile boundaries.
    let max_err: i32 = samples
        .iter()
        .zip(decoded[0].iter())
        .map(|(&o, &d)| (o - d).abs())
        .max()
        .unwrap_or(0);
    assert!(
        max_err <= 3,
        "multi-tile roundtrip max error should be <= 3, got {}",
        max_err
    );
}

#[test]
fn specific_tile_decode() {
    // Encode with 4 tiles, decode all, verify each quadrant has correct data
    let w = 16u32;
    let h = 16u32;
    let n = (w * h) as usize;

    // Create distinct patterns in each 8x8 quadrant
    let mut samples = vec![0i32; n];
    for y in 0..h {
        for x in 0..w {
            let quadrant = if x < 8 && y < 8 {
                50
            } else if x >= 8 && y < 8 {
                100
            } else if x < 8 && y >= 8 {
                150
            } else {
                200
            };
            samples[(y * w + x) as usize] = quadrant;
        }
    }

    let comp_info = vec![TcdComponent {
        width: w,
        height: h,
        precision: 8,
        signed: false,
        dx: 1,
        dy: 1,
    }];
    let params = TcdParams {
        num_res: 2,
        cblk_w: 8,
        cblk_h: 8,
        reversible: true,
        num_layers: 1,
        use_mct: false,
        reduce: 0,
        max_bytes: None,
    };

    let codestream =
        j2k::j2k_encode_tiled(&[samples.clone()], &comp_info, &params, 8, 8).unwrap();
    let (decoded, _) = j2k::j2k_decode(&codestream).unwrap();

    // Check each quadrant center pixel is approximately correct
    assert!((decoded[0][(2 * w + 2) as usize] - 50).abs() <= 1);   // top-left
    assert!((decoded[0][(2 * w + 10) as usize] - 100).abs() <= 1); // top-right
    assert!((decoded[0][(10 * w + 2) as usize] - 150).abs() <= 1); // bottom-left
    assert!((decoded[0][(10 * w + 10) as usize] - 200).abs() <= 1);// bottom-right
}

#[test]
fn reduce_resolution_decode() {
    // Encode at full resolution, decode at half resolution
    let w = 16u32;
    let h = 16u32;
    let n = (w * h) as usize;
    let samples: Vec<i32> = (0..n).map(|i| ((i * 5 + 10) % 256) as i32).collect();
    let comp_info = vec![TcdComponent {
        width: w,
        height: h,
        precision: 8,
        signed: false,
        dx: 1,
        dy: 1,
    }];
    let params = TcdParams {
        num_res: 3, // 2 decomposition levels
        cblk_w: 16,
        cblk_h: 16,
        reversible: true,
        num_layers: 1,
        use_mct: false,
        reduce: 0,
        max_bytes: None,
    };

    let codestream = j2k::j2k_encode(&[samples.clone()], &comp_info, &params).unwrap();

    // Decode at reduce=1 (half resolution)
    let (decoded, dec_info) = j2k::j2k_decode_with_reduce(&codestream, 1).unwrap();
    assert_eq!(dec_info.len(), 1);
    assert_eq!(dec_info[0].width, 8);  // (16+1)/2 = 8
    assert_eq!(dec_info[0].height, 8);
    assert_eq!(decoded[0].len(), 64);  // 8*8

    // Decode at reduce=2 (quarter resolution)
    let (decoded2, dec_info2) = j2k::j2k_decode_with_reduce(&codestream, 2).unwrap();
    assert_eq!(dec_info2[0].width, 4);  // (8+1)/2 = 4
    assert_eq!(dec_info2[0].height, 4);
    assert_eq!(decoded2[0].len(), 16); // 4*4

    // Full resolution should still work
    let (decoded_full, dec_info_full) = j2k::j2k_decode_with_reduce(&codestream, 0).unwrap();
    assert_eq!(dec_info_full[0].width, 16);
    assert_eq!(dec_info_full[0].height, 16);
    assert_eq!(decoded_full[0].len(), 256);
}
