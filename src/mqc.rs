/// MQ 산술 코더 (ITU-T T.800 Annex C)
///
/// 컨텍스트 기반 이진 산술 코딩. JPEG 2000 Tier-1 (EBCOT)의 핵심.

use crate::types::MQC_NUMCTXS;

// ── 상태 테이블 (47개 상태) ──

/// MQ 코더 확률 상태
#[derive(Debug, Clone, Copy)]
pub struct MqcState {
    /// 확률 값 (Qe)
    pub qe: u32,
    /// MPS 후 다음 상태 인덱스
    pub nmps: u8,
    /// LPS 후 다음 상태 인덱스
    pub nlps: u8,
    /// LPS 시 MPS 반전 여부
    pub switch: bool,
}

/// ITU-T T.800 Table D.2 — 47개 확률 상태
pub static MQC_STATES: [MqcState; 47] = [
    MqcState { qe: 0x5601, nmps:  1, nlps:  1, switch: true  }, //  0
    MqcState { qe: 0x3401, nmps:  2, nlps:  6, switch: false }, //  1
    MqcState { qe: 0x1801, nmps:  3, nlps:  9, switch: false }, //  2
    MqcState { qe: 0x0ac1, nmps:  4, nlps: 12, switch: false }, //  3
    MqcState { qe: 0x0521, nmps:  5, nlps: 29, switch: false }, //  4
    MqcState { qe: 0x0221, nmps: 38, nlps: 33, switch: false }, //  5
    MqcState { qe: 0x5601, nmps:  7, nlps:  6, switch: true  }, //  6
    MqcState { qe: 0x5401, nmps:  8, nlps: 14, switch: false }, //  7
    MqcState { qe: 0x4801, nmps:  9, nlps: 14, switch: false }, //  8
    MqcState { qe: 0x3801, nmps: 10, nlps: 14, switch: false }, //  9
    MqcState { qe: 0x3001, nmps: 11, nlps: 17, switch: false }, // 10
    MqcState { qe: 0x2401, nmps: 12, nlps: 18, switch: false }, // 11
    MqcState { qe: 0x1c01, nmps: 13, nlps: 20, switch: false }, // 12
    MqcState { qe: 0x1601, nmps: 29, nlps: 21, switch: false }, // 13
    MqcState { qe: 0x5601, nmps: 15, nlps: 14, switch: true  }, // 14
    MqcState { qe: 0x5401, nmps: 16, nlps: 14, switch: false }, // 15
    MqcState { qe: 0x5101, nmps: 17, nlps: 15, switch: false }, // 16
    MqcState { qe: 0x4801, nmps: 18, nlps: 16, switch: false }, // 17
    MqcState { qe: 0x3801, nmps: 19, nlps: 17, switch: false }, // 18
    MqcState { qe: 0x3401, nmps: 20, nlps: 18, switch: false }, // 19
    MqcState { qe: 0x3001, nmps: 21, nlps: 19, switch: false }, // 20
    MqcState { qe: 0x2801, nmps: 22, nlps: 19, switch: false }, // 21
    MqcState { qe: 0x2401, nmps: 23, nlps: 20, switch: false }, // 22
    MqcState { qe: 0x2201, nmps: 24, nlps: 21, switch: false }, // 23
    MqcState { qe: 0x1c01, nmps: 25, nlps: 22, switch: false }, // 24
    MqcState { qe: 0x1801, nmps: 26, nlps: 23, switch: false }, // 25
    MqcState { qe: 0x1601, nmps: 27, nlps: 24, switch: false }, // 26
    MqcState { qe: 0x1401, nmps: 28, nlps: 25, switch: false }, // 27
    MqcState { qe: 0x1201, nmps: 29, nlps: 26, switch: false }, // 28
    MqcState { qe: 0x1101, nmps: 30, nlps: 27, switch: false }, // 29
    MqcState { qe: 0x0ac1, nmps: 31, nlps: 28, switch: false }, // 30
    MqcState { qe: 0x09c1, nmps: 32, nlps: 29, switch: false }, // 31
    MqcState { qe: 0x08a1, nmps: 33, nlps: 30, switch: false }, // 32
    MqcState { qe: 0x0521, nmps: 34, nlps: 31, switch: false }, // 33
    MqcState { qe: 0x0441, nmps: 35, nlps: 32, switch: false }, // 34
    MqcState { qe: 0x02a1, nmps: 36, nlps: 33, switch: false }, // 35
    MqcState { qe: 0x0221, nmps: 37, nlps: 34, switch: false }, // 36
    MqcState { qe: 0x0141, nmps: 38, nlps: 35, switch: false }, // 37
    MqcState { qe: 0x0111, nmps: 39, nlps: 36, switch: false }, // 38
    MqcState { qe: 0x0085, nmps: 40, nlps: 37, switch: false }, // 39
    MqcState { qe: 0x0049, nmps: 41, nlps: 38, switch: false }, // 40
    MqcState { qe: 0x0025, nmps: 42, nlps: 39, switch: false }, // 41
    MqcState { qe: 0x0015, nmps: 43, nlps: 40, switch: false }, // 42
    MqcState { qe: 0x0009, nmps: 44, nlps: 41, switch: false }, // 43
    MqcState { qe: 0x0005, nmps: 45, nlps: 42, switch: false }, // 44
    MqcState { qe: 0x0001, nmps: 45, nlps: 43, switch: false }, // 45
    MqcState { qe: 0x5601, nmps: 46, nlps: 46, switch: false }, // 46 (uniform)
];

