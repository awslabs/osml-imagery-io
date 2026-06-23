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
    // Register GeoTIFF custom tags so TIFFClientOpen knows about them
    install_geotiff_extender();
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
unsafe extern "C" fn tiff_read_proc(clientdata: *mut c_void, buf: *mut c_void, size: i64) -> i64 {
    if clientdata.is_null() || buf.is_null() || size < 0 {
        return -1;
    }

    let stream = &mut *(clientdata as *mut MemoryReadStreamData);
    let remaining = stream.len.saturating_sub(stream.pos);
    let to_read = (size as usize).min(remaining);

    if to_read == 0 {
        return 0;
    }

    ptr::copy_nonoverlapping(stream.data.add(stream.pos), buf as *mut u8, to_read);
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
unsafe extern "C" fn tiff_seek_proc(clientdata: *mut c_void, offset: i64, whence: c_int) -> i64 {
    if clientdata.is_null() {
        return -1;
    }

    let stream = &mut *(clientdata as *mut MemoryReadStreamData);

    let new_pos: i64 = match whence {
        0 => offset,                     // SEEK_SET
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
// Memory Write Stream Callbacks
// =============================================================================

/// Memory write stream data for TIFFClientOpen write callbacks.
/// Holds a growable `Vec<u8>` buffer and current write position.
pub(crate) struct MemoryWriteStreamData {
    pub(crate) buffer: Vec<u8>,
    pub(crate) pos: usize,
}

/// POSIX-style write callback for writable TIFF streams.
/// Writes bytes into the growable `Vec<u8>` buffer, extending it as needed.
unsafe extern "C" fn tiff_write_proc_writable(
    clientdata: *mut c_void,
    buf: *mut c_void,
    size: i64,
) -> i64 {
    if clientdata.is_null() || buf.is_null() || size < 0 {
        return -1;
    }

    let stream = &mut *(clientdata as *mut MemoryWriteStreamData);
    let count = size as usize;

    // Ensure the buffer is large enough for the write at the current position
    let end = stream.pos + count;
    if end > stream.buffer.len() {
        stream.buffer.resize(end, 0);
    }

    ptr::copy_nonoverlapping(
        buf as *const u8,
        stream.buffer.as_mut_ptr().add(stream.pos),
        count,
    );
    stream.pos += count;

    size
}

/// POSIX-style read callback for writable TIFF streams.
/// libtiff may read back data during write operations (e.g., updating offsets).
unsafe extern "C" fn tiff_read_proc_writable(
    clientdata: *mut c_void,
    buf: *mut c_void,
    size: i64,
) -> i64 {
    if clientdata.is_null() || buf.is_null() || size < 0 {
        return -1;
    }

    let stream = &mut *(clientdata as *mut MemoryWriteStreamData);
    let remaining = stream.buffer.len().saturating_sub(stream.pos);
    let to_read = (size as usize).min(remaining);

    if to_read == 0 {
        return 0;
    }

    ptr::copy_nonoverlapping(
        stream.buffer.as_ptr().add(stream.pos),
        buf as *mut u8,
        to_read,
    );
    stream.pos += to_read;

    to_read as i64
}

/// POSIX-style seek callback for writable TIFF streams.
/// Supports SEEK_SET (0), SEEK_CUR (1), and SEEK_END (2).
unsafe extern "C" fn tiff_seek_proc_writable(
    clientdata: *mut c_void,
    offset: i64,
    whence: c_int,
) -> i64 {
    if clientdata.is_null() {
        return -1;
    }

    let stream = &mut *(clientdata as *mut MemoryWriteStreamData);

    let new_pos: i64 = match whence {
        0 => offset,                              // SEEK_SET
        1 => stream.pos as i64 + offset,          // SEEK_CUR
        2 => stream.buffer.len() as i64 + offset, // SEEK_END
        _ => return -1,
    };

    if new_pos < 0 {
        return -1;
    }

    stream.pos = new_pos as usize;
    new_pos
}

/// Close callback for writable TIFF streams. No-op because Rust owns the memory.
unsafe extern "C" fn tiff_close_proc_writable(_clientdata: *mut c_void) -> c_int {
    0 // Success
}

/// Size callback for writable TIFF streams. Returns the current buffer length.
unsafe extern "C" fn tiff_size_proc_writable(clientdata: *mut c_void) -> i64 {
    if clientdata.is_null() {
        return 0;
    }
    let stream = &*(clientdata as *mut MemoryWriteStreamData);
    stream.buffer.len() as i64
}

// =============================================================================
// Stream Data Enum
// =============================================================================

/// Holds either read or write stream data, keeping it alive for the libtiff handle.
enum StreamData {
    Read(Box<MemoryReadStreamData>),
    Write(Box<MemoryWriteStreamData>),
}

// =============================================================================
// GeoTIFF Custom Tag Registration
// =============================================================================

// Static tag name strings — must outlive the TIFFMergeFieldInfo registration.
static TAG_GEO_KEY_DIR: &[u8] = b"GeoKeyDirectoryTag\0";
static TAG_GEO_DOUBLE: &[u8] = b"GeoDoubleParamsTag\0";
static TAG_GEO_ASCII: &[u8] = b"GeoAsciiParamsTag\0";
static TAG_PIXEL_SCALE: &[u8] = b"ModelPixelScaleTag\0";
static TAG_TIEPOINT: &[u8] = b"ModelTiepointTag\0";
static TAG_TRANSFORMATION: &[u8] = b"ModelTransformationTag\0";

/// Static GeoTIFF field info array for `TIFFMergeFieldInfo`.
/// Tags are sorted ascending by tag number as required by libtiff.
///
/// SAFETY: This array and the name pointers are 'static, satisfying libtiff's
/// requirement that the field info data outlives the TIFF handle.
static GEOTIFF_FIELD_INFO: [sys::TIFFFieldInfo; 6] = [
    sys::TIFFFieldInfo {
        tag: 33550, // MODEL_PIXEL_SCALE_TAG
        read_count: -1,
        write_count: -1,
        data_type: sys::TIFF_DOUBLE,
        field_bit: sys::FIELD_CUSTOM,
        ok_to_change: 1,
        pass_count: 1,
        name: TAG_PIXEL_SCALE.as_ptr() as *const c_char,
    },
    sys::TIFFFieldInfo {
        tag: 33922, // MODEL_TIEPOINT_TAG
        read_count: -1,
        write_count: -1,
        data_type: sys::TIFF_DOUBLE,
        field_bit: sys::FIELD_CUSTOM,
        ok_to_change: 1,
        pass_count: 1,
        name: TAG_TIEPOINT.as_ptr() as *const c_char,
    },
    sys::TIFFFieldInfo {
        tag: 34264, // MODEL_TRANSFORMATION_TAG
        read_count: -1,
        write_count: -1,
        data_type: sys::TIFF_DOUBLE,
        field_bit: sys::FIELD_CUSTOM,
        ok_to_change: 1,
        pass_count: 1,
        name: TAG_TRANSFORMATION.as_ptr() as *const c_char,
    },
    sys::TIFFFieldInfo {
        tag: 34735, // GEO_KEY_DIRECTORY_TAG
        read_count: -1,
        write_count: -1,
        data_type: sys::TIFF_SHORT,
        field_bit: sys::FIELD_CUSTOM,
        ok_to_change: 1,
        pass_count: 1,
        name: TAG_GEO_KEY_DIR.as_ptr() as *const c_char,
    },
    sys::TIFFFieldInfo {
        tag: 34736, // GEO_DOUBLE_PARAMS_TAG
        read_count: -1,
        write_count: -1,
        data_type: sys::TIFF_DOUBLE,
        field_bit: sys::FIELD_CUSTOM,
        ok_to_change: 1,
        pass_count: 1,
        name: TAG_GEO_DOUBLE.as_ptr() as *const c_char,
    },
    sys::TIFFFieldInfo {
        tag: 34737, // GEO_ASCII_PARAMS_TAG
        read_count: -1,
        write_count: -1,
        data_type: sys::TIFF_ASCII,
        field_bit: sys::FIELD_CUSTOM,
        ok_to_change: 1,
        pass_count: 0,
        name: TAG_GEO_ASCII.as_ptr() as *const c_char,
    },
];

/// Register GeoTIFF custom tags with a TIFF handle via `TIFFMergeFieldInfo`.
///
/// Called after `TIFFClientOpen` to make libtiff aware of GeoTIFF tags.
fn geotiff_merge_tags(handle: *mut c_void) {
    unsafe {
        sys::TIFFMergeFieldInfo(
            handle,
            GEOTIFF_FIELD_INFO.as_ptr(),
            GEOTIFF_FIELD_INFO.len() as u32,
        );
    }
}

/// Previous tag extender saved by `install_geotiff_extender`.
static PARENT_EXTENDER: std::sync::Mutex<Option<unsafe extern "C" fn(*mut c_void)>> =
    std::sync::Mutex::new(None);

/// Tag extender callback installed via `TIFFSetTagExtender`.
/// libtiff calls this for every `TIFFClientOpen`, giving us a chance to
/// register custom tags on the newly opened handle.
unsafe extern "C" fn geotiff_tag_extender(tif: *mut c_void) {
    geotiff_merge_tags(tif);

    // Chain to any previously installed extender
    if let Ok(guard) = PARENT_EXTENDER.lock() {
        if let Some(parent) = *guard {
            parent(tif);
        }
    }
}

/// Install the GeoTIFF tag extender globally. Safe to call multiple times —
/// uses `Once` to ensure the extender is installed exactly once.
fn install_geotiff_extender() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| unsafe {
        let prev = sys::TIFFSetTagExtender(Some(geotiff_tag_extender));
        if let Ok(mut guard) = PARENT_EXTENDER.lock() {
            *guard = prev;
        }
    });
}

// =============================================================================
// IFD Tag Entry
// =============================================================================

/// Describes a single tag entry in a TIFF IFD.
///
/// Returned by [`TiffHandle::enumerate_ifd_tags`] to allow callers to discover
/// all tags present in an IFD without a hardcoded list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IfdTagEntry {
    /// TIFF tag number (e.g. 256 for ImageWidth).
    pub tag: u32,
    /// TIFF field type (1=BYTE, 2=ASCII, … 12=DOUBLE, 16=LONG8, etc).
    pub field_type: u16,
    /// Number of values for this tag (u64 for BigTIFF, capped at u64::MAX).
    pub count: u64,
}

// =============================================================================
// IfdReader — Format-aware IFD navigation
// =============================================================================

const MAX_IFD_ENTRIES: u64 = 4096;

struct IfdReader<'a> {
    raw: &'a [u8],
    is_little_endian: bool,
    is_bigtiff: bool,
}

impl<'a> IfdReader<'a> {
    fn new(raw: &'a [u8], is_bigtiff: bool) -> Result<Self, CodecError> {
        let min_header = if is_bigtiff { 16 } else { 8 };
        if raw.len() < min_header {
            return Err(CodecError::Decode(
                "TIFF data too short for header".to_string(),
            ));
        }

        let is_little_endian = match (raw[0], raw[1]) {
            (0x49, 0x49) => true,
            (0x4D, 0x4D) => false,
            _ => {
                return Err(CodecError::Decode(
                    "Invalid TIFF byte order marker".to_string(),
                ));
            }
        };

        Ok(Self {
            raw,
            is_little_endian,
            is_bigtiff,
        })
    }

    fn read_u16(&self, offset: usize) -> Result<u16, CodecError> {
        if offset + 2 > self.raw.len() {
            return Err(CodecError::Decode(format!(
                "Read u16 out of bounds at offset {}",
                offset
            )));
        }
        let bytes: [u8; 2] = [self.raw[offset], self.raw[offset + 1]];
        Ok(if self.is_little_endian {
            u16::from_le_bytes(bytes)
        } else {
            u16::from_be_bytes(bytes)
        })
    }

    fn read_i16(&self, offset: usize) -> Result<i16, CodecError> {
        if offset + 2 > self.raw.len() {
            return Err(CodecError::Decode(format!(
                "Read i16 out of bounds at offset {}",
                offset
            )));
        }
        let bytes: [u8; 2] = [self.raw[offset], self.raw[offset + 1]];
        Ok(if self.is_little_endian {
            i16::from_le_bytes(bytes)
        } else {
            i16::from_be_bytes(bytes)
        })
    }

