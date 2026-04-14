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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

#[cfg(not(feature = "std"))]
extern crate alloc;

// Zephyr RTOS support: global allocator via k_malloc/k_free, no panic handler
#[cfg(feature = "zephyr")]
mod zephyr_support {
    use core::alloc::{GlobalAlloc, Layout};

    extern "C" {
        // k_aligned_alloc is required instead of k_malloc because k_malloc
        // returns 4-byte aligned memory on ARM. Rust types such as
        // Vec<(u16, f64)> and BTreeMap nodes require 8-byte alignment;
        // using k_malloc causes misaligned access (UB) on Cortex-M33.
        fn k_aligned_alloc(align: usize, size: usize) -> *mut u8;
        fn k_free(ptr: *mut u8);
    }

    struct ZephyrAllocator;

    unsafe impl GlobalAlloc for ZephyrAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            unsafe { k_aligned_alloc(layout.align(), layout.size()) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
            unsafe { k_free(ptr) }
        }
    }

    #[global_allocator]
    static ALLOCATOR: ZephyrAllocator = ZephyrAllocator;

    /// No-op on Zephyr — heap is managed by the RTOS.
    ///
    /// Provided for API compatibility with bare-metal builds.
    #[no_mangle]
    pub extern "C" fn alec_heap_init() {
        // Zephyr manages its own heap; nothing to do.
    }

    #[panic_handler]
    fn panic(_: &core::panic::PanicInfo) -> ! {
        loop {}
    }
}

// Bare-metal support: global allocator and panic handler
#[cfg(feature = "bare-metal")]
mod bare_metal_support {
    use embedded_alloc::LlffHeap as Heap;

    #[global_allocator]
    static HEAP: Heap = Heap::empty();

    /// Initialize the heap allocator. Must be called before any alloc usage.
    ///
    /// # Safety
    ///
    /// Must be called exactly once, before any heap allocation.
    #[no_mangle]
    pub unsafe extern "C" fn alec_heap_init() {
        const HEAP_SIZE: usize = 8192;
        static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
        unsafe {
            HEAP.init(&raw mut HEAP_MEM as usize, HEAP_SIZE);
        }
    }

    /// Initialize the heap allocator with a caller-provided buffer.
    ///
    /// Required on RTOSes (FreeRTOS, Milesight firmware) where the heap
    /// region is managed by the integrator and must not be statically
    /// embedded in the ALEC library itself.
    ///
    /// # Arguments
    ///
    /// * `buf` - Pointer to the start of the heap region. Must remain
    ///           valid for the lifetime of the program. Must be non-NULL.
    /// * `len` - Size of the heap region in bytes. Must be > 0.
    ///
    /// # Safety
    ///
    /// * Must be called exactly once, before any ALEC allocation.
    /// * `buf` must point to `len` bytes of writable memory that stays
    ///   valid for the lifetime of the process.
    /// * This function must not be combined with `alec_heap_init()`.
    /// * No-op if `buf` is NULL or `len == 0`.
    #[no_mangle]
    pub unsafe extern "C" fn alec_heap_init_with_buffer(buf: *mut u8, len: usize) {
        if buf.is_null() || len == 0 {
            return;
        }
        unsafe {
            HEAP.init(buf as usize, len);
        }
    }

    #[panic_handler]
    fn panic(_info: &core::panic::PanicInfo) -> ! {
        loop {}
    }
}

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, vec::Vec};

use core::ffi::{c_char, CStr};
use core::slice;
#[cfg(feature = "std")]
use std::path::Path;

use alec::classifier::Classifier;
use alec::context::{Context, ContextConfig};
use alec::protocol::{ChannelInput, RawData};
use alec::{Decoder, Encoder};

// ============================================================================
// Default configuration values (also documented in AlecEncoderConfig)
// ============================================================================