/// 컨텍스트: 상태 인덱스 + MPS 값
#[derive(Debug, Clone, Copy)]
pub struct MqcContext {
    pub state: u8,
    pub mps: u8,
}

impl Default for MqcContext {
    fn default() -> Self {
        Self { state: 0, mps: 0 }
    }
}

// ── MQ 인코더 ──

/// MQ 산술 인코더
pub struct MqcEncoder {
    /// 출력 버퍼 (앞에 1바이트 여유 공간 포함)
    buf: Vec<u8>,
    /// A 레지스터 (interval)
    a: u32,
    /// C 레지스터 (code)
    c: u32,
    /// 카운터
    ct: u32,
    /// 현재 출력 위치 (buf 인덱스)
    bp: usize,
    /// 컨텍스트 배열
    ctxs: [MqcContext; MQC_NUMCTXS],
    /// 현재 컨텍스트 인덱스
    cur_ctx: usize,
}

impl MqcEncoder {
    pub fn new() -> Self {
        let mut buf = Vec::with_capacity(1024);
        // 앞에 더미 바이트 1개 (openjpeg의 bp = start - 1 패턴)
        buf.push(0x00);

        Self {
            buf,
            a: 0x8000,
            c: 0,
            ct: 12,
            bp: 0, // 더미 바이트 위치
            ctxs: [MqcContext::default(); MQC_NUMCTXS],
            cur_ctx: 0,
        }
    }

    /// 현재 컨텍스트 설정
    #[inline]
    pub fn set_cur_ctx(&mut self, ctx: usize) {
        self.cur_ctx = ctx;
    }

    /// 비트 인코딩
    pub fn encode(&mut self, ctx: usize, d: u32) {
        self.cur_ctx = ctx;
        let mps = self.ctxs[ctx].mps as u32;
        if mps == d {
            self.codemps();
        } else {
            self.codelps();
        }
    }

    /// 인코더 플러시
    pub fn flush(&mut self) {
        self.setbits();
        self.c <<= self.ct;
        self.byteout();
        self.c <<= self.ct;
        self.byteout();

        // 마지막 바이트가 0xFF가 아니면 포인터 전진
        if self.buf[self.bp] != 0xFF {
            self.bp += 1;
        }
    }

    /// 인코딩된 바이트 수
    /// flush 후: bp가 마지막 바이트 다음을 가리키거나 (0xFF가 아닌 경우),
    /// 마지막 0xFF를 가리킴. 유효 데이터 = buf[1..bp]
    pub fn numbytes(&self) -> usize {
        self.bp.saturating_sub(1)
    }

