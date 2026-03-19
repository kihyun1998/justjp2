# justjp2 — Pure Rust JPEG 2000 Implementation Roadmap

> 각 Phase/Step은 해당 테스트를 통과해야 완료로 간주한다.
> 테스트 경로: `tests/` 디렉토리 또는 각 모듈 내 `#[cfg(test)]`

---

## Phase 0: 기반 타입 및 유틸리티

모든 모듈이 공유하는 기본 타입, 에러, 바이트 I/O를 정의한다.

### Step 0.1 — 기본 타입 정의 (`types.rs`)

JPEG 2000에서 사용하는 상수와 enum을 정의한다. Rust 네이티브 타입(i32, u32 등)을 직접 사용.

```
- J2K_MAXRLVLS (33), J2K_MAXBANDS (97), J2K_MAX_CBLK_SIZE (64), MQC_NUMCTXS (19)
- ProgOrder enum (LRCP, RLCP, RPCL, PCRL, CPRL)
- ColorSpace enum (Unknown, Unspecified, sRGB, Grayscale, YCC, CMYK, eYCC)
- CodecFormat enum (J2K, JP2)
- QuantStyle enum (None, ScalarImplicit, ScalarExplicit)
```

**테스트**: `tests/phase0_types.rs`
```rust
#[test] fn maxbands_value()           // 3*33-2 == 97
#[test] fn maxrlvls_value()           // 33
#[test] fn prog_order_roundtrip()     // enum → u8 → enum
#[test] fn prog_order_invalid()       // 범위 밖 값 → None
#[test] fn color_space_default()      // Unknown이 기본값
#[test] fn codec_format_variants()    // J2K, JP2 구분
#[test] fn quant_style_variants()     // 양자화 스타일 값
```

### Step 0.2 — 에러 타입 (`error.rs`)

라이브러리 전역 에러 타입을 정의한다.

```
- Jp2Error enum (Io, InvalidMarker, InvalidData, UnsupportedFeature, BufferTooSmall, ...)
- Result<T> = std::result::Result<T, Jp2Error>
- Display, std::error::Error impl
```

**테스트**: `tests/phase0_error.rs`
```rust
#[test] fn error_display()            // 에러 메시지 포맷
#[test] fn error_from_io()            // std::io::Error → Jp2Error 변환
#[test] fn error_is_send_sync()       // Send + Sync 보장
#[test] fn error_source()             // std::error::Error::source() 체인
```

### Step 0.3 — 바이트 I/O (`stream.rs`)

Big-endian 바이트 읽기/쓰기 함수 및 커서 기반 Reader/Writer를 정의한다.

```
- read_u8, read_u16_be, read_u32_be, read_u64_be (슬라이스 함수)
- write_u8, write_u16_be, write_u32_be (슬라이스 함수)
- SliceReader: read, skip, seek, tell, remaining
- VecWriter: write, as_slice, into_vec
```

**테스트**: `tests/phase0_stream.rs`
```rust
#[test] fn read_u16_be_test()         // [0xFF, 0x4F] → 0xFF4F
#[test] fn read_u32_be_test()         // 4바이트 BE 읽기
#[test] fn write_roundtrip_u16()      // write → read 왕복
#[test] fn write_roundtrip_u32()      // write → read 왕복
#[test] fn slice_reader_remaining()   // 남은 바이트 수 추적
#[test] fn slice_reader_skip()        // skip 후 위치 확인
#[test] fn slice_reader_seek()        // seek 후 위치 확인
#[test] fn slice_reader_read_bytes()  // n바이트 읽기
#[test] fn slice_reader_read_u64_be() // u64 BE 읽기
#[test] fn slice_reader_seek_to_end_is_ok() // 끝까지 seek OK
#[test] fn vec_writer_basic()         // VecWriter 기본 동작
#[test] fn vec_writer_write_bytes()   // 바이트 슬라이스 쓰기
#[test] fn vec_writer_u64()           // u64 BE 쓰기
#[test] fn read_past_end_returns_err()// 범위 초과 시 에러
```

