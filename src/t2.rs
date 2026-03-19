/// Tier-2 Packet Encoding/Decoding
///
/// Assembles code-block contributions into packets and disassembles them back.
/// Implements simplified packet header coding with SOP/EPH marker support.

use crate::bio::{BioReader, BioWriter};
use crate::error::{Jp2Error, Result};

/// SOP marker: 0xFF91
const SOP_MARKER: u16 = 0xFF91;
/// EPH marker: 0xFF92
const EPH_MARKER: u16 = 0xFF92;

/// A single code-block's contribution to a packet.
#[derive(Debug, Clone)]
pub struct CblkContribution {
    /// Compressed code-block data.
    pub data: Vec<u8>,
    /// Number of coding passes included.
    pub num_passes: u32,
    /// Number of zero bitplanes (for first inclusion).
    pub zero_bitplanes: u32,
    /// Whether this code-block is included in this packet.
    pub included: bool,
}

/// Simplified packet: header + body for code-block contributions.
#[derive(Debug, Clone)]
pub struct PacketData {
    /// Whether the packet is empty (no contributions).
    pub is_empty: bool,
    /// Code-block contributions.
    pub cblk_data: Vec<CblkContribution>,
}

/// Encode the number of new coding passes using the variable-length scheme:
/// 1 pass  -> 0
/// 2 passes -> 10
/// 3-5 passes -> 1100 + 2-bit value (0..3 => 3..5: offset by 3)
/// 6-36 passes -> 1101 + 5-bit value (0..31 => 6..36: offset by 6)  [but only up to 164]
/// 37+ passes -> 11100000 + 7-bit value (but we cap at reasonable values)
///
/// Simplified encoding following JPEG 2000 spec Table B.4:
/// 1       -> 0
/// 2       -> 10
/// 3..5    -> 1100 + (n-3) in 2 bits
/// 6..36   -> 1101 + (n-6) in 5 bits
/// 37..164 -> 11100000 + (n-37) in 7 bits
fn encode_num_passes(bio: &mut BioWriter, num_passes: u32) {
    if num_passes == 1 {
        bio.putbit(0);
    } else if num_passes == 2 {
        bio.putbit(1);
        bio.putbit(0);
    } else if num_passes <= 5 {
        bio.write(0b1100, 4);
        bio.write(num_passes - 3, 2);
    } else if num_passes <= 36 {
        bio.write(0b1101, 4);
        bio.write(num_passes - 6, 5);
    } else {
        bio.write(0b11100000, 8);
        bio.write(num_passes - 37, 7);
    }
}

/// Decode the number of new coding passes.
fn decode_num_passes(bio: &mut BioReader) -> Result<u32> {
    let bit0 = bio.getbit()?;
    if bit0 == 0 {
        return Ok(1);
    }
    let bit1 = bio.getbit()?;
    if bit1 == 0 {
        return Ok(2);
    }
    // Two more bits to distinguish 3-5 vs 6-36 vs 37+
    let bit2 = bio.getbit()?;
    let bit3 = bio.getbit()?;
    if bit2 == 0 && bit3 == 0 {
        // 1100 + 2-bit value
        let v = bio.read(2)?;
        Ok(3 + v)
    } else if bit2 == 0 && bit3 == 1 {
        // 1101 + 5-bit value
        let v = bio.read(5)?;
        Ok(6 + v)
    } else {
        // 111x xxxx + 7 bits
        // We already read 4 bits (11, bit2, bit3), need 4 more for the prefix then 7-bit value
        let _extra = bio.read(4)?; // remaining prefix bits
        let v = bio.read(7)?;
        Ok(37 + v)
    }
}

/// Compute the number of bytes needed to represent `len` as a variable-length field.
/// Uses a simple scheme: lengths < 256 use 1 byte, < 65536 use 2 bytes, etc.
fn length_num_bytes(len: usize) -> u32 {
    if len < 256 {
        1
    } else if len < 65536 {
        2
    } else if len < 16_777_216 {
        3
    } else {
        4
    }
}

