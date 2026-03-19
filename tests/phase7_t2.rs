use justjp2::t2::{self, CblkContribution};

#[test]
fn encode_decode_empty_packet() {
    let contributions: Vec<CblkContribution> = vec![];
    let encoded = t2::encode_packet(&contributions, false, false);
    assert!(!encoded.is_empty());

    let (packet, bytes_consumed) = t2::decode_packet(&encoded, 0, &[]).unwrap();
    assert!(packet.is_empty);
    assert_eq!(packet.cblk_data.len(), 0);
    assert_eq!(bytes_consumed, encoded.len());
}

#[test]
fn encode_decode_single_cblk() {
    let data = vec![0x01, 0x02, 0x03, 0x04, 0x05];
    let contributions = vec![CblkContribution {
        data: data.clone(),
        num_passes: 3,
        zero_bitplanes: 2,
        included: true,
    }];

    let encoded = t2::encode_packet(&contributions, false, false);
    let first_inclusion = vec![true];
    let (packet, bytes_consumed) = t2::decode_packet(&encoded, 1, &first_inclusion).unwrap();

    assert!(!packet.is_empty);
    assert_eq!(packet.cblk_data.len(), 1);
    assert!(packet.cblk_data[0].included);
    assert_eq!(packet.cblk_data[0].data, data);
    assert_eq!(packet.cblk_data[0].num_passes, 3);
    assert_eq!(packet.cblk_data[0].zero_bitplanes, 2);
    assert_eq!(bytes_consumed, encoded.len());
}

#[test]
fn encode_decode_multi_cblk() {
    let data1 = vec![0xAA, 0xBB];
    let data2 = vec![0xCC, 0xDD, 0xEE];
    let contributions = vec![
        CblkContribution {
            data: data1.clone(),
            num_passes: 1,
            zero_bitplanes: 0,
            included: true,
        },
        CblkContribution {
            data: Vec::new(),
            num_passes: 0,
            zero_bitplanes: 0,
            included: false,
        },
        CblkContribution {
            data: data2.clone(),
            num_passes: 5,
            zero_bitplanes: 4,
            included: true,
        },
    ];

    let encoded = t2::encode_packet(&contributions, false, false);
    let first_inclusion = vec![true, true, true];
    let (packet, bytes_consumed) = t2::decode_packet(&encoded, 3, &first_inclusion).unwrap();

    assert!(!packet.is_empty);
    assert_eq!(packet.cblk_data.len(), 3);

    assert!(packet.cblk_data[0].included);
    assert_eq!(packet.cblk_data[0].data, data1);
    assert_eq!(packet.cblk_data[0].num_passes, 1);

    assert!(!packet.cblk_data[1].included);
    assert!(packet.cblk_data[1].data.is_empty());

    assert!(packet.cblk_data[2].included);
    assert_eq!(packet.cblk_data[2].data, data2);
    assert_eq!(packet.cblk_data[2].num_passes, 5);
    assert_eq!(packet.cblk_data[2].zero_bitplanes, 4);

    assert_eq!(bytes_consumed, encoded.len());
}

#[test]
fn sop_marker_present() {
    let contributions = vec![CblkContribution {
        data: vec![0x42],
        num_passes: 1,
        zero_bitplanes: 0,
        included: true,
    }];

    let encoded = t2::encode_packet(&contributions, true, false);

    // SOP marker: FF 91
    assert_eq!(encoded[0], 0xFF);
    assert_eq!(encoded[1], 0x91);
    // Lsop = 0x0004
    assert_eq!(encoded[2], 0x00);
    assert_eq!(encoded[3], 0x04);

    // Should still be decodable
    let first_inclusion = vec![true];
    let (packet, _) = t2::decode_packet(&encoded, 1, &first_inclusion).unwrap();
    assert!(!packet.is_empty);
    assert_eq!(packet.cblk_data[0].data, vec![0x42]);
}

#[test]
fn eph_marker_present() {
    let contributions = vec![CblkContribution {
        data: vec![0x42],
        num_passes: 1,
        zero_bitplanes: 0,
        included: true,
    }];

    let encoded = t2::encode_packet(&contributions, false, true);

    // EPH marker: FF 92 should appear somewhere in the encoded data
    let has_eph = encoded
        .windows(2)
        .any(|w| w[0] == 0xFF && w[1] == 0x92);
    assert!(has_eph, "EPH marker not found in encoded packet");

    // Should still be decodable
    let first_inclusion = vec![true];
    let (packet, _) = t2::decode_packet(&encoded, 1, &first_inclusion).unwrap();
    assert!(!packet.is_empty);
    assert_eq!(packet.cblk_data[0].data, vec![0x42]);
}

#[test]
fn inclusion_tag_tree() {
    // Test that a code-block marked as not included is properly handled
    let contributions = vec![
        CblkContribution {
            data: vec![0x01, 0x02],
            num_passes: 2,
            zero_bitplanes: 1,
            included: true,
        },
        CblkContribution {
            data: Vec::new(),
            num_passes: 0,
            zero_bitplanes: 0,
            included: false,
        },
    ];

    let encoded = t2::encode_packet(&contributions, false, false);
    let first_inclusion = vec![true, false];
    let (packet, bytes_consumed) = t2::decode_packet(&encoded, 2, &first_inclusion).unwrap();

    assert!(!packet.is_empty);
    assert_eq!(packet.cblk_data.len(), 2);
    assert!(packet.cblk_data[0].included);
    assert_eq!(packet.cblk_data[0].data, vec![0x01, 0x02]);
    assert_eq!(packet.cblk_data[0].num_passes, 2);
    assert!(!packet.cblk_data[1].included);
    assert!(packet.cblk_data[1].data.is_empty());
    assert_eq!(bytes_consumed, encoded.len());
}
