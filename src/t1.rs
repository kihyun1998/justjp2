/// Tier-1 EBCOT block coder (ITU-T T.800 Annex D)
///
/// Per-pixel flag-based implementation for clarity.
/// Implements significance propagation, magnitude refinement, and cleanup passes
/// using MQ arithmetic coding.

use crate::mqc::{MqcDecoder, MqcEncoder};

// ── Context label offsets (Table D.1 / D.2 / D.3) ──

pub const T1_CTXNO_ZC: usize = 0; // 9 contexts (0-8)
pub const T1_CTXNO_SC: usize = 9; // 5 contexts (9-13)
pub const T1_CTXNO_MAG: usize = 14; // 3 contexts (14-16)
pub const T1_CTXNO_AGG: usize = 17; // 1 context
pub const T1_CTXNO_UNI: usize = 18; // 1 context
pub const T1_NUMCTXS: usize = 19;

// ── Code-block style flags ──

bitflags::bitflags! {
    /// Code-block coding style (cblksty) flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CblkStyle: u8 {
        /// BYPASS (LAZY): use raw coding for sig/ref passes after 4th bitplane
        const BYPASS  = 0x01;
        /// RESET: reset MQC contexts after each pass
        const RESET   = 0x02;
        /// TERMALL: terminate all passes (flush after every pass)
        const TERMALL = 0x04;
        /// VSC: vertical stripe causal context (not implemented yet)
        const VSC     = 0x08;
        /// PTERM: predictable termination
        const PTERM   = 0x10;
        /// SEGSYM: segmentation symbols
        const SEGSYM  = 0x20;
    }
}

// ── Subband orientation ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orient {
    LL = 0,
    HL = 1,
    LH = 2,
    HH = 3,
}

// ── Per-pixel flags ──

#[derive(Debug, Clone, Copy, Default)]
struct CoeffFlags {
    /// sigma: coefficient is significant
    significant: bool,
    /// chi: sign of coefficient (true = negative)
    sign: bool,
    /// mu: has been refined at least once
    refined: bool,
    /// pi: visited in significance propagation pass this bitplane
    visited: bool,
}

// ── Pass info ──

/// Information about one coding pass within a code-block.
#[derive(Debug, Clone)]
pub struct CblkPass {
    /// Cumulative byte count at end of this pass.
    pub rate: usize,
    /// Pass type: 0 = significance propagation, 1 = magnitude refinement, 2 = cleanup.
    pub pass_type: u8,
}

// ── Sign-context LUT (Table D.3) ──
// Index: 8 bits built from (sig_N, sign_N, sig_E, sign_E, sig_S, sign_S, sig_W, sign_W)
// bit7=sig_N, bit6=sign_N, bit5=sig_E, bit4=sign_E, bit3=sig_S, bit2=sign_S, bit1=sig_W, bit0=sign_W
//
// We compute SC context and sign prediction bit (SPB) on the fly from the
// contribution rules in ITU-T T.800 Table D.3.

// ── T1 state ──

/// Tier-1 encoder/decoder state for a single code-block.
pub struct T1 {
    pub w: u32,
    pub h: u32,
    /// Coefficient data in sign-magnitude representation.
    /// Bit 31 = sign (1 = negative), bits 0..30 = magnitude.
    pub data: Vec<u32>,
    /// Per-pixel flag grid with 1-pixel border on each side.
    /// Dimensions: flags_stride * (h + 2).
    flags: Vec<CoeffFlags>,
    flags_stride: usize,
}

impl T1 {
    pub fn new(w: u32, h: u32) -> Self {
        let stride = (w + 2) as usize;
        let flag_count = stride * (h + 2) as usize;
        Self {
            w,
            h,
            data: vec![0u32; (w * h) as usize],
            flags: vec![CoeffFlags::default(); flag_count],
            flags_stride: stride,
        }
    }

    // ── Data conversion ──

    /// Load data from signed integers (two's complement) into sign-magnitude.
    pub fn set_data_from_i32(&mut self, input: &[i32]) {
        assert!(input.len() >= (self.w * self.h) as usize);
        for (i, &v) in input.iter().take((self.w * self.h) as usize).enumerate() {
            if v < 0 {
                self.data[i] = (1u32 << 31) | ((-v) as u32);
            } else {
                self.data[i] = v as u32;
            }
        }
    }

