/// Phase 9: J2K Codestream encoder/decoder.
///
/// Wraps TCD (Tile Coder/Decoder) with J2K marker segment framing.
/// Supports multi-tile images and reduced resolution decoding.

use rayon::prelude::*;

use crate::error::{Jp2Error, Result};
use crate::marker::*;
use crate::stream::{SliceReader, VecWriter};
use crate::tcd::{self, EncodedTile, TcdComponent, TcdParams, TileData};

/// J2K codestream header information (parsed from SIZ + COD + QCD).
#[derive(Debug, Clone)]
pub struct J2kHeader {
    pub siz: SizMarker,
    pub cod: CodMarker,
    pub qcd: QcdMarker,
}

/// Encode an image as a J2K codestream (supports multi-tile).
///
/// # Arguments
/// * `components` - Per-component sample arrays (row-major, full image)
/// * `comp_info` - Component metadata (width, height, precision, etc.)
/// * `params` - Coding parameters (decomposition levels, code-block size, etc.)
///
/// # Returns
/// A `Vec<u8>` containing the complete J2K codestream.
pub fn j2k_encode(
    components: &[Vec<i32>],
    comp_info: &[TcdComponent],
    params: &TcdParams,
) -> Result<Vec<u8>> {
    let num_comps = comp_info.len();
    if components.len() != num_comps || num_comps == 0 {
        return Err(Jp2Error::InvalidData(
            "component count mismatch or zero components".to_string(),
        ));
    }

    let width = comp_info[0].width;
    let height = comp_info[0].height;

    // Build SIZ marker
    let siz = build_siz(comp_info, width, height, params);

    // Calculate tile grid
    let tile_width = siz.tile_width;
    let tile_height = siz.tile_height;
    let tile_x_offset = siz.tile_x_offset;
    let tile_y_offset = siz.tile_y_offset;
    let x_offset = siz.x_offset;
    let y_offset = siz.y_offset;

    let num_tiles_x = ceil_div(width - tile_x_offset, tile_width);
    let num_tiles_y = ceil_div(height - tile_y_offset, tile_height);
    let total_tiles = num_tiles_x * num_tiles_y;

    // Build COD marker
    let cod = build_cod(params);

    // Build QCD marker
    let qcd = build_qcd(params);

    // Assemble codestream
    let mut writer = VecWriter::new();

    // 1. SOC
    writer.write_u16_be(SOC);

    // 2. SIZ
    write_siz(&mut writer, &siz);

    // 3. COD
    write_cod(&mut writer, &cod);

    // 4. QCD
    write_qcd(&mut writer, &qcd);

    // 5. Collect tile descriptions, encode in parallel, then write sequentially
    let mut tile_descs: Vec<(u32, u32, u32, u32)> = Vec::new();
    for ty in 0..num_tiles_y {
        for tx in 0..num_tiles_x {
            let t_x0 = (tile_x_offset + tx * tile_width).max(x_offset);
            let t_y0 = (tile_y_offset + ty * tile_height).max(y_offset);
            let t_x1 = (tile_x_offset + (tx + 1) * tile_width).min(width);
            let t_y1 = (tile_y_offset + (ty + 1) * tile_height).min(height);
            tile_descs.push((t_x0, t_y0, t_x1, t_y1));
        }
    }

    // Encode all tiles in parallel using rayon
    let encoded_tiles: std::result::Result<Vec<EncodedTile>, Jp2Error> = tile_descs
        .par_iter()
        .map(|&(t_x0, t_y0, t_x1, t_y1)| {
            let t_w = t_x1 - t_x0;
            let t_h = t_y1 - t_y0;

            let mut tile_comps = Vec::with_capacity(num_comps);
            let mut tile_comp_info = Vec::with_capacity(num_comps);
            for ci in 0..num_comps {
                let comp_w = comp_info[ci].width;
                let mut tile_samples = vec![0i32; (t_w * t_h) as usize];
                for y in 0..t_h {
                    for x in 0..t_w {
                        let src_x = t_x0 + x;
                        let src_y = t_y0 + y;
                        tile_samples[(y * t_w + x) as usize] =
                            components[ci][(src_y * comp_w + src_x) as usize];
                    }
                }
                tile_comps.push(tile_samples);
                tile_comp_info.push(TcdComponent {
                    width: t_w,
                    height: t_h,
                    precision: comp_info[ci].precision,
                    signed: comp_info[ci].signed,
                    dx: comp_info[ci].dx,
                    dy: comp_info[ci].dy,
                });
            }

            let tile_data = TileData {
                components: tile_comps,
                width: t_w,
                height: t_h,
            };
            tcd::encode_tile(&tile_data, &tile_comp_info, params)
        })
        .collect();

    let encoded_tiles = encoded_tiles?;
    assert_eq!(encoded_tiles.len(), total_tiles as usize);

    // Write tiles sequentially to maintain correct ordering
    for (tile_index, encoded_tile) in encoded_tiles.iter().enumerate() {
        let tile_part_len = 2 + 2 + 8 + 2 + encoded_tile.data.len() as u32;

        let sot = SotMarker {
            tile_index: tile_index as u16,
            tile_part_len,
            tile_part_no: 0,
            num_tile_parts: 1,
        };
        write_sot(&mut writer, &sot);
        writer.write_u16_be(SOD);
        writer.write_bytes(&encoded_tile.data);
    }

    // 6. EOC
    writer.write_u16_be(EOC);

    Ok(writer.into_vec())
}

