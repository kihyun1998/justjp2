/// Phase 3 — Tier-1 EBCOT block coder tests

use justjp2::t1::*;

const NO_STYLE: CblkStyle = CblkStyle::empty();

// ── ZC context tests ──

#[test]
fn zc_context_no_neighbors() {
    // No significant neighbors → context 0
    let t = T1::new(4, 4);
    let ctx = t.get_zc_ctx(1, 1, Orient::LL);
    assert_eq!(ctx, T1_CTXNO_ZC); // 0
}

#[test]
fn zc_context_hl_band() {
    // HL band: vertical neighbors play the "horizontal" role
    // Set north neighbor significant → should give context for sumH=1, sumV=0, sumD=0 → 5
    let mut t = T1::new(4, 4);
    t.update_flags(1, 0, false); // north of (1,1)
    let ctx = t.get_zc_ctx(1, 1, Orient::HL);
    // In HL, N/S are "horizontal": sumH=1, sumV(E/W)=0, sumD=0 → 5
    assert_eq!(ctx, T1_CTXNO_ZC + 5);
}

#[test]
fn zc_context_lh_band() {
    // LH band: same as LL
    // Set west neighbor significant → sumH=1, sumV=0, sumD=0 → 5
    let mut t = T1::new(4, 4);
    t.update_flags(0, 1, false); // west of (1,1)
    let ctx = t.get_zc_ctx(1, 1, Orient::LH);
    assert_eq!(ctx, T1_CTXNO_ZC + 5);
}

#[test]
fn zc_context_hh_band() {
    // HH band: sumHV=1, sumD=0 → 3
    let mut t = T1::new(4, 4);
    t.update_flags(1, 0, false); // north of (1,1)
    let ctx = t.get_zc_ctx(1, 1, Orient::HH);
    assert_eq!(ctx, T1_CTXNO_ZC + 3);
}

// ── SC context tests ──

#[test]
fn sc_context_positive() {
    // Positive neighbor context
    let mut t = T1::new(4, 4);
    t.update_flags(0, 1, false); // west neighbor positive
    let (ctx, spb) = t.get_sc_ctx_and_spb(1, 1);
    assert!(ctx >= T1_CTXNO_SC && ctx <= T1_CTXNO_SC + 4);
    // With one positive horizontal neighbor, spb should predict positive
    assert_eq!(spb, 0);
}

#[test]
fn sc_context_negative() {
    // Negative neighbor context
    let mut t = T1::new(4, 4);
    t.update_flags(0, 1, true); // west neighbor negative
    let (ctx, spb) = t.get_sc_ctx_and_spb(1, 1);
    assert!(ctx >= T1_CTXNO_SC && ctx <= T1_CTXNO_SC + 4);
    // With one negative horizontal neighbor, spb should predict negative
    assert_eq!(spb, 1);
}

// ── MAG context tests ──

#[test]
fn mag_context_first_ref() {
    // First refinement: significant neighbor exists, not yet refined
    let mut t = T1::new(4, 4);
    t.update_flags(0, 1, false); // significant neighbor
    let ctx = t.get_mag_ctx(1, 1);
    // Has significant neighbor, coeff at (1,1) not yet refined → 15
    assert_eq!(ctx, T1_CTXNO_MAG + 1);
}

#[test]
fn mag_context_subsequent() {
    // No significant neighbor → ctx 14
    let t = T1::new(4, 4);
    let ctx = t.get_mag_ctx(1, 1);
    assert_eq!(ctx, T1_CTXNO_MAG);
}

// ── Encode/decode roundtrip tests ──

/// Helper: encode then decode, return decoded values.
fn roundtrip(input: &[i32], w: u32, h: u32, orient: Orient) -> Vec<i32> {
    roundtrip_styled(input, w, h, orient, NO_STYLE)
}

/// Helper with cblksty support.
fn roundtrip_styled(
    input: &[i32],
    w: u32,
    h: u32,
    orient: Orient,
    cblksty: CblkStyle,
) -> Vec<i32> {
    let mut enc = T1::new(w, h);
    enc.set_data_from_i32(input);
    let numbps = enc.get_numbps();
    let (encoded, passes) = enc.encode_cblk(orient, cblksty);

    if numbps == 0 {
        return vec![0i32; (w * h) as usize];
    }

    let num_passes = passes.len() as u32;

    let pass_rates: Vec<usize> = passes.iter().map(|p| p.rate).collect();

    let mut dec = T1::new(w, h);
    dec.decode_cblk(&encoded, num_passes, orient, 0, numbps, cblksty, &pass_rates);
    dec.get_data_as_i32()
}

#[test]
fn encode_decode_zero_block() {
    let input = vec![0i32; 16];
    let output = roundtrip(&input, 4, 4, Orient::LL);
    assert_eq!(output, input);
}

#[test]
fn encode_decode_constant_block() {
    // All coefficients = 8 (power of 2)
    let input = vec![8i32; 16];
    let output = roundtrip(&input, 4, 4, Orient::LL);
    // With 1.5-bit rounding, decoded should be close
    for (i, (&inp, &out)) in input.iter().zip(output.iter()).enumerate() {
        assert!(
            (out - inp).abs() <= 1,
            "mismatch at index {}: input={}, output={}",
            i,
            inp,
            out
        );
    }
}

#[test]
fn encode_decode_gradient() {
    let input: Vec<i32> = (0..16).collect();
    let output = roundtrip(&input, 4, 4, Orient::LL);
    for (i, (&inp, &out)) in input.iter().zip(output.iter()).enumerate() {
        if inp == 0 {
            assert_eq!(out, 0, "zero should decode to zero at index {}", i);
        } else {
            assert_eq!(
                out.signum(),
                inp.signum(),
                "sign mismatch at index {}",
                i
            );
            // Allow rounding tolerance
            let tol = std::cmp::max(1, inp.abs() / 4);
            assert!(
                (out - inp).abs() <= tol,
                "value mismatch at index {}: input={}, output={}, tol={}",
                i,
                inp,
                out,
                tol
            );
        }
    }
}

