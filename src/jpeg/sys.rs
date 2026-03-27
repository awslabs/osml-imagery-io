//! Raw FFI declarations for libjpeg-turbo.
//!
//! This module contains the raw C FFI bindings to the libjpeg-turbo library.
//! These are low-level unsafe bindings - use the safe wrappers in `ffi.rs` instead.
//!
//! # API Overview
//!
//! - **TurboJPEG API**: High-level API for 8-bit JPEG compression/decompression.
//!   Simpler to use with memory-to-memory operations.
//! - **libjpeg API**: Lower-level API required for 12-bit JPEG support.
//!
//! # Safety
//!
//! All functions in this module are unsafe and require careful handling of
//! pointers and memory management according to libjpeg-turbo's API contract.

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_ulong, c_void};

// =============================================================================
// TurboJPEG API Constants
// =============================================================================

/// Pixel format: RGB (3 bytes per pixel)
pub const TJPF_RGB: c_int = 0;

/// Pixel format: BGR (3 bytes per pixel)
pub const TJPF_BGR: c_int = 1;

/// Pixel format: RGBX (4 bytes per pixel, X is padding)
pub const TJPF_RGBX: c_int = 2;

/// Pixel format: BGRX (4 bytes per pixel, X is padding)
pub const TJPF_BGRX: c_int = 3;

/// Pixel format: XBGR (4 bytes per pixel, X is padding)
pub const TJPF_XBGR: c_int = 4;

/// Pixel format: XRGB (4 bytes per pixel, X is padding)
pub const TJPF_XRGB: c_int = 5;

/// Pixel format: Grayscale (1 byte per pixel)
pub const TJPF_GRAY: c_int = 6;

/// Pixel format: RGBA (4 bytes per pixel)
pub const TJPF_RGBA: c_int = 7;

/// Pixel format: BGRA (4 bytes per pixel)
pub const TJPF_BGRA: c_int = 8;

/// Pixel format: ABGR (4 bytes per pixel)
pub const TJPF_ABGR: c_int = 9;

/// Pixel format: ARGB (4 bytes per pixel)
pub const TJPF_ARGB: c_int = 10;

/// Pixel format: CMYK (4 bytes per pixel)
pub const TJPF_CMYK: c_int = 11;

/// Chrominance subsampling: 4:4:4 (no subsampling)
pub const TJSAMP_444: c_int = 0;

/// Chrominance subsampling: 4:2:2
pub const TJSAMP_422: c_int = 1;

/// Chrominance subsampling: 4:2:0
pub const TJSAMP_420: c_int = 2;

/// Chrominance subsampling: Grayscale
pub const TJSAMP_GRAY: c_int = 3;

/// Chrominance subsampling: 4:4:0
pub const TJSAMP_440: c_int = 4;

/// Chrominance subsampling: 4:1:1
pub const TJSAMP_411: c_int = 5;

/// Flag: Use accurate DCT/IDCT algorithms (slower but more accurate)
pub const TJFLAG_ACCURATEDCT: c_int = 1 << 12;

/// Flag: Use bottom-up row order instead of top-down
pub const TJFLAG_BOTTOMUP: c_int = 1 << 1;

/// Flag: Use fast, inaccurate upsampling routines
pub const TJFLAG_FASTUPSAMPLE: c_int = 1 << 8;

/// Flag: Use fast, inaccurate DCT/IDCT algorithms
pub const TJFLAG_FASTDCT: c_int = 1 << 11;

// =============================================================================
// TurboJPEG API Types
// =============================================================================

/// Opaque handle for TurboJPEG compressor/decompressor
pub type tjhandle = *mut c_void;

// =============================================================================
// TurboJPEG API Functions
// =============================================================================

