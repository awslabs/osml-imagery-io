//! Raw extern "C" FFI declarations for libtiff.
//!
//! This module contains the raw C FFI bindings to the libtiff library.
//! These are low-level unsafe bindings - use the safe wrappers in `ffi.rs` instead.
//!
//! # Safety
//!
//! All functions in this module are unsafe and require careful handling of
//! pointers and memory management according to libtiff's API contract.

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

use std::os::raw::{c_char, c_int, c_void};

// =============================================================================
// Callback Types
// =============================================================================

/// Read/write callback for TIFFClientOpen.
/// `fn(clientdata, buf, size) -> bytes_read_or_written`
pub type TIFFReadWriteProc =
    Option<unsafe extern "C" fn(clientdata: *mut c_void, buf: *mut c_void, size: i64) -> i64>;

/// Seek callback for TIFFClientOpen.
/// `fn(clientdata, offset, whence) -> new_position`
pub type TIFFSeekProc =
    Option<unsafe extern "C" fn(clientdata: *mut c_void, offset: i64, whence: c_int) -> i64>;

/// Close callback for TIFFClientOpen.
/// `fn(clientdata) -> status`
pub type TIFFCloseProc = Option<unsafe extern "C" fn(clientdata: *mut c_void) -> c_int>;

/// Size callback for TIFFClientOpen.
/// `fn(clientdata) -> size`
pub type TIFFSizeProc = Option<unsafe extern "C" fn(clientdata: *mut c_void) -> i64>;

/// Memory-map callback for TIFFClientOpen (unused, pass None).
pub type TIFFMapFileProc = Option<
    unsafe extern "C" fn(clientdata: *mut c_void, base: *mut *mut c_void, size: *mut i64) -> c_int,
>;

/// Memory-unmap callback for TIFFClientOpen (unused, pass None).
pub type TIFFUnmapFileProc =
    Option<unsafe extern "C" fn(clientdata: *mut c_void, base: *mut c_void, size: i64)>;

/// Error/warning handler callback for TIFFSetErrorHandler / TIFFSetWarningHandler.
/// Note: libtiff's actual signature uses varargs (`...`), but Rust FFI cannot
/// represent varargs callbacks. We declare the handler as taking a module name
/// and a pre-formatted message string. The custom handlers installed via
/// TIFFSetErrorHandlerExt / TIFFSetWarningHandlerExt receive the formatted
/// string, so this type is used for the Ext variants.
pub type TIFFErrorHandler = Option<unsafe extern "C" fn(module: *const c_char, fmt: *const c_char)>;

/// Extended error/warning handler that receives a client data pointer.
pub type TIFFErrorHandlerExt = Option<
    unsafe extern "C" fn(clientdata: *mut c_void, module: *const c_char, fmt: *const c_char),
>;

// =============================================================================
// External Functions
// =============================================================================

