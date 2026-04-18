//! Raw FFI declarations for OpenJPEG (libopenjp2).
//!
//! This module contains the raw C FFI bindings to the OpenJPEG library.
//! These are low-level unsafe bindings - use the safe wrappers in `ffi.rs` instead.
//!
//! # Safety
//!
//! All functions in this module are unsafe and require careful handling of
//! pointers and memory management according to OpenJPEG's API contract.

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

use std::os::raw::{c_char, c_int, c_void};

// =============================================================================
// Constants
// =============================================================================

/// Codec type for raw JPEG 2000 codestream (no JP2 container)
pub const OPJ_CODEC_J2K: c_int = 0;

/// Codec type for JP2 file format (with container)
pub const OPJ_CODEC_JP2: c_int = 2;

/// Boolean true value for OpenJPEG
pub const OPJ_TRUE: c_int = 1;

/// Boolean false value for OpenJPEG
pub const OPJ_FALSE: c_int = 0;

// Color spaces
/// Unknown color space
pub const OPJ_CLRSPC_UNKNOWN: c_int = -1;
/// Unspecified color space
pub const OPJ_CLRSPC_UNSPECIFIED: c_int = 0;
/// sRGB color space
pub const OPJ_CLRSPC_SRGB: c_int = 1;
/// Grayscale color space
pub const OPJ_CLRSPC_GRAY: c_int = 2;
/// sYCC color space
pub const OPJ_CLRSPC_SYCC: c_int = 3;
/// e-YCC color space
pub const OPJ_CLRSPC_EYCC: c_int = 4;
/// CMYK color space
pub const OPJ_CLRSPC_CMYK: c_int = 5;

// Progression orders
/// Layer-Resolution-Component-Position progression
pub const OPJ_LRCP: c_int = 0;
/// Resolution-Layer-Component-Position progression
pub const OPJ_RLCP: c_int = 1;
/// Resolution-Position-Component-Layer progression
pub const OPJ_RPCL: c_int = 2;
/// Position-Component-Resolution-Layer progression
pub const OPJ_PCRL: c_int = 3;
/// Component-Position-Resolution-Layer progression
pub const OPJ_CPRL: c_int = 4;

// Stream constants
/// Default stream buffer size (1MB)
pub const OPJ_STREAM_DEFAULT_BUFFER_SIZE: usize = 1024 * 1024;

// =============================================================================
// Opaque Types
// =============================================================================

/// Opaque codec handle
#[repr(C)]
pub struct opj_codec_t {
    _private: [u8; 0],
}

/// Opaque stream handle
#[repr(C)]
pub struct opj_stream_t {
    _private: [u8; 0],
}

/// Opaque codestream info handle
#[repr(C)]
pub struct opj_cstr_info_t {
    _private: [u8; 0],
}

/// Opaque codestream index handle
#[repr(C)]
pub struct opj_cstr_index_t {
    _private: [u8; 0],
}

// =============================================================================
// Struct Representations
// =============================================================================

/// Image component data structure
#[repr(C)]
#[derive(Debug)]
pub struct opj_image_comp_t {
    /// X component offset compared to the whole image
    pub dx: u32,
    /// Y component offset compared to the whole image
    pub dy: u32,
    /// Component width
    pub w: u32,
    /// Component height
    pub h: u32,
    /// X offset from the origin of the reference grid
    pub x0: u32,
    /// Y offset from the origin of the reference grid
    pub y0: u32,
    /// Precision (number of bits per component value)
    pub prec: u32,
    /// Obsolete: use prec instead
    pub bpp: u32,
    /// Signed (1) / unsigned (0)
    pub sgnd: u32,
    /// Number of decoded resolution levels
    pub resno_decoded: u32,
    /// Factor for reducing image resolution (2^factor)
    pub factor: u32,
    /// Image component data (allocated by opj_image_data_alloc)
    pub data: *mut i32,
    /// Alpha channel (0 = not alpha, 1 = alpha, 2 = premultiplied alpha)
    pub alpha: u16,
}