#[link(name = "turbojpeg")]
extern "C" {
    // -------------------------------------------------------------------------
    // Instance Management
    // -------------------------------------------------------------------------

    /// Create a TurboJPEG compressor instance.
    ///
    /// # Returns
    /// A handle to the compressor instance, or NULL on error.
    pub fn tjInitCompress() -> tjhandle;

    /// Create a TurboJPEG decompressor instance.
    ///
    /// # Returns
    /// A handle to the decompressor instance, or NULL on error.
    pub fn tjInitDecompress() -> tjhandle;

    /// Destroy a TurboJPEG compressor or decompressor instance.
    ///
    /// # Arguments
    /// * `handle` - The handle to destroy
    ///
    /// # Returns
    /// 0 on success, -1 on error.
    pub fn tjDestroy(handle: tjhandle) -> c_int;

    // -------------------------------------------------------------------------
    // Compression Functions
    // -------------------------------------------------------------------------

    /// Compress an RGB, grayscale, or CMYK image to a JPEG image in memory.
    ///
    /// # Arguments
    /// * `handle` - Compressor instance
    /// * `srcBuf` - Source image buffer
    /// * `width` - Image width in pixels
    /// * `pitch` - Bytes per row (0 = width * pixel_size)
    /// * `height` - Image height in pixels
    /// * `pixelFormat` - Pixel format (TJPF_*)
    /// * `jpegBuf` - Pointer to receive JPEG buffer (will be allocated if *jpegBuf is NULL)
    /// * `jpegSize` - Pointer to receive JPEG buffer size
    /// * `jpegSubsamp` - Chrominance subsampling (TJSAMP_*)
    /// * `jpegQual` - JPEG quality (1-100)
    /// * `flags` - Compression flags (TJFLAG_*)
    ///
    /// # Returns
    /// 0 on success, -1 on error.
    pub fn tjCompress2(
        handle: tjhandle,
        srcBuf: *const c_uchar,
        width: c_int,
        pitch: c_int,
        height: c_int,
        pixelFormat: c_int,
        jpegBuf: *mut *mut c_uchar,
        jpegSize: *mut c_ulong,
        jpegSubsamp: c_int,
        jpegQual: c_int,
        flags: c_int,
    ) -> c_int;

    /// Get the maximum size of the JPEG buffer for the given parameters.
    ///
    /// # Arguments
    /// * `width` - Image width
    /// * `height` - Image height
    /// * `jpegSubsamp` - Chrominance subsampling
    ///
    /// # Returns
    /// Maximum buffer size, or -1 on error.
    pub fn tjBufSize(width: c_int, height: c_int, jpegSubsamp: c_int) -> c_ulong;

    // -------------------------------------------------------------------------
    // Decompression Functions
    // -------------------------------------------------------------------------

    /// Retrieve information about a JPEG image without decompressing it.
    ///
    /// # Arguments
    /// * `handle` - Decompressor instance
    /// * `jpegBuf` - JPEG buffer
    /// * `jpegSize` - Size of JPEG buffer
    /// * `width` - Pointer to receive image width
    /// * `height` - Pointer to receive image height
    /// * `jpegSubsamp` - Pointer to receive subsampling type
    /// * `jpegColorspace` - Pointer to receive colorspace
    ///
    /// # Returns
    /// 0 on success, -1 on error.
    pub fn tjDecompressHeader3(
        handle: tjhandle,
        jpegBuf: *const c_uchar,
        jpegSize: c_ulong,
        width: *mut c_int,
        height: *mut c_int,
        jpegSubsamp: *mut c_int,
        jpegColorspace: *mut c_int,
    ) -> c_int;

    /// Decompress a JPEG image to an RGB, grayscale, or CMYK image.
    ///
    /// # Arguments
    /// * `handle` - Decompressor instance
    /// * `jpegBuf` - JPEG buffer
    /// * `jpegSize` - Size of JPEG buffer
    /// * `dstBuf` - Destination buffer (must be pre-allocated)
    /// * `width` - Desired output width (0 = use JPEG width)
    /// * `pitch` - Bytes per row in destination (0 = width * pixel_size)
    /// * `height` - Desired output height (0 = use JPEG height)
    /// * `pixelFormat` - Desired pixel format (TJPF_*)
    /// * `flags` - Decompression flags (TJFLAG_*)
    ///
    /// # Returns
    /// 0 on success, -1 on error.
    pub fn tjDecompress2(
        handle: tjhandle,
        jpegBuf: *const c_uchar,
        jpegSize: c_ulong,
        dstBuf: *mut c_uchar,
        width: c_int,
        pitch: c_int,
        height: c_int,
        pixelFormat: c_int,
        flags: c_int,
    ) -> c_int;

    // -------------------------------------------------------------------------
    // Memory Management
    // -------------------------------------------------------------------------

    /// Free a buffer allocated by TurboJPEG.
    ///
    /// # Arguments
    /// * `buffer` - Buffer to free
    pub fn tjFree(buffer: *mut c_uchar);

    /// Allocate a buffer for JPEG compression.
    ///
    /// # Arguments
    /// * `bytes` - Number of bytes to allocate
    ///
    /// # Returns
    /// Pointer to allocated buffer, or NULL on error.
    pub fn tjAlloc(bytes: c_int) -> *mut c_uchar;

    // -------------------------------------------------------------------------
    // Error Handling
    // -------------------------------------------------------------------------

    /// Get the last error message.
    ///
    /// # Arguments
    /// * `handle` - Compressor/decompressor instance (can be NULL for global errors)
    ///
    /// # Returns
    /// Pointer to error message string.
    pub fn tjGetErrorStr2(handle: tjhandle) -> *mut c_char;

    /// Get the last error code.
    ///
    /// # Arguments
    /// * `handle` - Compressor/decompressor instance
    ///
    /// # Returns
    /// Error code (0 = no error, negative = error).
    pub fn tjGetErrorCode(handle: tjhandle) -> c_int;
}