/// Encode an image as a J2K codestream with explicit tile dimensions.
///
/// Same as `j2k_encode` but allows specifying tile width/height via SIZ
/// parameters embedded in the comp_info and params.
pub fn j2k_encode_tiled(
    components: &[Vec<i32>],
    comp_info: &[TcdComponent],
    params: &TcdParams,
    tile_width: u32,
    tile_height: u32,
) -> Result<Vec<u8>> {
    let num_comps = comp_info.len();
    if components.len() != num_comps || num_comps == 0 {
        return Err(Jp2Error::InvalidData(
            "component count mismatch or zero components".to_string(),
        ));
    }

    let width = comp_info[0].width;
    let height = comp_info[0].height;

    // Build SIZ marker with specified tile dimensions
    let siz = build_siz_tiled(comp_info, width, height, tile_width, tile_height);

    // Calculate tile grid
    let num_tiles_x = ceil_div(width - siz.tile_x_offset, tile_width);
    let num_tiles_y = ceil_div(height - siz.tile_y_offset, tile_height);

    // Build COD marker
    let cod = build_cod(params);

    // Build QCD marker
    let qcd = build_qcd(params);

    // Assemble codestream
    let mut writer = VecWriter::new();

    // 1. SOC
    writer.write_u16_be(SOC);

    // 2. SIZ
    write_siz(&mut writer, &siz);

    // 3. COD
    write_cod(&mut writer, &cod);

    // 4. QCD
    write_qcd(&mut writer, &qcd);

    // 5. Collect tile descriptions, encode in parallel, then write sequentially
    let mut tile_descs: Vec<(u32, u32, u32, u32)> = Vec::new();
    for ty in 0..num_tiles_y {
        for tx in 0..num_tiles_x {
            let t_x0 = (siz.tile_x_offset + tx * tile_width).max(siz.x_offset);
            let t_y0 = (siz.tile_y_offset + ty * tile_height).max(siz.y_offset);
            let t_x1 = (siz.tile_x_offset + (tx + 1) * tile_width).min(width);
            let t_y1 = (siz.tile_y_offset + (ty + 1) * tile_height).min(height);
            tile_descs.push((t_x0, t_y0, t_x1, t_y1));
        }
    }

    // Encode all tiles in parallel using rayon
    let encoded_tiles: std::result::Result<Vec<EncodedTile>, Jp2Error> = tile_descs
        .par_iter()
        .map(|&(t_x0, t_y0, t_x1, t_y1)| {
            let t_w = t_x1 - t_x0;
            let t_h = t_y1 - t_y0;

            let mut tile_comps = Vec::with_capacity(num_comps);
            let mut tile_comp_info = Vec::with_capacity(num_comps);
            for ci in 0..num_comps {
                let comp_w = comp_info[ci].width;
                let mut tile_samples = vec![0i32; (t_w * t_h) as usize];
                for y in 0..t_h {
                    for x in 0..t_w {
                        let src_x = t_x0 + x;
                        let src_y = t_y0 + y;
                        tile_samples[(y * t_w + x) as usize] =
                            components[ci][(src_y * comp_w + src_x) as usize];
                    }
                }
                tile_comps.push(tile_samples);
                tile_comp_info.push(TcdComponent {
                    width: t_w,
                    height: t_h,
                    precision: comp_info[ci].precision,
                    signed: comp_info[ci].signed,
                    dx: comp_info[ci].dx,
                    dy: comp_info[ci].dy,
                });
            }

            let tile_data = TileData {
                components: tile_comps,
                width: t_w,
                height: t_h,
            };
            tcd::encode_tile(&tile_data, &tile_comp_info, params)
        })
        .collect();

    let encoded_tiles = encoded_tiles?;

    // Write tiles sequentially to maintain correct ordering
    for (tile_index, encoded_tile) in encoded_tiles.iter().enumerate() {
        let tile_part_len = 2 + 2 + 8 + 2 + encoded_tile.data.len() as u32;

        let sot = SotMarker {
            tile_index: tile_index as u16,
            tile_part_len,
            tile_part_no: 0,
            num_tile_parts: 1,
        };
        write_sot(&mut writer, &sot);
        writer.write_u16_be(SOD);
        writer.write_bytes(&encoded_tile.data);
    }

    // 6. EOC
    writer.write_u16_be(EOC);

    Ok(writer.into_vec())
}