---

## Phase 1: 비트 레벨 기초 모듈

비트 단위 I/O와 태그 트리 — 상위 모듈의 기반이 되는 최하위 모듈.

### Step 1.1 — Bit I/O (`bio.rs`)

비트 단위 읽기/쓰기. MQC, T1, T2에서 사용한다.

```
- BioWriter: init, putbit, write(value, nbits), flush, numbytes
- BioReader: init, getbit, read(nbits), inalign, numbytes
- 0xFF 바이트 뒤 bit-stuffing 처리
```

**테스트**: `tests/phase1_bio.rs`
```rust
#[test] fn write_single_bits()        // 비트 하나씩 쓰고 결과 확인
#[test] fn write_nbits()              // write(0b1101, 4) 검증
#[test] fn read_single_bits()         // 비트 하나씩 읽기
#[test] fn read_nbits()               // read(4) → 0b1101
#[test] fn write_read_roundtrip()     // write 후 read로 동일 값 복원
#[test] fn flush_pads_to_byte()       // flush 후 바이트 정렬 확인
#[test] fn inalign_skips_padding()    // inalign 후 바이트 경계 정렬
#[test] fn bit_stuffing_after_ff()    // 0xFF 뒤 stuff bit 삽입/스킵
#[test] fn numbytes_tracking()        // 처리된 바이트 수 정확성
#[test] fn empty_stream()             // 빈 스트림 처리
#[test] fn multi_byte_roundtrip()     // 다양한 비트폭 데이터 왕복
```

### Step 1.2 — 태그 트리 (`tgt.rs`)

코드블록 inclusion/zero-bitplane 정보를 계층적으로 인코딩하는 트리.

```
- TgtNode { value, low, known }
- TgtTree: create(w, h), reset, setvalue, encode, decode
- 부모 노드 자동 생성 (ceil 기반 계층)
```

**테스트**: `tests/phase1_tgt.rs`
```rust
#[test] fn create_1x1()               // 1x1 트리 → 노드 1개
#[test] fn create_4x4()               // 4x4 트리 → 올바른 계층 수
#[test] fn create_3x2()               // 비정방 트리
#[test] fn setvalue_and_read()        // 값 설정 후 확인
#[test] fn reset_clears_state()       // reset 후 초기 상태
#[test] fn encode_decode_roundtrip()  // 인코딩 → 디코딩 값 일치
#[test] fn encode_threshold()         // threshold 이하만 인코딩
#[test] fn multi_leaf_encode_decode() // 여러 리프 순차 인코딩/디코딩
#[test] fn parent_propagation()       // 부모 노드 값 전파 확인
```

---

## Phase 2: MQ 산술 코더

EBCOT (T1)의 핵심. 컨텍스트 기반 이진 산술 코딩.

### Step 2.1 — MQ 코더 상태 테이블

47개 확률 상태 (Qe, MPS 전이, LPS 전이, MPS switch) 테이블.

```
- MqcState { qe: u32, nmps: u8, nlps: u8, switch: bool }
- MQC_STATES: [MqcState; 47] — ITU-T T.800 Table D.2
```

### Step 2.2 — MQ 인코더

```
- MqcEncoder: init, encode(ctx, bit), flush
- 19개 컨텍스트 (MQC_NUMCTXS)
- resetstates, setstate, segmark
- bypass 모드: bypass_init, bypass_enc, bypass_flush
- erterm (에러 종료), restart_init (재시작)
```

### Step 2.3 — MQ 디코더

```
- MqcDecoder: new(data), new_raw(data), decode(ctx), raw_decode
- renormalize, bytein (0xFF 핸들링)
- resetstates, setstate
```

> 상태 테이블 + 인코더 + 디코더를 `mqc.rs` 하나에 통합 구현.

