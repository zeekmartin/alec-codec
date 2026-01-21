// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! C/C++ bindings for ALEC compression library
//!
//! This crate provides a C-compatible FFI layer for the ALEC compression
//! library, enabling use from C/C++ firmware and embedded systems.
//!
//! # Safety
//!
//! All functions in this module use raw pointers.
//! Callers must ensure:
//! - Pointers are valid and non-null (unless documented otherwise)
//! - Buffer sizes are accurate
//! - Handles are not used after being freed
//! - Thread safety is managed by the caller

use std::ffi::{c_char, CStr};
use std::path::Path;
use std::slice;

use alec::classifier::Classifier;
use alec::context::Context;
use alec::protocol::RawData;
use alec::{Decoder, Encoder};

/// Result codes for ALEC FFI functions
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlecResult {
    /// Operation completed successfully
    Ok = 0,
    /// Invalid input data provided
    ErrorInvalidInput = 1,
    /// Output buffer is too small
    ErrorBufferTooSmall = 2,
    /// Encoding operation failed
    ErrorEncodingFailed = 3,
    /// Decoding operation failed
    ErrorDecodingFailed = 4,
    /// Null pointer was provided
    ErrorNullPointer = 5,
    /// Invalid UTF-8 string
    ErrorInvalidUtf8 = 6,
    /// File I/O error
    ErrorFileIo = 7,
    /// Context version mismatch
    ErrorVersionMismatch = 8,
}

/// Opaque encoder handle
///
/// Created with `alec_encoder_new()`, freed with `alec_encoder_free()`.
/// Do not access internal fields directly.
pub struct AlecEncoder {
    encoder: Encoder,
    classifier: Classifier,
    context: Context,
}

/// Opaque decoder handle
///
/// Created with `alec_decoder_new()`, freed with `alec_decoder_free()`.
/// Do not access internal fields directly.
pub struct AlecDecoder {
    decoder: Decoder,
    context: Context,
}

// ============================================================================
// Version and Utility Functions
// ============================================================================

/// Get the ALEC library version string
///
/// # Returns
///
/// A null-terminated string containing the version (e.g., "1.0.0").
/// The returned pointer is valid for the lifetime of the program.
///
/// # Example (C)
///
/// ```c
/// printf("ALEC version: %s\n", alec_version());
/// ```
#[no_mangle]
pub extern "C" fn alec_version() -> *const c_char {
    // Include null terminator
    static VERSION: &[u8] = b"1.0.0\0";
    VERSION.as_ptr() as *const c_char
}

/// Convert a result code to a human-readable string
///
/// # Arguments
///
/// * `result` - The result code to convert
///
/// # Returns
///
/// A null-terminated string describing the result.
/// The returned pointer is valid for the lifetime of the program.
#[no_mangle]
pub extern "C" fn alec_result_to_string(result: AlecResult) -> *const c_char {
    let msg: &'static [u8] = match result {
        AlecResult::Ok => b"Success\0",
        AlecResult::ErrorInvalidInput => b"Invalid input data\0",
        AlecResult::ErrorBufferTooSmall => b"Output buffer too small\0",
        AlecResult::ErrorEncodingFailed => b"Encoding failed\0",
        AlecResult::ErrorDecodingFailed => b"Decoding failed\0",
        AlecResult::ErrorNullPointer => b"Null pointer provided\0",
        AlecResult::ErrorInvalidUtf8 => b"Invalid UTF-8 string\0",
        AlecResult::ErrorFileIo => b"File I/O error\0",
        AlecResult::ErrorVersionMismatch => b"Context version mismatch\0",
    };
    msg.as_ptr() as *const c_char
}

// ============================================================================
// Encoder Functions
// ============================================================================

/// Create a new ALEC encoder
///
/// # Returns
///
/// A pointer to a new encoder, or NULL on allocation failure.
/// The encoder must be freed with `alec_encoder_free()` when no longer needed.
///
/// # Example (C)
///
/// ```c
/// AlecEncoder* enc = alec_encoder_new();
/// if (enc == NULL) {
///     // Handle allocation failure
/// }
/// // ... use encoder ...
/// alec_encoder_free(enc);
/// ```
#[no_mangle]
pub extern "C" fn alec_encoder_new() -> *mut AlecEncoder {
    let encoder = Box::new(AlecEncoder {
        encoder: Encoder::new(),
        classifier: Classifier::default(),
        context: Context::new(),
    });
    Box::into_raw(encoder)
}