#[test]
fn encode_decode_random_4x4() {
    // Pseudo-random values (deterministic)
    let input = vec![
        42, -17, 0, 100, -55, 23, 7, -1, 0, 0, 63, -63, 31, -31, 15, -15,
    ];
    let output = roundtrip(&input, 4, 4, Orient::LL);
    for (i, (&inp, &out)) in input.iter().zip(output.iter()).enumerate() {
        if inp == 0 {
            assert_eq!(out, 0, "zero should decode to zero at index {}", i);
        } else {
            assert_eq!(
                out.signum(),
                inp.signum(),
                "sign mismatch at index {}: input={}, output={}",
                i,
                inp,
                out
            );
        }
    }
}

#[test]
fn encode_decode_64x64() {
    // Larger block — max code-block size
    let n = 64 * 64;
    let input: Vec<i32> = (0..n as i32).map(|i| (i % 127) - 63).collect();
    let output = roundtrip(&input, 64, 64, Orient::LL);
    assert_eq!(output.len(), n);
    for (i, (&inp, &out)) in input.iter().zip(output.iter()).enumerate() {
        if inp == 0 {
            assert_eq!(out, 0, "zero mismatch at index {}", i);
        } else {
            assert_eq!(
                out.signum(),
                inp.signum(),
                "sign mismatch at index {}: input={}, output={}",
                i,
                inp,
                out
            );
        }
    }
}

#[test]
fn encode_decode_signed() {
    // Mix of positive and negative
    let input = vec![-128, -64, -32, -16, -8, -4, -2, -1, 1, 2, 4, 8, 16, 32, 64, 128];
    let output = roundtrip(&input, 4, 4, Orient::LL);
    for (i, (&inp, &out)) in input.iter().zip(output.iter()).enumerate() {
        assert_eq!(
            out.signum(),
            inp.signum(),
            "sign mismatch at index {}: input={}, output={}",
            i,
            inp,
            out
        );
        // Magnitude should be within 1 of original for these power-of-2 values
        let tol = std::cmp::max(1, inp.abs() / 4);
        assert!(
            (out.abs() - inp.abs()).abs() <= tol,
            "magnitude mismatch at index {}: input={}, output={}",
            i,
            inp,
            out,
        );
    }
}

#[test]
fn multiple_passes_count() {
    // Verify pass count = 3 * numbps - 2
    let mut t = T1::new(4, 4);
    let input = vec![15i32; 16]; // numbps = 4
    t.set_data_from_i32(&input);
    let numbps = t.get_numbps();
    assert_eq!(numbps, 4);

    let (_, passes) = t.encode_cblk(Orient::LL, NO_STYLE);
    let expected = 3 * numbps - 2;
    assert_eq!(
        passes.len() as u32,
        expected,
        "expected {} passes for {} bitplanes, got {}",
        expected,
        numbps,
        passes.len()
    );

    // Verify pass types cycle: 2, 0, 1, 2, 0, 1, ...
    for (i, pass) in passes.iter().enumerate() {
        // First pass is always cleanup (2), then sig(0), ref(1), cln(2), ...
        let expected_type = match i {
            0 => 2u8,
            _ => [0u8, 1, 2][(i - 1) % 3],
        };
        assert_eq!(
            pass.pass_type, expected_type,
            "pass {} should be type {}, got {}",
            i, expected_type, pass.pass_type
        );
    }
}

// ── Code-block style mode tests ──

#[test]
fn bypass_mode() {
    // BYPASS (LAZY): sig/ref passes use raw coding after 4th bitplane
    let input = vec![
        42, -17, 0, 100, -55, 23, 7, -1, 0, 0, 63, -63, 31, -31, 15, -15,
    ];
    let output = roundtrip_styled(&input, 4, 4, Orient::LL, CblkStyle::BYPASS);
    for (i, (&inp, &out)) in input.iter().zip(output.iter()).enumerate() {
        if inp == 0 {
            assert_eq!(out, 0, "zero mismatch at {i}");
        } else {
            assert_eq!(out.signum(), inp.signum(), "sign mismatch at {i}: in={inp}, out={out}");
        }
    }
}

#[test]
fn reset_mode() {
    // RESET: MQC contexts reset after every pass
    let input = vec![
        42, -17, 0, 100, -55, 23, 7, -1, 0, 0, 63, -63, 31, -31, 15, -15,
    ];
    let output = roundtrip_styled(&input, 4, 4, Orient::LL, CblkStyle::RESET);
    for (i, (&inp, &out)) in input.iter().zip(output.iter()).enumerate() {
        if inp == 0 {
            assert_eq!(out, 0, "zero mismatch at {i}");
        } else {
            assert_eq!(out.signum(), inp.signum(), "sign mismatch at {i}: in={inp}, out={out}");
        }
    }
}

#[test]
fn termall_mode() {
    // TERMALL: flush after every pass
    let input = vec![
        42, -17, 0, 100, -55, 23, 7, -1, 0, 0, 63, -63, 31, -31, 15, -15,
    ];
    let output = roundtrip_styled(&input, 4, 4, Orient::LL, CblkStyle::TERMALL);
    for (i, (&inp, &out)) in input.iter().zip(output.iter()).enumerate() {
        if inp == 0 {
            assert_eq!(out, 0, "zero mismatch at {i}");
        } else {
            assert_eq!(out.signum(), inp.signum(), "sign mismatch at {i}: in={inp}, out={out}");
        }
    }
}
