# Design Document: JPEG DCT Compression (IC=C3/M3/I1)

## Overview

This design document describes the implementation of JPEG DCT compression support for the osml-imagery-io library. The implementation follows the existing patterns established by the JPEG 2000 codec (src/jbp/j2k/) and integrates with the JBP image reader/writer infrastructure.

JPEG DCT compression is a lossy compression format that uses the Discrete Cosine Transform to achieve compression ratios typically between 10:1 and 20:1 while maintaining acceptable visual quality. The JBP specification defines three IC codes for JPEG DCT:

- **C3**: JPEG DCT compressed imagery
- **M3**: JPEG DCT compressed imagery with block mask (sparse images)
- **I1**: Downsampled JPEG (single block ≤2048×2048, typically for thumbnails)

## Architecture

The JPEG DCT implementation follows the same modular architecture as the existing J2K codec:

```
src/jbp/
├── jpeg/                    # New JPEG DCT module
│   ├── mod.rs              # Module exports
│   ├── codec.rs            # Codec trait and capabilities
│   ├── decoder.rs          # JpegBlockDecoder implementation
│   ├── encoder.rs          # JpegBlockEncoder implementation
│   └── comrat.rs           # COMRAT parsing for JPEG quality
├── image/
│   ├── decoder.rs          # Updated to dispatch to JPEG decoder
│   └── encoder.rs          # Updated to dispatch to JPEG encoder
```

The design leverages the existing `BlockDecoder` and `BlockEncoder` traits, allowing JPEG to be integrated as another codec option alongside uncompressed and J2K.

## Components and Interfaces

### JpegCodec

The main codec interface providing encoding and decoding capabilities:

```rust
pub struct JpegCodec {
    quality: u8,  // JPEG quality 1-100
}

impl JpegCodec {
    pub fn new() -> Self;
    pub fn with_quality(quality: u8) -> Self;
    pub fn capabilities(&self) -> JpegCodecCapabilities;
}

pub struct JpegCodecCapabilities {
    pub supports_8bit: bool,
    pub supports_12bit: bool,
    pub supports_rgb: bool,
    pub supports_ycbcr: bool,
}
```

### JpegBlockDecoder

Decodes JPEG compressed blocks from NITF image segments:

```rust
pub struct JpegBlockDecoder {
    codec: JpegCodec,
    pixel_type: PixelType,
    num_bands: usize,
    block_width: usize,
    block_height: usize,
    imode: IMode,
    color_space: ColorSpace,
}

impl JpegBlockDecoder {
    pub fn new(
        pixel_type: PixelType,
        num_bands: usize,
        block_width: usize,
        block_height: usize,
        imode: IMode,
        color_space: ColorSpace,
    ) -> Result<Self, CodecError>;
    
    pub fn decode_block(
        &self,
        jpeg_data: &[u8],
        bands: Option<&[usize]>,
    ) -> Result<Array3<u8>, CodecError>;
}
```

### JpegBlockEncoder

Encodes image blocks to JPEG format:

```rust
pub struct JpegBlockEncoder {
    codec: JpegCodec,
    pixel_type: PixelType,
    num_bands: usize,
    block_width: usize,
    block_height: usize,
    imode: IMode,
    color_space: ColorSpace,
    quality: u8,
}

impl JpegBlockEncoder {
    pub fn new(
        pixel_type: PixelType,
        num_bands: usize,
        block_width: usize,
        block_height: usize,
        imode: IMode,
        color_space: ColorSpace,
        quality: u8,
    ) -> Result<Self, CodecError>;
    
    pub fn encode_block(
        &self,
        block_data: ArrayView3<u8>,
    ) -> Result<Vec<u8>, CodecError>;
}
```

### JpegComrat

Parses and generates COMRAT values for JPEG compression:

```rust
pub enum JpegComrat {
    Quality(u8),      // Quality factor 0-100
    Default,          // Default quality (75)
}

impl JpegComrat {
    pub fn parse(comrat: &str) -> Result<Self, CodecError>;
    pub fn to_comrat_string(&self) -> String;
    pub fn quality(&self) -> u8;
}
```

### ColorSpace Enum

Represents the color space for multi-band images:

