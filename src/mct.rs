/// Multi-Component Transform for JPEG 2000
///
/// Implements both the Reversible Color Transform (RCT) for lossless coding
/// and the Irreversible Color Transform (ICT) for lossy coding.

/// Forward Reversible Color Transform (RCT).
///
/// Input:  c0 = R, c1 = G, c2 = B (integer samples)
/// Output: c0 = Y, c1 = Cb, c2 = Cr
///
/// ```text
/// Y  = floor((R + 2G + B) / 4)
/// Cb = B - G
/// Cr = R - G
/// ```
pub fn rct_forward(c0: &mut [i32], c1: &mut [i32], c2: &mut [i32]) {
    let n = c0.len().min(c1.len()).min(c2.len());
    for i in 0..n {
        let r = c0[i];
        let g = c1[i];
        let b = c2[i];
        c0[i] = (r + 2 * g + b) >> 2;
        c1[i] = b - g;
        c2[i] = r - g;
    }
}

/// Inverse Reversible Color Transform (RCT).
///
/// Input:  c0 = Y, c1 = Cb, c2 = Cr
/// Output: c0 = R, c1 = G, c2 = B
///
/// ```text
/// G = Y - floor((Cb + Cr) / 4)
/// R = Cr + G
/// B = Cb + G
/// ```
pub fn rct_inverse(c0: &mut [i32], c1: &mut [i32], c2: &mut [i32]) {
    let n = c0.len().min(c1.len()).min(c2.len());
    for i in 0..n {
        let y = c0[i];
        let cb = c1[i];
        let cr = c2[i];
        let g = y - ((cb + cr) >> 2);
        let r = cr + g;
        let b = cb + g;
        c0[i] = r;
        c1[i] = g;
        c2[i] = b;
    }
}

/// Forward Irreversible Color Transform (ICT).
///
/// Input:  c0 = R, c1 = G, c2 = B (float samples)
/// Output: c0 = Y, c1 = Cb, c2 = Cr
pub fn ict_forward(c0: &mut [f32], c1: &mut [f32], c2: &mut [f32]) {
    let n = c0.len().min(c1.len()).min(c2.len());
    for i in 0..n {
        let r = c0[i];
        let g = c1[i];
        let b = c2[i];
        c0[i] = 0.299 * r + 0.587 * g + 0.114 * b;
        c1[i] = -0.16875 * r - 0.33126 * g + 0.5 * b;
        c2[i] = 0.5 * r - 0.41869 * g - 0.08131 * b;
    }
}

/// Inverse Irreversible Color Transform (ICT).
///
/// Input:  c0 = Y, c1 = Cb, c2 = Cr
/// Output: c0 = R, c1 = G, c2 = B
pub fn ict_inverse(c0: &mut [f32], c1: &mut [f32], c2: &mut [f32]) {
    let n = c0.len().min(c1.len()).min(c2.len());
    for i in 0..n {
        let y = c0[i];
        let cb = c1[i];
        let cr = c2[i];
        c0[i] = y + 1.402 * cr;
        c1[i] = y - 0.34413 * cb - 0.71414 * cr;
        c2[i] = y + 1.772 * cb;
    }
}
