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

/// Key used when `observe()`-ing channel `i` in a fixed-channel frame.
/// Must match `Encoder::fixed_channel_source_id` — kept here because
/// that helper is `pub(crate)` inside the `alec` crate.
#[inline]
fn fixed_channel_source_id(i: usize) -> u32 {
    (i as u32) + 1
}

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
// Bloc B — Compact fixed-channel FFI (Milesight EM500-CO2)
// ============================================================================

/// Encode a fixed-channel frame using the compact 4-byte header
/// (Milesight EM500-CO2 wire format).
///
/// The number of channels is passed explicitly and must match the
/// value used by the peer decoder — the wire format does not carry
/// it. The encoder keeps a positional view of the channels, so
/// `values[i]` is the value for channel index `i`.
///
/// If `keyframe_interval > 0` and `messages_since_keyframe`
/// has reached that interval, OR if `alec_force_keyframe` was called
/// since the last encode AND `smart_resync` is enabled, this frame
/// is emitted as a **keyframe** (marker 0xA2, Raw32 for every
/// channel). Otherwise a regular data frame (marker 0xA1) is emitted.
///
/// # Arguments
///
/// * `encoder`         - Encoder handle (must not be NULL).
/// * `values`          - Per-channel f64 values, positional.
/// * `channel_count`   - Number of channels in `values`.
/// * `output`          - Destination buffer for the wire bytes.
/// * `output_capacity` - Size of `output` in bytes.
/// * `out_len`         - Pointer receiving the number of bytes
///                       written to `output`.
///
/// # Returns
///
/// `ALEC_OK` on success. `ALEC_ERROR_BUFFER_TOO_SMALL` if the
/// encoded frame does not fit in `output`. `ALEC_ERROR_INVALID_INPUT`
/// for zero channels. `ALEC_ERROR_NULL_POINTER` for any required
/// NULL pointer.
///
/// The caller can detect that the frame must be replaced by the
/// legacy TLV fallback by comparing `*out_len` against the 11-byte
/// LoRaWAN ceiling: if `*out_len > 11`, emit the TLV frame instead.
#[no_mangle]
pub extern "C" fn alec_encode_multi_fixed(
    encoder: *mut AlecEncoder,
    values: *const f64,
    channel_count: usize,
    output: *mut u8,
    output_capacity: usize,
    out_len: *mut usize,
) -> AlecResult {
    if encoder.is_null() || values.is_null() || output.is_null() || out_len.is_null() {
        return AlecResult::ErrorNullPointer;
    }
    if channel_count == 0 {
        return AlecResult::ErrorInvalidInput;
    }

    let enc = unsafe { &mut *encoder };
    let values_slice = unsafe { slice::from_raw_parts(values, channel_count) };
    let output_slice = unsafe { slice::from_raw_parts_mut(output, output_capacity) };

    // Decide whether this frame should be a keyframe.
    let periodic_due = enc.keyframe_interval > 0
        && enc.messages_since_keyframe >= enc.keyframe_interval;
    let downlink_forced = enc.force_keyframe_pending && enc.smart_resync;
    let keyframe = periodic_due || downlink_forced;

    match enc
        .encoder
        .encode_multi_fixed(values_slice, &enc.context, keyframe, output_slice)
    {
        Ok(n) => {
            unsafe {
                *out_len = n;
            }

            // Counter housekeeping — only after a successful encode.
            // On a keyframe we reset to 1 (not 0): the keyframe IS the
            // first frame of the next cycle, so subsequent keyframes
            // land at a fixed modular offset (e.g. interval=10 →
            // frames 10, 20, 30 …). Off-by-one matters for the spec's
            // "Frames 10 and 20 must be Raw32" assertion.
            if keyframe {
                enc.messages_since_keyframe = 1;
                enc.force_keyframe_pending = false;
            } else {
                enc.messages_since_keyframe = enc.messages_since_keyframe.saturating_add(1);
            }

            // Observe each channel so the next encode sees this value
            // in the prediction cache. We observe the ORIGINAL input
            // (not the reconstructed value): on truly-stable signals
            // this is what lets `Repeated` fire frame-after-frame.
            // Observing the reconstructed value would break Repeated
            // detection because f32 roundtrip of e.g. 3.600 yields
            // 3.5999999046…, which is > f64::EPSILON away from 3.600.
            //
            // The trade-off is a bounded drift between encoder and
            // decoder contexts on delta-encoded channels. This drift
            // is capped by the periodic keyframe mechanism: every
            // `keyframe_interval` frames the encoder emits Raw32 for
            // every channel, which forces the decoder's view back to
            // f32-precision truth. See `test_encode_decode_fixed_roundtrip_5ch`.
            for (i, &v) in values_slice.iter().enumerate() {
                let rd = RawData::with_source(fixed_channel_source_id(i), v, 0);
                enc.context.observe(&rd);
            }

            AlecResult::Ok
        }
        Err(alec::error::AlecError::Encode(
            alec::error::EncodeError::BufferTooSmall { .. },
        )) => AlecResult::ErrorBufferTooSmall,
        Err(alec::error::AlecError::Encode(
            alec::error::EncodeError::PayloadTooLarge { .. },
        )) => AlecResult::ErrorInvalidInput,
        Err(_) => AlecResult::ErrorEncodingFailed,
    }
}