// =============================================================================
// libjpeg API Constants (for 12-bit support)
// =============================================================================

/// JPEG library version
pub const JPEG_LIB_VERSION: c_int = 62;

/// Maximum number of components
pub const MAX_COMPONENTS: usize = 10;

/// DCT method: Integer DCT (slow but accurate)
pub const JDCT_ISLOW: c_int = 0;

/// DCT method: Integer DCT (fast but less accurate)
pub const JDCT_IFAST: c_int = 1;

/// DCT method: Floating-point DCT
pub const JDCT_FLOAT: c_int = 2;

/// Color space: Unknown
pub const JCS_UNKNOWN: c_int = 0;

/// Color space: Grayscale
pub const JCS_GRAYSCALE: c_int = 1;

/// Color space: RGB
pub const JCS_RGB: c_int = 2;

/// Color space: YCbCr (also known as YUV)
#[allow(non_upper_case_globals)]
pub const JCS_YCbCr: c_int = 3;

/// Color space: CMYK
pub const JCS_CMYK: c_int = 4;

/// Color space: YCCK
pub const JCS_YCCK: c_int = 5;

// =============================================================================
// libjpeg API Types (for 12-bit support)
// =============================================================================

/// JPEG sample type (8-bit or 12-bit depending on library build)
pub type JSAMPLE = c_uchar;

/// 12-bit JPEG sample type
pub type JSAMPLE12 = u16;

/// Pointer to a row of samples
pub type JSAMPROW = *mut JSAMPLE;

/// Pointer to a row of 12-bit samples
pub type JSAMPROW12 = *mut JSAMPLE12;

/// Array of sample rows
pub type JSAMPARRAY = *mut JSAMPROW;

/// Array of 12-bit sample rows
pub type JSAMPARRAY12 = *mut JSAMPROW12;

/// Boolean type for libjpeg
pub type boolean = c_int;

/// JPEG quantization table
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JQUANT_TBL {
    /// Quantization values (64 entries for 8x8 DCT)
    pub quantval: [u16; 64],
    /// Sent to output file flag
    pub sent_table: boolean,
}

/// JPEG Huffman table
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct JHUFF_TBL {
    /// Number of codes of each length 1-16
    pub bits: [u8; 17],
    /// Symbols in order of increasing code length
    pub huffval: [u8; 256],
    /// Sent to output file flag
    pub sent_table: boolean,
}