#[link(name = "tiff")]
extern "C" {
    // -------------------------------------------------------------------------
    // Lifecycle Functions
    // -------------------------------------------------------------------------

    /// Open a TIFF file using client-provided I/O callbacks.
    ///
    /// # Arguments
    /// * `name` - Filename (used for error messages only)
    /// * `mode` - Open mode string ("r" for read, "w" for write)
    /// * `clientdata` - Opaque pointer passed to all callbacks
    /// * `readproc` - Read callback
    /// * `writeproc` - Write callback
    /// * `seekproc` - Seek callback
    /// * `closeproc` - Close callback
    /// * `sizeproc` - Size callback
    /// * `mapproc` - Memory-map callback (can be None)
    /// * `unmapproc` - Memory-unmap callback (can be None)
    ///
    /// # Returns
    /// TIFF handle or null on failure
    pub fn TIFFClientOpen(
        name: *const c_char,
        mode: *const c_char,
        clientdata: *mut c_void,
        readproc: TIFFReadWriteProc,
        writeproc: TIFFReadWriteProc,
        seekproc: TIFFSeekProc,
        closeproc: TIFFCloseProc,
        sizeproc: TIFFSizeProc,
        mapproc: TIFFMapFileProc,
        unmapproc: TIFFUnmapFileProc,
    ) -> *mut c_void;

    /// Close a TIFF handle and release all associated resources.
    pub fn TIFFClose(tif: *mut c_void);

    // -------------------------------------------------------------------------
    // Tag Access Functions
    // -------------------------------------------------------------------------

    /// Get the value of a tag from the current directory.
    /// The variadic arguments depend on the tag type.
    ///
    /// # Returns
    /// 1 on success, 0 if the tag is not present
    pub fn TIFFGetField(tif: *mut c_void, tag: u32, ...) -> c_int;

    /// Set the value of a tag in the current directory.
    /// The variadic arguments depend on the tag type.
    ///
    /// # Returns
    /// 1 on success, 0 on failure
    pub fn TIFFSetField(tif: *mut c_void, tag: u32, ...) -> c_int;

    // -------------------------------------------------------------------------
    // Tile I/O Functions
    // -------------------------------------------------------------------------

    /// Read and decompress a tile of data.
    ///
    /// # Arguments
    /// * `tif` - TIFF handle
    /// * `tile` - Tile index
    /// * `buf` - Buffer to receive decompressed data
    /// * `size` - Buffer size in bytes
    ///
    /// # Returns
    /// Number of bytes read, or -1 on error
    pub fn TIFFReadEncodedTile(tif: *mut c_void, tile: u32, buf: *mut c_void, size: i64) -> i64;

    /// Compress and write a tile of data.
    ///
    /// # Arguments
    /// * `tif` - TIFF handle
    /// * `tile` - Tile index
    /// * `data` - Data to compress and write
    /// * `size` - Data size in bytes
    ///
    /// # Returns
    /// Number of bytes written, or -1 on error
    pub fn TIFFWriteEncodedTile(tif: *mut c_void, tile: u32, data: *mut c_void, size: i64) -> i64;

    /// Read a tile of data (raw coordinates version).
    ///
    /// # Arguments
    /// * `tif` - TIFF handle
    /// * `buf` - Buffer to receive data
    /// * `x` - X pixel coordinate
    /// * `y` - Y pixel coordinate
    /// * `z` - Z coordinate (depth)
    /// * `s` - Sample number (for planar configuration)
    ///
    /// # Returns
    /// Number of bytes read, or -1 on error
    pub fn TIFFReadTile(tif: *mut c_void, buf: *mut c_void, x: u32, y: u32, z: u32, s: u16) -> i64;

    /// Write a tile of data (raw coordinates version).
    ///
    /// # Arguments
    /// * `tif` - TIFF handle
    /// * `buf` - Data to write
    /// * `x` - X pixel coordinate
    /// * `y` - Y pixel coordinate
    /// * `z` - Z coordinate (depth)
    /// * `s` - Sample number (for planar configuration)
    ///
    /// # Returns
    /// Number of bytes written, or -1 on error
    pub fn TIFFWriteTile(tif: *mut c_void, buf: *mut c_void, x: u32, y: u32, z: u32, s: u16)
        -> i64;

    /// Return the size in bytes of a decoded tile.
    pub fn TIFFTileSize(tif: *mut c_void) -> i64;

    /// Return the number of tiles in the image.
    pub fn TIFFNumberOfTiles(tif: *mut c_void) -> u32;

    // -------------------------------------------------------------------------
    // Strip I/O Functions
    // -------------------------------------------------------------------------

    /// Read and decompress a strip of data.
    ///
    /// # Arguments
    /// * `tif` - TIFF handle
    /// * `strip` - Strip index
    /// * `buf` - Buffer to receive decompressed data
    /// * `size` - Buffer size in bytes
    ///
    /// # Returns
    /// Number of bytes read, or -1 on error
    pub fn TIFFReadEncodedStrip(tif: *mut c_void, strip: u32, buf: *mut c_void, size: i64) -> i64;

    /// Return the size in bytes of a decoded strip.
    pub fn TIFFStripSize(tif: *mut c_void) -> i64;

    /// Return the number of strips in the image.
    pub fn TIFFNumberOfStrips(tif: *mut c_void) -> u32;

    /// Return whether the image is organized in tiles.
    ///
    /// # Returns
    /// Non-zero if tiled, 0 if stripped
    pub fn TIFFIsTiled(tif: *mut c_void) -> c_int;

    // -------------------------------------------------------------------------
    // Directory (IFD) Navigation Functions
    // -------------------------------------------------------------------------

    /// Set the current directory to the given index.
    ///
    /// # Returns
    /// 1 on success, 0 on failure
    pub fn TIFFSetDirectory(tif: *mut c_void, dirnum: u16) -> c_int;

    /// Return the index of the current directory.
    pub fn TIFFCurrentDirectory(tif: *mut c_void) -> u16;

    /// Return the number of directories in the file.
    pub fn TIFFNumberOfDirectories(tif: *mut c_void) -> u16;

    // -------------------------------------------------------------------------
    // Error/Warning Handler Functions
    // -------------------------------------------------------------------------

    /// Set the error handler callback.
    ///
    /// # Returns
    /// The previous error handler
    pub fn TIFFSetErrorHandler(handler: TIFFErrorHandler) -> TIFFErrorHandler;

    /// Set the warning handler callback.
    ///
    /// # Returns
    /// The previous warning handler
    pub fn TIFFSetWarningHandler(handler: TIFFErrorHandler) -> TIFFErrorHandler;

    /// Set the extended error handler callback (with client data).
    ///
    /// # Returns
    /// The previous extended error handler
    pub fn TIFFSetErrorHandlerExt(handler: TIFFErrorHandlerExt) -> TIFFErrorHandlerExt;

    /// Set the extended warning handler callback (with client data).
    ///
    /// # Returns
    /// The previous extended warning handler
    pub fn TIFFSetWarningHandlerExt(handler: TIFFErrorHandlerExt) -> TIFFErrorHandlerExt;

    /// Set the tag extender callback for custom tag registration.
    ///
    /// # Arguments
    /// * `proc` - The new extender callback, or None to clear
    ///
    /// # Returns
    /// The previous extender callback
    pub fn TIFFSetTagExtender(
        proc_: Option<unsafe extern "C" fn(tif: *mut c_void)>,
    ) -> Option<unsafe extern "C" fn(tif: *mut c_void)>;

    /// Write the current directory to the file and set up to create a new one.
    ///
    /// # Returns
    /// 1 on success, 0 on failure
    pub fn TIFFWriteDirectory(tif: *mut c_void) -> c_int;

    /// Register custom tag definitions with a TIFF handle.
    ///
    /// # Arguments
    /// * `tif` - TIFF handle
    /// * `info` - Array of TIFFFieldInfo structures
    /// * `n` - Number of entries in the array
    ///
    /// # Returns
    /// 0 on success, -1 on failure
    pub fn TIFFMergeFieldInfo(tif: *mut c_void, info: *const TIFFFieldInfo, n: u32) -> c_int;
}

