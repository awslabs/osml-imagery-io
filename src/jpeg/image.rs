//! JPEGImageAssetProvider — implements ImageAssetProvider for standalone JPEG images.
//!
//! Provides access to JPEG image data. The raw JPEG bytes are retained as
//! `Arc<[u8]>` and decoded on demand when `get_block()` is called.
//! JPEG images are always a 1×1 block grid with a single resolution level.
//!
//! On decode, pixel-interleaved RGB data from libjpeg-turbo is converted
//! to BSQ (band-sequential) format for consistency with the rest of the codebase.

use std::sync::Arc;

use crate::error::CodecError;
use crate::jpeg::ffi::TjDecompressor;
use crate::jpeg::metadata::JPEGMetadataProvider;
use crate::traits::asset::AssetProvider;
use crate::traits::image::ImageAssetProvider;
use crate::traits::metadata::MetadataProvider;
use crate::types::{AssetType, PixelType};

/// Image asset provider for a standalone JPEG image.
///
/// Retains the raw JPEG bytes and decodes on demand when `get_block()` is
/// called. JPEG images have a single resolution level and a 1×1 block grid.
pub struct JPEGImageAssetProvider {
    /// Unique key identifying this asset (e.g., "image:0")
    key: String,
    /// Image width in pixels
    width: u32,
    /// Image height in pixels
    height: u32,
    /// Number of bands (1 for grayscale, 3 for RGB)
    num_bands: u32,
    /// Raw JPEG file bytes, retained for on-demand decode
    jpeg_data: Arc<[u8]>,
    /// STAC-aligned roles (e.g., "data")
    roles: Vec<String>,
    /// Per-image metadata
    metadata: Arc<JPEGMetadataProvider>,
}

impl JPEGImageAssetProvider {
    /// Create a new `JPEGImageAssetProvider`.
    pub fn new(
        key: String,
        width: u32,
        height: u32,
        num_bands: u32,
        jpeg_data: Arc<[u8]>,
        roles: Vec<String>,
        metadata: Arc<JPEGMetadataProvider>,
    ) -> Self {
        Self {
            key,
            width,
            height,
            num_bands,
            jpeg_data,
            roles,
            metadata,
        }
    }

    /// Convert pixel-interleaved data (RGBRGB...) to BSQ format.
    ///
    /// For grayscale (1 band), this is a no-op identity copy.
    /// For RGB (3 bands), rearranges from [R0,G0,B0,R1,G1,B1,...] to
    /// [R0,R1,...,G0,G1,...,B0,B1,...].
    fn interleaved_to_bsq(interleaved: &[u8], width: usize, height: usize, num_bands: usize) -> Vec<u8> {
        if num_bands == 1 {
            return interleaved.to_vec();
        }

        let num_pixels = width * height;
        let mut bsq = vec![0u8; num_pixels * num_bands];

        for pixel in 0..num_pixels {
            for band in 0..num_bands {
                bsq[band * num_pixels + pixel] = interleaved[pixel * num_bands + band];
            }
        }

        bsq
    }
}

// =============================================================================
// AssetProvider Implementation
// =============================================================================

impl AssetProvider for JPEGImageAssetProvider {
    fn key(&self) -> &str {
        &self.key
    }

    fn title(&self) -> &str {
        &self.key
    }

    fn description(&self) -> &str {
        "JPEG image segment"
    }

    fn media_type(&self) -> &str {
        "image/jpeg"
    }

    fn roles(&self) -> &[String] {
        &self.roles
    }

    fn asset_type(&self) -> AssetType {
        AssetType::Image
    }

    fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
        Err(CodecError::Unsupported(
            "raw_asset() not supported for JPEG; use get_block()".to_string(),
        ))
    }

    fn metadata(&self) -> Arc<dyn MetadataProvider> {
        self.metadata.clone()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// =============================================================================
// ImageAssetProvider Implementation
// =============================================================================

impl ImageAssetProvider for JPEGImageAssetProvider {
    fn has_block(&self, block_row: u32, block_col: u32, resolution_level: u32) -> bool {
        resolution_level == 0 && block_row == 0 && block_col == 0
    }

    fn get_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
        // Validate resolution level (only level 0 supported)
        if resolution_level != 0 {
            return Err(CodecError::InvalidResolutionLevel(resolution_level));
        }

        // Validate block coordinates (only 0,0 valid for 1×1 grid)
        if block_row != 0 || block_col != 0 {
            return Err(CodecError::InvalidBlockCoordinates(
                block_row,
                block_col,
                resolution_level,
            ));
        }

        // Decompress the JPEG data via libjpeg-turbo
        let decompressor = TjDecompressor::new()?;
        let interleaved = decompressor.decompress(&self.jpeg_data, self.num_bands as usize)?;

        // Convert pixel-interleaved to BSQ
        let bsq = Self::interleaved_to_bsq(
            &interleaved,
            self.width as usize,
            self.height as usize,
            self.num_bands as usize,
        );

        // Handle band subsetting
        let requested_bands = match bands {
            Some(b) => b.to_vec(),
            None => (0..self.num_bands).collect(),
        };
        let num_out_bands = requested_bands.len() as u32;

        // If all bands requested in order, return the full buffer
        if requested_bands.len() == self.num_bands as usize
            && requested_bands
                .iter()
                .enumerate()
                .all(|(i, &b)| b == i as u32)
        {
            return Ok((bsq, [num_out_bands, self.height, self.width]));
        }

        // Band subsetting: extract only requested bands
        let band_size = (self.width * self.height) as usize;
        let mut output = Vec::with_capacity(requested_bands.len() * band_size);
        for &band in &requested_bands {
            let start = band as usize * band_size;
            let end = start + band_size;
            output.extend_from_slice(&bsq[start..end]);
        }

        Ok((output, [num_out_bands, self.height, self.width]))
    }

    fn num_resolution_levels(&self) -> u32 {
        1
    }

    fn num_bands(&self) -> u32 {
        self.num_bands
    }

    fn num_rows(&self) -> u32 {
        self.height
    }

    fn num_columns(&self) -> u32 {
        self.width
    }

    fn num_pixels_per_block_horizontal(&self) -> u32 {
        self.width
    }

    fn num_pixels_per_block_vertical(&self) -> u32 {
        self.height
    }

    fn num_bits_per_pixel(&self) -> u32 {
        8
    }

    fn actual_bits_per_pixel(&self) -> u32 {
        8
    }

    fn pixel_value_type(&self) -> PixelType {
        PixelType::UInt8
    }

    fn pad_pixel_value(&self) -> f64 {
        0.0
    }
}