**테스트**: `tests/phase2_mqc.rs`
```rust
// 상태 테이블
#[test] fn state_count()              // 47개 상태
#[test] fn state0_qe()                // state[0].qe == 0x5601
#[test] fn state46_is_uniform()       // 마지막 상태 확인
#[test] fn mps_transition_valid()     // nmps 인덱스 범위 확인
#[test] fn lps_transition_valid()     // nlps 인덱스 범위 확인
// 인코더
#[test] fn encode_single_bit()        // 1비트 인코딩
#[test] fn encode_sequence()          // 여러 비트 시퀀스
#[test] fn flush_produces_valid()     // flush 후 유효한 바이트열
#[test] fn numbytes_after_encode()    // 인코딩 후 바이트 수
#[test] fn context_switching()        // 컨텍스트 간 전환
#[test] fn resetstates_enc()          // 상태 리셋 후 기본값
#[test] fn bypass_mode_encode()       // raw 바이패스 모드
#[test] fn bypass_mode_roundtrip()    // 바이패스 모드 왕복
#[test] fn erterm_flush()             // 에러 종료 모드
#[test] fn segmark_roundtrip()        // 세그먼트 마커 왕복
// 디코더
#[test] fn decode_single_bit()        // 1비트 디코딩
#[test] fn encode_decode_roundtrip()  // enc → dec 완전 일치
#[test] fn raw_mode_roundtrip()       // raw 모드 왕복
#[test] fn long_sequence_roundtrip()  // 1000비트 시퀀스 왕복
#[test] fn multi_context_roundtrip()  // 여러 컨텍스트 왕복
#[test] fn ff_byte_handling()         // 0xFF 바이트 경계 처리
#[test] fn all_zeros_roundtrip()      // 전부 0 왕복
#[test] fn all_ones_roundtrip()       // 전부 1 왕복
#[test] fn setstate_affects_coding()  // 다른 초기 상태 → 다른 결과
```

---

## Phase 3: Tier-1 코딩 (EBCOT 블록 코더)

코드블록 단위의 비트플레인 인코딩/디코딩. MQC 위에 구축.

### Step 3.1 — T1 컨텍스트 계산 (`t1_ctx.rs`)

이웃 비트플레인 상태로부터 MQC 컨텍스트 번호를 계산한다.

```
- significance context (ZC): 9가지, orient(HL/LH/HH/LL)에 따라 다름
- sign context (SC): 5가지
- magnitude refinement context (MAG): 3가지
- 플래그 비트 레이아웃 (SIGMA, CHI, MU, PI)
```

**테스트**: `tests/phase3_t1_ctx.rs`
```rust
#[test] fn zc_context_no_neighbors()  // 이웃 없으면 ctx 0
#[test] fn zc_context_hl_band()       // HL 밴드 컨텍스트 테이블
#[test] fn zc_context_lh_band()       // LH 밴드 컨텍스트 테이블
#[test] fn zc_context_hh_band()       // HH 밴드 컨텍스트 테이블
#[test] fn sc_context_positive()      // 양수 부호 컨텍스트
#[test] fn sc_context_negative()      // 음수 부호 컨텍스트
#[test] fn mag_context_first_ref()    // 첫 리파인먼트
#[test] fn mag_context_subsequent()   // 이후 리파인먼트
```

### Step 3.2 — T1 코딩 패스 (`t1.rs`)

세 가지 코딩 패스: Significance Propagation, Magnitude Refinement, Cleanup.

```
- T1 { data: Vec<i32>, flags: Vec<u32>, w, h, mqc }
- encode_cblk / decode_cblk
- sigpass, refpass, clnpass (enc/dec 각각)
- BYPASS, RESET, TERMALL, VSC, CAUSAL 모드 지원
```

