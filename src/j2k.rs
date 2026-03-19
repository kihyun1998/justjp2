/// Phase 9: J2K Codestream encoder/decoder.
///
/// Wraps TCD (Tile Coder/Decoder) with J2K marker segment framing.
/// This implementation supports a single tile covering the entire image.

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

/// Encode an image as a J2K codestream.
///
/// # Arguments
/// * `components` - Per-component sample arrays (row-major)
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
    let siz = build_siz(comp_info, width, height);

    // Build COD marker
    let cod = build_cod(params);

    // Build QCD marker
    let qcd = build_qcd(params);

    // Encode tile data via TCD
    let tile_data = TileData {
        components: components.to_vec(),
        width,
        height,
    };
    let encoded_tile = tcd::encode_tile(&tile_data, comp_info, params)?;

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

    // 5. SOT + SOD + tile data
    // Compute tile-part length: SOT marker (2) + Lsot (2) + SOT payload (8) +
    //                           SOD marker (2) + tile data length
    let tile_part_len = 2 + 2 + 8 + 2 + encoded_tile.data.len() as u32;

    let sot = SotMarker {
        tile_index: 0,
        tile_part_len,
        tile_part_no: 0,
        num_tile_parts: 1,
    };
    write_sot(&mut writer, &sot);

    // 6. SOD
    writer.write_u16_be(SOD);

    // 7. Tile data
    writer.write_bytes(&encoded_tile.data);

    // 8. EOC
    writer.write_u16_be(EOC);

    Ok(writer.into_vec())
}

/// Decode a J2K codestream.
///
/// # Returns
/// A tuple of (per-component samples, component info).
pub fn j2k_decode(data: &[u8]) -> Result<(Vec<Vec<i32>>, Vec<TcdComponent>)> {
    let header = j2k_read_header(data)?;

    // Re-parse to find tile data
    let mut reader = SliceReader::new(data);

    // Skip SOC
    let soc = read_marker(&mut reader)?;
    if soc != SOC {
        return Err(Jp2Error::InvalidMarker(soc));
    }

    // Read markers until SOT
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
                // Skip comment marker
                let len = reader.read_u16_be()? as usize;
                if len >= 2 {
                    reader.skip(len - 2)?;
                }
            }
            SOT => {
                break;
            }
            _ => {
                // Skip unknown marker segment
                let len = reader.read_u16_be()? as usize;
                if len >= 2 {
                    reader.skip(len - 2)?;
                }
            }
        }
    }

    // We just read SOT marker code; now read SOT payload
    let sot = read_sot(&mut reader)?;

    // Read SOD
    let sod = read_marker(&mut reader)?;
    if sod != SOD {
        return Err(Jp2Error::InvalidMarker(sod));
    }

    // Extract tile data: from current position to either tile_part_len boundary or EOC
    let _tile_data_start = reader.tell();
    // tile_part_len counts from SOT marker start.
    // SOT marker (2) + Lsot(2) + Isot(2) + Psot(4) + TPsot(1) + TNsot(1) + SOD(2) = 14
    // tile data = tile_part_len - 14
    let tile_data_len = if sot.tile_part_len > 14 {
        (sot.tile_part_len - 14) as usize
    } else {
        // tile_part_len == 0 means rest of codestream (minus EOC)
        let remaining = reader.remaining();
        if remaining >= 2 {
            remaining - 2 // strip EOC
        } else {
            remaining
        }
    };

    let tile_bytes = reader.read_bytes(tile_data_len)?;

    // Reconstruct TcdComponent and TcdParams from header
    let comp_info = siz_to_components(&header.siz);
    let params = header_to_params(&header);

    let encoded = EncodedTile {
        data: tile_bytes.to_vec(),
        numbps: vec![0; comp_info.len()], // not used by decode_tile
    };

    let decoded = tcd::decode_tile(
        &encoded,
        &comp_info,
        &params,
        header.siz.width,
        header.siz.height,
    )?;

    Ok((decoded.components, comp_info))
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

/// Build a SIZ marker from component info.
fn build_siz(comp_info: &[TcdComponent], width: u32, height: u32) -> SizMarker {
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
        // Number of subbands = 1 + 3 * num_decomp (for num_decomp > 0), or 1.
        let num_bands = if num_decomp > 0 {
            1 + 3 * num_decomp
        } else {
            1
        };
        // For no-quantization, step sizes are exponent values (1 byte each).
        // Use a simple default: precision + gain for each band.
        let stepsizes = vec![0u16; num_bands as usize];
        QcdMarker {
            quant_style: 0, // no quantization, 0 guard bits
            stepsizes,
        }
    } else {
        // Scalar explicit quantization (style 2)
        let num_bands = if num_decomp > 0 {
            1 + 3 * num_decomp
        } else {
            1
        };
        let stepsizes = vec![0x0020u16; num_bands as usize]; // default step size
        QcdMarker {
            quant_style: 0x02, // scalar explicit
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
    }
}

/// Integer log2 for powers of 2.
fn log2_u32(v: u32) -> u32 {
    if v == 0 {
        return 0;
    }
    31 - v.leading_zeros()
}