// =============================================================================
// Custom Tag Registration Structures
// =============================================================================

/// Field info structure for registering custom TIFF tags with libtiff.
///
/// This corresponds to libtiff's `TIFFFieldInfo` struct. Each entry describes
/// one custom tag: its numeric ID, read/write count, data type, field bit,
/// whether it's ok to change, whether it has a count, and a human-readable name.
#[repr(C)]
pub struct TIFFFieldInfo {
    /// Tag number (e.g., 34735 for GeoKeyDirectoryTag)
    pub tag: u32,
    /// Number of values for reading (-1 = variable)
    pub read_count: i16,
    /// Number of values for writing (-1 = variable)
    pub write_count: i16,
    /// Data type (see TIFF_* constants below)
    pub data_type: u32,
    /// Field bit (use FIELD_CUSTOM = 65)
    pub field_bit: u16,
    /// Whether the tag value can be changed after initial set (1 = ok)
    pub ok_to_change: u8,
    /// Whether the tag has a count prefix (1 = yes, for variable-length arrays)
    pub pass_count: u8,
    /// Human-readable tag name
    pub name: *const c_char,
}

// SAFETY: TIFFFieldInfo is a plain C struct with no interior mutability.
// The `name` pointer must remain valid for the lifetime of the registration,
// which we ensure by using static string literals.
unsafe impl Send for TIFFFieldInfo {}
unsafe impl Sync for TIFFFieldInfo {}

// libtiff data type constants for TIFFFieldInfo.data_type
/// TIFF BYTE (u8) data type
pub const TIFF_BYTE: u32 = 1;
/// TIFF ASCII (string) data type
pub const TIFF_ASCII: u32 = 2;
/// TIFF SHORT (u16) data type
pub const TIFF_SHORT: u32 = 3;
/// TIFF LONG (u32) data type
pub const TIFF_LONG: u32 = 4;
/// TIFF RATIONAL (two u32) data type
pub const TIFF_RATIONAL: u32 = 5;
/// TIFF SBYTE (i8) data type
pub const TIFF_SBYTE: u32 = 6;
/// TIFF UNDEFINED (u8, application-defined) data type
pub const TIFF_UNDEFINED: u32 = 7;
/// TIFF SSHORT (i16) data type
pub const TIFF_SSHORT: u32 = 8;
/// TIFF SLONG (i32) data type
pub const TIFF_SLONG: u32 = 9;
/// TIFF SRATIONAL (two i32) data type
pub const TIFF_SRATIONAL: u32 = 10;
/// TIFF FLOAT (f32) data type
pub const TIFF_FLOAT: u32 = 11;
/// TIFF DOUBLE (f64) data type
pub const TIFF_DOUBLE: u32 = 12;

/// Field bit value for custom (non-standard) tags
pub const FIELD_CUSTOM: u16 = 65;

// Compile-time size assertions — ensures Rust struct layouts match C ABI
const _: () = assert!(std::mem::size_of::<TIFFFieldInfo>() == 24);
