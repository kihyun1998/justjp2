/// 바이트 스트림 I/O — Big-endian 읽기/쓰기 및 추상 스트림

use crate::error::{Jp2Error, Result};

// ── 바이트 읽기 (Big-endian) ──

/// 슬라이스에서 u8 읽기
#[inline]
pub fn read_u8(buf: &[u8], offset: usize) -> Result<u8> {
    buf.get(offset)
        .copied()
        .ok_or(Jp2Error::OutOfBounds { offset, len: 1 })
}

/// 슬라이스에서 u16 BE 읽기
#[inline]
pub fn read_u16_be(buf: &[u8], offset: usize) -> Result<u16> {
    let b = buf
        .get(offset..offset + 2)
        .ok_or(Jp2Error::OutOfBounds { offset, len: 2 })?;
    Ok(u16::from_be_bytes([b[0], b[1]]))
}

/// 슬라이스에서 u32 BE 읽기
#[inline]
pub fn read_u32_be(buf: &[u8], offset: usize) -> Result<u32> {
    let b = buf
        .get(offset..offset + 4)
        .ok_or(Jp2Error::OutOfBounds { offset, len: 4 })?;
    Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// 슬라이스에서 u64 BE 읽기
#[inline]
pub fn read_u64_be(buf: &[u8], offset: usize) -> Result<u64> {
    let b = buf
        .get(offset..offset + 8)
        .ok_or(Jp2Error::OutOfBounds { offset, len: 8 })?;
    Ok(u64::from_be_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}

// ── 바이트 쓰기 (Big-endian) ──

/// 슬라이스에 u8 쓰기
#[inline]
pub fn write_u8(buf: &mut [u8], offset: usize, val: u8) -> Result<()> {
    let slot = buf
        .get_mut(offset)
        .ok_or(Jp2Error::OutOfBounds { offset, len: 1 })?;
    *slot = val;
    Ok(())
}

/// 슬라이스에 u16 BE 쓰기
#[inline]
pub fn write_u16_be(buf: &mut [u8], offset: usize, val: u16) -> Result<()> {
    let b = buf
        .get_mut(offset..offset + 2)
        .ok_or(Jp2Error::OutOfBounds { offset, len: 2 })?;
    let bytes = val.to_be_bytes();
    b[0] = bytes[0];
    b[1] = bytes[1];
    Ok(())
}

/// 슬라이스에 u32 BE 쓰기
#[inline]
pub fn write_u32_be(buf: &mut [u8], offset: usize, val: u32) -> Result<()> {
    let b = buf
        .get_mut(offset..offset + 4)
        .ok_or(Jp2Error::OutOfBounds { offset, len: 4 })?;
    let bytes = val.to_be_bytes();
    b.copy_from_slice(&bytes);
    Ok(())
}

// ── SliceReader ──

/// 슬라이스 기반 읽기 커서
pub struct SliceReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> SliceReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// 현재 위치
    #[inline]
    pub fn tell(&self) -> usize {
        self.pos
    }

    /// 남은 바이트 수
    #[inline]
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    /// u8 읽기
    pub fn read_u8(&mut self) -> Result<u8> {
        let v = read_u8(self.data, self.pos)?;
        self.pos += 1;
        Ok(v)
    }

    /// u16 BE 읽기
    pub fn read_u16_be(&mut self) -> Result<u16> {
        let v = read_u16_be(self.data, self.pos)?;
        self.pos += 2;
        Ok(v)
    }

    /// u32 BE 읽기
    pub fn read_u32_be(&mut self) -> Result<u32> {
        let v = read_u32_be(self.data, self.pos)?;
        self.pos += 4;
        Ok(v)
    }

    /// u64 BE 읽기
    pub fn read_u64_be(&mut self) -> Result<u64> {
        let v = read_u64_be(self.data, self.pos)?;
        self.pos += 8;
        Ok(v)
    }

    /// n바이트 읽기
    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos + n;
        let slice = self.data.get(self.pos..end).ok_or(Jp2Error::OutOfBounds {
            offset: self.pos,
            len: n,
        })?;
        self.pos = end;
        Ok(slice)
    }

    /// n바이트 건너뛰기
    pub fn skip(&mut self, n: usize) -> Result<()> {
        let new_pos = self.pos + n;
        if new_pos > self.data.len() {
            return Err(Jp2Error::OutOfBounds {
                offset: self.pos,
                len: n,
            });
        }
        self.pos = new_pos;
        Ok(())
    }

    /// 절대 위치로 이동
    pub fn seek(&mut self, pos: usize) -> Result<()> {
        if pos > self.data.len() {
            return Err(Jp2Error::OutOfBounds {
                offset: pos,
                len: 0,
            });
        }
        self.pos = pos;
        Ok(())
    }
}

// ── VecWriter ──

/// Vec 기반 쓰기 버퍼
pub struct VecWriter {
    data: Vec<u8>,
}

impl VecWriter {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            data: Vec::with_capacity(cap),
        }
    }

    /// 현재 위치 (= 길이)
    #[inline]
    pub fn tell(&self) -> usize {
        self.data.len()
    }

    /// u8 쓰기
    pub fn write_u8(&mut self, val: u8) {
        self.data.push(val);
    }

    /// u16 BE 쓰기
    pub fn write_u16_be(&mut self, val: u16) {
        self.data.extend_from_slice(&val.to_be_bytes());
    }

    /// u32 BE 쓰기
    pub fn write_u32_be(&mut self, val: u32) {
        self.data.extend_from_slice(&val.to_be_bytes());
    }

    /// u64 BE 쓰기
    pub fn write_u64_be(&mut self, val: u64) {
        self.data.extend_from_slice(&val.to_be_bytes());
    }

    /// 바이트 슬라이스 쓰기
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }

    /// 내부 버퍼 참조
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// 내부 버퍼 소비하여 반환
    pub fn into_vec(self) -> Vec<u8> {
        self.data
    }
}

impl Default for VecWriter {
    fn default() -> Self {
        Self::new()
    }
}
