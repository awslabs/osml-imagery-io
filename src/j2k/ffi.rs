//! Safe Rust wrappers for OpenJPEG FFI.
//!
//! This module provides safe abstractions over the raw OpenJPEG FFI bindings,
//! handling memory management, error handling, and type conversions.
//!
//! These wrappers are used internally by the `OpenJpegCodec` implementation.

use std::cell::RefCell;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

use crate::error::CodecError;

use super::sys::{
    self, opj_codec_t, opj_cparameters_t, opj_dparameters_t, opj_image_cmptparm_t, opj_image_t,
    opj_stream_t, OPJ_CLRSPC_GRAY, OPJ_CLRSPC_SRGB, OPJ_CLRSPC_UNSPECIFIED, OPJ_CODEC_J2K,
    OPJ_FALSE, OPJ_TRUE,
};

// =============================================================================
// Message Handler
// =============================================================================

// Thread-local storage for capturing OpenJPEG messages
thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
    static LAST_WARNING: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Callback for OpenJPEG error messages
unsafe extern "C" fn error_callback(msg: *const c_char, _client_data: *mut c_void) {
    if !msg.is_null() {
        if let Ok(s) = CStr::from_ptr(msg).to_str() {
            let trimmed = s.trim().to_string();
            LAST_ERROR.with(|e| {
                *e.borrow_mut() = Some(trimmed);
            });
        }
    }
}

/// Callback for OpenJPEG warning messages
unsafe extern "C" fn warning_callback(msg: *const c_char, _client_data: *mut c_void) {
    if !msg.is_null() {
        if let Ok(s) = CStr::from_ptr(msg).to_str() {
            let trimmed = s.trim().to_string();
            LAST_WARNING.with(|w| {
                *w.borrow_mut() = Some(trimmed);
            });
        }
    }
}

/// Callback for OpenJPEG info messages (ignored)
unsafe extern "C" fn info_callback(_msg: *const c_char, _client_data: *mut c_void) {
    // Info messages are ignored
}

/// Get and clear the last error message
pub(super) fn take_last_error() -> Option<String> {
    LAST_ERROR.with(|e| e.borrow_mut().take())
}

/// Get and clear the last warning message
#[allow(dead_code)]
pub(super) fn take_last_warning() -> Option<String> {
    LAST_WARNING.with(|w| w.borrow_mut().take())
}

// =============================================================================
// Memory Stream Adapters
// =============================================================================

/// User data for memory read stream
struct MemoryReadStreamData {
    data: *const u8,
    len: usize,
    pos: usize,
}

/// Read callback for memory stream
unsafe extern "C" fn memory_read_callback(
    p_buffer: *mut c_void,
    p_nb_bytes: usize,
    p_user_data: *mut c_void,
) -> usize {
    if p_user_data.is_null() || p_buffer.is_null() {
        return usize::MAX; // Error indicator
    }

    let stream_data = &mut *(p_user_data as *mut MemoryReadStreamData);
    let remaining = stream_data.len.saturating_sub(stream_data.pos);
    let to_read = p_nb_bytes.min(remaining);

    if to_read == 0 {
        return usize::MAX; // EOF
    }

    ptr::copy_nonoverlapping(
        stream_data.data.add(stream_data.pos),
        p_buffer as *mut u8,
        to_read,
    );
    stream_data.pos += to_read;

    to_read
}

/// Skip callback for memory stream
unsafe extern "C" fn memory_skip_callback(p_nb_bytes: i64, p_user_data: *mut c_void) -> i64 {
    if p_user_data.is_null() {
        return -1;
    }

    let stream_data = &mut *(p_user_data as *mut MemoryReadStreamData);

    if p_nb_bytes < 0 {
        // Backward skip
        let skip = (-p_nb_bytes) as usize;
        if skip > stream_data.pos {
            stream_data.pos = 0;
        } else {
            stream_data.pos -= skip;
        }
    } else {
        // Forward skip
        let skip = p_nb_bytes as usize;
        let new_pos = stream_data.pos.saturating_add(skip);
        stream_data.pos = new_pos.min(stream_data.len);
    }

    p_nb_bytes
}

/// Seek callback for memory stream
unsafe extern "C" fn memory_seek_callback(p_nb_bytes: i64, p_user_data: *mut c_void) -> c_int {
    if p_user_data.is_null() || p_nb_bytes < 0 {
        return OPJ_FALSE;
    }

    let stream_data = &mut *(p_user_data as *mut MemoryReadStreamData);
    let new_pos = p_nb_bytes as usize;

    if new_pos > stream_data.len {
        return OPJ_FALSE;
    }

    stream_data.pos = new_pos;
    OPJ_TRUE
}

