/// Phase 8: Tile Coder/Decoder (TCD)
///
/// Orchestrates the full encode/decode pipeline for a single tile:
/// - Encode: DC shift -> MCT -> DWT -> T1 -> assemble
/// - Decode: disassemble -> T1 -> inverse DWT -> inverse MCT -> DC unshift

use crate::dwt;
use crate::error::{Jp2Error, Result};
use crate::mct;
use crate::t1::{CblkStyle, Orient, T1};

/// Image component info
#[derive(Debug, Clone)]
pub struct TcdComponent {
    pub width: u32,
    pub height: u32,
    pub precision: u32,
    pub signed: bool,
    pub dx: u32,
    pub dy: u32,
}

/// Tile encoding/decoding parameters
#[derive(Debug, Clone)]
pub struct TcdParams {
    pub num_res: u32,
    pub cblk_w: u32,
    pub cblk_h: u32,
    pub reversible: bool,
    pub num_layers: u32,
    pub use_mct: bool,
}

impl Default for TcdParams {
    fn default() -> Self {
        Self {
            num_res: 6,
            cblk_w: 64,
            cblk_h: 64,
            reversible: true,
            num_layers: 1,
            use_mct: false,
        }
    }
}

/// Represents a tile's image data (all components).
#[derive(Debug, Clone)]
pub struct TileData {
    pub components: Vec<Vec<i32>>,
    pub width: u32,
    pub height: u32,
}

/// Encoded tile data (compressed).
#[derive(Debug)]
pub struct EncodedTile {
    pub data: Vec<u8>,
    /// Number of bit-planes per component (for decode)
    pub numbps: Vec<u32>,
}

// ---------------------------------------------------------------------------
// Subband geometry helpers
// ---------------------------------------------------------------------------

/// Info about one subband within a DWT decomposition.
#[derive(Debug, Clone)]
struct SubbandInfo {
    /// Orientation (LL, HL, LH, HH)
    orient: Orient,
    /// Width of the subband
    width: usize,
    /// Height of the subband
    height: usize,
    /// X offset within the component buffer (column start)
    x_off: usize,
    /// Y offset within the component buffer (row start)
    y_off: usize,
    /// Decomposition level (0 = coarsest)
    _level: u32,
}

/// Compute subband layout for a component after `num_decomp` levels of DWT.
///
/// The DWT stores data in-place with row-major layout stride = full component width.
/// At each level the current LL region (top-left) is split into LL, HL, LH, HH.
fn compute_subbands(comp_w: usize, comp_h: usize, num_decomp: u32) -> Vec<SubbandInfo> {
    if num_decomp == 0 {
        return vec![SubbandInfo {
            orient: Orient::LL,
            width: comp_w,
            height: comp_h,
            x_off: 0,
            y_off: 0,
            _level: 0,
        }];
    }

    // First compute LL sizes at each level
    let mut ws = Vec::with_capacity(num_decomp as usize + 1);
    let mut hs = Vec::with_capacity(num_decomp as usize + 1);
    ws.push(comp_w);
    hs.push(comp_h);
    for _ in 0..num_decomp {
        let w = *ws.last().unwrap();
        let h = *hs.last().unwrap();
        ws.push((w + 1) / 2);
        hs.push((h + 1) / 2);
    }

    let mut bands = Vec::new();

    // LL at the deepest level
    bands.push(SubbandInfo {
        orient: Orient::LL,
        width: ws[num_decomp as usize],
        height: hs[num_decomp as usize],
        x_off: 0,
        y_off: 0,
        _level: num_decomp,
    });

    // From deepest to finest
    for lev in (0..num_decomp).rev() {
        let idx = lev as usize;
        let w = ws[idx];
        let h = hs[idx];
        let ll_w = (w + 1) / 2;
        let ll_h = (h + 1) / 2;
        let hl_w = w - ll_w; // w / 2
        let lh_h = h - ll_h; // h / 2

        // HL: top-right
        bands.push(SubbandInfo {
            orient: Orient::HL,
            width: hl_w,
            height: ll_h,
            x_off: ll_w,
            y_off: 0,
            _level: lev + 1,
        });

        // LH: bottom-left
        bands.push(SubbandInfo {
            orient: Orient::LH,
            width: ll_w,
            height: lh_h,
            x_off: 0,
            y_off: ll_h,
            _level: lev + 1,
        });

        // HH: bottom-right
        bands.push(SubbandInfo {
            orient: Orient::HH,
            width: hl_w,
            height: lh_h,
            x_off: ll_w,
            y_off: ll_h,
            _level: lev + 1,
        });
    }

    bands
}