**테스트**: `tests/phase3_t1.rs`
```rust
// 컨텍스트 (Step 3.1)
#[test] fn zc_context_no_neighbors()      // 이웃 없으면 ctx 0
#[test] fn zc_context_hl_band()           // HL 밴드 컨텍스트
#[test] fn zc_context_lh_band()           // LH 밴드 컨텍스트
#[test] fn zc_context_hh_band()           // HH 밴드 컨텍스트
#[test] fn sc_context_positive()          // 양수 부호 컨텍스트
#[test] fn sc_context_negative()          // 음수 부호 컨텍스트
#[test] fn mag_context_first_ref()        // 첫 리파인먼트
#[test] fn mag_context_subsequent()       // 이후 리파인먼트
// 인코드/디코드 왕복 (Step 3.2)
#[test] fn encode_decode_zero_block()     // 전부 0인 블록
#[test] fn encode_decode_constant_block() // 동일 값 블록
#[test] fn encode_decode_gradient()       // 그라디언트 패턴
#[test] fn encode_decode_random_4x4()     // 4x4 랜덤 데이터
#[test] fn encode_decode_64x64()          // 최대 코드블록 크기
#[test] fn encode_decode_signed()         // 음수 포함 데이터
#[test] fn multiple_passes_count()        // 패스 수 = 3*numbps - 2
// 코드블록 스타일 모드
#[test] fn bypass_mode()                  // BYPASS(LAZY) 모드 왕복
#[test] fn reset_mode()                   // RESET 모드 왕복
#[test] fn termall_mode()                 // TERMALL 모드 왕복
```

---

## Phase 4: 이산 웨이블릿 변환 (DWT)

공간→주파수 도메인 변환. 5/3 (가역) 및 9/7 (비가역).

### Step 4.1 — 1D 리프팅

```
- dwt53_forward_1d / dwt53_inverse_1d (가역 5/3, 정수)
- dwt97_forward_1d / dwt97_inverse_1d (비가역 9/7, 실수)
- 경계 대칭 확장 (mirror), split/merge 내부 처리
```

### Step 4.2 — 2D DWT

```
- dwt53_forward_2d / dwt53_inverse_2d — 가역
- dwt97_forward_2d / dwt97_inverse_2d — 비가역
- 행 → 열 순서로 1D 적용, 다중 해상도 레벨 (재귀 LL)
```

> 1D + 2D를 `dwt.rs` 하나에 통합 구현.

**테스트**: `tests/phase4_dwt.rs`
```rust
// 1D 5/3
#[test] fn dwt53_encode_decode_4()        // 길이 4 왕복
#[test] fn dwt53_encode_decode_8()        // 길이 8 왕복
#[test] fn dwt53_encode_decode_odd()      // 홀수 길이
#[test] fn dwt53_single_sample()          // 길이 1 패스스루
#[test] fn dwt53_two_samples()            // 길이 2
#[test] fn dwt53_roundtrip_all_zeros()    // 전부 0
#[test] fn dwt53_roundtrip_length_3()     // 길이 3
#[test] fn dwt53_roundtrip_negative_values() // 음수 포함
// 1D 9/7
#[test] fn dwt97_encode_decode_8()        // 길이 8 왕복 (오차)
#[test] fn dwt97_encode_decode_odd()      // 홀수 길이
#[test] fn dwt97_precision()              // 오차 < 1e-6
// 2D
#[test] fn dwt53_2d_4x4_roundtrip()       // 4x4 왕복
#[test] fn dwt53_2d_8x8_roundtrip()       // 8x8 왕복
#[test] fn dwt53_2d_non_square()          // 직사각형 (8x4)
#[test] fn dwt53_2d_multi_level()         // 3레벨 분해/복원
#[test] fn dwt53_2d_single_level()        // 1레벨만
#[test] fn dwt53_2d_multi_level_non_power_of_two() // 비2멱 크기
#[test] fn dwt97_2d_8x8_roundtrip()       // 9/7 2D 왕복
#[test] fn dwt97_2d_multi_level()         // 9/7 다중 레벨
```

---