/// Image structure
#[repr(C)]
#[derive(Debug)]
pub struct opj_image_t {
    /// X offset from the origin of the reference grid
    pub x0: u32,
    /// Y offset from the origin of the reference grid
    pub y0: u32,
    /// Image width
    pub x1: u32,
    /// Image height
    pub y1: u32,
    /// Number of components
    pub numcomps: u32,
    /// Color space
    pub color_space: c_int,
    /// Image components
    pub comps: *mut opj_image_comp_t,
    /// ICC profile data (null if none)
    pub icc_profile_buf: *mut u8,
    /// ICC profile length
    pub icc_profile_len: u32,
}

/// Image component creation parameters
#[repr(C)]
#[derive(Debug, Clone)]
pub struct opj_image_cmptparm_t {
    /// X component offset compared to the whole image
    pub dx: u32,
    /// Y component offset compared to the whole image
    pub dy: u32,
    /// Component width
    pub w: u32,
    /// Component height
    pub h: u32,
    /// X offset from the origin of the reference grid
    pub x0: u32,
    /// Y offset from the origin of the reference grid
    pub y0: u32,
    /// Precision (number of bits per component value)
    pub prec: u32,
    /// Obsolete: use prec instead
    pub bpp: u32,
    /// Signed (1) / unsigned (0)
    pub sgnd: u32,
}

/// Decompression parameters
#[repr(C)]
#[derive(Debug)]
pub struct opj_dparameters_t {
    /// Set the number of highest resolution levels to be discarded
    pub cp_reduce: u32,
    /// Set the maximum number of quality layers to decode
    pub cp_layer: u32,
    /// Input file name (not used for memory streams)
    pub infile: [c_char; 4096],
    /// Output file name (not used for memory streams)
    pub outfile: [c_char; 4096],
    /// Decoding format (0: J2K, 1: JPT, 2: JP2)
    pub decod_format: c_int,
    /// Output format (not used)
    pub cod_format: c_int,
    /// Decoding area left boundary
    pub da_x0: u32,
    /// Decoding area right boundary
    pub da_x1: u32,
    /// Decoding area up boundary
    pub da_y0: u32,
    /// Decoding area bottom boundary
    pub da_y1: u32,
    /// Verbose mode
    pub m_verbose: c_int,
    /// Tile number to decode (0 = all tiles)
    pub tile_index: u32,
    /// Number of tiles to decode
    pub nb_tile_to_decode: u32,
    /// JPWL correction capabilities
    pub jpwl_correct: c_int,
    /// Expected number of components (JPWL)
    pub jpwl_exp_comps: c_int,
    /// Maximum number of tiles (JPWL)
    pub jpwl_max_tiles: c_int,
    /// Flags (internal use)
    pub flags: u32,
}