/// Extract a code-block from a subband within the component buffer.
fn extract_cblk(
    comp_buf: &[i32],
    comp_stride: usize,
    sb: &SubbandInfo,
    cb_x: usize,
    cb_y: usize,
    cb_w: usize,
    cb_h: usize,
) -> Vec<i32> {
    let mut out = vec![0i32; cb_w * cb_h];
    for r in 0..cb_h {
        let src_row = sb.y_off + cb_y + r;
        let src_col = sb.x_off + cb_x;
        for c in 0..cb_w {
            out[r * cb_w + c] = comp_buf[src_row * comp_stride + src_col + c];
        }
    }
    out
}

/// Place a code-block back into the component buffer.
fn place_cblk(
    comp_buf: &mut [i32],
    comp_stride: usize,
    sb: &SubbandInfo,
    cb_x: usize,
    cb_y: usize,
    cb_w: usize,
    cb_h: usize,
    cblk_data: &[i32],
) {
    for r in 0..cb_h {
        let dst_row = sb.y_off + cb_y + r;
        let dst_col = sb.x_off + cb_x;
        for c in 0..cb_w {
            comp_buf[dst_row * comp_stride + dst_col + c] = cblk_data[r * cb_w + c];
        }
    }
}

// ---------------------------------------------------------------------------
// Encoded code-block info stored in the bitstream for decode
// ---------------------------------------------------------------------------

/// Header for an encoded code-block stored in our simplified format.
#[derive(Debug, Clone)]
struct EncodedCblk {
    /// Component index
    comp: u16,
    /// Subband index within the component
    band: u16,
    /// Code-block x position within the subband (sample offset)
    cb_x: u16,
    /// Code-block y position within the subband (sample offset)
    cb_y: u16,
    /// Code-block width
    cb_w: u16,
    /// Code-block height
    cb_h: u16,
    /// Number of bit-planes
    numbps: u32,
    /// Number of coding passes
    num_passes: u32,
    /// Length of compressed data
    data_len: u32,
}

// ---------------------------------------------------------------------------
// Encode
// ---------------------------------------------------------------------------

