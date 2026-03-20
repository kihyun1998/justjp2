# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-03-20

### Changed

- Switch license to dual MIT OR Apache-2.0
- Set MSRV (minimum supported Rust version) to 1.80

### Added

- GitHub Actions CI (check, test, fmt, clippy across Linux/Windows/macOS)

## [0.1.0] - 2026-03-19

### Added

- **Core codec pipeline**: Full JPEG 2000 Part 1 (ITU-T T.800) encode/decode
  - MQ arithmetic coder (47-state context model)
  - Tier-1 EBCOT block coder (significance, refinement, cleanup passes)
  - Tier-2 packet assembly with all 5 progression orders (LRCP, RLCP, RPCL, PCRL, CPRL)
  - Discrete Wavelet Transform: 5/3 reversible (lossless) and 9/7 irreversible (lossy)
  - Multi-Component Transform: RCT (reversible) and ICT (irreversible)
  - Scalar quantization with DWT/MCT norm tables
- **File formats**: JP2 box format and raw J2K codestream
- **Public API**:
  - `encode()` / `decode()` with JP2/J2K auto-detection
  - `decode_with_reduce()` for reduced resolution decoding
  - `decode_region()` for region-of-interest cropping
- **Multi-tile** support with parallel tile encoding via rayon
- **Rate allocation** via `max_bytes` truncation
- **Code-block styles**: BYPASS, RESET, TERMALL mode support
- **HTJ2K foundation**: CAP marker parsing, HT flag detection (decoder stub)
- **SIMD-friendly batch operations**: RCT/ICT and DWT predict/update
- **Compatibility**: Verified header parsing of openjpeg-generated files

[0.1.1]: https://github.com/kihyun1998/justjp2/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/kihyun1998/justjp2/releases/tag/v0.1.0
