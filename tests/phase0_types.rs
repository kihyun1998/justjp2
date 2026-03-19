use justjp2::types::*;

#[test]
fn maxbands_value() {
    assert_eq!(J2K_MAXBANDS, 3 * 33 - 2);
    assert_eq!(J2K_MAXBANDS, 97);
}

#[test]
fn maxrlvls_value() {
    assert_eq!(J2K_MAXRLVLS, 33);
}

#[test]
fn prog_order_roundtrip() {
    let orders = [
        (ProgOrder::Lrcp, 0u8),
        (ProgOrder::Rlcp, 1),
        (ProgOrder::Rpcl, 2),
        (ProgOrder::Pcrl, 3),
        (ProgOrder::Cprl, 4),
    ];
    for (order, val) in orders {
        assert_eq!(order as u8, val);
        assert_eq!(ProgOrder::from_u8(val), Some(order));
    }
}

#[test]
fn prog_order_invalid() {
    assert_eq!(ProgOrder::from_u8(5), None);
    assert_eq!(ProgOrder::from_u8(255), None);
}

#[test]
fn color_space_default() {
    let cs: ColorSpace = Default::default();
    assert_eq!(cs, ColorSpace::Unknown);
}

#[test]
fn codec_format_variants() {
    assert_ne!(CodecFormat::J2k, CodecFormat::Jp2);
    assert_eq!(CodecFormat::J2k as u8, 0);
    assert_eq!(CodecFormat::Jp2 as u8, 1);
}

#[test]
fn quant_style_variants() {
    assert_eq!(QuantStyle::None as u8, 0);
    assert_eq!(QuantStyle::ScalarImplicit as u8, 1);
    assert_eq!(QuantStyle::ScalarExplicit as u8, 2);
}