/// Encode a tile: raw samples -> compressed bitstream.
pub fn encode_tile(
    tile: &TileData,
    components: &[TcdComponent],
    params: &TcdParams,
) -> Result<EncodedTile> {
    let num_comps = components.len();
    if tile.components.len() != num_comps {
        return Err(Jp2Error::InvalidData(
            "component count mismatch".to_string(),
        ));
    }

    let _w = tile.width as usize;
    let _h = tile.height as usize;
    let num_decomp = if params.num_res > 0 {
        params.num_res - 1
    } else {
        0
    };

    // Clone component data for in-place processing
    let mut comp_bufs: Vec<Vec<i32>> = tile.components.clone();

    // 1. DC level shift: for unsigned components, subtract 2^(precision-1)
    for (ci, comp) in components.iter().enumerate() {
        if !comp.signed {
            let shift = 1i32 << (comp.precision - 1);
            for v in &mut comp_bufs[ci] {
                *v -= shift;
            }
        }
    }

    // 2. MCT (if enabled and 3 components)
    if params.use_mct && num_comps >= 3 {
        if params.reversible {
            let (first, rest) = comp_bufs.split_at_mut(1);
            let (second, third) = rest.split_at_mut(1);
            mct::rct_forward(&mut first[0], &mut second[0], &mut third[0]);
        } else {
            // ICT operates on f32
            let mut c0f: Vec<f32> = comp_bufs[0].iter().map(|&v| v as f32).collect();
            let mut c1f: Vec<f32> = comp_bufs[1].iter().map(|&v| v as f32).collect();
            let mut c2f: Vec<f32> = comp_bufs[2].iter().map(|&v| v as f32).collect();
            mct::ict_forward(&mut c0f, &mut c1f, &mut c2f);
            let len = comp_bufs[0].len();
            for i in 0..len {
                comp_bufs[0][i] = c0f[i].round() as i32;
                comp_bufs[1][i] = c1f[i].round() as i32;
                comp_bufs[2][i] = c2f[i].round() as i32;
            }
        }
    }

    // 3. DWT per component
    for ci in 0..num_comps {
        let cw = components[ci].width as usize;
        let ch = components[ci].height as usize;
        if num_decomp > 0 {
            if params.reversible {
                dwt::dwt53_forward_2d(&mut comp_bufs[ci], cw, ch, num_decomp as usize);
            } else {
                // 9/7 DWT operates on f64
                let mut fdata: Vec<f64> = comp_bufs[ci].iter().map(|&v| v as f64).collect();
                dwt::dwt97_forward_2d(&mut fdata, cw, ch, num_decomp as usize);
                for i in 0..comp_bufs[ci].len() {
                    comp_bufs[ci][i] = fdata[i].round() as i32;
                }
            }
        }
    }

    // 4. T1 encode per code-block
    // Collect all encoded code-blocks
    let mut encoded_cblks: Vec<EncodedCblk> = Vec::new();
    let mut all_cblk_data: Vec<u8> = Vec::new();
    let mut max_numbps_per_comp: Vec<u32> = vec![0; num_comps];

    for ci in 0..num_comps {
        let cw = components[ci].width as usize;
        let ch = components[ci].height as usize;
        let subbands = compute_subbands(cw, ch, num_decomp);

        for (bi, sb) in subbands.iter().enumerate() {
            if sb.width == 0 || sb.height == 0 {
                continue;
            }

            // Iterate over code-blocks within this subband
            let cblk_w = params.cblk_w as usize;
            let cblk_h = params.cblk_h as usize;

            let mut cb_y = 0usize;
            while cb_y < sb.height {
                let cur_cb_h = (sb.height - cb_y).min(cblk_h);
                let mut cb_x = 0usize;
                while cb_x < sb.width {
                    let cur_cb_w = (sb.width - cb_x).min(cblk_w);

                    // Extract code-block data
                    let cblk_samples =
                        extract_cblk(&comp_bufs[ci], cw, sb, cb_x, cb_y, cur_cb_w, cur_cb_h);

                    // Create T1 and encode
                    let mut t1 = T1::new(cur_cb_w as u32, cur_cb_h as u32);
                    t1.set_data_from_i32(&cblk_samples);

                    let numbps = t1.get_numbps();
                    if numbps > max_numbps_per_comp[ci] {
                        max_numbps_per_comp[ci] = numbps;
                    }

                    let (enc_bytes, passes) = t1.encode_cblk(sb.orient, CblkStyle::empty());
                    let num_passes = passes.len() as u32;

                    let data_offset = all_cblk_data.len();
                    all_cblk_data.extend_from_slice(&enc_bytes);

                    encoded_cblks.push(EncodedCblk {
                        comp: ci as u16,
                        band: bi as u16,
                        cb_x: cb_x as u16,
                        cb_y: cb_y as u16,
                        cb_w: cur_cb_w as u16,
                        cb_h: cur_cb_h as u16,
                        numbps,
                        num_passes,
                        data_len: enc_bytes.len() as u32,
                    });

                    let _ = data_offset; // used implicitly via sequential appending

                    cb_x += cblk_w;
                }
                cb_y += cblk_h;
            }
        }
    }

    // 5. Assemble: simplified T2 format
    // Format: [num_cblks: u32] then for each cblk: [header fields] then [all data]
    let mut output = Vec::new();

    // Number of code-blocks
    let num_cblks = encoded_cblks.len() as u32;
    output.extend_from_slice(&num_cblks.to_le_bytes());

    // Write headers
    for ec in &encoded_cblks {
        output.extend_from_slice(&ec.comp.to_le_bytes());
        output.extend_from_slice(&ec.band.to_le_bytes());
        output.extend_from_slice(&ec.cb_x.to_le_bytes());
        output.extend_from_slice(&ec.cb_y.to_le_bytes());
        output.extend_from_slice(&ec.cb_w.to_le_bytes());
        output.extend_from_slice(&ec.cb_h.to_le_bytes());
        output.extend_from_slice(&ec.numbps.to_le_bytes());
        output.extend_from_slice(&ec.num_passes.to_le_bytes());
        output.extend_from_slice(&ec.data_len.to_le_bytes());
    }

    // Write all compressed data
    output.extend_from_slice(&all_cblk_data);

    Ok(EncodedTile {
        data: output,
        numbps: max_numbps_per_comp,
    })
}