/// Compression parameters
#[repr(C)]
#[derive(Debug)]
pub struct opj_cparameters_t {
    /// Tile size on X axis
    pub tile_size_on: c_int,
    /// X offset of the origin of the tile grid
    pub cp_tx0: c_int,
    /// Y offset of the origin of the tile grid
    pub cp_ty0: c_int,
    /// Tile width
    pub cp_tdx: c_int,
    /// Tile height
    pub cp_tdy: c_int,
    /// Allocation by rate/distortion
    pub cp_disto_alloc: c_int,
    /// Allocation by fixed layer
    pub cp_fixed_alloc: c_int,
    /// Fixed layer (not used)
    pub cp_fixed_quality: c_int,
    /// Fixed layer
    pub cp_matrice: *mut c_int,
    /// Comment for coding
    pub cp_comment: *mut c_char,
    /// CSIZ: coding style
    pub csty: c_int,
    /// Progression order
    pub prog_order: c_int,
    /// Progression order changes
    pub POC: [opj_poc_t; 32],
    /// Number of progression order changes
    pub numpocs: u32,
    /// Number of layers
    pub tcp_numlayers: c_int,
    /// Rates of layers (for lossy compression)
    pub tcp_rates: [f32; 100],
    /// Distortion values (for fixed quality)
    pub tcp_distoratio: [f32; 100],
    /// Number of resolutions
    pub numresolution: c_int,
    /// Code-block width
    pub cblockw_init: c_int,
    /// Code-block height
    pub cblockh_init: c_int,
    /// Mode switch
    pub mode: c_int,
    /// Irreversible transform (1) or reversible (0)
    pub irreversible: c_int,
    /// Region of interest: affected component
    pub roi_compno: c_int,
    /// Region of interest: upshift value
    pub roi_shift: c_int,
    /// Precinct width
    pub res_spec: c_int,
    /// Precinct width for each resolution level
    pub prcw_init: [c_int; 33],
    /// Precinct height for each resolution level
    pub prch_init: [c_int; 33],
    /// Input file name (not used for memory streams)
    pub infile: [c_char; 4096],
    /// Output file name (not used for memory streams)
    pub outfile: [c_char; 4096],
    /// Index file name (not used)
    pub index_on: c_int,
    /// Index file name (not used)
    pub index: [c_char; 4096],
    /// Subimage encoding: origin X
    pub image_offset_x0: c_int,
    /// Subimage encoding: origin Y
    pub image_offset_y0: c_int,
    /// Subsampling value for dx
    pub subsampling_dx: c_int,
    /// Subsampling value for dy
    pub subsampling_dy: c_int,
    /// Input file format (not used)
    pub decod_format: c_int,
    /// Output file format (not used)
    pub cod_format: c_int,
    /// JPWL encoding parameters (not used)
    pub jpwl_epc_on: c_int,
    /// JPWL parameters (not used)
    pub jpwl_hprot_MH: c_int,
    /// JPWL parameters (not used)
    pub jpwl_hprot_TPH_tileno: [c_int; 16],
    /// JPWL parameters (not used)
    pub jpwl_hprot_TPH: [c_int; 16],
    /// JPWL parameters (not used)
    pub jpwl_pprot_tileno: [c_int; 16],
    /// JPWL parameters (not used)
    pub jpwl_pprot_packno: [c_int; 16],
    /// JPWL parameters (not used)
    pub jpwl_pprot: [c_int; 16],
    /// JPWL parameters (not used)
    pub jpwl_sens_size: c_int,
    /// JPWL parameters (not used)
    pub jpwl_sens_addr: c_int,
    /// JPWL parameters (not used)
    pub jpwl_sens_range: c_int,
    /// JPWL parameters (not used)
    pub jpwl_sens_MH: c_int,
    /// JPWL parameters (not used)
    pub jpwl_sens_TPH_tileno: [c_int; 16],
    /// JPWL parameters (not used)
    pub jpwl_sens_TPH: [c_int; 16],
    /// Cinema mode (not used)
    pub cp_cinema: c_int,
    /// Maximum rate for each component (not used)
    pub max_comp_size: c_int,
    /// Profile (not used)
    pub cp_rsiz: c_int,
    /// Tile part generation (not used)
    pub tp_on: c_char,
    /// Tile part flag (not used)
    pub tp_flag: c_char,
    /// MCT (multiple component transform)
    pub tcp_mct: c_char,
    /// Enable JPIP indexing (not used)
    pub jpip_on: c_int,
    /// MCT data (not used)
    pub mct_data: *mut c_void,
    /// Maximum memory size for tile data
    pub max_cs_size: c_int,
    /// RSIZ capabilities
    pub rsiz: u16,
}

/// Progression order change
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct opj_poc_t {
    /// Resolution num start, Component num start, given by POC
    pub resno0: u32,
    pub compno0: u32,
    /// Layer num end, Resolution num end, Component num end, given by POC
    pub layno1: u32,
    pub resno1: u32,
    pub compno1: u32,
    /// Layer num start, Precinct num start, Precinct num end
    pub layno0: u32,
    pub precno0: u32,
    pub precno1: u32,
    /// Progression order enum
    pub prg1: c_int,
    pub prg: c_int,
    /// Progression order string
    pub progorder: [c_char; 5],
    /// Tile number (starting at 1)
    pub tile: u32,
    /// Start and end values for Tile width and height
    pub tx0: i32,
    pub tx1: i32,
    pub ty0: i32,
    pub ty1: i32,
    /// Start value, initialised in pi_initialise_encode
    pub lay_s: u32,
    pub res_s: u32,
    pub comp_s: u32,
    pub prc_s: u32,
    /// End value, initialised in pi_initialise_encode
    pub lay_e: u32,
    pub res_e: u32,
    pub comp_e: u32,
    pub prc_e: u32,
    /// Start and end values of Tile width and height, initialised in pi_initialise_encode
    pub tx_s: u32,
    pub tx_e: u32,
    pub ty_s: u32,
    pub ty_e: u32,
    pub dx: u32,
    pub dy: u32,
    /// Temporary values for Tile parts, initialised in pi_create_encode
    pub lay_t: u32,
    pub res_t: u32,
    pub comp_t: u32,
    pub prc_t: u32,
    pub tx0_t: u32,
    pub ty0_t: u32,
}

