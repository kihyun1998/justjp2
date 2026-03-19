/// Phase 10: JP2 File Format tests

use justjp2::jp2;
use justjp2::jp2_box::*;
use justjp2::stream::{SliceReader, VecWriter};
use justjp2::tcd::{TcdComponent, TcdParams};

// ── Helper: build a simple grayscale component set ──

fn make_grayscale_8x8() -> (Vec<Vec<i32>>, Vec<TcdComponent>, TcdParams) {
    let width = 8u32;
    let height = 8u32;
    let pixels: Vec<i32> = (0..64).map(|i| (i * 4) % 256).collect();
    let comp_info = vec![TcdComponent {
        width,
        height,
        precision: 8,
        signed: false,
        dx: 1,
        dy: 1,
    }];
    let params = TcdParams {
        num_res: 1, // no DWT decomposition for lossless roundtrip
        cblk_w: 64,
        cblk_h: 64,
        reversible: true,
        num_layers: 1,
        use_mct: false,
    };
    (vec![pixels], comp_info, params)
}

fn make_rgb_8x8() -> (Vec<Vec<i32>>, Vec<TcdComponent>, TcdParams) {
    let width = 8u32;
    let height = 8u32;
    let r: Vec<i32> = (0..64).map(|i| (i * 3) % 256).collect();
    let g: Vec<i32> = (0..64).map(|i| (i * 5 + 10) % 256).collect();
    let b: Vec<i32> = (0..64).map(|i| (i * 7 + 20) % 256).collect();
    let comp_info = vec![
        TcdComponent { width, height, precision: 8, signed: false, dx: 1, dy: 1 },
        TcdComponent { width, height, precision: 8, signed: false, dx: 1, dy: 1 },
        TcdComponent { width, height, precision: 8, signed: false, dx: 1, dy: 1 },
    ];
    let params = TcdParams {
        num_res: 1, // no DWT decomposition for lossless roundtrip
        cblk_w: 64,
        cblk_h: 64,
        reversible: true,
        num_layers: 1,
        use_mct: true,
    };
    (vec![r, g, b], comp_info, params)
}

// ── Box header tests ──

#[test]
fn read_signature_box() {
    // Build a JP signature box: length=12, type=JP, data=signature
    let mut w = VecWriter::new();
    w.write_u32_be(12);          // LBox = 12
    w.write_u32_be(JP2_JP);      // TBox
    w.write_u32_be(JP2_SIGNATURE); // content

    let data = w.into_vec();
    let mut reader = SliceReader::new(&data);
    let header = read_box_header(&mut reader).unwrap();

    assert_eq!(header.box_type, JP2_JP);
    assert_eq!(header.length, 12);
    assert_eq!(header.header_size, 8);

    let sig = reader.read_u32_be().unwrap();
    assert_eq!(sig, JP2_SIGNATURE);
}

#[test]
fn read_ftyp_box() {
    let mut w = VecWriter::new();
    // FTYP box: header(8) + brand(4) + MinVer(4) + CL(4) = 20
    w.write_u32_be(20);
    w.write_u32_be(JP2_FTYP);
    w.write_u32_be(JP2_BRAND);   // brand
    w.write_u32_be(0);            // MinVersion
    w.write_u32_be(JP2_BRAND);   // compatibility

    let data = w.into_vec();
    let mut reader = SliceReader::new(&data);
    let header = read_box_header(&mut reader).unwrap();

    assert_eq!(header.box_type, JP2_FTYP);
    assert_eq!(header.length, 20);

    let brand = reader.read_u32_be().unwrap();
    assert_eq!(brand, JP2_BRAND);
    let min_ver = reader.read_u32_be().unwrap();
    assert_eq!(min_ver, 0);
    let compat = reader.read_u32_be().unwrap();
    assert_eq!(compat, JP2_BRAND);
}

