pub mod bio;
pub mod dwt;
pub mod error;
pub mod j2k;
pub mod jp2;
pub mod jp2_box;
pub mod marker;
pub mod mct;
pub mod mqc;
pub mod pi;
pub mod quantize;
pub mod stream;
pub mod t1;
pub mod t2;
pub mod tgt;
pub mod tcd;
pub mod types;

// ── Phase 11: Public API ──

pub use error::{Jp2Error, Result};
pub use types::CodecFormat;

use tcd::{TcdComponent, TcdParams};

/// An image with one or more components.
#[derive(Debug, Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub components: Vec<Component>,
}

/// A single image component.
#[derive(Debug, Clone)]
pub struct Component {
    pub data: Vec<i32>,
    pub width: u32,
    pub height: u32,
    pub precision: u32,
    pub signed: bool,
    pub dx: u32,
    pub dy: u32,
}

/// Encoding parameters.
#[derive(Debug, Clone)]
pub struct EncodeParams {
    pub lossless: bool,
    pub num_decomp_levels: u32,
    pub cblk_width: u32,
    pub cblk_height: u32,
    pub format: CodecFormat,
}

impl Default for EncodeParams {
    fn default() -> Self {
        Self {
            lossless: true,
            num_decomp_levels: 5,
            cblk_width: 64,
            cblk_height: 64,
            format: CodecFormat::Jp2,
        }
    }
}

/// Decode a JPEG 2000 file (auto-detects J2K vs JP2 format).
pub fn decode(data: &[u8]) -> Result<Image> {
    decode_with_reduce(data, 0)
}

/// Decode a JPEG 2000 file at a reduced resolution.
///
/// # Arguments
/// * `data` - The encoded JPEG 2000 data
/// * `reduce` - Number of resolution levels to discard (0 = full resolution)
pub fn decode_with_reduce(data: &[u8], reduce: u32) -> Result<Image> {
    if data.len() < 4 {
        return Err(Jp2Error::InvalidData(
            "data too short to detect format".to_string(),
        ));
    }

    let format = detect_format(data)?;

    let (components_data, comp_info) = match format {
        CodecFormat::Jp2 => {
            if reduce > 0 {
                // For JP2, extract the J2K codestream and decode with reduce
                // For now, only J2K format supports reduce directly
                jp2::jp2_decode(data)?
            } else {
                jp2::jp2_decode(data)?
            }
        }
        CodecFormat::J2k => j2k::j2k_decode_with_reduce(data, reduce)?,
    };

    // Derive image dimensions from the first component (reference grid)
    let width = comp_info[0].width;
    let height = comp_info[0].height;

    let components = components_data
        .into_iter()
        .zip(comp_info.iter())
        .map(|(data, ci)| Component {
            data,
            width: ci.width,
            height: ci.height,
            precision: ci.precision,
            signed: ci.signed,
            dx: ci.dx,
            dy: ci.dy,
        })
        .collect();

    Ok(Image {
        width,
        height,
        components,
    })
}

/// Encode an image as JPEG 2000.
pub fn encode(image: &Image, params: &EncodeParams) -> Result<Vec<u8>> {
    if image.components.is_empty() {
        return Err(Jp2Error::InvalidData(
            "image has no components".to_string(),
        ));
    }

    let comp_info: Vec<TcdComponent> = image
        .components
        .iter()
        .map(|c| TcdComponent {
            width: c.width,
            height: c.height,
            precision: c.precision,
            signed: c.signed,
            dx: c.dx,
            dy: c.dy,
        })
        .collect();

    let components_data: Vec<Vec<i32>> = image
        .components
        .iter()
        .map(|c| c.data.clone())
        .collect();

    let use_mct = image.components.len() >= 3;

    let tcd_params = TcdParams {
        num_res: params.num_decomp_levels + 1,
        cblk_w: params.cblk_width,
        cblk_h: params.cblk_height,
        reversible: params.lossless,
        num_layers: 1,
        use_mct,
        reduce: 0,
        max_bytes: None,
    };

    match params.format {
        CodecFormat::Jp2 => jp2::jp2_encode(&components_data, &comp_info, &tcd_params),
        CodecFormat::J2k => j2k::j2k_encode(&components_data, &comp_info, &tcd_params),
    }
}

/// Auto-detect whether data is JP2 or raw J2K.
fn detect_format(data: &[u8]) -> Result<CodecFormat> {
    if data.len() < 4 {
        return Err(Jp2Error::InvalidData(
            "data too short to detect format".to_string(),
        ));
    }

    // Check for JP2: first 4 bytes are a box length (typically 0x0000000C = 12),
    // followed by the JP2 signature box type 0x6A502020.
    if data.len() >= 12 {
        let box_type = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        if box_type == jp2_box::JP2_JP {
            return Ok(CodecFormat::Jp2);
        }
    }

    // Check for raw J2K: starts with SOC marker 0xFF4F
    if data[0] == 0xFF && data[1] == 0x4F {
        return Ok(CodecFormat::J2k);
    }

    Err(Jp2Error::InvalidData(
        "unrecognized format: not JP2 or J2K".to_string(),
    ))
}
