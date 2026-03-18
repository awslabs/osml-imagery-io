//! ImageAssetProvider trait for blocked/tiled image access.
//!
//! This module defines the interface for accessing large imagery through
//! a blocked/tiled access pattern for memory efficiency.

use crate::error::CodecError;
use crate::traits::AssetProvider;
use crate::types::PixelType;

/// Trait for blocked/tiled image access.
///
/// This trait extends `AssetProvider` to provide efficient access to large imagery
/// through a blocked/tiled access pattern. Images are divided into rectangular blocks
/// that can be read independently, allowing processing of images larger than available memory.
///
/// # Image Pyramid
///
/// Images may have multiple resolution levels forming a pyramid, where level 0 is the
/// full resolution and higher levels are progressively downsampled.
///
/// # Thread Safety
///
/// Implementations must be thread-safe (`Send + Sync`) to allow concurrent access
/// from multiple threads.
pub trait ImageAssetProvider: AssetProvider {
    /// Check if a block exists at the given coordinates.
    ///
    /// # Arguments
    ///
    /// * `block_row` - Row index of the block in the block grid
    /// * `block_col` - Column index of the block in the block grid
    /// * `resolution_level` - Resolution level (0 = full resolution)
    ///
    /// # Returns
    ///
    /// `true` if the block exists, `false` otherwise.
    fn has_block(&self, block_row: u32, block_col: u32, resolution_level: u32) -> bool;

    /// Retrieve block data as a contiguous array.
    ///
    /// # Arguments
    ///
    /// * `block_row` - Row index of the block in the block grid
    /// * `block_col` - Column index of the block in the block grid
    /// * `resolution_level` - Resolution level (0 = full resolution)
    /// * `bands` - Optional slice of band indices to retrieve. If `None`, all bands are returned.
    ///
    /// # Returns
    ///
    /// A tuple of `(data, shape)` where:
    /// - `data` is the raw pixel data as bytes in band-sequential (BSQ) format
    /// - `shape` is `[bands, rows, cols]` describing the block dimensions (CHW format)
    ///
    /// # Errors
    ///
    /// Returns `CodecError::InvalidBlockCoordinates` if the block coordinates are out of bounds.
    /// Returns `CodecError::InvalidResolutionLevel` if the resolution level is invalid.
    fn get_block(
        &self,
        block_row: u32,
        block_col: u32,
        resolution_level: u32,
        bands: Option<&[u32]>,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError>;

    /// Returns the number of resolution levels in the image pyramid.
    ///
    /// The minimum value is 1 (full resolution only).
    fn num_resolution_levels(&self) -> u32;

    /// Returns the number of spectral bands.
    ///
    /// The minimum value is 1.
    fn num_bands(&self) -> u32;

    /// Returns the image height at full resolution in pixels.
    fn num_rows(&self) -> u32;

    /// Returns the image width at full resolution in pixels.
    fn num_columns(&self) -> u32;

    /// Returns the block width in pixels.
    fn num_pixels_per_block_horizontal(&self) -> u32;

    /// Returns the block height in pixels.
    fn num_pixels_per_block_vertical(&self) -> u32;

    /// Returns the nominal bits per pixel.
    fn num_bits_per_pixel(&self) -> u32;

    /// Returns the actual bits per pixel (may differ from nominal).
    fn actual_bits_per_pixel(&self) -> u32;

    /// Returns the pixel data type.
    fn pixel_value_type(&self) -> PixelType;

    /// Returns the value used for padding incomplete edge blocks.
    fn pad_pixel_value(&self) -> f64;

    /// Returns the image dimensions as `(bands, rows, columns)` (CHW format).
    ///
    /// This is a convenience method that combines `num_bands()`, `num_rows()`,
    /// and `num_columns()`.
    fn image_shape(&self) -> (u32, u32, u32) {
        (self.num_bands(), self.num_rows(), self.num_columns())
    }

    /// Returns the block dimensions as `(bands, rows, columns)` (CHW format).
    ///
    /// This is a convenience method that combines `num_bands()`,
    /// `num_pixels_per_block_vertical()`, and `num_pixels_per_block_horizontal()`.
    fn block_shape(&self) -> (u32, u32, u32) {
        (
            self.num_bands(),
            self.num_pixels_per_block_vertical(),
            self.num_pixels_per_block_horizontal(),
        )
    }

    /// Returns the number of blocks in each dimension as `(rows, cols)`.
    ///
    /// This computes the block grid size by dividing the image dimensions by
    /// the block dimensions, rounding up. If block dimensions are 0 (meaning
    /// the entire image is a single block, as in some JPEG 2000 NITF files),
    /// returns `(1, 1)`.
    fn block_grid_size(&self) -> (u32, u32) {
        let (_, h, w) = self.image_shape();
        let (_, bh, bw) = self.block_shape();
        // Block size of 0 means entire image is one block
        let rows = if bh == 0 { 1 } else { h.div_ceil(bh) };
        let cols = if bw == 0 { 1 } else { w.div_ceil(bw) };
        (rows, cols)
    }
}