/// Create a new encoder with checksum enabled
///
/// # Returns
///
/// A pointer to a new encoder with checksum enabled, or NULL on failure.
#[no_mangle]
pub extern "C" fn alec_encoder_new_with_checksum() -> *mut AlecEncoder {
    let encoder = Box::new(AlecEncoder {
        encoder: Encoder::with_checksum(),
        classifier: Classifier::default(),
        context: Context::new(),
    });
    Box::into_raw(encoder)
}

/// Free an encoder
///
/// # Arguments
///
/// * `encoder` - Encoder to free. May be NULL (no-op).
///
/// # Safety
///
/// The encoder must not be used after calling this function.
#[no_mangle]
pub extern "C" fn alec_encoder_free(encoder: *mut AlecEncoder) {
    if !encoder.is_null() {
        unsafe {
            drop(Box::from_raw(encoder));
        }
    }
}

/// Encode a single floating-point value
///
/// # Arguments
///
/// * `encoder` - Encoder handle (must not be NULL)
/// * `value` - The value to encode
/// * `timestamp` - Timestamp for the value (can be 0 if not used)
/// * `source_id` - Source identifier string (null-terminated, can be NULL)
/// * `output` - Output buffer for encoded data
/// * `output_capacity` - Size of output buffer in bytes
/// * `output_len` - Pointer to store actual encoded length
///
/// # Returns
///
/// `ALEC_OK` on success, error code otherwise.
#[no_mangle]
pub extern "C" fn alec_encode_value(
    encoder: *mut AlecEncoder,
    value: f64,
    timestamp: u64,
    _source_id: *const c_char,
    output: *mut u8,
    output_capacity: usize,
    output_len: *mut usize,
) -> AlecResult {
    // Null checks
    if encoder.is_null() || output.is_null() || output_len.is_null() {
        return AlecResult::ErrorNullPointer;
    }

    let enc = unsafe { &mut *encoder };

    // Create RawData
    let raw_data = RawData::new(value, timestamp);

    // Classify the data
    let classification = enc.classifier.classify(&raw_data, &enc.context);

    // Encode the message
    let message = enc.encoder.encode(&raw_data, &classification, &enc.context);

    // Convert to bytes
    let encoded = message.to_bytes();
    if encoded.len() > output_capacity {
        return AlecResult::ErrorBufferTooSmall;
    }

    let output_slice = unsafe { slice::from_raw_parts_mut(output, output_capacity) };
    output_slice[..encoded.len()].copy_from_slice(&encoded);
    unsafe {
        *output_len = encoded.len();
    }

    // Observe the data for context learning
    enc.context.observe(&raw_data);

    AlecResult::Ok
}

/// Encode multiple values at once
///
/// # Arguments
///
/// * `encoder` - Encoder handle
/// * `values` - Array of values to encode
/// * `value_count` - Number of values in the array
/// * `timestamp` - Timestamp for the values
/// * `source_id` - Source identifier string (null-terminated, can be NULL)
/// * `output` - Output buffer for encoded data
/// * `output_capacity` - Size of output buffer in bytes
/// * `output_len` - Pointer to store actual encoded length
///
/// # Returns
///
/// `ALEC_OK` on success, error code otherwise.
#[no_mangle]
pub extern "C" fn alec_encode_multi(
    encoder: *mut AlecEncoder,
    values: *const f64,
    value_count: usize,
    timestamp: u64,
    _source_id: *const c_char,
    output: *mut u8,
    output_capacity: usize,
    output_len: *mut usize,
) -> AlecResult {
    // Null checks
    if encoder.is_null() || values.is_null() || output.is_null() || output_len.is_null() {
        return AlecResult::ErrorNullPointer;
    }

    if value_count == 0 {
        return AlecResult::ErrorInvalidInput;
    }

    let enc = unsafe { &mut *encoder };
    let values_slice = unsafe { slice::from_raw_parts(values, value_count) };

    // Convert to (name_id, value) pairs
    let value_pairs: Vec<(u16, f64)> = values_slice
        .iter()
        .enumerate()
        .map(|(i, &v)| (i as u16, v))
        .collect();

    // Encode multi
    let message = enc.encoder.encode_multi(
        &value_pairs,
        0, // source_id
        timestamp,
        alec::protocol::Priority::P3Normal,
        &enc.context,
    );

    // Convert to bytes
    let encoded = message.to_bytes();
    if encoded.len() > output_capacity {
        return AlecResult::ErrorBufferTooSmall;
    }

    let output_slice = unsafe { slice::from_raw_parts_mut(output, output_capacity) };
    output_slice[..encoded.len()].copy_from_slice(&encoded);
    unsafe {
        *output_len = encoded.len();
    }

    // Observe all values
    for &v in values_slice {
        let rd = RawData::new(v, timestamp);
        enc.context.observe(&rd);
    }

    AlecResult::Ok
}