    /// Extract data as signed integers.
    pub fn get_data_as_i32(&self) -> Vec<i32> {
        self.data
            .iter()
            .map(|&v| {
                let mag = (v & 0x7FFF_FFFF) as i32;
                if v & (1 << 31) != 0 {
                    -mag
                } else {
                    mag
                }
            })
            .collect()
    }

    // ── Bit-plane count ──

    pub fn get_numbps(&self) -> u32 {
        let max_mag = self
            .data
            .iter()
            .map(|&v| v & 0x7FFF_FFFF)
            .max()
            .unwrap_or(0);
        if max_mag == 0 {
            0
        } else {
            32 - max_mag.leading_zeros()
        }
    }

    // ── Flag access helpers ──

    #[inline]
    fn fi(&self, x: u32, y: u32) -> usize {
        (y + 1) as usize * self.flags_stride + (x + 1) as usize
    }

    #[inline]
    fn di(&self, x: u32, y: u32) -> usize {
        y as usize * self.w as usize + x as usize
    }

    /// Returns whether any of the 8 neighbors is significant.
    fn has_significant_neighbor(&self, x: u32, y: u32) -> bool {
        let fi = self.fi(x, y);
        let s = self.flags_stride;
        self.flags[fi - s - 1].significant
            || self.flags[fi - s].significant
            || self.flags[fi - s + 1].significant
            || self.flags[fi - 1].significant
            || self.flags[fi + 1].significant
            || self.flags[fi + s - 1].significant
            || self.flags[fi + s].significant
            || self.flags[fi + s + 1].significant
    }

    /// Update flags after a coefficient becomes significant.
    pub fn update_flags(&mut self, x: u32, y: u32, sign: bool) {
        let fi = self.fi(x, y);
        self.flags[fi].significant = true;
        self.flags[fi].sign = sign;
    }

    // ── Context computation ──

    /// Zero-coding context (Table D.1).
    pub fn get_zc_ctx(&self, x: u32, y: u32, orient: Orient) -> usize {
        let fi = self.fi(x, y);
        let s = self.flags_stride;

        let sig_n = self.flags[fi - s].significant as u32;
        let sig_s = self.flags[fi + s].significant as u32;
        let sig_w = self.flags[fi - 1].significant as u32;
        let sig_e = self.flags[fi + 1].significant as u32;
        let sig_nw = self.flags[fi - s - 1].significant as u32;
        let sig_ne = self.flags[fi - s + 1].significant as u32;
        let sig_sw = self.flags[fi + s - 1].significant as u32;
        let sig_se = self.flags[fi + s + 1].significant as u32;

        let ctx = match orient {
            Orient::LL | Orient::LH => {
                let sum_h = sig_w + sig_e;
                let sum_v = sig_n + sig_s;
                let sum_d = sig_nw + sig_ne + sig_sw + sig_se;
                if sum_h == 2 {
                    8
                } else if sum_h == 1 && sum_v >= 1 {
                    7
                } else if sum_h == 1 && sum_d >= 1 {
                    6
                } else if sum_h == 1 {
                    5
                } else if sum_v == 2 {
                    4
                } else if sum_v == 1 {
                    3
                } else if sum_d >= 2 {
                    2
                } else if sum_d == 1 {
                    1
                } else {
                    0
                }
            }
            Orient::HL => {
                // Swap H and V roles compared to LL/LH
                let sum_h = sig_n + sig_s; // vertical becomes "horizontal" role
                let sum_v = sig_w + sig_e; // horizontal becomes "vertical" role
                let sum_d = sig_nw + sig_ne + sig_sw + sig_se;
                if sum_h == 2 {
                    8
                } else if sum_h == 1 && sum_v >= 1 {
                    7
                } else if sum_h == 1 && sum_d >= 1 {
                    6
                } else if sum_h == 1 {
                    5
                } else if sum_v == 2 {
                    4
                } else if sum_v == 1 {
                    3
                } else if sum_d >= 2 {
                    2
                } else if sum_d == 1 {
                    1
                } else {
                    0
                }
            }
            Orient::HH => {
                let sum_hv = sig_n + sig_s + sig_e + sig_w;
                let sum_d = sig_nw + sig_ne + sig_sw + sig_se;
                if sum_hv >= 3 {
                    8
                } else if sum_hv == 2 && sum_d >= 1 {
                    7
                } else if sum_hv == 2 {
                    6
                } else if sum_hv == 1 && sum_d >= 2 {
                    5
                } else if sum_hv == 1 && sum_d == 1 {
                    4
                } else if sum_hv == 1 {
                    3
                } else if sum_d >= 2 {
                    2
                } else if sum_d == 1 {
                    1
                } else {
                    0
                }
            }
        };

        T1_CTXNO_ZC + ctx
    }