/// Decode a J2K codestream (supports multi-tile).
///
/// # Returns
/// A tuple of (per-component samples, component info).
pub fn j2k_decode(data: &[u8]) -> Result<(Vec<Vec<i32>>, Vec<TcdComponent>)> {
    j2k_decode_with_reduce(data, 0)
}

/// Decode a J2K codestream at a reduced resolution.
///
/// # Arguments
/// * `data` - The J2K codestream bytes
/// * `reduce` - Number of resolution levels to discard (0 = full resolution)
///
/// # Returns
/// A tuple of (per-component samples, component info) at the reduced size.
pub fn j2k_decode_with_reduce(
    data: &[u8],
    reduce: u32,
) -> Result<(Vec<Vec<i32>>, Vec<TcdComponent>)> {
    let header = j2k_read_header(data)?;

    let siz = &header.siz;
    let width = siz.width;
    let height = siz.height;
    let tile_width = siz.tile_width;
    let tile_height = siz.tile_height;
    let tile_x_offset = siz.tile_x_offset;
    let tile_y_offset = siz.tile_y_offset;
    let x_offset = siz.x_offset;
    let y_offset = siz.y_offset;
    let num_comps = siz.num_comps as usize;

    let num_tiles_x = ceil_div(width - tile_x_offset, tile_width);
    let _num_tiles_y = ceil_div(height - tile_y_offset, tile_height);

    // Compute output dimensions with reduce
    let (out_w, out_h) = tcd::resolution_size(width, height, reduce);

    // Build params from header
    let mut params = header_to_params(&header);
    params.reduce = reduce;

    // Initialize output component buffers at reduced resolution
    let comp_info = siz_to_components(siz);
    let mut output_comps: Vec<Vec<i32>> = (0..num_comps)
        .map(|_| vec![0i32; (out_w * out_h) as usize])
        .collect();

    // Re-parse to find tile data
    let mut reader = SliceReader::new(data);

    // Skip SOC
    let soc = read_marker(&mut reader)?;
    if soc != SOC {
        return Err(Jp2Error::InvalidMarker(soc));
    }

    // Read markers until first SOT
    loop {
        let marker = read_marker(&mut reader)?;
        match marker {
            SIZ => {
                let _ = read_siz(&mut reader)?;
            }
            COD => {
                let _ = read_cod(&mut reader)?;
            }
            QCD => {
                let _ = read_qcd(&mut reader)?;
            }
            COM => {
                let len = reader.read_u16_be()? as usize;
                if len >= 2 {
                    reader.skip(len - 2)?;
                }
            }
            SOT => {
                break;
            }
            _ => {
                let len = reader.read_u16_be()? as usize;
                if len >= 2 {
                    reader.skip(len - 2)?;
                }
            }
        }
    }

    // Now read tiles in a loop
    let mut first_sot = true;
    loop {
        let sot = if first_sot {
            first_sot = false;
            read_sot(&mut reader)?
        } else {
            // Read next marker - should be SOT or EOC
            if reader.remaining() < 2 {
                break;
            }
            let marker = read_marker(&mut reader)?;
            if marker == EOC {
                break;
            }
            if marker != SOT {
                return Err(Jp2Error::InvalidMarker(marker));
            }
            read_sot(&mut reader)?
        };

        // Read SOD
        let sod = read_marker(&mut reader)?;
        if sod != SOD {
            return Err(Jp2Error::InvalidMarker(sod));
        }

        // Extract tile data
        let tile_data_len = if sot.tile_part_len > 14 {
            (sot.tile_part_len - 14) as usize
        } else {
            let remaining = reader.remaining();
            if remaining >= 2 {
                remaining - 2
            } else {
                remaining
            }
        };

        let tile_bytes = reader.read_bytes(tile_data_len)?;

        // Compute tile position in the grid
        let ti = sot.tile_index as u32;
        let tx = ti % num_tiles_x;
        let ty = ti / num_tiles_x;

        // Compute tile bounds in full image coords
        let t_x0 = (tile_x_offset + tx * tile_width).max(x_offset);
        let t_y0 = (tile_y_offset + ty * tile_height).max(y_offset);
        let t_x1 = (tile_x_offset + (tx + 1) * tile_width).min(width);
        let t_y1 = (tile_y_offset + (ty + 1) * tile_height).min(height);
        let t_w = t_x1 - t_x0;
        let t_h = t_y1 - t_y0;

        // Build tile-specific component info
        let tile_comp_info: Vec<TcdComponent> = comp_info
            .iter()
            .map(|c| TcdComponent {
                width: t_w,
                height: t_h,
                precision: c.precision,
                signed: c.signed,
                dx: c.dx,
                dy: c.dy,
            })
            .collect();

        let encoded = EncodedTile {
            data: tile_bytes.to_vec(),
            numbps: vec![0; num_comps],
        };

        let decoded = tcd::decode_tile(&encoded, &tile_comp_info, &params, t_w, t_h)?;

        // Place decoded tile data into the full output at the correct position
        // After reduce, tile output dimensions are reduced proportionally
        let (tile_out_w, tile_out_h) = (decoded.width, decoded.height);

        // Compute tile placement in the output (reduced) image
        let (out_t_x0, out_t_y0) = if reduce > 0 {
            let (rx0, ry0) = tile_reduced_origin(t_x0, t_y0, reduce);
            (rx0, ry0)
        } else {
            (t_x0, t_y0)
        };

        for ci in 0..num_comps {
            for y in 0..tile_out_h {
                for x in 0..tile_out_w {
                    let dst_x = out_t_x0 + x;
                    let dst_y = out_t_y0 + y;
                    if dst_x < out_w && dst_y < out_h {
                        output_comps[ci][(dst_y * out_w + dst_x) as usize] =
                            decoded.components[ci][(y * tile_out_w + x) as usize];
                    }
                }
            }
        }
    }

    // Build output component info with reduced dimensions
    let out_comp_info: Vec<TcdComponent> = comp_info
        .iter()
        .map(|c| TcdComponent {
            width: out_w,
            height: out_h,
            precision: c.precision,
            signed: c.signed,
            dx: c.dx,
            dy: c.dy,
        })
        .collect();

    Ok((output_comps, out_comp_info))
}

