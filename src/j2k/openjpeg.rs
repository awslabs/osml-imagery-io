//! OpenJPEG-based JPEG 2000 codec implementation.
//!
//! This module provides the default JPEG 2000 codec implementation using
//! the OpenJPEG library (libopenjp2) via FFI bindings.
//!
//! # Features
//!
//! - JPEG 2000 Part 1 encoding and decoding
//! - Bit depths from 1 to 38 bits per pixel
//! - Multi-component (multi-band) images
//! - Tile-based incremental encoding
//! - Multi-threaded encoding/decoding
//!
//! # Limitations
//!
//! - HTJ2K (Part 15) is not supported by OpenJPEG
//!
//! # Example
//!
//! ```ignore
//! use osml_imagery_io::jbp::j2k::{OpenJpegCodec, J2KCodec, J2KDecodeParams};
//!
//! let codec = OpenJpegCodec::new();
//! let params = J2KDecodeParams::default();
//! let result = codec.decode(&codestream, &params)?;
//! ```

use std::os::raw::c_int;
use std::sync::Arc;

use crate::error::CodecError;

use super::codec::{
    J2KCodec, J2KCodecCapabilities, J2KDecodeParams, J2KDecodeResult, J2KEncodeParams,
    J2KEncodeState,
};
use super::ffi::{OjpCodec, OjpImage, OjpStream};
use super::sys::{self, opj_cparameters_t, opj_dparameters_t, OPJ_TRUE};

// =============================================================================
// OpenJPEG Codec
// =============================================================================

/// OpenJPEG-based J2K codec implementation.
///
/// This is the default JPEG 2000 codec, using the OpenJPEG library (libopenjp2).
/// It supports JPEG 2000 Part 1 encoding and decoding with bit depths up to 38 bits.
///
/// # Thread Safety
///
/// `OpenJpegCodec` is thread-safe and can be shared across threads. Each
/// encoding/decoding operation creates its own internal state.
///
/// # Example
///
/// ```ignore
/// let codec = OpenJpegCodec::new();
/// let params = J2KDecodeParams::default();
/// let result = codec.decode(&codestream, &params)?;
/// ```
pub struct OpenJpegCodec {
    num_threads: usize,
}

impl OpenJpegCodec {
    /// Create a new OpenJPEG codec with default settings.
    ///
    /// Uses all available CPU cores for multi-threaded encoding/decoding.
    pub fn new() -> Self {
        Self {
            num_threads: std::thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(1),
        }
    }

    /// Create with specific thread count.
    ///
    /// # Arguments
    /// * `num_threads` - Number of threads to use (0 = single-threaded)
    pub fn with_threads(num_threads: usize) -> Self {
        Self { num_threads }
    }

    /// Scan a J2K codestream for the COD marker (0xFF52) and extract the number of
    /// decomposition levels from the SPcod parameter.
    ///
    /// The caller must ensure the codestream starts with a valid SOC marker (0xFF4F).
    fn parse_cod_decomposition_levels(codestream: &[u8]) -> Result<u8, CodecError> {
        let len = codestream.len();
        let mut pos = 2; // skip SOC marker

        while pos + 1 < len {
            // Each marker starts with 0xFF
            if codestream[pos] != 0xFF {
                return Err(CodecError::Decode(
                    "Invalid JPEG 2000 codestream: expected marker".into(),
                ));
            }

            let marker_type = codestream[pos + 1];

            // SOC (0x4F) and SOD (0x93) have no length field; skip them
            if marker_type == 0x4F {
                pos += 2;
                continue;
            }

            // SOD marks the start of tile data — stop scanning
            if marker_type == 0x93 {
                break;
            }

            // All other markers have a 2-byte length field after the marker
            if pos + 3 >= len {
                break;
            }
            let seg_len = ((codestream[pos + 2] as usize) << 8) | (codestream[pos + 3] as usize);

            // COD marker: 0xFF52
            if marker_type == 0x52 {
                // Need at least 8 bytes after marker: Lcod(2) + Scod(1) + SGcod(4) + SPcod(1)
                if pos + 9 >= len || seg_len < 8 {
                    return Err(CodecError::Decode(
                        "JPEG 2000 COD marker segment too short".into(),
                    ));
                }
                // SPcod byte 0 is at offset 9 from marker start:
                // marker(2) + Lcod(2) + Scod(1) + SGcod(4) = 9
                return Ok(codestream[pos + 9]);
            }

            // Advance past this marker segment: marker(2) + segment body(seg_len)
            pos += 2 + seg_len;
        }

        Err(CodecError::Decode(
            "JPEG 2000 codestream: COD marker not found".into(),
        ))
    }
}

impl Default for OpenJpegCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl J2KCodec for OpenJpegCodec {
    fn capabilities(&self) -> J2KCodecCapabilities {
        J2KCodecCapabilities {
            max_bit_depth: 38,
            htj2k_decode: false, // OpenJPEG doesn't support HTJ2K
            htj2k_encode: false,
            name: "OpenJPEG",
        }
    }