// =============================================================================
// Callback Types
// =============================================================================

/// Stream read callback function type
pub type opj_stream_read_fn = Option<
    unsafe extern "C" fn(
        p_buffer: *mut c_void,
        p_nb_bytes: usize,
        p_user_data: *mut c_void,
    ) -> usize,
>;

/// Stream write callback function type
pub type opj_stream_write_fn = Option<
    unsafe extern "C" fn(
        p_buffer: *mut c_void,
        p_nb_bytes: usize,
        p_user_data: *mut c_void,
    ) -> usize,
>;

/// Stream skip callback function type
pub type opj_stream_skip_fn =
    Option<unsafe extern "C" fn(p_nb_bytes: i64, p_user_data: *mut c_void) -> i64>;

/// Stream seek callback function type
pub type opj_stream_seek_fn =
    Option<unsafe extern "C" fn(p_nb_bytes: i64, p_user_data: *mut c_void) -> c_int>;

/// Stream free user data callback function type
pub type opj_stream_free_user_data_fn = Option<unsafe extern "C" fn(p_user_data: *mut c_void)>;

/// Message handler callback function type
pub type opj_msg_callback =
    Option<unsafe extern "C" fn(msg: *const c_char, client_data: *mut c_void)>;

// =============================================================================
// External Functions
// =============================================================================