    /// Sign-coding context and sign prediction bit (Table D.3).
    ///
    /// Returns (context_index, spb) where spb is the sign prediction bit
    /// (0 = predict positive, 1 = predict negative). The actual sign bit
    /// to encode is `actual_sign XOR spb`.
    pub fn get_sc_ctx_and_spb(&self, x: u32, y: u32) -> (usize, u32) {
        let fi = self.fi(x, y);
        let s = self.flags_stride;

        // Contribution for each direction:
        //  +1 if neighbor is significant and positive
        //  -1 if neighbor is significant and negative
        //   0 if neighbor is not significant

        let contrib = |f: &CoeffFlags| -> i32 {
            if !f.significant {
                0
            } else if f.sign {
                -1
            } else {
                1
            }
        };

        // Horizontal contribution
        let h = contrib(&self.flags[fi - 1]) + contrib(&self.flags[fi + 1]);
        // Vertical contribution
        let v = contrib(&self.flags[fi - s]) + contrib(&self.flags[fi + s]);

        // Table D.3: map (h, v) to (context, sign prediction)
        // h_contrib in {-2,-1,0,1,2}, v_contrib in {-2,-1,0,1,2}

        let (ctx_offset, xor_bit) = match (h, v) {
            (h, v) if h > 0 && v > 0 => (4, 0u32),
            (h, 0) if h > 0 => (3, 0),
            (0, v) if v > 0 => (3, 0),
            (0, 0) => (0, 0),
            (h, v) if h < 0 && v < 0 => (4, 1),
            (h, 0) if h < 0 => (3, 1),
            (0, v) if v < 0 => (3, 1),
            // Mixed signs: one positive, one negative (or one zero)
            (h, v) if h > 0 && v < 0 => (2, 0),
            (h, v) if h >= 0 && v < 0 => (1, 0),
            (h, v) if h < 0 && v > 0 => (2, 1),
            (h, v) if h < 0 && v >= 0 => (1, 1),
            // Remaining: h > 0, v < 0 and h < 0, v > 0 already covered
            // h == 0 cases covered, h > 0 v > 0, h < 0 v < 0 covered
            _ => {
                // This handles edge cases with magnitude 2 mixed
                if h + v >= 0 {
                    (1, 0)
                } else {
                    (1, 1)
                }
            }
        };

        (T1_CTXNO_SC + ctx_offset, xor_bit)
    }

    /// Magnitude refinement context (Table D.4).
    pub fn get_mag_ctx(&self, x: u32, y: u32) -> usize {
        let fi = self.fi(x, y);
        let s = self.flags_stride;

        let has_sig_neighbor = self.flags[fi - s - 1].significant
            || self.flags[fi - s].significant
            || self.flags[fi - s + 1].significant
            || self.flags[fi - 1].significant
            || self.flags[fi + 1].significant
            || self.flags[fi + s - 1].significant
            || self.flags[fi + s].significant
            || self.flags[fi + s + 1].significant;

        if !has_sig_neighbor {
            T1_CTXNO_MAG // 14
        } else if !self.flags[fi].refined {
            T1_CTXNO_MAG + 1 // 15 — first refinement
        } else {
            T1_CTXNO_MAG + 2 // 16 — subsequent refinements
        }
    }

    // ── Reset helpers ──

    fn reset_flags(&mut self) {
        for f in &mut self.flags {
            *f = CoeffFlags::default();
        }
    }

    fn clear_visited(&mut self) {
        for f in &mut self.flags {
            f.visited = false;
        }
    }

    fn init_mqc_contexts_enc(mqc: &mut MqcEncoder) {
        mqc.resetstates();
        mqc.setstate(T1_CTXNO_UNI, 0, 46); // Uniform context
        mqc.setstate(T1_CTXNO_AGG, 0, 3); // Aggregation context
        mqc.setstate(T1_CTXNO_ZC, 0, 4); // ZC context 0
    }