/// Save encoder context to a file (for preload generation)
///
/// # Arguments
///
/// * `encoder` - Encoder handle
/// * `path` - File path (null-terminated string)
/// * `sensor_type` - Sensor type identifier (null-terminated string)
///
/// # Returns
///
/// `ALEC_OK` on success, error code otherwise.
#[no_mangle]
pub extern "C" fn alec_encoder_save_context(
    encoder: *mut AlecEncoder,
    path: *const c_char,
    sensor_type: *const c_char,
) -> AlecResult {
    if encoder.is_null() || path.is_null() || sensor_type.is_null() {
        return AlecResult::ErrorNullPointer;
    }

    let enc = unsafe { &*encoder };

    let path_str = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(s) => s,
        Err(_) => return AlecResult::ErrorInvalidUtf8,
    };

    let sensor_type_str = match unsafe { CStr::from_ptr(sensor_type) }.to_str() {
        Ok(s) => s,
        Err(_) => return AlecResult::ErrorInvalidUtf8,
    };

    match enc.context.save_to_file(Path::new(path_str), sensor_type_str) {
        Ok(()) => AlecResult::Ok,
        Err(_) => AlecResult::ErrorFileIo,
    }
}

/// Load encoder context from a preload file
///
/// # Arguments
///
/// * `encoder` - Encoder handle
/// * `path` - File path to preload (null-terminated string)
///
/// # Returns
///
/// `ALEC_OK` on success, error code otherwise.
#[no_mangle]
pub extern "C" fn alec_encoder_load_context(
    encoder: *mut AlecEncoder,
    path: *const c_char,
) -> AlecResult {
    if encoder.is_null() || path.is_null() {
        return AlecResult::ErrorNullPointer;
    }

    let enc = unsafe { &mut *encoder };

    let path_str = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(s) => s,
        Err(_) => return AlecResult::ErrorInvalidUtf8,
    };

    match Context::load_from_file(Path::new(path_str)) {
        Ok(ctx) => {
            enc.context = ctx;
            AlecResult::Ok
        }
        Err(_) => AlecResult::ErrorFileIo,
    }
}

/// Get the current context version
///
/// # Arguments
///
/// * `encoder` - Encoder handle
///
/// # Returns
///
/// The context version number, or 0 if encoder is NULL.
#[no_mangle]
pub extern "C" fn alec_encoder_context_version(encoder: *const AlecEncoder) -> u32 {
    if encoder.is_null() {
        return 0;
    }
    let enc = unsafe { &*encoder };
    enc.context.context_version()
}

// ============================================================================
// Decoder Functions
// ============================================================================

/// Create a new ALEC decoder
///
/// # Returns
///
/// A pointer to a new decoder, or NULL on allocation failure.
/// The decoder must be freed with `alec_decoder_free()` when no longer needed.
#[no_mangle]
pub extern "C" fn alec_decoder_new() -> *mut AlecDecoder {
    let decoder = Box::new(AlecDecoder {
        decoder: Decoder::new(),
        context: Context::new(),
    });
    Box::into_raw(decoder)
}

/// Create a new decoder with checksum verification enabled
///
/// # Returns
///
/// A pointer to a new decoder with checksum enabled, or NULL on failure.
#[no_mangle]
pub extern "C" fn alec_decoder_new_with_checksum() -> *mut AlecDecoder {
    let decoder = Box::new(AlecDecoder {
        decoder: Decoder::with_checksum_verification(),
        context: Context::new(),
    });
    Box::into_raw(decoder)
}

/// Free a decoder
///
/// # Arguments
///
/// * `decoder` - Decoder to free. May be NULL (no-op).
#[no_mangle]
pub extern "C" fn alec_decoder_free(decoder: *mut AlecDecoder) {
    if !decoder.is_null() {
        unsafe {
            drop(Box::from_raw(decoder));
        }
    }
}