    fn read_u32(&self, offset: usize) -> Result<u32, CodecError> {
        if offset + 4 > self.raw.len() {
            return Err(CodecError::Decode(format!(
                "Read u32 out of bounds at offset {}",
                offset
            )));
        }
        let bytes: [u8; 4] = [
            self.raw[offset],
            self.raw[offset + 1],
            self.raw[offset + 2],
            self.raw[offset + 3],
        ];
        Ok(if self.is_little_endian {
            u32::from_le_bytes(bytes)
        } else {
            u32::from_be_bytes(bytes)
        })
    }

    fn read_i32(&self, offset: usize) -> Result<i32, CodecError> {
        if offset + 4 > self.raw.len() {
            return Err(CodecError::Decode(format!(
                "Read i32 out of bounds at offset {}",
                offset
            )));
        }
        let bytes: [u8; 4] = [
            self.raw[offset],
            self.raw[offset + 1],
            self.raw[offset + 2],
            self.raw[offset + 3],
        ];
        Ok(if self.is_little_endian {
            i32::from_le_bytes(bytes)
        } else {
            i32::from_be_bytes(bytes)
        })
    }

    fn read_u64(&self, offset: usize) -> Result<u64, CodecError> {
        if offset + 8 > self.raw.len() {
            return Err(CodecError::Decode(format!(
                "Read u64 out of bounds at offset {}",
                offset
            )));
        }
        let bytes: [u8; 8] = [
            self.raw[offset],
            self.raw[offset + 1],
            self.raw[offset + 2],
            self.raw[offset + 3],
            self.raw[offset + 4],
            self.raw[offset + 5],
            self.raw[offset + 6],
            self.raw[offset + 7],
        ];
        Ok(if self.is_little_endian {
            u64::from_le_bytes(bytes)
        } else {
            u64::from_be_bytes(bytes)
        })
    }

    fn read_i64(&self, offset: usize) -> Result<i64, CodecError> {
        if offset + 8 > self.raw.len() {
            return Err(CodecError::Decode(format!(
                "Read i64 out of bounds at offset {}",
                offset
            )));
        }
        let bytes: [u8; 8] = [
            self.raw[offset],
            self.raw[offset + 1],
            self.raw[offset + 2],
            self.raw[offset + 3],
            self.raw[offset + 4],
            self.raw[offset + 5],
            self.raw[offset + 6],
            self.raw[offset + 7],
        ];
        Ok(if self.is_little_endian {
            i64::from_le_bytes(bytes)
        } else {
            i64::from_be_bytes(bytes)
        })
    }

    fn read_f32(&self, offset: usize) -> Result<f32, CodecError> {
        if offset + 4 > self.raw.len() {
            return Err(CodecError::Decode(format!(
                "Read f32 out of bounds at offset {}",
                offset
            )));
        }
        let bytes: [u8; 4] = [
            self.raw[offset],
            self.raw[offset + 1],
            self.raw[offset + 2],
            self.raw[offset + 3],
        ];
        Ok(if self.is_little_endian {
            f32::from_le_bytes(bytes)
        } else {
            f32::from_be_bytes(bytes)
        })
    }

    fn read_f64(&self, offset: usize) -> Result<f64, CodecError> {
        if offset + 8 > self.raw.len() {
            return Err(CodecError::Decode(format!(
                "Read f64 out of bounds at offset {}",
                offset
            )));
        }
        let bytes: [u8; 8] = [
            self.raw[offset],
            self.raw[offset + 1],
            self.raw[offset + 2],
            self.raw[offset + 3],
            self.raw[offset + 4],
            self.raw[offset + 5],
            self.raw[offset + 6],
            self.raw[offset + 7],
        ];
        Ok(if self.is_little_endian {
            f64::from_le_bytes(bytes)
        } else {
            f64::from_be_bytes(bytes)
        })
    }

    fn first_ifd_offset(&self) -> Result<u64, CodecError> {
        if self.is_bigtiff {
            self.read_u64(8)
        } else {
            self.read_u32(4).map(|v| v as u64)
        }
    }

    fn entry_size(&self) -> usize {
        if self.is_bigtiff {
            20
        } else {
            12
        }
    }

    fn read_entry_count(&self, ifd_offset: usize) -> Result<u64, CodecError> {
        if self.is_bigtiff {
            self.read_u64(ifd_offset)
        } else {
            self.read_u16(ifd_offset).map(|v| v as u64)
        }
    }

    fn entry_count_size(&self) -> usize {
        if self.is_bigtiff {
            8
        } else {
            2
        }
    }

    fn read_next_ifd_offset(&self, ifd_offset: usize, entry_count: u64) -> Result<u64, CodecError> {
        let pos = ifd_offset + self.entry_count_size() + entry_count as usize * self.entry_size();
        if self.is_bigtiff {
            self.read_u64(pos)
        } else {
            self.read_u32(pos).map(|v| v as u64)
        }
    }

    fn walk_to_ifd(&self, target_dir: u32) -> Result<usize, CodecError> {
        let mut ifd_offset = self.first_ifd_offset()? as usize;
        for _ in 0..target_dir {
            if ifd_offset == 0 {
                return Err(CodecError::Decode(format!(
                    "IFD chain broken before directory {}",
                    target_dir
                )));
            }
            let entry_count = self.read_entry_count(ifd_offset)?;
            ifd_offset = self.read_next_ifd_offset(ifd_offset, entry_count)? as usize;
        }
        if ifd_offset == 0 {
            return Err(CodecError::Decode(format!(
                "Invalid IFD offset 0 for directory {}",
                target_dir
            )));
        }
        Ok(ifd_offset)
    }

    fn enumerate_entries(&self, ifd_offset: usize) -> Result<Vec<IfdTagEntry>, CodecError> {
        let entry_count = self.read_entry_count(ifd_offset)?;
        let capped_count = entry_count.min(MAX_IFD_ENTRIES) as usize;
        let entries_start = ifd_offset + self.entry_count_size();
        let entry_size = self.entry_size();

        let mut entries = Vec::with_capacity(capped_count);
        for i in 0..capped_count {
            let eo = entries_start + i * entry_size;
            if eo + entry_size > self.raw.len() {
                break;
            }
            let tag = self.read_u16(eo)? as u32;
            let field_type = self.read_u16(eo + 2)?;
            let count = if self.is_bigtiff {
                self.read_u64(eo + 4)?
            } else {
                self.read_u32(eo + 4)? as u64
            };
            entries.push(IfdTagEntry {
                tag,
                field_type,
                count,
            });
        }

        Ok(entries)
    }

    fn inline_threshold(&self) -> usize {
        if self.is_bigtiff {
            8
        } else {
            4
        }
    }

    fn value_field_offset_in_entry(&self) -> usize {
        if self.is_bigtiff {
            12
        } else {
            8
        }
    }

    fn read_data_offset(&self, entry_file_offset: usize) -> Result<u64, CodecError> {
        let vfo = entry_file_offset + self.value_field_offset_in_entry();
        if self.is_bigtiff {
            self.read_u64(vfo)
        } else {
            self.read_u32(vfo).map(|v| v as u64)
        }
    }

    fn find_entry_offset(&self, ifd_offset: usize, tag: u32) -> Result<Option<usize>, CodecError> {
        let entry_count = self.read_entry_count(ifd_offset)?;
        let capped_count = entry_count.min(MAX_IFD_ENTRIES) as usize;
        let entries_start = ifd_offset + self.entry_count_size();
        let entry_size = self.entry_size();

        for i in 0..capped_count {
            let eo = entries_start + i * entry_size;
            if eo + entry_size > self.raw.len() {
                break;
            }
            let t = self.read_u16(eo)? as u32;
            if t == tag {
                return Ok(Some(eo));
            }
        }
        Ok(None)
    }
}

// =============================================================================
// TiffHandle
// =============================================================================

/// Convert an f64 to a `serde_json::Value`, preferring integer representation
/// when the value has no fractional part.
fn json_f64(v: f64) -> serde_json::Value {
    if v.fract() == 0.0 && v.is_finite() && v.abs() < (i64::MAX as f64) {
        serde_json::Value::from(v as i64)
    } else {
        serde_json::Value::from(v)
    }
}

