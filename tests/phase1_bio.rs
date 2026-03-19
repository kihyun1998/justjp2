use justjp2::bio::{BioReader, BioWriter};

#[test]
fn write_single_bits() {
    let mut w = BioWriter::new();
    w.putbit(1);
    w.putbit(0);
    w.putbit(1);
    w.putbit(1);
    w.putbit(0);
    w.putbit(0);
    w.putbit(1);
    w.putbit(0);
    w.flush().unwrap();
    // 10110010 = 0xB2
    assert_eq!(w.as_slice(), &[0xB2]);
}

#[test]
fn write_nbits() {
    let mut w = BioWriter::new();
    w.write(0b1101, 4);
    w.write(0b0011, 4);
    w.flush().unwrap();
    // 11010011 = 0xD3
    assert_eq!(w.as_slice(), &[0xD3]);
}

#[test]
fn read_single_bits() {
    let data = [0xB2]; // 10110010
    let mut r = BioReader::new(&data);
    assert_eq!(r.getbit().unwrap(), 1);
    assert_eq!(r.getbit().unwrap(), 0);
    assert_eq!(r.getbit().unwrap(), 1);
    assert_eq!(r.getbit().unwrap(), 1);
    assert_eq!(r.getbit().unwrap(), 0);
    assert_eq!(r.getbit().unwrap(), 0);
    assert_eq!(r.getbit().unwrap(), 1);
    assert_eq!(r.getbit().unwrap(), 0);
}

#[test]
fn read_nbits() {
    let data = [0xD3]; // 11010011
    let mut r = BioReader::new(&data);
    assert_eq!(r.read(4).unwrap(), 0b1101);
    assert_eq!(r.read(4).unwrap(), 0b0011);
}

#[test]
fn write_read_roundtrip() {
    let mut w = BioWriter::new();
    w.write(0b10101010, 8);
    w.write(0b11001100, 8);
    w.write(0b111, 3);
    w.flush().unwrap();

    let data = w.into_vec();
    let mut r = BioReader::new(&data);
    assert_eq!(r.read(8).unwrap(), 0b10101010);
    assert_eq!(r.read(8).unwrap(), 0b11001100);
    assert_eq!(r.read(3).unwrap(), 0b111);
}

#[test]
fn flush_pads_to_byte() {
    let mut w = BioWriter::new();
    w.write(0b101, 3);
    w.flush().unwrap();
    assert_eq!(w.numbytes(), 1);
    // 10100000 = 0xA0
    assert_eq!(w.as_slice(), &[0xA0]);
}

#[test]
fn inalign_skips_padding() {
    let data = [0xA0, 0x42]; // 10100000 01000010
    let mut r = BioReader::new(&data);
    r.read(3).unwrap(); // 101 읽기
    r.inalign().unwrap(); // 남은 비트 버림
    // 다음 바이트부터 읽어야 함
    assert_eq!(r.read(8).unwrap(), 0x42);
}

#[test]
fn bit_stuffing_after_ff() {
    // 0xFF 바이트를 쓰면 다음 바이트는 7비트만 사용
    let mut w = BioWriter::new();
    w.write(0xFF, 8); // 0xFF 출력
    w.write(0b1010101, 7); // 7비트만 (stuff bit 때문에)
    w.flush().unwrap();

    let data = w.into_vec();
    assert_eq!(data[0], 0xFF);

    // 읽기 시에도 0xFF 뒤는 7비트
    let mut r = BioReader::new(&data);
    assert_eq!(r.read(8).unwrap(), 0xFF);
    assert_eq!(r.read(7).unwrap(), 0b1010101);
}

#[test]
fn numbytes_tracking() {
    let mut w = BioWriter::new();
    assert_eq!(w.numbytes(), 0);
    w.write(0xFF, 8);
    w.flush().unwrap();
    assert_eq!(w.numbytes(), 1);
}

#[test]
fn empty_stream() {
    let data: [u8; 0] = [];
    let mut r = BioReader::new(&data);
    assert!(r.getbit().is_err());
}

#[test]
fn multi_byte_roundtrip() {
    let values: Vec<(u32, u32)> = vec![
        (0x1F, 5),
        (0x00, 3),
        (0xAB, 8),
        (0x07, 3),
        (0xFFFF, 16),
    ];

    let mut w = BioWriter::new();
    for &(val, bits) in &values {
        w.write(val, bits);
    }
    w.flush().unwrap();

    let data = w.into_vec();
    let mut r = BioReader::new(&data);
    for &(val, bits) in &values {
        let mask = (1u32 << bits) - 1;
        assert_eq!(r.read(bits).unwrap(), val & mask);
    }
}
