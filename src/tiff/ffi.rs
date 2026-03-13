//! Safe RAII wrapper around libtiff (TiffHandle) and memory callbacks.
//!
//! This module provides:
//! - `MemoryReadStreamData` and POSIX-style I/O callbacks for `TIFFClientOpen`
//! - `TiffHandle` RAII wrapper with `Drop` calling `TIFFClose`
//! - Thread-local error/warning capture via extended handlers
//! - Typed tag getters and IFD navigation
//! - Tile and strip I/O methods

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

use crate::error::CodecError;

use super::sys;

// =============================================================================
// Thread-local Error/Warning Capture
// =============================================================================

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
    static LAST_WARNING: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Extended error handler callback for libtiff.
/// Receives a pre-formatted error string (no varargs).
unsafe extern "C" fn tiff_error_handler_ext(
    _clientdata: *mut c_void,
    _module: *const c_char,
    fmt: *const c_char,
) {
    if !fmt.is_null() {
        if let Ok(s) = CStr::from_ptr(fmt).to_str() {
            let trimmed = s.trim().to_string();
            LAST_ERROR.with(|e| {
                *e.borrow_mut() = Some(trimmed);
            });
        }
    }
}

/// Extended warning handler callback for libtiff.
/// Receives a pre-formatted warning string (no varargs).
unsafe extern "C" fn tiff_warning_handler_ext(
    _clientdata: *mut c_void,
    _module: *const c_char,
    fmt: *const c_char,
) {
    if !fmt.is_null() {
        if let Ok(s) = CStr::from_ptr(fmt).to_str() {
            let trimmed = s.trim().to_string();
            LAST_WARNING.with(|w| {
                *w.borrow_mut() = Some(trimmed);
            });
        }
    }
}

/// Get and clear the last captured libtiff error message.
fn take_last_error() -> Option<String> {
    LAST_ERROR.with(|e| e.borrow_mut().take())
}

/// Get and clear the last captured libtiff warning message.
#[allow(dead_code)]
fn take_last_warning() -> Option<String> {
    LAST_WARNING.with(|w| w.borrow_mut().take())
}

/// Install thread-local error/warning handlers for libtiff.
/// Suppresses default stderr output and captures messages for programmatic access.
fn install_error_handlers() {
    unsafe {
        // Suppress default varargs handlers (Rust can't represent varargs callbacks)
        sys::TIFFSetErrorHandler(None);
        sys::TIFFSetWarningHandler(None);
        // Install extended handlers that receive pre-formatted strings
        sys::TIFFSetErrorHandlerExt(Some(tiff_error_handler_ext));
        sys::TIFFSetWarningHandlerExt(Some(tiff_warning_handler_ext));
    }
}

// =============================================================================
// Memory Stream Callbacks
// =============================================================================

/// Memory read stream data for TIFFClientOpen callbacks.
/// Holds a pointer to the byte slice data, total length, and current read position.
pub(crate) struct MemoryReadStreamData {
    data: *const u8,
    len: usize,
    pos: usize,
}

/// POSIX-style read callback for libtiff.
/// Reads up to `size` bytes from the memory stream into `buf`.
/// Returns the number of bytes actually read, or -1 on error.
unsafe extern "C" fn tiff_read_proc(
    clientdata: *mut c_void,
    buf: *mut c_void,
    size: i64,
) -> i64 {
    if clientdata.is_null() || buf.is_null() || size < 0 {
        return -1;
    }

    let stream = &mut *(clientdata as *mut MemoryReadStreamData);
    let remaining = stream.len.saturating_sub(stream.pos);
    let to_read = (size as usize).min(remaining);

    if to_read == 0 {
        return 0;
    }

    ptr::copy_nonoverlapping(
        stream.data.add(stream.pos),
        buf as *mut u8,
        to_read,
    );
    stream.pos += to_read;

    to_read as i64
}

/// POSIX-style write callback for libtiff.
/// Returns -1 because this is a read-only stream.
unsafe extern "C" fn tiff_write_proc(
    _clientdata: *mut c_void,
    _buf: *mut c_void,
    _size: i64,
) -> i64 {
    -1 // Read-only
}