/// Read just the J2K header (SIZ + COD + QCD) without decoding tile data.
pub fn j2k_read_header(data: &[u8]) -> Result<J2kHeader> {
    let mut reader = SliceReader::new(data);

    // Read SOC
    let soc = read_marker(&mut reader)?;
    if soc != SOC {
        return Err(Jp2Error::InvalidData(format!(
            "expected SOC (0xFF4F), got 0x{soc:04X}"
        )));
    }

    let mut siz: Option<SizMarker> = None;
    let mut cod: Option<CodMarker> = None;
    let mut qcd: Option<QcdMarker> = None;

    // Read markers until SOT or SOD
    loop {
        if reader.remaining() < 2 {
            break;
        }
        let marker = read_marker(&mut reader)?;
        match marker {
            SIZ => {
                siz = Some(read_siz(&mut reader)?);
            }
            COD => {
                cod = Some(read_cod(&mut reader)?);
            }
            QCD => {
                qcd = Some(read_qcd(&mut reader)?);
            }
            COM => {
                let len = reader.read_u16_be()? as usize;
                if len >= 2 {
                    reader.skip(len - 2)?;
                }
            }
            SOT | SOD | EOC => {
                break;
            }
            _ => {
                // Skip unknown marker segment
                if reader.remaining() >= 2 {
                    let len = reader.read_u16_be()? as usize;
                    if len >= 2 {
                        reader.skip(len - 2)?;
                    }
                }
            }
        }
    }

    let siz = siz.ok_or_else(|| Jp2Error::InvalidData("missing SIZ marker".to_string()))?;
    let cod = cod.ok_or_else(|| Jp2Error::InvalidData("missing COD marker".to_string()))?;
    let qcd = qcd.ok_or_else(|| Jp2Error::InvalidData("missing QCD marker".to_string()))?;

    Ok(J2kHeader { siz, cod, qcd })
}