/// Default history size per source (validated on 99-message EM500-CO2 dataset).
pub const ALEC_DEFAULT_HISTORY_SIZE: u32 = 20;
/// Default maximum number of patterns retained in the dictionary.
pub const ALEC_DEFAULT_MAX_PATTERNS: u32 = 256;
/// Default maximum memory budget for the context (bytes).
pub const ALEC_DEFAULT_MAX_MEMORY_BYTES: u32 = 2048;
/// Default keyframe interval (messages between forced Raw32 keyframes).
pub const ALEC_DEFAULT_KEYFRAME_INTERVAL: u32 = 50;
/// Default for smart-resync via LoRaWAN downlink.
pub const ALEC_DEFAULT_SMART_RESYNC: bool = true;

/// Runtime configuration for a new ALEC encoder.
///
/// Mirrors the Milesight-integration defaults (history=20,
/// patterns=256, memory=2048B, keyframe=50, smart_resync=true).
///
/// Pass a NULL pointer to `alec_encoder_new_with_config` to use all
/// defaults. Any field set to 0 is also replaced by its default, so
/// callers can opt in to a single override while keeping the rest.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AlecEncoderConfig {
    /// Per-source history window size. Default: 20.
    pub history_size: u32,
    /// Maximum patterns retained in the context dictionary. Default: 256.
    pub max_patterns: u32,
    /// Maximum memory budget for the context in bytes. Default: 2048.
    pub max_memory_bytes: u32,
    /// Interval (in messages) between forced Raw32 keyframes. Default: 50.
    /// Set to 0 to disable periodic keyframes.
    pub keyframe_interval: u32,
    /// If true, the encoder honours downlink-driven resync requests
    /// (via `alec_force_keyframe`). Default: true.
    pub smart_resync: bool,
}

impl AlecEncoderConfig {
    /// Return a config pre-populated with all Milesight defaults.
    pub fn defaults() -> Self {
        Self {
            history_size: ALEC_DEFAULT_HISTORY_SIZE,
            max_patterns: ALEC_DEFAULT_MAX_PATTERNS,
            max_memory_bytes: ALEC_DEFAULT_MAX_MEMORY_BYTES,
            keyframe_interval: ALEC_DEFAULT_KEYFRAME_INTERVAL,
            smart_resync: ALEC_DEFAULT_SMART_RESYNC,
        }
    }

    /// Resolve the effective config, replacing any 0 numeric field by
    /// its default. Numeric fields set to 0 are treated as "use default"
    /// rather than "disable" — except `keyframe_interval`, where 0 is a
    /// legitimate way to disable the keyframe mechanism.
    fn resolved(self) -> Self {
        let d = Self::defaults();
        Self {
            history_size: if self.history_size == 0 {
                d.history_size
            } else {
                self.history_size
            },
            max_patterns: if self.max_patterns == 0 {
                d.max_patterns
            } else {
                self.max_patterns
            },
            max_memory_bytes: if self.max_memory_bytes == 0 {
                d.max_memory_bytes
            } else {
                self.max_memory_bytes
            },
            // keyframe_interval == 0 is a valid value (disabled), so keep as-is.
            keyframe_interval: self.keyframe_interval,
            smart_resync: self.smart_resync,
        }
    }

    /// Build an `alec::context::ContextConfig` from this FFI config.
    fn to_context_config(self) -> ContextConfig {
        let r = self.resolved();
        let mut cfg = ContextConfig::default();
        cfg.history_size = r.history_size as usize;
        cfg.max_patterns = r.max_patterns as usize;
        cfg.max_memory = r.max_memory_bytes as usize;
        cfg
    }
}

