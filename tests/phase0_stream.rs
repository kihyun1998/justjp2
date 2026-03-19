use justjp2::stream::*;

// ── 바이트 읽기/쓰기 함수 ──

#[test]
fn read_u16_be_test() {
    let buf = [0xFF, 0x4F];
    assert_eq!(read_u16_be(&buf, 0).unwrap(), 0xFF4F);
}

#[test]
fn read_u32_be_test() {
    let buf = [0x00, 0x01, 0x02, 0x03];
    assert_eq!(read_u32_be(&buf, 0).unwrap(), 0x00010203);
}

#[test]
fn write_roundtrip_u16() {
    let mut buf = [0u8; 2];
    write_u16_be(&mut buf, 0, 0xABCD).unwrap();
    assert_eq!(read_u16_be(&buf, 0).unwrap(), 0xABCD);
}

#[test]
fn write_roundtrip_u32() {
    let mut buf = [0u8; 4];
    write_u32_be(&mut buf, 0, 0xDEADBEEF).unwrap();
    assert_eq!(read_u32_be(&buf, 0).unwrap(), 0xDEADBEEF);
}

#[test]
fn read_past_end_returns_err() {
    let buf = [0x01];
    assert!(read_u16_be(&buf, 0).is_err());
    assert!(read_u32_be(&buf, 0).is_err());
    assert!(read_u8(&buf, 1).is_err());
}

// ── SliceReader ──

#[test]
fn slice_reader_remaining() {
    let data = [1, 2, 3, 4, 5];
    let mut r = SliceReader::new(&data);
    assert_eq!(r.remaining(), 5);
    r.read_u8().unwrap();
    assert_eq!(r.remaining(), 4);
    r.read_u16_be().unwrap();
    assert_eq!(r.remaining(), 2);
}

#[test]
fn slice_reader_skip() {
    let data = [0u8; 10];
    let mut r = SliceReader::new(&data);
    r.skip(3).unwrap();
    assert_eq!(r.tell(), 3);
    r.skip(5).unwrap();
    assert_eq!(r.tell(), 8);
    assert!(r.skip(5).is_err()); // 범위 초과
}

#[test]
fn slice_reader_seek() {
    let data = [0u8; 10];
    let mut r = SliceReader::new(&data);
    r.seek(7).unwrap();
    assert_eq!(r.tell(), 7);
    r.seek(0).unwrap();
    assert_eq!(r.tell(), 0);
    assert!(r.seek(11).is_err()); // 범위 초과
}

#[test]
fn slice_reader_read_bytes() {
    let data = [10, 20, 30, 40, 50];
    let mut r = SliceReader::new(&data);
    let b = r.read_bytes(3).unwrap();
    assert_eq!(b, &[10, 20, 30]);
    assert_eq!(r.tell(), 3);
    assert!(r.read_bytes(5).is_err()); // 범위 초과
}

#[test]
fn slice_reader_read_u64_be() {
    let data = 0x0102030405060708u64.to_be_bytes();
    let mut r = SliceReader::new(&data);
    assert_eq!(r.read_u64_be().unwrap(), 0x0102030405060708);
}

// ── VecWriter ──

#[test]
fn vec_writer_basic() {
    let mut w = VecWriter::new();
    w.write_u8(0xFF);
    w.write_u16_be(0x4F00);
    w.write_u32_be(0xDEADBEEF);
    assert_eq!(w.tell(), 1 + 2 + 4);

    let data = w.into_vec();
    assert_eq!(data.len(), 7);
    assert_eq!(data[0], 0xFF);
    assert_eq!(read_u16_be(&data, 1).unwrap(), 0x4F00);
    assert_eq!(read_u32_be(&data, 3).unwrap(), 0xDEADBEEF);
}

#[test]
fn vec_writer_write_bytes() {
    let mut w = VecWriter::new();
    w.write_bytes(&[1, 2, 3]);
    w.write_bytes(&[4, 5]);
    assert_eq!(w.as_slice(), &[1, 2, 3, 4, 5]);
}

#[test]
fn vec_writer_u64() {
    let mut w = VecWriter::new();
    w.write_u64_be(0x0102030405060708);
    let mut r = SliceReader::new(w.as_slice());
    assert_eq!(r.read_u64_be().unwrap(), 0x0102030405060708);
}

#[test]
fn slice_reader_seek_to_end_is_ok() {
    let data = [0u8; 5];
    let mut r = SliceReader::new(&data);
    // seek to exactly the end should be OK (remaining == 0)
    r.seek(5).unwrap();
    assert_eq!(r.remaining(), 0);
    assert!(r.read_u8().is_err());
}