    /// 결과 바이트열 (더미 바이트 제외)
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[1..self.bp]
    }

    /// 결과 바이트열 복사
    pub fn to_vec(&self) -> Vec<u8> {
        self.buf[1..self.bp].to_vec()
    }

    /// 모든 컨텍스트를 기본 상태(state=0, mps=0)로 리셋
    pub fn resetstates(&mut self) {
        for ctx in &mut self.ctxs {
            ctx.state = 0;
            ctx.mps = 0;
        }
    }

    /// 특정 컨텍스트의 상태 설정
    pub fn setstate(&mut self, ctxno: usize, mps: u8, state: u8) {
        self.ctxs[ctxno].mps = mps;
        self.ctxs[ctxno].state = state;
    }

    /// 세그먼트 마커 인코딩 (1, 0, 1, 0 패턴)
    pub fn segmark(&mut self) {
        for i in 1..5u32 {
            self.encode(18, i % 2);
        }
    }

    /// 바이패스(raw) 모드 초기화
    /// flush() 후에 호출해야 한다.
    pub fn bypass_init(&mut self) {
        self.c = 0;
        self.ct = 8;
    }

    /// 바이패스 모드에서 비트 인코딩 (산술 코딩 없이 직접 비트 쓰기)
    pub fn bypass_enc(&mut self, d: u32) {
        self.ct -= 1;
        self.c += (d & 1) << self.ct;
        if self.ct == 0 {
            self.bp += 1;
            self.ensure_buf_size();
            self.buf[self.bp] = self.c as u8;
            self.ct = if self.buf[self.bp] == 0xFF { 7 } else { 8 };
            self.c = 0;
        }
    }

    /// 바이패스 모드 플러시
    pub fn bypass_flush(&mut self, erterm: bool) {
        if self.ct < 7 || (self.ct == 7 && (erterm || self.buf[self.bp] != 0xFF)) {
            // 남은 비트를 교차 패턴(0,1,0,1...)으로 채우기
            let mut bit_value = 0u32;
            while self.ct > 0 {
                self.ct -= 1;
                self.c += bit_value << self.ct;
                bit_value = 1 - bit_value;
            }
            self.bp += 1;
            self.ensure_buf_size();
            self.buf[self.bp] = self.c as u8;
        } else if self.ct == 7 && self.buf[self.bp] == 0xFF {
            // 마지막 0xFF 제거
            debug_assert!(!erterm);
            self.bp -= 1;
        } else if self.ct == 8
            && !erterm
            && self.bp >= 2
            && self.buf[self.bp] == 0x7F
            && self.buf[self.bp - 1] == 0xFF
        {
            // 0xFF 0x7F 최적화 제거
            self.bp -= 2;
        }
        // bp 전진 (numbytes 계산용)
        self.bp += 1;
    }

    /// 에러 종료 모드 (ERTERM) — 비트스트림에 명확한 종료 패턴 삽입
    pub fn erterm(&mut self) {
        let mut k = 11i32 - self.ct as i32 + 1;
        while k > 0 {
            self.c <<= self.ct;
            self.ct = 0;
            self.byteout();
            k -= self.ct as i32;
        }
        if self.buf[self.bp] != 0xFF {
            self.byteout();
        }
    }

    /// 재시작 초기화 (RESTART 모드용)
    /// flush() 후 새 산술 코딩 세그먼트를 시작한다.
    pub fn restart_init(&mut self) {
        self.a = 0x8000;
        self.c = 0;
        self.ct = 12;
        self.bp -= 1;
        if self.buf[self.bp] == 0xFF {
            self.ct = 13;
        }
    }

    // ── 내부 함수 ──

    fn codemps(&mut self) {
        let ctx = &self.ctxs[self.cur_ctx];
        let qe = MQC_STATES[ctx.state as usize].qe;
        let nmps = MQC_STATES[ctx.state as usize].nmps;

        self.a -= qe;
        if (self.a & 0x8000) == 0 {
            if self.a < qe {
                self.a = qe;
            } else {
                self.c += qe;
            }
            self.ctxs[self.cur_ctx].state = nmps;
            self.renorme();
        } else {
            self.c += qe;
        }
    }

    fn codelps(&mut self) {
        let ctx = &self.ctxs[self.cur_ctx];
        let qe = MQC_STATES[ctx.state as usize].qe;
        let nlps = MQC_STATES[ctx.state as usize].nlps;
        let switch = MQC_STATES[ctx.state as usize].switch;

        self.a -= qe;
        if self.a < qe {
            self.c += qe;
        } else {
            self.a = qe;
        }
        if switch {
            self.ctxs[self.cur_ctx].mps ^= 1;
        }
        self.ctxs[self.cur_ctx].state = nlps;
        self.renorme();
    }

    fn renorme(&mut self) {
        loop {
            self.a <<= 1;
            self.c <<= 1;
            self.ct -= 1;
            if self.ct == 0 {
                self.byteout();
            }
            if (self.a & 0x8000) != 0 {
                break;
            }
        }
    }

    fn byteout(&mut self) {
        if self.buf[self.bp] == 0xFF {
            self.bp += 1;
            self.ensure_buf_size();
            self.buf[self.bp] = (self.c >> 20) as u8;
            self.c &= 0xFFFFF;
            self.ct = 7;
        } else {
            if (self.c & 0x8000000) == 0 {
                self.bp += 1;
                self.ensure_buf_size();
                self.buf[self.bp] = (self.c >> 19) as u8;
                self.c &= 0x7FFFF;
                self.ct = 8;
            } else {
                self.buf[self.bp] += 1;
                if self.buf[self.bp] == 0xFF {
                    self.c &= 0x7FFFFFF;
                    self.bp += 1;
                    self.ensure_buf_size();
                    self.buf[self.bp] = (self.c >> 20) as u8;
                    self.c &= 0xFFFFF;
                    self.ct = 7;
                } else {
                    self.bp += 1;
                    self.ensure_buf_size();
                    self.buf[self.bp] = (self.c >> 19) as u8;
                    self.c &= 0x7FFFF;
                    self.ct = 8;
                }
            }
        }
    }

    fn setbits(&mut self) {
        let tempc = self.c.wrapping_add(self.a);
        self.c |= 0xFFFF;
        if self.c >= tempc {
            self.c -= 0x8000;
        }
    }

    fn ensure_buf_size(&mut self) {
        while self.bp >= self.buf.len() {
            self.buf.push(0);
        }
    }
}

