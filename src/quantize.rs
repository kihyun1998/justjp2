/// Phase 6: Quantization
///
/// Implements JPEG 2000 scalar quantization as specified in ITU-T T.800.
/// Provides step size encoding/decoding, forward/inverse quantization,
/// DWT norm tables, and step size calculation for all subbands.

/// Quantization step size (exponent + mantissa representation)
/// As stored in QCD/QCC marker segments.
/// Effective step size = (1 + mantissa/2^11) * 2^(exponent - guard_bits)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepSize {
    /// 5-bit exponent
    pub exponent: u8,
    /// 11-bit mantissa
    pub mantissa: u16,
}

impl StepSize {
    /// Pack into 16-bit value: [exponent(5) | mantissa(11)]
    pub fn to_u16(&self) -> u16 {
        ((self.exponent as u16) << 11) | (self.mantissa & 0x7FF)
    }

    /// Unpack from 16-bit value
    pub fn from_u16(val: u16) -> Self {
        StepSize {
            exponent: (val >> 11) as u8,
            mantissa: val & 0x7FF,
        }
    }

    /// Get the effective floating-point step size given guard_bits.
    /// step = (1 + mantissa / 2048) * 2^(exponent - guard_bits)
    /// For no-quantization mode (mantissa == 0), this simplifies to 2^(exponent - guard_bits).
    pub fn to_f64(&self) -> f64 {
        let base = 1.0 + (self.mantissa as f64) / 2048.0;
        // exponent is typically (numbps - p) where p can be negative,
        // but for to_f64 we just return the raw representation.
        // The caller must account for guard_bits in the actual quantization.
        base * 2.0_f64.powi(self.exponent as i32)
    }
}

// ---------------------------------------------------------------------------
// DWT norm tables from openjpeg (dwt.c)
// ---------------------------------------------------------------------------

/// DWT 5/3 norms: [orient][level]
/// orient: 0=LL, 1=HL, 2=LH, 3=HH
static DWT_NORMS: [[f64; 10]; 4] = [
    [1.000, 1.500, 2.750, 5.375, 10.68, 21.34, 42.67, 85.33, 170.7, 341.3],
    [1.038, 1.592, 2.919, 5.703, 11.33, 22.64, 45.25, 90.48, 180.9, 0.0],
    [1.038, 1.592, 2.919, 5.703, 11.33, 22.64, 45.25, 90.48, 180.9, 0.0],
    [0.7186, 0.9218, 1.586, 3.043, 6.019, 12.01, 24.00, 47.97, 95.93, 0.0],
];

/// DWT 9/7 norms: [orient][level]
/// orient: 0=LL, 1=HL, 2=LH, 3=HH
static DWT_NORMS_REAL: [[f64; 10]; 4] = [
    [1.000, 1.965, 4.177, 8.403, 16.90, 33.84, 67.69, 135.3, 270.6, 540.9],
    [2.022, 3.989, 8.355, 17.04, 34.27, 68.63, 137.3, 274.6, 549.0, 0.0],
    [2.022, 3.989, 8.355, 17.04, 34.27, 68.63, 137.3, 274.6, 549.0, 0.0],
    [2.080, 3.865, 8.307, 17.18, 34.71, 69.59, 139.3, 278.6, 557.2, 0.0],
];

/// MCT norms (reversible)
static MCT_NORMS: [f64; 3] = [1.732, 0.8292, 0.8292];

/// MCT norms (irreversible)
static MCT_NORMS_REAL: [f64; 3] = [1.732, 1.805, 1.573];

/// DWT 5/3 norm for given decomposition level and subband orientation.
/// Level 0 = finest, orient: 0=LL, 1=HL, 2=LH, 3=HH.
/// Clamps to table bounds like openjpeg.
pub fn dwt_getnorm(level: u32, orient: u32) -> f64 {
    let level = if orient == 0 {
        level.min(9) as usize
    } else {
        level.min(8) as usize
    };
    DWT_NORMS[orient as usize][level]
}

/// DWT 9/7 norm for given decomposition level and subband orientation.
pub fn dwt_getnorm_real(level: u32, orient: u32) -> f64 {
    let level = if orient == 0 {
        level.min(9) as usize
    } else {
        level.min(8) as usize
    };
    DWT_NORMS_REAL[orient as usize][level]
}

/// MCT norm for component (reversible transform).
pub fn mct_getnorm(compno: u32) -> f64 {
    MCT_NORMS[compno as usize]
}

/// MCT norm for component (irreversible transform).
pub fn mct_getnorm_real(compno: u32) -> f64 {
    MCT_NORMS_REAL[compno as usize]
}

// ---------------------------------------------------------------------------
// Quantization / dequantization
// ---------------------------------------------------------------------------

/// Forward scalar quantization: coeff -> quantized index.
/// q = sign(c) * floor(|c| / step)
///
/// The effective step size is computed from the StepSize and guard_bits as:
///   step = (1 + mantissa/2048) * 2^(exponent - guard_bits)
/// but we work in fixed-point to avoid floating-point issues.
pub fn quantize_band(coeffs: &mut [i32], stepsize: &StepSize, guard_bits: u8) {
    // Effective step = (1 + mantissa/2048) * 2^(exponent - guard_bits)
    // We compute in floating point for simplicity.
    let exp_val = stepsize.exponent as i32 - guard_bits as i32;
    let step = (1.0 + stepsize.mantissa as f64 / 2048.0) * 2.0_f64.powi(exp_val);

    if step <= 0.0 {
        return;
    }

    for c in coeffs.iter_mut() {
        if *c == 0 {
            continue;
        }
        let sign = if *c < 0 { -1 } else { 1 };
        let abs_val = (*c as f64).abs();
        *c = sign * (abs_val / step).floor() as i32;
    }
}