/// Free callback for memory read stream
unsafe extern "C" fn memory_read_free_callback(p_user_data: *mut c_void) {
    if !p_user_data.is_null() {
        drop(Box::from_raw(p_user_data as *mut MemoryReadStreamData));
    }
}

/// User data for memory write stream
struct MemoryWriteStreamData {
    buffer: Vec<u8>,
    pos: usize,
}

/// Write callback for memory stream
unsafe extern "C" fn memory_write_callback(
    p_buffer: *mut c_void,
    p_nb_bytes: usize,
    p_user_data: *mut c_void,
) -> usize {
    if p_user_data.is_null() || p_buffer.is_null() {
        return usize::MAX;
    }

    let stream_data = &mut *(p_user_data as *mut MemoryWriteStreamData);
    let src = std::slice::from_raw_parts(p_buffer as *const u8, p_nb_bytes);

    // Ensure buffer is large enough
    let required_len = stream_data.pos + p_nb_bytes;
    if required_len > stream_data.buffer.len() {
        stream_data.buffer.resize(required_len, 0);
    }

    // Copy data
    stream_data.buffer[stream_data.pos..stream_data.pos + p_nb_bytes].copy_from_slice(src);
    stream_data.pos += p_nb_bytes;

    p_nb_bytes
}

/// Skip callback for write stream
unsafe extern "C" fn memory_write_skip_callback(p_nb_bytes: i64, p_user_data: *mut c_void) -> i64 {
    if p_user_data.is_null() {
        return -1;
    }

    let stream_data = &mut *(p_user_data as *mut MemoryWriteStreamData);

    if p_nb_bytes < 0 {
        let skip = (-p_nb_bytes) as usize;
        if skip > stream_data.pos {
            stream_data.pos = 0;
        } else {
            stream_data.pos -= skip;
        }
    } else {
        let skip = p_nb_bytes as usize;
        stream_data.pos = stream_data.pos.saturating_add(skip);
        // Extend buffer if needed
        if stream_data.pos > stream_data.buffer.len() {
            stream_data.buffer.resize(stream_data.pos, 0);
        }
    }

    p_nb_bytes
}

/// Seek callback for write stream
unsafe extern "C" fn memory_write_seek_callback(p_nb_bytes: i64, p_user_data: *mut c_void) -> c_int {
    if p_user_data.is_null() || p_nb_bytes < 0 {
        return OPJ_FALSE;
    }

    let stream_data = &mut *(p_user_data as *mut MemoryWriteStreamData);
    let new_pos = p_nb_bytes as usize;

    // Extend buffer if needed
    if new_pos > stream_data.buffer.len() {
        stream_data.buffer.resize(new_pos, 0);
    }

    stream_data.pos = new_pos;
    OPJ_TRUE
}

/// Free callback for memory write stream (does not free - we extract the data)
unsafe extern "C" fn memory_write_free_callback(_p_user_data: *mut c_void) {
    // Don't free - the data will be extracted by finalize()
}

// =============================================================================
// Safe Wrapper Types
// =============================================================================

/// Safe wrapper for OpenJPEG codec handle.
pub struct OjpCodec {
    ptr: *mut opj_codec_t,
}

impl OjpCodec {
    /// Create a new decompression codec.
    pub fn new_decompress() -> Result<Self, CodecError> {
        let ptr = unsafe { sys::opj_create_decompress(OPJ_CODEC_J2K) };
        if ptr.is_null() {
            return Err(CodecError::Decode("Failed to create OpenJPEG decoder".into()));
        }

        // Set up message handlers
        unsafe {
            sys::opj_set_error_handler(ptr, Some(error_callback), ptr::null_mut());
            sys::opj_set_warning_handler(ptr, Some(warning_callback), ptr::null_mut());
            sys::opj_set_info_handler(ptr, Some(info_callback), ptr::null_mut());
        }

        Ok(Self { ptr })
    }

