/// 비트 단위 I/O — MQC, T1, T2에서 사용
///
/// JPEG 2000의 bit-stuffing 규칙:
/// 0xFF 바이트 뒤에는 최상위 비트를 0으로 강제 (stuff bit)

use crate::error::{Jp2Error, Result};

// ── BioWriter ──

/// 비트 단위 출력 스트림
pub struct BioWriter {
    buf: Vec<u8>,
    /// 현재 바이트 누적 값
    acc: u32,
    /// 현재 바이트에서 남은 비트 수
    bits: u32,
    /// 직전 바이트가 0xFF인지 (bit-stuffing용)
    last_was_ff: bool,
}

impl BioWriter {
    pub fn new() -> Self {
        Self {
            buf: Vec::new(),
            acc: 0,
            bits: 8,
            last_was_ff: false,
        }
    }

    /// 단일 비트 쓰기
    #[inline]
    pub fn putbit(&mut self, bit: u32) {
        if self.bits == 0 {
            self.byteout();
        }
        self.bits -= 1;
        self.acc |= (bit & 1) << self.bits;
    }

    /// n비트 쓰기 (MSB first)
    pub fn write(&mut self, value: u32, nbits: u32) {
        for i in (0..nbits).rev() {
            self.putbit((value >> i) & 1);
        }
    }

    /// 바이트 경계까지 0으로 패딩 후 플러시
    pub fn flush(&mut self) -> Result<()> {
        // 남은 비트를 0으로 채워 바이트를 완성
        if self.bits < 8 {
            self.byteout();
        }
        Ok(())
    }

    /// 처리된 바이트 수
    pub fn numbytes(&self) -> usize {
        self.buf.len()
    }

    /// 결과 바이트열 참조
    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    /// 결과 바이트열 소비
    pub fn into_vec(self) -> Vec<u8> {
        self.buf
    }

    /// 현재 누적 바이트를 버퍼에 출력
    fn byteout(&mut self) {
        let byte = self.acc as u8;
        self.buf.push(byte);
        self.last_was_ff = byte == 0xFF;
        self.acc = 0;
        // 0xFF 뒤에는 7비트만 사용 (stuff bit)
        self.bits = if self.last_was_ff { 7 } else { 8 };
    }
}

impl Default for BioWriter {
    fn default() -> Self {
        Self::new()
    }
}

// ── BioReader ──

/// 비트 단위 입력 스트림
pub struct BioReader<'a> {
    data: &'a [u8],
    pos: usize,
    /// 현재 바이트 값
    acc: u32,
    /// 남은 비트 수
    bits: u32,
    /// 직전 바이트가 0xFF인지
    last_was_ff: bool,
}

impl<'a> BioReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            acc: 0,
            bits: 0,
            last_was_ff: false,
        }
    }

    /// 단일 비트 읽기
    #[inline]
    pub fn getbit(&mut self) -> Result<u32> {
        if self.bits == 0 {
            self.bytein()?;
        }
        self.bits -= 1;
        Ok((self.acc >> self.bits) & 1)
    }

    /// n비트 읽기 (MSB first)
    pub fn read(&mut self, nbits: u32) -> Result<u32> {
        let mut value = 0u32;
        for _ in 0..nbits {
            value = (value << 1) | self.getbit()?;
        }
        Ok(value)
    }

    /// 바이트 경계로 정렬 (남은 비트 버림)
    pub fn inalign(&mut self) -> Result<()> {
        self.bits = 0;
        self.last_was_ff = false;
        Ok(())
    }

    /// 처리된 바이트 수
    pub fn numbytes(&self) -> usize {
        self.pos
    }

    /// 다음 바이트를 읽어오기
    fn bytein(&mut self) -> Result<()> {
        if self.pos >= self.data.len() {
            return Err(Jp2Error::OutOfBounds {
                offset: self.pos,
                len: 1,
            });
        }
        self.acc = self.data[self.pos] as u32;
        self.pos += 1;
        // 직전이 0xFF였으면 7비트만 사용
        self.bits = if self.last_was_ff { 7 } else { 8 };
        self.last_was_ff = self.acc == 0xFF;
        Ok(())
    }
}