/// Component info structure
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct jpeg_component_info {
    /// Component ID
    pub component_id: c_int,
    /// Component index in SOF
    pub component_index: c_int,
    /// Horizontal sampling factor
    pub h_samp_factor: c_int,
    /// Vertical sampling factor
    pub v_samp_factor: c_int,
    /// Quantization table selector
    pub quant_tbl_no: c_int,
    /// DC entropy table selector
    pub dc_tbl_no: c_int,
    /// AC entropy table selector
    pub ac_tbl_no: c_int,
    // Additional fields omitted for brevity - not needed for basic operations
    pub width_in_blocks: c_uint,
    pub height_in_blocks: c_uint,
    pub DCT_h_scaled_size: c_int,
    pub DCT_v_scaled_size: c_int,
    pub downsampled_width: c_uint,
    pub downsampled_height: c_uint,
    pub component_needed: boolean,
    pub MCU_width: c_int,
    pub MCU_height: c_int,
    pub MCU_blocks: c_int,
    pub MCU_sample_width: c_int,
    pub last_col_width: c_int,
    pub last_row_height: c_int,
    pub quant_table: *mut JQUANT_TBL,
    pub dct_table: *mut c_void,
}

/// Common fields for compress and decompress structs
#[repr(C)]
pub struct jpeg_common_struct {
    /// Error handler module
    pub err: *mut jpeg_error_mgr,
    /// Memory manager module
    pub mem: *mut c_void,
    /// Progress monitor
    pub progress: *mut c_void,
    /// Client data
    pub client_data: *mut c_void,
    /// Is decompressor flag
    pub is_decompressor: boolean,
    /// Global state
    pub global_state: c_int,
}

/// Error manager structure
#[repr(C)]
pub struct jpeg_error_mgr {
    /// Error exit function
    pub error_exit: Option<unsafe extern "C" fn(cinfo: *mut jpeg_common_struct)>,
    /// Emit message function
    pub emit_message: Option<unsafe extern "C" fn(cinfo: *mut jpeg_common_struct, msg_level: c_int)>,
    /// Output message function
    pub output_message: Option<unsafe extern "C" fn(cinfo: *mut jpeg_common_struct)>,
    /// Format message function
    pub format_message: Option<unsafe extern "C" fn(cinfo: *mut jpeg_common_struct, buffer: *mut c_char)>,
    /// Reset error manager function
    pub reset_error_mgr: Option<unsafe extern "C" fn(cinfo: *mut jpeg_common_struct)>,
    /// Last message code
    pub msg_code: c_int,
    /// Message parameters
    pub msg_parm: jpeg_message_parm,
    /// Trace level
    pub trace_level: c_int,
    /// Number of warnings
    pub num_warnings: c_long,
    /// Message table pointer
    pub jpeg_message_table: *const *const c_char,
    /// Last JPEG message
    pub last_jpeg_message: c_int,
    /// Addon message table
    pub addon_message_table: *const *const c_char,
    /// First addon message
    pub first_addon_message: c_int,
    /// Last addon message
    pub last_addon_message: c_int,
}

/// Message parameter union
#[repr(C)]
#[derive(Copy, Clone)]
pub union jpeg_message_parm {
    pub i: [c_int; 8],
    pub s: [c_char; 80],
}

/// Long type for libjpeg
pub type c_long = std::os::raw::c_long;

/// Destination manager for compression
#[repr(C)]
pub struct jpeg_destination_mgr {
    /// Next output byte
    pub next_output_byte: *mut c_uchar,
    /// Remaining space in buffer
    pub free_in_buffer: usize,
    /// Initialize destination
    pub init_destination: Option<unsafe extern "C" fn(cinfo: *mut jpeg_compress_struct)>,
    /// Empty output buffer
    pub empty_output_buffer: Option<unsafe extern "C" fn(cinfo: *mut jpeg_compress_struct) -> boolean>,
    /// Terminate destination
    pub term_destination: Option<unsafe extern "C" fn(cinfo: *mut jpeg_compress_struct)>,
}

/// Source manager for decompression
#[repr(C)]
pub struct jpeg_source_mgr {
    /// Next input byte
    pub next_input_byte: *const c_uchar,
    /// Bytes remaining in buffer
    pub bytes_in_buffer: usize,
    /// Initialize source
    pub init_source: Option<unsafe extern "C" fn(cinfo: *mut jpeg_decompress_struct)>,
    /// Fill input buffer
    pub fill_input_buffer: Option<unsafe extern "C" fn(cinfo: *mut jpeg_decompress_struct) -> boolean>,
    /// Skip input data
    pub skip_input_data: Option<unsafe extern "C" fn(cinfo: *mut jpeg_decompress_struct, num_bytes: c_long)>,
    /// Resync to restart
    pub resync_to_restart: Option<unsafe extern "C" fn(cinfo: *mut jpeg_decompress_struct, desired: c_int) -> boolean>,
    /// Terminate source
    pub term_source: Option<unsafe extern "C" fn(cinfo: *mut jpeg_decompress_struct)>,
}


