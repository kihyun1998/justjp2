use justjp2::pi::{PacketIndex, PiImage, PiIterator, PiParams};
use justjp2::types::ProgOrder;

/// Helper: create a simple image with 2 components, 2 resolution levels each,
/// and varying precinct counts.
fn test_image() -> PiImage {
    PiImage {
        num_comps: 2,
        num_res: vec![2, 2],
        num_precincts: vec![
            // comp 0: res 0 -> (1,1)=1 precinct, res 1 -> (2,1)=2 precincts
            vec![(1, 1), (2, 1)],
            // comp 1: res 0 -> (1,1)=1 precinct, res 1 -> (1,2)=2 precincts
            vec![(1, 1), (1, 2)],
        ],
    }
}

/// Expected total valid packets = num_layers * sum over comps of sum over res of precincts
/// = 2 * ((1+2) + (1+2)) = 2 * 6 = 12
fn expected_packet_count(num_layers: u32) -> usize {
    // comp0: res0=1, res1=2 -> 3
    // comp1: res0=1, res1=2 -> 3
    // total per layer = 6
    (num_layers as usize) * 6
}

#[test]
fn lrcp_order() {
    let image = test_image();
    let params = PiParams {
        num_layers: 2,
        prog_order: ProgOrder::Lrcp,
    };
    let iter = PiIterator::new(image, params);
    let packets: Vec<PacketIndex> = iter.packets().to_vec();

    assert_eq!(packets.len(), expected_packet_count(2));

    // LRCP: layer is outermost loop
    // All packets with layer=0 should come before all packets with layer=1
    let first_layer1 = packets.iter().position(|p| p.layer == 1).unwrap();
    let last_layer0 = packets.iter().rposition(|p| p.layer == 0).unwrap();
    assert!(last_layer0 < first_layer1);

    // Within a layer, resolution is the next loop
    // For layer=0: all res=0 packets before res=1 packets
    let layer0: Vec<_> = packets.iter().filter(|p| p.layer == 0).collect();
    let first_res1_in_l0 = layer0.iter().position(|p| p.res == 1).unwrap();
    let last_res0_in_l0 = layer0.iter().rposition(|p| p.res == 0).unwrap();
    assert!(last_res0_in_l0 < first_res1_in_l0);
}

#[test]
fn rlcp_order() {
    let image = test_image();
    let params = PiParams {
        num_layers: 2,
        prog_order: ProgOrder::Rlcp,
    };
    let iter = PiIterator::new(image, params);
    let packets: Vec<PacketIndex> = iter.packets().to_vec();

    assert_eq!(packets.len(), expected_packet_count(2));

    // RLCP: resolution is outermost loop
    let first_res1 = packets.iter().position(|p| p.res == 1).unwrap();
    let last_res0 = packets.iter().rposition(|p| p.res == 0).unwrap();
    assert!(last_res0 < first_res1);

    // Within a resolution, layer is the next loop
    let res0: Vec<_> = packets.iter().filter(|p| p.res == 0).collect();
    let first_layer1_in_r0 = res0.iter().position(|p| p.layer == 1).unwrap();
    let last_layer0_in_r0 = res0.iter().rposition(|p| p.layer == 0).unwrap();
    assert!(last_layer0_in_r0 < first_layer1_in_r0);
}

#[test]
fn rpcl_order() {
    let image = test_image();
    let params = PiParams {
        num_layers: 2,
        prog_order: ProgOrder::Rpcl,
    };
    let iter = PiIterator::new(image, params);
    let packets: Vec<PacketIndex> = iter.packets().to_vec();

    assert_eq!(packets.len(), expected_packet_count(2));

    // RPCL: resolution is outermost
    let first_res1 = packets.iter().position(|p| p.res == 1).unwrap();
    let last_res0 = packets.iter().rposition(|p| p.res == 0).unwrap();
    assert!(last_res0 < first_res1);

    // Within a resolution, precinct is the next loop, then comp, then layer
    // For res=0: all packets have precinct=0 (both comps have 1 precinct at res 0)
    let res0: Vec<_> = packets.iter().filter(|p| p.res == 0).collect();
    assert!(res0.iter().all(|p| p.precinct == 0));
}

#[test]
fn pcrl_order() {
    let image = test_image();
    let params = PiParams {
        num_layers: 2,
        prog_order: ProgOrder::Pcrl,
    };
    let iter = PiIterator::new(image, params);
    let packets: Vec<PacketIndex> = iter.packets().to_vec();

    assert_eq!(packets.len(), expected_packet_count(2));

    // PCRL: precinct is outermost, then component, then resolution, then layer
    // Precinct 0 packets should come before precinct 1 packets
    let first_prec1 = packets.iter().position(|p| p.precinct == 1).unwrap();
    let last_prec0 = packets.iter().rposition(|p| p.precinct == 0).unwrap();
    assert!(last_prec0 < first_prec1);
}

#[test]
fn cprl_order() {
    let image = test_image();
    let params = PiParams {
        num_layers: 2,
        prog_order: ProgOrder::Cprl,
    };
    let iter = PiIterator::new(image, params);
    let packets: Vec<PacketIndex> = iter.packets().to_vec();

    assert_eq!(packets.len(), expected_packet_count(2));

    // CPRL: component is outermost
    let first_comp1 = packets.iter().position(|p| p.comp == 1).unwrap();
    let last_comp0 = packets.iter().rposition(|p| p.comp == 0).unwrap();
    assert!(last_comp0 < first_comp1);

    // Within a component, precinct is next
    let comp0: Vec<_> = packets.iter().filter(|p| p.comp == 0).collect();
    let first_prec1_in_c0 = comp0.iter().position(|p| p.precinct == 1).unwrap();
    let last_prec0_in_c0 = comp0.iter().rposition(|p| p.precinct == 0).unwrap();
    assert!(last_prec0_in_c0 < first_prec1_in_c0);
}

#[test]
fn packet_count() {
    let image = test_image();
    for &order in &[
        ProgOrder::Lrcp,
        ProgOrder::Rlcp,
        ProgOrder::Rpcl,
        ProgOrder::Pcrl,
        ProgOrder::Cprl,
    ] {
        let params = PiParams {
            num_layers: 3,
            prog_order: order,
        };
        let iter = PiIterator::new(image.clone(), params);
        // Total = 3 layers * (1+2+1+2) precincts = 3 * 6 = 18
        assert_eq!(
            iter.packet_count(),
            expected_packet_count(3),
            "Failed for {:?}",
            order
        );
    }
}

#[test]
fn single_tile_single_comp() {
    let image = PiImage {
        num_comps: 1,
        num_res: vec![1],
        num_precincts: vec![
            vec![(1, 1)], // 1 precinct at the only resolution
        ],
    };
    let params = PiParams {
        num_layers: 1,
        prog_order: ProgOrder::Lrcp,
    };
    let iter = PiIterator::new(image, params);
    let packets: Vec<PacketIndex> = iter.packets().to_vec();

    assert_eq!(packets.len(), 1);
    assert_eq!(
        packets[0],
        PacketIndex {
            layer: 0,
            res: 0,
            comp: 0,
            precinct: 0,
        }
    );
}