    fn decode(
        &self,
        codestream: &[u8],
        params: &J2KDecodeParams,
    ) -> Result<J2KDecodeResult, CodecError> {
        // Validate codestream magic bytes (SOC marker: 0xFF4F)
        if codestream.len() < 2 {
            return Err(CodecError::Decode(
                "Invalid JPEG 2000 codestream: too short".into(),
            ));
        }
        if codestream[0] != 0xFF || codestream[1] != 0x4F {
            return Err(CodecError::Decode(format!(
                "Invalid JPEG 2000 codestream: missing SOC marker at offset 0 (found 0x{:02X}{:02X}, expected 0xFF4F)",
                codestream[0], codestream[1]
            )));
        }

        // Create decoder
        let codec = OjpCodec::new_decompress()?;
        codec.set_threads(self.num_threads)?;

        // Set up decoder parameters
        let mut dparams: opj_dparameters_t = unsafe { std::mem::zeroed() };
        unsafe {
            sys::opj_set_default_decoder_parameters(&mut dparams);
        }
        dparams.cp_reduce = params.resolution_level;

        codec.setup_decoder(&mut dparams)?;

        // Create input stream
        let stream = OjpStream::from_memory_read(codestream)?;

        // Read header
        let image = codec.read_header(&stream)?;

        // Set resolution factor if needed
        if params.resolution_level > 0 {
            codec.set_decoded_resolution_factor(params.resolution_level)?;
        }

        // Decode
        codec.decode(&stream, &image)?;
        codec.end_decompress(&stream)?;

        // Extract decoded data
        let num_components = image.num_components();
        let comp0 = image
            .component(0)
            .ok_or_else(|| CodecError::Decode("No components in decoded image".into()))?;

        let width = comp0.width;
        let height = comp0.height;
        let precision = comp0.precision;
        let is_signed = comp0.is_signed;

        // Calculate bytes per sample
        let bytes_per_sample = (precision as usize).div_ceil(8);

        // Convert to band-sequential byte format
        let pixels_per_band = (width * height) as usize;
        let mut output =
            Vec::with_capacity(pixels_per_band * num_components as usize * bytes_per_sample);

        for comp_idx in 0..num_components {
            let comp_data = image.component_data(comp_idx).ok_or_else(|| {
                CodecError::Decode(format!("Missing data for component {}", comp_idx))
            })?;

            for &value in comp_data {
                match bytes_per_sample {
                    1 => {
                        output.push(value as u8);
                    }
                    2 => {
                        // OpenJPEG returns native integers; serialize as native-endian
                        // to match the internal Vec<u8> contract.
                        output.extend_from_slice(&(value as u16).to_ne_bytes());
                    }
                    3 => {
                        let bytes = (value as u32).to_ne_bytes();
                        output.extend_from_slice(&bytes[..3]);
                    }
                    4 => {
                        // OpenJPEG returns native integers; serialize as native-endian.
                        output.extend_from_slice(&(value as u32).to_ne_bytes());
                    }
                    5 => {
                        let bytes = (value as i64 as u64).to_ne_bytes();
                        output.extend_from_slice(&bytes[..5]);
                    }
                    _ => {
                        return Err(CodecError::Decode(format!(
                            "Unsupported precision: {} bits ({} bytes per sample)",
                            precision, bytes_per_sample
                        )));
                    }
                }
            }
        }

        // Calculate number of resolution levels
        // The factor field indicates how many levels were discarded during decode
        let num_resolution_levels = comp0.factor + 1 + params.resolution_level;

        Ok(J2KDecodeResult {
            data: output,
            width,
            height,
            num_components,
            bits_per_component: precision,
            is_signed,
            num_resolution_levels,
        })
    }

    fn start_encode(
        &self,
        params: &J2KEncodeParams,
    ) -> Result<Box<dyn J2KEncodeState>, CodecError> {
        if params.htj2k {
            return Err(CodecError::Unsupported(
                "OpenJPEG does not support HTJ2K encoding".into(),
            ));
        }

        let state = OpenJpegEncodeState::new(params, self.num_threads)?;
        Ok(Box::new(state))
    }

    fn get_resolution_levels(&self, codestream: &[u8]) -> Result<u32, CodecError> {
        // Validate codestream: must start with SOC marker (0xFF4F)
        if codestream.len() < 2 || codestream[0] != 0xFF || codestream[1] != 0x4F {
            return Err(CodecError::Decode(
                "Invalid JPEG 2000 codestream: missing SOC marker".into(),
            ));
        }

        // Parse the COD marker (0xFF52) from the codestream header to read the actual
        // number of decomposition levels. The COD marker segment layout is:
        //   [0..2]  marker: 0xFF52
        //   [2..4]  Lcod: segment length (big-endian u16, includes itself but not marker)
        //   [4]     Scod: coding style
        //   [5]     SGcod byte 0: progression order
        //   [6..8]  SGcod bytes 1-2: number of layers (big-endian u16)
        //   [8]     SGcod byte 3: multiple component transform
        //   [9]     SPcod byte 0: number of decomposition levels
        //
        // Resolution levels = decomposition_levels + 1.
        let decomp_levels = Self::parse_cod_decomposition_levels(codestream)?;
        Ok(decomp_levels as u32 + 1)
    }