#[link(name = "openjp2")]
extern "C" {
    // -------------------------------------------------------------------------
    // Codec Lifecycle Functions
    // -------------------------------------------------------------------------

    /// Create a decompression codec
    ///
    /// # Arguments
    /// * `format` - Codec format (OPJ_CODEC_J2K or OPJ_CODEC_JP2)
    ///
    /// # Returns
    /// Codec handle or null on failure
    pub fn opj_create_decompress(format: c_int) -> *mut opj_codec_t;

    /// Create a compression codec
    ///
    /// # Arguments
    /// * `format` - Codec format (OPJ_CODEC_J2K or OPJ_CODEC_JP2)
    ///
    /// # Returns
    /// Codec handle or null on failure
    pub fn opj_create_compress(format: c_int) -> *mut opj_codec_t;

    /// Destroy a codec
    ///
    /// # Arguments
    /// * `p_codec` - Codec handle to destroy
    pub fn opj_destroy_codec(p_codec: *mut opj_codec_t);

    // -------------------------------------------------------------------------
    // Setup Functions
    // -------------------------------------------------------------------------

    /// Setup the decoder with decompression parameters
    ///
    /// # Arguments
    /// * `p_codec` - Decompression codec handle
    /// * `parameters` - Decompression parameters
    ///
    /// # Returns
    /// OPJ_TRUE on success, OPJ_FALSE on failure
    pub fn opj_setup_decoder(
        p_codec: *mut opj_codec_t,
        parameters: *mut opj_dparameters_t,
    ) -> c_int;

    /// Setup the encoder with compression parameters
    ///
    /// # Arguments
    /// * `p_codec` - Compression codec handle
    /// * `parameters` - Compression parameters
    /// * `image` - Image to encode
    ///
    /// # Returns
    /// OPJ_TRUE on success, OPJ_FALSE on failure
    pub fn opj_setup_encoder(
        p_codec: *mut opj_codec_t,
        parameters: *mut opj_cparameters_t,
        image: *mut opj_image_t,
    ) -> c_int;

    /// Set default decoder parameters
    ///
    /// # Arguments
    /// * `parameters` - Parameters structure to initialize
    pub fn opj_set_default_decoder_parameters(parameters: *mut opj_dparameters_t);

    /// Set default encoder parameters
    ///
    /// # Arguments
    /// * `parameters` - Parameters structure to initialize
    pub fn opj_set_default_encoder_parameters(parameters: *mut opj_cparameters_t);

    // -------------------------------------------------------------------------
    // Stream Functions
    // -------------------------------------------------------------------------

    /// Create a stream
    ///
    /// # Arguments
    /// * `p_buffer_size` - Size of the internal buffer
    /// * `p_is_input` - Whether this is an input stream (1) or output stream (0)
    ///
    /// # Returns
    /// Stream handle or null on failure
    pub fn opj_stream_create(p_buffer_size: usize, p_is_input: c_int) -> *mut opj_stream_t;

    /// Destroy a stream
    ///
    /// # Arguments
    /// * `p_stream` - Stream handle to destroy
    pub fn opj_stream_destroy(p_stream: *mut opj_stream_t);

    /// Set the read callback for a stream
    pub fn opj_stream_set_read_function(
        p_stream: *mut opj_stream_t,
        p_function: opj_stream_read_fn,
    );

    /// Set the write callback for a stream
    pub fn opj_stream_set_write_function(
        p_stream: *mut opj_stream_t,
        p_function: opj_stream_write_fn,
    );

    /// Set the skip callback for a stream
    pub fn opj_stream_set_skip_function(
        p_stream: *mut opj_stream_t,
        p_function: opj_stream_skip_fn,
    );

    /// Set the seek callback for a stream
    pub fn opj_stream_set_seek_function(
        p_stream: *mut opj_stream_t,
        p_function: opj_stream_seek_fn,
    );

    /// Set the user data for a stream
    pub fn opj_stream_set_user_data(
        p_stream: *mut opj_stream_t,
        p_data: *mut c_void,
        p_function: opj_stream_free_user_data_fn,
    );

    /// Set the length of the user data (stream length)
    pub fn opj_stream_set_user_data_length(p_stream: *mut opj_stream_t, data_length: u64);

    // -------------------------------------------------------------------------
    // Decoding Functions
    // -------------------------------------------------------------------------

    /// Read the main header of the codestream
    ///
    /// # Arguments
    /// * `p_codec` - Decompression codec handle
    /// * `p_stream` - Input stream
    /// * `p_image` - Output image pointer (will be allocated)
    ///
    /// # Returns
    /// OPJ_TRUE on success, OPJ_FALSE on failure
    pub fn opj_read_header(
        p_stream: *mut opj_stream_t,
        p_codec: *mut opj_codec_t,
        p_image: *mut *mut opj_image_t,
    ) -> c_int;

    /// Decode an image from a JPEG 2000 codestream
    ///
    /// # Arguments
    /// * `p_codec` - Decompression codec handle
    /// * `p_stream` - Input stream
    /// * `p_image` - Image to decode into
    ///
    /// # Returns
    /// OPJ_TRUE on success, OPJ_FALSE on failure
    pub fn opj_decode(
        p_codec: *mut opj_codec_t,
        p_stream: *mut opj_stream_t,
        p_image: *mut opj_image_t,
    ) -> c_int;

    /// End decompression
    ///
    /// # Arguments
    /// * `p_codec` - Decompression codec handle
    /// * `p_stream` - Input stream
    ///
    /// # Returns
    /// OPJ_TRUE on success, OPJ_FALSE on failure
    pub fn opj_end_decompress(p_codec: *mut opj_codec_t, p_stream: *mut opj_stream_t) -> c_int;

    /// Set the decode area
    ///
    /// # Arguments
    /// * `p_codec` - Decompression codec handle
    /// * `p_image` - Image
    /// * `p_start_x` - Start X coordinate
    /// * `p_start_y` - Start Y coordinate
    /// * `p_end_x` - End X coordinate
    /// * `p_end_y` - End Y coordinate
    ///
    /// # Returns
    /// OPJ_TRUE on success, OPJ_FALSE on failure
    pub fn opj_set_decode_area(
        p_codec: *mut opj_codec_t,
        p_image: *mut opj_image_t,
        p_start_x: i32,
        p_start_y: i32,
        p_end_x: i32,
        p_end_y: i32,
    ) -> c_int;

    /// Set the decoded resolution factor
    ///
    /// # Arguments
    /// * `p_codec` - Decompression codec handle
    /// * `res_factor` - Resolution factor (0 = full resolution)
    ///
    /// # Returns
    /// OPJ_TRUE on success, OPJ_FALSE on failure
    pub fn opj_set_decoded_resolution_factor(p_codec: *mut opj_codec_t, res_factor: u32) -> c_int;

    // -------------------------------------------------------------------------
    // Tile Decoding Functions
    // -------------------------------------------------------------------------

    /// Read the header of a tile
    ///
    /// # Returns
    /// OPJ_TRUE on success, OPJ_FALSE on failure
    pub fn opj_read_tile_header(
        p_codec: *mut opj_codec_t,
        p_stream: *mut opj_stream_t,
        p_tile_index: *mut u32,
        p_data_size: *mut u32,
        p_tile_x0: *mut i32,
        p_tile_y0: *mut i32,
        p_tile_x1: *mut i32,
        p_tile_y1: *mut i32,
        p_nb_comps: *mut u32,
        p_should_go_on: *mut c_int,
    ) -> c_int;

    /// Decode tile data
    ///
    /// # Returns
    /// OPJ_TRUE on success, OPJ_FALSE on failure
    pub fn opj_decode_tile_data(
        p_codec: *mut opj_codec_t,
        p_tile_index: u32,
        p_data: *mut u8,
        p_data_size: u32,
        p_stream: *mut opj_stream_t,
    ) -> c_int;

    /// Get a decoded tile
    ///
    /// # Returns
    /// OPJ_TRUE on success, OPJ_FALSE on failure
    pub fn opj_get_decoded_tile(
        p_codec: *mut opj_codec_t,
        p_stream: *mut opj_stream_t,
        p_image: *mut opj_image_t,
        tile_index: u32,
    ) -> c_int;

    // -------------------------------------------------------------------------
    // Encoding Functions
    // -------------------------------------------------------------------------

    /// Start compression
    ///
    /// # Arguments
    /// * `p_codec` - Compression codec handle
    /// * `p_image` - Image to encode
    /// * `p_stream` - Output stream
    ///
    /// # Returns
    /// OPJ_TRUE on success, OPJ_FALSE on failure
    pub fn opj_start_compress(
        p_codec: *mut opj_codec_t,
        p_image: *mut opj_image_t,
        p_stream: *mut opj_stream_t,
    ) -> c_int;

    /// Encode an image
    ///
    /// # Arguments
    /// * `p_codec` - Compression codec handle
    /// * `p_stream` - Output stream
    ///
    /// # Returns
    /// OPJ_TRUE on success, OPJ_FALSE on failure
    pub fn opj_encode(p_codec: *mut opj_codec_t, p_stream: *mut opj_stream_t) -> c_int;

    /// End compression
    ///
    /// # Arguments
    /// * `p_codec` - Compression codec handle
    /// * `p_stream` - Output stream
    ///
    /// # Returns
    /// OPJ_TRUE on success, OPJ_FALSE on failure
    pub fn opj_end_compress(p_codec: *mut opj_codec_t, p_stream: *mut opj_stream_t) -> c_int;

    // -------------------------------------------------------------------------
    // Tile Encoding Function
    // -------------------------------------------------------------------------

    /// Write a tile
    ///
    /// # Arguments
    /// * `p_codec` - Compression codec handle
    /// * `p_tile_index` - Tile index
    /// * `p_data` - Tile data
    /// * `p_data_size` - Size of tile data
    /// * `p_stream` - Output stream
    ///
    /// # Returns
    /// OPJ_TRUE on success, OPJ_FALSE on failure
    pub fn opj_write_tile(
        p_codec: *mut opj_codec_t,
        p_tile_index: u32,
        p_data: *mut u8,
        p_data_size: u32,
        p_stream: *mut opj_stream_t,
    ) -> c_int;

    // -------------------------------------------------------------------------
    // Image Functions
    // -------------------------------------------------------------------------

    /// Create an image
    ///
    /// # Arguments
    /// * `numcmpts` - Number of components
    /// * `cmptparms` - Component parameters array
    /// * `clrspc` - Color space
    ///
    /// # Returns
    /// Image handle or null on failure
    pub fn opj_image_create(
        numcmpts: u32,
        cmptparms: *mut opj_image_cmptparm_t,
        clrspc: c_int,
    ) -> *mut opj_image_t;

    /// Create an image for tile-based encoding
    ///
    /// # Arguments
    /// * `numcmpts` - Number of components
    /// * `cmptparms` - Component parameters array
    /// * `clrspc` - Color space
    ///
    /// # Returns
    /// Image handle or null on failure
    pub fn opj_image_tile_create(
        numcmpts: u32,
        cmptparms: *mut opj_image_cmptparm_t,
        clrspc: c_int,
    ) -> *mut opj_image_t;

    /// Destroy an image
    ///
    /// # Arguments
    /// * `image` - Image handle to destroy
    pub fn opj_image_destroy(image: *mut opj_image_t);

    /// Allocate image component data
    ///
    /// # Arguments
    /// * `size` - Size in bytes to allocate
    ///
    /// # Returns
    /// Pointer to allocated memory or null on failure
    pub fn opj_image_data_alloc(size: usize) -> *mut c_void;

    /// Free image component data
    ///
    /// # Arguments
    /// * `ptr` - Pointer to free
    pub fn opj_image_data_free(ptr: *mut c_void);

    // -------------------------------------------------------------------------
    // Info Functions
    // -------------------------------------------------------------------------

    /// Get codestream info
    pub fn opj_get_cstr_info(p_codec: *mut opj_codec_t) -> *mut opj_cstr_info_t;

    /// Get codestream index
    pub fn opj_get_cstr_index(p_codec: *mut opj_codec_t) -> *mut opj_cstr_index_t;

    /// Destroy codestream info
    pub fn opj_destroy_cstr_info(p_cstr_info: *mut *mut opj_cstr_info_t);

    /// Destroy codestream index
    pub fn opj_destroy_cstr_index(p_cstr_index: *mut *mut opj_cstr_index_t);

    // -------------------------------------------------------------------------
    // Message Handler Functions
    // -------------------------------------------------------------------------

    /// Set the info handler callback
    pub fn opj_set_info_handler(
        p_codec: *mut opj_codec_t,
        p_callback: opj_msg_callback,
        p_user_data: *mut c_void,
    ) -> c_int;

    /// Set the warning handler callback
    pub fn opj_set_warning_handler(
        p_codec: *mut opj_codec_t,
        p_callback: opj_msg_callback,
        p_user_data: *mut c_void,
    ) -> c_int;

    /// Set the error handler callback
    pub fn opj_set_error_handler(
        p_codec: *mut opj_codec_t,
        p_callback: opj_msg_callback,
        p_user_data: *mut c_void,
    ) -> c_int;

    // -------------------------------------------------------------------------
    // Threading Function
    // -------------------------------------------------------------------------

    /// Set the number of threads to use for encoding/decoding
    ///
    /// # Arguments
    /// * `p_codec` - Codec handle
    /// * `num_threads` - Number of threads (0 = single-threaded)
    ///
    /// # Returns
    /// OPJ_TRUE on success, OPJ_FALSE on failure
    pub fn opj_codec_set_threads(p_codec: *mut opj_codec_t, num_threads: c_int) -> c_int;

    /// Set extra encoder options
    ///
    /// This may be called after opj_setup_encoder() and before opj_start_compress().
    /// Supported options (since OpenJPEG 2.4.0):
    /// - PLT=YES/NO - Write PLT marker segments (packet length in tile-part header)
    /// - TLM=YES/NO - Write TLM marker segments (tile-part lengths)
    /// - GUARD_BITS=value - Number of guard bits in [0,7] range (since 2.5.0)
    ///
    /// # Arguments
    /// * `p_codec` - Compression codec handle
    /// * `p_options` - NULL-terminated array of "KEY=VALUE" strings
    ///
    /// # Returns
    /// OPJ_TRUE on success, OPJ_FALSE on failure
    pub fn opj_encoder_set_extra_options(
        p_codec: *mut opj_codec_t,
        p_options: *const *const c_char,
    ) -> c_int;
}

// Compile-time size assertions — ensures Rust struct layouts match C ABI
const _: () = assert!(std::mem::size_of::<opj_image_comp_t>() == 64);
const _: () = assert!(std::mem::size_of::<opj_image_t>() == 48);
const _: () = assert!(std::mem::size_of::<opj_image_cmptparm_t>() == 36);
const _: () = assert!(std::mem::size_of::<opj_poc_t>() == 148);
const _: () = assert!(std::mem::size_of::<opj_dparameters_t>() == 8252);
const _: () = assert!(std::mem::size_of::<opj_cparameters_t>() == 18720);