// =============================================================================
// libjpeg Compression Structure
// =============================================================================

/// JPEG compression structure
#[repr(C)]
pub struct jpeg_compress_struct {
    // Common fields (must match jpeg_common_struct layout)
    /// Error handler module
    pub err: *mut jpeg_error_mgr,
    /// Memory manager module
    pub mem: *mut c_void,
    /// Progress monitor
    pub progress: *mut c_void,
    /// Client data
    pub client_data: *mut c_void,
    /// Is decompressor flag (always 0 for compress)
    pub is_decompressor: boolean,
    /// Global state
    pub global_state: c_int,

    // Compression-specific fields
    /// Destination manager
    pub dest: *mut jpeg_destination_mgr,

    /// Image width in pixels
    pub image_width: c_uint,
    /// Image height in pixels
    pub image_height: c_uint,
    /// Number of color components
    pub input_components: c_int,
    /// Color space of input image
    pub in_color_space: c_int,

    /// Input gamma
    pub input_gamma: f64,

    /// Data precision (8 or 12 bits)
    pub data_precision: c_int,

    /// Number of components in JPEG image
    pub num_components: c_int,
    /// Color space of JPEG image
    pub jpeg_color_space: c_int,

    /// Component info array
    pub comp_info: *mut jpeg_component_info,

    /// Quantization tables
    pub quant_tbl_ptrs: [*mut JQUANT_TBL; 4],
    /// DC Huffman tables
    pub dc_huff_tbl_ptrs: [*mut JHUFF_TBL; 4],
    /// AC Huffman tables
    pub ac_huff_tbl_ptrs: [*mut JHUFF_TBL; 4],

    /// Arith-coding DC context
    pub arith_dc_L: [u8; 16],
    /// Arith-coding DC context
    pub arith_dc_U: [u8; 16],
    /// Arith-coding AC context
    pub arith_ac_K: [u8; 16],

    /// Number of scans
    pub num_scans: c_int,
    /// Scan info array
    pub scan_info: *const c_void,

    /// Raw data input flag
    pub raw_data_in: boolean,
    /// Arithmetic coding flag
    pub arith_code: boolean,
    /// Optimize Huffman tables flag
    pub optimize_coding: boolean,
    /// CCIR601 sampling flag
    pub CCIR601_sampling: boolean,
    /// Smoothing factor
    pub smoothing_factor: c_int,
    /// DCT algorithm selector
    pub dct_method: c_int,

    /// MCU restart interval
    pub restart_interval: c_uint,
    /// Restart in rows
    pub restart_in_rows: c_int,

    /// Write JFIF APP0 marker
    pub write_JFIF_header: boolean,
    /// JFIF major version
    pub JFIF_major_version: u8,
    /// JFIF minor version
    pub JFIF_minor_version: u8,
    /// JFIF density unit
    pub density_unit: u8,
    /// JFIF X density
    pub X_density: u16,
    /// JFIF Y density
    pub Y_density: u16,
    /// Write Adobe APP14 marker
    pub write_Adobe_marker: boolean,

    // Private fields (opaque to application)
    pub next_scanline: c_uint,
    pub progressive_mode: boolean,
    pub max_h_samp_factor: c_int,
    pub max_v_samp_factor: c_int,
    pub total_iMCU_rows: c_uint,
    pub comps_in_scan: c_int,
    pub cur_comp_info: [*mut jpeg_component_info; 4],
    pub MCUs_per_row: c_uint,
    pub MCU_rows_in_scan: c_uint,
    pub blocks_in_MCU: c_int,
    pub MCU_membership: [c_int; 10],
    pub Ss: c_int,
    pub Se: c_int,
    pub Ah: c_int,
    pub Al: c_int,