    fn get_dimensions(&self, codestream: &[u8]) -> Result<(u32, u32, u32), CodecError> {
        // Validate codestream
        if codestream.len() < 2 || codestream[0] != 0xFF || codestream[1] != 0x4F {
            return Err(CodecError::Decode(
                "Invalid JPEG 2000 codestream: missing SOC marker".into(),
            ));
        }

        let codec = OjpCodec::new_decompress()?;

        let mut dparams: opj_dparameters_t = unsafe { std::mem::zeroed() };
        unsafe {
            sys::opj_set_default_decoder_parameters(&mut dparams);
        }
        codec.setup_decoder(&mut dparams)?;

        let stream = OjpStream::from_memory_read(codestream)?;
        let image = codec.read_header(&stream)?;

        Ok((image.width(), image.height(), image.num_components()))
    }

    fn get_tile_info(&self, codestream: &[u8]) -> Result<(u32, u32, u32, u32), CodecError> {
        // Validate codestream
        if codestream.len() < 2 || codestream[0] != 0xFF || codestream[1] != 0x4F {
            return Err(CodecError::Decode(
                "Invalid JPEG 2000 codestream: missing SOC marker".into(),
            ));
        }

        // Parse SIZ marker to get tile dimensions
        // SIZ marker structure (after SOC 0xFF4F):
        // - 0xFF51 (SIZ marker)
        // - Lsiz (2 bytes) - length of marker segment
        // - Rsiz (2 bytes) - capabilities
        // - Xsiz (4 bytes) - image width
        // - Ysiz (4 bytes) - image height
        // - XOsiz (4 bytes) - image X offset
        // - YOsiz (4 bytes) - image Y offset
        // - XTsiz (4 bytes) - tile width
        // - YTsiz (4 bytes) - tile height
        // - XTOsiz (4 bytes) - tile X offset
        // - YTOsiz (4 bytes) - tile Y offset

        // Find SIZ marker (0xFF51)
        let mut pos = 2; // Skip SOC
        while pos + 2 <= codestream.len() {
            if codestream[pos] == 0xFF && codestream[pos + 1] == 0x51 {
                // Found SIZ marker
                if pos + 41 > codestream.len() {
                    return Err(CodecError::Decode("SIZ marker truncated".into()));
                }

                // Parse SIZ marker fields (big-endian)
                let xsiz = u32::from_be_bytes([
                    codestream[pos + 6],
                    codestream[pos + 7],
                    codestream[pos + 8],
                    codestream[pos + 9],
                ]);
                let ysiz = u32::from_be_bytes([
                    codestream[pos + 10],
                    codestream[pos + 11],
                    codestream[pos + 12],
                    codestream[pos + 13],
                ]);
                // XOsiz/YOsiz at pos+14..pos+21 are not needed for tile count
                let xtsiz = u32::from_be_bytes([
                    codestream[pos + 22],
                    codestream[pos + 23],
                    codestream[pos + 24],
                    codestream[pos + 25],
                ]);
                let ytsiz = u32::from_be_bytes([
                    codestream[pos + 26],
                    codestream[pos + 27],
                    codestream[pos + 28],
                    codestream[pos + 29],
                ]);
                let xtosiz = u32::from_be_bytes([
                    codestream[pos + 30],
                    codestream[pos + 31],
                    codestream[pos + 32],
                    codestream[pos + 33],
                ]);
                let ytosiz = u32::from_be_bytes([
                    codestream[pos + 34],
                    codestream[pos + 35],
                    codestream[pos + 36],
                    codestream[pos + 37],
                ]);

                // Calculate number of tiles per ISO 15444-1 Annex B.5:
                // num_tiles_x = ceil((Xsiz - XTOsiz) / XTsiz)
                // num_tiles_y = ceil((Ysiz - YTOsiz) / YTsiz)
                let num_tiles_x = (xsiz - xtosiz).div_ceil(xtsiz);
                let num_tiles_y = (ysiz - ytosiz).div_ceil(ytsiz);

                return Ok((xtsiz, ytsiz, num_tiles_x, num_tiles_y));
            }

            // Skip to next marker
            if codestream[pos] == 0xFF {
                let second = codestream[pos + 1];
                // Delimiter markers (no length field): SOC, SOD, EOC, EPH, 0xFF30–0xFF3F
                if second == 0x4F
                    || second == 0x93
                    || second == 0xD9
                    || second == 0x92
                    || (0x30..=0x3F).contains(&second)
                {
                    pos += 2;
                } else {
                    if pos + 4 > codestream.len() {
                        break;
                    }
                    let marker_len =
                        u16::from_be_bytes([codestream[pos + 2], codestream[pos + 3]]) as usize;
                    pos += 2 + marker_len;
                }
            } else {
                pos += 1;
            }
        }

        Err(CodecError::Decode(
            "SIZ marker not found in codestream".into(),
        ))
    }