/// POSIX-style seek callback for libtiff.
/// Supports SEEK_SET (0), SEEK_CUR (1), and SEEK_END (2).
/// Returns the new absolute position, or -1 on error.
unsafe extern "C" fn tiff_seek_proc(
    clientdata: *mut c_void,
    offset: i64,
    whence: c_int,
) -> i64 {
    if clientdata.is_null() {
        return -1;
    }

    let stream = &mut *(clientdata as *mut MemoryReadStreamData);

    let new_pos: i64 = match whence {
        0 => offset, // SEEK_SET
        1 => stream.pos as i64 + offset, // SEEK_CUR
        2 => stream.len as i64 + offset, // SEEK_END
        _ => return -1,
    };

    if new_pos < 0 {
        return -1;
    }

    // Allow seeking past end (libtiff may do this during probing)
    stream.pos = new_pos as usize;
    new_pos
}

/// Close callback for libtiff. No-op because Rust owns the memory.
unsafe extern "C" fn tiff_close_proc(_clientdata: *mut c_void) -> c_int {
    0 // Success
}

/// Size callback for libtiff. Returns the total byte slice length.
unsafe extern "C" fn tiff_size_proc(clientdata: *mut c_void) -> i64 {
    if clientdata.is_null() {
        return 0;
    }
    let stream = &*(clientdata as *mut MemoryReadStreamData);
    stream.len as i64
}

// =============================================================================
// TiffHandle — Safe RAII Wrapper
// =============================================================================

/// Safe RAII wrapper around a libtiff `TIFF*` handle.
///
/// `Drop` calls `TIFFClose` to release all libtiff resources.
/// Implements `Send` (not `Sync`) — libtiff is not thread-safe for concurrent
/// access to the same handle, so callers must serialize access via `Mutex`.
pub(crate) struct TiffHandle {
    handle: *mut c_void,
    /// Prevent deallocation of the stream data while the handle is alive.
    /// libtiff holds a pointer to this data internally.
    _stream_data: Box<MemoryReadStreamData>,
}

impl std::fmt::Debug for TiffHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TiffHandle")
            .field("handle", &self.handle)
            .finish()
    }
}

// SAFETY: TiffHandle can be transferred between threads. Concurrent access
// is prevented by wrapping in Arc<Mutex<TiffHandle>> at the reader level.
unsafe impl Send for TiffHandle {}

impl Drop for TiffHandle {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                sys::TIFFClose(self.handle);
            }
            self.handle = ptr::null_mut();
        }
    }
}

impl TiffHandle {
    /// Open a TIFF from a byte slice using `TIFFClientOpen` with memory callbacks.
    ///
    /// Returns `CodecError::InvalidFormat` if the data is not a valid TIFF or
    /// if `TIFFClientOpen` fails for any reason.
    pub fn from_bytes(data: &[u8]) -> Result<Self, CodecError> {
        if data.is_empty() {
            return Err(CodecError::InvalidFormat(
                "Cannot open TIFF from empty data".to_string(),
            ));
        }

        install_error_handlers();

        // Clear any stale error messages
        let _ = take_last_error();
        let _ = take_last_warning();

        let stream_data = Box::new(MemoryReadStreamData {
            data: data.as_ptr(),
            len: data.len(),
            pos: 0,
        });

        let clientdata = &*stream_data as *const MemoryReadStreamData as *mut c_void;

        let name = CString::new("memory").unwrap();
        let mode = CString::new("rm").unwrap(); // read, memory-mapped disabled

        let handle = unsafe {
            sys::TIFFClientOpen(
                name.as_ptr(),
                mode.as_ptr(),
                clientdata,
                Some(tiff_read_proc),
                Some(tiff_write_proc),
                Some(tiff_seek_proc),
                Some(tiff_close_proc),
                Some(tiff_size_proc),
                None, // mapproc
                None, // unmapproc
            )
        };

        if handle.is_null() {
            let error_msg = take_last_error()
                .unwrap_or_else(|| "Unknown error opening TIFF".to_string());
            return Err(CodecError::InvalidFormat(format!(
                "Failed to open TIFF: {}",
                error_msg
            )));
        }

        Ok(TiffHandle {
            handle,
            _stream_data: stream_data,
        })
    }

    // =========================================================================
    // Tag Getters
    // =========================================================================

    /// Get a `u16` tag value from the current IFD.
    pub fn get_field_u16(&self, tag: u32) -> Result<u16, CodecError> {
        let mut value: u16 = 0;
        let ret = unsafe { sys::TIFFGetField(self.handle, tag, &mut value as *mut u16) };
        if ret == 1 {
            Ok(value)
        } else {
            Err(CodecError::Decode(format!(
                "TIFF tag {} not found or not a u16",
                tag
            )))
        }
    }

