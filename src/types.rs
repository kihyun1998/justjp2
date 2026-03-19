/// JPEG 2000 기본 타입 및 상수 정의

/// 최대 해상도 레벨 수
pub const J2K_MAXRLVLS: u32 = 33;

/// 최대 서브밴드 수: 3 * MAXRLVLS - 2
pub const J2K_MAXBANDS: u32 = 3 * J2K_MAXRLVLS - 2;

/// 최대 코드블록 크기
pub const J2K_MAX_CBLK_SIZE: u32 = 64;

/// MQC 컨텍스트 수
pub const MQC_NUMCTXS: usize = 19;

/// 진행 순서 (Progression Order)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ProgOrder {
    /// Layer-Resolution-Component-Precinct
    Lrcp = 0,
    /// Resolution-Layer-Component-Precinct
    Rlcp = 1,
    /// Resolution-Precinct-Component-Layer
    Rpcl = 2,
    /// Precinct-Component-Resolution-Layer
    Pcrl = 3,
    /// Component-Precinct-Resolution-Layer
    Cprl = 4,
}

impl ProgOrder {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Lrcp),
            1 => Some(Self::Rlcp),
            2 => Some(Self::Rpcl),
            3 => Some(Self::Pcrl),
            4 => Some(Self::Cprl),
            _ => None,
        }
    }
}

/// 색공간
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ColorSpace {
    #[default]
    Unknown = 0,
    Unspecified = 1,
    /// sRGB
    Srgb = 2,
    /// Grayscale
    Gray = 3,
    /// YCbCr (YUV)
    Ycc = 4,
    /// CMYK
    Cmyk = 5,
    /// e-YCC
    Eycc = 6,
}

/// 코덱 포맷
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CodecFormat {
    /// Raw J2K codestream
    J2k = 0,
    /// JP2 파일 포맷
    Jp2 = 1,
}

/// 양자화 스타일
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum QuantStyle {
    /// No quantization
    None = 0,
    /// Scalar implicit (derived)
    ScalarImplicit = 1,
    /// Scalar explicit
    ScalarExplicit = 2,
}