// ── Helper functions ──

/// Ceiling division.
fn ceil_div(a: u32, b: u32) -> u32 {
    (a + b - 1) / b
}

/// Compute the origin of a tile in reduced-resolution output coordinates.
fn tile_reduced_origin(t_x0: u32, t_y0: u32, reduce: u32) -> (u32, u32) {
    let mut rx = t_x0;
    let mut ry = t_y0;
    for _ in 0..reduce {
        rx = (rx + 1) / 2;
        ry = (ry + 1) / 2;
    }
    (rx, ry)
}

/// Build a SIZ marker from component info (single tile covering entire image).
fn build_siz(comp_info: &[TcdComponent], width: u32, height: u32, _params: &TcdParams) -> SizMarker {
    let comps: Vec<SizComp> = comp_info
        .iter()
        .map(|c| {
            let ssiz = if c.signed {
                0x80 | ((c.precision - 1) as u8)
            } else {
                (c.precision - 1) as u8
            };
            SizComp {
                precision: ssiz,
                dx: c.dx as u8,
                dy: c.dy as u8,
            }
        })
        .collect();

    SizMarker {
        profile: 0, // unrestricted
        width,
        height,
        x_offset: 0,
        y_offset: 0,
        tile_width: width,  // single tile
        tile_height: height,
        tile_x_offset: 0,
        tile_y_offset: 0,
        num_comps: comp_info.len() as u16,
        comps,
    }
}