    fn init_mqc_contexts_dec(mqc: &mut MqcDecoder) {
        mqc.resetstates();
        mqc.setstate(T1_CTXNO_UNI, 0, 46);
        mqc.setstate(T1_CTXNO_AGG, 0, 3);
        mqc.setstate(T1_CTXNO_ZC, 0, 4);
    }

    // ── Encode ──

    /// Encode a code-block. Returns encoded bytes and per-pass information.
    ///
    /// `cblksty` controls coding modes (BYPASS, RESET, TERMALL, etc.).
    /// Pass `CblkStyle::empty()` for default (MQC-only, no termination).
    pub fn encode_cblk(
        &mut self,
        orient: Orient,
        cblksty: CblkStyle,
    ) -> (Vec<u8>, Vec<CblkPass>) {
        let numbps = self.get_numbps();
        if numbps == 0 {
            return (Vec::new(), Vec::new());
        }

        self.reset_flags();

        let termall = cblksty.contains(CblkStyle::TERMALL);
        let reset = cblksty.contains(CblkStyle::RESET);

        // When TERMALL: each pass gets its own MQC segment, concatenated.
        // Otherwise: single MQC stream for all passes.
        let mut all_bytes: Vec<u8> = Vec::new();
        let mut mqc = MqcEncoder::new();
        Self::init_mqc_contexts_enc(&mut mqc);

        let mut passes = Vec::new();
        let mut passtype: u8 = 2;
        let mut bpno = numbps - 1;
        let total_passes = 3 * numbps - 2;

        for passno in 0..total_passes {
            let is_last_pass = passno == total_passes - 1;

            match passtype {
                0 => self.enc_sigpass(&mut mqc, bpno, orient),
                1 => self.enc_refpass(&mut mqc, bpno),
                2 => self.enc_clnpass(&mut mqc, bpno, orient),
                _ => unreachable!(),
            }

            // TERMALL: flush after every pass (including last)
            if termall {
                mqc.flush();
                all_bytes.extend_from_slice(&mqc.to_vec());
                passes.push(CblkPass {
                    rate: all_bytes.len(),
                    pass_type: passtype,
                });
                if !is_last_pass {
                    mqc = MqcEncoder::new();
                    Self::init_mqc_contexts_enc(&mut mqc);
                }
            } else {
                passes.push(CblkPass {
                    rate: 0, // filled after final flush
                    pass_type: passtype,
                });
            }

            // RESET: reset contexts after each pass
            if reset && !termall {
                Self::init_mqc_contexts_enc(&mut mqc);
            }

            passtype += 1;
            if passtype == 3 {
                passtype = 0;
                if bpno == 0 {
                    break;
                }
                bpno -= 1;
            }
        }

        if termall {
            (all_bytes, passes)
        } else {
            mqc.flush();
            let encoded = mqc.to_vec();
            let final_rate = encoded.len();
            for p in &mut passes {
                p.rate = final_rate;
            }
            (encoded, passes)
        }
    }

    // ── Decode ──

    /// Decode a code-block.
    ///
    /// `data` — encoded byte stream
    /// `num_passes` — total number of coding passes to decode
    /// `orient` — subband orientation
    /// `roishift` — ROI bit shift (usually 0)
    /// `numbps` — number of significant bit-planes
    /// `cblksty` — code-block style flags (BYPASS, RESET, TERMALL, etc.)
    /// `pass_rates` — per-pass cumulative byte offsets (required for TERMALL; ignored otherwise)
    pub fn decode_cblk(
        &mut self,
        data: &[u8],
        num_passes: u32,
        orient: Orient,
        roishift: u32,
        numbps: u32,
        cblksty: CblkStyle,
        pass_rates: &[usize],
    ) {
        if num_passes == 0 || numbps == 0 {
            return;
        }

        self.reset_flags();
        for d in &mut self.data {
            *d = 0;
        }

        let termall = cblksty.contains(CblkStyle::TERMALL);
        let reset = cblksty.contains(CblkStyle::RESET);

        // For TERMALL, each pass is a separate segment.
        // pass_rates[i] = cumulative end offset for pass i.
        let mut seg_start = 0usize;

        let mut mqc = if termall && !pass_rates.is_empty() {
            let seg_end = pass_rates[0];
            let d = MqcDecoder::new(&data[..seg_end]);
            seg_start = seg_end;
            d
        } else {
            MqcDecoder::new(data)
        };
        Self::init_mqc_contexts_dec(&mut mqc);

        let mut passtype: u8 = 2;
        let mut bpno = roishift + numbps - 1;

        for p in 0..num_passes {
            match passtype {
                0 => self.dec_sigpass_mqc(&mut mqc, bpno, orient),
                1 => self.dec_refpass_mqc(&mut mqc, bpno),
                2 => self.dec_clnpass(&mut mqc, bpno, orient),
                _ => unreachable!(),
            }

            // TERMALL: init new decoder for next segment
            if termall {
                let next_pass = (p + 1) as usize;
                if next_pass < num_passes as usize && next_pass < pass_rates.len() {
                    let seg_end = pass_rates[next_pass];
                    mqc = MqcDecoder::new(&data[seg_start..seg_end]);
                    Self::init_mqc_contexts_dec(&mut mqc);
                    seg_start = seg_end;
                }
            }

            // RESET: reset contexts after each pass
            if reset && !termall {
                Self::init_mqc_contexts_dec(&mut mqc);
            }

            passtype += 1;
            if passtype == 3 {
                passtype = 0;
                if bpno == 0 {
                    break;
                }
                bpno -= 1;
            }
        }
    }

