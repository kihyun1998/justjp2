# justjp2

Pure Rust JPEG 2000 (JP2/J2K) encoder and decoder.

## Features

- **Encode/Decode** JP2 file format and raw J2K codestreams
- **Lossless** (5/3 DWT + RCT) and **lossy** (9/7 DWT + ICT) compression
- **Multi-tile** support with parallel encoding via rayon
- **Reduce resolution** decoding (decode at lower resolution levels)
- **Region of interest** decoding (crop to a specific area)
- **Format auto-detection** (JP2 vs J2K)
- No unsafe code, no C dependencies

## Quick Start

```rust
use justjp2::{Image, Component, EncodeParams, CodecFormat};

// Encode
let image = Image {
    width: 256,
    height: 256,
    components: vec![Component {
        data: vec![128i32; 256 * 256],
        width: 256,
        height: 256,
        precision: 8,
        signed: false,
        dx: 1,
        dy: 1,
    }],
};

let params = EncodeParams::default(); // lossless JP2
let bytes = justjp2::encode(&image, &params).unwrap();

// Decode
let decoded = justjp2::decode(&bytes).unwrap();
assert_eq!(decoded.width, 256);
```

## Supported Formats

| Format | Extension | Encode | Decode |
|--------|-----------|--------|--------|
| JP2    | `.jp2`    | Yes    | Yes    |
| J2K    | `.j2k`    | Yes    | Yes    |

## API

```rust
// Decode (auto-detects JP2 vs J2K)
justjp2::decode(data: &[u8]) -> Result<Image>

// Decode at reduced resolution
justjp2::decode_with_reduce(data: &[u8], reduce: u32) -> Result<Image>

// Decode a specific region
justjp2::decode_region(data: &[u8], x0: u32, y0: u32, x1: u32, y1: u32) -> Result<Image>

// Encode
justjp2::encode(image: &Image, params: &EncodeParams) -> Result<Vec<u8>>
```

## Architecture

Built from the ground up following ITU-T T.800 (JPEG 2000 Part 1):

| Module | Description |
|--------|-------------|
| `bio` | Bit-level I/O with 0xFF bit-stuffing |
| `tgt` | Tag tree encoder/decoder |
| `mqc` | MQ arithmetic coder (47-state context model) |
| `t1` | Tier-1 EBCOT block coder (sig/ref/cleanup passes) |
| `dwt` | Discrete Wavelet Transform (5/3 reversible, 9/7 irreversible) |
| `mct` | Multi-Component Transform (RCT/ICT color transforms) |
| `quantize` | Scalar quantization with DWT/MCT norm tables |
| `t2` | Tier-2 packet assembly |
| `pi` | Packet iterator (LRCP, RLCP, RPCL, PCRL, CPRL) |
| `tcd` | Tile coder/decoder pipeline |
| `marker` | J2K marker segment parsing |
| `j2k` | J2K codestream codec |
| `jp2_box` | JP2 box format parser |
| `jp2` | JP2 file format codec |

## License

Apache-2.0