    /// Get a `u32` tag value from the current IFD.
    pub fn get_field_u32(&self, tag: u32) -> Result<u32, CodecError> {
        let mut value: u32 = 0;
        let ret = unsafe { sys::TIFFGetField(self.handle, tag, &mut value as *mut u32) };
        if ret == 1 {
            Ok(value)
        } else {
            Err(CodecError::Decode(format!(
                "TIFF tag {} not found or not a u32",
                tag
            )))
        }
    }

    /// Get an `f32` tag value from the current IFD.
    pub fn get_field_f32(&self, tag: u32) -> Result<f32, CodecError> {
        let mut value: f32 = 0.0;
        let ret = unsafe { sys::TIFFGetField(self.handle, tag, &mut value as *mut f32) };
        if ret == 1 {
            Ok(value)
        } else {
            Err(CodecError::Decode(format!(
                "TIFF tag {} not found or not an f32",
                tag
            )))
        }
    }

    /// Get an `f64` tag value from the current IFD.
    pub fn get_field_f64(&self, tag: u32) -> Result<f64, CodecError> {
        let mut value: f64 = 0.0;
        let ret = unsafe { sys::TIFFGetField(self.handle, tag, &mut value as *mut f64) };
        if ret == 1 {
            Ok(value)
        } else {
            Err(CodecError::Decode(format!(
                "TIFF tag {} not found or not an f64",
                tag
            )))
        }
    }

    /// Get a string tag value from the current IFD.
    pub fn get_field_string(&self, tag: u32) -> Result<String, CodecError> {
        let mut ptr: *const c_char = ptr::null();
        let ret = unsafe { sys::TIFFGetField(self.handle, tag, &mut ptr as *mut *const c_char) };
        if ret == 1 && !ptr.is_null() {
            let cstr = unsafe { CStr::from_ptr(ptr) };
            Ok(cstr.to_string_lossy().into_owned())
        } else {
            Err(CodecError::Decode(format!(
                "TIFF tag {} not found or not a string",
                tag
            )))
        }
    }

    // =========================================================================
    // IFD Navigation
    // =========================================================================

    /// Set the current directory (IFD) to the given index.
    pub fn set_directory(&self, index: u16) -> Result<(), CodecError> {
        let ret = unsafe { sys::TIFFSetDirectory(self.handle, index) };
        if ret == 1 {
            Ok(())
        } else {
            Err(CodecError::Decode(format!(
                "Failed to set TIFF directory to index {}",
                index
            )))
        }
    }

    /// Return the index of the current directory.
    pub fn current_directory(&self) -> u16 {
        unsafe { sys::TIFFCurrentDirectory(self.handle) }
    }

    /// Return the total number of directories (IFDs) in the file.
    pub fn number_of_directories(&self) -> u16 {
        unsafe { sys::TIFFNumberOfDirectories(self.handle) }
    }

    // =========================================================================
    // Tile and Strip I/O
    // =========================================================================

    /// Read and decompress a tile. Returns the decompressed pixel data.
    pub fn read_encoded_tile(&self, tile_index: u32) -> Result<Vec<u8>, CodecError> {
        let buf_size = self.tile_size();
        if buf_size <= 0 {
            return Err(CodecError::Decode(
                "TIFFTileSize returned invalid size".to_string(),
            ));
        }

        let mut buf = vec![0u8; buf_size as usize];
        let bytes_read = unsafe {
            sys::TIFFReadEncodedTile(
                self.handle,
                tile_index,
                buf.as_mut_ptr() as *mut c_void,
                buf_size,
            )
        };

        if bytes_read < 0 {
            let error_msg = take_last_error()
                .unwrap_or_else(|| format!("Failed to read tile {}", tile_index));
            return Err(CodecError::Decode(error_msg));
        }

        buf.truncate(bytes_read as usize);
        Ok(buf)
    }

    /// Read and decompress a strip. Returns the decompressed pixel data.
    pub fn read_encoded_strip(&self, strip_index: u32) -> Result<Vec<u8>, CodecError> {
        let buf_size = self.strip_size();
        if buf_size <= 0 {
            return Err(CodecError::Decode(
                "TIFFStripSize returned invalid size".to_string(),
            ));
        }

        let mut buf = vec![0u8; buf_size as usize];
        let bytes_read = unsafe {
            sys::TIFFReadEncodedStrip(
                self.handle,
                strip_index,
                buf.as_mut_ptr() as *mut c_void,
                buf_size,
            )
        };

        if bytes_read < 0 {
            let error_msg = take_last_error()
                .unwrap_or_else(|| format!("Failed to read strip {}", strip_index));
            return Err(CodecError::Decode(error_msg));
        }

        buf.truncate(bytes_read as usize);
        Ok(buf)
    }