    /// Create a new compression codec.
    pub fn new_compress() -> Result<Self, CodecError> {
        let ptr = unsafe { sys::opj_create_compress(OPJ_CODEC_J2K) };
        if ptr.is_null() {
            return Err(CodecError::Encode("Failed to create OpenJPEG encoder".into()));
        }

        // Set up message handlers
        unsafe {
            sys::opj_set_error_handler(ptr, Some(error_callback), ptr::null_mut());
            sys::opj_set_warning_handler(ptr, Some(warning_callback), ptr::null_mut());
            sys::opj_set_info_handler(ptr, Some(info_callback), ptr::null_mut());
        }

        Ok(Self { ptr })
    }

    /// Set the number of threads for encoding/decoding.
    pub fn set_threads(&self, num_threads: usize) -> Result<(), CodecError> {
        let result = unsafe { sys::opj_codec_set_threads(self.ptr, num_threads as c_int) };
        if result == OPJ_FALSE {
            return Err(CodecError::Decode("Failed to set thread count".into()));
        }
        Ok(())
    }

    /// Setup decoder with parameters.
    pub fn setup_decoder(&self, params: &mut opj_dparameters_t) -> Result<(), CodecError> {
        let result = unsafe { sys::opj_setup_decoder(self.ptr, params) };
        if result == OPJ_FALSE {
            let msg = take_last_error().unwrap_or_else(|| "Unknown error".into());
            return Err(CodecError::Decode(format!("Failed to setup decoder: {}", msg)));
        }
        Ok(())
    }

    /// Setup encoder with parameters and image.
    pub fn setup_encoder(
        &self,
        params: &mut opj_cparameters_t,
        image: &OjpImage,
    ) -> Result<(), CodecError> {
        let result = unsafe { sys::opj_setup_encoder(self.ptr, params, image.ptr) };
        if result == OPJ_FALSE {
            let msg = take_last_error().unwrap_or_else(|| "Unknown error".into());
            return Err(CodecError::Encode(format!("Failed to setup encoder: {}", msg)));
        }
        Ok(())
    }

    /// Set extra encoder options.
    ///
    /// This must be called after `setup_encoder()` and before `start_compress()`.
    /// Supported options (OpenJPEG 2.4.0+):
    /// - `TLM=YES` - Write TLM (Tile-part Length Marker) segments
    /// - `PLT=YES` - Write PLT (Packet Length) marker segments
    ///
    /// # Arguments
    /// * `options` - Slice of option strings in "KEY=VALUE" format
    pub fn set_extra_options(&self, options: &[&str]) -> Result<(), CodecError> {
        use std::ffi::CString;
        
        // Convert options to CStrings
        let c_options: Vec<CString> = options
            .iter()
            .map(|s| CString::new(*s).expect("Option string contains null byte"))
            .collect();
        
        // Create array of pointers (null-terminated)
        let mut option_ptrs: Vec<*const c_char> = c_options
            .iter()
            .map(|s| s.as_ptr())
            .collect();
        option_ptrs.push(ptr::null()); // Null terminator
        
        let result = unsafe {
            sys::opj_encoder_set_extra_options(self.ptr, option_ptrs.as_ptr())
        };
        
        if result == OPJ_FALSE {
            let msg = take_last_error().unwrap_or_else(|| "Unknown error".into());
            return Err(CodecError::Encode(format!("Failed to set extra options: {}", msg)));
        }
        Ok(())
    }

    /// Read header from stream.
    pub fn read_header(&self, stream: &OjpStream) -> Result<OjpImage, CodecError> {
        let mut image_ptr: *mut opj_image_t = ptr::null_mut();
        let result = unsafe { sys::opj_read_header(stream.ptr, self.ptr, &mut image_ptr) };
        if result == OPJ_FALSE || image_ptr.is_null() {
            let msg = take_last_error().unwrap_or_else(|| "Unknown error".into());
            return Err(CodecError::Decode(format!("Failed to read header: {}", msg)));
        }
        Ok(OjpImage { ptr: image_ptr })
    }

    /// Set decoded resolution factor.
    pub fn set_decoded_resolution_factor(&self, factor: u32) -> Result<(), CodecError> {
        let result = unsafe { sys::opj_set_decoded_resolution_factor(self.ptr, factor) };
        if result == OPJ_FALSE {
            let msg = take_last_error().unwrap_or_else(|| "Unknown error".into());
            return Err(CodecError::Decode(format!(
                "Failed to set resolution factor: {}",
                msg
            )));
        }
        Ok(())
    }