#[test]
fn read_ihdr_box() {
    let mut w = VecWriter::new();
    write_box_header(&mut w, JP2_IHDR, 14);
    let ihdr = IhdrBox {
        height: 480,
        width: 640,
        num_comps: 3,
        bpc: 7, // 8 bits unsigned = (8-1) = 7
        compression: 7,
        unk_colorspace: 0,
        ipr: 0,
    };
    write_ihdr(&mut w, &ihdr);

    let data = w.into_vec();
    let mut reader = SliceReader::new(&data);
    let header = read_box_header(&mut reader).unwrap();
    assert_eq!(header.box_type, JP2_IHDR);

    let parsed = read_ihdr(&mut reader).unwrap();
    assert_eq!(parsed.height, 480);
    assert_eq!(parsed.width, 640);
    assert_eq!(parsed.num_comps, 3);
    assert_eq!(parsed.bpc, 7);
    assert_eq!(parsed.compression, 7);
}

#[test]
fn read_colr_box_enum() {
    let mut w = VecWriter::new();
    let colr = ColrBox {
        method: 1,
        precedence: 0,
        approx: 0,
        enum_cs: Some(CS_SRGB),
        icc_profile: None,
    };
    let payload_len = 7u64; // method(1) + prec(1) + approx(1) + enum_cs(4)
    write_box_header(&mut w, JP2_COLR, payload_len);
    write_colr(&mut w, &colr);

    let data = w.into_vec();
    let mut reader = SliceReader::new(&data);
    let header = read_box_header(&mut reader).unwrap();
    assert_eq!(header.box_type, JP2_COLR);

    let content_len = header.length as usize - header.header_size as usize;
    let parsed = read_colr(&mut reader, content_len).unwrap();
    assert_eq!(parsed.method, 1);
    assert_eq!(parsed.enum_cs, Some(CS_SRGB));
    assert!(parsed.icc_profile.is_none());
}

#[test]
fn write_read_box_roundtrip() {
    // Normal header
    let mut w = VecWriter::new();
    write_box_header(&mut w, JP2_IHDR, 14);
    let data = w.into_vec();
    let mut reader = SliceReader::new(&data);
    let h = read_box_header(&mut reader).unwrap();
    assert_eq!(h.box_type, JP2_IHDR);
    assert_eq!(h.length, 22); // 8 + 14
    assert_eq!(h.header_size, 8);

    // Extended header
    let mut w2 = VecWriter::new();
    write_box_header_xl(&mut w2, JP2_JP2C, 0x1_0000_0000); // > 4GB
    let data2 = w2.into_vec();
    let mut reader2 = SliceReader::new(&data2);
    let h2 = read_box_header(&mut reader2).unwrap();
    assert_eq!(h2.box_type, JP2_JP2C);
    assert_eq!(h2.length, 0x1_0000_0000 + 16);
    assert_eq!(h2.header_size, 16);
}

#[test]
fn nested_jp2h_box() {
    // Build a JP2H super box containing IHDR + COLR
    let mut inner = VecWriter::new();

    // IHDR sub-box
    write_box_header(&mut inner, JP2_IHDR, 14);
    write_ihdr(&mut inner, &IhdrBox {
        height: 100,
        width: 200,
        num_comps: 1,
        bpc: 7,
        compression: 7,
        unk_colorspace: 0,
        ipr: 0,
    });

    // COLR sub-box
    let colr_payload = 7u64;
    write_box_header(&mut inner, JP2_COLR, colr_payload);
    write_colr(&mut inner, &ColrBox {
        method: 1,
        precedence: 0,
        approx: 0,
        enum_cs: Some(CS_GRAYSCALE),
        icc_profile: None,
    });

    let inner_data = inner.into_vec();

    // Wrap in JP2H
    let mut outer = VecWriter::new();
    write_box_header(&mut outer, JP2_JP2H, inner_data.len() as u64);
    outer.write_bytes(&inner_data);

    let data = outer.into_vec();
    let mut reader = SliceReader::new(&data);

    // Parse JP2H header
    let jp2h = read_box_header(&mut reader).unwrap();
    assert_eq!(jp2h.box_type, JP2_JP2H);

    let jp2h_end = reader.tell() + (jp2h.length as usize - jp2h.header_size as usize);

    // Parse IHDR sub-box
    let ihdr_hdr = read_box_header(&mut reader).unwrap();
    assert_eq!(ihdr_hdr.box_type, JP2_IHDR);
    let ihdr = read_ihdr(&mut reader).unwrap();
    assert_eq!(ihdr.width, 200);
    assert_eq!(ihdr.height, 100);
    assert_eq!(ihdr.num_comps, 1);

    // Parse COLR sub-box
    let colr_hdr = read_box_header(&mut reader).unwrap();
    assert_eq!(colr_hdr.box_type, JP2_COLR);
    let content_len = colr_hdr.length as usize - colr_hdr.header_size as usize;
    let colr = read_colr(&mut reader, content_len).unwrap();
    assert_eq!(colr.enum_cs, Some(CS_GRAYSCALE));

    assert_eq!(reader.tell(), jp2h_end);
}

