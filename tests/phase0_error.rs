use justjp2::error::Jp2Error;

#[test]
fn error_display() {
    let e = Jp2Error::InvalidMarker(0xFF4F);
    assert!(format!("{e}").contains("0xFF4F"));

    let e = Jp2Error::InvalidData("bad header".into());
    assert!(format!("{e}").contains("bad header"));

    let e = Jp2Error::BufferTooSmall { need: 100, have: 10 };
    let msg = format!("{e}");
    assert!(msg.contains("100"));
    assert!(msg.contains("10"));
}

#[test]
fn error_from_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "eof");
    let jp2_err: Jp2Error = io_err.into();
    match jp2_err {
        Jp2Error::Io(e) => assert_eq!(e.kind(), std::io::ErrorKind::UnexpectedEof),
        _ => panic!("expected Io variant"),
    }
}

#[test]
fn error_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Jp2Error>();
}

#[test]
fn error_source() {
    use std::error::Error;

    let io_err = std::io::Error::new(std::io::ErrorKind::Other, "test");
    let jp2_err = Jp2Error::Io(io_err);
    assert!(jp2_err.source().is_some());

    let jp2_err = Jp2Error::InvalidMarker(0);
    assert!(jp2_err.source().is_none());
}