    /// Decode image from stream.
    pub fn decode(&self, stream: &OjpStream, image: &OjpImage) -> Result<(), CodecError> {
        let result = unsafe { sys::opj_decode(self.ptr, stream.ptr, image.ptr) };
        if result == OPJ_FALSE {
            let msg = take_last_error().unwrap_or_else(|| "Unknown error".into());
            return Err(CodecError::Decode(format!("Failed to decode: {}", msg)));
        }
        Ok(())
    }

    /// End decompression.
    pub fn end_decompress(&self, stream: &OjpStream) -> Result<(), CodecError> {
        let result = unsafe { sys::opj_end_decompress(self.ptr, stream.ptr) };
        if result == OPJ_FALSE {
            let msg = take_last_error().unwrap_or_else(|| "Unknown error".into());
            return Err(CodecError::Decode(format!(
                "Failed to end decompress: {}",
                msg
            )));
        }
        Ok(())
    }

    /// Start compression.
    pub fn start_compress(&self, image: &OjpImage, stream: &OjpStream) -> Result<(), CodecError> {
        let result = unsafe { sys::opj_start_compress(self.ptr, image.ptr, stream.ptr) };
        if result == OPJ_FALSE {
            let msg = take_last_error().unwrap_or_else(|| "Unknown error".into());
            return Err(CodecError::Encode(format!(
                "Failed to start compress: {}",
                msg
            )));
        }
        Ok(())
    }

    /// Encode image.
    pub fn encode(&self, stream: &OjpStream) -> Result<(), CodecError> {
        let result = unsafe { sys::opj_encode(self.ptr, stream.ptr) };
        if result == OPJ_FALSE {
            let msg = take_last_error().unwrap_or_else(|| "Unknown error".into());
            return Err(CodecError::Encode(format!("Failed to encode: {}", msg)));
        }
        Ok(())
    }

    /// End compression.
    pub fn end_compress(&self, stream: &OjpStream) -> Result<(), CodecError> {
        let result = unsafe { sys::opj_end_compress(self.ptr, stream.ptr) };
        if result == OPJ_FALSE {
            let msg = take_last_error().unwrap_or_else(|| "Unknown error".into());
            return Err(CodecError::Encode(format!("Failed to end compress: {}", msg)));
        }
        Ok(())
    }

    /// Write a tile.
    pub fn write_tile(
        &self,
        tile_index: u32,
        data: &[u8],
        stream: &OjpStream,
    ) -> Result<(), CodecError> {
        let result = unsafe {
            sys::opj_write_tile(
                self.ptr,
                tile_index,
                data.as_ptr() as *mut u8,
                data.len() as u32,
                stream.ptr,
            )
        };
        if result == OPJ_FALSE {
            let msg = take_last_error().unwrap_or_else(|| "Unknown error".into());
            return Err(CodecError::Encode(format!(
                "Failed to write tile {}: {}",
                tile_index, msg
            )));
        }
        Ok(())
    }

    /// Decode a specific tile from the codestream.
    ///
    /// This decodes only the specified tile, not the entire image.
    /// The decoded data is written into the image's component buffers.
    ///
    /// # Arguments
    /// * `stream` - The input stream containing the codestream
    /// * `image` - The image to decode into (must have been created from read_header)
    /// * `tile_index` - The index of the tile to decode (row-major order)
    pub fn get_decoded_tile(
        &self,
        stream: &OjpStream,
        image: &OjpImage,
        tile_index: u32,
    ) -> Result<(), CodecError> {
        let result = unsafe {
            sys::opj_get_decoded_tile(self.ptr, stream.ptr, image.ptr, tile_index)
        };
        if result == OPJ_FALSE {
            let msg = take_last_error().unwrap_or_else(|| "Unknown error".into());
            return Err(CodecError::Decode(format!(
                "Failed to decode tile {}: {}",
                tile_index, msg
            )));
        }
        Ok(())
    }
}

impl Drop for OjpCodec {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                sys::opj_destroy_codec(self.ptr);
            }
        }
    }
}

// Safety: OpenJPEG codec can be sent between threads
unsafe impl Send for OjpCodec {}

/// Safe wrapper for OpenJPEG stream handle.
pub struct OjpStream {
    ptr: *mut opj_stream_t,
    /// Keep the user data alive
    _user_data: Option<Box<dyn std::any::Any>>,
}