```rust
pub enum ColorSpace {
    Grayscale,
    RGB,
    YCbCr601,
}
```

## Data Models

### Supported Configurations

| Configuration | IC Code | Pixel Type | Bands | IMODE | Read | Write | Notes |
|--------------|---------|------------|-------|-------|------|-------|-------|
| Mono 8-bit | C3/M3/I1 | UInt8 | 1 | B/S | ✅ | ✅ | Standard grayscale |
| Mono 12-bit | C3/M3 | UInt16 | 1 | B/S | ❌ | ❌ | Not supported (see limitation) |
| RGB 24-bit | C3/M3/I1 | UInt8 | 3 | P | ✅ | ✅ | Pixel interleaved |
| YCbCr601 24-bit | C3/M3/I1 | UInt8 | 3 | P | ✅ | ✅ | Color space conversion |
| Multiband 8-bit | C3/M3 | UInt8 | 2-999 | B/S | ✅ | ✅ | Each band separate JPEG |
| Multiband 12-bit | C3/M3 | UInt16 | 2-999 | B/S | ❌ | ❌ | Not supported (see limitation) |

### COMRAT Format for JPEG

The COMRAT field for JPEG DCT uses a quality factor format:

- Format: `nn.n` where nn.n represents quality (00.0 to 99.9)
- Higher values = higher quality, larger files
- Default: 75.0 (maps to JPEG quality 75)

### Block Data Layout

For IMODE=P (pixel interleaved RGB/YCbCr):
- Single JPEG stream containing all 3 components
- JPEG handles interleaving internally

For IMODE=B/S (band sequential):
- Separate JPEG stream per band
- Streams concatenated in band order
- Each stream prefixed with 4-byte length

## Third-Party Library Selection

Based on the licensing requirements (MIT/Apache 2.0 only, no GPL/LGPL) and the need for both 8-bit and 12-bit support, we will use FFI bindings to **libjpeg-turbo**.

### libjpeg-turbo

- **License**: BSD-3-Clause, IJG (compatible with Apache 2.0)
- **Supports**: 8-bit and 12-bit JPEG encoding and decoding
- **Performance**: SIMD-optimized, significantly faster than pure Rust alternatives
- **Availability**: Widely available on Linux, macOS, Windows

### Integration Approach

Following the same pattern as the OpenJPEG integration for JPEG 2000:

1. **`src/jbp/jpeg/sys.rs`**: Raw FFI declarations for libjpeg-turbo (turbojpeg API)
2. **`src/jbp/jpeg/ffi.rs`**: Safe Rust wrappers around the unsafe FFI
3. **`src/jbp/jpeg/codec.rs`**: High-level codec interface

The turbojpeg API is preferred over the lower-level libjpeg API because:
- Simpler memory-to-memory compression/decompression
- Built-in buffer management
- Cleaner error handling
- Direct support for various pixel formats

### Build Configuration

Similar to OpenJPEG, libjpeg-turbo will be linked dynamically:

```toml
# Cargo.toml
[features]
default = ["openjpeg", "libjpeg-turbo"]
libjpeg-turbo = []
```

The build will use `pkg-config` to locate the library, with fallback to standard library paths.

### 12-bit Support

**12-bit JPEG is not supported** due to architectural constraints in libjpeg-turbo:

1. The TurboJPEG API only supports 8-bit samples
2. 12-bit JPEG requires a separately compiled libjpeg library with `BITS_IN_JSAMPLE=12`
3. This produces a different library (`libjpeg12`) with renamed symbols
4. The TurboJPEG API is disabled when building with 12-bit support
5. Supporting both 8-bit and 12-bit requires linking against two separate libraries

For 12-bit imagery, users should use JPEG 2000 (IC=C8) which fully supports 12-bit samples, or uncompressed format (IC=NC).

The decoder and encoder will return a clear `CodecError::Unsupported` error when 12-bit JPEG is requested, explaining the limitation and suggesting alternatives.



## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

Based on the prework analysis, the following consolidated properties will be validated through property-based testing:

### Property 1: JPEG DCT Lossy Roundtrip Quality