    fn decode_tile(
        &self,
        codestream: &[u8],
        tile_index: u32,
        params: &J2KDecodeParams,
    ) -> Result<J2KDecodeResult, CodecError> {
        // Validate codestream magic bytes
        if codestream.len() < 2 {
            return Err(CodecError::Decode(
                "Invalid JPEG 2000 codestream: too short".into(),
            ));
        }
        if codestream[0] != 0xFF || codestream[1] != 0x4F {
            return Err(CodecError::Decode(format!(
                "Invalid JPEG 2000 codestream: missing SOC marker at offset 0 (found 0x{:02X}{:02X}, expected 0xFF4F)",
                codestream[0], codestream[1]
            )));
        }

        // Get tile info to validate tile_index
        let (tile_width, tile_height, num_tiles_x, num_tiles_y) = self.get_tile_info(codestream)?;
        let total_tiles = num_tiles_x * num_tiles_y;

        if tile_index >= total_tiles {
            return Err(CodecError::InvalidBlockCoordinates(
                tile_index / num_tiles_x,
                tile_index % num_tiles_x,
                params.resolution_level,
            ));
        }

        // Create decoder
        let codec = OjpCodec::new_decompress()?;
        codec.set_threads(self.num_threads)?;

        // Set up decoder parameters
        let mut dparams: opj_dparameters_t = unsafe { std::mem::zeroed() };
        unsafe {
            sys::opj_set_default_decoder_parameters(&mut dparams);
        }
        dparams.cp_reduce = params.resolution_level;

        codec.setup_decoder(&mut dparams)?;

        // Create input stream
        let stream = OjpStream::from_memory_read(codestream)?;

        // Read header
        let image = codec.read_header(&stream)?;

        // Set resolution factor if needed
        if params.resolution_level > 0 {
            codec.set_decoded_resolution_factor(params.resolution_level)?;
        }

        // Decode specific tile
        codec.get_decoded_tile(&stream, &image, tile_index)?;

        // Extract decoded data
        let num_components = image.num_components();
        let comp0 = image
            .component(0)
            .ok_or_else(|| CodecError::Decode("No components in decoded image".into()))?;

        // Calculate tile dimensions at this resolution level
        let scale = 1u32 << params.resolution_level;

        // For edge tiles, the actual dimensions may be smaller
        let tile_row = tile_index / num_tiles_x;
        let tile_col = tile_index % num_tiles_x;

        // Get full image dimensions
        let (full_width, full_height, _) = self.get_dimensions(codestream)?;

        // Calculate actual tile dimensions (may be smaller for edge tiles)
        let tile_x0 = tile_col * tile_width;
        let tile_y0 = tile_row * tile_height;
        let actual_tile_width = (full_width - tile_x0).min(tile_width);
        let actual_tile_height = (full_height - tile_y0).min(tile_height);
        let scaled_actual_width = actual_tile_width.div_ceil(scale);
        let scaled_actual_height = actual_tile_height.div_ceil(scale);

        let width = comp0.width.min(scaled_actual_width);
        let height = comp0.height.min(scaled_actual_height);
        let precision = comp0.precision;
        let is_signed = comp0.is_signed;

        // Calculate bytes per sample
        let bytes_per_sample = (precision as usize).div_ceil(8);

        // Convert to band-sequential byte format
        let pixels_per_band = (width * height) as usize;
        let mut output =
            Vec::with_capacity(pixels_per_band * num_components as usize * bytes_per_sample);

        for comp_idx in 0..num_components {
            let comp_data = image.component_data(comp_idx).ok_or_else(|| {
                CodecError::Decode(format!("Missing data for component {}", comp_idx))
            })?;

            // Take only the pixels we need (in case component has more data)
            let pixels_to_take = pixels_per_band.min(comp_data.len());
            for &value in &comp_data[..pixels_to_take] {
                match bytes_per_sample {
                    1 => {
                        output.push(value as u8);
                    }
                    2 => {
                        // OpenJPEG returns native integers; serialize as native-endian
                        // to match the internal Vec<u8> contract.
                        output.extend_from_slice(&(value as u16).to_ne_bytes());
                    }
                    3 => {
                        let bytes = (value as u32).to_ne_bytes();
                        output.extend_from_slice(&bytes[..3]);
                    }
                    4 => {
                        // OpenJPEG returns native integers; serialize as native-endian.
                        output.extend_from_slice(&(value as u32).to_ne_bytes());
                    }
                    5 => {
                        let bytes = (value as i64 as u64).to_ne_bytes();
                        output.extend_from_slice(&bytes[..5]);
                    }
                    _ => {
                        return Err(CodecError::Decode(format!(
                            "Unsupported precision: {} bits ({} bytes per sample)",
                            precision, bytes_per_sample
                        )));
                    }
                }
            }
        }

        // Calculate number of resolution levels
        let num_resolution_levels = comp0.factor + 1 + params.resolution_level;

        Ok(J2KDecodeResult {
            data: output,
            width,
            height,
            num_components,
            bits_per_component: precision,
            is_signed,
            num_resolution_levels,
        })
    }
}