## Phase 5: 다중 컴포넌트 변환 (MCT)

색공간 변환 (RGB ↔ YCbCr).

### Step 5.1 — MCT (`mct.rs`)

```
- rct_forward / rct_inverse — 가역 (RCT): Y=floor((R+2G+B)/4), Cb=B-G, Cr=R-G
- ict_forward / ict_inverse — 비가역 (ICT): YCbCr 행렬
```

> DWT/MCT norm 테이블(getnorm)은 Phase 6(양자화)에서 구현.

**테스트**: `tests/phase5_mct.rs`
```rust
#[test] fn rct_encode_white()             // (255,255,255)
#[test] fn rct_encode_red()               // (255,0,0) 부호 확인
#[test] fn rct_roundtrip()                // forward→inverse 원본 복원
#[test] fn rct_roundtrip_random()         // 랜덤 데이터 왕복
#[test] fn ict_roundtrip()                // 비가역 왕복 (오차 허용)
#[test] fn ict_precision()                // 오차 < 0.5
#[test] fn mct_1000_samples()             // 대량 데이터 왕복
```

---

## Phase 6: 양자화 (Quantization)

DWT 계수를 양자화/역양자화한다.

### Step 6.1 — 양자화 (`quantize.rs`)

```
- StepSize { exponent: u8, mantissa: u16 }
- quantize_band(coeffs, stepsize, guard_bits) — 순방향
- dequantize_band(coeffs, stepsize, guard_bits) — 역방향
- calc_stepsizes(tccp, prec) — DWT norm 기반 스텝사이즈 계산
- QNTSTY_NOQNT (no quantization), QNTSTY_SIQNT (scalar implicit), QNTSTY_SEQNT (scalar explicit)
- DWT norm 테이블: dwt_getnorm(level, orient), dwt_getnorm_real(level, orient)
- MCT norm 테이블: mct_getnorm(compno), mct_getnorm_real(compno)
```

**테스트**: `tests/phase6_quantize.rs`
```rust
#[test] fn no_quantization_passthrough()  // NOQNT → 값 불변
#[test] fn scalar_quantize_dequantize()   // 양자화→역양자화 근사
#[test] fn stepsize_encode_decode()       // StepSize 바이트 표현 왕복
#[test] fn calc_stepsizes_5levels()       // 5레벨 스텝사이즈 개수 확인
#[test] fn guard_bits_effect()            // guard bits에 따른 범위
#[test] fn zero_coefficient()             // 0 계수 처리
#[test] fn dwt_norms()                    // DWT getnorm 테이블 검증
#[test] fn mct_norms()                    // MCT getnorm 테이블 검증
```

---

## Phase 7: Tier-2 코딩 및 패킷 이터레이터

패킷 헤더 파싱/조립과 진행 순서 관리.

### Step 7.1 — 패킷 이터레이터 (`pi.rs`)

```
- PiIterator { comp, res, precinct, layer, ... }
- 5가지 진행 순서: next_lrcp, next_rlcp, next_rpcl, next_pcrl, next_cprl
- pi_create(image, cp, tile_no) → PiIterator
```

**테스트**: `tests/phase7_pi.rs`
```rust
#[test] fn lrcp_order()                   // LRCP 순서 정확성
#[test] fn rlcp_order()                   // RLCP 순서 정확성
#[test] fn rpcl_order()                   // RPCL 순서 정확성
#[test] fn pcrl_order()                   // PCRL 순서 정확성
#[test] fn cprl_order()                   // CPRL 순서 정확성
#[test] fn packet_count()                 // 총 패킷 수 = L*R*C*P
#[test] fn single_tile_single_comp()      // 최소 케이스
```

### Step 7.2 — T2 패킷 인코딩/디코딩 (`t2.rs`)

```
- encode_packet(tile, comp, res, precinct, layer) → bytes
- decode_packet(bytes, tile, comp, res, precinct, layer)
- 패킷 헤더: inclusion (tgt), zero-bitplane (tgt), passes, lengths
- SOP/EPH 마커 옵션
```