*For any* valid image with supported pixel type (UInt8 8-bit, UInt16 12-bit) and band configuration (mono, RGB, YCbCr, multiband), encoding with IC=C3 then decoding SHALL produce an image with:
- PSNR >= 30 dB
- SSIM >= 0.95
- Identical shape (bands, rows, cols)
- Identical pixel type (dtype)

**Validates: Requirements 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 7.1, 7.2, 7.3, 7.4**

### Property 2: Masked JPEG Roundtrip

*For any* masked image with IC=M3 and any mask pattern (checkerboard, border, random), writing then reading SHALL:
- Preserve the exact mask pattern (has_block() returns same values)
- Return valid block data for all provided blocks
- Return false from has_block() for all omitted blocks

**Validates: Requirements 3.1, 3.2, 3.3, 3.4, 3.5**

### Property 3: Downsampled JPEG (I1) Roundtrip

*For any* valid image with dimensions ≤2048×2048, encoding with IC=I1 then decoding SHALL produce an image with acceptable quality (PSNR >= 30 dB, SSIM >= 0.95) and preserved dimensions.

**Validates: Requirements 4.1, 4.2, 4.3**

### Property 4: COMRAT Metadata Preservation

*For any* JPEG DCT image written with a specific COMRAT value, reading the image SHALL expose the same COMRAT value in the metadata.

**Validates: Requirements 5.1, 5.2, 5.3**

## Error Handling

The JPEG DCT implementation will handle the following error conditions:

### Decoding Errors

| Error Condition | Error Type | Description |
|----------------|------------|-------------|
| Invalid JPEG data | `CodecError::InvalidData` | JPEG bitstream is malformed or truncated |
| Unsupported JPEG features | `CodecError::UnsupportedFeature` | JPEG uses features not supported by the codec |
| Dimension mismatch | `CodecError::DimensionMismatch` | Decoded dimensions don't match expected |
| Pixel type mismatch | `CodecError::PixelTypeMismatch` | Decoded bit depth doesn't match expected |

### Encoding Errors

| Error Condition | Error Type | Description |
|----------------|------------|-------------|
| Unsupported pixel type | `CodecError::UnsupportedPixelType` | Pixel type not supported for JPEG (e.g., Float32) |
| Invalid quality | `CodecError::InvalidParameter` | Quality value outside valid range |
| I1 dimension exceeded | `CodecError::DimensionExceeded` | Image exceeds 2048×2048 for IC=I1 |
| Encoding failure | `CodecError::EncodingFailed` | JPEG encoder returned an error |

### Error Propagation

Errors from the JPEG codec will be wrapped in the existing `CodecError` enum and propagated through the `JBPImageAssetProvider` to the Python bindings as `IoError` exceptions.

## Testing Strategy

### Dual Testing Approach

The implementation will use both unit tests and property-based tests:

- **Unit tests**: Verify specific examples, edge cases, and error conditions
- **Property tests**: Verify universal properties across many generated inputs

### Property-Based Testing Configuration

- **Library**: `hypothesis` (Python) for property tests
- **Minimum iterations**: 100 per property test
- **Tag format**: `Feature: jpeg-dct-compression, Property {number}: {property_text}`

### Test Organization

Property tests will be added to `tests/property/`:

```
tests/property/
├── strategies.py           # Updated with JPEG-specific strategies
├── test_jpeg_roundtrip.py  # New file for JPEG property tests
└── quality.py              # Existing quality metrics (PSNR, SSIM)
```

### Strategy Updates

The `strategies.py` file will be extended with:

1. **JPEG IC codes**: Add C3, M3, I1 to IC code strategies
2. **JPEG pixel types**: UInt8 (8-bit) and UInt16 (12-bit only, values 0-4095)
3. **JPEG configurations**: Mono, RGB, YCbCr, multiband combinations
4. **I1 dimension constraints**: Images ≤2048×2048 for IC=I1

### Unit Test Coverage

Unit tests will cover:

1. COMRAT parsing edge cases
2. Invalid JPEG data handling
3. Dimension constraint validation for IC=I1
4. Color space conversion accuracy
5. 12-bit value range validation

### Integration with Existing Tests

The existing `test_image_roundtrip.py` patterns will be followed, with JPEG-specific tests in a new `test_jpeg_roundtrip.py` file to keep the test organization clean.