    // Master record
    pub master: *mut c_void,
    pub main_ptr: *mut c_void,
    pub prep: *mut c_void,
    pub coef: *mut c_void,
    pub marker: *mut c_void,
    pub cconvert: *mut c_void,
    pub downsample: *mut c_void,
    pub fdct: *mut c_void,
    pub entropy: *mut c_void,
    pub script_space: *mut c_void,
    pub script_space_size: c_int,
}

// =============================================================================
// libjpeg Decompression Structure
// =============================================================================

/// JPEG decompression structure
#[repr(C)]
pub struct jpeg_decompress_struct {
    // Common fields (must match jpeg_common_struct layout)
    /// Error handler module
    pub err: *mut jpeg_error_mgr,
    /// Memory manager module
    pub mem: *mut c_void,
    /// Progress monitor
    pub progress: *mut c_void,
    /// Client data
    pub client_data: *mut c_void,
    /// Is decompressor flag (always 1 for decompress)
    pub is_decompressor: boolean,
    /// Global state
    pub global_state: c_int,

    // Decompression-specific fields
    /// Source manager
    pub src: *mut jpeg_source_mgr,

    /// Image width in pixels
    pub image_width: c_uint,
    /// Image height in pixels
    pub image_height: c_uint,
    /// Number of color components
    pub num_components: c_int,
    /// Color space of JPEG image
    pub jpeg_color_space: c_int,

    /// Output color space
    pub out_color_space: c_int,

    /// Scale numerator
    pub scale_num: c_uint,
    /// Scale denominator
    pub scale_denom: c_uint,

    /// Output gamma
    pub output_gamma: f64,

    /// Buffered image mode
    pub buffered_image: boolean,
    /// Raw data output
    pub raw_data_out: boolean,

    /// DCT algorithm selector
    pub dct_method: c_int,
    /// Do fancy upsampling
    pub do_fancy_upsampling: boolean,
    /// Do block smoothing
    pub do_block_smoothing: boolean,

    /// Quantize colors
    pub quantize_colors: boolean,
    /// Dither mode
    pub dither_mode: c_int,
    /// Two-pass quantize
    pub two_pass_quantize: boolean,
    /// Desired number of colors
    pub desired_number_of_colors: c_int,
    /// Enable 1-pass quantizer
    pub enable_1pass_quant: boolean,
    /// Enable external colormap
    pub enable_external_quant: boolean,
    /// Enable 2-pass quantizer
    pub enable_2pass_quant: boolean,

    // Output dimensions
    /// Output width
    pub output_width: c_uint,
    /// Output height
    pub output_height: c_uint,
    /// Output components
    pub out_color_components: c_int,
    /// Actual output components
    pub output_components: c_int,
    /// Recommended output height
    pub rec_outbuf_height: c_int,

    /// Actual number of colors
    pub actual_number_of_colors: c_int,
    /// Colormap
    pub colormap: JSAMPARRAY,

    /// Current scanline
    pub output_scanline: c_uint,
    /// Input scan number
    pub input_scan_number: c_int,
    /// Input iMCU row
    pub input_iMCU_row: c_uint,
    /// Output scan number
    pub output_scan_number: c_int,
    /// Output iMCU row
    pub output_iMCU_row: c_uint,

    /// Coefficient array
    pub coef_bits: *mut [c_int; 64],

    /// Quantization tables
    pub quant_tbl_ptrs: [*mut JQUANT_TBL; 4],
    /// DC Huffman tables
    pub dc_huff_tbl_ptrs: [*mut JHUFF_TBL; 4],
    /// AC Huffman tables
    pub ac_huff_tbl_ptrs: [*mut JHUFF_TBL; 4],

    /// Data precision
    pub data_precision: c_int,

    /// Component info
    pub comp_info: *mut jpeg_component_info,

    /// Progressive mode
    pub progressive_mode: boolean,
    /// Arithmetic coding
    pub arith_code: boolean,

    /// Arith DC L
    pub arith_dc_L: [u8; 16],
    /// Arith DC U
    pub arith_dc_U: [u8; 16],
    /// Arith AC K
    pub arith_ac_K: [u8; 16],

    /// Restart interval
    pub restart_interval: c_uint,