/// Build a SIZ marker with explicit tile dimensions.
fn build_siz_tiled(
    comp_info: &[TcdComponent],
    width: u32,
    height: u32,
    tile_width: u32,
    tile_height: u32,
) -> SizMarker {
    let comps: Vec<SizComp> = comp_info
        .iter()
        .map(|c| {
            let ssiz = if c.signed {
                0x80 | ((c.precision - 1) as u8)
            } else {
                (c.precision - 1) as u8
            };
            SizComp {
                precision: ssiz,
                dx: c.dx as u8,
                dy: c.dy as u8,
            }
        })
        .collect();

    SizMarker {
        profile: 0,
        width,
        height,
        x_offset: 0,
        y_offset: 0,
        tile_width,
        tile_height,
        tile_x_offset: 0,
        tile_y_offset: 0,
        num_comps: comp_info.len() as u16,
        comps,
    }
}

/// Build a COD marker from TcdParams.
fn build_cod(params: &TcdParams) -> CodMarker {
    let num_decomp = if params.num_res > 0 {
        (params.num_res - 1) as u8
    } else {
        0
    };

    // Code-block exponent: cblk_w = 2^(exp+2), so exp = log2(cblk_w) - 2
    let cblk_width_exp = log2_u32(params.cblk_w).saturating_sub(2) as u8;
    let cblk_height_exp = log2_u32(params.cblk_h).saturating_sub(2) as u8;

    CodMarker {
        coding_style: 0,
        prog_order: 0, // LRCP
        num_layers: params.num_layers as u16,
        mct: if params.use_mct { 1 } else { 0 },
        num_decomp,
        cblk_width_exp,
        cblk_height_exp,
        cblk_style: 0,
        transform: if params.reversible { 1 } else { 0 },
    }
}

/// Build a QCD marker for the given parameters.
fn build_qcd(params: &TcdParams) -> QcdMarker {
    let num_decomp = if params.num_res > 0 {
        params.num_res - 1
    } else {
        0
    };

    if params.reversible {
        // No quantization (style 0). One step size per subband.
        let num_bands = if num_decomp > 0 {
            1 + 3 * num_decomp
        } else {
            1
        };
        let stepsizes = vec![0u16; num_bands as usize];
        QcdMarker {
            quant_style: 0,
            stepsizes,
        }
    } else {
        // Scalar explicit quantization (style 2)
        let num_bands = if num_decomp > 0 {
            1 + 3 * num_decomp
        } else {
            1
        };
        let stepsizes = vec![0x0020u16; num_bands as usize];
        QcdMarker {
            quant_style: 0x02,
            stepsizes,
        }
    }
}

/// Convert SIZ marker to TcdComponent array.
fn siz_to_components(siz: &SizMarker) -> Vec<TcdComponent> {
    siz.comps
        .iter()
        .map(|c| {
            let signed = (c.precision & 0x80) != 0;
            let precision = (c.precision & 0x7F) as u32 + 1;
            TcdComponent {
                width: siz.width,
                height: siz.height,
                precision,
                signed,
                dx: c.dx as u32,
                dy: c.dy as u32,
            }
        })
        .collect()
}

/// Convert J2kHeader to TcdParams.
fn header_to_params(header: &J2kHeader) -> TcdParams {
    let num_res = header.cod.num_decomp as u32 + 1;
    let cblk_w = 1u32 << (header.cod.cblk_width_exp as u32 + 2);
    let cblk_h = 1u32 << (header.cod.cblk_height_exp as u32 + 2);
    TcdParams {
        num_res,
        cblk_w,
        cblk_h,
        reversible: header.cod.transform == 1,
        num_layers: header.cod.num_layers as u32,
        use_mct: header.cod.mct != 0,
        reduce: 0,
        max_bytes: None,
    }
}

/// Integer log2 for powers of 2.
fn log2_u32(v: u32) -> u32 {
    if v == 0 {
        return 0;
    }
    31 - v.leading_zeros()
}