impl Default for MqcEncoder {
    fn default() -> Self {
        Self::new()
    }
}

// ── MQ 디코더 ──

/// MQ 산술 디코더
pub struct MqcDecoder {
    /// 입력 데이터 (뒤에 0xFF 0xFF 센티널 추가)
    data: Vec<u8>,
    /// 현재 읽기 위치
    bp: usize,
    /// A 레지스터
    a: u32,
    /// C 레지스터
    c: u32,
    /// 카운터
    ct: u32,
    /// 컨텍스트 배열
    ctxs: [MqcContext; MQC_NUMCTXS],
    /// 현재 컨텍스트 인덱스
    cur_ctx: usize,
}

impl MqcDecoder {
    /// 디코더 초기화 (ISO 15444-1 C.3.5 INITDEC)
    pub fn new(data: &[u8]) -> Self {
        // 센티널 추가
        let mut buf = Vec::with_capacity(data.len() + 2);
        buf.extend_from_slice(data);
        buf.push(0xFF);
        buf.push(0xFF);

        let mut dec = Self {
            data: buf,
            bp: 0,
            a: 0x8000,
            c: 0,
            ct: 0,
            ctxs: [MqcContext::default(); MQC_NUMCTXS],
            cur_ctx: 0,
        };

        if data.is_empty() {
            dec.c = 0xFF << 16;
        } else {
            dec.c = (dec.data[0] as u32) << 16;
        }

        dec.bytein();
        dec.c <<= 7;
        dec.ct = dec.ct.saturating_sub(7);
        dec.a = 0x8000;

        dec
    }

    /// Raw 모드 디코더 초기화
    pub fn new_raw(data: &[u8]) -> Self {
        let mut buf = Vec::with_capacity(data.len() + 2);
        buf.extend_from_slice(data);
        buf.push(0xFF);
        buf.push(0xFF);

        Self {
            data: buf,
            bp: 0,
            a: 0,
            c: 0,
            ct: 0,
            ctxs: [MqcContext::default(); MQC_NUMCTXS],
            cur_ctx: 0,
        }
    }

