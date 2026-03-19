/// Phase 12.1: HTJ2K foundation tests

use justjp2::htj2k::{self, CapMarker};
use justjp2::stream::{SliceReader, VecWriter};

#[test]
fn is_htj2k_flag() {
    // Bit 6 (0x40) set indicates HTJ2K
    assert!(htj2k::is_htj2k(0x40));
    assert!(htj2k::is_htj2k(0x60)); // 0x40 | 0x20
    assert!(htj2k::is_htj2k(0xFF)); // all bits set
    assert!(htj2k::is_htj2k(0x41)); // 0x40 | 0x01
}

#[test]
fn is_not_htj2k() {
    // Without bit 6 set, it's regular J2K
    assert!(!htj2k::is_htj2k(0x00));
    assert!(!htj2k::is_htj2k(0x01)); // BYPASS only
    assert!(!htj2k::is_htj2k(0x20)); // SEGSYM only
    assert!(!htj2k::is_htj2k(0x3F)); // all lower bits, but not 0x40
}

#[test]
fn cap_marker_roundtrip() {
    let original = CapMarker {
        pcap: 0x0002_0000, // bit 17 set -> 1 ccap entry
        ccap: vec![0x4020],
    };

    // Write
    let mut writer = VecWriter::new();
    htj2k::write_cap(&mut writer, &original);
    let bytes = writer.into_vec();

    // Read back
    let mut reader = SliceReader::new(&bytes);
    let parsed = htj2k::read_cap(&mut reader).expect("read_cap should succeed");

    assert_eq!(parsed.pcap, original.pcap);
    assert_eq!(parsed.ccap, original.ccap);
}

#[test]
fn cap_marker_roundtrip_multiple_ccap() {
    // pcap with bits 16 and 17 set -> 2 ccap entries
    let original = CapMarker {
        pcap: 0x0003_0000,
        ccap: vec![0x0020, 0x4000],
    };

    let mut writer = VecWriter::new();
    htj2k::write_cap(&mut writer, &original);
    let bytes = writer.into_vec();

    let mut reader = SliceReader::new(&bytes);
    let parsed = htj2k::read_cap(&mut reader).expect("read_cap should succeed");

    assert_eq!(parsed, original);
}

#[test]
fn cap_marker_no_ccap() {
    // pcap = 0 -> no ccap entries
    let original = CapMarker {
        pcap: 0,
        ccap: vec![],
    };

    let mut writer = VecWriter::new();
    htj2k::write_cap(&mut writer, &original);
    let bytes = writer.into_vec();

    let mut reader = SliceReader::new(&bytes);
    let parsed = htj2k::read_cap(&mut reader).expect("read_cap should succeed");

    assert_eq!(parsed, original);
}

#[test]
fn ht_decode_returns_error() {
    let data = vec![0u8; 64];
    let result = htj2k::ht_decode_cblk(&data, 8, 8, 1);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        "HTJ2K decoding not yet implemented"
    );
}