// Safety: OpenJpegCodec is thread-safe (stateless)
unsafe impl Send for OpenJpegCodec {}
unsafe impl Sync for OpenJpegCodec {}

// =============================================================================
// OpenJPEG Encode State
// =============================================================================

/// State for incremental JPEG 2000 encoding using OpenJPEG.
///
/// This struct maintains the internal state needed for tile-by-tile encoding.
/// It is created by `OpenJpegCodec::start_encode()` and implements `J2KEncodeState`.
pub struct OpenJpegEncodeState {
    codec: OjpCodec,
    image: OjpImage,
    stream: OjpStream,
    params: J2KEncodeParams,
    tiles_written: u32,
    total_tiles: u32,
    started: bool,
}

impl OpenJpegEncodeState {
    fn new(params: &J2KEncodeParams, num_threads: usize) -> Result<Self, CodecError> {
        // Create codec
        let codec = OjpCodec::new_compress()?;
        codec.set_threads(num_threads)?;

        // Create image for tile-based encoding
        let image = OjpImage::new_tile(
            params.width,
            params.height,
            params.num_components,
            params.bits_per_component,
            params.is_signed,
        )?;

        // Set up encoder parameters
        let mut cparams: opj_cparameters_t = unsafe { std::mem::zeroed() };
        unsafe {
            sys::opj_set_default_encoder_parameters(&mut cparams);
        }

        // Configure tile size
        cparams.tile_size_on = OPJ_TRUE;
        cparams.cp_tx0 = 0;
        cparams.cp_ty0 = 0;
        cparams.cp_tdx = params.tile_width as c_int;
        cparams.cp_tdy = params.tile_height as c_int;

        // Configure compression
        cparams.numresolution = params.num_decomposition_levels as c_int + 1;
        cparams.tcp_numlayers = params.num_quality_layers as c_int;

        // For small tiles, we need to reduce the code-block size and precinct size
        // OpenJPEG default code-block is 64x64, precinct is 2^15 x 2^15
        // Both must fit within tiles
        let min_tile_dim = params.tile_width.min(params.tile_height);

        // Code-block size must be a power of 2 and <= tile size
        // Use the largest power of 2 that fits, minimum 4
        let cblock_size = if min_tile_dim >= 64 {
            64
        } else if min_tile_dim >= 32 {
            32
        } else if min_tile_dim >= 16 {
            16
        } else if min_tile_dim >= 8 {
            8
        } else {
            4
        };
        cparams.cblockw_init = cblock_size;
        cparams.cblockh_init = cblock_size;

        // Set precinct sizes for each resolution level
        // Precinct size must be >= code-block size and fit within tile
        // For small tiles, use the tile size as precinct size
        let precinct_size = if min_tile_dim >= 256 {
            256
        } else if min_tile_dim >= 128 {
            128
        } else if min_tile_dim >= 64 {
            64
        } else if min_tile_dim >= 32 {
            32
        } else if min_tile_dim >= 16 {
            16
        } else if min_tile_dim >= 8 {
            8
        } else {
            4
        };

        // Set precinct sizes for all resolution levels
        cparams.res_spec = cparams.numresolution;
        for i in 0..cparams.numresolution as usize {
            cparams.prcw_init[i] = precinct_size;
            cparams.prch_init[i] = precinct_size;
        }

        if params.lossless {
            cparams.irreversible = 0; // Reversible (lossless)
            cparams.tcp_rates[0] = 0.0; // Lossless
        } else if let Some(ratio) = params.compression_ratio {
            cparams.irreversible = 1; // Irreversible (lossy)
            cparams.tcp_rates[0] = ratio as f32;
            cparams.cp_disto_alloc = 1;
        }

        codec.setup_encoder(&mut cparams, &image)?;

        // Enable TLM (Tile-part Length Marker) segments for optimized random access.
        // TLM encoding was added in OpenJPEG 2.5.0 but had bugs that caused
        // segfaults during encoding. These were fixed in 2.5.3 (#1538).
        // Only enable TLM on versions known to be safe.
        if crate::j2k::ffi::openjpeg_version_at_least(2, 5, 3) {
            codec.set_extra_options(&["TLM=YES"])?;
        }

        // Create output stream
        let stream = OjpStream::new_memory_write()?;

        // Calculate total tiles
        let tiles_x = params.width.div_ceil(params.tile_width);
        let tiles_y = params.height.div_ceil(params.tile_height);
        let total_tiles = tiles_x * tiles_y;

        Ok(Self {
            codec,
            image,
            stream,
            params: params.clone(),
            tiles_written: 0,
            total_tiles,
            started: false,
        })
    }
}

impl J2KEncodeState for OpenJpegEncodeState {
    fn encode_tile(&mut self, tile_index: u32, data: &[u8]) -> Result<(), CodecError> {
        if tile_index >= self.total_tiles {
            return Err(CodecError::Encode(format!(
                "Tile index {} out of range (total: {})",
                tile_index, self.total_tiles
            )));
        }

        // Start compression on first tile
        if !self.started {
            self.codec.start_compress(&self.image, &self.stream)?;
            self.started = true;
        }

        // Write tile data
        self.codec.write_tile(tile_index, data, &self.stream)?;
        self.tiles_written += 1;

        Ok(())
    }