// ---------------------------------------------------------------------------
// Decode
// ---------------------------------------------------------------------------

/// Decode a tile: compressed bitstream -> raw samples.
pub fn decode_tile(
    encoded: &EncodedTile,
    components: &[TcdComponent],
    params: &TcdParams,
    width: u32,
    height: u32,
) -> Result<TileData> {
    let num_comps = components.len();
    let _w = width as usize;
    let _h = height as usize;
    let num_decomp = if params.num_res > 0 {
        params.num_res - 1
    } else {
        0
    };

    let data = &encoded.data;
    if data.len() < 4 {
        return Err(Jp2Error::InvalidData("encoded tile too short".to_string()));
    }

    let num_cblks = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let header_size = 4 + num_cblks * 24; // 24 bytes per cblk header

    if data.len() < header_size {
        return Err(Jp2Error::InvalidData(
            "encoded tile header truncated".to_string(),
        ));
    }

    // Parse headers
    let mut cblk_headers = Vec::with_capacity(num_cblks);
    let mut offset = 4usize;
    for _ in 0..num_cblks {
        let comp = u16::from_le_bytes([data[offset], data[offset + 1]]);
        let band = u16::from_le_bytes([data[offset + 2], data[offset + 3]]);
        let cb_x = u16::from_le_bytes([data[offset + 4], data[offset + 5]]);
        let cb_y = u16::from_le_bytes([data[offset + 6], data[offset + 7]]);
        let cb_w = u16::from_le_bytes([data[offset + 8], data[offset + 9]]);
        let cb_h = u16::from_le_bytes([data[offset + 10], data[offset + 11]]);
        let numbps = u32::from_le_bytes([
            data[offset + 12],
            data[offset + 13],
            data[offset + 14],
            data[offset + 15],
        ]);
        let num_passes = u32::from_le_bytes([
            data[offset + 16],
            data[offset + 17],
            data[offset + 18],
            data[offset + 19],
        ]);
        let data_len = u32::from_le_bytes([
            data[offset + 20],
            data[offset + 21],
            data[offset + 22],
            data[offset + 23],
        ]);
        cblk_headers.push(EncodedCblk {
            comp,
            band,
            cb_x,
            cb_y,
            cb_w,
            cb_h,
            numbps,
            num_passes,
            data_len,
        });
        offset += 24;
    }

    // Initialize component buffers to zero
    let mut comp_bufs: Vec<Vec<i32>> = (0..num_comps)
        .map(|ci| {
            let cw = components[ci].width as usize;
            let ch = components[ci].height as usize;
            vec![0i32; cw * ch]
        })
        .collect();

    // Compute subbands per component
    let subbands_per_comp: Vec<Vec<SubbandInfo>> = (0..num_comps)
        .map(|ci| {
            let cw = components[ci].width as usize;
            let ch = components[ci].height as usize;
            compute_subbands(cw, ch, num_decomp)
        })
        .collect();

    // 1. T1 decode per code-block and place back
    let mut data_offset = header_size;
    for ec in &cblk_headers {
        let cblk_bytes = &data[data_offset..data_offset + ec.data_len as usize];
        data_offset += ec.data_len as usize;

        if ec.num_passes == 0 || ec.numbps == 0 {
            continue;
        }

        let ci = ec.comp as usize;
        let bi = ec.band as usize;
        let cw = components[ci].width as usize;

        let mut t1 = T1::new(ec.cb_w as u32, ec.cb_h as u32);
        t1.decode_cblk(
            cblk_bytes,
            ec.num_passes,
            subbands_per_comp[ci][bi].orient,
            0, // roishift
            ec.numbps,
            CblkStyle::empty(),
            &[], // pass_rates not needed without TERMALL
        );

        let decoded = t1.get_data_as_i32();
        place_cblk(
            &mut comp_bufs[ci],
            cw,
            &subbands_per_comp[ci][bi],
            ec.cb_x as usize,
            ec.cb_y as usize,
            ec.cb_w as usize,
            ec.cb_h as usize,
            &decoded,
        );
    }

    // 2. Inverse DWT per component
    for ci in 0..num_comps {
        let cw = components[ci].width as usize;
        let ch = components[ci].height as usize;
        if num_decomp > 0 {
            if params.reversible {
                dwt::dwt53_inverse_2d(&mut comp_bufs[ci], cw, ch, num_decomp as usize);
            } else {
                let mut fdata: Vec<f64> = comp_bufs[ci].iter().map(|&v| v as f64).collect();
                dwt::dwt97_inverse_2d(&mut fdata, cw, ch, num_decomp as usize);
                for i in 0..comp_bufs[ci].len() {
                    comp_bufs[ci][i] = fdata[i].round() as i32;
                }
            }
        }
    }

    // 3. Inverse MCT
    if params.use_mct && num_comps >= 3 {
        if params.reversible {
            let (first, rest) = comp_bufs.split_at_mut(1);
            let (second, third) = rest.split_at_mut(1);
            mct::rct_inverse(&mut first[0], &mut second[0], &mut third[0]);
        } else {
            let mut c0f: Vec<f32> = comp_bufs[0].iter().map(|&v| v as f32).collect();
            let mut c1f: Vec<f32> = comp_bufs[1].iter().map(|&v| v as f32).collect();
            let mut c2f: Vec<f32> = comp_bufs[2].iter().map(|&v| v as f32).collect();
            mct::ict_inverse(&mut c0f, &mut c1f, &mut c2f);
            let len = comp_bufs[0].len();
            for i in 0..len {
                comp_bufs[0][i] = c0f[i].round() as i32;
                comp_bufs[1][i] = c1f[i].round() as i32;
                comp_bufs[2][i] = c2f[i].round() as i32;
            }
        }
    }

    // 4. DC level unshift
    for (ci, comp) in components.iter().enumerate() {
        if !comp.signed {
            let shift = 1i32 << (comp.precision - 1);
            for v in &mut comp_bufs[ci] {
                *v += shift;
            }
        }
    }

    Ok(TileData {
        components: comp_bufs,
        width,
        height,
    })
}