    /// Saw JFIF marker
    pub saw_JFIF_marker: boolean,
    /// JFIF major version
    pub JFIF_major_version: u8,
    /// JFIF minor version
    pub JFIF_minor_version: u8,
    /// Density unit
    pub density_unit: u8,
    /// X density
    pub X_density: u16,
    /// Y density
    pub Y_density: u16,
    /// Saw Adobe marker
    pub saw_Adobe_marker: boolean,
    /// Adobe transform
    pub Adobe_transform: u8,

    /// CCIR601 sampling
    pub CCIR601_sampling: boolean,

    /// Marker list
    pub marker_list: *mut c_void,

    // Private fields
    pub max_h_samp_factor: c_int,
    pub max_v_samp_factor: c_int,
    pub min_DCT_h_scaled_size: c_int,
    pub min_DCT_v_scaled_size: c_int,
    pub total_iMCU_rows: c_uint,
    pub sample_range_limit: *mut JSAMPLE,
    pub comps_in_scan: c_int,
    pub cur_comp_info: [*mut jpeg_component_info; 4],
    pub MCUs_per_row: c_uint,
    pub MCU_rows_in_scan: c_uint,
    pub blocks_in_MCU: c_int,
    pub MCU_membership: [c_int; 10],
    pub Ss: c_int,
    pub Se: c_int,
    pub Ah: c_int,
    pub Al: c_int,
    pub unread_marker: c_int,

    // Module pointers
    pub master: *mut c_void,
    pub main_ptr: *mut c_void,
    pub coef: *mut c_void,
    pub post: *mut c_void,
    pub inputctl: *mut c_void,
    pub marker_ptr: *mut c_void,
    pub entropy: *mut c_void,
    pub idct: *mut c_void,
    pub upsample: *mut c_void,
    pub cconvert: *mut c_void,
    pub cquantize: *mut c_void,
}

// =============================================================================
// libjpeg API Functions
// =============================================================================