    // ── Significance Propagation Pass (encoder) ──

    fn enc_sigpass(&mut self, mqc: &mut MqcEncoder, bpno: u32, orient: Orient) {
        let w = self.w;
        let h = self.h;

        for y in 0..h {
            for x in 0..w {
                let fi = self.fi(x, y);
                if self.flags[fi].significant || self.flags[fi].visited {
                    continue;
                }
                if !self.has_significant_neighbor(x, y) {
                    continue;
                }

                let di = self.di(x, y);
                let mag = self.data[di] & 0x7FFF_FFFF;
                let bit = (mag >> bpno) & 1;

                let ctx = self.get_zc_ctx(x, y, orient);
                mqc.encode(ctx, bit);

                if bit == 1 {
                    let sign = if self.data[di] & (1 << 31) != 0 {
                        1u32
                    } else {
                        0u32
                    };
                    let (sc_ctx, spb) = self.get_sc_ctx_and_spb(x, y);
                    mqc.encode(sc_ctx, sign ^ spb);
                    self.update_flags(x, y, sign != 0);
                }

                self.flags[fi].visited = true;
            }
        }
    }

    // ── Magnitude Refinement Pass (encoder) ──

    fn enc_refpass(&mut self, mqc: &mut MqcEncoder, bpno: u32) {
        let w = self.w;
        let h = self.h;

        for y in 0..h {
            for x in 0..w {
                let fi = self.fi(x, y);
                if !self.flags[fi].significant || self.flags[fi].visited {
                    continue;
                }

                let di = self.di(x, y);
                let mag = self.data[di] & 0x7FFF_FFFF;
                let bit = (mag >> bpno) & 1;

                let ctx = self.get_mag_ctx(x, y);
                mqc.encode(ctx, bit);

                self.flags[fi].refined = true;
            }
        }
    }

    // ── Cleanup Pass (encoder) ──