**테스트**: `tests/phase7_t2.rs`
```rust
#[test] fn encode_decode_empty_packet()   // 빈 패킷 (no data)
#[test] fn encode_decode_single_cblk()    // 코드블록 1개
#[test] fn encode_decode_multi_cblk()     // 코드블록 여러개
#[test] fn sop_marker_present()           // SOP 마커 삽입 확인
#[test] fn eph_marker_present()           // EPH 마커 삽입 확인
#[test] fn inclusion_tag_tree()           // inclusion 태그트리 연동
```

---

## Phase 8: 타일 코더/디코더 (TCD)

전체 파이프라인 오케스트레이션: MCT → DWT → T1 → T2.

### Step 8.1 — TCD 구조체 및 타일 초기화 (`tcd.rs`)

```
- TcdImage, TcdTile, TcdTileComp, TcdResolution, TcdBand, TcdPrecinct, TcdCodeblock
- tcd_init(image, cp) → Tcd
- init_encode_tile / init_decode_tile
- 해상도/밴드/프리싱크/코드블록 그리드 계산
```

**테스트**: `tests/phase8_tcd.rs`
```rust
#[test] fn tile_grid_single_tile()        // 1타일 그리드
#[test] fn tile_grid_multi_tile()         // 4x4 타일 그리드
#[test] fn resolution_dimensions()        // 해상도별 크기 계산
#[test] fn band_dimensions()              // 밴드(LL,HL,LH,HH) 크기
#[test] fn precinct_count()               // 프리싱크 개수
#[test] fn codeblock_count()              // 코드블록 개수
#[test] fn codeblock_max_64x64()          // 코드블록 최대 64x64
```

### Step 8.2 — TCD 인코딩/디코딩 파이프라인 (`tcd_pipeline.rs`)

```
- encode_tile: DC shift → MCT → DWT → T1 → T2
- decode_tile: T2 → T1 → DWT → MCT → DC shift
- rate allocation (rate-distortion optimization)
```

**테스트**: `tests/phase8_tcd_pipeline.rs`
```rust
#[test] fn encode_decode_gray_tile()      // 8bit 그레이 타일 왕복
#[test] fn encode_decode_rgb_tile()       // RGB 타일 (MCT 포함) 왕복
#[test] fn lossless_gray()                // 가역 압축 → 무손실
#[test] fn lossy_psnr_threshold()         // 비가역 → PSNR > 30dB
#[test] fn rate_allocation()              // 레이트 할당 후 크기 제한
```

---

## Phase 9: J2K 코드스트림

마커 세그먼트 파싱 및 생성. 메인 헤더 + 타일 파트.

### Step 9.1 — 마커 정의 및 파서 (`marker.rs`)

```
- Marker enum (SOC=0xFF4F, SOT=0xFF90, SOD=0xFF93, EOC=0xFFD9, SIZ, COD, COC, QCD, QCC, ...)
- read_marker(stream) → Marker
- 각 마커별 read/write 함수
```

**테스트**: `tests/phase9_marker.rs`
```rust
#[test] fn parse_soc()                    // SOC 마커 인식
#[test] fn parse_eoc()                    // EOC 마커 인식
#[test] fn parse_siz()                    // SIZ: 이미지 크기, 컴포넌트 수
#[test] fn parse_cod()                    // COD: 코딩 스타일
#[test] fn parse_qcd()                    // QCD: 양자화 파라미터
#[test] fn write_read_siz_roundtrip()     // SIZ 쓰기→읽기 왕복
#[test] fn write_read_cod_roundtrip()     // COD 왕복
#[test] fn write_read_qcd_roundtrip()     // QCD 왕복
#[test] fn unknown_marker_skip()          // 알 수 없는 마커 건너뛰기
#[test] fn marker_segment_length()        // 마커 세그먼트 길이 검증
```