/// Decode compressed data to a single value
///
/// # Arguments
///
/// * `decoder` - Decoder handle
/// * `input` - Compressed input data
/// * `input_len` - Length of input data
/// * `value` - Pointer to store decoded value
/// * `timestamp` - Pointer to store decoded timestamp (can be NULL)
///
/// # Returns
///
/// `ALEC_OK` on success, error code otherwise.
#[no_mangle]
pub extern "C" fn alec_decode_value(
    decoder: *mut AlecDecoder,
    input: *const u8,
    input_len: usize,
    value: *mut f64,
    timestamp: *mut u64,
) -> AlecResult {
    if decoder.is_null() || input.is_null() || value.is_null() {
        return AlecResult::ErrorNullPointer;
    }

    let dec = unsafe { &mut *decoder };
    let input_slice = unsafe { slice::from_raw_parts(input, input_len) };

    match dec.decoder.decode_bytes(input_slice, &dec.context) {
        Ok(decoded_data) => {
            unsafe {
                *value = decoded_data.value;
                if !timestamp.is_null() {
                    *timestamp = decoded_data.timestamp;
                }
            }
            AlecResult::Ok
        }
        Err(_) => AlecResult::ErrorDecodingFailed,
    }
}

/// Decode compressed data to multiple values
///
/// # Arguments
///
/// * `decoder` - Decoder handle
/// * `input` - Compressed input data
/// * `input_len` - Length of input data
/// * `values` - Output buffer for decoded values
/// * `values_capacity` - Maximum number of values that can be stored
/// * `values_count` - Pointer to store actual number of decoded values
///
/// # Returns
///
/// `ALEC_OK` on success, error code otherwise.
#[no_mangle]
pub extern "C" fn alec_decode_multi(
    decoder: *mut AlecDecoder,
    input: *const u8,
    input_len: usize,
    values: *mut f64,
    values_capacity: usize,
    values_count: *mut usize,
) -> AlecResult {
    if decoder.is_null() || input.is_null() || values.is_null() || values_count.is_null() {
        return AlecResult::ErrorNullPointer;
    }

    let dec = unsafe { &mut *decoder };
    let input_slice = unsafe { slice::from_raw_parts(input, input_len) };

    // First parse the message from bytes
    let message = match alec::protocol::EncodedMessage::from_bytes(input_slice) {
        Some(msg) => msg,
        None => return AlecResult::ErrorDecodingFailed,
    };

    match dec.decoder.decode_multi(&message, &dec.context) {
        Ok(value_pairs) => {
            if value_pairs.len() > values_capacity {
                return AlecResult::ErrorBufferTooSmall;
            }

            let values_slice = unsafe { slice::from_raw_parts_mut(values, values_capacity) };
            for (i, (_, val)) in value_pairs.iter().enumerate() {
                values_slice[i] = *val;
            }
            unsafe {
                *values_count = value_pairs.len();
            }
            AlecResult::Ok
        }
        Err(_) => AlecResult::ErrorDecodingFailed,
    }
}

/// Load decoder context from a preload file
///
/// # Arguments
///
/// * `decoder` - Decoder handle
/// * `path` - File path to preload (null-terminated string)
///
/// # Returns
///
/// `ALEC_OK` on success, error code otherwise.
#[no_mangle]
pub extern "C" fn alec_decoder_load_context(
    decoder: *mut AlecDecoder,
    path: *const c_char,
) -> AlecResult {
    if decoder.is_null() || path.is_null() {
        return AlecResult::ErrorNullPointer;
    }

    let dec = unsafe { &mut *decoder };

    let path_str = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(s) => s,
        Err(_) => return AlecResult::ErrorInvalidUtf8,
    };

    match Context::load_from_file(Path::new(path_str)) {
        Ok(ctx) => {
            dec.context = ctx;
            AlecResult::Ok
        }
        Err(_) => AlecResult::ErrorFileIo,
    }
}