    fn enc_clnpass(&mut self, mqc: &mut MqcEncoder, bpno: u32, orient: Orient) {
        let w = self.w;
        let h = self.h;

        // Process in vertical strips of 4
        let mut y = 0u32;
        while y < h {
            let strip_h = std::cmp::min(4, h - y);

            for x in 0..w {
                // Check if we can do aggregation for this strip of 4
                if strip_h == 4 {
                    // Check if all 4 are non-significant and not visited,
                    // and none have significant neighbors
                    let mut all_clear = true;
                    for dy in 0..4 {
                        let fi = self.fi(x, y + dy);
                        if self.flags[fi].significant
                            || self.flags[fi].visited
                            || self.has_significant_neighbor(x, y + dy)
                        {
                            all_clear = false;
                            break;
                        }
                    }

                    if all_clear {
                        // Check if all 4 bits are zero
                        let mut all_zero = true;
                        let mut first_nz = 4u32;
                        for dy in 0..4 {
                            let di = self.di(x, y + dy);
                            let mag = self.data[di] & 0x7FFF_FFFF;
                            if (mag >> bpno) & 1 != 0 {
                                if first_nz == 4 {
                                    first_nz = dy;
                                }
                                all_zero = false;
                            }
                        }

                        // Encode aggregation bit
                        mqc.encode(T1_CTXNO_AGG, if all_zero { 0 } else { 1 });

                        if all_zero {
                            // Skip all 4 — clear visited flags
                            for dy in 0..4 {
                                let fi = self.fi(x, y + dy);
                                self.flags[fi].visited = false;
                            }
                            continue;
                        }

                        // Encode run length (which of the 4 is first non-zero)
                        mqc.encode(T1_CTXNO_UNI, (first_nz >> 1) & 1);
                        mqc.encode(T1_CTXNO_UNI, first_nz & 1);

                        // The first non-zero is known to be significant — encode its sign
                        {
                            let di = self.di(x, y + first_nz);
                            let sign = if self.data[di] & (1 << 31) != 0 {
                                1u32
                            } else {
                                0u32
                            };
                            let (sc_ctx, spb) = self.get_sc_ctx_and_spb(x, y + first_nz);
                            mqc.encode(sc_ctx, sign ^ spb);
                            self.update_flags(x, y + first_nz, sign != 0);
                        }

                        // Mark up to first_nz as done, continue rest normally
                        for dy in 0..=first_nz {
                            let fi = self.fi(x, y + dy);
                            self.flags[fi].visited = false;
                        }

                        // Process remaining in the strip after first_nz
                        for dy in (first_nz + 1)..4 {
                            self.enc_clnpass_step(mqc, bpno, orient, x, y + dy);
                        }
                        continue;
                    }
                }

                // Non-aggregation: process each row in the strip
                for dy in 0..strip_h {
                    self.enc_clnpass_step(mqc, bpno, orient, x, y + dy);
                }
            }
            y += 4;
        }

        self.clear_visited();
    }

    /// Single coefficient step for cleanup pass (encoder).
    fn enc_clnpass_step(
        &mut self,
        mqc: &mut MqcEncoder,
        bpno: u32,
        orient: Orient,
        x: u32,
        y: u32,
    ) {
        let fi = self.fi(x, y);

        if self.flags[fi].significant {
            // Already significant — nothing to code in cleanup
            self.flags[fi].visited = false;
            return;
        }
        if self.flags[fi].visited {
            // Was coded in sigpass
            self.flags[fi].visited = false;
            return;
        }

        let di = self.di(x, y);
        let mag = self.data[di] & 0x7FFF_FFFF;
        let bit = (mag >> bpno) & 1;

        let ctx = self.get_zc_ctx(x, y, orient);
        mqc.encode(ctx, bit);

        if bit == 1 {
            let sign = if self.data[di] & (1 << 31) != 0 {
                1u32
            } else {
                0u32
            };
            let (sc_ctx, spb) = self.get_sc_ctx_and_spb(x, y);
            mqc.encode(sc_ctx, sign ^ spb);
            self.update_flags(x, y, sign != 0);
        }
    }

    // ── Significance Propagation Pass (decoder) ──

    fn dec_sigpass_mqc(&mut self, mqc: &mut MqcDecoder, bpno: u32, orient: Orient) {
        let w = self.w;
        let h = self.h;
        let one = 1u32 << bpno;
        let half = one >> 1;
        let oneplushalf = one | half;

        for y in 0..h {
            for x in 0..w {
                let fi = self.fi(x, y);
                if self.flags[fi].significant || self.flags[fi].visited {
                    continue;
                }
                if !self.has_significant_neighbor(x, y) {
                    continue;
                }

                let ctx = self.get_zc_ctx(x, y, orient);
                let bit = mqc.decode(ctx);

                if bit != 0 {
                    let (sc_ctx, spb) = self.get_sc_ctx_and_spb(x, y);
                    let sign_bit = mqc.decode(sc_ctx);
                    let sign = (sign_bit ^ spb) != 0;

                    let di = self.di(x, y);
                    self.data[di] = if sign { 1u32 << 31 } else { 0 } | oneplushalf;
                    self.update_flags(x, y, sign);
                }

                self.flags[fi].visited = true;
            }
        }
    }

    // ── Magnitude Refinement Pass (decoder) ──

    fn dec_refpass_mqc(&mut self, mqc: &mut MqcDecoder, bpno: u32) {
        let w = self.w;
        let h = self.h;
        let one = 1u32 << bpno;
        let poshalf = one >> 1;

        for y in 0..h {
            for x in 0..w {
                let fi = self.fi(x, y);
                if !self.flags[fi].significant || self.flags[fi].visited {
                    continue;
                }

                let ctx = self.get_mag_ctx(x, y);
                let bit = mqc.decode(ctx);

                let di = self.di(x, y);
                let sign_bit = self.data[di] & (1u32 << 31);
                let mut mag = self.data[di] & 0x7FFF_FFFF;

                if bit != 0 {
                    mag += poshalf;
                } else {
                    mag -= poshalf;
                }

                self.data[di] = sign_bit | mag;
                self.flags[fi].refined = true;
            }
        }
    }