    /// Return the size in bytes of a decoded tile for the current IFD.
    pub fn tile_size(&self) -> i64 {
        unsafe { sys::TIFFTileSize(self.handle) }
    }

    /// Return the size in bytes of a decoded strip for the current IFD.
    pub fn strip_size(&self) -> i64 {
        unsafe { sys::TIFFStripSize(self.handle) }
    }

    /// Return the number of tiles in the current IFD.
    pub fn number_of_tiles(&self) -> u32 {
        unsafe { sys::TIFFNumberOfTiles(self.handle) }
    }

    /// Return the number of strips in the current IFD.
    pub fn number_of_strips(&self) -> u32 {
        unsafe { sys::TIFFNumberOfStrips(self.handle) }
    }

    /// Return whether the current IFD is organized in tiles (vs strips).
    pub fn is_tiled(&self) -> bool {
        unsafe { sys::TIFFIsTiled(self.handle) != 0 }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tiff::tags;

    /// Build a minimal valid TIFF byte buffer: 1x1 pixel, 8-bit grayscale,
    /// uncompressed, stripped layout, little-endian.
    ///
    /// Layout:
    ///   Offset 0:   TIFF header (8 bytes)
    ///   Offset 8:   IFD with N entries
    ///   After IFD:  Pixel data (1 byte)
    fn make_minimal_tiff() -> Vec<u8> {
        let mut buf = Vec::new();

        // --- TIFF Header (8 bytes) ---
        // Byte order: little-endian ("II")
        buf.extend_from_slice(&[0x49, 0x49]);
        // Magic number: 42
        buf.extend_from_slice(&42u16.to_le_bytes());
        // Offset to first IFD: 8
        buf.extend_from_slice(&8u32.to_le_bytes());

        // --- IFD ---
        // We need these required tags for a minimal stripped grayscale TIFF:
        //   ImageWidth (256), ImageLength (257), BitsPerSample (258),
        //   Compression (259), PhotometricInterpretation (262),
        //   StripOffsets (273), SamplesPerPixel (277),
        //   RowsPerStrip (278), StripByteCounts (279)
        let num_entries: u16 = 9;
        buf.extend_from_slice(&num_entries.to_le_bytes());

        // IFD entry helper: tag(u16), type(u16), count(u32), value(u32)
        // TIFF types: SHORT=3, LONG=4
        let short_type: u16 = 3; // SHORT
        let long_type: u16 = 4; // LONG

        // Calculate pixel data offset: header(8) + ifd_count(2) + entries(9*12) + next_ifd(4)
        let pixel_data_offset: u32 = 8 + 2 + (num_entries as u32 * 12) + 4;

        // Entry 1: ImageWidth = 4 (use 4x2 so we have enough data for a strip)
        let width: u32 = 4;
        buf.extend_from_slice(&256u16.to_le_bytes()); // tag
        buf.extend_from_slice(&short_type.to_le_bytes()); // type
        buf.extend_from_slice(&1u32.to_le_bytes()); // count
        buf.extend_from_slice(&width.to_le_bytes()); // value (fits in 4 bytes for SHORT)

        // Entry 2: ImageLength = 2
        let height: u32 = 2;
        buf.extend_from_slice(&257u16.to_le_bytes());
        buf.extend_from_slice(&short_type.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.extend_from_slice(&height.to_le_bytes());

        // Entry 3: BitsPerSample = 8
        buf.extend_from_slice(&258u16.to_le_bytes());
        buf.extend_from_slice(&short_type.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.extend_from_slice(&8u32.to_le_bytes());

        // Entry 4: Compression = 1 (None)
        buf.extend_from_slice(&259u16.to_le_bytes());
        buf.extend_from_slice(&short_type.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes());

        // Entry 5: PhotometricInterpretation = 1 (MinIsBlack)
        buf.extend_from_slice(&262u16.to_le_bytes());
        buf.extend_from_slice(&short_type.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes());

        // Entry 6: StripOffsets = pixel_data_offset
        buf.extend_from_slice(&273u16.to_le_bytes());
        buf.extend_from_slice(&long_type.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.extend_from_slice(&pixel_data_offset.to_le_bytes());

        // Entry 7: SamplesPerPixel = 1
        buf.extend_from_slice(&277u16.to_le_bytes());
        buf.extend_from_slice(&short_type.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes());

        // Entry 8: RowsPerStrip = 2 (all rows in one strip)
        buf.extend_from_slice(&278u16.to_le_bytes());
        buf.extend_from_slice(&short_type.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.extend_from_slice(&height.to_le_bytes());

        // Entry 9: StripByteCounts = width * height * 1 byte
        let strip_bytes: u32 = width * height;
        buf.extend_from_slice(&279u16.to_le_bytes());
        buf.extend_from_slice(&long_type.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.extend_from_slice(&strip_bytes.to_le_bytes());

        // Next IFD offset: 0 (no more IFDs)
        buf.extend_from_slice(&0u32.to_le_bytes());

        // --- Pixel data ---
        // 4x2 = 8 pixels, values 10, 20, 30, 40, 50, 60, 70, 80
        assert_eq!(buf.len(), pixel_data_offset as usize);
        buf.extend_from_slice(&[10, 20, 30, 40, 50, 60, 70, 80]);

        buf
    }

    #[test]
    fn test_from_bytes_valid_tiff() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data);
        assert!(handle.is_ok(), "from_bytes should succeed for valid TIFF data");
        let handle = handle.unwrap();
        assert_eq!(handle.number_of_directories(), 1);
        assert_eq!(handle.current_directory(), 0);
    }

    #[test]
    fn test_from_bytes_invalid_data() {
        let data = b"This is not a TIFF file at all";
        let result = TiffHandle::from_bytes(data);
        assert!(result.is_err());
        match result.unwrap_err() {
            CodecError::InvalidFormat(msg) => {
                assert!(msg.contains("Failed to open TIFF"), "Error: {}", msg);
            }
            other => panic!("Expected InvalidFormat, got: {:?}", other),
        }
    }

    #[test]
    fn test_from_bytes_empty_slice() {
        let result = TiffHandle::from_bytes(&[]);
        assert!(result.is_err());
        match result.unwrap_err() {
            CodecError::InvalidFormat(msg) => {
                assert!(msg.contains("empty"), "Error: {}", msg);
            }
            other => panic!("Expected InvalidFormat, got: {:?}", other),
        }
    }

    #[test]
    fn test_ifd_navigation() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();

        assert_eq!(handle.number_of_directories(), 1);
        assert_eq!(handle.current_directory(), 0);

        // Setting to directory 0 should succeed
        assert!(handle.set_directory(0).is_ok());
        assert_eq!(handle.current_directory(), 0);

        // Setting to directory 1 should fail (only 1 IFD)
        assert!(handle.set_directory(1).is_err());
    }

    #[test]
    fn test_tag_getters() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();

        // ImageWidth = 4 (stored as SHORT, but libtiff normalizes to u32 for these tags)
        let width = handle.get_field_u32(tags::IMAGE_WIDTH).unwrap();
        assert_eq!(width, 4);

        // ImageLength = 2
        let height = handle.get_field_u32(tags::IMAGE_LENGTH).unwrap();
        assert_eq!(height, 2);

        // BitsPerSample = 8
        let bps = handle.get_field_u16(tags::BITS_PER_SAMPLE).unwrap();
        assert_eq!(bps, 8);

        // SamplesPerPixel = 1
        let spp = handle.get_field_u16(tags::SAMPLES_PER_PIXEL).unwrap();
        assert_eq!(spp, 1);

        // Compression = 1 (None)
        let compression = handle.get_field_u16(tags::COMPRESSION).unwrap();
        assert_eq!(compression, tags::COMPRESSION_NONE);

        // PhotometricInterpretation = 1 (MinIsBlack)
        let photo = handle.get_field_u16(tags::PHOTOMETRIC_INTERPRETATION).unwrap();
        assert_eq!(photo, tags::PHOTOMETRIC_MINISBLACK);
    }

    #[test]
    fn test_tag_getter_missing_tag() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();

        // TileWidth is not set in our stripped TIFF
        let result = handle.get_field_u32(tags::TILE_WIDTH);
        assert!(result.is_err());
    }

    #[test]
    fn test_strip_reading() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();

        assert!(!handle.is_tiled());
        assert_eq!(handle.number_of_strips(), 1);
        assert!(handle.strip_size() > 0);

        let strip_data = handle.read_encoded_strip(0).unwrap();
        assert_eq!(strip_data.len(), 8); // 4x2 pixels, 1 byte each
        assert_eq!(strip_data, &[10, 20, 30, 40, 50, 60, 70, 80]);
    }

    #[test]
    fn test_send_trait() {
        // Verify TiffHandle implements Send by moving it to another thread
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();

        let join_handle = std::thread::spawn(move || {
            assert_eq!(handle.number_of_directories(), 1);
        });
        join_handle.join().unwrap();
    }
}