/// Encode a packet from code-block contributions.
///
/// Returns the encoded packet bytes including optional SOP/EPH markers.
pub fn encode_packet(
    contributions: &[CblkContribution],
    use_sop: bool,
    use_eph: bool,
) -> Vec<u8> {
    let mut output = Vec::new();

    // SOP marker segment: FF91 + Lsop(2 bytes=0004) + Nsop(2 bytes)
    if use_sop {
        output.push((SOP_MARKER >> 8) as u8);
        output.push((SOP_MARKER & 0xFF) as u8);
        output.push(0x00);
        output.push(0x04); // Lsop = 4
        output.push(0x00);
        output.push(0x00); // Nsop = 0 (simplified)
    }

    // Check if packet is empty
    let has_data = contributions.iter().any(|c| c.included && !c.data.is_empty());

    let mut bio = BioWriter::new();

    if !has_data {
        // Empty packet: just write 0 bit
        bio.putbit(0);
        let _ = bio.flush();
        output.extend_from_slice(bio.as_slice());

        if use_eph {
            output.push((EPH_MARKER >> 8) as u8);
            output.push((EPH_MARKER & 0xFF) as u8);
        }
        return output;
    }

    // Non-empty packet: write 1 bit
    bio.putbit(1);

    // For each code-block, write header info
    for contrib in contributions {
        if !contrib.included {
            // Not included: write 0 bit
            bio.putbit(0);
            continue;
        }

        // Included: write 1 bit
        bio.putbit(1);

        // Zero bitplanes (simplified: write as 8-bit value)
        bio.write(contrib.zero_bitplanes, 8);

        // Number of coding passes
        encode_num_passes(&mut bio, contrib.num_passes);

        // Length of code-block data
        let num_len_bytes = length_num_bytes(contrib.data.len());
        bio.write(num_len_bytes, 2); // 2 bits for length-of-length
        bio.write(contrib.data.len() as u32, num_len_bytes * 8);
    }

    let _ = bio.flush();
    output.extend_from_slice(bio.as_slice());

    // EPH marker after packet header
    if use_eph {
        output.push((EPH_MARKER >> 8) as u8);
        output.push((EPH_MARKER & 0xFF) as u8);
    }

    // Packet body: concatenated code-block data
    for contrib in contributions {
        if contrib.included {
            output.extend_from_slice(&contrib.data);
        }
    }

    output
}

/// Decode a packet, returning code-block contributions and bytes consumed.
///
/// `num_cblks` is the expected number of code-blocks in the precinct.
/// `first_inclusion` indicates whether each code-block is being included for the first time
/// (used for zero-bitplane decoding).
pub fn decode_packet(
    data: &[u8],
    num_cblks: usize,
    _first_inclusion: &[bool],
) -> Result<(PacketData, usize)> {
    let mut offset = 0usize;

    // Check for SOP marker
    if data.len() >= offset + 2
        && data[offset] == (SOP_MARKER >> 8) as u8
        && data[offset + 1] == (SOP_MARKER & 0xFF) as u8
    {
        // Skip SOP marker segment: marker(2) + Lsop(2) + Nsop(2) = 6 bytes
        offset += 6;
    }

    if offset >= data.len() {
        return Err(Jp2Error::OutOfBounds {
            offset,
            len: 1,
        });
    }

    let mut bio = BioReader::new(&data[offset..]);

    // Read empty flag
    let non_empty = bio.getbit()?;

    if non_empty == 0 {
        let _ = bio.inalign();
        let header_bytes = bio.numbytes();
        offset += header_bytes;

        // Check for EPH marker
        if data.len() >= offset + 2
            && data[offset] == (EPH_MARKER >> 8) as u8
            && data[offset + 1] == (EPH_MARKER & 0xFF) as u8
        {
            offset += 2;
        }

        return Ok((
            PacketData {
                is_empty: true,
                cblk_data: Vec::new(),
            },
            offset,
        ));
    }

    // Non-empty packet: read code-block headers
    let mut contrib_infos: Vec<(bool, u32, u32, usize)> = Vec::with_capacity(num_cblks);

    for _i in 0..num_cblks {
        let included = bio.getbit()?;
        if included == 0 {
            contrib_infos.push((false, 0, 0, 0));
            continue;
        }

        // Zero bitplanes
        let zero_bp = bio.read(8)?;

        // Number of passes
        let num_passes = decode_num_passes(&mut bio)?;

        // Length
        let num_len_bytes = bio.read(2)?;
        let data_len = bio.read(num_len_bytes * 8)? as usize;

        contrib_infos.push((true, zero_bp, num_passes, data_len));
    }

    let _ = bio.inalign();
    let header_bytes = bio.numbytes();
    offset += header_bytes;

    // Check for EPH marker
    if data.len() >= offset + 2
        && data[offset] == (EPH_MARKER >> 8) as u8
        && data[offset + 1] == (EPH_MARKER & 0xFF) as u8
    {
        offset += 2;
    }

    // Read packet body
    let mut contributions = Vec::with_capacity(num_cblks);
    for (included, zero_bp, num_passes, data_len) in contrib_infos {
        if !included {
            contributions.push(CblkContribution {
                data: Vec::new(),
                num_passes: 0,
                zero_bitplanes: 0,
                included: false,
            });
            continue;
        }

        if offset + data_len > data.len() {
            return Err(Jp2Error::OutOfBounds {
                offset,
                len: data_len,
            });
        }

        let cblk_data = data[offset..offset + data_len].to_vec();
        offset += data_len;

        contributions.push(CblkContribution {
            data: cblk_data,
            num_passes,
            zero_bitplanes: zero_bp,
            included: true,
        });
    }

    Ok((
        PacketData {
            is_empty: false,
            cblk_data: contributions,
        },
        offset,
    ))
}