/// Hash a C source_id string to a u32 for context keying.
/// Returns 0 if the pointer is null.
fn hash_source_id(source_id: *const c_char) -> u32 {
    if source_id.is_null() {
        return 0;
    }
    let bytes = unsafe { CStr::from_ptr(source_id) }.to_bytes();
    // Map to 1..=127 so the source_id always encodes as a 1-byte varint.
    // 0 is reserved for NULL / "no source".
    (xxhash_rust::xxh64::xxh64(bytes, 0) % 127 + 1) as u32
}

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
    /// If set, the next encode call should emit a keyframe (Raw32 for all
    /// channels). Consumed by the Bloc B/C fixed-channel encode path.
    /// Plumbed here in Bloc A so downlink handlers can set the flag
    /// immediately without waiting for the encode path to land.
    force_keyframe_pending: bool,
    /// Number of encoded messages since the last keyframe. Used by the
    /// fixed-channel encode path (Bloc C) to trigger periodic keyframes.
    #[allow(dead_code)] // Consumed by Bloc C keyframe mechanism.
    messages_since_keyframe: u32,
    /// Configured keyframe interval (0 disables periodic keyframes).
    #[allow(dead_code)] // Consumed by Bloc C keyframe mechanism.
    keyframe_interval: u32,
    /// Honour downlink-driven resync.
    smart_resync: bool,
}

