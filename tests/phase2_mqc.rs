use justjp2::mqc::*;

// ── Step 2.1: 상태 테이블 ──

#[test]
fn state_count() {
    assert_eq!(MQC_STATES.len(), 47);
}

#[test]
fn state0_qe() {
    assert_eq!(MQC_STATES[0].qe, 0x5601);
}

#[test]
fn state46_is_uniform() {
    let s = &MQC_STATES[46];
    assert_eq!(s.qe, 0x5601);
    assert_eq!(s.nmps, 46);
    assert_eq!(s.nlps, 46);
    assert!(!s.switch);
}

#[test]
fn mps_transition_valid() {
    for (i, s) in MQC_STATES.iter().enumerate() {
        assert!(
            (s.nmps as usize) < 47,
            "state {i}: nmps {} out of range",
            s.nmps
        );
    }
}

#[test]
fn lps_transition_valid() {
    for (i, s) in MQC_STATES.iter().enumerate() {
        assert!(
            (s.nlps as usize) < 47,
            "state {i}: nlps {} out of range",
            s.nlps
        );
    }
}

// ── Step 2.2: MQ 인코더 ──

#[test]
fn encode_single_bit() {
    let mut enc = MqcEncoder::new();
    enc.encode(0, 0);
    enc.flush();
    assert!(enc.numbytes() > 0);
}

#[test]
fn encode_sequence() {
    let mut enc = MqcEncoder::new();
    for i in 0..20 {
        enc.encode(0, i % 2);
    }
    enc.flush();
    assert!(enc.numbytes() > 0);
}

#[test]
fn flush_produces_valid() {
    let mut enc = MqcEncoder::new();
    enc.encode(0, 1);
    enc.encode(0, 0);
    enc.encode(0, 1);
    enc.flush();
    let bytes = enc.to_vec();
    assert!(!bytes.is_empty());
    // 마지막 바이트가 0xFF로 끝나면 안 됨
    assert_ne!(*bytes.last().unwrap(), 0xFF);
}

#[test]
fn numbytes_after_encode() {
    let mut enc = MqcEncoder::new();
    enc.flush();
    let n1 = enc.numbytes();

    let mut enc2 = MqcEncoder::new();
    for _ in 0..100 {
        enc2.encode(0, 1);
    }
    enc2.flush();
    // 더 많은 데이터 → 더 많은 바이트
    assert!(enc2.numbytes() >= n1);
}

#[test]
fn context_switching() {
    let mut enc = MqcEncoder::new();
    enc.encode(0, 1);
    enc.encode(1, 0);
    enc.encode(2, 1);
    enc.encode(0, 0);
    enc.flush();
    assert!(enc.numbytes() > 0);
}

#[test]
fn resetstates_enc() {
    let mut enc = MqcEncoder::new();
    enc.setstate(0, 1, 10);
    enc.resetstates();
    // 리셋 후 인코딩해도 정상 작동
    enc.encode(0, 0);
    enc.flush();
    assert!(enc.numbytes() > 0);
}

// ── Step 2.3: MQ 디코더 ──

#[test]
fn decode_single_bit() {
    let mut enc = MqcEncoder::new();
    enc.encode(0, 1);
    enc.flush();

    let mut dec = MqcDecoder::new(&enc.to_vec());
    let d = dec.decode(0);
    assert_eq!(d, 1);
}

#[test]
fn encode_decode_roundtrip() {
    let bits = [0u32, 1, 1, 0, 1, 0, 0, 1, 1, 1, 0, 0];

    let mut enc = MqcEncoder::new();
    for &b in &bits {
        enc.encode(0, b);
    }
    enc.flush();

    let mut dec = MqcDecoder::new(&enc.to_vec());
    for (i, &expected) in bits.iter().enumerate() {
        let d = dec.decode(0);
        assert_eq!(d, expected, "bit {i} mismatch");
    }
}

#[test]
fn raw_mode_roundtrip() {
    // Raw 모드에서는 바이트를 직접 읽어서 비트 단위로 반환
    let data = [0b10110011u8, 0b01010101];
    let mut dec = MqcDecoder::new_raw(&data);

    // 첫 바이트: 10110011
    assert_eq!(dec.raw_decode(), 1);
    assert_eq!(dec.raw_decode(), 0);
    assert_eq!(dec.raw_decode(), 1);
    assert_eq!(dec.raw_decode(), 1);
    assert_eq!(dec.raw_decode(), 0);
    assert_eq!(dec.raw_decode(), 0);
    assert_eq!(dec.raw_decode(), 1);
    assert_eq!(dec.raw_decode(), 1);
}

