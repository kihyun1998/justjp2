/// 라이브러리 전역 에러 타입

use std::fmt;

/// justjp2 에러 타입
#[derive(Debug)]
pub enum Jp2Error {
    /// I/O 에러
    Io(std::io::Error),
    /// 유효하지 않은 마커
    InvalidMarker(u16),
    /// 유효하지 않은 데이터
    InvalidData(String),
    /// 지원하지 않는 기능
    UnsupportedFeature(String),
    /// 버퍼 크기 부족
    BufferTooSmall { need: usize, have: usize },
    /// 범위 초과
    OutOfBounds { offset: usize, len: usize },
    /// 잘못된 상태
    InvalidState(String),
}

impl fmt::Display for Jp2Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::InvalidMarker(m) => write!(f, "invalid marker: 0x{m:04X}"),
            Self::InvalidData(msg) => write!(f, "invalid data: {msg}"),
            Self::UnsupportedFeature(msg) => write!(f, "unsupported feature: {msg}"),
            Self::BufferTooSmall { need, have } => {
                write!(f, "buffer too small: need {need}, have {have}")
            }
            Self::OutOfBounds { offset, len } => {
                write!(f, "out of bounds: offset {offset}, length {len}")
            }
            Self::InvalidState(msg) => write!(f, "invalid state: {msg}"),
        }
    }
}

impl std::error::Error for Jp2Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Jp2Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// 라이브러리 Result 타입
pub type Result<T> = std::result::Result<T, Jp2Error>;