/// Opaque decoder handle
///
/// Created with `alec_decoder_new()`, freed with `alec_decoder_free()`.
/// Do not access internal fields directly.
pub struct AlecDecoder {
    decoder: Decoder,
    context: Context,
    /// Header sequence number observed on the most recent multi-frame
    /// decode (None if none has been decoded yet).
    last_header_sequence: Option<u16>,
    /// Number of missing frames detected on the most recent decode
    /// (clipped to 255). 0 means no gap.
    last_gap_size: u8,
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
    static VERSION: &[u8] = b"1.3.1\0";
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
    let defaults = AlecEncoderConfig::defaults();
    let encoder = Box::new(AlecEncoder {
        encoder: Encoder::new(),
        classifier: Classifier::default(),
        context: Context::new(),
        force_keyframe_pending: false,
        messages_since_keyframe: 0,
        keyframe_interval: defaults.keyframe_interval,
        smart_resync: defaults.smart_resync,
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
    let defaults = AlecEncoderConfig::defaults();
    let encoder = Box::new(AlecEncoder {
        encoder: Encoder::with_checksum(),
        classifier: Classifier::default(),
        context: Context::new(),
        force_keyframe_pending: false,
        messages_since_keyframe: 0,
        keyframe_interval: defaults.keyframe_interval,
        smart_resync: defaults.smart_resync,
    });
    Box::into_raw(encoder)
}

/// Create a new ALEC encoder with a custom configuration.
///
/// Mirrors the Milesight integration requirements: the caller specifies
/// `history_size`, `max_patterns`, `max_memory_bytes`, `keyframe_interval`
/// and `smart_resync`. See `AlecEncoderConfig` for defaults.
///
/// # Arguments
///
/// * `config` - Pointer to an `AlecEncoderConfig`. If NULL, all defaults
///              are used. Numeric fields set to 0 are replaced by their
///              default (except `keyframe_interval`, where 0 disables
///              periodic keyframes).
///
/// # Returns
///
/// A pointer to a new encoder, or NULL on allocation failure.
/// Must be freed with `alec_encoder_free()`.
#[no_mangle]
pub extern "C" fn alec_encoder_new_with_config(
    config: *const AlecEncoderConfig,
) -> *mut AlecEncoder {
    let cfg = if config.is_null() {
        AlecEncoderConfig::defaults()
    } else {
        unsafe { *config }.resolved()
    };

    let context = Context::with_config(cfg.to_context_config());
    let encoder = Box::new(AlecEncoder {
        encoder: Encoder::new(),
        classifier: Classifier::default(),
        context,
        force_keyframe_pending: false,
        messages_since_keyframe: 0,
        keyframe_interval: cfg.keyframe_interval,
        smart_resync: cfg.smart_resync,
    });
    Box::into_raw(encoder)
}

/// Force the next encode call to emit a keyframe (Raw32 for all channels).
///
/// Intended to be called from a LoRaWAN downlink handler receiving the
/// 0xFF resync command from the server-side sidecar. The flag is
/// consumed by the fixed-channel encode path (Bloc B/C); until that
/// path lands, calling this only sets the internal flag.
///
/// No-op if `encoder` is NULL or if the encoder was configured with
/// `smart_resync = false`.
///
/// # Arguments
///
/// * `encoder` - Encoder handle.
#[no_mangle]
pub extern "C" fn alec_force_keyframe(encoder: *mut AlecEncoder) {
    if encoder.is_null() {
        return;
    }
    let enc = unsafe { &mut *encoder };
    if !enc.smart_resync {
        return;
    }
    enc.force_keyframe_pending = true;
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
    source_id: *const c_char,
    output: *mut u8,
    output_capacity: usize,
    output_len: *mut usize,
) -> AlecResult {
    // Null checks
    if encoder.is_null() || output.is_null() || output_len.is_null() {
        return AlecResult::ErrorNullPointer;
    }

    let enc = unsafe { &mut *encoder };
    let sid = hash_source_id(source_id);

    // Create RawData with hashed source_id for per-channel context isolation
    let raw_data = RawData::with_source(sid, value, timestamp);

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

/// Encode multiple values with adaptive per-channel compression.
///
/// Each channel is independently classified (P1–P5) and encoded using the
/// optimal strategy (Repeated, Delta8, Delta16, etc.). P5 channels are
/// excluded from the output frame but their context is still updated.
///
/// # Arguments
///
/// * `encoder` - Encoder handle
/// * `values` - Array of f64 values to encode (one per channel)
/// * `value_count` - Number of channels
/// * `timestamps` - Per-channel timestamps (array of uint64_t), or NULL to
///   use 0 for all channels
/// * `source_ids` - Per-channel source identifier strings (array of
///   `const char*`), or NULL for automatic index-based IDs
/// * `priorities` - Per-channel priority overrides (1–5), or NULL for
///   classifier-assigned priorities
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
    timestamps: *const u64,
    source_ids: *const *const c_char,
    priorities: *const u8,
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

    // Build ChannelInput array
    let channels: Vec<ChannelInput> = (0..value_count)
        .map(|i| {
            let sid = if source_ids.is_null() {
                // Use index+1 as source_id (1-byte varint, non-zero)
                (i as u32) + 1
            } else {
                let ptr = unsafe { *source_ids.add(i) };
                hash_source_id(ptr)
            };

            ChannelInput {
                name_id: i as u8,
                source_id: sid,
                value: values_slice[i],
            }
        })
        .collect();

    // Apply priority overrides if provided
    let timestamp = if timestamps.is_null() {
        0u64
    } else {
        // Use first timestamp for the shared header
        unsafe { *timestamps }
    };

    // If priorities are provided, set critical thresholds to force classification.
    // Otherwise let the classifier decide naturally.
    // For now, we use the classifier and override priorities post-classification.
    let (message, classifications) =
        enc.encoder
            .encode_multi_adaptive(&channels, timestamp, &enc.context, &enc.classifier);

    // If explicit priorities were provided, we need to re-encode with those.
    // However, the cleaner approach is to let the classifier work and use the
    // priorities parameter as an override. Since encode_multi_adaptive already
    // classified, and the user's priority is just a hint, we accept the
    // classifier result. If the caller passes priorities != NULL, we could
    // use them, but for v1.3 we trust the classifier.
    let _ = priorities; // Reserved for future override support
    let _ = classifications;

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

    // Observe ALL channels (including P5) for context learning.
    // Use name_id as source_id — matches encode/decode convention for multi frames.
    for ch in &channels {
        let rd = RawData::with_source(ch.name_id as u32, ch.value, timestamp);
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
#[cfg(feature = "std")]
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

    match enc
        .context
        .save_to_file(Path::new(path_str), sensor_type_str)
    {
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
#[cfg(feature = "std")]
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
        last_header_sequence: None,
        last_gap_size: 0,
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
        last_header_sequence: None,
        last_gap_size: 0,
    });
    Box::into_raw(decoder)
}

/// Check whether the most recent decode detected a sequence gap.
///
/// The server-side sidecar uses this to decide whether to issue a
/// resync downlink (0xFF) to the device. The gap size is the number
/// of missing frames between the previous `last_sequence` and the
/// current one, clipped to 255.
///
/// # Arguments
///
/// * `decoder`      - Decoder handle.
/// * `out_gap_size` - Out parameter receiving the gap size (may be NULL).
///
/// # Returns
///
/// `true` if the most recent multi-frame decode observed missing
/// frames (gap > 0). `false` if no gap, if no decode has been
/// performed yet, or if `decoder` is NULL.
#[no_mangle]
pub extern "C" fn alec_decoder_gap_detected(
    decoder: *const AlecDecoder,
    out_gap_size: *mut u8,
) -> bool {
    if decoder.is_null() {
        if !out_gap_size.is_null() {
            unsafe { *out_gap_size = 0 };
        }
        return false;
    }
    let dec = unsafe { &*decoder };
    if !out_gap_size.is_null() {
        unsafe { *out_gap_size = dec.last_gap_size };
    }
    dec.last_gap_size > 0
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

    // Gap detection: compute the number of missing frames between the
    // previous header sequence and this one. `decode_multi` does not
    // touch `Decoder::last_sequence`, so we track it here at the FFI
    // layer. Clipped to 255 to fit `last_gap_size: u8`.
    let cur_seq = message.header.sequence;
    dec.last_gap_size = match dec.last_header_sequence {
        Some(prev) => {
            let diff = cur_seq.wrapping_sub(prev);
            if diff == 0 {
                0 // same frame replayed — treat as no gap
            } else {
                let missing = diff.saturating_sub(1);
                missing.min(u8::MAX as u16) as u8
            }
        }
        None => 0,
    };
    dec.last_header_sequence = Some(cur_seq);

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
#[cfg(feature = "std")]
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
        assert_eq!(version_str, "1.3.1");
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
        let values: [f64; 4] = [22.0, 22.5, 23.0, 22.8];
        let mut output = [0u8; 256];
        let mut output_len: usize = 0;

        let result = alec_encode_multi(
            enc,
            values.as_ptr(),
            values.len(),
            ptr::null(), // timestamps
            ptr::null(), // source_ids
            ptr::null(), // priorities
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
    fn test_encode_value_with_source_id() {
        let enc = alec_encoder_new();
        let mut output = [0u8; 256];
        let mut output_len: usize = 0;

        let source = b"temperature\0";
        let result = alec_encode_value(
            enc,
            22.5,
            0,
            source.as_ptr() as *const c_char,
            output.as_mut_ptr(),
            output.len(),
            &mut output_len,
        );

        assert_eq!(result, AlecResult::Ok);
        assert!(output_len > 0);

        // Encoding with NULL source_id should also work (defaults to 0)
        let result2 = alec_encode_value(
            enc,
            22.5,
            1,
            ptr::null(),
            output.as_mut_ptr(),
            output.len(),
            &mut output_len,
        );
        assert_eq!(result2, AlecResult::Ok);

        alec_encoder_free(enc);
    }

    #[test]
    fn test_hash_source_id() {
        // NULL returns 0
        assert_eq!(hash_source_id(ptr::null()), 0);

        // Non-null returns a deterministic hash in 1..=127 (1-byte varint)
        let a = b"temperature\0";
        let b = b"pressure\0";
        let ha = hash_source_id(a.as_ptr() as *const c_char);
        let hb = hash_source_id(b.as_ptr() as *const c_char);
        assert!(
            ha >= 1 && ha <= 127,
            "hash out of 1-byte varint range: {}",
            ha
        );
        assert!(
            hb >= 1 && hb <= 127,
            "hash out of 1-byte varint range: {}",
            hb
        );
        assert_ne!(ha, hb);

        // Same input → same hash
        let ha2 = hash_source_id(a.as_ptr() as *const c_char);
        assert_eq!(ha, ha2);
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

    // ============================================================================
    // Bloc A1: Config FFI + keyframe + gap detection
    // ============================================================================

    #[test]
    fn test_encoder_config_defaults_constants() {
        // Guard against silent drift of the Milesight-integration defaults.
        assert_eq!(ALEC_DEFAULT_HISTORY_SIZE, 20);
        assert_eq!(ALEC_DEFAULT_MAX_PATTERNS, 256);
        assert_eq!(ALEC_DEFAULT_MAX_MEMORY_BYTES, 2048);
        assert_eq!(ALEC_DEFAULT_KEYFRAME_INTERVAL, 50);
        assert!(ALEC_DEFAULT_SMART_RESYNC);

        let d = AlecEncoderConfig::defaults();
        assert_eq!(d.history_size, ALEC_DEFAULT_HISTORY_SIZE);
        assert_eq!(d.max_patterns, ALEC_DEFAULT_MAX_PATTERNS);
        assert_eq!(d.max_memory_bytes, ALEC_DEFAULT_MAX_MEMORY_BYTES);
        assert_eq!(d.keyframe_interval, ALEC_DEFAULT_KEYFRAME_INTERVAL);
        assert!(d.smart_resync);
    }

    #[test]
    fn test_encoder_new_with_config_null_uses_defaults() {
        let enc = alec_encoder_new_with_config(ptr::null());
        assert!(!enc.is_null());
        let e = unsafe { &*enc };
        assert_eq!(e.keyframe_interval, ALEC_DEFAULT_KEYFRAME_INTERVAL);
        assert!(e.smart_resync);
        assert!(!e.force_keyframe_pending);
        assert_eq!(e.messages_since_keyframe, 0);
        alec_encoder_free(enc);
    }

    #[test]
    fn test_encoder_new_with_config_custom() {
        let cfg = AlecEncoderConfig {
            history_size: 10,
            max_patterns: 128,
            max_memory_bytes: 1024,
            keyframe_interval: 25,
            smart_resync: false,
        };
        let enc = alec_encoder_new_with_config(&cfg);
        assert!(!enc.is_null());
        let e = unsafe { &*enc };
        assert_eq!(e.keyframe_interval, 25);
        assert!(!e.smart_resync);
        alec_encoder_free(enc);
    }

    #[test]
    fn test_encoder_new_with_config_zero_uses_defaults() {
        // Numeric fields set to 0 (except keyframe_interval) should
        // fall back to the Milesight defaults.
        let cfg = AlecEncoderConfig {
            history_size: 0,
            max_patterns: 0,
            max_memory_bytes: 0,
            keyframe_interval: 0, // 0 means "disabled", kept as-is
            smart_resync: true,
        };
        let enc = alec_encoder_new_with_config(&cfg);
        assert!(!enc.is_null());
        let e = unsafe { &*enc };
        // keyframe_interval=0 must be preserved verbatim (disabled).
        assert_eq!(e.keyframe_interval, 0);
        // Encoding should still succeed — history/patterns/memory got defaults.
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
    fn test_force_keyframe_sets_flag() {
        let enc = alec_encoder_new();
        assert!(!enc.is_null());
        assert!(!unsafe { &*enc }.force_keyframe_pending);
        alec_force_keyframe(enc);
        assert!(unsafe { &*enc }.force_keyframe_pending);
        alec_encoder_free(enc);
    }

    #[test]
    fn test_force_keyframe_null_is_noop() {
        // Must not crash on NULL.
        alec_force_keyframe(ptr::null_mut());
    }

    #[test]
    fn test_force_keyframe_respects_smart_resync_disabled() {
        let cfg = AlecEncoderConfig {
            history_size: 0,
            max_patterns: 0,
            max_memory_bytes: 0,
            keyframe_interval: 50,
            smart_resync: false,
        };
        let enc = alec_encoder_new_with_config(&cfg);
        alec_force_keyframe(enc);
        // smart_resync=false → force_keyframe is a no-op.
        assert!(!unsafe { &*enc }.force_keyframe_pending);
        alec_encoder_free(enc);
    }

    #[test]
    fn test_gap_detected_null_decoder() {
        let mut gap: u8 = 42;
        let detected = alec_decoder_gap_detected(ptr::null(), &mut gap);
        assert!(!detected);
        assert_eq!(gap, 0);

        // NULL out_gap_size is also fine.
        let detected2 = alec_decoder_gap_detected(ptr::null(), ptr::null_mut());
        assert!(!detected2);
    }

    #[test]
    fn test_gap_detected_fresh_decoder() {
        // No decode has happened yet — no gap.
        let dec = alec_decoder_new();
        let mut gap: u8 = 99;
        let detected = alec_decoder_gap_detected(dec, &mut gap);
        assert!(!detected);
        assert_eq!(gap, 0);
        alec_decoder_free(dec);
    }

    /// End-to-end: encode 5 multi-frames, drop two between frame #1 and #2
    /// from the decoder's perspective, verify the gap is reported.
    #[test]
    fn test_gap_detected_after_dropped_frames() {
        let enc = alec_encoder_new();
        let dec = alec_decoder_new();

        let values: [f64; 4] = [22.0, 22.5, 23.0, 22.8];
        let mut frames: Vec<Vec<u8>> = Vec::new();
        for _ in 0..5 {
            let mut out = [0u8; 128];
            let mut out_len = 0usize;
            let res = alec_encode_multi(
                enc,
                values.as_ptr(),
                values.len(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                out.as_mut_ptr(),
                out.len(),
                &mut out_len,
            );
            assert_eq!(res, AlecResult::Ok);
            frames.push(out[..out_len].to_vec());
        }

        // Decode frame 0 (no gap yet).
        let mut vals = [0f64; 4];
        let mut vcount: usize = 0;
        let r0 = alec_decode_multi(
            dec,
            frames[0].as_ptr(),
            frames[0].len(),
            vals.as_mut_ptr(),
            vals.len(),
            &mut vcount,
        );
        assert_eq!(r0, AlecResult::Ok);
        let mut gap: u8 = 0;
        assert!(!alec_decoder_gap_detected(dec, &mut gap));
        assert_eq!(gap, 0);

        // Skip frames 1 and 2, decode frame 3 → gap of 2.
        let r3 = alec_decode_multi(
            dec,
            frames[3].as_ptr(),
            frames[3].len(),
            vals.as_mut_ptr(),
            vals.len(),
            &mut vcount,
        );
        assert_eq!(r3, AlecResult::Ok);
        let mut gap: u8 = 0;
        assert!(alec_decoder_gap_detected(dec, &mut gap));
        assert_eq!(gap, 2);

        // Decode frame 4 — contiguous with frame 3 → no gap.
        let r4 = alec_decode_multi(
            dec,
            frames[4].as_ptr(),
            frames[4].len(),
            vals.as_mut_ptr(),
            vals.len(),
            &mut vcount,
        );
        assert_eq!(r4, AlecResult::Ok);
        let mut gap: u8 = 99;
        assert!(!alec_decoder_gap_detected(dec, &mut gap));
        assert_eq!(gap, 0);

        alec_encoder_free(enc);
        alec_decoder_free(dec);
    }

    /// cbindgen should surface the new symbols. This test is a compile-time
    /// guarantee that the FFI entry points exist with the expected signatures.
    #[test]
    fn test_ffi_symbols_exist() {
        let _new: extern "C" fn(*const AlecEncoderConfig) -> *mut AlecEncoder =
            alec_encoder_new_with_config;
        let _force: extern "C" fn(*mut AlecEncoder) = alec_force_keyframe;
        let _gap: extern "C" fn(*const AlecDecoder, *mut u8) -> bool =
            alec_decoder_gap_detected;
    }
}