impl OjpStream {
    /// Create a memory read stream from a byte slice.
    ///
    /// # Safety
    /// The returned stream holds a pointer to the data. The caller must ensure
    /// the data outlives the stream.
    pub fn from_memory_read(data: &[u8]) -> Result<Self, CodecError> {
        let ptr = unsafe { sys::opj_stream_create(sys::OPJ_STREAM_DEFAULT_BUFFER_SIZE, OPJ_TRUE) };
        if ptr.is_null() {
            return Err(CodecError::Decode("Failed to create read stream".into()));
        }

        // Create user data
        let user_data = Box::new(MemoryReadStreamData {
            data: data.as_ptr(),
            len: data.len(),
            pos: 0,
        });
        let user_data_ptr = Box::into_raw(user_data);

        unsafe {
            sys::opj_stream_set_read_function(ptr, Some(memory_read_callback));
            sys::opj_stream_set_skip_function(ptr, Some(memory_skip_callback));
            sys::opj_stream_set_seek_function(ptr, Some(memory_seek_callback));
            sys::opj_stream_set_user_data(
                ptr,
                user_data_ptr as *mut c_void,
                Some(memory_read_free_callback),
            );
            sys::opj_stream_set_user_data_length(ptr, data.len() as u64);
        }

        Ok(Self {
            ptr,
            _user_data: None, // User data is managed by OpenJPEG via free callback
        })
    }

    /// Create a memory write stream.
    pub fn new_memory_write() -> Result<Self, CodecError> {
        let ptr = unsafe { sys::opj_stream_create(sys::OPJ_STREAM_DEFAULT_BUFFER_SIZE, OPJ_FALSE) };
        if ptr.is_null() {
            return Err(CodecError::Encode("Failed to create write stream".into()));
        }

        // Create user data
        let user_data = Box::new(MemoryWriteStreamData {
            buffer: Vec::with_capacity(1024 * 1024), // 1MB initial capacity
            pos: 0,
        });
        let user_data_ptr = Box::into_raw(user_data);

        unsafe {
            sys::opj_stream_set_write_function(ptr, Some(memory_write_callback));
            sys::opj_stream_set_skip_function(ptr, Some(memory_write_skip_callback));
            sys::opj_stream_set_seek_function(ptr, Some(memory_write_seek_callback));
            sys::opj_stream_set_user_data(
                ptr,
                user_data_ptr as *mut c_void,
                Some(memory_write_free_callback),
            );
        }

        Ok(Self {
            ptr,
            _user_data: Some(unsafe { Box::from_raw(user_data_ptr) }),
        })
    }

    /// Extract the written data from a write stream.
    ///
    /// This consumes the stream and returns the written bytes.
    pub fn finalize_write(mut self) -> Result<Vec<u8>, CodecError> {
        if let Some(user_data) = self._user_data.take() {
            if let Ok(stream_data) = user_data.downcast::<MemoryWriteStreamData>() {
                let mut buffer = stream_data.buffer;
                // Truncate to actual written length
                buffer.truncate(stream_data.pos);
                return Ok(buffer);
            }
        }
        Err(CodecError::Encode("Failed to extract written data".into()))
    }
}

impl Drop for OjpStream {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                sys::opj_stream_destroy(self.ptr);
            }
        }
    }
}

// Safety: OpenJPEG stream can be sent between threads
unsafe impl Send for OjpStream {}

/// Safe wrapper for OpenJPEG image handle.
pub struct OjpImage {
    ptr: *mut opj_image_t,
}

impl OjpImage {
    /// Create a new image for encoding.
    pub fn new(
        width: u32,
        height: u32,
        num_components: u32,
        bits_per_component: u8,
        is_signed: bool,
    ) -> Result<Self, CodecError> {
        let mut cmptparms: Vec<opj_image_cmptparm_t> = (0..num_components)
            .map(|_| opj_image_cmptparm_t {
                dx: 1,
                dy: 1,
                w: width,
                h: height,
                x0: 0,
                y0: 0,
                prec: bits_per_component as u32,
                bpp: bits_per_component as u32,
                sgnd: if is_signed { 1 } else { 0 },
            })
            .collect();

        let color_space = match num_components {
            1 => OPJ_CLRSPC_GRAY,
            3 => OPJ_CLRSPC_SRGB,
            _ => OPJ_CLRSPC_UNSPECIFIED,
        };

        let ptr =
            unsafe { sys::opj_image_create(num_components, cmptparms.as_mut_ptr(), color_space) };

        if ptr.is_null() {
            return Err(CodecError::Encode("Failed to create image".into()));
        }

        // Set image dimensions
        unsafe {
            (*ptr).x0 = 0;
            (*ptr).y0 = 0;
            (*ptr).x1 = width;
            (*ptr).y1 = height;
        }

        Ok(Self { ptr })
    }