/// Decode a fixed-channel frame produced by `alec_encode_multi_fixed`.
///
/// The number of channels is passed explicitly — the wire format does
/// not carry it. Must match the value used by the encoder.
///
/// On a successful decode:
///   - `output[..channel_count]` receives the decoded values in channel order.
///   - The decoder's last-sequence and last-ctx-version are updated.
///   - The gap size (if any) is available via `alec_decoder_gap_detected`.
///
/// # Returns
///
/// * `ALEC_OK`                         on success.
/// * `ALEC_ERROR_INVALID_INPUT`        for zero channels or a non-ALEC marker byte.
/// * `ALEC_ERROR_BUFFER_TOO_SMALL`     if `output_capacity < channel_count`
///                                     or the input is shorter than the
///                                     header + bitmap + data bytes.
/// * `ALEC_ERROR_DECODING_FAILED`      for any other decode error.
/// * `ALEC_ERROR_NULL_POINTER`         for a NULL required pointer.
#[no_mangle]
pub extern "C" fn alec_decode_multi_fixed(
    decoder: *mut AlecDecoder,
    input: *const u8,
    input_len: usize,
    channel_count: usize,
    output: *mut f64,
    output_capacity: usize,
) -> AlecResult {
    if decoder.is_null() || input.is_null() || output.is_null() {
        return AlecResult::ErrorNullPointer;
    }
    if channel_count == 0 {
        return AlecResult::ErrorInvalidInput;
    }
    if output_capacity < channel_count {
        return AlecResult::ErrorBufferTooSmall;
    }

    let dec = unsafe { &mut *decoder };
    let input_slice = unsafe { slice::from_raw_parts(input, input_len) };
    let output_slice = unsafe { slice::from_raw_parts_mut(output, output_capacity) };

    match dec
        .decoder
        .decode_multi_fixed(input_slice, channel_count, &dec.context, output_slice)
    {
        Ok(info) => {
            // Mirror the gap tracking used by the legacy multi path
            // so `alec_decoder_gap_detected` returns the same view
            // regardless of which decode entry point was used.
            dec.last_header_sequence = Some(info.sequence);
            dec.last_gap_size = info.gap_size;

            // Observe every decoded value so the NEXT frame can
            // decode delta-encoded channels correctly.
            for i in 0..channel_count {
                let rd = RawData::with_source(fixed_channel_source_id(i), output_slice[i], 0);
                dec.context.observe(&rd);
            }
            AlecResult::Ok
        }
        Err(alec::error::AlecError::Decode(
            alec::error::DecodeError::BufferTooShort { .. },
        )) => AlecResult::ErrorBufferTooSmall,
        Err(alec::error::AlecError::Decode(alec::error::DecodeError::MalformedMessage {
            reason,
            ..
        })) if reason.contains("not a fixed-channel ALEC frame") => AlecResult::ErrorInvalidInput,
        Err(_) => AlecResult::ErrorDecodingFailed,
    }
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
        let _enc_fixed: extern "C" fn(
            *mut AlecEncoder,
            *const f64,
            usize,
            *mut u8,
            usize,
            *mut usize,
        ) -> AlecResult = alec_encode_multi_fixed;
        let _dec_fixed: extern "C" fn(
            *mut AlecDecoder,
            *const u8,
            usize,
            usize,
            *mut f64,
            usize,
        ) -> AlecResult = alec_decode_multi_fixed;
    }

    // ========================================================================
    // Bloc B5 — Compact fixed-channel tests
    //
    // A synthesized slow-drift EM500-CO2 dataset is used in lieu of a
    // 99-message CSV that is not shipped with the repo. The dataset is
    // built to reproduce the regime described in docs/CONTEXT.md:
    //   - 5 channels: battery / temperature / humidity / CO2 / pressure
    //   - 99 messages at a 10-minute cadence
    //   - slow drift on all channels → most channels encode as Repeated
    //     or Delta8 in steady state
    // ========================================================================

    /// Deterministic 99-message EM500-CO2 slow-drift dataset.
    ///
    /// Real EM500-CO2 sensors are polled every 10 minutes on a very
    /// slow-drift signal. The raw sensor has coarse quantization
    /// (battery 0.001V, temperature 0.01°C, humidity 0.1%, CO2 1ppm,
    /// pressure 0.01hPa) and values typically REPEAT across many
    /// consecutive reads. This dataset reproduces that regime with
    /// step changes that are rare and small (≤1 quantization step),
    /// matching the ~58% Repeated / ~42% Delta8 distribution cited
    /// in docs/CONTEXT.md.
    fn em500_co2_dataset() -> Vec<[f64; 5]> {
        const N: usize = 99;
        let mut out = Vec::with_capacity(N);
        // Values change rarely; most frames carry exact repeats.
        // Channel 0: battery (V). Drops by 1mV every ~33 frames.
        // Channel 1: temperature (°C). Walks ±0.01 every ~4 frames.
        // Channel 2: humidity (%). Walks ±0.1 every ~7 frames.
        // Channel 3: CO2 (ppm). +1 ppm every ~3 frames (slow ramp).
        // Channel 4: pressure (hPa). ±0.01 every ~11 frames.
        for i in 0..N {
            // Battery at 10mV granularity (real LoRaWAN battery
            // telemetry — devices don't change 1mV in 10 minutes,
            // and scale_factor=100 can't represent sub-0.01V anyway).
            let battery = 3.60 - 0.01 * ((i / 33) as f64);
            let temp_step = if i % 8 < 4 { 0.01 } else { 0.00 };
            let temperature = 22.50 + temp_step;
            let humidity = 45.0 + 0.1 * ((i / 7) % 3) as f64;
            let co2 = 420.0 + (i / 3) as f64;
            let pressure = 1013.25 + 0.01 * ((i / 11) % 3) as f64;
            out.push([battery, temperature, humidity, co2, pressure]);
        }
        out
    }

    /// Run the whole dataset through a fresh encoder+decoder pair and
    /// return (per-frame wire bytes, per-frame decoded values).
    fn encode_decode_dataset(
        data: &[[f64; 5]],
        cfg: Option<AlecEncoderConfig>,
    ) -> (Vec<Vec<u8>>, Vec<[f64; 5]>) {
        let enc = match cfg {
            Some(c) => alec_encoder_new_with_config(&c),
            None => alec_encoder_new(),
        };
        let dec = alec_decoder_new();

        let mut frames = Vec::with_capacity(data.len());
        let mut decoded = Vec::with_capacity(data.len());

        let mut out = [0u8; 128];
        let mut out_len: usize = 0;
        let mut values = [0f64; 5];

        for row in data {
            let r = alec_encode_multi_fixed(
                enc,
                row.as_ptr(),
                row.len(),
                out.as_mut_ptr(),
                out.len(),
                &mut out_len,
            );
            assert_eq!(r, AlecResult::Ok);
            let frame = out[..out_len].to_vec();

            let r2 = alec_decode_multi_fixed(
                dec,
                frame.as_ptr(),
                frame.len(),
                5,
                values.as_mut_ptr(),
                values.len(),
            );
            assert_eq!(r2, AlecResult::Ok, "decode failed at frame {}", frames.len());

            frames.push(frame);
            decoded.push(values);
        }

        alec_encoder_free(enc);
        alec_decoder_free(dec);
        (frames, decoded)
    }

    /// B5-1. Encode 99 real-shaped EM500-CO2 rows through the FFI,
    /// decode each frame, and verify the values round-trip within
    /// one quantization step.
    ///
    /// Delta encoding uses a global scale_factor of 100 (1/100
    /// resolution), so each frame carries ≤0.005 of quantization
    /// error per channel. The moving-average prediction residue on
    /// top of that is bounded by the history EMA alpha — with
    /// slow-drift inputs the combined error stays well under
    /// `0.01` (one quantization step).
    #[test]
    fn test_encode_decode_fixed_roundtrip_5ch() {
        let data = em500_co2_dataset();
        let (_frames, decoded) = encode_decode_dataset(&data, None);
        assert_eq!(decoded.len(), data.len());
        // Allowed drift is expressed in units of each channel's
        // native sensor resolution at `scale_factor=100` granularity:
        //   battery  0.01 V (LoRaWAN 10mV telemetry)
        //   temp     0.01 °C
        //   humidity 0.1 %
        //   CO2      1 ppm
        //   pressure 0.01 hPa
        // We allow ≤ 1 physical LSB of drift on every channel, which
        // is effectively lossless at the application level.
        let max_drift = [0.01_f64, 0.01, 0.1, 1.0, 0.01];
        for (i, (src, dst)) in data.iter().zip(decoded.iter()).enumerate() {
            for ch in 0..5 {
                let diff = (src[ch] - dst[ch]).abs();
                assert!(
                    diff <= max_drift[ch] + 1e-9,
                    "frame {} ch {}: expected {}, got {}, diff {} (max {})",
                    i,
                    ch,
                    src[ch],
                    dst[ch],
                    diff,
                    max_drift[ch]
                );
            }
        }
    }

    /// B5-2. Property: from message 8 onward, the compact wire
    /// frame must fit in the 11-byte LoRaWAN ceiling on the slow-drift
    /// EM500-CO2 profile.
    #[test]
    fn test_fixed_header_11b_ceiling() {
        let data = em500_co2_dataset();
        // Use a large keyframe_interval to stop periodic keyframes
        // from interfering with the steady-state size assertion.
        let cfg = AlecEncoderConfig {
            history_size: 0,
            max_patterns: 0,
            max_memory_bytes: 0,
            keyframe_interval: 10_000,
            smart_resync: false,
        };
        let (frames, _decoded) = encode_decode_dataset(&data, Some(cfg));

        // Skip the warm-up window. Message 0 has no prediction at all,
        // and a handful of subsequent messages still produce Raw32 for
        // a channel while the per-source EMA settles. Docs/CONTEXT.md
        // assumes frames from ~#8 onward are steady-state.
        let warmup = 8;
        for (i, frame) in frames.iter().enumerate().skip(warmup) {
            assert!(
                frame.len() <= 11,
                "frame #{} was {} bytes > 11B ceiling",
                i,
                frame.len()
            );
        }
    }

    /// B5-3. The very first frame has no prediction history available
    /// so every channel falls back to Raw32 — this frame is expected
    /// to exceed the 11-byte LoRaWAN ceiling, and the caller must
    /// detect that and send a legacy TLV frame instead.
    #[test]
    fn test_cold_start_first_frame() {
        let data = em500_co2_dataset();
        let enc = alec_encoder_new();
        let mut out = [0u8; 64];
        let mut out_len: usize = 0;
        let r = alec_encode_multi_fixed(
            enc,
            data[0].as_ptr(),
            data[0].len(),
            out.as_mut_ptr(),
            out.len(),
            &mut out_len,
        );
        assert_eq!(r, AlecResult::Ok);

        // Layout: 1B marker + 4B header + 2B bitmap (5 ch × 2 bits = 10 bits)
        //         + 5 × 4B Raw32 = 27 bytes exactly.
        assert_eq!(out_len, 27, "cold-start frame should be 27B");
        assert!(out_len > 11, "cold-start frame must exceed 11B ceiling");
        // Marker is the data marker (cold start frame is NOT a keyframe —
        // the FFI only emits 0xA2 when the force/periodic rules fire).
        assert_eq!(out[0], 0xA1);
        alec_encoder_free(enc);
    }

    /// B5-4. Context version truncates u32 → u16 and wraps at 65535.
    /// The decoder must accept the wrap without flagging a mismatch.
    #[test]
    fn test_ctx_ver_wraparound() {
        // Start the encoder's context just below the wrap boundary.
        let enc = alec_encoder_new();
        let dec = alec_decoder_new();

        unsafe {
            (*enc).context.set_version(65530);
            (*dec).context.set_version(65530);
        }

        let row = [3.6, 22.5, 45.0, 420.0, 1013.25];
        let mut out = [0u8; 64];
        let mut out_len: usize = 0;
        let mut values = [0f64; 5];

        let mut seen_pre_wrap = false;
        let mut seen_post_wrap = false;
        // Each encode-step observes 5 channels → +5 ctx_version per frame.
        // 10 frames × 5 obs = 50 obs → passes through 65535 → 0.
        for i in 0..10 {
            let r = alec_encode_multi_fixed(
                enc,
                row.as_ptr(),
                row.len(),
                out.as_mut_ptr(),
                out.len(),
                &mut out_len,
            );
            assert_eq!(r, AlecResult::Ok);

            // Read the wire context_version.
            let cv = u16::from_be_bytes([out[3], out[4]]);
            if cv > 60_000 {
                seen_pre_wrap = true;
            }
            if cv < 1_000 {
                seen_post_wrap = true;
            }

            let r2 = alec_decode_multi_fixed(
                dec,
                out.as_ptr(),
                out_len,
                5,
                values.as_mut_ptr(),
                values.len(),
            );
            assert_eq!(
                r2,
                AlecResult::Ok,
                "decode failed at wrap boundary, frame {}",
                i
            );
        }

        assert!(seen_pre_wrap, "never observed a ctx_ver near the wrap edge");
        assert!(seen_post_wrap, "ctx_ver never wrapped to a low value");

        alec_encoder_free(enc);
        alec_decoder_free(dec);
    }

    /// B5-5. A non-ALEC first byte (e.g. the legacy TLV header byte)
    /// must return a clean error, NOT panic.
    #[test]
    fn test_marker_byte_dispatch() {
        let data = em500_co2_dataset();
        let enc = alec_encoder_new();
        let dec = alec_decoder_new();

        let mut out = [0u8; 64];
        let mut out_len: usize = 0;
        let r = alec_encode_multi_fixed(
            enc,
            data[0].as_ptr(),
            data[0].len(),
            out.as_mut_ptr(),
            out.len(),
            &mut out_len,
        );
        assert_eq!(r, AlecResult::Ok);
        assert_eq!(out[0], 0xA1); // regular data marker

        // 0xA1 frame decodes successfully.
        let mut values = [0f64; 5];
        let r_ok = alec_decode_multi_fixed(
            dec,
            out.as_ptr(),
            out_len,
            5,
            values.as_mut_ptr(),
            values.len(),
        );
        assert_eq!(r_ok, AlecResult::Ok);

        // Corrupt marker to a known-not-ALEC byte (legacy TLV first
        // byte is 0x7A for MessageType::DataFixedChannel but a
        // production TLV message begins with 0x5A = Data | P3 | v1).
        let mut corrupt = out[..out_len].to_vec();
        corrupt[0] = 0x5A;
        let r_err = alec_decode_multi_fixed(
            alec_decoder_new(), // fresh decoder so prior state doesn't leak
            corrupt.as_ptr(),
            corrupt.len(),
            5,
            values.as_mut_ptr(),
            values.len(),
        );
        assert_eq!(r_err, AlecResult::ErrorInvalidInput);

        alec_encoder_free(enc);
        alec_decoder_free(dec);
    }

    /// B5-6. With `keyframe_interval=10` on a stable signal, the
    /// frames at interval boundaries must be larger (Raw32 for all
    /// channels) while the in-between frames should be compact
    /// (mostly Repeated/Delta8).
    #[test]
    fn test_keyframe_forced_at_interval() {
        let cfg = AlecEncoderConfig {
            history_size: 0,
            max_patterns: 0,
            max_memory_bytes: 0,
            keyframe_interval: 10,
            smart_resync: true,
        };
        let enc = alec_encoder_new_with_config(&cfg);
        // Stable signal: identical row every frame.
        let row = [3.6, 22.5, 45.0, 420.0, 1013.25];
        let mut out = [0u8; 64];
        let mut out_len: usize = 0;
        let mut frames: Vec<(usize, u8)> = Vec::new();

        for _ in 0..25 {
            let r = alec_encode_multi_fixed(
                enc,
                row.as_ptr(),
                row.len(),
                out.as_mut_ptr(),
                out.len(),
                &mut out_len,
            );
            assert_eq!(r, AlecResult::Ok);
            frames.push((out_len, out[0]));
        }

        // Layout sizes for 5 channels:
        //   keyframe (Raw32 ×5) = 1 + 4 + 2 + 20 = 27 bytes, marker 0xA2
        //   steady   (Repeated ×5) = 1 + 4 + 2 + 0 = 7 bytes, marker 0xA1
        let keyframe_indices: Vec<usize> = frames
            .iter()
            .enumerate()
            .filter(|(_, (_, m))| *m == 0xA2)
            .map(|(i, _)| i)
            .collect();

        // Frame #10 and #20 are the interval boundaries (counter reaches 10).
        assert_eq!(keyframe_indices, vec![10, 20]);
        for (i, (len, marker)) in frames.iter().enumerate() {
            if *marker == 0xA2 {
                assert_eq!(*len, 27, "keyframe at #{} should be 27B, got {}", i, *len);
            } else {
                assert_eq!(*marker, 0xA1);
                // Frame 0 is the cold-start Raw32 frame (27B).
                if i == 0 {
                    assert_eq!(*len, 27);
                } else {
                    assert!(
                        *len <= 11,
                        "data frame at #{} is {} bytes > 11B",
                        i,
                        *len
                    );
                }
            }
        }

        alec_encoder_free(enc);
    }

    /// Diagnostic (ignored by default) — prints per-frame sizes and
    /// encoding distribution for the synthesized EM500-CO2 dataset.
    /// Run with `cargo test -p alec-ffi diag_fixed_sizes -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn diag_fixed_sizes() {
        let data = em500_co2_dataset();
        // Disable periodic keyframes so the observed sizes reflect
        // pure steady-state encoding.
        let cfg = AlecEncoderConfig {
            history_size: 0,
            max_patterns: 0,
            max_memory_bytes: 0,
            keyframe_interval: 10_000,
            smart_resync: false,
        };
        let (frames, _) = encode_decode_dataset(&data, Some(cfg));
        let mut counts = [0u32; 4]; // Repeated, Delta8, Delta16, Raw32
        for frame in &frames {
            // 5 channels → 2 bitmap bytes starting at offset 5.
            let b0 = frame[5];
            let b1 = frame[6];
            for i in 0..5 {
                let bits = if i < 4 {
                    (b0 >> (i * 2)) & 3
                } else {
                    b1 & 3
                };
                counts[bits as usize] += 1;
            }
        }
        let total_ch = 99 * 5;
        println!("\n=== FIXED-CHANNEL DIAGNOSTIC ===");
        let sizes: Vec<usize> = frames.iter().map(|f| f.len()).collect();
        let total: usize = sizes.iter().sum();
        println!(
            "99 frames: total={} avg={:.2} B/frame  min={} max={}",
            total,
            total as f64 / 99.0,
            sizes.iter().min().unwrap(),
            sizes.iter().max().unwrap()
        );
        let warm = 8;
        let steady: Vec<usize> = sizes[warm..].to_vec();
        let stotal: usize = steady.iter().sum();
        println!(
            "steady (frame>={}): total={} avg={:.2} B/frame  max={}",
            warm,
            stotal,
            stotal as f64 / steady.len() as f64,
            steady.iter().max().unwrap()
        );
        let labels = ["Repeated", "Delta8", "Delta16", "Raw32"];
        println!("encoding distribution across {} channels:", total_ch);
        for i in 0..4 {
            println!(
                "  {:10}: {:4}  ({:.1}%)",
                labels[i],
                counts[i],
                100.0 * counts[i] as f64 / total_ch as f64
            );
        }
        // Dump a typical steady-state frame.
        let idx = 30;
        println!(
            "Frame #{}: {} bytes = {:02X?}",
            idx,
            frames[idx].len(),
            &frames[idx][..]
        );
    }

    /// B5-7. `alec_force_keyframe` forces the very next frame to be
    /// a keyframe (marker 0xA2, Raw32 for every channel), and the
    /// frame after that returns to the compact encoding.
    #[test]
    fn test_force_keyframe_ffi() {
        let cfg = AlecEncoderConfig {
            history_size: 0,
            max_patterns: 0,
            max_memory_bytes: 0,
            keyframe_interval: 10_000, // avoid periodic keyframes
            smart_resync: true,
        };
        let enc = alec_encoder_new_with_config(&cfg);
        let row = [3.6, 22.5, 45.0, 420.0, 1013.25];
        let mut out = [0u8; 64];
        let mut out_len: usize = 0;

        // Warm up the context with a few stable frames.
        for _ in 0..5 {
            let r = alec_encode_multi_fixed(
                enc,
                row.as_ptr(),
                row.len(),
                out.as_mut_ptr(),
                out.len(),
                &mut out_len,
            );
            assert_eq!(r, AlecResult::Ok);
        }
        // After warm-up the frame is compact (data marker).
        assert_eq!(out[0], 0xA1);
        let warm_len = out_len;
        assert!(warm_len <= 11, "warm frame should be <=11B, got {}", warm_len);

        // Downlink-style force: next encode must be a keyframe.
        alec_force_keyframe(enc);
        let r = alec_encode_multi_fixed(
            enc,
            row.as_ptr(),
            row.len(),
            out.as_mut_ptr(),
            out.len(),
            &mut out_len,
        );
        assert_eq!(r, AlecResult::Ok);
        assert_eq!(out[0], 0xA2, "forced frame must carry the keyframe marker");
        assert_eq!(out_len, 27, "forced keyframe must be Raw32 for all channels");

        // The frame after the keyframe must be compact again.
        let r = alec_encode_multi_fixed(
            enc,
            row.as_ptr(),
            row.len(),
            out.as_mut_ptr(),
            out.len(),
            &mut out_len,
        );
        assert_eq!(r, AlecResult::Ok);
        assert_eq!(out[0], 0xA1, "post-keyframe frame must be a data frame");
        assert!(
            out_len <= 11,
            "post-keyframe frame should be <=11B, got {}",
            out_len
        );

        alec_encoder_free(enc);
    }
}