    // ── Cleanup Pass (decoder) ──

    fn dec_clnpass(&mut self, mqc: &mut MqcDecoder, bpno: u32, orient: Orient) {
        let w = self.w;
        let h = self.h;
        let one = 1u32 << bpno;
        let half = one >> 1;
        let oneplushalf = one | half;

        let mut y = 0u32;
        while y < h {
            let strip_h = std::cmp::min(4, h - y);

            for x in 0..w {
                if strip_h == 4 {
                    // Check aggregation eligibility
                    let mut all_clear = true;
                    for dy in 0..4 {
                        let fi = self.fi(x, y + dy);
                        if self.flags[fi].significant
                            || self.flags[fi].visited
                            || self.has_significant_neighbor(x, y + dy)
                        {
                            all_clear = false;
                            break;
                        }
                    }

                    if all_clear {
                        let agg_bit = mqc.decode(T1_CTXNO_AGG);
                        if agg_bit == 0 {
                            // All four are zero
                            for dy in 0..4 {
                                let fi = self.fi(x, y + dy);
                                self.flags[fi].visited = false;
                            }
                            continue;
                        }

                        // Decode run length
                        let run_hi = mqc.decode(T1_CTXNO_UNI);
                        let run_lo = mqc.decode(T1_CTXNO_UNI);
                        let first_nz = (run_hi << 1) | run_lo;

                        // Decode sign for first non-zero
                        {
                            let (sc_ctx, spb) =
                                self.get_sc_ctx_and_spb(x, y + first_nz);
                            let sign_bit = mqc.decode(sc_ctx);
                            let sign = (sign_bit ^ spb) != 0;

                            let di = self.di(x, y + first_nz);
                            self.data[di] =
                                if sign { 1u32 << 31 } else { 0 } | oneplushalf;
                            self.update_flags(x, y + first_nz, sign);
                        }

                        for dy in 0..=first_nz {
                            let fi = self.fi(x, y + dy);
                            self.flags[fi].visited = false;
                        }

                        // Process rest of strip
                        for dy in (first_nz + 1)..4 {
                            self.dec_clnpass_step(
                                mqc,
                                bpno,
                                orient,
                                x,
                                y + dy,
                                oneplushalf,
                            );
                        }
                        continue;
                    }
                }

                // Non-aggregation path
                for dy in 0..strip_h {
                    self.dec_clnpass_step(mqc, bpno, orient, x, y + dy, oneplushalf);
                }
            }
            y += 4;
        }

        self.clear_visited();
    }

    fn dec_clnpass_step(
        &mut self,
        mqc: &mut MqcDecoder,
        _bpno: u32,
        orient: Orient,
        x: u32,
        y: u32,
        oneplushalf: u32,
    ) {
        let fi = self.fi(x, y);

        if self.flags[fi].significant {
            self.flags[fi].visited = false;
            return;
        }
        if self.flags[fi].visited {
            self.flags[fi].visited = false;
            return;
        }

        let ctx = self.get_zc_ctx(x, y, orient);
        let bit = mqc.decode(ctx);

        if bit != 0 {
            let (sc_ctx, spb) = self.get_sc_ctx_and_spb(x, y);
            let sign_bit = mqc.decode(sc_ctx);
            let sign = (sign_bit ^ spb) != 0;

            let di = self.di(x, y);
            self.data[di] = if sign { 1u32 << 31 } else { 0 } | oneplushalf;
            self.update_flags(x, y, sign);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_correct_sizes() {
        let t = T1::new(4, 4);
        assert_eq!(t.data.len(), 16);
        assert_eq!(t.flags.len(), 6 * 6);
        assert_eq!(t.flags_stride, 6);
    }

    #[test]
    fn test_data_roundtrip_i32() {
        let mut t = T1::new(2, 2);
        let input = vec![10, -20, 0, 30];
        t.set_data_from_i32(&input);
        let output = t.get_data_as_i32();
        assert_eq!(output, input);
    }
}