    /// Create a new image for tile-based encoding.
    pub fn new_tile(
        width: u32,
        height: u32,
        num_components: u32,
        bits_per_component: u8,
        is_signed: bool,
    ) -> Result<Self, CodecError> {
        let mut cmptparms: Vec<opj_image_cmptparm_t> = (0..num_components)
            .map(|_| opj_image_cmptparm_t {
                dx: 1,
                dy: 1,
                w: width,
                h: height,
                x0: 0,
                y0: 0,
                prec: bits_per_component as u32,
                bpp: bits_per_component as u32,
                sgnd: if is_signed { 1 } else { 0 },
            })
            .collect();

        let color_space = match num_components {
            1 => OPJ_CLRSPC_GRAY,
            3 => OPJ_CLRSPC_SRGB,
            _ => OPJ_CLRSPC_UNSPECIFIED,
        };

        let ptr = unsafe {
            sys::opj_image_tile_create(num_components, cmptparms.as_mut_ptr(), color_space)
        };

        if ptr.is_null() {
            return Err(CodecError::Encode("Failed to create tile image".into()));
        }

        // Set image dimensions
        unsafe {
            (*ptr).x0 = 0;
            (*ptr).y0 = 0;
            (*ptr).x1 = width;
            (*ptr).y1 = height;
        }

        Ok(Self { ptr })
    }

    /// Get image width.
    pub fn width(&self) -> u32 {
        unsafe { (*self.ptr).x1 - (*self.ptr).x0 }
    }

    /// Get image height.
    pub fn height(&self) -> u32 {
        unsafe { (*self.ptr).y1 - (*self.ptr).y0 }
    }

    /// Get number of components.
    pub fn num_components(&self) -> u32 {
        unsafe { (*self.ptr).numcomps }
    }

    /// Get component info.
    pub fn component(&self, index: u32) -> Option<ComponentInfo> {
        if index >= self.num_components() {
            return None;
        }
        unsafe {
            let comp = &*(*self.ptr).comps.add(index as usize);
            Some(ComponentInfo {
                width: comp.w,
                height: comp.h,
                precision: comp.prec as u8,
                is_signed: comp.sgnd != 0,
                factor: comp.factor,
            })
        }
    }

    /// Get component data as a slice.
    pub fn component_data(&self, index: u32) -> Option<&[i32]> {
        if index >= self.num_components() {
            return None;
        }
        unsafe {
            let comp = &*(*self.ptr).comps.add(index as usize);
            if comp.data.is_null() {
                return None;
            }
            let len = (comp.w * comp.h) as usize;
            Some(std::slice::from_raw_parts(comp.data, len))
        }
    }

    /// Set component data from a slice.
    pub fn set_component_data(&mut self, index: u32, data: &[i32]) -> Result<(), CodecError> {
        if index >= self.num_components() {
            return Err(CodecError::Encode(format!(
                "Component index {} out of range",
                index
            )));
        }
        unsafe {
            let comp = &mut *(*self.ptr).comps.add(index as usize);
            let expected_len = (comp.w * comp.h) as usize;
            if data.len() != expected_len {
                return Err(CodecError::Encode(format!(
                    "Data length {} doesn't match component size {}",
                    data.len(),
                    expected_len
                )));
            }
            if comp.data.is_null() {
                comp.data = sys::opj_image_data_alloc(expected_len * std::mem::size_of::<i32>())
                    as *mut i32;
                if comp.data.is_null() {
                    return Err(CodecError::Encode("Failed to allocate component data".into()));
                }
            }
            ptr::copy_nonoverlapping(data.as_ptr(), comp.data, expected_len);
        }
        Ok(())
    }
}

impl Drop for OjpImage {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                sys::opj_image_destroy(self.ptr);
            }
        }
    }
}

// Safety: OpenJPEG image can be sent between threads
unsafe impl Send for OjpImage {}

/// Information about an image component.
#[derive(Debug, Clone)]
pub struct ComponentInfo {
    /// Component width
    pub width: u32,
    /// Component height
    pub height: u32,
    /// Precision (bits per sample)
    pub precision: u8,
    /// Whether the component is signed
    pub is_signed: bool,
    /// Resolution reduction factor
    pub factor: u32,
}
