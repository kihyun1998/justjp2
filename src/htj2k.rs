/// HTJ2K (JPEG 2000 Part 15) foundation module.
/// Implements the basic HT block decoder structure and CAP marker support.

use crate::error::{Jp2Error, Result};
use crate::stream::{SliceReader, VecWriter};

/// HT code-block decoder state.
///
/// Contains the three sub-decoder states required for HTJ2K block decoding:
/// - MEL (Modular Embedded block coder with Optimized truncation - Length)
/// - VLC (Variable Length Code) decoder
/// - MagSgn (Magnitude and Sign) decoder
pub struct HtDecoder {
    /// MEL decoder state: current run length and remaining runs
    pub mel_run: u32,
    pub mel_remaining: u32,
    /// VLC decoder state: bit buffer and position
    pub vlc_bits: u64,
    pub vlc_pos: u32,
    /// MagSgn decoder state: bit buffer and position
    pub magsgn_bits: u64,
    pub magsgn_pos: u32,
}

impl HtDecoder {
    /// Create a new HT decoder with zeroed state.
    pub fn new() -> Self {
        Self {
            mel_run: 0,
            mel_remaining: 0,
            vlc_bits: 0,
            vlc_pos: 0,
            magsgn_bits: 0,
            magsgn_pos: 0,
        }
    }
}

impl Default for HtDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// CAP marker data (extended capabilities for Part 15).
///
/// The CAP marker signals that a codestream uses HTJ2K features.
/// It contains a bitmask of profile capabilities and per-component
/// capability words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapMarker {
    /// Profile capabilities bitmask.
    /// Each set bit indicates a corresponding Ccap value is present.
    pub pcap: u32,
    /// Component capabilities — one u16 per set bit in pcap.
    pub ccap: Vec<u16>,
}

/// Decode an HT-encoded code block.
///
/// Returns decoded coefficient data as a vector of i32 samples.
///
/// Note: This is a stub implementation that returns an error for now,
/// as full HTJ2K decoding requires implementing MEL, VLC, and MagSgn
/// decoders which are beyond the current scope.
pub fn ht_decode_cblk(
    _data: &[u8],
    _width: u32,
    _height: u32,
    _num_passes: u32,
) -> std::result::Result<Vec<i32>, String> {
    Err("HTJ2K decoding not yet implemented".to_string())
}

/// Check if a codestream uses HTJ2K (Part 15) by checking the HT
/// code-block style flag.
///
/// The HT flag is bit 6 (0x40) in the code-block style byte. When set,
/// the code-block uses the HT (High Throughput) block coder instead of
/// the traditional EBCOT MQ coder.
pub fn is_htj2k(cblk_style: u8) -> bool {
    (cblk_style & 0x40) != 0
}

/// Parse a CAP marker segment from the reader.
///
/// CAP marker format:
/// - Lcap (u16): marker segment length (already consumed by caller typically,
///   but we read it here for self-contained parsing)
/// - Pcap (u32): profile capabilities bitmask
/// - Ccap_i (u16): one per set bit in Pcap
pub fn read_cap(reader: &mut SliceReader) -> Result<CapMarker> {
    // Read segment length
    let lcap = reader.read_u16_be()? as usize;
    if lcap < 6 {
        return Err(Jp2Error::InvalidData(
            "CAP marker segment too short".to_string(),
        ));
    }

    let pcap = reader.read_u32_be()?;

    // Count set bits in pcap to know how many Ccap entries
    let num_ccap = pcap.count_ones() as usize;

    // Validate length: 2 (Lcap) + 4 (Pcap) + 2*num_ccap
    let expected_len = 6 + 2 * num_ccap;
    if lcap != expected_len {
        return Err(Jp2Error::InvalidData(format!(
            "CAP marker length mismatch: Lcap={lcap}, expected={expected_len}"
        )));
    }

    let mut ccap = Vec::with_capacity(num_ccap);
    for _ in 0..num_ccap {
        ccap.push(reader.read_u16_be()?);
    }

    Ok(CapMarker { pcap, ccap })
}

/// Write a CAP marker segment to the writer.
///
/// Writes Lcap, Pcap, and all Ccap entries.
/// Does NOT write the marker code (0xFF50) itself — the caller is
/// responsible for writing the marker prefix.
pub fn write_cap(writer: &mut VecWriter, cap: &CapMarker) {
    let num_ccap = cap.pcap.count_ones() as usize;
    // Lcap = 2 (self) + 4 (Pcap) + 2 * num_ccap
    let lcap = 6 + 2 * num_ccap;
    writer.write_u16_be(lcap as u16);
    writer.write_u32_be(cap.pcap);
    for &c in &cap.ccap {
        writer.write_u16_be(c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ht_decoder_default() {
        let dec = HtDecoder::new();
        assert_eq!(dec.mel_run, 0);
        assert_eq!(dec.vlc_bits, 0);
        assert_eq!(dec.magsgn_bits, 0);
    }

    #[test]
    fn cap_marker_basic() {
        let cap = CapMarker {
            pcap: 0x0001_0000, // bit 16 set -> 1 ccap entry
            ccap: vec![0x0020],
        };
        assert_eq!(cap.pcap.count_ones(), 1);
        assert_eq!(cap.ccap.len(), 1);
    }
}