/// Get the current decoder context version
///
/// # Arguments
///
/// * `decoder` - Decoder handle
///
/// # Returns
///
/// The context version number, or 0 if decoder is NULL.
#[no_mangle]
pub extern "C" fn alec_decoder_context_version(decoder: *const AlecDecoder) -> u32 {
    if decoder.is_null() {
        return 0;
    }
    let dec = unsafe { &*decoder };
    dec.context.context_version()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_version() {
        let version = alec_version();
        assert!(!version.is_null());
        let version_str = unsafe { CStr::from_ptr(version) }.to_str().unwrap();
        assert_eq!(version_str, "1.0.0");
    }

    #[test]
    fn test_result_to_string() {
        let ok_str = alec_result_to_string(AlecResult::Ok);
        assert!(!ok_str.is_null());
        let ok = unsafe { CStr::from_ptr(ok_str) }.to_str().unwrap();
        assert_eq!(ok, "Success");
    }

    #[test]
    fn test_encoder_lifecycle() {
        let enc = alec_encoder_new();
        assert!(!enc.is_null());
        alec_encoder_free(enc);
    }

    #[test]
    fn test_encoder_free_null() {
        // Should not crash
        alec_encoder_free(ptr::null_mut());
    }

    #[test]
    fn test_decoder_lifecycle() {
        let dec = alec_decoder_new();
        assert!(!dec.is_null());
        alec_decoder_free(dec);
    }

    #[test]
    fn test_encode_value() {
        let enc = alec_encoder_new();
        assert!(!enc.is_null());

        let mut output = [0u8; 256];
        let mut output_len: usize = 0;

        let result = alec_encode_value(
            enc,
            22.5,
            1234567890,
            ptr::null(),
            output.as_mut_ptr(),
            output.len(),
            &mut output_len,
        );

        assert_eq!(result, AlecResult::Ok);
        assert!(output_len > 0);

        alec_encoder_free(enc);
    }

    #[test]
    fn test_encode_null_pointer() {
        let mut output = [0u8; 256];
        let mut output_len: usize = 0;

        let result = alec_encode_value(
            ptr::null_mut(),
            22.5,
            0,
            ptr::null(),
            output.as_mut_ptr(),
            output.len(),
            &mut output_len,
        );

        assert_eq!(result, AlecResult::ErrorNullPointer);
    }

    #[test]
    fn test_encode_multi() {
        let enc = alec_encoder_new();
        let values = [22.0, 22.5, 23.0, 22.8];
        let mut output = [0u8; 256];
        let mut output_len: usize = 0;

        let result = alec_encode_multi(
            enc,
            values.as_ptr(),
            values.len(),
            0,
            ptr::null(),
            output.as_mut_ptr(),
            output.len(),
            &mut output_len,
        );

        assert_eq!(result, AlecResult::Ok);
        assert!(output_len > 0);

        alec_encoder_free(enc);
    }

    #[test]
    fn test_context_version() {
        let enc = alec_encoder_new();
        let version = alec_encoder_context_version(enc);
        assert_eq!(version, 0); // Initial version

        // Encode some data to increment version
        let mut output = [0u8; 256];
        let mut output_len: usize = 0;
        alec_encode_value(
            enc,
            22.5,
            0,
            ptr::null(),
            output.as_mut_ptr(),
            output.len(),
            &mut output_len,
        );

        let new_version = alec_encoder_context_version(enc);
        assert!(new_version > version);

        alec_encoder_free(enc);
    }

    #[test]
    fn test_encoder_with_checksum() {
        let enc = alec_encoder_new_with_checksum();
        assert!(!enc.is_null());

        let mut output = [0u8; 256];
        let mut output_len: usize = 0;

        let result = alec_encode_value(
            enc,
            22.5,
            0,
            ptr::null(),
            output.as_mut_ptr(),
            output.len(),
            &mut output_len,
        );

        assert_eq!(result, AlecResult::Ok);
        alec_encoder_free(enc);
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let enc = alec_encoder_new();
        let dec = alec_decoder_new();

        let original_value = 22.5;
        let mut encoded = [0u8; 256];
        let mut encoded_len: usize = 0;

        // Encode
        let result = alec_encode_value(
            enc,
            original_value,
            12345,
            ptr::null(),
            encoded.as_mut_ptr(),
            encoded.len(),
            &mut encoded_len,
        );
        assert_eq!(result, AlecResult::Ok);

        // Decode
        let mut decoded_value: f64 = 0.0;
        let mut decoded_timestamp: u64 = 0;
        let result = alec_decode_value(
            dec,
            encoded.as_ptr(),
            encoded_len,
            &mut decoded_value,
            &mut decoded_timestamp,
        );
        assert_eq!(result, AlecResult::Ok);
        assert!((decoded_value - original_value).abs() < 0.01);

        alec_encoder_free(enc);
        alec_decoder_free(dec);
    }
}
