/// Phase 10: JP2 Box Parser
///
/// JP2 wraps a J2K codestream in a box-based file format (ISO 15444-1 Annex I).

use crate::error::{Jp2Error, Result};
use crate::stream::{SliceReader, VecWriter};

// ── Box type constants (4-byte big-endian) ──

pub const JP2_JP: u32   = 0x6A502020;  // JPEG 2000 signature
pub const JP2_FTYP: u32 = 0x66747970;  // File type
pub const JP2_JP2H: u32 = 0x6A703268;  // JP2 header (super box)
pub const JP2_IHDR: u32 = 0x69686472;  // Image header
pub const JP2_COLR: u32 = 0x636F6C72;  // Color specification
pub const JP2_JP2C: u32 = 0x6A703263;  // Contiguous codestream
pub const JP2_PCLR: u32 = 0x70636C72;  // Palette
pub const JP2_CMAP: u32 = 0x636D6170;  // Component mapping
pub const JP2_CDEF: u32 = 0x63646566;  // Channel definition
pub const JP2_RES: u32  = 0x72657320;  // Resolution (super box)

/// JP2 signature magic value (content of the JP box)
pub const JP2_SIGNATURE: u32 = 0x0D0A870A;

/// JP2 brand value "jp2\040"
pub const JP2_BRAND: u32 = 0x6A703220;

// ── Box header ──

/// A JP2 box header
#[derive(Debug, Clone)]
pub struct BoxHeader {
    pub box_type: u32,
    /// Total box length including header (0 = to end of file)
    pub length: u64,
    /// 8 for normal, 16 for extended length
    pub header_size: u8,
}

/// Read a box header from stream.
pub fn read_box_header(reader: &mut SliceReader) -> Result<BoxHeader> {
    let lbox = reader.read_u32_be()? as u64;
    let tbox = reader.read_u32_be()?;

    let (length, header_size) = if lbox == 1 {
        // Extended length: next 8 bytes are the real length
        let xl = reader.read_u64_be()?;
        (xl, 16u8)
    } else if lbox == 0 {
        // Box extends to end of file
        (0u64, 8u8)
    } else {
        (lbox, 8u8)
    };

    Ok(BoxHeader {
        box_type: tbox,
        length,
        header_size,
    })
}

/// Write a normal box header (8-byte header).
/// `data_len` is the length of the box content (not including header).
pub fn write_box_header(writer: &mut VecWriter, box_type: u32, data_len: u64) {
    let total = data_len + 8;
    writer.write_u32_be(total as u32);
    writer.write_u32_be(box_type);
}

/// Write an extended-length box header (16-byte header).
/// `data_len` is the length of the box content (not including header).
pub fn write_box_header_xl(writer: &mut VecWriter, box_type: u32, data_len: u64) {
    let total = data_len + 16;
    writer.write_u32_be(1); // signals extended length
    writer.write_u32_be(box_type);
    writer.write_u64_be(total);
}

// ── Image Header Box (IHDR) ──

#[derive(Debug, Clone)]
pub struct IhdrBox {
    pub height: u32,
    pub width: u32,
    pub num_comps: u16,
    /// Bits per component (7 bits precision + sign bit at bit 7)
    pub bpc: u8,
    /// Compression type: 7 = JPEG 2000
    pub compression: u8,
    /// Unknown colorspace flag
    pub unk_colorspace: u8,
    /// Intellectual property flag
    pub ipr: u8,
}

/// Read an IHDR box payload (14 bytes).
pub fn read_ihdr(reader: &mut SliceReader) -> Result<IhdrBox> {
    let height = reader.read_u32_be()?;
    let width = reader.read_u32_be()?;
    let num_comps = reader.read_u16_be()?;
    let bpc = reader.read_u8()?;
    let compression = reader.read_u8()?;
    let unk_colorspace = reader.read_u8()?;
    let ipr = reader.read_u8()?;

    if compression != 7 {
        return Err(Jp2Error::UnsupportedFeature(format!(
            "IHDR compression type {compression}, expected 7"
        )));
    }

    Ok(IhdrBox {
        height,
        width,
        num_comps,
        bpc,
        compression,
        unk_colorspace,
        ipr,
    })
}

/// Write an IHDR box payload (14 bytes) to the writer.
pub fn write_ihdr(writer: &mut VecWriter, ihdr: &IhdrBox) {
    writer.write_u32_be(ihdr.height);
    writer.write_u32_be(ihdr.width);
    writer.write_u16_be(ihdr.num_comps);
    writer.write_u8(ihdr.bpc);
    writer.write_u8(ihdr.compression);
    writer.write_u8(ihdr.unk_colorspace);
    writer.write_u8(ihdr.ipr);
}

// ── Color Specification Box (COLR) ──

#[derive(Debug, Clone)]
pub struct ColrBox {
    /// 1 = enumerated colorspace, 2 = ICC profile
    pub method: u8,
    pub precedence: u8,
    pub approx: u8,
    /// If method == 1: 16=sRGB, 17=grayscale, 18=YCC
    pub enum_cs: Option<u32>,
    /// If method == 2: raw ICC profile data
    pub icc_profile: Option<Vec<u8>>,
}

/// Enumerated color space: sRGB
pub const CS_SRGB: u32 = 16;
/// Enumerated color space: grayscale
pub const CS_GRAYSCALE: u32 = 17;
/// Enumerated color space: YCC
pub const CS_YCC: u32 = 18;

/// Read a COLR box payload.
/// `payload_len` is the number of bytes in the box content.
pub fn read_colr(reader: &mut SliceReader, payload_len: usize) -> Result<ColrBox> {
    let method = reader.read_u8()?;
    let precedence = reader.read_u8()?;
    let approx = reader.read_u8()?;

    match method {
        1 => {
            let enum_cs = reader.read_u32_be()?;
            Ok(ColrBox {
                method,
                precedence,
                approx,
                enum_cs: Some(enum_cs),
                icc_profile: None,
            })
        }
        2 => {
            let profile_len = payload_len.saturating_sub(3);
            let profile = reader.read_bytes(profile_len)?.to_vec();
            Ok(ColrBox {
                method,
                precedence,
                approx,
                enum_cs: None,
                icc_profile: Some(profile),
            })
        }
        _ => Err(Jp2Error::UnsupportedFeature(format!(
            "COLR method {method}"
        ))),
    }
}

/// Write a COLR box payload.
pub fn write_colr(writer: &mut VecWriter, colr: &ColrBox) {
    writer.write_u8(colr.method);
    writer.write_u8(colr.precedence);
    writer.write_u8(colr.approx);
    match colr.method {
        1 => {
            if let Some(cs) = colr.enum_cs {
                writer.write_u32_be(cs);
            }
        }
        2 => {
            if let Some(ref icc) = colr.icc_profile {
                writer.write_bytes(icc);
            }
        }
        _ => {}
    }
}