    /// 현재 컨텍스트 설정
    #[inline]
    pub fn set_cur_ctx(&mut self, ctx: usize) {
        self.cur_ctx = ctx;
    }

    /// 비트 디코딩
    pub fn decode(&mut self, ctx: usize) -> u32 {
        self.cur_ctx = ctx;

        let state = self.ctxs[ctx].state as usize;
        let qe = MQC_STATES[state].qe;

        self.a -= qe;
        let d;

        if (self.c >> 16) < qe {
            // LPS exchange
            d = self.lpsexchange();
            self.renormd();
        } else {
            self.c -= qe << 16;
            if (self.a & 0x8000) == 0 {
                // MPS exchange
                d = self.mpsexchange();
                self.renormd();
            } else {
                d = self.ctxs[self.cur_ctx].mps as u32;
            }
        }

        d
    }

    /// Raw 모드 비트 디코딩
    pub fn raw_decode(&mut self) -> u32 {
        if self.ct == 0 {
            if self.c == 0xFF {
                if self.data[self.bp] > 0x8F {
                    self.c = 0xFF;
                    self.ct = 8;
                } else {
                    self.c = self.data[self.bp] as u32;
                    self.bp += 1;
                    self.ct = 7;
                }
            } else {
                self.c = self.data[self.bp] as u32;
                self.bp += 1;
                self.ct = 8;
            }
        }
        self.ct -= 1;
        (self.c >> self.ct) & 1
    }

    /// 모든 컨텍스트를 기본 상태로 리셋
    pub fn resetstates(&mut self) {
        for ctx in &mut self.ctxs {
            ctx.state = 0;
            ctx.mps = 0;
        }
    }

    /// 특정 컨텍스트의 상태 설정
    pub fn setstate(&mut self, ctxno: usize, mps: u8, state: u8) {
        self.ctxs[ctxno].mps = mps;
        self.ctxs[ctxno].state = state;
    }

    // ── 내부 함수 ──

    fn mpsexchange(&mut self) -> u32 {
        let ctx = &self.ctxs[self.cur_ctx];
        let state = ctx.state as usize;
        let qe = MQC_STATES[state].qe;

        if self.a < qe {
            // conditional LPS exchange
            let d = 1 - ctx.mps as u32;
            let nlps = MQC_STATES[state].nlps;
            let switch = MQC_STATES[state].switch;
            if switch {
                self.ctxs[self.cur_ctx].mps ^= 1;
            }
            self.ctxs[self.cur_ctx].state = nlps;
            d
        } else {
            let d = ctx.mps as u32;
            let nmps = MQC_STATES[state].nmps;
            self.ctxs[self.cur_ctx].state = nmps;
            d
        }
    }

    fn lpsexchange(&mut self) -> u32 {
        let ctx = &self.ctxs[self.cur_ctx];
        let state = ctx.state as usize;
        let qe = MQC_STATES[state].qe;

        if self.a < qe {
            self.a = qe;
            let d = ctx.mps as u32;
            let nmps = MQC_STATES[state].nmps;
            self.ctxs[self.cur_ctx].state = nmps;
            d
        } else {
            self.a = qe;
            let d = 1 - ctx.mps as u32;
            let nlps = MQC_STATES[state].nlps;
            let switch = MQC_STATES[state].switch;
            if switch {
                self.ctxs[self.cur_ctx].mps ^= 1;
            }
            self.ctxs[self.cur_ctx].state = nlps;
            d
        }
    }

    fn renormd(&mut self) {
        loop {
            if self.ct == 0 {
                self.bytein();
            }
            self.a <<= 1;
            self.c <<= 1;
            self.ct -= 1;
            if self.a >= 0x8000 {
                break;
            }
        }
    }

    fn bytein(&mut self) {
        let next_byte = self.data[self.bp + 1] as u32;
        if self.data[self.bp] == 0xFF {
            if next_byte > 0x8F {
                self.c += 0xFF00;
                self.ct = 8;
            } else {
                self.bp += 1;
                self.c += next_byte << 9;
                self.ct = 7;
            }
        } else {
            self.bp += 1;
            self.c += next_byte << 8;
            self.ct = 8;
        }
    }
}