#[test]
fn long_sequence_roundtrip() {
    let bits: Vec<u32> = (0..1000).map(|i| ((i * 7 + 3) % 2) as u32).collect();

    let mut enc = MqcEncoder::new();
    for &b in &bits {
        enc.encode(0, b);
    }
    enc.flush();

    let mut dec = MqcDecoder::new(&enc.to_vec());
    for (i, &expected) in bits.iter().enumerate() {
        let d = dec.decode(0);
        assert_eq!(d, expected, "bit {i} mismatch");
    }
}

#[test]
fn multi_context_roundtrip() {
    // 여러 컨텍스트를 교차 사용
    let data: Vec<(usize, u32)> = vec![
        (0, 1), (1, 0), (2, 1), (0, 0), (1, 1),
        (2, 0), (0, 1), (1, 1), (2, 0), (0, 0),
        (3, 1), (3, 1), (3, 0), (3, 0), (3, 1),
    ];

    let mut enc = MqcEncoder::new();
    for &(ctx, bit) in &data {
        enc.encode(ctx, bit);
    }
    enc.flush();

    let mut dec = MqcDecoder::new(&enc.to_vec());
    for (i, &(ctx, expected)) in data.iter().enumerate() {
        let d = dec.decode(ctx);
        assert_eq!(d, expected, "entry {i} (ctx={ctx}) mismatch");
    }
}

#[test]
fn ff_byte_handling() {
    // 0xFF가 출력에 포함되는 경우를 유발하는 시퀀스
    // 높은 확률로 0xFF 바이트를 생성하는 패턴
    let mut enc = MqcEncoder::new();
    // LPS만 계속 생성하면 빠르게 0xFF에 도달
    for _ in 0..200 {
        enc.encode(0, 1); // state 0에서 mps=0이므로 1은 LPS
    }
    enc.flush();

    let bytes = enc.to_vec();
    // 마지막 바이트가 0xFF가 아닌지 확인
    assert_ne!(*bytes.last().unwrap(), 0xFF);

    // 디코딩으로 검증
    let mut dec = MqcDecoder::new(&bytes);
    for i in 0..200 {
        let d = dec.decode(0);
        assert_eq!(d, 1, "bit {i} mismatch");
    }
}

#[test]
fn segmark_roundtrip() {
    // 세그먼트 마커는 ctx=18에 0,1,0,1 패턴
    let mut enc = MqcEncoder::new();
    enc.encode(0, 1);
    enc.encode(0, 0);
    enc.segmark();
    enc.flush();

    let mut dec = MqcDecoder::new(&enc.to_vec());
    assert_eq!(dec.decode(0), 1);
    assert_eq!(dec.decode(0), 0);
    // segmark: ctx=18, pattern 1,0,1,0 (i=1..5, i%2)
    assert_eq!(dec.decode(18), 1);
    assert_eq!(dec.decode(18), 0);
    assert_eq!(dec.decode(18), 1);
    assert_eq!(dec.decode(18), 0);
}

#[test]
fn all_zeros_roundtrip() {
    let mut enc = MqcEncoder::new();
    for _ in 0..500 {
        enc.encode(0, 0);
    }
    enc.flush();

    let mut dec = MqcDecoder::new(&enc.to_vec());
    for i in 0..500 {
        assert_eq!(dec.decode(0), 0, "bit {i}");
    }
}

#[test]
fn all_ones_roundtrip() {
    let mut enc = MqcEncoder::new();
    for _ in 0..500 {
        enc.encode(0, 1);
    }
    enc.flush();

    let mut dec = MqcDecoder::new(&enc.to_vec());
    for i in 0..500 {
        assert_eq!(dec.decode(0), 1, "bit {i}");
    }
}

#[test]
fn setstate_affects_coding() {
    // 같은 데이터를 다른 초기 상태로 인코딩하면 다른 결과
    let bits = [1u32, 0, 1, 0, 1, 1, 0, 0];

    let mut enc1 = MqcEncoder::new();
    for &b in &bits {
        enc1.encode(0, b);
    }
    enc1.flush();
    let bytes1 = enc1.to_vec();

    let mut enc2 = MqcEncoder::new();
    enc2.setstate(0, 0, 20); // 다른 초기 상태
    for &b in &bits {
        enc2.encode(0, b);
    }
    enc2.flush();
    let bytes2 = enc2.to_vec();

    // 다른 초기 상태 → 다른 바이트열
    assert_ne!(bytes1, bytes2);

    // 디코더도 같은 상태로 초기화하면 복원 가능
    let mut dec2 = MqcDecoder::new(&bytes2);
    dec2.setstate(0, 0, 20);
    for (i, &expected) in bits.iter().enumerate() {
        assert_eq!(dec2.decode(0), expected, "bit {i}");
    }
}