/// Inverse scalar dequantization: quantized index -> reconstructed coefficient.
/// c' = q * step (with mid-point reconstruction: c' = sign(q) * (|q| + 0.5) * step)
///
/// Uses mid-point reconstruction for better quality.
pub fn dequantize_band(coeffs: &mut [i32], stepsize: &StepSize, guard_bits: u8) {
    let exp_val = stepsize.exponent as i32 - guard_bits as i32;
    let step = (1.0 + stepsize.mantissa as f64 / 2048.0) * 2.0_f64.powi(exp_val);

    if step <= 0.0 {
        return;
    }

    for c in coeffs.iter_mut() {
        if *c == 0 {
            continue;
        }
        let sign = if *c < 0 { -1.0 } else { 1.0 };
        let abs_val = (*c as f64).abs();
        // Mid-point reconstruction
        let reconstructed = sign * (abs_val + 0.5) * step;
        *c = reconstructed.round() as i32;
    }
}

/// No quantization passthrough (QNTSTY_NOQNT): just applies guard bit shift.
/// For the reversible 5/3 transform with no quantization, coefficients are
/// simply shifted by guard_bits to preserve dynamic range.
pub fn no_quantize(coeffs: &mut [i32], guard_bits: u8) {
    if guard_bits == 0 {
        return;
    }
    let shift = guard_bits as u32;
    for c in coeffs.iter_mut() {
        *c >>= shift;
    }
}

// ---------------------------------------------------------------------------
// Step size calculation
// ---------------------------------------------------------------------------

/// Encode a step size value into the StepSize representation, mirroring
/// openjpeg's `opj_dwt_encode_stepsize`.
///
/// `stepsize` is floor(raw_step * 8192)
/// `numbps` is prec + gain
fn encode_stepsize(stepsize: i32, numbps: i32) -> StepSize {
    let p = floorlog2(stepsize) - 13;
    let n = 11 - floorlog2(stepsize);
    let mant = if n < 0 {
        (stepsize >> (-n)) & 0x7FF
    } else {
        (stepsize << n) & 0x7FF
    };
    StepSize {
        exponent: (numbps - p) as u8,
        mantissa: mant as u16,
    }
}

/// Floor of log2 for positive integers.
fn floorlog2(mut val: i32) -> i32 {
    if val <= 0 {
        return 0;
    }
    let mut result = 0;
    while val > 1 {
        val >>= 1;
        result += 1;
    }
    result
}

/// Calculate quantization step sizes for all subbands based on DWT norms.
///
/// Returns step sizes ordered as: [LL, HL0, LH0, HH0, HL1, LH1, HH1, ...]
/// where index 0 is the coarsest level.
///
/// `num_res` = number of resolution levels (numresolutions)
/// `prec` = component bit precision
/// `is_reversible` = true for 5/3 wavelet, false for 9/7 wavelet
pub fn calc_stepsizes(num_res: u32, prec: u32, is_reversible: bool) -> Vec<StepSize> {
    let numbands = 3 * num_res - 2;
    let mut stepsizes = Vec::with_capacity(numbands as usize);

    for bandno in 0..numbands {
        let resno = if bandno == 0 { 0 } else { (bandno - 1) / 3 + 1 };
        let orient = if bandno == 0 { 0 } else { (bandno - 1) % 3 + 1 };
        let level = num_res - 1 - resno;

        // Gain depends on wavelet type:
        // For 9/7 (irreversible, qmfbid=0): gain = 0 for all
        // For 5/3 (reversible, qmfbid=1): gain = 0 for LL, 1 for HL/LH, 2 for HH
        let gain: u32 = if !is_reversible {
            0
        } else if orient == 0 {
            0
        } else if orient == 1 || orient == 2 {
            1
        } else {
            2
        };

        // For no-quantization (reversible), stepsize = 1.0
        // For scalar quantization (irreversible), stepsize = (1 << gain) / norm
        // But calc_stepsizes always computes explicit step sizes using the
        // 9/7 norms (as openjpeg does in opj_dwt_calc_explicit_stepsizes).
        let raw_stepsize = if is_reversible {
            // For reversible, no quantization: step = 1.0
            1.0
        } else {
            let norm = dwt_getnorm_real(level, orient);
            (1u32 << gain) as f64 / norm
        };

        let encoded = encode_stepsize(
            (raw_stepsize * 8192.0).floor() as i32,
            (prec + gain) as i32,
        );
        stepsizes.push(encoded);
    }

    stepsizes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepsize_roundtrip() {
        let ss = StepSize {
            exponent: 13,
            mantissa: 1024,
        };
        let encoded = ss.to_u16();
        let decoded = StepSize::from_u16(encoded);
        assert_eq!(ss, decoded);
    }

    #[test]
    fn stepsize_to_f64_basic() {
        // exponent=1, mantissa=0 -> (1 + 0/2048) * 2^1 = 2.0
        let ss = StepSize {
            exponent: 1,
            mantissa: 0,
        };
        assert!((ss.to_f64() - 2.0).abs() < 1e-10);
    }

    #[test]
    fn quantize_zero() {
        let mut coeffs = vec![0, 0, 0];
        let ss = StepSize {
            exponent: 10,
            mantissa: 0,
        };
        quantize_band(&mut coeffs, &ss, 2);
        assert_eq!(coeffs, vec![0, 0, 0]);
    }
}
