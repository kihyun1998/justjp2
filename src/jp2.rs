/// Phase 10: JP2 File Format Codec
///
/// Encodes/decodes JP2 files wrapping J2K codestreams in the ISO 15444-1 box format.

use crate::error::{Jp2Error, Result};
use crate::j2k;
use crate::jp2_box::*;
use crate::stream::{SliceReader, VecWriter};
use crate::tcd::{TcdComponent, TcdParams};

/// Encode image data as a complete JP2 file.
pub fn jp2_encode(
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

    // Encode the J2K codestream first
    let j2k_data = j2k::j2k_encode(components, comp_info, params)?;

    let mut writer = VecWriter::new();

    // 1. JP2 Signature box
    write_box_header(&mut writer, JP2_JP, 4);
    writer.write_u32_be(JP2_SIGNATURE);

    // 2. File Type box
    // Content: brand(4) + MinVersion(4) + CL[0](4) = 12 bytes
    write_box_header(&mut writer, JP2_FTYP, 12);
    writer.write_u32_be(JP2_BRAND); // brand = "jp2\040"
    writer.write_u32_be(0);          // MinVersion = 0
    writer.write_u32_be(JP2_BRAND); // compatibility list: "jp2\040"

    // 3. JP2 Header super box (JP2H)
    // Build IHDR and COLR content to compute super-box size
    let bpc = if comp_info[0].signed {
        0x80 | ((comp_info[0].precision - 1) as u8)
    } else {
        (comp_info[0].precision - 1) as u8
    };

    let ihdr = IhdrBox {
        height,
        width,
        num_comps: num_comps as u16,
        bpc,
        compression: 7,
        unk_colorspace: 0,
        ipr: 0,
    };

    let colr = if num_comps >= 3 {
        ColrBox {
            method: 1,
            precedence: 0,
            approx: 0,
            enum_cs: Some(CS_SRGB),
            icc_profile: None,
        }
    } else {
        ColrBox {
            method: 1,
            precedence: 0,
            approx: 0,
            enum_cs: Some(CS_GRAYSCALE),
            icc_profile: None,
        }
    };

    // IHDR box: header(8) + payload(14) = 22 bytes
    let ihdr_box_len = 8 + 14;
    // COLR box: header(8) + method(1) + precedence(1) + approx(1) + enum_cs(4) = 15
    let colr_payload_len: u64 = if colr.method == 1 { 7 } else { 3 };
    let colr_box_len = 8 + colr_payload_len;
    let jp2h_content_len = ihdr_box_len + colr_box_len;

    write_box_header(&mut writer, JP2_JP2H, jp2h_content_len);

    // 3a. IHDR box
    write_box_header(&mut writer, JP2_IHDR, 14);
    write_ihdr(&mut writer, &ihdr);

    // 3b. COLR box
    write_box_header(&mut writer, JP2_COLR, colr_payload_len);
    write_colr(&mut writer, &colr);

    // 4. Contiguous Codestream box (JP2C)
    write_box_header(&mut writer, JP2_JP2C, j2k_data.len() as u64);
    writer.write_bytes(&j2k_data);

    Ok(writer.into_vec())
}

/// Decode a JP2 file, returning per-component samples and component info.
pub fn jp2_decode(data: &[u8]) -> Result<(Vec<Vec<i32>>, Vec<TcdComponent>)> {
    let mut reader = SliceReader::new(data);

    // 1. Read and verify JP2 signature box
    let jp_header = read_box_header(&mut reader)?;
    if jp_header.box_type != JP2_JP {
        return Err(Jp2Error::InvalidData(format!(
            "expected JP signature box (0x{:08X}), got 0x{:08X}",
            JP2_JP, jp_header.box_type
        )));
    }
    let sig = reader.read_u32_be()?;
    if sig != JP2_SIGNATURE {
        return Err(Jp2Error::InvalidData(format!(
            "invalid JP2 signature: 0x{sig:08X}"
        )));
    }

    // 2. Read File Type box
    let ftyp_header = read_box_header(&mut reader)?;
    if ftyp_header.box_type != JP2_FTYP {
        return Err(Jp2Error::InvalidData(format!(
            "expected FTYP box (0x{:08X}), got 0x{:08X}",
            JP2_FTYP, ftyp_header.box_type
        )));
    }
    let ftyp_content_len = ftyp_header.length as usize - ftyp_header.header_size as usize;
    // Skip FTYP content
    reader.skip(ftyp_content_len)?;

    // 3. Read remaining boxes: find JP2H and JP2C
    let mut _ihdr: Option<IhdrBox> = None;
    let mut _colr: Option<ColrBox> = None;
    let mut j2k_codestream: Option<&[u8]> = None;

    while reader.remaining() >= 8 {
        let header = read_box_header(&mut reader)?;

        let content_len = if header.length == 0 {
            reader.remaining()
        } else {
            header.length as usize - header.header_size as usize
        };

        match header.box_type {
            JP2_JP2H => {
                // Parse nested boxes within JP2H
                let jp2h_end = reader.tell() + content_len;
                while reader.tell() < jp2h_end && reader.remaining() >= 8 {
                    let sub_header = read_box_header(&mut reader)?;
                    let sub_content_len =
                        sub_header.length as usize - sub_header.header_size as usize;

                    match sub_header.box_type {
                        JP2_IHDR => {
                            _ihdr = Some(read_ihdr(&mut reader)?);
                        }
                        JP2_COLR => {
                            _colr = Some(read_colr(&mut reader, sub_content_len)?);
                        }
                        _ => {
                            // Skip unknown sub-box
                            reader.skip(sub_content_len)?;
                        }
                    }
                }
            }
            JP2_JP2C => {
                // Contiguous codestream
                j2k_codestream = Some(reader.read_bytes(content_len)?);
            }
            _ => {
                // Skip unknown box
                reader.skip(content_len)?;
            }
        }

        // If we already found the codestream, we can stop
        if j2k_codestream.is_some() {
            break;
        }
    }

    let codestream = j2k_codestream
        .ok_or_else(|| Jp2Error::InvalidData("no JP2C (codestream) box found".to_string()))?;

    // 5. Decode the J2K codestream
    j2k::j2k_decode(codestream)
}
