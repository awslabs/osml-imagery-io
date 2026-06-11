use crate::error::CodecError;
use crate::traits::ImageAssetProvider;

/// Assembles output tiles from a source provider with a potentially
/// different block grid.
pub struct TileAssembler<'a> {
    source: &'a dyn ImageAssetProvider,
    output_tile_width: u32,
    output_tile_height: u32,
    source_tile_width: u32,
    source_tile_height: u32,
    image_width: u32,
    image_height: u32,
    num_bands: u32,
    bytes_per_pixel: usize,
}

impl<'a> TileAssembler<'a> {
    /// Create a new assembler. If output tile dims == source block dims,
    /// get_output_tile short-circuits to a direct get_block delegation.
    pub fn new(
        source: &'a dyn ImageAssetProvider,
        output_tile_width: u32,
        output_tile_height: u32,
    ) -> Self {
        Self {
            source,
            output_tile_width,
            output_tile_height,
            source_tile_width: source.num_pixels_per_block_horizontal(),
            source_tile_height: source.num_pixels_per_block_vertical(),
            image_width: source.num_columns(),
            image_height: source.num_rows(),
            num_bands: source.num_bands(),
            bytes_per_pixel: source.num_bits_per_pixel().div_ceil(8) as usize,
        }
    }

    /// Returns (grid_rows, grid_cols) for the output tile grid.
    pub fn output_grid_size(&self) -> (u32, u32) {
        let cols = self.image_width.div_ceil(self.output_tile_width);
        let rows = self.image_height.div_ceil(self.output_tile_height);
        (rows, cols)
    }

    /// Returns true if source and output grids are identical (fast path).
    pub fn grids_match(&self) -> bool {
        self.output_tile_width == self.source_tile_width
            && self.output_tile_height == self.source_tile_height
    }

    /// Pure geometry check: does this output tile's pixel region overlap
    /// the source image extent? No I/O — just a coordinate bounds check.
    pub fn has_block(&self, output_row: u32, output_col: u32) -> bool {
        let start_x = output_col * self.output_tile_width;
        let start_y = output_row * self.output_tile_height;
        start_x < self.image_width && start_y < self.image_height
    }

    /// Assemble one output tile. Returns (data, [bands, rows, cols]).
    pub fn get_output_tile(
        &self,
        output_row: u32,
        output_col: u32,
    ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
        let (grid_rows, grid_cols) = self.output_grid_size();
        if output_row >= grid_rows || output_col >= grid_cols {
            return Err(CodecError::InvalidBlockCoordinates(
                output_row, output_col, 0,
            ));
        }

        // Fast path: grids match, just delegate
        if self.grids_match() {
            return self.source.get_block(output_row, output_col, 0, None);
        }

        // Compute pixel region for this output tile
        let start_x = output_col * self.output_tile_width;
        let start_y = output_row * self.output_tile_height;
        let end_x = (start_x + self.output_tile_width).min(self.image_width);
        let end_y = (start_y + self.output_tile_height).min(self.image_height);
        let tile_width = end_x - start_x;
        let tile_height = end_y - start_y;

        // Determine which source blocks overlap
        let src_start_col = start_x / self.source_tile_width;
        let src_end_col = (end_x.saturating_sub(1)) / self.source_tile_width + 1;
        let src_start_row = start_y / self.source_tile_height;
        let src_end_row = (end_y.saturating_sub(1)) / self.source_tile_height + 1;

        // Allocate output buffer (BSQ format)
        let tile_pixels = (tile_width as usize) * (tile_height as usize);
        let mut output = vec![0u8; tile_pixels * (self.num_bands as usize) * self.bytes_per_pixel];

        // Read source tiles and copy relevant pixels
        for src_row in src_start_row..src_end_row {
            for src_col in src_start_col..src_end_col {
                let (src_data, src_shape) = self.source.get_block(src_row, src_col, 0, None)?;

                self.copy_tile_region(
                    &src_data,
                    src_shape,
                    src_row,
                    src_col,
                    &mut output,
                    start_x,
                    start_y,
                    tile_width,
                    tile_height,
                );
            }
        }

        Ok((output, [self.num_bands, tile_height, tile_width]))
    }