    fn finalize(self: Box<Self>) -> Result<Vec<u8>, CodecError> {
        if self.tiles_written != self.total_tiles {
            return Err(CodecError::Encode(format!(
                "Incomplete encoding: {} of {} tiles written",
                self.tiles_written, self.total_tiles
            )));
        }

        // End compression
        self.codec.end_compress(&self.stream)?;

        // Extract the encoded data
        self.stream.finalize_write()
    }
}

// Safety: OpenJpegEncodeState can be sent between threads
unsafe impl Send for OpenJpegEncodeState {}

// =============================================================================
// Codec Selection
// =============================================================================

/// Environment variable for codec selection.
const J2K_CODEC_ENV: &str = "OSML_IO_J2K_CODEC";

/// Get the configured J2K codec based on environment variable.
///
/// Checks `OSML_IO_J2K_CODEC` environment variable:
/// - `"openjpeg"` or unset: Use OpenJPEG (default)
/// - `"nvjpeg2000"`: Use NVIDIA nvJPEG2000 (future, not yet implemented)
///
/// This is an internal function - users of `JBPDatasetReader`/`JBPDatasetWriter`
/// do not need to interact with codecs directly.
///
/// # Panics
///
/// Panics if `"nvjpeg2000"` is requested but not yet implemented.
pub fn get_j2k_codec() -> Arc<dyn J2KCodec> {
    match std::env::var(J2K_CODEC_ENV).as_deref() {
        Ok("nvjpeg2000") => {
            // Future: return Arc::new(NvJpeg2000Codec::new())
            panic!("nvjpeg2000 codec not yet implemented")
        }
        _ => Arc::new(OpenJpegCodec::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Capability Tests
    // =========================================================================

    #[test]
    fn test_codec_capabilities() {
        let codec = OpenJpegCodec::new();
        let caps = codec.capabilities();
        assert_eq!(caps.max_bit_depth, 38);
        assert!(!caps.htj2k_decode);
        assert!(!caps.htj2k_encode);
        assert_eq!(caps.name, "OpenJPEG");
    }

    #[test]
    fn test_codec_with_threads() {
        let codec = OpenJpegCodec::with_threads(4);
        let caps = codec.capabilities();
        assert_eq!(caps.name, "OpenJPEG");
    }

    // =========================================================================
    // Error Handling Tests
    // =========================================================================

    #[test]
    fn test_invalid_codestream_empty() {
        let codec = OpenJpegCodec::new();
        let params = J2KDecodeParams::default();

        let result = codec.decode(&[], &params);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, CodecError::Decode(_)));
    }

    #[test]
    fn test_invalid_codestream_bad_magic() {
        let codec = OpenJpegCodec::new();
        let params = J2KDecodeParams::default();

        let result = codec.decode(&[0x00, 0x00], &params);
        assert!(result.is_err());
        let err = result.unwrap_err();
        if let CodecError::Decode(msg) = err {
            assert!(msg.contains("SOC marker"));
        } else {
            panic!("Expected Decode error");
        }
    }

    #[test]
    fn test_invalid_codestream_truncated() {
        let codec = OpenJpegCodec::new();
        let params = J2KDecodeParams::default();

        // Valid SOC marker but truncated codestream
        let result = codec.decode(&[0xFF, 0x4F, 0xFF, 0x51], &params);
        assert!(result.is_err());
    }

    #[test]
    fn test_htj2k_not_supported() {
        let codec = OpenJpegCodec::new();
        let params = J2KEncodeParams {
            width: 64,
            height: 64,
            htj2k: true,
            ..Default::default()
        };

        let result = codec.start_encode(&params);
        assert!(matches!(result, Err(CodecError::Unsupported(_))));
    }

    #[test]
    fn test_get_dimensions_invalid() {
        let codec = OpenJpegCodec::new();

        // Empty codestream
        let result = codec.get_dimensions(&[]);
        assert!(result.is_err());

        // Invalid magic
        let result = codec.get_dimensions(&[0x00, 0x00]);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_resolution_levels_invalid() {
        let codec = OpenJpegCodec::new();

        // Empty codestream
        let result = codec.get_resolution_levels(&[]);
        assert!(result.is_err());

        // Invalid magic
        let result = codec.get_resolution_levels(&[0x00, 0x00]);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_resolution_levels_from_encoded_codestream() {
        // Encode a codestream with a known number of decomposition levels and verify
        // that get_resolution_levels reads the actual count from the COD marker.
        let codec = OpenJpegCodec::new();

        for decomp_levels in 1u8..=5 {
            let params = J2KEncodeParams {
                width: 64,
                height: 64,
                num_components: 1,
                bits_per_component: 8,
                tile_width: 64,
                tile_height: 64,
                lossless: true,
                num_decomposition_levels: decomp_levels,
                ..Default::default()
            };

            let mut state = codec.start_encode(&params).unwrap();
            let data = vec![128u8; 64 * 64];
            state.encode_tile(0, &data).unwrap();
            let codestream = Box::new(state).finalize().unwrap();

            let levels = codec.get_resolution_levels(&codestream).unwrap();
            assert_eq!(
                levels,
                decomp_levels as u32 + 1,
                "For {} decomposition levels, expected {} resolution levels but got {}",
                decomp_levels,
                decomp_levels as u32 + 1,
                levels
            );
        }
    }

    #[test]
    fn test_get_resolution_levels_no_cod_marker() {
        // SOC marker followed by SOD (no COD marker) — should fail
        let codestream = [0xFF, 0x4F, 0xFF, 0x93];
        let codec = OpenJpegCodec::new();
        let result = codec.get_resolution_levels(&codestream);
        assert!(result.is_err());
        if let Err(CodecError::Decode(msg)) = result {
            assert!(msg.contains("COD marker not found"));
        } else {
            panic!("Expected Decode error about missing COD marker");
        }
    }

    #[test]
    fn test_get_resolution_levels_truncated_cod() {
        // SOC + COD marker but truncated before SPcod
        let codestream = [
            0xFF, 0x4F, // SOC
            0xFF, 0x52, // COD marker
            0x00, 0x04, // Lcod = 4 (too short for SPcod)
            0x00, 0x00, // partial Scod + SGcod
        ];
        let codec = OpenJpegCodec::new();
        let result = codec.get_resolution_levels(&codestream);
        assert!(result.is_err());
    }

    // =========================================================================
    // Codec Selection Tests
    // =========================================================================

    #[test]
    fn test_get_j2k_codec_default() {
        let codec = get_j2k_codec();
        let caps = codec.capabilities();
        assert_eq!(caps.name, "OpenJPEG");
    }

    // =========================================================================
    // Encode State Tests
    // =========================================================================

    #[test]
    fn test_encode_tile_out_of_range() {
        let codec = OpenJpegCodec::new();
        let params = J2KEncodeParams {
            width: 64,
            height: 64,
            num_components: 1,
            bits_per_component: 8,
            tile_width: 64,
            tile_height: 64,
            lossless: true,
            ..Default::default()
        };

        let mut state = codec.start_encode(&params).unwrap();

        // Tile index 1 is out of range for a single-tile image
        let data = vec![0u8; 64 * 64];
        let result = state.encode_tile(1, &data);
        assert!(matches!(result, Err(CodecError::Encode(_))));
    }

    #[test]
    fn test_finalize_incomplete() {
        let codec = OpenJpegCodec::new();
        let params = J2KEncodeParams {
            width: 128,
            height: 128,
            num_components: 1,
            bits_per_component: 8,
            tile_width: 64,
            tile_height: 64,
            lossless: true,
            ..Default::default()
        };

        let state = codec.start_encode(&params).unwrap();

        // Finalize without encoding any tiles should fail
        let result = Box::new(state).finalize();
        assert!(matches!(result, Err(CodecError::Encode(_))));
        if let Err(CodecError::Encode(msg)) = result {
            assert!(msg.contains("Incomplete"));
        }
    }

    #[test]
    fn test_encode_with_tlm_markers() {
        // TLM encoding is only enabled on OpenJPEG >= 2.5.3 due to bugs in
        // earlier versions. Skip the TLM assertion on older versions but still
        // verify that encoding produces a valid codestream.
        let tlm_expected = crate::j2k::ffi::openjpeg_version_at_least(2, 5, 3);

        let codec = OpenJpegCodec::new();
        let params = J2KEncodeParams {
            width: 64,
            height: 64,
            num_components: 1,
            bits_per_component: 8,
            tile_width: 64,
            tile_height: 64,
            lossless: true,
            ..Default::default()
        };

        let mut state = codec.start_encode(&params).unwrap();

        // Encode a single tile with test data
        let data = vec![128u8; 64 * 64];
        state.encode_tile(0, &data).unwrap();

        // Finalize and get the codestream
        let codestream = Box::new(state).finalize().unwrap();

        // Verify the codestream starts with SOC marker (0xFF4F)
        assert!(codestream.len() >= 2);
        assert_eq!(codestream[0], 0xFF);
        assert_eq!(codestream[1], 0x4F);

        if tlm_expected {
            // Search for TLM marker (0xFF55) in the codestream
            // TLM markers should appear in the main header (before SOT marker 0xFF90)
            let mut found_tlm = false;
            let mut i = 2;
            while i + 1 < codestream.len() {
                if codestream[i] == 0xFF {
                    let marker = codestream[i + 1];
                    if marker == 0x55 {
                        // Found TLM marker
                        found_tlm = true;
                        break;
                    }
                    if marker == 0x90 {
                        // Reached SOT marker, stop searching
                        break;
                    }
                    // Skip marker segment (marker + length + data)
                    if i + 3 < codestream.len() && marker != 0x4F && marker != 0xD9 {
                        let len =
                            u16::from_be_bytes([codestream[i + 2], codestream[i + 3]]) as usize;
                        i += 2 + len;
                    } else {
                        i += 2;
                    }
                } else {
                    i += 1;
                }
            }

            assert!(
                found_tlm,
                "TLM marker (0xFF55) not found in codestream header"
            );
        }
    }

    #[test]
    fn test_roundtrip_with_tlm() {
        // Test that encoding with TLM produces a decodable codestream
        let codec = OpenJpegCodec::new();
        let width = 64u32;
        let height = 64u32;

        // Create test image data (gradient pattern)
        let mut data = Vec::with_capacity((width * height) as usize);
        for y in 0..height {
            for x in 0..width {
                data.push(((x + y) % 256) as u8);
            }
        }

        // Encode
        let params = J2KEncodeParams {
            width,
            height,
            num_components: 1,
            bits_per_component: 8,
            tile_width: width,
            tile_height: height,
            lossless: true,
            ..Default::default()
        };

        let mut state = codec.start_encode(&params).unwrap();
        state.encode_tile(0, &data).unwrap();
        let codestream = Box::new(state).finalize().unwrap();

        // Decode
        let decode_params = J2KDecodeParams::default();
        let result = codec.decode(&codestream, &decode_params).unwrap();

        // Verify dimensions
        assert_eq!(result.width, width);
        assert_eq!(result.height, height);
        assert_eq!(result.num_components, 1);
        assert_eq!(result.bits_per_component, 8);

        // Verify data matches (lossless)
        assert_eq!(result.data.len(), data.len());
        assert_eq!(result.data, data);
    }

    /// Build a minimal codestream with only SOC + SIZ (enough for get_tile_info).
    fn build_siz_codestream(
        xsiz: u32,
        ysiz: u32,
        xosiz: u32,
        yosiz: u32,
        xtsiz: u32,
        ytsiz: u32,
        xtosiz: u32,
        ytosiz: u32,
    ) -> Vec<u8> {
        let mut cs = Vec::new();
        cs.extend_from_slice(&[0xFF, 0x4F]); // SOC
        cs.extend_from_slice(&[0xFF, 0x51]); // SIZ marker
                                             // Lsiz = 41 (minimum: covers through Csiz + one component)
        cs.extend_from_slice(&41u16.to_be_bytes());
        cs.extend_from_slice(&0u16.to_be_bytes()); // Rsiz
        cs.extend_from_slice(&xsiz.to_be_bytes());
        cs.extend_from_slice(&ysiz.to_be_bytes());
        cs.extend_from_slice(&xosiz.to_be_bytes());
        cs.extend_from_slice(&yosiz.to_be_bytes());
        cs.extend_from_slice(&xtsiz.to_be_bytes());
        cs.extend_from_slice(&ytsiz.to_be_bytes());
        cs.extend_from_slice(&xtosiz.to_be_bytes());
        cs.extend_from_slice(&ytosiz.to_be_bytes());
        cs.extend_from_slice(&1u16.to_be_bytes()); // Csiz = 1 component
        cs.push(0x07); // Ssiz: unsigned 8-bit
        cs
    }

    #[test]
    fn test_get_tile_info_xtosiz_less_than_xosiz() {
        // Reproduces the case from p1_01.j2k: XTOsiz=1 < XOsiz=5
        let cs = build_siz_codestream(127, 227, 5, 128, 127, 126, 1, 101);
        let codec = OpenJpegCodec::new();
        let (tw, th, ntx, nty) = codec.get_tile_info(&cs).unwrap();
        assert_eq!(tw, 127);
        assert_eq!(th, 126);
        // ceil((127 - 1) / 127) = ceil(126/127) = 1
        assert_eq!(ntx, 1);
        // ceil((227 - 101) / 126) = ceil(126/126) = 1
        assert_eq!(nty, 1);
    }

    #[test]
    fn test_get_tile_info_multiple_tiles_with_offset() {
        // Reproduces the case from p1_05.j2k: XTOsiz=8 < XOsiz=17
        let cs = build_siz_codestream(529, 524, 17, 12, 37, 37, 8, 2);
        let codec = OpenJpegCodec::new();
        let (tw, th, ntx, nty) = codec.get_tile_info(&cs).unwrap();
        assert_eq!(tw, 37);
        assert_eq!(th, 37);
        // ceil((529 - 8) / 37) = ceil(521/37) = 15
        assert_eq!(ntx, 15);
        // ceil((524 - 2) / 37) = ceil(522/37) = 15 (14*37=518, remainder 4)
        assert_eq!(nty, 15);
    }

    #[test]
    fn test_get_tile_info_zero_offsets() {
        // Common case: both offsets are zero
        let cs = build_siz_codestream(256, 256, 0, 0, 128, 128, 0, 0);
        let codec = OpenJpegCodec::new();
        let (tw, th, ntx, nty) = codec.get_tile_info(&cs).unwrap();
        assert_eq!(tw, 128);
        assert_eq!(th, 128);
        assert_eq!(ntx, 2);
        assert_eq!(nty, 2);
    }
}