// ---------------------------------------------------------------------------
// Public helper: subband/resolution/codeblock counting for tests
// ---------------------------------------------------------------------------

/// Returns the number of subbands for a component with `num_decomp` decomposition levels.
pub fn num_subbands(num_decomp: u32) -> u32 {
    if num_decomp == 0 {
        1
    } else {
        1 + 3 * num_decomp
    }
}

/// Returns (width, height) of the LL subband at the given resolution level.
/// Level 0 is the full resolution; level `num_decomp` is the smallest LL.
pub fn resolution_size(comp_w: u32, comp_h: u32, level: u32) -> (u32, u32) {
    let mut w = comp_w;
    let mut h = comp_h;
    for _ in 0..level {
        w = (w + 1) / 2;
        h = (h + 1) / 2;
    }
    (w, h)
}

/// Returns the number of code-blocks needed for a given subband size and code-block size.
pub fn codeblock_count(sb_w: u32, sb_h: u32, cblk_w: u32, cblk_h: u32) -> u32 {
    if sb_w == 0 || sb_h == 0 {
        return 0;
    }
    let nx = (sb_w + cblk_w - 1) / cblk_w;
    let ny = (sb_h + cblk_h - 1) / cblk_h;
    nx * ny
}

/// Compute subband dimensions for the given decomposition level (1-based) and orientation.
/// Returns (width, height).
/// `level` is 1-based (1 = finest detail, num_decomp = coarsest detail).
/// For level 0 (LL at coarsest), use `resolution_size(w, h, num_decomp)`.
pub fn band_dimensions(comp_w: u32, comp_h: u32, num_decomp: u32, level: u32, orient: Orient) -> (u32, u32) {
    if level == 0 {
        // LL band at coarsest level
        return resolution_size(comp_w, comp_h, num_decomp);
    }
    // level is 1..num_decomp
    // At this DWT level, the LL region being split has size = resolution_size(w, h, level-1)
    let (rw, rh) = resolution_size(comp_w, comp_h, level - 1);
    let ll_w = (rw + 1) / 2;
    let ll_h = (rh + 1) / 2;
    let hl_w = rw - ll_w;
    let lh_h = rh - ll_h;
    match orient {
        Orient::LL => (ll_w, ll_h),
        Orient::HL => (hl_w, ll_h),
        Orient::LH => (ll_w, lh_h),
        Orient::HH => (hl_w, lh_h),
    }
}