/// Safe RAII wrapper around a libtiff `TIFF*` handle.
///
/// `Drop` calls `TIFFClose` to release all libtiff resources.
/// Implements `Send` (not `Sync`) — libtiff is not thread-safe for concurrent
/// access to the same handle, so callers must serialize access via `Mutex`.
pub(crate) struct TiffHandle {
    handle: *mut c_void,
    /// Prevent deallocation of the stream data while the handle is alive.
    /// libtiff holds a pointer to this data internally.
    _stream_data: StreamData,
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
            let error_msg =
                take_last_error().unwrap_or_else(|| "Unknown error opening TIFF".to_string());
            return Err(CodecError::InvalidFormat(format!(
                "Failed to open TIFF: {}",
                error_msg
            )));
        }

        let tiff = TiffHandle {
            handle,
            _stream_data: StreamData::Read(stream_data),
        };
        Ok(tiff)
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
    pub fn set_directory(&self, index: u32) -> Result<(), CodecError> {
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
    pub fn current_directory(&self) -> u32 {
        unsafe { sys::TIFFCurrentDirectory(self.handle) }
    }

    /// Return the total number of directories (IFDs) in the file.
    pub fn number_of_directories(&self) -> u32 {
        unsafe { sys::TIFFNumberOfDirectories(self.handle) }
    }

    // =========================================================================
    // IFD Tag Enumeration
    // =========================================================================

    /// Enumerate all tag entries in the current IFD by parsing raw TIFF bytes.
    ///
    /// Returns a list of [`IfdTagEntry`] describing every tag present in the
    /// current directory. This avoids depending on a hardcoded tag list and
    /// gives callers full visibility into the IFD contents.
    ///
    /// Only available for read-mode handles (requires access to raw bytes).
    pub fn enumerate_ifd_tags(&self) -> Result<Vec<IfdTagEntry>, CodecError> {
        let raw = match &self._stream_data {
            StreamData::Read(rd) => unsafe { std::slice::from_raw_parts(rd.data, rd.len) },
            StreamData::Write(_) => {
                return Err(CodecError::Decode(
                    "enumerate_ifd_tags requires a read-mode handle".to_string(),
                ));
            }
        };

        let reader = IfdReader::new(raw, self.is_bigtiff())?;
        let ifd_offset = reader.walk_to_ifd(self.current_directory())?;
        reader.enumerate_entries(ifd_offset)
    }

    /// Read a tag value from the current IFD given its [`IfdTagEntry`].
    ///
    /// Returns the value as a `serde_json::Value`:
    /// - BYTE, SHORT, LONG → integer (or array for count > 1)
    /// - SBYTE, SSHORT, SLONG → integer (or array for count > 1)
    /// - ASCII → string
    /// - RATIONAL, SRATIONAL → float (num/denom) or array of floats
    /// - FLOAT, DOUBLE → float (or array for count > 1)
    /// - UNDEFINED → always array of byte integers
    /// - IFD (13), LONG8 (16), IFD8 (18) → u64 integer (or array)
    /// - SLONG8 (17) → i64 integer (or array)
    ///
    /// Returns `Err(CodecError::Decode)` for unknown field types or corrupt data.
    pub fn read_tag_value(&self, entry: &IfdTagEntry) -> Result<serde_json::Value, CodecError> {
        use serde_json::Value;

        let raw = match &self._stream_data {
            StreamData::Read(rd) => unsafe { std::slice::from_raw_parts(rd.data, rd.len) },
            StreamData::Write(_) => {
                return Err(CodecError::Decode(
                    "read_tag_value requires a read-mode handle".to_string(),
                ));
            }
        };

        let reader = IfdReader::new(raw, self.is_bigtiff())?;

        // TIFF field type sizes in bytes
        let type_size = match entry.field_type {
            1 => 1,  // BYTE
            2 => 1,  // ASCII
            3 => 2,  // SHORT
            4 => 4,  // LONG
            5 => 8,  // RATIONAL
            6 => 1,  // SBYTE
            7 => 1,  // UNDEFINED
            8 => 2,  // SSHORT
            9 => 4,  // SLONG
            10 => 8, // SRATIONAL
            11 => 4, // FLOAT
            12 => 8, // DOUBLE
            13 => 4, // IFD (u32 sub-IFD pointer)
            16 => 8, // LONG8
            17 => 8, // SLONG8
            18 => 8, // IFD8
            other => {
                return Err(CodecError::Decode(format!(
                    "Unknown TIFF field type {} for tag {}",
                    other, entry.tag
                )));
            }
        };

        let total_bytes = entry.count as usize * type_size;

        // Find the IFD entry for this tag to get the value/offset field.
        let ifd_offset = reader.walk_to_ifd(self.current_directory())?;
        let entry_offset = reader
            .find_entry_offset(ifd_offset, entry.tag)?
            .ok_or_else(|| CodecError::Decode(format!("Tag {} not found in IFD", entry.tag)))?;

        let value_field_offset = entry_offset + reader.value_field_offset_in_entry();
        let data_offset = if total_bytes <= reader.inline_threshold() {
            value_field_offset
        } else {
            reader.read_data_offset(entry_offset)? as usize
        };

        if data_offset + total_bytes > raw.len() {
            return Err(CodecError::Decode(format!(
                "Tag {} data at offset {} extends beyond file (need {} bytes, file is {} bytes)",
                entry.tag,
                data_offset,
                total_bytes,
                raw.len()
            )));
        }

        let count = entry.count as usize;

        match entry.field_type {
            // BYTE (1)
            1 => {
                if count == 1 {
                    Ok(Value::from(raw[data_offset] as u64))
                } else {
                    let arr: Vec<Value> = (0..count)
                        .map(|i| Value::from(raw[data_offset + i] as u64))
                        .collect();
                    Ok(Value::Array(arr))
                }
            }
            // ASCII (2)
            2 => {
                let end = data_offset + count;
                let slice = &raw[data_offset..end];
                let s = std::str::from_utf8(slice)
                    .unwrap_or("")
                    .trim_end_matches('\0');
                Ok(Value::String(s.to_string()))
            }
            // SHORT (3)
            3 => {
                if count == 1 {
                    Ok(Value::from(reader.read_u16(data_offset)? as u64))
                } else {
                    let arr: Result<Vec<Value>, _> = (0..count)
                        .map(|i| {
                            reader
                                .read_u16(data_offset + i * 2)
                                .map(|v| Value::from(v as u64))
                        })
                        .collect();
                    Ok(Value::Array(arr?))
                }
            }
            // LONG (4) or IFD (13) — both u32
            4 | 13 => {
                if count == 1 {
                    Ok(Value::from(reader.read_u32(data_offset)? as u64))
                } else {
                    let arr: Result<Vec<Value>, _> = (0..count)
                        .map(|i| {
                            reader
                                .read_u32(data_offset + i * 4)
                                .map(|v| Value::from(v as u64))
                        })
                        .collect();
                    Ok(Value::Array(arr?))
                }
            }
            // RATIONAL (5) — two u32: numerator / denominator → float
            5 => {
                let read_rational = |off: usize| -> Result<Value, CodecError> {
                    let num = reader.read_u32(off)? as f64;
                    let den = reader.read_u32(off + 4)? as f64;
                    let val = if den == 0.0 { f64::NAN } else { num / den };
                    Ok(json_f64(val))
                };
                if count == 1 {
                    read_rational(data_offset)
                } else {
                    let arr: Result<Vec<Value>, _> = (0..count)
                        .map(|i| read_rational(data_offset + i * 8))
                        .collect();
                    Ok(Value::Array(arr?))
                }
            }
            // SBYTE (6)
            6 => {
                if count == 1 {
                    Ok(Value::from(raw[data_offset] as i8 as i64))
                } else {
                    let arr: Vec<Value> = (0..count)
                        .map(|i| Value::from(raw[data_offset + i] as i8 as i64))
                        .collect();
                    Ok(Value::Array(arr))
                }
            }
            // UNDEFINED (7) — always array of byte values
            7 => {
                let arr: Vec<Value> = (0..count)
                    .map(|i| Value::from(raw[data_offset + i] as u64))
                    .collect();
                Ok(Value::Array(arr))
            }
            // SSHORT (8)
            8 => {
                if count == 1 {
                    Ok(Value::from(reader.read_i16(data_offset)? as i64))
                } else {
                    let arr: Result<Vec<Value>, _> = (0..count)
                        .map(|i| {
                            reader
                                .read_i16(data_offset + i * 2)
                                .map(|v| Value::from(v as i64))
                        })
                        .collect();
                    Ok(Value::Array(arr?))
                }
            }
            // SLONG (9)
            9 => {
                if count == 1 {
                    Ok(Value::from(reader.read_i32(data_offset)? as i64))
                } else {
                    let arr: Result<Vec<Value>, _> = (0..count)
                        .map(|i| {
                            reader
                                .read_i32(data_offset + i * 4)
                                .map(|v| Value::from(v as i64))
                        })
                        .collect();
                    Ok(Value::Array(arr?))
                }
            }
            // SRATIONAL (10) — two i32: numerator / denominator → float
            10 => {
                let read_srational = |off: usize| -> Result<Value, CodecError> {
                    let num = reader.read_i32(off)? as f64;
                    let den = reader.read_i32(off + 4)? as f64;
                    let val = if den == 0.0 { f64::NAN } else { num / den };
                    Ok(json_f64(val))
                };
                if count == 1 {
                    read_srational(data_offset)
                } else {
                    let arr: Result<Vec<Value>, _> = (0..count)
                        .map(|i| read_srational(data_offset + i * 8))
                        .collect();
                    Ok(Value::Array(arr?))
                }
            }
            // FLOAT (11)
            11 => {
                if count == 1 {
                    Ok(json_f64(reader.read_f32(data_offset)? as f64))
                } else {
                    let arr: Result<Vec<Value>, _> = (0..count)
                        .map(|i| {
                            reader
                                .read_f32(data_offset + i * 4)
                                .map(|v| json_f64(v as f64))
                        })
                        .collect();
                    Ok(Value::Array(arr?))
                }
            }
            // DOUBLE (12)
            12 => {
                if count == 1 {
                    Ok(json_f64(reader.read_f64(data_offset)?))
                } else {
                    let arr: Result<Vec<Value>, _> = (0..count)
                        .map(|i| reader.read_f64(data_offset + i * 8).map(json_f64))
                        .collect();
                    Ok(Value::Array(arr?))
                }
            }
            // LONG8 (16) or IFD8 (18) — u64
            16 | 18 => {
                if count == 1 {
                    Ok(Value::from(reader.read_u64(data_offset)?))
                } else {
                    let arr: Result<Vec<Value>, _> = (0..count)
                        .map(|i| reader.read_u64(data_offset + i * 8).map(Value::from))
                        .collect();
                    Ok(Value::Array(arr?))
                }
            }
            // SLONG8 (17) — i64
            17 => {
                if count == 1 {
                    Ok(Value::from(reader.read_i64(data_offset)?))
                } else {
                    let arr: Result<Vec<Value>, _> = (0..count)
                        .map(|i| reader.read_i64(data_offset + i * 8).map(Value::from))
                        .collect();
                    Ok(Value::Array(arr?))
                }
            }
            _ => unreachable!(),
        }
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
            let error_msg =
                take_last_error().unwrap_or_else(|| format!("Failed to read tile {}", tile_index));
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

    /// Return whether the file is BigTIFF format.
    pub fn is_bigtiff(&self) -> bool {
        unsafe { sys::TIFFIsBigTIFF(self.handle) != 0 }
    }

    // =========================================================================
    // Write-Mode Constructor and Methods
    // =========================================================================

    /// Open a new TIFF for writing using `TIFFClientOpen` with memory write callbacks.
    ///
    /// When `bigtiff` is true, produces BigTIFF output (mode `"w8"`); otherwise
    /// produces classic TIFF (mode `"w"`).
    ///
    /// Returns a `TiffHandle` backed by a growable `Vec<u8>` buffer. Use
    /// `set_field_u16()` / `set_field_u32()` to set tags, `write_encoded_tile()`
    /// to write tile data, `write_directory()` to finalize each IFD, and
    /// `into_bytes()` to close the handle and extract the assembled TIFF bytes.
    ///
    /// Returns `CodecError::Encode` if `TIFFClientOpen` fails.
    pub fn from_write(bigtiff: bool) -> Result<Self, CodecError> {
        install_error_handlers();

        let _ = take_last_error();
        let _ = take_last_warning();

        let stream_data = Box::new(MemoryWriteStreamData {
            buffer: Vec::new(),
            pos: 0,
        });

        let clientdata = &*stream_data as *const MemoryWriteStreamData as *mut c_void;

        let name = CString::new("memory").unwrap();
        let mode = CString::new(if bigtiff { "w8" } else { "w" }).unwrap();

        let handle = unsafe {
            sys::TIFFClientOpen(
                name.as_ptr(),
                mode.as_ptr(),
                clientdata,
                Some(tiff_read_proc_writable),
                Some(tiff_write_proc_writable),
                Some(tiff_seek_proc_writable),
                Some(tiff_close_proc_writable),
                Some(tiff_size_proc_writable),
                None, // mapproc
                None, // unmapproc
            )
        };

        if handle.is_null() {
            let error_msg = take_last_error()
                .unwrap_or_else(|| "Unknown error creating TIFF writer".to_string());
            return Err(CodecError::Encode(format!(
                "Failed to create TIFF writer: {}",
                error_msg
            )));
        }

        let tiff = TiffHandle {
            handle,
            _stream_data: StreamData::Write(stream_data),
        };
        Ok(tiff)
    }

    /// Set a `u16` tag value in the current IFD.
    pub fn set_field_u16(&self, tag: u32, value: u16) -> Result<(), CodecError> {
        let ret = unsafe { sys::TIFFSetField(self.handle, tag, value as c_int) };
        if ret == 1 {
            Ok(())
        } else {
            let error_msg = take_last_error().unwrap_or_else(|| {
                format!("Failed to set TIFF tag {} to u16 value {}", tag, value)
            });
            Err(CodecError::Encode(error_msg))
        }
    }

    /// Set a `u32` tag value in the current IFD.
    pub fn set_field_u32(&self, tag: u32, value: u32) -> Result<(), CodecError> {
        let ret = unsafe { sys::TIFFSetField(self.handle, tag, value) };
        if ret == 1 {
            Ok(())
        } else {
            let error_msg = take_last_error().unwrap_or_else(|| {
                format!("Failed to set TIFF tag {} to u32 value {}", tag, value)
            });
            Err(CodecError::Encode(error_msg))
        }
    }

    /// Set an `i32` (SLONG) tag value in the current IFD.
    pub fn set_field_i32(&self, tag: u32, value: i32) -> Result<(), CodecError> {
        let ret = unsafe { sys::TIFFSetField(self.handle, tag, value) };
        if ret == 1 {
            Ok(())
        } else {
            let error_msg = take_last_error().unwrap_or_else(|| {
                format!("Failed to set TIFF tag {} to i32 value {}", tag, value)
            });
            Err(CodecError::Encode(error_msg))
        }
    }

    /// Set an `f32` (FLOAT) tag value in the current IFD.
    pub fn set_field_f32(&self, tag: u32, value: f32) -> Result<(), CodecError> {
        // libtiff expects a double for float tags via varargs promotion
        let ret = unsafe { sys::TIFFSetField(self.handle, tag, value as f64) };
        if ret == 1 {
            Ok(())
        } else {
            let error_msg = take_last_error().unwrap_or_else(|| {
                format!("Failed to set TIFF tag {} to f32 value {}", tag, value)
            });
            Err(CodecError::Encode(error_msg))
        }
    }

    /// Set an `f64` (DOUBLE) tag value in the current IFD.
    pub fn set_field_f64(&self, tag: u32, value: f64) -> Result<(), CodecError> {
        let ret = unsafe { sys::TIFFSetField(self.handle, tag, value) };
        if ret == 1 {
            Ok(())
        } else {
            let error_msg = take_last_error().unwrap_or_else(|| {
                format!("Failed to set TIFF tag {} to f64 value {}", tag, value)
            });
            Err(CodecError::Encode(error_msg))
        }
    }

    /// Write a BYTE array tag (or UNDEFINED) to the current IFD.
    ///
    /// Uses count+pointer semantics for variable-length byte data.
    pub fn set_field_u8_array(&self, tag: u32, data: &[u8]) -> Result<(), CodecError> {
        let ret =
            unsafe { sys::TIFFSetField(self.handle, tag, data.len() as c_int, data.as_ptr()) };
        if ret == 1 {
            Ok(())
        } else {
            let error_msg = take_last_error()
                .unwrap_or_else(|| format!("Failed to set TIFF tag {} as u8 array", tag));
            Err(CodecError::Encode(error_msg))
        }
    }

    /// Write an `f32` (FLOAT) array tag to the current IFD.
    pub fn set_field_f32_array(&self, tag: u32, data: &[f32]) -> Result<(), CodecError> {
        let ret =
            unsafe { sys::TIFFSetField(self.handle, tag, data.len() as c_int, data.as_ptr()) };
        if ret == 1 {
            Ok(())
        } else {
            let error_msg = take_last_error()
                .unwrap_or_else(|| format!("Failed to set TIFF tag {} as f32 array", tag));
            Err(CodecError::Encode(error_msg))
        }
    }

    /// Write an `i16` (SSHORT) array tag to the current IFD.
    pub fn set_field_i16_array(&self, tag: u32, data: &[i16]) -> Result<(), CodecError> {
        let ret =
            unsafe { sys::TIFFSetField(self.handle, tag, data.len() as c_int, data.as_ptr()) };
        if ret == 1 {
            Ok(())
        } else {
            let error_msg = take_last_error()
                .unwrap_or_else(|| format!("Failed to set TIFF tag {} as i16 array", tag));
            Err(CodecError::Encode(error_msg))
        }
    }

    /// Write an `i32` (SLONG) array tag to the current IFD.
    pub fn set_field_i32_array(&self, tag: u32, data: &[i32]) -> Result<(), CodecError> {
        let ret =
            unsafe { sys::TIFFSetField(self.handle, tag, data.len() as c_int, data.as_ptr()) };
        if ret == 1 {
            Ok(())
        } else {
            let error_msg = take_last_error()
                .unwrap_or_else(|| format!("Failed to set TIFF tag {} as i32 array", tag));
            Err(CodecError::Encode(error_msg))
        }
    }

    // =========================================================================
    // Array Tag Access (GeoTIFF support)
    // =========================================================================

    /// Read a tile/strip offset or byte-count array from the current IFD.
    ///
    /// libtiff normalizes TileOffsets (324), TileByteCounts (325),
    /// StripOffsets (273), and StripByteCounts (279) to `u64` internally,
    /// regardless of classic TIFF (LONG) vs BigTIFF (LONG8). The call
    /// signature is `TIFFGetField(tif, tag, &ptr)` — no count parameter;
    /// the caller must know the count from `TIFFNumberOfTiles()` or
    /// `TIFFNumberOfStrips()`.
    pub fn get_field_u64_ptr(&self, tag: u32, count: u32) -> Result<Vec<u64>, CodecError> {
        let mut ptr: *const u64 = ptr::null();
        let ret = unsafe { sys::TIFFGetField(self.handle, tag, &mut ptr as *mut *const u64) };
        if ret == 1 && !ptr.is_null() {
            let slice = unsafe { std::slice::from_raw_parts(ptr, count as usize) };
            Ok(slice.to_vec())
        } else {
            Err(CodecError::Decode(format!(
                "TIFF tag {} not found or not a u64 array",
                tag
            )))
        }
    }

    /// Read a SHORT array tag from the current IFD.
    ///
    /// libtiff returns variable-length SHORT arrays (e.g., GeoKeyDirectoryTag 34735)
    /// via `TIFFGetField(tif, tag, &count, &ptr)` where count is `u16` and ptr
    /// points to libtiff-owned memory. We copy into a `Vec` before returning.
    pub fn get_field_u16_array(&self, tag: u32, _count: u16) -> Result<Vec<u16>, CodecError> {
        let mut actual_count: u16 = 0;
        let mut ptr: *const u16 = ptr::null();
        let ret = unsafe {
            sys::TIFFGetField(
                self.handle,
                tag,
                &mut actual_count as *mut u16,
                &mut ptr as *mut *const u16,
            )
        };
        if ret == 1 && !ptr.is_null() {
            let len = actual_count as usize;
            let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
            Ok(slice.to_vec())
        } else {
            Err(CodecError::Decode(format!(
                "TIFF tag {} not found or not a u16 array",
                tag
            )))
        }
    }

    /// Read a BYTE/UNDEFINED array tag from the current IFD.
    ///
    /// libtiff returns variable-length byte arrays (e.g., JPEGTables tag 347)
    /// via `TIFFGetField(tif, tag, &count, &ptr)` where count is `u32` and ptr
    /// points to libtiff-owned memory. We copy into a `Vec<u8>` before returning.
    pub fn get_field_u8_array(&self, tag: u32) -> Result<Vec<u8>, CodecError> {
        let mut count: u32 = 0;
        let mut ptr: *const u8 = ptr::null();
        let ret = unsafe {
            sys::TIFFGetField(
                self.handle,
                tag,
                &mut count as *mut u32,
                &mut ptr as *mut *const u8,
            )
        };
        if ret == 1 && !ptr.is_null() {
            let len = count as usize;
            let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
            Ok(slice.to_vec())
        } else {
            Err(CodecError::Decode(format!(
                "TIFF tag {} not found or not a u8 array",
                tag
            )))
        }
    }

    /// Read a DOUBLE array tag from the current IFD.
    ///
    /// libtiff returns variable-length DOUBLE arrays (e.g., GeoDoubleParamsTag 34736,
    /// ModelTiepointTag 33922) via `TIFFGetField(tif, tag, &count, &ptr)` where
    /// count is `u16` and ptr points to libtiff-owned memory.
    pub fn get_field_f64_array(&self, tag: u32, _count: u16) -> Result<Vec<f64>, CodecError> {
        let mut actual_count: u16 = 0;
        let mut ptr: *const f64 = ptr::null();
        let ret = unsafe {
            sys::TIFFGetField(
                self.handle,
                tag,
                &mut actual_count as *mut u16,
                &mut ptr as *mut *const f64,
            )
        };
        if ret == 1 && !ptr.is_null() {
            let len = actual_count as usize;
            let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
            Ok(slice.to_vec())
        } else {
            Err(CodecError::Decode(format!(
                "TIFF tag {} not found or not a f64 array",
                tag
            )))
        }
    }

    /// Write a SHORT array tag to the current IFD.
    ///
    /// Uses `TIFFSetField(tif, tag, count, ptr)` with count+pointer semantics.
    pub fn set_field_u16_array(&self, tag: u32, data: &[u16]) -> Result<(), CodecError> {
        let ret =
            unsafe { sys::TIFFSetField(self.handle, tag, data.len() as c_int, data.as_ptr()) };
        if ret == 1 {
            Ok(())
        } else {
            let error_msg = take_last_error()
                .unwrap_or_else(|| format!("Failed to set TIFF tag {} as u16 array", tag));
            Err(CodecError::Encode(error_msg))
        }
    }

    /// Write a DOUBLE array tag to the current IFD.
    ///
    /// Uses `TIFFSetField(tif, tag, count, ptr)` with count+pointer semantics.
    pub fn set_field_f64_array(&self, tag: u32, data: &[f64]) -> Result<(), CodecError> {
        let ret =
            unsafe { sys::TIFFSetField(self.handle, tag, data.len() as c_int, data.as_ptr()) };
        if ret == 1 {
            Ok(())
        } else {
            let error_msg = take_last_error()
                .unwrap_or_else(|| format!("Failed to set TIFF tag {} as f64 array", tag));
            Err(CodecError::Encode(error_msg))
        }
    }

    /// Write an ASCII string tag to the current IFD.
    ///
    /// Uses `TIFFSetField(tif, tag, cstring_ptr)` for string tags like
    /// GeoAsciiParamsTag (34737).
    pub fn set_field_string(&self, tag: u32, value: &str) -> Result<(), CodecError> {
        let cstr = CString::new(value).map_err(|e| {
            CodecError::Encode(format!("Invalid string for TIFF tag {}: {}", tag, e))
        })?;
        let ret = unsafe { sys::TIFFSetField(self.handle, tag, cstr.as_ptr()) };
        if ret == 1 {
            Ok(())
        } else {
            let error_msg = take_last_error()
                .unwrap_or_else(|| format!("Failed to set TIFF tag {} as string", tag));
            Err(CodecError::Encode(error_msg))
        }
    }

    /// Compress and write a tile of data.
    ///
    /// Returns `CodecError::Io` if `TIFFWriteEncodedTile` fails.
    pub fn write_encoded_tile(&self, tile_index: u32, data: &[u8]) -> Result<(), CodecError> {
        let bytes_written = unsafe {
            sys::TIFFWriteEncodedTile(
                self.handle,
                tile_index,
                data.as_ptr() as *mut c_void,
                data.len() as i64,
            )
        };

        if bytes_written < 0 {
            let error_msg =
                take_last_error().unwrap_or_else(|| format!("Failed to write tile {}", tile_index));
            return Err(CodecError::Io(std::io::Error::other(error_msg)));
        }

        Ok(())
    }

    /// Register a custom (non-standard) tag with libtiff so it can be written.
    ///
    /// libtiff rejects `TIFFSetField` calls for tags it doesn't know about.
    /// This method calls `TIFFMergeFieldInfo` to register a single custom tag
    /// with the given TIFF field type. The tag name is generated as "Tag{tag}".
    ///
    /// The `field_type` parameter is the TIFF 6.0 field type ID (1–12).
    ///
    /// When `scalar` is true, the tag is registered with `pass_count=0` and
    /// `count=1`, meaning `TIFFSetField` expects a single value (no count
    /// prefix). When false, it is registered with `pass_count=1` and
    /// `count=-1`, meaning `TIFFSetField` expects `(count, pointer)`.
    ///
    /// This distinction matters for DOUBLE and FLOAT custom tags: libtiff
    /// segfaults if a scalar value is passed to a tag registered as an array
    /// (pass_count=1) because it interprets the scalar bits as a pointer.
    pub fn register_custom_tag(
        &self,
        tag: u32,
        field_type: u16,
        scalar: bool,
    ) -> Result<(), CodecError> {
        // Map TIFF field type to libtiff data type constant
        let data_type: u32 = match field_type {
            1 => sys::TIFF_BYTE,
            2 => sys::TIFF_ASCII,
            3 => sys::TIFF_SHORT,
            4 => sys::TIFF_LONG,
            5 => sys::TIFF_RATIONAL,
            6 => sys::TIFF_SBYTE,
            7 => sys::TIFF_UNDEFINED,
            8 => sys::TIFF_SSHORT,
            9 => sys::TIFF_SLONG,
            10 => sys::TIFF_SRATIONAL,
            11 => sys::TIFF_FLOAT,
            12 => sys::TIFF_DOUBLE,
            _ => {
                return Err(CodecError::Encode(format!(
                    "Invalid TIFF field type {} for tag {}",
                    field_type, tag
                )));
            }
        };

        // Determine read/write counts and pass_count based on how our writer
        // calls TIFFSetField for each type:
        // - ASCII: pass_count=0, count=-1 (string pointer, no count prefix)
        // - Scalar types (LONG, SLONG, scalar DOUBLE/FLOAT):
        //   pass_count=0, count=1 (scalar TIFFSetField)
        // - Array types (BYTE, UNDEFINED, SHORT, SSHORT, array DOUBLE, etc.):
        //   pass_count=1, count=-1 (count+pointer TIFFSetField)
        let (rw_count, pass_count): (i16, u8) = match field_type {
            2 => (-1, 0),          // ASCII
            _ if scalar => (1, 0), // any scalar type
            _ => (-1, 1),          // arrays: BYTE, SHORT, SSHORT, DOUBLE, UNDEFINED, etc.
        };

        // Leak a CString for the tag name — libtiff requires the name pointer
        // to outlive the handle. This is a small, bounded leak per custom tag.
        let name = CString::new(format!("Tag{}", tag)).unwrap();
        let name_ptr = name.into_raw() as *const c_char;

        // Allocate the field info array (1 element) on the heap and leak it.
        // libtiff stores a pointer to the TIFFFieldInfo array internally, so
        // it must remain valid for the lifetime of the handle.
        let info_array: Box<[sys::TIFFFieldInfo; 1]> = Box::new([sys::TIFFFieldInfo {
            tag,
            read_count: rw_count,
            write_count: rw_count,
            data_type,
            field_bit: sys::FIELD_CUSTOM,
            ok_to_change: 1,
            pass_count,
            name: name_ptr,
        }]);
        let info_ptr = Box::into_raw(info_array) as *const sys::TIFFFieldInfo;

        let ret = unsafe { sys::TIFFMergeFieldInfo(self.handle, info_ptr, 1) };

        if ret != 0 {
            return Err(CodecError::Encode(format!(
                "Failed to register custom tag {}",
                tag
            )));
        }

        Ok(())
    }

    /// Write the current directory and prepare for a new one (multi-IFD support).
    ///
    /// Returns `CodecError::Encode` if `TIFFWriteDirectory` fails.
    pub fn write_directory(&self) -> Result<(), CodecError> {
        let ret = unsafe { sys::TIFFWriteDirectory(self.handle) };
        if ret == 1 {
            Ok(())
        } else {
            let error_msg =
                take_last_error().unwrap_or_else(|| "Failed to write TIFF directory".to_string());
            Err(CodecError::Encode(error_msg))
        }
    }

    /// Consume the handle, calling `TIFFClose`, and return the assembled TIFF bytes.
    ///
    /// This method takes ownership of the `TiffHandle` so that `Drop` does not
    /// double-close the libtiff handle.
    ///
    /// Returns `CodecError::Encode` if the handle was not opened in write mode.
    pub fn into_bytes(mut self) -> Result<Vec<u8>, CodecError> {
        // Close the libtiff handle to flush any pending data
        if !self.handle.is_null() {
            unsafe {
                sys::TIFFClose(self.handle);
            }
            self.handle = ptr::null_mut();
        }

        // Extract the buffer from the write stream data
        match self._stream_data {
            StreamData::Write(ref mut write_data) => Ok(std::mem::take(&mut write_data.buffer)),
            StreamData::Read(_) => Err(CodecError::Encode(
                "into_bytes() called on a read-mode TiffHandle".to_string(),
            )),
        }
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
        assert!(
            handle.is_ok(),
            "from_bytes should succeed for valid TIFF data"
        );
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
        let photo = handle
            .get_field_u16(tags::PHOTOMETRIC_INTERPRETATION)
            .unwrap();
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

    /// Helper: create a minimal tiled TIFF via the write path with optional
    /// GeoTIFF array tags, then return the assembled bytes.
    fn make_tiff_with_array_tags(
        u16_array: Option<(u32, &[u16])>,
        f64_array: Option<(u32, &[f64])>,
        string_tag: Option<(u32, &str)>,
    ) -> Vec<u8> {
        let handle = TiffHandle::from_write(false).unwrap();

        // Minimal 1×1 grayscale tiled image
        handle.set_field_u32(tags::IMAGE_WIDTH, 1).unwrap();
        handle.set_field_u32(tags::IMAGE_LENGTH, 1).unwrap();
        handle.set_field_u16(tags::BITS_PER_SAMPLE, 8).unwrap();
        handle.set_field_u16(tags::SAMPLES_PER_PIXEL, 1).unwrap();
        handle
            .set_field_u16(tags::SAMPLE_FORMAT, tags::SAMPLE_FORMAT_UINT)
            .unwrap();
        handle
            .set_field_u16(
                tags::PHOTOMETRIC_INTERPRETATION,
                tags::PHOTOMETRIC_MINISBLACK,
            )
            .unwrap();
        handle.set_field_u32(tags::TILE_WIDTH, 16).unwrap();
        handle.set_field_u32(tags::TILE_LENGTH, 16).unwrap();
        handle
            .set_field_u16(tags::COMPRESSION, tags::COMPRESSION_NONE)
            .unwrap();
        handle
            .set_field_u16(tags::PLANAR_CONFIGURATION, tags::PLANAR_CONFIG_CONTIG)
            .unwrap();

        // Set the array tags under test
        if let Some((tag, data)) = u16_array {
            handle.set_field_u16_array(tag, data).unwrap();
        }
        if let Some((tag, data)) = f64_array {
            handle.set_field_f64_array(tag, data).unwrap();
        }
        if let Some((tag, value)) = string_tag {
            handle.set_field_string(tag, value).unwrap();
        }

        // Write one tile of pixel data (16×16 = 256 bytes for the padded tile)
        let tile_data = vec![0u8; 16 * 16];
        handle.write_encoded_tile(0, &tile_data).unwrap();
        handle.write_directory().unwrap();

        handle.into_bytes().unwrap()
    }

    #[test]
    fn test_u16_array_write_read_roundtrip() {
        // Use GeoKeyDirectoryTag (34735) — a real GeoTIFF SHORT array tag
        let input: Vec<u16> = vec![1, 1, 0, 2, 1024, 0, 1, 1, 3072, 0, 1, 32618];
        let bytes =
            make_tiff_with_array_tags(Some((tags::GEO_KEY_DIRECTORY_TAG, &input)), None, None);

        let reader = TiffHandle::from_bytes(&bytes).unwrap();
        let result = reader
            .get_field_u16_array(tags::GEO_KEY_DIRECTORY_TAG, 0)
            .unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_f64_array_write_read_roundtrip() {
        // Use ModelPixelScaleTag (33550) — a real GeoTIFF DOUBLE array tag
        let input: Vec<f64> = vec![0.5, 0.5, 0.0];
        let bytes =
            make_tiff_with_array_tags(None, Some((tags::MODEL_PIXEL_SCALE_TAG, &input)), None);

        let reader = TiffHandle::from_bytes(&bytes).unwrap();
        let result = reader
            .get_field_f64_array(tags::MODEL_PIXEL_SCALE_TAG, 0)
            .unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_string_tag_write_read_roundtrip() {
        // Use GeoAsciiParamsTag (34737) — a real GeoTIFF ASCII tag
        let input = "WGS 84|";
        let bytes =
            make_tiff_with_array_tags(None, None, Some((tags::GEO_ASCII_PARAMS_TAG, input)));

        let reader = TiffHandle::from_bytes(&bytes).unwrap();
        let result = reader.get_field_string(tags::GEO_ASCII_PARAMS_TAG).unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_missing_u16_array_returns_error() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();

        let result = handle.get_field_u16_array(tags::GEO_KEY_DIRECTORY_TAG, 0);
        assert!(result.is_err());
        match result.unwrap_err() {
            CodecError::Decode(msg) => {
                assert!(
                    msg.contains("not found") || msg.contains("not a u16 array"),
                    "Unexpected error: {}",
                    msg
                );
            }
            other => panic!("Expected CodecError::Decode, got: {:?}", other),
        }
    }

    #[test]
    fn test_missing_f64_array_returns_error() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();

        let result = handle.get_field_f64_array(tags::MODEL_PIXEL_SCALE_TAG, 0);
        assert!(result.is_err());
        match result.unwrap_err() {
            CodecError::Decode(msg) => {
                assert!(
                    msg.contains("not found") || msg.contains("not a f64 array"),
                    "Unexpected error: {}",
                    msg
                );
            }
            other => panic!("Expected CodecError::Decode, got: {:?}", other),
        }
    }

    // =========================================================================
    // IFD Enumeration Tests
    // =========================================================================

    #[test]
    fn test_enumerate_ifd_tags_minimal_tiff() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();

        // Our minimal TIFF has 9 tags
        assert_eq!(entries.len(), 9);

        // Verify known tags are present with correct field types
        let find = |tag: u32| entries.iter().find(|e| e.tag == tag);

        // ImageWidth (256) — SHORT (3), count 1
        let iw = find(256).expect("ImageWidth not found");
        assert_eq!(iw.field_type, 3); // SHORT
        assert_eq!(iw.count, 1);

        // ImageLength (257) — SHORT (3), count 1
        let il = find(257).expect("ImageLength not found");
        assert_eq!(il.field_type, 3);
        assert_eq!(il.count, 1);

        // Compression (259) — SHORT (3), count 1
        let comp = find(259).expect("Compression not found");
        assert_eq!(comp.field_type, 3);
        assert_eq!(comp.count, 1);

        // StripOffsets (273) — LONG (4), count 1
        let so = find(273).expect("StripOffsets not found");
        assert_eq!(so.field_type, 4);
        assert_eq!(so.count, 1);
    }

    #[test]
    fn test_enumerate_ifd_tags_real_tiff_file() {
        let data = std::fs::read("data/unit/tiff-256x256-1band-8bit-tiled-deflate.tif").unwrap();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();

        // A real TIFF should have at least the basic required tags
        assert!(!entries.is_empty(), "Real TIFF should have tags");

        // ImageWidth (256) must be present
        assert!(
            entries.iter().any(|e| e.tag == 256),
            "ImageWidth tag not found in real TIFF"
        );
        // ImageLength (257) must be present
        assert!(
            entries.iter().any(|e| e.tag == 257),
            "ImageLength tag not found in real TIFF"
        );
    }

    // =========================================================================
    // read_tag_value Tests — All 12 TIFF Field Types
    // =========================================================================

    /// Build a TIFF with a custom tag entry for testing read_tag_value.
    /// Creates a minimal valid TIFF with the standard 9 tags plus one extra
    /// tag with the given field type, count, and raw value bytes.
    fn make_tiff_with_custom_tag(
        custom_tag: u16,
        field_type: u16,
        count: u32,
        value_bytes: &[u8],
    ) -> Vec<u8> {
        let mut buf = Vec::new();

        // TIFF Header — little-endian
        buf.extend_from_slice(&[0x49, 0x49]); // "II"
        buf.extend_from_slice(&42u16.to_le_bytes());
        buf.extend_from_slice(&8u32.to_le_bytes()); // IFD at offset 8

        // IFD: 10 entries (9 standard + 1 custom)
        // Tags MUST be sorted ascending by tag number per TIFF spec.
        let num_entries: u16 = 10;
        buf.extend_from_slice(&num_entries.to_le_bytes());

        let short_type: u16 = 3;
        let long_type: u16 = 4;
        let width: u32 = 4;
        let height: u32 = 2;

        // Calculate offsets:
        // IFD starts at 8, entries: 2 + 10*12 = 122 bytes, next_ifd: 4 bytes
        // So pixel data starts at 8 + 2 + 120 + 4 = 134
        let pixel_data_offset: u32 = 8 + 2 + (num_entries as u32 * 12) + 4;
        let strip_bytes: u32 = width * height; // 8 bytes of pixel data

        // After pixel data, we place the custom tag's overflow data (if any)
        let custom_data_offset: u32 = pixel_data_offset + strip_bytes;

        // Helper to write a 12-byte IFD entry
        let write_entry = |b: &mut Vec<u8>, tag: u16, typ: u16, cnt: u32, val: u32| {
            b.extend_from_slice(&tag.to_le_bytes());
            b.extend_from_slice(&typ.to_le_bytes());
            b.extend_from_slice(&cnt.to_le_bytes());
            b.extend_from_slice(&val.to_le_bytes());
        };

        // We need to insert the custom tag in sorted order.
        // Standard tags: 256,257,258,259,262,273,277,278,279
        // We'll use a tag number that sorts correctly.
        struct TagDef {
            tag: u16,
            typ: u16,
            count: u32,
            value: u32,
        }

        let type_size = match field_type {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 => 4,
            5 | 10 | 12 => 8,
            _ => 1,
        };
        let total_bytes = count as usize * type_size;
        let custom_value = if total_bytes <= 4 {
            // Inline: pack value_bytes into a u32
            let mut v = [0u8; 4];
            let copy_len = value_bytes.len().min(4);
            v[..copy_len].copy_from_slice(&value_bytes[..copy_len]);
            u32::from_le_bytes(v)
        } else {
            custom_data_offset
        };

        let mut all_tags = vec![
            TagDef {
                tag: 256,
                typ: short_type,
                count: 1,
                value: width,
            },
            TagDef {
                tag: 257,
                typ: short_type,
                count: 1,
                value: height,
            },
            TagDef {
                tag: 258,
                typ: short_type,
                count: 1,
                value: 8,
            },
            TagDef {
                tag: 259,
                typ: short_type,
                count: 1,
                value: 1,
            },
            TagDef {
                tag: 262,
                typ: short_type,
                count: 1,
                value: 1,
            },
            TagDef {
                tag: 273,
                typ: long_type,
                count: 1,
                value: pixel_data_offset,
            },
            TagDef {
                tag: 277,
                typ: short_type,
                count: 1,
                value: 1,
            },
            TagDef {
                tag: 278,
                typ: short_type,
                count: 1,
                value: height,
            },
            TagDef {
                tag: 279,
                typ: long_type,
                count: 1,
                value: strip_bytes,
            },
            TagDef {
                tag: custom_tag,
                typ: field_type,
                count,
                value: custom_value,
            },
        ];
        all_tags.sort_by_key(|t| t.tag);

        for td in &all_tags {
            write_entry(&mut buf, td.tag, td.typ, td.count, td.value);
        }

        // Next IFD offset: 0
        buf.extend_from_slice(&0u32.to_le_bytes());

        // Pixel data
        assert_eq!(buf.len(), pixel_data_offset as usize);
        buf.extend_from_slice(&[10, 20, 30, 40, 50, 60, 70, 80]);

        // Custom tag overflow data (if total_bytes > 4)
        if total_bytes > 4 {
            assert_eq!(buf.len(), custom_data_offset as usize);
            buf.extend_from_slice(value_bytes);
        }

        buf
    }

    #[test]
    fn test_read_tag_value_byte() {
        // BYTE (type 1), count 1, value = 42
        let data = make_tiff_with_custom_tag(700, 1, 1, &[42, 0, 0, 0]);
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries.iter().find(|e| e.tag == 700).unwrap();
        let val = handle.read_tag_value(entry).unwrap();
        assert_eq!(val, serde_json::json!(42));
    }

    #[test]
    fn test_read_tag_value_byte_array() {
        // BYTE (type 1), count 3, inline (fits in 4 bytes)
        let data = make_tiff_with_custom_tag(700, 1, 3, &[10, 20, 30, 0]);
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries.iter().find(|e| e.tag == 700).unwrap();
        let val = handle.read_tag_value(entry).unwrap();
        assert_eq!(val, serde_json::json!([10, 20, 30]));
    }

    #[test]
    fn test_read_tag_value_ascii() {
        // ASCII (type 2), "Hi\0" = 3 bytes, inline
        let data = make_tiff_with_custom_tag(700, 2, 3, b"Hi\0\0");
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries.iter().find(|e| e.tag == 700).unwrap();
        let val = handle.read_tag_value(entry).unwrap();
        assert_eq!(val, serde_json::json!("Hi"));
    }

    #[test]
    fn test_read_tag_value_short() {
        // SHORT (type 3), count 1, value = 1000
        let mut vb = [0u8; 4];
        vb[..2].copy_from_slice(&1000u16.to_le_bytes());
        let data = make_tiff_with_custom_tag(700, 3, 1, &vb);
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries.iter().find(|e| e.tag == 700).unwrap();
        let val = handle.read_tag_value(entry).unwrap();
        assert_eq!(val, serde_json::json!(1000));
    }

    #[test]
    fn test_read_tag_value_short_array() {
        // SHORT (type 3), count 2, inline (4 bytes)
        let mut vb = [0u8; 4];
        vb[..2].copy_from_slice(&100u16.to_le_bytes());
        vb[2..4].copy_from_slice(&200u16.to_le_bytes());
        let data = make_tiff_with_custom_tag(700, 3, 2, &vb);
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries.iter().find(|e| e.tag == 700).unwrap();
        let val = handle.read_tag_value(entry).unwrap();
        assert_eq!(val, serde_json::json!([100, 200]));
    }

    #[test]
    fn test_read_tag_value_long() {
        // LONG (type 4), count 1, value = 70000
        let data = make_tiff_with_custom_tag(700, 4, 1, &70000u32.to_le_bytes());
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries.iter().find(|e| e.tag == 700).unwrap();
        let val = handle.read_tag_value(entry).unwrap();
        assert_eq!(val, serde_json::json!(70000));
    }

    #[test]
    fn test_read_tag_value_rational() {
        // RATIONAL (type 5), count 1: num=3, den=2 → 1.5
        let mut vb = Vec::new();
        vb.extend_from_slice(&3u32.to_le_bytes());
        vb.extend_from_slice(&2u32.to_le_bytes());
        let data = make_tiff_with_custom_tag(700, 5, 1, &vb);
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries.iter().find(|e| e.tag == 700).unwrap();
        let val = handle.read_tag_value(entry).unwrap();
        assert_eq!(val, serde_json::json!(1.5));
    }

    #[test]
    fn test_read_tag_value_sbyte() {
        // SBYTE (type 6), count 1, value = -10 (0xF6)
        let data = make_tiff_with_custom_tag(700, 6, 1, &[0xF6, 0, 0, 0]);
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries.iter().find(|e| e.tag == 700).unwrap();
        let val = handle.read_tag_value(entry).unwrap();
        assert_eq!(val, serde_json::json!(-10));
    }

    #[test]
    fn test_read_tag_value_undefined() {
        // UNDEFINED (type 7), count 3, always array
        let data = make_tiff_with_custom_tag(700, 7, 3, &[0xDE, 0xAD, 0xBE, 0]);
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries.iter().find(|e| e.tag == 700).unwrap();
        let val = handle.read_tag_value(entry).unwrap();
        assert_eq!(val, serde_json::json!([0xDE, 0xAD, 0xBE]));
    }

    #[test]
    fn test_read_tag_value_undefined_count_1() {
        // UNDEFINED (type 7), count 1 — still returns array per spec
        let data = make_tiff_with_custom_tag(700, 7, 1, &[0xFF, 0, 0, 0]);
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries.iter().find(|e| e.tag == 700).unwrap();
        let val = handle.read_tag_value(entry).unwrap();
        assert_eq!(val, serde_json::json!([0xFF]));
    }

    #[test]
    fn test_read_tag_value_sshort() {
        // SSHORT (type 8), count 1, value = -500
        let mut vb = [0u8; 4];
        vb[..2].copy_from_slice(&(-500i16).to_le_bytes());
        let data = make_tiff_with_custom_tag(700, 8, 1, &vb);
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries.iter().find(|e| e.tag == 700).unwrap();
        let val = handle.read_tag_value(entry).unwrap();
        assert_eq!(val, serde_json::json!(-500));
    }

    #[test]
    fn test_read_tag_value_slong() {
        // SLONG (type 9), count 1, value = -100000
        let data = make_tiff_with_custom_tag(700, 9, 1, &(-100000i32).to_le_bytes());
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries.iter().find(|e| e.tag == 700).unwrap();
        let val = handle.read_tag_value(entry).unwrap();
        assert_eq!(val, serde_json::json!(-100000));
    }

    #[test]
    fn test_read_tag_value_srational() {
        // SRATIONAL (type 10), count 1: num=-7, den=2 → -3.5
        let mut vb = Vec::new();
        vb.extend_from_slice(&(-7i32).to_le_bytes());
        vb.extend_from_slice(&2i32.to_le_bytes());
        let data = make_tiff_with_custom_tag(700, 10, 1, &vb);
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries.iter().find(|e| e.tag == 700).unwrap();
        let val = handle.read_tag_value(entry).unwrap();
        assert_eq!(val, serde_json::json!(-3.5));
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_read_tag_value_float() {
        // FLOAT (type 11), count 1, value = 3.14
        let data = make_tiff_with_custom_tag(700, 11, 1, &3.14f32.to_le_bytes());
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries.iter().find(|e| e.tag == 700).unwrap();
        let val = handle.read_tag_value(entry).unwrap();
        let f = val.as_f64().unwrap();
        assert!((f - 3.14).abs() < 0.001, "Expected ~3.14, got {}", f);
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_read_tag_value_double() {
        // DOUBLE (type 12), count 1, value = 2.718281828
        let vb = 2.718281828f64.to_le_bytes().to_vec();
        let data = make_tiff_with_custom_tag(700, 12, 1, &vb);
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries.iter().find(|e| e.tag == 700).unwrap();
        let val = handle.read_tag_value(entry).unwrap();
        let f = val.as_f64().unwrap();
        assert!((f - 2.718281828).abs() < 1e-9, "Expected ~2.718, got {}", f);
    }

    #[test]
    fn test_read_tag_value_double_array() {
        // DOUBLE (type 12), count 2 — overflow data
        let mut vb = Vec::new();
        vb.extend_from_slice(&1.5f64.to_le_bytes());
        vb.extend_from_slice(&2.5f64.to_le_bytes());
        let data = make_tiff_with_custom_tag(700, 12, 2, &vb);
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries.iter().find(|e| e.tag == 700).unwrap();
        let val = handle.read_tag_value(entry).unwrap();
        assert_eq!(val, serde_json::json!([1.5, 2.5]));
    }

    // =========================================================================
    // Error Case Tests
    // =========================================================================

    #[test]
    fn test_read_tag_value_unknown_field_type() {
        // Field type 99 — should return Err
        let data = make_tiff_with_custom_tag(700, 99, 1, &[0, 0, 0, 0]);
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries.iter().find(|e| e.tag == 700).unwrap();
        assert_eq!(entry.field_type, 99);
        let result = handle.read_tag_value(entry);
        assert!(result.is_err());
        match result.unwrap_err() {
            CodecError::Decode(msg) => {
                assert!(msg.contains("Unknown TIFF field type"), "Error: {}", msg);
            }
            other => panic!("Expected Decode error, got: {:?}", other),
        }
    }

    #[test]
    fn test_read_tag_value_rational_zero_denominator() {
        // RATIONAL with denominator 0 → NaN
        let mut vb = Vec::new();
        vb.extend_from_slice(&1u32.to_le_bytes());
        vb.extend_from_slice(&0u32.to_le_bytes());
        let data = make_tiff_with_custom_tag(700, 5, 1, &vb);
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries.iter().find(|e| e.tag == 700).unwrap();
        let val = handle.read_tag_value(entry).unwrap();
        // NaN is represented as null in JSON
        assert!(val.is_null(), "Expected null for NaN, got {:?}", val);
    }

    // =========================================================================
    // Big-Endian Test
    // =========================================================================

    #[test]
    fn test_enumerate_ifd_tags_big_endian() {
        // Build a minimal big-endian TIFF
        let mut buf = Vec::new();

        // Header: big-endian ("MM")
        buf.extend_from_slice(&[0x4D, 0x4D]);
        buf.extend_from_slice(&42u16.to_be_bytes());
        buf.extend_from_slice(&8u32.to_be_bytes());

        // IFD with 9 entries
        let num_entries: u16 = 9;
        buf.extend_from_slice(&num_entries.to_be_bytes());

        let short_type: u16 = 3;
        let long_type: u16 = 4;
        let width: u32 = 4;
        let height: u32 = 2;
        let pixel_data_offset: u32 = 8 + 2 + (num_entries as u32 * 12) + 4;
        let strip_bytes: u32 = width * height;

        // In big-endian TIFF, SHORT values (count=1) are left-justified in the
        // 4-byte value field: value occupies bytes 0-1, bytes 2-3 are padding.
        let write_be_entry = |b: &mut Vec<u8>, tag: u16, typ: u16, cnt: u32, val: u32| {
            b.extend_from_slice(&tag.to_be_bytes());
            b.extend_from_slice(&typ.to_be_bytes());
            b.extend_from_slice(&cnt.to_be_bytes());
            if typ == short_type && cnt == 1 {
                // Left-justify: SHORT value in first 2 bytes, zero-pad last 2
                b.extend_from_slice(&(val as u16).to_be_bytes());
                b.extend_from_slice(&[0, 0]);
            } else {
                b.extend_from_slice(&val.to_be_bytes());
            }
        };

        write_be_entry(&mut buf, 256, short_type, 1, width);
        write_be_entry(&mut buf, 257, short_type, 1, height);
        write_be_entry(&mut buf, 258, short_type, 1, 8);
        write_be_entry(&mut buf, 259, short_type, 1, 1);
        write_be_entry(&mut buf, 262, short_type, 1, 1);
        write_be_entry(&mut buf, 273, long_type, 1, pixel_data_offset);
        write_be_entry(&mut buf, 277, short_type, 1, 1);
        write_be_entry(&mut buf, 278, short_type, 1, height);
        write_be_entry(&mut buf, 279, long_type, 1, strip_bytes);

        buf.extend_from_slice(&0u32.to_be_bytes()); // next IFD
        buf.extend_from_slice(&[10, 20, 30, 40, 50, 60, 70, 80]); // pixel data

        let handle = TiffHandle::from_bytes(&buf).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        assert_eq!(entries.len(), 9);

        let iw = entries.iter().find(|e| e.tag == 256).unwrap();
        assert_eq!(iw.field_type, 3);
        assert_eq!(iw.count, 1);

        // Also test read_tag_value on big-endian
        let val = handle.read_tag_value(iw).unwrap();
        assert_eq!(val, serde_json::json!(4));
    }

    // =========================================================================
    // Write/Read Roundtrip Tests via libtiff
    // =========================================================================

    #[test]
    fn test_roundtrip_u16_array_via_libtiff() {
        // Write SHORT array via libtiff, read back via enumerate + read_tag_value
        let input: Vec<u16> = vec![1, 1, 0, 2, 1024, 0, 1, 1, 3072, 0, 1, 32618];
        let bytes =
            make_tiff_with_array_tags(Some((tags::GEO_KEY_DIRECTORY_TAG, &input)), None, None);
        let handle = TiffHandle::from_bytes(&bytes).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries
            .iter()
            .find(|e| e.tag == tags::GEO_KEY_DIRECTORY_TAG)
            .unwrap();
        assert_eq!(entry.field_type, 3); // SHORT
        let val = handle.read_tag_value(entry).unwrap();
        let arr: Vec<u64> = val
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        let expected: Vec<u64> = input.iter().map(|&v| v as u64).collect();
        assert_eq!(arr, expected);
    }

    #[test]
    fn test_roundtrip_f64_array_via_libtiff() {
        // Write DOUBLE array via libtiff, read back via enumerate + read_tag_value
        let input: Vec<f64> = vec![0.5, 0.5, 0.0];
        let bytes =
            make_tiff_with_array_tags(None, Some((tags::MODEL_PIXEL_SCALE_TAG, &input)), None);
        let handle = TiffHandle::from_bytes(&bytes).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries
            .iter()
            .find(|e| e.tag == tags::MODEL_PIXEL_SCALE_TAG)
            .unwrap();
        assert_eq!(entry.field_type, 12); // DOUBLE
        let val = handle.read_tag_value(entry).unwrap();
        let arr: Vec<f64> = val
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        assert_eq!(arr, input);
    }

    #[test]
    fn test_roundtrip_string_via_libtiff() {
        // Write ASCII string via libtiff, read back via enumerate + read_tag_value
        let input = "WGS 84|";
        let bytes =
            make_tiff_with_array_tags(None, None, Some((tags::GEO_ASCII_PARAMS_TAG, input)));
        let handle = TiffHandle::from_bytes(&bytes).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();
        let entry = entries
            .iter()
            .find(|e| e.tag == tags::GEO_ASCII_PARAMS_TAG)
            .unwrap();
        assert_eq!(entry.field_type, 2); // ASCII
        let val = handle.read_tag_value(entry).unwrap();
        assert_eq!(val.as_str().unwrap(), input);
    }

    // =========================================================================
    // get_field_u8_array Tests
    // =========================================================================

    /// Helper: create a JPEG-compressed tiled TIFF with a known JPEGTables tag
    /// via the write path, then return the assembled bytes.
    fn make_jpeg_tiff_with_tables(jpeg_tables: &[u8]) -> Vec<u8> {
        let handle = TiffHandle::from_write(false).unwrap();

        // Minimal 16×16 RGB tiled image with JPEG compression
        handle.set_field_u32(tags::IMAGE_WIDTH, 16).unwrap();
        handle.set_field_u32(tags::IMAGE_LENGTH, 16).unwrap();
        handle.set_field_u16(tags::BITS_PER_SAMPLE, 8).unwrap();
        handle.set_field_u16(tags::SAMPLES_PER_PIXEL, 3).unwrap();
        handle
            .set_field_u16(tags::SAMPLE_FORMAT, tags::SAMPLE_FORMAT_UINT)
            .unwrap();
        handle
            .set_field_u16(tags::PHOTOMETRIC_INTERPRETATION, tags::PHOTOMETRIC_YCBCR)
            .unwrap();
        handle.set_field_u32(tags::TILE_WIDTH, 16).unwrap();
        handle.set_field_u32(tags::TILE_LENGTH, 16).unwrap();
        handle
            .set_field_u16(tags::COMPRESSION, tags::COMPRESSION_JPEG)
            .unwrap();
        handle
            .set_field_u16(tags::PLANAR_CONFIGURATION, tags::PLANAR_CONFIG_CONTIG)
            .unwrap();

        // Set the JPEGTables tag with our known bytes
        handle
            .set_field_u8_array(tags::JPEG_TABLES, jpeg_tables)
            .unwrap();

        // Write one tile of pixel data (16×16×3 = 768 bytes)
        let tile_data = vec![128u8; 16 * 16 * 3];
        handle.write_encoded_tile(0, &tile_data).unwrap();
        handle.write_directory().unwrap();

        handle.into_bytes().unwrap()
    }

    #[test]
    fn test_get_field_u8_array_jpeg_tables() {
        // Write a JPEG-compressed TIFF with known JPEGTables, read them back
        // Note: libtiff may modify the JPEGTables during JPEG setup, so we
        // read back whatever libtiff stored and verify it's a non-empty byte array.
        // The key property: get_field_u8_array returns Ok with bytes for a present tag.
        let input_tables: Vec<u8> = vec![0xFF, 0xD8, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0xFF, 0xD9];
        let bytes = make_jpeg_tiff_with_tables(&input_tables);

        let reader = TiffHandle::from_bytes(&bytes).unwrap();
        let result = reader.get_field_u8_array(tags::JPEG_TABLES);
        assert!(
            result.is_ok(),
            "get_field_u8_array should succeed for JPEGTables: {:?}",
            result.err()
        );

        let tables = result.unwrap();
        // libtiff generates its own JPEGTables during JPEG encoding, so the
        // returned bytes will be valid JPEG tables (starting with SOI marker
        // 0xFF 0xD8 and ending with EOI marker 0xFF 0xD9).
        assert!(!tables.is_empty(), "JPEGTables should not be empty");
        assert_eq!(tables[0], 0xFF, "JPEGTables should start with 0xFF");
        assert_eq!(tables[1], 0xD8, "JPEGTables should start with SOI marker");
        let len = tables.len();
        assert_eq!(tables[len - 2], 0xFF, "JPEGTables should end with 0xFF");
        assert_eq!(
            tables[len - 1],
            0xD9,
            "JPEGTables should end with EOI marker"
        );
    }

    #[test]
    fn test_get_field_u8_array_absent_tag_returns_error() {
        // Use a minimal stripped TIFF that has no JPEGTables tag
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();

        let result = handle.get_field_u8_array(tags::JPEG_TABLES);
        assert!(
            result.is_err(),
            "get_field_u8_array should fail for absent tag"
        );
        match result.unwrap_err() {
            CodecError::Decode(msg) => {
                assert!(
                    msg.contains("not found") || msg.contains("not a u8 array"),
                    "Unexpected error message: {}",
                    msg
                );
            }
            other => panic!("Expected CodecError::Decode, got: {:?}", other),
        }
    }

    // =========================================================================
    // BigTIFF Tests
    // =========================================================================

    /// Build a minimal BigTIFF byte buffer (little-endian): 4x2 grayscale, uncompressed.
    ///
    /// BigTIFF header layout (16 bytes):
    ///   0-1: byte order "II"
    ///   2-3: version 43 (0x2B)
    ///   4-5: offset size = 8
    ///   6-7: reserved = 0
    ///   8-15: first IFD offset (u64)
    ///
    /// BigTIFF IFD layout:
    ///   entry count: u64
    ///   each entry: 20 bytes (tag u16, type u16, count u64, value/offset u64)
    ///   next IFD: u64
    fn make_minimal_bigtiff_le() -> Vec<u8> {
        let mut buf = Vec::new();

        // Header (16 bytes)
        buf.extend_from_slice(&[0x49, 0x49]); // LE
        buf.extend_from_slice(&43u16.to_le_bytes()); // Version 43
        buf.extend_from_slice(&8u16.to_le_bytes()); // Offset size
        buf.extend_from_slice(&0u16.to_le_bytes()); // Reserved
        buf.extend_from_slice(&16u64.to_le_bytes()); // First IFD at offset 16

        // IFD at offset 16
        let num_entries: u64 = 9;
        buf.extend_from_slice(&num_entries.to_le_bytes()); // entry count (u64)

        // Calculate pixel data offset:
        // header(16) + entry_count(8) + entries(9*20) + next_ifd(8)
        let pixel_data_offset: u64 = 16 + 8 + (num_entries * 20) + 8;

        let width: u64 = 4;
        let height: u64 = 2;

        // Helper: write a 20-byte BigTIFF IFD entry
        let write_entry = |b: &mut Vec<u8>, tag: u16, typ: u16, count: u64, value: u64| {
            b.extend_from_slice(&tag.to_le_bytes());
            b.extend_from_slice(&typ.to_le_bytes());
            b.extend_from_slice(&count.to_le_bytes());
            b.extend_from_slice(&value.to_le_bytes());
        };

        // Entries (sorted by tag)
        write_entry(&mut buf, 256, 3, 1, width); // ImageWidth (SHORT)
        write_entry(&mut buf, 257, 3, 1, height); // ImageLength (SHORT)
        write_entry(&mut buf, 258, 3, 1, 8); // BitsPerSample
        write_entry(&mut buf, 259, 3, 1, 1); // Compression=None
        write_entry(&mut buf, 262, 3, 1, 1); // PhotometricInterpretation
        write_entry(&mut buf, 273, 16, 1, pixel_data_offset); // StripOffsets (LONG8)
        write_entry(&mut buf, 277, 3, 1, 1); // SamplesPerPixel
        write_entry(&mut buf, 278, 3, 1, height); // RowsPerStrip
        write_entry(&mut buf, 279, 16, 1, width * height); // StripByteCounts (LONG8)

        // Next IFD offset: 0 (no more IFDs)
        buf.extend_from_slice(&0u64.to_le_bytes());

        // Pixel data
        assert_eq!(buf.len(), pixel_data_offset as usize);
        buf.extend_from_slice(&[10, 20, 30, 40, 50, 60, 70, 80]);

        buf
    }

    /// Build a minimal BigTIFF byte buffer (big-endian): 4x2 grayscale, uncompressed.
    ///
    /// In BE BigTIFF, inline values are left-justified in the 8-byte value field:
    /// a SHORT value of 4 is stored as [0x00, 0x04, 0, 0, 0, 0, 0, 0].
    fn make_minimal_bigtiff_be() -> Vec<u8> {
        let mut buf = Vec::new();

        // Header (16 bytes)
        buf.extend_from_slice(&[0x4D, 0x4D]); // BE
        buf.extend_from_slice(&43u16.to_be_bytes()); // Version 43
        buf.extend_from_slice(&8u16.to_be_bytes()); // Offset size
        buf.extend_from_slice(&0u16.to_be_bytes()); // Reserved
        buf.extend_from_slice(&16u64.to_be_bytes()); // First IFD at offset 16

        // IFD at offset 16
        let num_entries: u64 = 9;
        buf.extend_from_slice(&num_entries.to_be_bytes());

        let pixel_data_offset: u64 = 16 + 8 + (num_entries * 20) + 8;
        let width: u16 = 4;
        let height: u16 = 2;

        // BE BigTIFF entry: inline values are left-justified in the 8-byte field
        let write_entry_short = |b: &mut Vec<u8>, tag: u16, value: u16| {
            b.extend_from_slice(&tag.to_be_bytes());
            b.extend_from_slice(&3u16.to_be_bytes()); // SHORT
            b.extend_from_slice(&1u64.to_be_bytes()); // count
                                                      // Left-justify: SHORT in first 2 bytes, zero-pad remaining 6
            b.extend_from_slice(&value.to_be_bytes());
            b.extend(std::iter::repeat_n(0u8, 6));
        };

        let write_entry_long8 = |b: &mut Vec<u8>, tag: u16, value: u64| {
            b.extend_from_slice(&tag.to_be_bytes());
            b.extend_from_slice(&16u16.to_be_bytes()); // LONG8
            b.extend_from_slice(&1u64.to_be_bytes()); // count
            b.extend_from_slice(&value.to_be_bytes()); // full 8 bytes
        };

        write_entry_short(&mut buf, 256, width);
        write_entry_short(&mut buf, 257, height);
        write_entry_short(&mut buf, 258, 8);
        write_entry_short(&mut buf, 259, 1);
        write_entry_short(&mut buf, 262, 1);
        write_entry_long8(&mut buf, 273, pixel_data_offset);
        write_entry_short(&mut buf, 277, 1);
        write_entry_short(&mut buf, 278, height);
        write_entry_long8(&mut buf, 279, (width as u64) * (height as u64));

        buf.extend_from_slice(&0u64.to_be_bytes());

        assert_eq!(buf.len(), pixel_data_offset as usize);
        buf.extend_from_slice(&[10, 20, 30, 40, 50, 60, 70, 80]);

        buf
    }

    #[test]
    fn test_bigtiff_le_magic_accepted() {
        let data = make_minimal_bigtiff_le();
        let handle = TiffHandle::from_bytes(&data);
        assert!(handle.is_ok(), "BigTIFF LE should open: {:?}", handle.err());
        let handle = handle.unwrap();
        assert!(handle.is_bigtiff());
    }

    #[test]
    fn test_bigtiff_be_magic_accepted() {
        let data = make_minimal_bigtiff_be();
        let handle = TiffHandle::from_bytes(&data);
        assert!(handle.is_ok(), "BigTIFF BE should open: {:?}", handle.err());
        let handle = handle.unwrap();
        assert!(handle.is_bigtiff());
    }

    #[test]
    fn test_classic_tiff_is_not_bigtiff() {
        let data = make_minimal_tiff();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        assert!(!handle.is_bigtiff());
    }

    #[test]
    fn test_bigtiff_le_enumerate_ifd_tags() {
        let data = make_minimal_bigtiff_le();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();

        assert_eq!(entries.len(), 9);

        let find = |tag: u32| entries.iter().find(|e| e.tag == tag);

        let iw = find(256).expect("ImageWidth not found");
        assert_eq!(iw.field_type, 3); // SHORT
        assert_eq!(iw.count, 1);

        let so = find(273).expect("StripOffsets not found");
        assert_eq!(so.field_type, 16); // LONG8
        assert_eq!(so.count, 1);

        let sbc = find(279).expect("StripByteCounts not found");
        assert_eq!(sbc.field_type, 16); // LONG8
        assert_eq!(sbc.count, 1);
    }

    #[test]
    fn test_bigtiff_be_enumerate_ifd_tags() {
        let data = make_minimal_bigtiff_be();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();

        assert_eq!(entries.len(), 9);

        let iw = entries.iter().find(|e| e.tag == 256).unwrap();
        assert_eq!(iw.field_type, 3);
        assert_eq!(iw.count, 1);
    }

    #[test]
    fn test_bigtiff_le_read_tag_value_short() {
        let data = make_minimal_bigtiff_le();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();

        let iw = entries.iter().find(|e| e.tag == 256).unwrap();
        let val = handle.read_tag_value(iw).unwrap();
        assert_eq!(val, serde_json::json!(4)); // ImageWidth = 4
    }

    #[test]
    fn test_bigtiff_le_read_tag_value_long8() {
        let data = make_minimal_bigtiff_le();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();

        // StripByteCounts (LONG8) = 8 (4*2 pixels)
        let sbc = entries.iter().find(|e| e.tag == 279).unwrap();
        let val = handle.read_tag_value(sbc).unwrap();
        assert_eq!(val, serde_json::json!(8));
    }

    #[test]
    fn test_bigtiff_be_read_tag_value() {
        let data = make_minimal_bigtiff_be();
        let handle = TiffHandle::from_bytes(&data).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();

        let iw = entries.iter().find(|e| e.tag == 256).unwrap();
        let val = handle.read_tag_value(iw).unwrap();
        assert_eq!(val, serde_json::json!(4));

        let sbc = entries.iter().find(|e| e.tag == 279).unwrap();
        let val = handle.read_tag_value(sbc).unwrap();
        assert_eq!(val, serde_json::json!(8));
    }

    #[test]
    fn test_bigtiff_ifd_entry_count_cap() {
        // Test IfdReader directly: craft a BigTIFF buffer with entry count > 4096
        let mut data = make_minimal_bigtiff_le();
        // Overwrite entry count at offset 16 with 5000
        let count_bytes = 5000u64.to_le_bytes();
        data[16..24].copy_from_slice(&count_bytes);

        // Use IfdReader directly to verify capping (libtiff rejects large counts)
        let reader = IfdReader::new(&data, true).unwrap();
        let entries = reader.enumerate_entries(16).unwrap();
        // IfdReader caps at 4096 entries but only parses what fits in the buffer
        assert!(entries.len() <= MAX_IFD_ENTRIES as usize);
    }

    #[test]
    fn test_bigtiff_roundtrip_via_from_write() {
        // Write a BigTIFF via from_write(true), read it back
        let handle = TiffHandle::from_write(true).unwrap();

        handle.set_field_u32(tags::IMAGE_WIDTH, 4).unwrap();
        handle.set_field_u32(tags::IMAGE_LENGTH, 2).unwrap();
        handle.set_field_u16(tags::BITS_PER_SAMPLE, 8).unwrap();
        handle.set_field_u16(tags::SAMPLES_PER_PIXEL, 1).unwrap();
        handle
            .set_field_u16(tags::SAMPLE_FORMAT, tags::SAMPLE_FORMAT_UINT)
            .unwrap();
        handle
            .set_field_u16(
                tags::PHOTOMETRIC_INTERPRETATION,
                tags::PHOTOMETRIC_MINISBLACK,
            )
            .unwrap();
        handle.set_field_u32(tags::TILE_WIDTH, 16).unwrap();
        handle.set_field_u32(tags::TILE_LENGTH, 16).unwrap();
        handle
            .set_field_u16(tags::COMPRESSION, tags::COMPRESSION_NONE)
            .unwrap();
        handle
            .set_field_u16(tags::PLANAR_CONFIGURATION, tags::PLANAR_CONFIG_CONTIG)
            .unwrap();

        let tile_data = vec![42u8; 16 * 16];
        handle.write_encoded_tile(0, &tile_data).unwrap();
        handle.write_directory().unwrap();

        let bytes = handle.into_bytes().unwrap();

        // Verify header magic
        assert_eq!(bytes[0], 0x49); // 'I'
        assert_eq!(bytes[1], 0x49); // 'I'
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), 43); // version 43

        // Read it back
        let reader = TiffHandle::from_bytes(&bytes).unwrap();
        assert!(reader.is_bigtiff());
        assert_eq!(reader.number_of_directories(), 1);

        let entries = reader.enumerate_ifd_tags().unwrap();
        let iw = entries.iter().find(|e| e.tag == 256).unwrap();
        let val = reader.read_tag_value(iw).unwrap();
        assert_eq!(val, serde_json::json!(4));
    }

    #[test]
    fn test_bigtiff_field_type_13_ifd() {
        // Build a BigTIFF with a tag of type 13 (IFD - u32 sub-IFD pointer)
        let mut buf = Vec::new();

        // Header
        buf.extend_from_slice(&[0x49, 0x49]);
        buf.extend_from_slice(&43u16.to_le_bytes());
        buf.extend_from_slice(&8u16.to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes());
        buf.extend_from_slice(&16u64.to_le_bytes());

        let num_entries: u64 = 10;
        buf.extend_from_slice(&num_entries.to_le_bytes());

        let pixel_data_offset: u64 = 16 + 8 + (num_entries * 20) + 8;
        let width: u64 = 4;
        let height: u64 = 2;

        let write_entry = |b: &mut Vec<u8>, tag: u16, typ: u16, count: u64, value: u64| {
            b.extend_from_slice(&tag.to_le_bytes());
            b.extend_from_slice(&typ.to_le_bytes());
            b.extend_from_slice(&count.to_le_bytes());
            b.extend_from_slice(&value.to_le_bytes());
        };

        write_entry(&mut buf, 256, 3, 1, width);
        write_entry(&mut buf, 257, 3, 1, height);
        write_entry(&mut buf, 258, 3, 1, 8);
        write_entry(&mut buf, 259, 3, 1, 1);
        write_entry(&mut buf, 262, 3, 1, 1);
        write_entry(&mut buf, 273, 16, 1, pixel_data_offset);
        write_entry(&mut buf, 277, 3, 1, 1);
        write_entry(&mut buf, 278, 3, 1, height);
        write_entry(&mut buf, 279, 16, 1, width * height);
        // SubIFDs tag (330) with type 13 (IFD), value = 0 (dummy pointer)
        write_entry(&mut buf, 330, 13, 1, 0);

        buf.extend_from_slice(&0u64.to_le_bytes());

        assert_eq!(buf.len(), pixel_data_offset as usize);
        buf.extend_from_slice(&[10, 20, 30, 40, 50, 60, 70, 80]);

        let handle = TiffHandle::from_bytes(&buf).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();

        let sub_ifd = entries.iter().find(|e| e.tag == 330).unwrap();
        assert_eq!(sub_ifd.field_type, 13);
        assert_eq!(sub_ifd.count, 1);

        let val = handle.read_tag_value(sub_ifd).unwrap();
        assert_eq!(val, serde_json::json!(0));
    }

    #[test]
    fn test_bigtiff_field_type_17_slong8() {
        // Build a BigTIFF with a SLONG8 tag (type 17)
        let mut buf = Vec::new();

        buf.extend_from_slice(&[0x49, 0x49]);
        buf.extend_from_slice(&43u16.to_le_bytes());
        buf.extend_from_slice(&8u16.to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes());
        buf.extend_from_slice(&16u64.to_le_bytes());

        let num_entries: u64 = 10;
        buf.extend_from_slice(&num_entries.to_le_bytes());

        let pixel_data_offset: u64 = 16 + 8 + (num_entries * 20) + 8;
        let width: u64 = 4;
        let height: u64 = 2;

        let write_entry = |b: &mut Vec<u8>, tag: u16, typ: u16, count: u64, value: u64| {
            b.extend_from_slice(&tag.to_le_bytes());
            b.extend_from_slice(&typ.to_le_bytes());
            b.extend_from_slice(&count.to_le_bytes());
            b.extend_from_slice(&value.to_le_bytes());
        };

        write_entry(&mut buf, 256, 3, 1, width);
        write_entry(&mut buf, 257, 3, 1, height);
        write_entry(&mut buf, 258, 3, 1, 8);
        write_entry(&mut buf, 259, 3, 1, 1);
        write_entry(&mut buf, 262, 3, 1, 1);
        write_entry(&mut buf, 273, 16, 1, pixel_data_offset);
        write_entry(&mut buf, 277, 3, 1, 1);
        write_entry(&mut buf, 278, 3, 1, height);
        write_entry(&mut buf, 279, 16, 1, width * height);
        // Custom tag 700 with type 17 (SLONG8), value = -42 (as i64 bitcast to u64)
        write_entry(&mut buf, 700, 17, 1, (-42i64) as u64);

        buf.extend_from_slice(&0u64.to_le_bytes());

        assert_eq!(buf.len(), pixel_data_offset as usize);
        buf.extend_from_slice(&[10, 20, 30, 40, 50, 60, 70, 80]);

        let handle = TiffHandle::from_bytes(&buf).unwrap();
        let entries = handle.enumerate_ifd_tags().unwrap();

        let custom = entries.iter().find(|e| e.tag == 700).unwrap();
        assert_eq!(custom.field_type, 17);

        let val = handle.read_tag_value(custom).unwrap();
        assert_eq!(val, serde_json::json!(-42));
    }
}