#[link(name = "jpeg")]
extern "C" {
    // -------------------------------------------------------------------------
    // Error Handling
    // -------------------------------------------------------------------------

    /// Initialize standard error handler.
    ///
    /// # Arguments
    /// * `err` - Error manager to initialize
    ///
    /// # Returns
    /// Pointer to the initialized error manager.
    pub fn jpeg_std_error(err: *mut jpeg_error_mgr) -> *mut jpeg_error_mgr;

    // -------------------------------------------------------------------------
    // Compression Functions
    // -------------------------------------------------------------------------

    /// Create a compression object.
    ///
    /// # Arguments
    /// * `cinfo` - Compression info structure
    /// * `version` - JPEG library version
    /// * `structsize` - Size of jpeg_compress_struct
    pub fn jpeg_CreateCompress(
        cinfo: *mut jpeg_compress_struct,
        version: c_int,
        structsize: usize,
    );

    /// Destroy a compression object.
    ///
    /// # Arguments
    /// * `cinfo` - Compression info structure
    pub fn jpeg_destroy_compress(cinfo: *mut jpeg_compress_struct);

    /// Set default compression parameters.
    ///
    /// # Arguments
    /// * `cinfo` - Compression info structure
    pub fn jpeg_set_defaults(cinfo: *mut jpeg_compress_struct);

    /// Set compression quality.
    ///
    /// # Arguments
    /// * `cinfo` - Compression info structure
    /// * `quality` - Quality value (0-100)
    /// * `force_baseline` - Force baseline JPEG
    pub fn jpeg_set_quality(
        cinfo: *mut jpeg_compress_struct,
        quality: c_int,
        force_baseline: boolean,
    );

    /// Start compression.
    ///
    /// # Arguments
    /// * `cinfo` - Compression info structure
    /// * `write_all_tables` - Write all quantization and Huffman tables
    pub fn jpeg_start_compress(cinfo: *mut jpeg_compress_struct, write_all_tables: boolean);

    /// Write scanlines.
    ///
    /// # Arguments
    /// * `cinfo` - Compression info structure
    /// * `scanlines` - Array of scanline pointers
    /// * `num_lines` - Number of scanlines to write
    ///
    /// # Returns
    /// Number of scanlines written.
    pub fn jpeg_write_scanlines(
        cinfo: *mut jpeg_compress_struct,
        scanlines: JSAMPARRAY,
        num_lines: c_uint,
    ) -> c_uint;

    /// Finish compression.
    ///
    /// # Arguments
    /// * `cinfo` - Compression info structure
    pub fn jpeg_finish_compress(cinfo: *mut jpeg_compress_struct);

    /// Set up memory destination for compression.
    ///
    /// # Arguments
    /// * `cinfo` - Compression info structure
    /// * `outbuffer` - Pointer to output buffer pointer
    /// * `outsize` - Pointer to output size
    pub fn jpeg_mem_dest(
        cinfo: *mut jpeg_compress_struct,
        outbuffer: *mut *mut c_uchar,
        outsize: *mut c_ulong,
    );

    // -------------------------------------------------------------------------
    // Decompression Functions
    // -------------------------------------------------------------------------

    /// Create a decompression object.
    ///
    /// # Arguments
    /// * `cinfo` - Decompression info structure
    /// * `version` - JPEG library version
    /// * `structsize` - Size of jpeg_decompress_struct
    pub fn jpeg_CreateDecompress(
        cinfo: *mut jpeg_decompress_struct,
        version: c_int,
        structsize: usize,
    );

    /// Destroy a decompression object.
    ///
    /// # Arguments
    /// * `cinfo` - Decompression info structure
    pub fn jpeg_destroy_decompress(cinfo: *mut jpeg_decompress_struct);

    /// Set up memory source for decompression.
    ///
    /// # Arguments
    /// * `cinfo` - Decompression info structure
    /// * `inbuffer` - Input buffer
    /// * `insize` - Input size
    pub fn jpeg_mem_src(
        cinfo: *mut jpeg_decompress_struct,
        inbuffer: *const c_uchar,
        insize: c_ulong,
    );

    /// Read JPEG header.
    ///
    /// # Arguments
    /// * `cinfo` - Decompression info structure
    /// * `require_image` - Require image data
    ///
    /// # Returns
    /// JPEG_HEADER_OK, JPEG_HEADER_TABLES_ONLY, or JPEG_SUSPENDED.
    pub fn jpeg_read_header(
        cinfo: *mut jpeg_decompress_struct,
        require_image: boolean,
    ) -> c_int;

    /// Start decompression.
    ///
    /// # Arguments
    /// * `cinfo` - Decompression info structure
    ///
    /// # Returns
    /// TRUE on success.
    pub fn jpeg_start_decompress(cinfo: *mut jpeg_decompress_struct) -> boolean;

    /// Read scanlines.
    ///
    /// # Arguments
    /// * `cinfo` - Decompression info structure
    /// * `scanlines` - Array of scanline pointers
    /// * `max_lines` - Maximum number of scanlines to read
    ///
    /// # Returns
    /// Number of scanlines read.
    pub fn jpeg_read_scanlines(
        cinfo: *mut jpeg_decompress_struct,
        scanlines: JSAMPARRAY,
        max_lines: c_uint,
    ) -> c_uint;

    /// Finish decompression.
    ///
    /// # Arguments
    /// * `cinfo` - Decompression info structure
    ///
    /// # Returns
    /// TRUE on success.
    pub fn jpeg_finish_decompress(cinfo: *mut jpeg_decompress_struct) -> boolean;
}

// =============================================================================
// 12-bit libjpeg API Functions
// =============================================================================

// Note: 12-bit JPEG support requires a specially compiled version of libjpeg
// with 12-bit sample precision. The functions below are the same as the 8-bit
// versions but operate on 12-bit samples.
//
// libjpeg-turbo provides separate 12-bit libraries (libjpeg12.so) that can be
// linked for 12-bit support. The API is identical but uses 16-bit sample types.

#[cfg(feature = "libjpeg-turbo-12bit")]
#[link(name = "jpeg12")]
extern "C" {
    /// Write 12-bit scanlines.
    pub fn jpeg12_write_scanlines(
        cinfo: *mut jpeg_compress_struct,
        scanlines: JSAMPARRAY12,
        num_lines: c_uint,
    ) -> c_uint;

    /// Read 12-bit scanlines.
    pub fn jpeg12_read_scanlines(
        cinfo: *mut jpeg_decompress_struct,
        scanlines: JSAMPARRAY12,
        max_lines: c_uint,
    ) -> c_uint;
}