### Step 9.2 — J2K 코덱 (`j2k.rs`)

```
- J2kDecoder: read_header, decode_tile, decode
- J2kEncoder: setup, write_header, encode_tile, encode
- 타일 파트 관리
- POC, TLM, PLT 마커 지원
```

**테스트**: `tests/phase9_j2k.rs`
```rust
#[test] fn decode_minimal_j2k()           // 최소 J2K 스트림 디코딩
#[test] fn encode_minimal_j2k()           // 최소 J2K 스트림 인코딩
#[test] fn encode_decode_roundtrip()      // 인코딩→디코딩 왕복
#[test] fn header_parsing()               // 헤더 정보 추출
#[test] fn multi_tile_decode()            // 다중 타일 디코딩
#[test] fn specific_tile_decode()         // 특정 타일만 디코딩
#[test] fn reduce_resolution()            // 해상도 축소 디코딩
```

---

## Phase 10: JP2 파일 포맷

JP2 박스 기반 파일 포맷 래퍼. J2K 코드스트림을 감싼다.

### Step 10.1 — JP2 박스 파서 (`jp2_box.rs`)

```
- Jp2Box { box_type: u32, length: u64, data }
- 박스 타입: JP (signature), FTYP, JP2H, IHDR, COLR, JP2C, PCLR, CMAP, CDEF, RES, ...
- read_box / write_box
- read_box_header / write_box_header
```

**테스트**: `tests/phase10_jp2_box.rs`
```rust
#[test] fn read_signature_box()           // JP signature 박스
#[test] fn read_ftyp_box()                // File Type 박스
#[test] fn read_ihdr_box()                // Image Header 박스
#[test] fn read_colr_box_enum()           // Color Spec (enum)
#[test] fn read_colr_box_icc()            // Color Spec (ICC 프로필)
#[test] fn write_read_box_roundtrip()     // 박스 쓰기→읽기 왕복
#[test] fn nested_jp2h_box()              // JP2 Header 중첩 박스
#[test] fn extended_length_box()          // 확장 길이 (>2^32) 박스
```

### Step 10.2 — JP2 코덱 (`jp2.rs`)

```
- Jp2Decoder: read_header, decode → Image
- Jp2Encoder: setup, encode(Image) → bytes
- ICC 프로필 처리
- Palette (PCLR), Component Mapping (CMAP), Channel Definition (CDEF)
```

**테스트**: `tests/phase10_jp2.rs`
```rust
#[test] fn decode_minimal_jp2()           // 최소 JP2 파일 디코딩
#[test] fn encode_minimal_jp2()           // 최소 JP2 파일 인코딩
#[test] fn encode_decode_roundtrip()      // JP2 왕복
#[test] fn color_space_srgb()             // sRGB 색공간
#[test] fn color_space_grayscale()        // 그레이스케일
#[test] fn icc_profile_passthrough()      // ICC 프로필 보존
```

---

## Phase 11: Public API & 통합

사용자가 사용하는 최종 API. 파일 읽기/쓰기 통합.

### Step 11.1 — Public API (`lib.rs`)

```
- decode(reader) → Image
- encode(image, writer, params)
- Image { width, height, components: Vec<Component> }
- Component { data: Vec<i32>, width, height, precision, signed, dx, dy }
- EncodeParams { lossless, quality_layers, tile_size, ... }
```

**테스트**: `tests/phase11_api.rs`
```rust
#[test] fn decode_from_bytes()            // &[u8] → Image
#[test] fn encode_to_vec()                // Image → Vec<u8>
#[test] fn decode_j2k_format()            // J2K raw codestream
#[test] fn decode_jp2_format()            // JP2 파일 포맷
#[test] fn encode_j2k_format()            // J2K 인코딩
#[test] fn encode_jp2_format()            // JP2 인코딩
#[test] fn lossless_roundtrip_gray()      // 그레이 무손실 왕복
#[test] fn lossless_roundtrip_rgb()       // RGB 무손실 왕복
#[test] fn lossy_quality()                // 손실 압축 품질 확인
```