    fn copy_tile_region(
        &self,
        src_data: &[u8],
        src_shape: [u32; 3],
        src_row: u32,
        src_col: u32,
        output: &mut [u8],
        out_start_x: u32,
        out_start_y: u32,
        out_width: u32,
        out_height: u32,
    ) {
        let src_rows = src_shape[1];
        let src_cols = src_shape[2];

        let src_start_x = src_col * self.source_tile_width;
        let src_start_y = src_row * self.source_tile_height;

        let overlap_start_x = out_start_x.max(src_start_x);
        let overlap_start_y = out_start_y.max(src_start_y);
        let overlap_end_x = (out_start_x + out_width).min(src_start_x + src_cols);
        let overlap_end_y = (out_start_y + out_height).min(src_start_y + src_rows);

        if overlap_start_x >= overlap_end_x || overlap_start_y >= overlap_end_y {
            return;
        }

        let overlap_width = overlap_end_x - overlap_start_x;

        let src_offset_x = overlap_start_x - src_start_x;
        let src_offset_y = overlap_start_y - src_start_y;
        let out_offset_x = overlap_start_x - out_start_x;
        let out_offset_y = overlap_start_y - out_start_y;

        let bpp = self.bytes_per_pixel;
        let src_pixels_per_band = (src_rows as usize) * (src_cols as usize);
        let out_pixels_per_band = (out_height as usize) * (out_width as usize);

        for band in 0..self.num_bands {
            let src_band_offset = (band as usize) * src_pixels_per_band * bpp;
            let out_band_offset = (band as usize) * out_pixels_per_band * bpp;

            for row in 0..(overlap_end_y - overlap_start_y) {
                let src_row_idx = (src_offset_y + row) as usize;
                let out_row_idx = (out_offset_y + row) as usize;

                let src_row_offset = src_band_offset
                    + src_row_idx * (src_cols as usize) * bpp
                    + (src_offset_x as usize) * bpp;
                let out_row_offset = out_band_offset
                    + out_row_idx * (out_width as usize) * bpp
                    + (out_offset_x as usize) * bpp;

                let copy_bytes = (overlap_width as usize) * bpp;

                output[out_row_offset..out_row_offset + copy_bytes]
                    .copy_from_slice(&src_data[src_row_offset..src_row_offset + copy_bytes]);
            }
        }
    }
}

