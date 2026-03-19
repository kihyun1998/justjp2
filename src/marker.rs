/// Phase 9: JPEG 2000 marker segment definitions and read/write functions.

use crate::error::{Jp2Error, Result};
use crate::stream::{SliceReader, VecWriter};

// ── Marker codes (2 bytes each) ──

pub const SOC: u16 = 0xFF4F; // Start of codestream
pub const SOT: u16 = 0xFF90; // Start of tile-part
pub const SOD: u16 = 0xFF93; // Start of data
pub const EOC: u16 = 0xFFD9; // End of codestream
pub const SIZ: u16 = 0xFF51; // Image and tile size
pub const COD: u16 = 0xFF52; // Coding style default
pub const COC: u16 = 0xFF53; // Coding style component
pub const QCD: u16 = 0xFF5C; // Quantization default
pub const QCC: u16 = 0xFF5D; // Quantization component
pub const POC: u16 = 0xFF5F; // Progression order change
pub const COM: u16 = 0xFF64; // Comment
pub const SOP: u16 = 0xFF91; // Start of packet
pub const EPH: u16 = 0xFF92; // End of packet header

// ── SIZ marker data ──

#[derive(Debug, Clone)]
pub struct SizMarker {
    pub profile: u16,       // Rsiz
    pub width: u32,         // Xsiz - reference grid width
    pub height: u32,        // Ysiz - reference grid height
    pub x_offset: u32,      // XOsiz
    pub y_offset: u32,      // YOsiz
    pub tile_width: u32,    // XTsiz
    pub tile_height: u32,   // YTsiz
    pub tile_x_offset: u32, // XTOsiz
    pub tile_y_offset: u32, // YTOsiz
    pub num_comps: u16,     // Csiz
    pub comps: Vec<SizComp>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizComp {
    pub precision: u8, // Ssiz: bit 7 = signed, bits 0-6 = precision-1
    pub dx: u8,        // XRsiz: horizontal sub-sampling
    pub dy: u8,        // YRsiz: vertical sub-sampling
}

// ── COD marker data ──

#[derive(Debug, Clone)]
pub struct CodMarker {
    pub coding_style: u8,   // Scod
    pub prog_order: u8,     // SGcod: progression order
    pub num_layers: u16,    // SGcod: number of layers
    pub mct: u8,            // SGcod: multiple component transform (0 or 1)
    pub num_decomp: u8,     // SPcod: number of decomposition levels
    pub cblk_width_exp: u8, // SPcod: code-block width exponent offset (actual = 2^(exp+2))
    pub cblk_height_exp: u8, // SPcod: code-block height exponent offset
    pub cblk_style: u8,     // SPcod: code-block coding style
    pub transform: u8,      // SPcod: wavelet transform (0=9/7, 1=5/3)
}

// ── QCD marker data ──

#[derive(Debug, Clone)]
pub struct QcdMarker {
    pub quant_style: u8,     // Sqcd: quantization style (bits 0-4) + guard bits (bits 5-7)
    pub stepsizes: Vec<u16>, // SPqcd: step sizes
}

// ── Read functions ──

/// Read a 2-byte marker code from the stream.
pub fn read_marker(reader: &mut SliceReader) -> Result<u16> {
    let marker = reader.read_u16_be()?;
    if marker & 0xFF00 != 0xFF00 {
        return Err(Jp2Error::InvalidMarker(marker));
    }
    Ok(marker)
}

/// Read SIZ marker segment (after marker code has been consumed).
/// Reads the length field, then the SIZ payload.
pub fn read_siz(reader: &mut SliceReader) -> Result<SizMarker> {
    let length = reader.read_u16_be()? as usize; // Lsiz
    if length < 41 {
        return Err(Jp2Error::InvalidData("SIZ marker too short".to_string()));
    }

    let profile = reader.read_u16_be()?;
    let width = reader.read_u32_be()?;
    let height = reader.read_u32_be()?;
    let x_offset = reader.read_u32_be()?;
    let y_offset = reader.read_u32_be()?;
    let tile_width = reader.read_u32_be()?;
    let tile_height = reader.read_u32_be()?;
    let tile_x_offset = reader.read_u32_be()?;
    let tile_y_offset = reader.read_u32_be()?;
    let num_comps = reader.read_u16_be()?;

    if num_comps == 0 {
        return Err(Jp2Error::InvalidData(
            "SIZ: zero components".to_string(),
        ));
    }

    // Expected length: 38 + 3 * num_comps + 2 (Lsiz field itself counted in length)
    let expected = 38 + 3 * num_comps as usize;
    if length < expected {
        return Err(Jp2Error::InvalidData(format!(
            "SIZ marker length mismatch: got {length}, expected at least {expected}"
        )));
    }

    let mut comps = Vec::with_capacity(num_comps as usize);
    for _ in 0..num_comps {
        let precision = reader.read_u8()?;
        let dx = reader.read_u8()?;
        let dy = reader.read_u8()?;
        comps.push(SizComp { precision, dx, dy });
    }

    Ok(SizMarker {
        profile,
        width,
        height,
        x_offset,
        y_offset,
        tile_width,
        tile_height,
        tile_x_offset,
        tile_y_offset,
        num_comps,
        comps,
    })
}

/// Write SIZ marker segment (marker code + length + payload).
pub fn write_siz(writer: &mut VecWriter, siz: &SizMarker) {
    writer.write_u16_be(SIZ);
    let length = 38 + 3 * siz.num_comps as u16;
    writer.write_u16_be(length); // Lsiz
    writer.write_u16_be(siz.profile);
    writer.write_u32_be(siz.width);
    writer.write_u32_be(siz.height);
    writer.write_u32_be(siz.x_offset);
    writer.write_u32_be(siz.y_offset);
    writer.write_u32_be(siz.tile_width);
    writer.write_u32_be(siz.tile_height);
    writer.write_u32_be(siz.tile_x_offset);
    writer.write_u32_be(siz.tile_y_offset);
    writer.write_u16_be(siz.num_comps);
    for c in &siz.comps {
        writer.write_u8(c.precision);
        writer.write_u8(c.dx);
        writer.write_u8(c.dy);
    }
}

/// Read COD marker segment (after marker code has been consumed).
pub fn read_cod(reader: &mut SliceReader) -> Result<CodMarker> {
    let length = reader.read_u16_be()? as usize; // Lcod
    if length < 12 {
        return Err(Jp2Error::InvalidData("COD marker too short".to_string()));
    }

    let coding_style = reader.read_u8()?;  // Scod
    let prog_order = reader.read_u8()?;     // SGcod: progression order
    let num_layers = reader.read_u16_be()?; // SGcod: number of layers
    let mct = reader.read_u8()?;            // SGcod: MCT
    let num_decomp = reader.read_u8()?;     // SPcod: decomposition levels
    let cblk_width_exp = reader.read_u8()?; // SPcod: cblk width exp
    let cblk_height_exp = reader.read_u8()?; // SPcod: cblk height exp
    let cblk_style = reader.read_u8()?;     // SPcod: cblk style
    let transform = reader.read_u8()?;      // SPcod: wavelet transform

    Ok(CodMarker {
        coding_style,
        prog_order,
        num_layers,
        mct,
        num_decomp,
        cblk_width_exp,
        cblk_height_exp,
        cblk_style,
        transform,
    })
}

/// Write COD marker segment (marker code + length + payload).
pub fn write_cod(writer: &mut VecWriter, cod: &CodMarker) {
    writer.write_u16_be(COD);
    let length: u16 = 12; // Lcod (includes itself): 2 + 10 bytes payload
    writer.write_u16_be(length);
    writer.write_u8(cod.coding_style);
    writer.write_u8(cod.prog_order);
    writer.write_u16_be(cod.num_layers);
    writer.write_u8(cod.mct);
    writer.write_u8(cod.num_decomp);
    writer.write_u8(cod.cblk_width_exp);
    writer.write_u8(cod.cblk_height_exp);
    writer.write_u8(cod.cblk_style);
    writer.write_u8(cod.transform);
}

/// Read QCD marker segment (after marker code has been consumed).
pub fn read_qcd(reader: &mut SliceReader) -> Result<QcdMarker> {
    let length = reader.read_u16_be()? as usize; // Lqcd
    if length < 3 {
        return Err(Jp2Error::InvalidData("QCD marker too short".to_string()));
    }

    let quant_style = reader.read_u8()?; // Sqcd

    let style_bits = quant_style & 0x1F;
    let remaining = length - 3; // subtract Lqcd (2) + Sqcd (1)

    let stepsizes = if style_bits == 0 {
        // No quantization: each step size is 1 byte (exponent only)
        let mut steps = Vec::with_capacity(remaining);
        for _ in 0..remaining {
            steps.push(reader.read_u8()? as u16);
        }
        steps
    } else {
        // Scalar quantization: each step size is 2 bytes
        let num_steps = remaining / 2;
        let mut steps = Vec::with_capacity(num_steps);
        for _ in 0..num_steps {
            steps.push(reader.read_u16_be()?);
        }
        steps
    };

    Ok(QcdMarker {
        quant_style,
        stepsizes,
    })
}

/// Write QCD marker segment (marker code + length + payload).
pub fn write_qcd(writer: &mut VecWriter, qcd: &QcdMarker) {
    writer.write_u16_be(QCD);

    let style_bits = qcd.quant_style & 0x1F;
    let step_bytes = if style_bits == 0 {
        qcd.stepsizes.len() // 1 byte each
    } else {
        qcd.stepsizes.len() * 2 // 2 bytes each
    };
    let length = 3 + step_bytes as u16; // Lqcd(2) + Sqcd(1) + step data
    writer.write_u16_be(length);
    writer.write_u8(qcd.quant_style);

    if style_bits == 0 {
        for &s in &qcd.stepsizes {
            writer.write_u8(s as u8);
        }
    } else {
        for &s in &qcd.stepsizes {
            writer.write_u16_be(s);
        }
    }
}

/// SOT marker data (parsed fields).
#[derive(Debug, Clone)]
pub struct SotMarker {
    pub tile_index: u16,  // Isot
    pub tile_part_len: u32, // Psot: length of tile-part (0 = until EOC)
    pub tile_part_no: u8, // TPsot
    pub num_tile_parts: u8, // TNsot (0 = unknown)
}

/// Read SOT marker segment (after marker code has been consumed).
pub fn read_sot(reader: &mut SliceReader) -> Result<SotMarker> {
    let length = reader.read_u16_be()?; // Lsot = 10
    if length != 10 {
        return Err(Jp2Error::InvalidData(format!(
            "SOT marker length should be 10, got {length}"
        )));
    }
    let tile_index = reader.read_u16_be()?;
    let tile_part_len = reader.read_u32_be()?;
    let tile_part_no = reader.read_u8()?;
    let num_tile_parts = reader.read_u8()?;
    Ok(SotMarker {
        tile_index,
        tile_part_len,
        tile_part_no,
        num_tile_parts,
    })
}

/// Write SOT marker segment. Returns the byte offset of the Psot field
/// so it can be back-patched with the actual tile-part length later.
pub fn write_sot(writer: &mut VecWriter, sot: &SotMarker) -> usize {
    writer.write_u16_be(SOT);
    writer.write_u16_be(10); // Lsot
    writer.write_u16_be(sot.tile_index);
    let psot_offset = writer.tell();
    writer.write_u32_be(sot.tile_part_len);
    writer.write_u8(sot.tile_part_no);
    writer.write_u8(sot.num_tile_parts);
    psot_offset
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_constants() {
        assert_eq!(SOC, 0xFF4F);
        assert_eq!(EOC, 0xFFD9);
        assert_eq!(SIZ, 0xFF51);
        assert_eq!(COD, 0xFF52);
        assert_eq!(QCD, 0xFF5C);
        assert_eq!(SOT, 0xFF90);
        assert_eq!(SOD, 0xFF93);
    }
}