### Step 11.2 — 참조 호환성 테스트

openjpeg 과 동일한 입력에 대해 동일한 출력을 생성하는지 검증.

**테스트**: `tests/phase11_compat.rs`
```rust
#[test] fn compat_decode_gray_lossless()  // openjpeg 생성 파일 디코딩
#[test] fn compat_decode_rgb_lossless()   // openjpeg 생성 RGB 파일
#[test] fn compat_decode_lossy()          // openjpeg 생성 손실 파일
#[test] fn compat_multi_tile()            // 다중 타일 파일
#[test] fn compat_multi_resolution()      // 다중 해상도
```

---

## Phase 12: 확장 (선택)

### Step 12.1 — HTJ2K (Part 15) 디코더

```
- HT 블록 디코더: MEL, REV, UVLC, Forward stream
- CAP 마커 파싱
```

### Step 12.2 — 영역 지정 디코딩 (ROI)

```
- set_decode_area(x0, y0, x1, y1)
- 부분 DWT (partial decode)
```

### Step 12.3 — 멀티스레딩

```
- 타일 병렬 디코딩/인코딩
- DWT 행/열 병렬화
- T1 코드블록 병렬화
```

### Step 12.4 — SIMD 최적화

```
- DWT lifting SSE2/AVX2/NEON
- MCT SIMD
```

---

## 의존성 그래프

```
Phase 0 (types, error, stream)
  └─► Phase 1 (bio, tgt)
       └─► Phase 2 (mqc)
            └─► Phase 3 (t1)
       Phase 4 (dwt) ◄── Phase 0
       Phase 5 (mct) ◄── Phase 0
       Phase 6 (quantize) ◄── Phase 4
            └─► Phase 7 (pi, t2) ◄── Phase 1, Phase 3
                 └─► Phase 8 (tcd) ◄── Phase 3, 4, 5, 6, 7
                      └─► Phase 9 (j2k) ◄── Phase 8
                           └─► Phase 10 (jp2) ◄── Phase 9
                                └─► Phase 11 (api, compat)
                                     └─► Phase 12 (htj2k, roi, threads, simd)
```

---

## 체크리스트

| Phase | Step | 모듈 | 상태 |
|-------|------|------|------|
| 0 | 0.1 | types | ✅ |
| 0 | 0.2 | error | ✅ |
| 0 | 0.3 | stream | ✅ |
| 1 | 1.1 | bio | ✅ |
| 1 | 1.2 | tgt | ✅ |
| 2 | 2.1 | mqc_states | ✅ |
| 2 | 2.2 | mqc_enc | ✅ |
| 2 | 2.3 | mqc_dec | ✅ |
| 3 | 3.1 | t1_ctx | ✅ |
| 3 | 3.2 | t1 | ✅ |
| 4 | 4.1 | dwt_1d | ✅ |
| 4 | 4.2 | dwt_2d | ✅ |
| 5 | 5.1 | mct | ✅ |
| 6 | 6.1 | quantize | ✅ |
| 7 | 7.1 | pi | ✅ |
| 7 | 7.2 | t2 | ✅ |
| 8 | 8.1 | tcd | ✅ |
| 8 | 8.2 | tcd_pipeline | ✅ |
| 9 | 9.1 | marker | ✅ |
| 9 | 9.2 | j2k | ✅ |
| 10 | 10.1 | jp2_box | ⬜ |
| 10 | 10.2 | jp2 | ⬜ |
| 11 | 11.1 | api | ⬜ |
| 11 | 11.2 | compat | ⬜ |
| 12 | 12.1 | htj2k | ⬜ |
| 12 | 12.2 | roi | ⬜ |
| 12 | 12.3 | threads | ⬜ |
| 12 | 12.4 | simd | ⬜ |