/// Reassemble the full image from a multi-block provider into a single
/// contiguous BSQ buffer. Convenience wrapper over TileAssembler.
pub(crate) fn reassemble_full_image(
    source: &dyn ImageAssetProvider,
) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
    let w = source.num_columns();
    let h = source.num_rows();
    let assembler = TileAssembler::new(source, w, h);
    assembler.get_output_tile(0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CodecError;
    use crate::traits::asset::AssetMetadata;
    use crate::traits::metadata::MetadataProvider;
    use crate::types::PixelType;
    use std::collections::HashMap;
    use std::sync::Arc;

    struct EmptyMetadataProvider;
    impl MetadataProvider for EmptyMetadataProvider {
        fn raw(&self) -> &[u8] {
            &[]
        }
        fn entries(&self, _name: Option<&str>) -> HashMap<String, serde_json::Value> {
            HashMap::new()
        }
    }

    /// Mock provider for testing with configurable grid dimensions.
    struct MockProvider {
        image_width: u32,
        image_height: u32,
        block_width: u32,
        block_height: u32,
        num_bands: u32,
        bits_per_pixel: u32,
    }

    impl MockProvider {
        fn new(image_width: u32, image_height: u32, block_width: u32, block_height: u32) -> Self {
            Self {
                image_width,
                image_height,
                block_width,
                block_height,
                num_bands: 1,
                bits_per_pixel: 8,
            }
        }

        fn with_bands(mut self, bands: u32) -> Self {
            self.num_bands = bands;
            self
        }

        fn with_bits_per_pixel(mut self, bpp: u32) -> Self {
            self.bits_per_pixel = bpp;
            self
        }
    }

    impl AssetMetadata for MockProvider {
        fn key(&self) -> &str {
            "test"
        }
        fn title(&self) -> &str {
            "test"
        }
        fn description(&self) -> &str {
            ""
        }
        fn media_type(&self) -> &str {
            "application/octet-stream"
        }
        fn roles(&self) -> &[String] {
            &[]
        }
        fn metadata(&self) -> Arc<dyn MetadataProvider> {
            Arc::new(EmptyMetadataProvider)
        }
        fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
            Ok(vec![])
        }
    }

    impl ImageAssetProvider for MockProvider {
        fn has_block(
            &self,
            block_row: u32,
            block_col: u32,
            _resolution_level: u32,
        ) -> Result<bool, CodecError> {
            let (grid_rows, grid_cols) = self.block_grid_size();
            Ok(block_row < grid_rows && block_col < grid_cols)
        }

        fn get_block(
            &self,
            block_row: u32,
            block_col: u32,
            _resolution_level: u32,
            _bands: Option<&[u32]>,
        ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
            let (grid_rows, grid_cols) = self.block_grid_size();
            if block_row >= grid_rows || block_col >= grid_cols {
                return Err(CodecError::InvalidBlockCoordinates(block_row, block_col, 0));
            }

            let start_x = block_col * self.block_width;
            let start_y = block_row * self.block_height;
            let end_x = (start_x + self.block_width).min(self.image_width);
            let end_y = (start_y + self.block_height).min(self.image_height);
            let w = end_x - start_x;
            let h = end_y - start_y;

            let bpp = self.bits_per_pixel.div_ceil(8) as usize;
            let pixels = (w as usize) * (h as usize);
            let mut data = vec![0u8; pixels * (self.num_bands as usize) * bpp];

            // Fill with a deterministic pattern: pixel value = (y * image_width + x) % 256
            // for the first byte of each pixel
            for band in 0..self.num_bands as usize {
                for row in 0..h as usize {
                    for col in 0..w as usize {
                        let img_x = start_x as usize + col;
                        let img_y = start_y as usize + row;
                        let value =
                            ((img_y * self.image_width as usize + img_x + band * 37) % 256) as u8;
                        let offset = band * pixels * bpp + row * (w as usize) * bpp + col * bpp;
                        data[offset] = value;
                    }
                }
            }

            Ok((data, [self.num_bands, h, w]))
        }

        fn num_resolution_levels(&self) -> u32 {
            1
        }
        fn num_bands(&self) -> u32 {
            self.num_bands
        }
        fn num_rows(&self) -> u32 {
            self.image_height
        }
        fn num_columns(&self) -> u32 {
            self.image_width
        }
        fn num_pixels_per_block_horizontal(&self) -> u32 {
            self.block_width
        }
        fn num_pixels_per_block_vertical(&self) -> u32 {
            self.block_height
        }
        fn num_bits_per_pixel(&self) -> u32 {
            self.bits_per_pixel
        }
        fn actual_bits_per_pixel(&self) -> u32 {
            self.bits_per_pixel
        }
        fn pixel_value_type(&self) -> PixelType {
            match self.bits_per_pixel {
                8 => PixelType::UInt8,
                16 => PixelType::UInt16,
                32 => PixelType::UInt32,
                _ => PixelType::UInt8,
            }
        }
        fn pad_pixel_value(&self) -> f64 {
            0.0
        }
    }

    /// Helper: build a full-image reference by reading all pixels directly.
    fn build_reference_image(provider: &MockProvider) -> Vec<u8> {
        let w = provider.image_width as usize;
        let h = provider.image_height as usize;
        let bpp = provider.bits_per_pixel.div_ceil(8) as usize;
        let bands = provider.num_bands as usize;
        let mut reference = vec![0u8; w * h * bands * bpp];

        for band in 0..bands {
            for row in 0..h {
                for col in 0..w {
                    let value = ((row * w + col + band * 37) % 256) as u8;
                    let offset = band * w * h * bpp + row * w * bpp + col * bpp;
                    reference[offset] = value;
                }
            }
        }
        reference
    }

    // =========================================================================
    // Identity (grids match) tests
    // =========================================================================

    #[test]
    fn identity_grids_match_flag() {
        let provider = MockProvider::new(64, 64, 32, 32);
        let assembler = TileAssembler::new(&provider, 32, 32);
        assert!(assembler.grids_match());
    }

    #[test]
    fn identity_grid_size() {
        let provider = MockProvider::new(64, 64, 32, 32);
        let assembler = TileAssembler::new(&provider, 32, 32);
        assert_eq!(assembler.output_grid_size(), (2, 2));
    }

    #[test]
    fn identity_delegates_directly() {
        let provider = MockProvider::new(64, 64, 32, 32);
        let assembler = TileAssembler::new(&provider, 32, 32);

        let (data, shape) = assembler.get_output_tile(0, 0).unwrap();
        let (expected, expected_shape) = provider.get_block(0, 0, 0, None).unwrap();
        assert_eq!(shape, expected_shape);
        assert_eq!(data, expected);
    }

    #[test]
    fn identity_all_tiles_correct() {
        let provider = MockProvider::new(64, 48, 32, 32);
        let assembler = TileAssembler::new(&provider, 32, 32);
        let (rows, cols) = assembler.output_grid_size();

        for row in 0..rows {
            for col in 0..cols {
                let (data, shape) = assembler.get_output_tile(row, col).unwrap();
                let (expected, expected_shape) = provider.get_block(row, col, 0, None).unwrap();
                assert_eq!(shape, expected_shape);
                assert_eq!(data, expected);
            }
        }
    }

    // =========================================================================
    // Retiling tests
    // =========================================================================

    #[test]
    fn retile_larger_output_tiles() {
        // Source: 4x4 blocks on a 16x16 image → output: 8x8 tiles
        let provider = MockProvider::new(16, 16, 4, 4);
        let assembler = TileAssembler::new(&provider, 8, 8);

        assert!(!assembler.grids_match());
        assert_eq!(assembler.output_grid_size(), (2, 2));

        let reference = build_reference_image(&provider);

        // Check each output tile
        for out_row in 0..2u32 {
            for out_col in 0..2u32 {
                let (data, shape) = assembler.get_output_tile(out_row, out_col).unwrap();
                assert_eq!(shape, [1, 8, 8]);

                // Verify pixel values against reference
                for row in 0..8usize {
                    for col in 0..8usize {
                        let img_x = out_col as usize * 8 + col;
                        let img_y = out_row as usize * 8 + row;
                        let ref_offset = img_y * 16 + img_x;
                        let tile_offset = row * 8 + col;
                        assert_eq!(
                            data[tile_offset], reference[ref_offset],
                            "Mismatch at output tile ({}, {}), pixel ({}, {})",
                            out_row, out_col, row, col
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn retile_smaller_output_tiles() {
        // Source: 8x8 blocks on a 16x16 image → output: 4x4 tiles
        let provider = MockProvider::new(16, 16, 8, 8);
        let assembler = TileAssembler::new(&provider, 4, 4);

        assert!(!assembler.grids_match());
        assert_eq!(assembler.output_grid_size(), (4, 4));

        let reference = build_reference_image(&provider);

        for out_row in 0..4u32 {
            for out_col in 0..4u32 {
                let (data, shape) = assembler.get_output_tile(out_row, out_col).unwrap();
                assert_eq!(shape, [1, 4, 4]);

                for row in 0..4usize {
                    for col in 0..4usize {
                        let img_x = out_col as usize * 4 + col;
                        let img_y = out_row as usize * 4 + row;
                        let ref_offset = img_y * 16 + img_x;
                        let tile_offset = row * 4 + col;
                        assert_eq!(data[tile_offset], reference[ref_offset]);
                    }
                }
            }
        }
    }

    #[test]
    fn retile_scanline_source_to_tiled_output() {
        // Source: scanline (width x 1 blocks) → output: 4x4 tiles
        let provider = MockProvider::new(12, 8, 12, 1);
        let assembler = TileAssembler::new(&provider, 4, 4);

        assert!(!assembler.grids_match());
        assert_eq!(assembler.output_grid_size(), (2, 3));

        let reference = build_reference_image(&provider);

        for out_row in 0..2u32 {
            for out_col in 0..3u32 {
                let (data, shape) = assembler.get_output_tile(out_row, out_col).unwrap();
                assert_eq!(shape, [1, 4, 4]);

                for row in 0..4usize {
                    for col in 0..4usize {
                        let img_x = out_col as usize * 4 + col;
                        let img_y = out_row as usize * 4 + row;
                        let ref_offset = img_y * 12 + img_x;
                        let tile_offset = row * 4 + col;
                        assert_eq!(data[tile_offset], reference[ref_offset]);
                    }
                }
            }
        }
    }

    // =========================================================================
    // Edge tile tests (image not evenly divisible)
    // =========================================================================

    #[test]
    fn edge_tiles_partial_width() {
        // 10x8 image with 4x4 output tiles → last column has 2-pixel-wide tiles
        let provider = MockProvider::new(10, 8, 10, 8);
        let assembler = TileAssembler::new(&provider, 4, 4);

        assert_eq!(assembler.output_grid_size(), (2, 3));

        let (data, shape) = assembler.get_output_tile(0, 2).unwrap();
        assert_eq!(shape, [1, 4, 2]); // partial width

        let reference = build_reference_image(&provider);
        for row in 0..4usize {
            for col in 0..2usize {
                let img_x = 8 + col;
                let img_y = row;
                let ref_offset = img_y * 10 + img_x;
                let tile_offset = row * 2 + col;
                assert_eq!(data[tile_offset], reference[ref_offset]);
            }
        }
    }

    #[test]
    fn edge_tiles_partial_height() {
        // 8x10 image with 4x4 output tiles → last row has 2-pixel-tall tiles
        let provider = MockProvider::new(8, 10, 8, 10);
        let assembler = TileAssembler::new(&provider, 4, 4);

        assert_eq!(assembler.output_grid_size(), (3, 2));

        let (data, shape) = assembler.get_output_tile(2, 0).unwrap();
        assert_eq!(shape, [1, 2, 4]); // partial height

        let reference = build_reference_image(&provider);
        for row in 0..2usize {
            for col in 0..4usize {
                let img_x = col;
                let img_y = 8 + row;
                let ref_offset = img_y * 8 + img_x;
                let tile_offset = row * 4 + col;
                assert_eq!(data[tile_offset], reference[ref_offset]);
            }
        }
    }

    #[test]
    fn edge_tiles_partial_both() {
        // 10x10 image with 4x4 output tiles → bottom-right tile is 2x2
        let provider = MockProvider::new(10, 10, 10, 10);
        let assembler = TileAssembler::new(&provider, 4, 4);

        assert_eq!(assembler.output_grid_size(), (3, 3));

        let (data, shape) = assembler.get_output_tile(2, 2).unwrap();
        assert_eq!(shape, [1, 2, 2]);

        let reference = build_reference_image(&provider);
        for row in 0..2usize {
            for col in 0..2usize {
                let img_x = 8 + col;
                let img_y = 8 + row;
                let ref_offset = img_y * 10 + img_x;
                let tile_offset = row * 2 + col;
                assert_eq!(data[tile_offset], reference[ref_offset]);
            }
        }
    }

    // =========================================================================
    // Multi-band tests
    // =========================================================================

    #[test]
    fn multi_band_retiling() {
        let provider = MockProvider::new(8, 8, 4, 4).with_bands(3);
        let assembler = TileAssembler::new(&provider, 8, 8);

        assert_eq!(assembler.output_grid_size(), (1, 1));

        let (data, shape) = assembler.get_output_tile(0, 0).unwrap();
        assert_eq!(shape, [3, 8, 8]);

        let reference = build_reference_image(&provider);
        assert_eq!(data, reference);
    }

    #[test]
    fn multi_band_edge_tiles() {
        let provider = MockProvider::new(10, 10, 10, 10).with_bands(2);
        let assembler = TileAssembler::new(&provider, 4, 4);

        let (data, shape) = assembler.get_output_tile(0, 0).unwrap();
        assert_eq!(shape, [2, 4, 4]);

        let reference = build_reference_image(&provider);
        let img_w = 10usize;
        let img_h = 10usize;
        let bpp = 1usize;

        for band in 0..2usize {
            for row in 0..4usize {
                for col in 0..4usize {
                    let ref_offset = band * img_w * img_h * bpp + row * img_w * bpp + col * bpp;
                    let tile_offset = band * 4 * 4 * bpp + row * 4 * bpp + col * bpp;
                    assert_eq!(data[tile_offset], reference[ref_offset]);
                }
            }
        }
    }

    // =========================================================================
    // Multi-byte pixel type tests
    // =========================================================================

    #[test]
    fn uint16_retiling() {
        let provider = MockProvider::new(8, 8, 4, 4).with_bits_per_pixel(16);
        let assembler = TileAssembler::new(&provider, 8, 8);

        let (data, shape) = assembler.get_output_tile(0, 0).unwrap();
        assert_eq!(shape, [1, 8, 8]);
        assert_eq!(data.len(), 8 * 8 * 2); // 16 bits = 2 bytes per pixel
    }

    // =========================================================================
    // has_block tests
    // =========================================================================

    #[test]
    fn has_block_within_extent() {
        let provider = MockProvider::new(16, 16, 8, 8);
        let assembler = TileAssembler::new(&provider, 4, 4);

        assert!(assembler.has_block(0, 0));
        assert!(assembler.has_block(3, 3));
    }

    #[test]
    fn has_block_outside_extent() {
        let provider = MockProvider::new(16, 16, 8, 8);
        let assembler = TileAssembler::new(&provider, 4, 4);

        // Grid is 4x4 (16/4), so (4, 0) would start at y=16 which is == image_height
        assert!(!assembler.has_block(4, 0));
        assert!(!assembler.has_block(0, 4));
        assert!(!assembler.has_block(100, 100));
    }

    #[test]
    fn has_block_edge_boundary() {
        let provider = MockProvider::new(10, 10, 10, 10);
        let assembler = TileAssembler::new(&provider, 4, 4);

        // Grid is 3x3 (ceil(10/4)). Last tile starts at pixel 8, which < 10.
        assert!(assembler.has_block(2, 2));
        // Tile (3, 0) would start at y=12, >= 10.
        assert!(!assembler.has_block(3, 0));
    }

    // =========================================================================
    // Error propagation tests
    // =========================================================================

    #[test]
    fn out_of_bounds_returns_error() {
        let provider = MockProvider::new(16, 16, 8, 8);
        let assembler = TileAssembler::new(&provider, 4, 4);

        let result = assembler.get_output_tile(4, 0);
        assert!(result.is_err());

        let result = assembler.get_output_tile(0, 4);
        assert!(result.is_err());
    }

    /// Mock that always returns an error from get_block.
    struct ErrorProvider;

    impl AssetMetadata for ErrorProvider {
        fn key(&self) -> &str {
            "error"
        }
        fn title(&self) -> &str {
            "error"
        }
        fn description(&self) -> &str {
            ""
        }
        fn media_type(&self) -> &str {
            "application/octet-stream"
        }
        fn roles(&self) -> &[String] {
            &[]
        }
        fn metadata(&self) -> Arc<dyn MetadataProvider> {
            Arc::new(EmptyMetadataProvider)
        }
        fn raw_asset(&self) -> Result<Vec<u8>, CodecError> {
            Ok(vec![])
        }
    }

    impl ImageAssetProvider for ErrorProvider {
        fn has_block(&self, _: u32, _: u32, _: u32) -> Result<bool, CodecError> {
            Ok(true)
        }
        fn get_block(
            &self,
            _: u32,
            _: u32,
            _: u32,
            _: Option<&[u32]>,
        ) -> Result<(Vec<u8>, [u32; 3]), CodecError> {
            Err(CodecError::Decode("mock error".to_string()))
        }
        fn num_resolution_levels(&self) -> u32 {
            1
        }
        fn num_bands(&self) -> u32 {
            1
        }
        fn num_rows(&self) -> u32 {
            16
        }
        fn num_columns(&self) -> u32 {
            16
        }
        fn num_pixels_per_block_horizontal(&self) -> u32 {
            8
        }
        fn num_pixels_per_block_vertical(&self) -> u32 {
            8
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

    #[test]
    fn error_propagation_from_source() {
        let provider = ErrorProvider;
        // Use different tile sizes to avoid fast path
        let assembler = TileAssembler::new(&provider, 4, 4);

        let result = assembler.get_output_tile(0, 0);
        assert!(result.is_err());
        match result.unwrap_err() {
            CodecError::Decode(msg) => assert_eq!(msg, "mock error"),
            other => panic!("Expected Decode error, got: {:?}", other),
        }
    }

    // =========================================================================
    // reassemble_full_image tests
    // =========================================================================

    #[test]
    fn reassemble_full_image_single_block() {
        let provider = MockProvider::new(8, 8, 8, 8);
        let (data, shape) = reassemble_full_image(&provider).unwrap();
        assert_eq!(shape, [1, 8, 8]);

        let (expected, _) = provider.get_block(0, 0, 0, None).unwrap();
        assert_eq!(data, expected);
    }

    #[test]
    fn reassemble_full_image_multi_block() {
        let provider = MockProvider::new(16, 16, 4, 4);
        let (data, shape) = reassemble_full_image(&provider).unwrap();
        assert_eq!(shape, [1, 16, 16]);

        let reference = build_reference_image(&provider);
        assert_eq!(data, reference);
    }

    #[test]
    fn reassemble_full_image_multi_band() {
        let provider = MockProvider::new(8, 8, 4, 4).with_bands(3);
        let (data, shape) = reassemble_full_image(&provider).unwrap();
        assert_eq!(shape, [3, 8, 8]);

        let reference = build_reference_image(&provider);
        assert_eq!(data, reference);
    }

    #[test]
    fn reassemble_full_image_non_divisible() {
        let provider = MockProvider::new(10, 10, 4, 4);
        let (data, shape) = reassemble_full_image(&provider).unwrap();
        assert_eq!(shape, [1, 10, 10]);

        let reference = build_reference_image(&provider);
        assert_eq!(data, reference);
    }

    #[test]
    fn reassemble_full_image_error_propagation() {
        let provider = ErrorProvider;
        let result = reassemble_full_image(&provider);
        assert!(result.is_err());
    }

    // =========================================================================
    // Single-block source → multi-tile output
    // =========================================================================

    #[test]
    fn single_block_source_to_multi_tile() {
        let provider = MockProvider::new(8, 8, 8, 8);
        let assembler = TileAssembler::new(&provider, 4, 4);

        assert_eq!(assembler.output_grid_size(), (2, 2));

        let reference = build_reference_image(&provider);

        for out_row in 0..2u32 {
            for out_col in 0..2u32 {
                let (data, shape) = assembler.get_output_tile(out_row, out_col).unwrap();
                assert_eq!(shape, [1, 4, 4]);

                for row in 0..4usize {
                    for col in 0..4usize {
                        let img_x = out_col as usize * 4 + col;
                        let img_y = out_row as usize * 4 + row;
                        let ref_offset = img_y * 8 + img_x;
                        let tile_offset = row * 4 + col;
                        assert_eq!(data[tile_offset], reference[ref_offset]);
                    }
                }
            }
        }
    }
}