#[test]
fn encode_minimal_jp2() {
    let (pixels, comp_info, params) = make_grayscale_8x8();
    let jp2_data = jp2::jp2_encode(&pixels, &comp_info, &params).unwrap();

    // Verify it starts with JP signature box
    let mut reader = SliceReader::new(&jp2_data);
    let jp_hdr = read_box_header(&mut reader).unwrap();
    assert_eq!(jp_hdr.box_type, JP2_JP);
    let sig = reader.read_u32_be().unwrap();
    assert_eq!(sig, JP2_SIGNATURE);

    // Next is FTYP
    let ftyp_hdr = read_box_header(&mut reader).unwrap();
    assert_eq!(ftyp_hdr.box_type, JP2_FTYP);
}

#[test]
fn decode_minimal_jp2() {
    let (pixels, comp_info, params) = make_grayscale_8x8();
    let jp2_data = jp2::jp2_encode(&pixels, &comp_info, &params).unwrap();
    let (decoded, dec_info) = jp2::jp2_decode(&jp2_data).unwrap();

    assert_eq!(dec_info.len(), 1);
    assert_eq!(dec_info[0].width, 8);
    assert_eq!(dec_info[0].height, 8);
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].len(), 64);
}

#[test]
fn encode_decode_roundtrip() {
    let (pixels, comp_info, params) = make_grayscale_8x8();
    let jp2_data = jp2::jp2_encode(&pixels, &comp_info, &params).unwrap();
    let (decoded, dec_info) = jp2::jp2_decode(&jp2_data).unwrap();

    assert_eq!(dec_info.len(), 1);
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].len(), pixels[0].len());

    // Reversible transform roundtrip: max error <= 1 (due to DC level shift rounding)
    let max_err = pixels[0]
        .iter()
        .zip(decoded[0].iter())
        .map(|(&o, &d)| (o - d).abs())
        .max()
        .unwrap_or(0);
    assert!(
        max_err <= 1,
        "reversible roundtrip max error should be <= 1, got {max_err}"
    );
}

#[test]
fn color_space_srgb() {
    let (pixels, comp_info, params) = make_rgb_8x8();
    let jp2_data = jp2::jp2_encode(&pixels, &comp_info, &params).unwrap();

    // Parse to find COLR box
    let mut reader = SliceReader::new(&jp2_data);

    // Skip through boxes to find JP2H -> COLR
    loop {
        if reader.remaining() < 8 {
            panic!("COLR box not found");
        }
        let hdr = read_box_header(&mut reader).unwrap();
        let content_len = hdr.length as usize - hdr.header_size as usize;

        if hdr.box_type == JP2_COLR {
            let colr = read_colr(&mut reader, content_len).unwrap();
            assert_eq!(colr.method, 1);
            assert_eq!(colr.enum_cs, Some(CS_SRGB));
            return;
        } else if hdr.box_type == JP2_JP2H {
            // Don't skip -- parse sub-boxes
            continue;
        } else {
            reader.skip(content_len).unwrap();
        }
    }
}

#[test]
fn color_space_grayscale() {
    let (pixels, comp_info, params) = make_grayscale_8x8();
    let jp2_data = jp2::jp2_encode(&pixels, &comp_info, &params).unwrap();

    // Parse to find COLR box
    let mut reader = SliceReader::new(&jp2_data);

    loop {
        if reader.remaining() < 8 {
            panic!("COLR box not found");
        }
        let hdr = read_box_header(&mut reader).unwrap();
        let content_len = hdr.length as usize - hdr.header_size as usize;

        if hdr.box_type == JP2_COLR {
            let colr = read_colr(&mut reader, content_len).unwrap();
            assert_eq!(colr.method, 1);
            assert_eq!(colr.enum_cs, Some(CS_GRAYSCALE));
            return;
        } else if hdr.box_type == JP2_JP2H {
            continue;
        } else {
            reader.skip(content_len).unwrap();
        }
    }
}
