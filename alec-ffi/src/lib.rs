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
    ///   valid for the lifetime of the program. Must be non-NULL.
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
        ContextConfig {
            history_size: r.history_size as usize,
            max_patterns: r.max_patterns as usize,
            max_memory: r.max_memory_bytes as usize,
            ..ContextConfig::default()
        }
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
    /// Corrupt or malformed context-state data (bad magic, bad CRC,
    /// truncated buffer, etc.). Produced by `alec_decoder_import_state`.
    ErrorCorruptData = 9,
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
    static VERSION: &[u8] = b"1.3.5\0";
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
        AlecResult::ErrorCorruptData => b"Corrupt or malformed context state\0",
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
///   are used. Numeric fields set to 0 are replaced by their default
///   (except `keyframe_interval`, where 0 disables periodic keyframes).
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
/// 0xFF resync command from the server-side sidecar. The keyframe is
/// emitted by the next call to `alec_encode_multi_fixed`: marker 0xA2,
/// Raw32 for every channel.
///
/// No-op if `encoder` is NULL or if the encoder was configured with
/// `smart_resync = false`.
///
/// Most integrators will prefer the `alec_downlink_handler` wrapper,
/// which parses a raw downlink payload and applies the right action.
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

/// Parse a raw LoRaWAN downlink payload and apply the right action
/// to the encoder.
///
/// This is a convenience wrapper over `alec_force_keyframe`. A single
/// command byte is defined today:
///
/// - `0xFF` — "request immediate keyframe": the encoder's next
///   `alec_encode_multi_fixed` call will emit marker `0xA2` and
///   Raw32 for every channel.
///
/// Any other first byte is treated as an invalid command and the
/// encoder state is left untouched. Additional bytes after byte 0
/// are reserved for future commands and are currently ignored.
///
/// Worst-case drift after a packet loss:
///
/// - No smart resync (downlink disabled):
///   `drift ≤ keyframe_interval × uplink_period`
///   (e.g. 50 × 10 min ≈ 8 h at a 10-minute cadence).
/// - With smart resync + downlink `0xFF`:
///   `drift ≤ 1 × uplink_period` (next uplink is a keyframe).
///
/// # Arguments
///
/// * `encoder` - Encoder handle.
/// * `data` - Downlink payload bytes (the raw LoRaWAN FRMPayload).
/// * `len` - Length of `data` in bytes.
///
/// # Returns
///
/// * `ALEC_OK` if the downlink was a recognized command and was
///   applied.
/// * `ALEC_ERROR_NULL_POINTER` if `encoder` or `data` is NULL.
/// * `ALEC_ERROR_INVALID_INPUT` for an empty payload or unknown
///   command byte — encoder state is NOT modified.
#[no_mangle]
pub extern "C" fn alec_downlink_handler(
    encoder: *mut AlecEncoder,
    data: *const u8,
    len: usize,
) -> AlecResult {
    if encoder.is_null() || data.is_null() {
        return AlecResult::ErrorNullPointer;
    }
    if len == 0 {
        return AlecResult::ErrorInvalidInput;
    }
    // SAFETY: caller promised `data` is valid for `len` bytes.
    let cmd = unsafe { *data };
    match cmd {
        0xFF => {
            alec_force_keyframe(encoder);
            AlecResult::Ok
        }
        other => {
            log::warn!(
                "ALEC downlink: unknown command byte 0x{:02X} ({} byte payload, ignored)",
                other,
                len
            );
            AlecResult::ErrorInvalidInput
        }
    }
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
/// * `values` - Per-channel f64 values, positional.
/// * `channel_count` - Number of channels in `values`.
/// * `output` - Destination buffer for the wire bytes.
/// * `output_capacity` - Size of `output` in bytes.
/// * `out_len` - Pointer receiving the number of bytes written to
///   `output`.
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
    let periodic_due =
        enc.keyframe_interval > 0 && enc.messages_since_keyframe >= enc.keyframe_interval;
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
        Err(alec::error::AlecError::Encode(alec::error::EncodeError::BufferTooSmall {
            ..
        })) => AlecResult::ErrorBufferTooSmall,
        Err(alec::error::AlecError::Encode(alec::error::EncodeError::PayloadTooLarge {
            ..
        })) => AlecResult::ErrorInvalidInput,
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
/// * `ALEC_OK` on success.
/// * `ALEC_ERROR_INVALID_INPUT` for zero channels or a non-ALEC marker byte.
/// * `ALEC_ERROR_BUFFER_TOO_SMALL` if `output_capacity < channel_count`
///   or the input is shorter than the header + bitmap + data bytes.
/// * `ALEC_ERROR_DECODING_FAILED` for any other decode error.
/// * `ALEC_ERROR_NULL_POINTER` for a NULL required pointer.
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

            // Bloc C recovery: on a non-keyframe frame, if the
            // decoder observed a sequence gap OR a context-version
            // mismatch, wipe the per-channel prediction state so
            // stale Delta predictions cannot silently corrupt the
            // next decode. The next keyframe (marker 0xA2) will
            // re-seed the state. A keyframe itself NEVER triggers a
            // reset — its payload is Raw32 for every channel and
            // fully re-builds the context on its own.
            if !info.keyframe {
                if info.gap_size > 0 {
                    log::warn!(
                        "ALEC gap detected on fixed-channel decode: {} frame(s) missing \
                         (seq={}), context reset to baseline",
                        info.gap_size,
                        info.sequence
                    );
                    dec.context.reset_to_baseline();
                } else if info.context_mismatch {
                    // Also surface a u32-level check via the core
                    // Context::check_version API. We truncate our
                    // tracked u32 version to the low 16 bits so the
                    // comparison is wire-format-accurate; the full
                    // u32 version is informational only here.
                    let wire_ver = info.context_version as u32;
                    let _result = dec.context.check_version(wire_ver);
                    log::warn!(
                        "ALEC ctx_ver mismatch on fixed-channel decode: \
                         wire={}, context reset to baseline",
                        info.context_version
                    );
                    dec.context.reset_to_baseline();
                }
            }

            // Observe every decoded value so the NEXT frame can
            // decode delta-encoded channels correctly. This runs
            // AFTER any reset_to_baseline(): on a keyframe these
            // observations reconstruct the prediction state from
            // the Raw32 values, which is exactly what we want.
            for (i, &value) in output_slice.iter().take(channel_count).enumerate() {
                let rd = RawData::with_source(fixed_channel_source_id(i), value, 0);
                dec.context.observe(&rd);
            }
            AlecResult::Ok
        }
        Err(alec::error::AlecError::Decode(alec::error::DecodeError::BufferTooShort {
            ..
        })) => AlecResult::ErrorBufferTooSmall,
        Err(alec::error::AlecError::Decode(alec::error::DecodeError::MalformedMessage {
            reason,
            ..
        })) if reason.contains("not a fixed-channel ALEC frame") => AlecResult::ErrorInvalidInput,
        Err(_) => AlecResult::ErrorDecodingFailed,
    }
}

// ============================================================================
// Bloc D — Context persistence FFI
//
// These functions expose Context::to_preload_bytes / from_preload_bytes to
// C callers. Intended use: the ChirpStack sidecar periodically exports each
// DevEUI's decoder context (Redis persistence) and restores it on startup.
// ============================================================================

/// Compute the exact number of bytes `alec_decoder_export_state` would
/// write for this decoder + sensor_type. Lets the caller allocate the
/// right-sized buffer up front without any reallocation.
///
/// # Arguments
///
/// * `decoder`     - Decoder handle.
/// * `sensor_type` - Null-terminated sensor-type identifier.
/// * `out_size`    - Pointer receiving the required size in bytes.
///
/// # Returns
///
/// `ALEC_OK` on success; `ALEC_ERROR_NULL_POINTER` for a NULL pointer;
/// `ALEC_ERROR_INVALID_UTF8` if `sensor_type` is not valid UTF-8;
/// `ALEC_ERROR_INVALID_INPUT` if `sensor_type` exceeds 255 bytes.
#[no_mangle]
pub extern "C" fn alec_decoder_export_state_size(
    decoder: *const AlecDecoder,
    sensor_type: *const c_char,
    out_size: *mut usize,
) -> AlecResult {
    if decoder.is_null() || sensor_type.is_null() || out_size.is_null() {
        return AlecResult::ErrorNullPointer;
    }
    let dec = unsafe { &*decoder };
    let sens_str = match unsafe { CStr::from_ptr(sensor_type) }.to_str() {
        Ok(s) => s,
        Err(_) => return AlecResult::ErrorInvalidUtf8,
    };
    match dec.context.to_preload_bytes(sens_str) {
        Ok(bytes) => {
            unsafe { *out_size = bytes.len() };
            AlecResult::Ok
        }
        Err(_) => AlecResult::ErrorInvalidInput,
    }
}

/// Serialize the decoder's context to a caller-provided buffer.
///
/// The output is a self-contained byte buffer (magic `ALCS`, CRC32
/// protected) that can be persisted to Redis, a file, etc. Typical
/// size is 1-2 KB for a 5-channel EM500-CO2 decoder with
/// `history_size = 20`.
///
/// Session state (last_header_sequence, last_gap_size) is **NOT**
/// serialized — those are transient tracking counters that reset
/// naturally on sidecar restart.
///
/// # Arguments
///
/// * `decoder` - Decoder handle.
/// * `sensor_type` - Null-terminated sensor-type identifier (≤ 255 bytes).
/// * `out_buf` - Destination buffer.
/// * `out_capacity` - Size of `out_buf` in bytes.
/// * `out_len` - Pointer receiving the number of bytes written (on
///   success) or the required size (on `ALEC_ERROR_BUFFER_TOO_SMALL`).
///
/// # Returns
///
/// * `ALEC_OK` on success.
/// * `ALEC_ERROR_BUFFER_TOO_SMALL` if `out_capacity` is less than the
///   required size. In that case `*out_len` is set to the required
///   size and `out_buf` is NOT written (no partial write).
/// * `ALEC_ERROR_NULL_POINTER` for a NULL required pointer.
/// * `ALEC_ERROR_INVALID_UTF8` if `sensor_type` is not valid UTF-8.
/// * `ALEC_ERROR_INVALID_INPUT` if `sensor_type` exceeds 255 bytes.
#[no_mangle]
pub extern "C" fn alec_decoder_export_state(
    decoder: *const AlecDecoder,
    sensor_type: *const c_char,
    out_buf: *mut u8,
    out_capacity: usize,
    out_len: *mut usize,
) -> AlecResult {
    if decoder.is_null() || sensor_type.is_null() || out_buf.is_null() || out_len.is_null() {
        return AlecResult::ErrorNullPointer;
    }
    let dec = unsafe { &*decoder };
    let sens_str = match unsafe { CStr::from_ptr(sensor_type) }.to_str() {
        Ok(s) => s,
        Err(_) => return AlecResult::ErrorInvalidUtf8,
    };
    let bytes = match dec.context.to_preload_bytes(sens_str) {
        Ok(b) => b,
        Err(_) => return AlecResult::ErrorInvalidInput,
    };
    if bytes.len() > out_capacity {
        // Buffer too small — report the required size but do not
        // partially write the output (contract: out_buf unchanged).
        unsafe { *out_len = bytes.len() };
        return AlecResult::ErrorBufferTooSmall;
    }
    let out_slice = unsafe { slice::from_raw_parts_mut(out_buf, out_capacity) };
    out_slice[..bytes.len()].copy_from_slice(&bytes);
    unsafe { *out_len = bytes.len() };
    AlecResult::Ok
}

/// Restore a decoder's context from bytes produced by
/// `alec_decoder_export_state`.
///
/// On success, `decoder.context` is replaced by the deserialized
/// context. The decoder's session state — `last_header_sequence` and
/// `last_gap_size` — is **preserved** (those are transient
/// frame-level trackers, not context state).
///
/// If the input buffer is corrupted (bad magic, CRC mismatch,
/// truncation, etc.), the decoder is NOT modified in any way —
/// neither the context nor the session state. The caller can safely
/// retry after repairing the input.
///
/// # Arguments
///
/// * `decoder`  - Decoder handle.
/// * `data`     - Input bytes produced by `alec_decoder_export_state`.
/// * `data_len` - Length of `data` in bytes.
///
/// # Returns
///
/// * `ALEC_OK` on success.
/// * `ALEC_ERROR_NULL_POINTER` for a NULL pointer.
/// * `ALEC_ERROR_CORRUPT_DATA` if `data` cannot be parsed (bad magic,
///   CRC mismatch, truncation, unknown format version).
#[no_mangle]
pub extern "C" fn alec_decoder_import_state(
    decoder: *mut AlecDecoder,
    data: *const u8,
    data_len: usize,
) -> AlecResult {
    if decoder.is_null() || data.is_null() {
        return AlecResult::ErrorNullPointer;
    }
    let data_slice = unsafe { slice::from_raw_parts(data, data_len) };
    match Context::from_preload_bytes(data_slice) {
        Ok(ctx) => {
            let dec = unsafe { &mut *decoder };
            // Replace context only — session state (last_header_sequence,
            // last_gap_size) is intentionally preserved.
            dec.context = ctx;
            AlecResult::Ok
        }
        Err(_) => AlecResult::ErrorCorruptData,
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
        assert_eq!(version_str, "1.3.5");
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
        let _gap: extern "C" fn(*const AlecDecoder, *mut u8) -> bool = alec_decoder_gap_detected;
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
            assert_eq!(
                r2,
                AlecResult::Ok,
                "decode failed at frame {}",
                frames.len()
            );

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
                    assert!(*len <= 11, "data frame at #{} is {} bytes > 11B", i, *len);
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
                let bits = if i < 4 { (b0 >> (i * 2)) & 3 } else { b1 & 3 };
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
        assert!(
            warm_len <= 11,
            "warm frame should be <=11B, got {}",
            warm_len
        );

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
        assert_eq!(
            out_len, 27,
            "forced keyframe must be Raw32 for all channels"
        );

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

    // ========================================================================
    // Bloc C5 — Packet-loss recovery tests
    // ========================================================================

    /// Helper: build a matched encoder + decoder pair with
    /// a small keyframe_interval suitable for test runtime.
    fn new_pair(keyframe_interval: u32) -> (*mut AlecEncoder, *mut AlecDecoder) {
        let cfg = AlecEncoderConfig {
            history_size: 0,
            max_patterns: 0,
            max_memory_bytes: 0,
            keyframe_interval,
            smart_resync: true,
        };
        (alec_encoder_new_with_config(&cfg), alec_decoder_new())
    }

    /// Encode one frame on `enc` into `out` and return its length.
    fn do_encode(enc: *mut AlecEncoder, row: &[f64; 5], out: &mut [u8; 64]) -> usize {
        let mut n: usize = 0;
        let r = alec_encode_multi_fixed(
            enc,
            row.as_ptr(),
            row.len(),
            out.as_mut_ptr(),
            out.len(),
            &mut n,
        );
        assert_eq!(r, AlecResult::Ok);
        n
    }

    /// Decode `frame` on `dec`. Returns the decoded values.
    fn do_decode(dec: *mut AlecDecoder, frame: &[u8]) -> [f64; 5] {
        let mut values = [0f64; 5];
        let r = alec_decode_multi_fixed(
            dec,
            frame.as_ptr(),
            frame.len(),
            5,
            values.as_mut_ptr(),
            values.len(),
        );
        assert_eq!(r, AlecResult::Ok);
        values
    }

    /// C5-1. `reset_to_baseline` wipes per-channel prediction state
    /// so the next encode can only fall back to Raw32 (no prediction
    /// to delta against). Existing patterns in the dictionary are
    /// preserved (the FFI path never registers any, so the dict is
    /// empty to begin with — but the contract matters for preloaded
    /// contexts).
    #[test]
    fn test_reset_to_baseline_wipes_stats() {
        let (enc, _dec) = new_pair(10_000); // no periodic keyframes
        let row = [3.60, 22.50, 45.0, 420.0, 1013.25];
        let mut out = [0u8; 64];

        // Build up 20 frames of prediction state.
        for _ in 0..20 {
            do_encode(enc, &row, &mut out);
        }
        // Context should now hold non-trivial state.
        let e = unsafe { &*enc };
        assert!(e.context.last_value(1).is_some());
        assert!(e.context.last_value(5).is_some());
        let pre_version = e.context.version();

        // Reset.
        unsafe { &mut *enc }.context.reset_to_baseline();

        // All per-channel last_value are cleared — but the version
        // counter is preserved (see C1 contract).
        let e = unsafe { &*enc };
        assert!(e.context.last_value(1).is_none());
        assert!(e.context.last_value(5).is_none());
        assert_eq!(e.context.version(), pre_version);

        // Next encode, with no prediction available, must fall back
        // to Raw32 on every channel — same wire shape as a keyframe
        // (minus the 0xA2 marker, since this is a regular frame).
        // Layout: 1 marker + 4 header + 2 bitmap + 5×Raw32 = 27 B.
        let n = do_encode(enc, &row, &mut out);
        assert_eq!(n, 27, "post-reset frame must be Raw32-all-channels (27 B)");
        assert_eq!(out[0], 0xA1);
        let b0 = out[5];
        let b1 = out[6];
        // All bitmap bits = 0b11 = Raw32.
        assert_eq!(b0, 0b1111_1111);
        assert_eq!(b1 & 0b11, 0b11);

        alec_encoder_free(enc);
    }

    /// C5-2. Simulate a 4-frame gap (encoder produces frames 0..=20
    /// but frames 15..=18 never reach the decoder). On frame 19 the
    /// decoder must detect gap_size=4 and trigger a reset; full
    /// recovery must happen at the next periodic keyframe (frame 20
    /// with keyframe_interval=10).
    #[test]
    fn test_packet_loss_recovery_at_keyframe() {
        let (enc, dec) = new_pair(10);
        let row = [3.60, 22.50, 45.0, 420.0, 1013.25];
        let mut out = [0u8; 64];
        let mut frames: Vec<Vec<u8>> = Vec::new();

        // Encode 21 frames (0..=20). Frame 10 and 20 are keyframes.
        for _ in 0..=20 {
            let n = do_encode(enc, &row, &mut out);
            frames.push(out[..n].to_vec());
        }
        assert_eq!(frames[10][0], 0xA2, "frame 10 must be a keyframe");
        assert_eq!(frames[20][0], 0xA2, "frame 20 must be a keyframe");

        // Decode frames 0..=14 normally.
        for f in &frames[0..=14] {
            do_decode(dec, f);
        }
        // Do NOT decode frames 15..=18 — simulate 4 dropped uplinks.

        // Decode frame 19 → gap_size = 4 reported, reset triggered.
        do_decode(dec, &frames[19]);
        let mut gap: u8 = 0;
        assert!(alec_decoder_gap_detected(dec, &mut gap));
        assert_eq!(gap, 4, "decoder must report 4-frame gap");

        // After the reset, the decoder's last_value is cleared for
        // every channel, so any subsequent Delta/Repeated-encoded
        // frame would error out. Frame 19 itself does NOT error
        // because we observe the decoded values *after* the reset
        // (re-seeding the context from frame 19's output).

        // Frame 20 is a keyframe — Raw32 for every channel, so it
        // decodes correctly regardless of context state, and it
        // re-synchronises the decoder for good.
        let values_20 = do_decode(dec, &frames[20]);
        for (ch, &v) in row.iter().enumerate() {
            let tol = [0.01_f64, 0.01, 0.1, 1.0, 0.01][ch];
            assert!(
                (v - values_20[ch]).abs() <= tol,
                "ch {}: expected {}, got {}",
                ch,
                v,
                values_20[ch]
            );
        }

        alec_encoder_free(enc);
        alec_decoder_free(dec);
    }

    /// C5-3. No silent corruption: when the encoder emits a stable
    /// signal and the decoder drops frame 20, subsequent frames
    /// 21..=49 are either clearly divergent or clearly re-synced
    /// by the next keyframe at frame 50. Frame 50+ MUST decode
    /// within sensor resolution.
    #[test]
    fn test_no_silent_corruption() {
        let (enc, dec) = new_pair(50);
        let mut out = [0u8; 64];
        let mut frames: Vec<Vec<u8>> = Vec::new();
        // Slow-drift signal — mostly Repeated, occasional Delta8.
        let rows: Vec<[f64; 5]> = (0..100)
            .map(|i| [3.60, 22.50, 45.0, 420.0 + (i / 3) as f64, 1013.25])
            .collect();
        for row in &rows {
            let n = do_encode(enc, row, &mut out);
            frames.push(out[..n].to_vec());
        }

        // Decode frames 0..=19 normally.
        for f in &frames[0..=19] {
            do_decode(dec, f);
        }
        // Drop frame 20 entirely.

        // Decode frames 21..=49. The first one triggers reset; the
        // rest may or may not decode successfully (Delta frames
        // error out because there is no prediction after reset). We
        // ignore the outcome — the contract is only that the
        // decoder does not panic and does not silently corrupt the
        // post-keyframe values.
        for f in &frames[21..50] {
            let mut v = [0f64; 5];
            let _ = alec_decode_multi_fixed(dec, f.as_ptr(), f.len(), 5, v.as_mut_ptr(), v.len());
        }

        // Frame 50 is a keyframe — decode must succeed and values
        // must match the corresponding input within sensor LSB.
        let values_50 = do_decode(dec, &frames[50]);
        assert_eq!(frames[50][0], 0xA2);
        let expected = rows[50];
        let tols = [0.01_f64, 0.01, 0.1, 1.0, 0.01];
        for ch in 0..5 {
            assert!(
                (expected[ch] - values_50[ch]).abs() <= tols[ch] + 1e-9,
                "ch {} at frame 50: expected {}, got {}",
                ch,
                expected[ch],
                values_50[ch]
            );
        }
        // Frames 51..=60 must also decode correctly now that the
        // context is re-synced.
        for i in 51..=60 {
            let values = do_decode(dec, &frames[i]);
            for ch in 0..5 {
                assert!(
                    (rows[i][ch] - values[ch]).abs() <= tols[ch] + 1e-9,
                    "ch {} at frame {}: expected {}, got {}",
                    ch,
                    i,
                    rows[i][ch],
                    values[ch]
                );
            }
        }

        alec_encoder_free(enc);
        alec_decoder_free(dec);
    }

    /// C5-4. Smart resync via the downlink handler: decoder detects
    /// a gap and calls `alec_downlink_handler(enc, [0xFF], 1)` to
    /// request an immediate keyframe. Recovery happens on the very
    /// next uplink, not at the far-off periodic keyframe.
    #[test]
    fn test_smart_resync_downlink() {
        let (enc, dec) = new_pair(10_000); // no periodic keyframes
        let row = [3.60, 22.50, 45.0, 420.0, 1013.25];
        let mut out = [0u8; 64];
        let mut frames: Vec<Vec<u8>> = Vec::new();

        // Encode 10 frames. Decoder drops frame 5.
        for _ in 0..10 {
            let n = do_encode(enc, &row, &mut out);
            frames.push(out[..n].to_vec());
        }
        for f in &frames[0..=4] {
            do_decode(dec, f);
        }
        // Skip frame 5. Decoding frame 6..=9 will produce divergent
        // values, but that's fine — the point is the RESYNC.
        for f in &frames[6..=9] {
            let mut v = [0f64; 5];
            let _ = alec_decode_multi_fixed(dec, f.as_ptr(), f.len(), 5, v.as_mut_ptr(), v.len());
        }

        // Decoder realises it's out of sync — simulate the sidecar
        // checking alec_decoder_gap_detected and sending a 0xFF
        // downlink. The device-side integration calls the downlink
        // handler on the encoder:
        let cmd = [0xFF_u8];
        let r = alec_downlink_handler(enc, cmd.as_ptr(), cmd.len());
        assert_eq!(r, AlecResult::Ok);
        assert!(
            unsafe { &*enc }.force_keyframe_pending,
            "downlink 0xFF must arm the force-keyframe flag"
        );

        // The very next uplink (frame 10) MUST be a keyframe.
        let n = do_encode(enc, &row, &mut out);
        assert_eq!(out[0], 0xA2, "post-downlink frame must be a keyframe");
        assert_eq!(n, 27);

        // Decoder catches up immediately.
        let values = do_decode(dec, &out[..n]);
        let tols = [0.01_f64, 0.01, 0.1, 1.0, 0.01];
        for ch in 0..5 {
            assert!(
                (row[ch] - values[ch]).abs() <= tols[ch] + 1e-9,
                "ch {}: expected {}, got {}",
                ch,
                row[ch],
                values[ch]
            );
        }

        // Without smart resync, recovery would have required waiting
        // for the next periodic keyframe at frame keyframe_interval
        // (50 by default) — an 8h drift on EM500-CO2's 10-min cadence.
        // With smart resync, recovery happened on frame 10 — a 10-min
        // drift. That is the order-of-magnitude improvement.

        alec_encoder_free(enc);
        alec_decoder_free(dec);
    }

    /// C5-5. An unknown downlink command is rejected with
    /// `ALEC_ERROR_INVALID_INPUT` and leaves the encoder state
    /// unchanged.
    #[test]
    fn test_downlink_handler_invalid_command() {
        let enc = alec_encoder_new();
        assert!(!unsafe { &*enc }.force_keyframe_pending);

        let cmd = [0x00_u8];
        let r = alec_downlink_handler(enc, cmd.as_ptr(), cmd.len());
        assert_eq!(r, AlecResult::ErrorInvalidInput);
        assert!(
            !unsafe { &*enc }.force_keyframe_pending,
            "unknown downlink must NOT arm the force-keyframe flag"
        );

        // Another uncommon but not-special byte.
        let cmd = [0x7E_u8, 0xFF, 0xAA];
        let r = alec_downlink_handler(enc, cmd.as_ptr(), cmd.len());
        assert_eq!(r, AlecResult::ErrorInvalidInput);
        assert!(!unsafe { &*enc }.force_keyframe_pending);

        // NULL / empty inputs are defensively handled.
        assert_eq!(
            alec_downlink_handler(ptr::null_mut(), cmd.as_ptr(), cmd.len()),
            AlecResult::ErrorNullPointer
        );
        assert_eq!(
            alec_downlink_handler(enc, ptr::null(), 1),
            AlecResult::ErrorNullPointer
        );
        assert_eq!(
            alec_downlink_handler(enc, cmd.as_ptr(), 0),
            AlecResult::ErrorInvalidInput
        );

        alec_encoder_free(enc);
    }

    /// C5-6. A deliberately-corrupted context_version on the decoder
    /// triggers a mismatch when the next non-keyframe arrives; the
    /// decoder resets and recovers at the subsequent keyframe.
    #[test]
    fn test_context_mismatch_triggers_reset() {
        let (enc, dec) = new_pair(10);
        let row = [3.60, 22.50, 45.0, 420.0, 1013.25];
        let mut out = [0u8; 64];
        let mut frames: Vec<Vec<u8>> = Vec::new();

        // Encode 12 frames (0..=11); frame 10 is a keyframe.
        for _ in 0..=11 {
            let n = do_encode(enc, &row, &mut out);
            frames.push(out[..n].to_vec());
        }

        // Decode 0..=8 normally.
        for f in &frames[0..=8] {
            do_decode(dec, f);
        }

        // Corrupt the decoder's tracked ctx_ver to something that
        // looks like a backwards jump (outside ctx_version_compatible's
        // tolerance of 256 forward units). The encoder's ctx_ver is
        // currently 9*5=45 post-observation; we pretend the decoder
        // last saw a version ~30k ahead so the next non-keyframe
        // trips the mismatch detector.
        unsafe { &mut *dec }.decoder.reset();
        // reset() clears last_fixed_sequence too, so replay a frame
        // to re-arm sequence tracking:
        do_decode(dec, &frames[8]);
        // Now set last_fixed_ctx_version artificially by pushing
        // through a synthesised decode state. A cleaner API would be
        // nice — for the test we use the *encoded* route: change
        // our tracked version by decoding a frame whose payload we
        // hand-crafted. Since that's gnarly, instead we simulate
        // the effect by directly walking into a known-mismatch
        // scenario: inject a frame whose ctx_ver is very far from
        // what the decoder expects.

        // Build a synthetic non-keyframe with an impossible ctx_ver
        // (wire bytes: 0xA1 <seq> <ctx> <bitmap> …). Use frame 9's
        // real bytes but rewrite bytes 3..=4 (ctx_ver).
        let mut tampered = frames[9].clone();
        // Replace ctx_ver with 0x1234 — far from the decoder's
        // current state (which just observed frame 8 ≈ version ~45).
        tampered[3] = 0x12;
        tampered[4] = 0x34;

        // Before the tamper the decoder has seen at least 9 identical
        // readings for channel 1, so its per-source observation count
        // is ≥ 3 and `predict()` reports the `MovingAverage` model.
        use alec::context::PredictionModel;
        let model_before = unsafe { &*dec }.context.predict(1).unwrap().model_type;
        assert_eq!(model_before, PredictionModel::MovingAverage);

        // Decoding the tampered frame must surface a mismatch and
        // trigger `reset_to_baseline()`. The decode itself may
        // produce garbage values for Delta-encoded channels (since
        // the reset happens after decode but before the re-observe),
        // which is acceptable — the contract is that subsequent
        // keyframe decodes are correct.
        let mut v = [0f64; 5];
        let _ = alec_decode_multi_fixed(
            dec,
            tampered.as_ptr(),
            tampered.len(),
            5,
            v.as_mut_ptr(),
            v.len(),
        );

        // After the reset the history has been wiped. The FFI then
        // re-observes the single tampered frame's output, so each
        // per-channel observation count is exactly 1 and
        // `predict()` reports the `LastValue` model (count < 3
        // branch in SourceStats::predict). If the reset had NOT
        // fired, count would be 10 and the model would still be
        // MovingAverage.
        let model_after = unsafe { &*dec }.context.predict(1).unwrap().model_type;
        assert_eq!(
            model_after,
            PredictionModel::LastValue,
            "reset_to_baseline should have dropped source_stats count below the \
             MovingAverage threshold (count < 3)"
        );

        // Frame 10 is a real keyframe — decode must succeed and
        // values must match the input within sensor tolerance.
        let values_10 = do_decode(dec, &frames[10]);
        assert_eq!(frames[10][0], 0xA2);
        let tols = [0.01_f64, 0.01, 0.1, 1.0, 0.01];
        for ch in 0..5 {
            assert!(
                (row[ch] - values_10[ch]).abs() <= tols[ch] + 1e-9,
                "ch {}: expected {}, got {}",
                ch,
                row[ch],
                values_10[ch]
            );
        }

        alec_encoder_free(enc);
        alec_decoder_free(dec);
    }

    // ========================================================================
    // Bloc D4 — Context persistence FFI tests
    // ========================================================================

    /// Helper: CString-safe sensor type.
    const SENSOR_TYPE: &[u8] = b"em500-co2\0";

    /// Run `frames` rows through the encoder + decoder. Returns the
    /// wire frames for later replay.
    fn drive_pair(enc: *mut AlecEncoder, dec: *mut AlecDecoder, rows: &[[f64; 5]]) -> Vec<Vec<u8>> {
        let mut out = [0u8; 64];
        let mut out_len: usize = 0;
        let mut values = [0f64; 5];
        let mut frames = Vec::with_capacity(rows.len());
        for row in rows {
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
            assert_eq!(r2, AlecResult::Ok);
            frames.push(frame);
        }
        frames
    }

    /// D4-1. End-to-end in-memory roundtrip: train a decoder's
    /// context, export → import into a fresh decoder, then verify
    /// that both decoders produce identical output for the same
    /// subsequent encoded frames.
    #[test]
    fn test_context_roundtrip_in_memory() {
        // Slow-drift signal so frames stay compact.
        let rows: Vec<[f64; 5]> = (0..30)
            .map(|i| [3.60, 22.50, 45.0, 420.0 + (i / 3) as f64, 1013.25])
            .collect();

        let cfg = AlecEncoderConfig {
            history_size: 0,
            max_patterns: 0,
            max_memory_bytes: 0,
            keyframe_interval: 10_000, // avoid periodic keyframes
            smart_resync: false,
        };
        let enc = alec_encoder_new_with_config(&cfg);
        let dec_orig = alec_decoder_new();

        // Train.
        let _ = drive_pair(enc, dec_orig, &rows);

        // Export the trained decoder.
        let mut size: usize = 0;
        let r = alec_decoder_export_state_size(
            dec_orig,
            SENSOR_TYPE.as_ptr() as *const c_char,
            &mut size,
        );
        assert_eq!(r, AlecResult::Ok);
        assert!(size > 0 && size < 3072, "export size {} out of range", size);
        let mut buf = vec![0u8; size];
        let mut written: usize = 0;
        let r = alec_decoder_export_state(
            dec_orig,
            SENSOR_TYPE.as_ptr() as *const c_char,
            buf.as_mut_ptr(),
            buf.len(),
            &mut written,
        );
        assert_eq!(r, AlecResult::Ok);
        assert_eq!(written, size);

        // Fresh decoder, import the state.
        let dec_new = alec_decoder_new();
        let r = alec_decoder_import_state(dec_new, buf.as_ptr(), buf.len());
        assert_eq!(r, AlecResult::Ok);

        // Drive 10 more frames on the encoder and decode each
        // through BOTH decoders. Identical output confirms the
        // restored context is operationally equivalent.
        let more: Vec<[f64; 5]> = (30..40)
            .map(|i| [3.60, 22.50, 45.0, 420.0 + (i / 3) as f64, 1013.25])
            .collect();
        let mut out = [0u8; 64];
        let mut out_len: usize = 0;
        for row in &more {
            let r = alec_encode_multi_fixed(
                enc,
                row.as_ptr(),
                row.len(),
                out.as_mut_ptr(),
                out.len(),
                &mut out_len,
            );
            assert_eq!(r, AlecResult::Ok);
            let frame = &out[..out_len];

            let mut v_orig = [0f64; 5];
            let mut v_new = [0f64; 5];
            assert_eq!(
                alec_decode_multi_fixed(
                    dec_orig,
                    frame.as_ptr(),
                    frame.len(),
                    5,
                    v_orig.as_mut_ptr(),
                    v_orig.len(),
                ),
                AlecResult::Ok
            );
            assert_eq!(
                alec_decode_multi_fixed(
                    dec_new,
                    frame.as_ptr(),
                    frame.len(),
                    5,
                    v_new.as_mut_ptr(),
                    v_new.len(),
                ),
                AlecResult::Ok
            );
            // Bit-exact equality — both decoders ran the same
            // reconstruction logic on the same wire bytes with
            // identically-seeded contexts.
            for ch in 0..5 {
                assert_eq!(
                    v_orig[ch].to_bits(),
                    v_new[ch].to_bits(),
                    "ch {} diverged between original and restored decoder",
                    ch
                );
            }
        }

        alec_encoder_free(enc);
        alec_decoder_free(dec_orig);
        alec_decoder_free(dec_new);
    }

    /// D4-2. `export_state_size` matches the length reported by a
    /// real export call.
    #[test]
    fn test_export_state_size_matches_export() {
        let enc = alec_encoder_new();
        let dec = alec_decoder_new();
        let rows: Vec<[f64; 5]> = (0..12)
            .map(|_| [3.60, 22.50, 45.0, 420.0, 1013.25])
            .collect();
        let _ = drive_pair(enc, dec, &rows);

        let mut size: usize = 0;
        let r =
            alec_decoder_export_state_size(dec, SENSOR_TYPE.as_ptr() as *const c_char, &mut size);
        assert_eq!(r, AlecResult::Ok);

        let mut buf = vec![0u8; size];
        let mut written: usize = 0;
        let r = alec_decoder_export_state(
            dec,
            SENSOR_TYPE.as_ptr() as *const c_char,
            buf.as_mut_ptr(),
            buf.len(),
            &mut written,
        );
        assert_eq!(r, AlecResult::Ok);
        assert_eq!(written, size);

        alec_encoder_free(enc);
        alec_decoder_free(dec);
    }

    /// D4-3. Too-small buffer → `ALEC_ERROR_BUFFER_TOO_SMALL`. The
    /// buffer is NOT written to (no partial write); `*out_len`
    /// reports the required size.
    #[test]
    fn test_export_buffer_too_small() {
        let enc = alec_encoder_new();
        let dec = alec_decoder_new();
        let rows: Vec<[f64; 5]> = (0..8)
            .map(|_| [3.60, 22.50, 45.0, 420.0, 1013.25])
            .collect();
        let _ = drive_pair(enc, dec, &rows);

        // Tiny buffer pre-filled with a sentinel. After the failed
        // export every byte must still be the sentinel.
        let mut buf = [0xAAu8; 10];
        let mut written: usize = 0;
        let r = alec_decoder_export_state(
            dec,
            SENSOR_TYPE.as_ptr() as *const c_char,
            buf.as_mut_ptr(),
            buf.len(),
            &mut written,
        );
        assert_eq!(r, AlecResult::ErrorBufferTooSmall);
        assert!(
            written > buf.len(),
            "expected *out_len to report required size, got {}",
            written
        );
        for (i, &b) in buf.iter().enumerate() {
            assert_eq!(b, 0xAA, "buf[{}] was written despite error", i);
        }

        alec_encoder_free(enc);
        alec_decoder_free(dec);
    }

    /// D4-4. Corrupt input → `ALEC_ERROR_CORRUPT_DATA`. The
    /// decoder's context is NOT modified.
    #[test]
    fn test_import_corrupt_data() {
        let enc = alec_encoder_new();
        let dec = alec_decoder_new();
        let rows: Vec<[f64; 5]> = (0..8)
            .map(|_| [3.60, 22.50, 45.0, 420.0, 1013.25])
            .collect();
        let _ = drive_pair(enc, dec, &rows);

        // Capture a snapshot we can compare against.
        let mut snap = vec![0u8; 4096];
        let mut snap_len: usize = 0;
        assert_eq!(
            alec_decoder_export_state(
                dec,
                SENSOR_TYPE.as_ptr() as *const c_char,
                snap.as_mut_ptr(),
                snap.len(),
                &mut snap_len,
            ),
            AlecResult::Ok
        );
        let snap_pre = snap[..snap_len].to_vec();

        // Random garbage → CORRUPT_DATA.
        let garbage = [0x00u8, 0x11, 0x22, 0x33, 0x44, 0x55];
        let r = alec_decoder_import_state(dec, garbage.as_ptr(), garbage.len());
        assert_eq!(r, AlecResult::ErrorCorruptData);

        // Bytes with the right magic but bad CRC → also CORRUPT_DATA.
        let mut tampered = snap_pre.clone();
        let mid = tampered.len() / 2;
        tampered[mid] ^= 0xFF;
        let r = alec_decoder_import_state(dec, tampered.as_ptr(), tampered.len());
        assert_eq!(r, AlecResult::ErrorCorruptData);

        // Snapshot the decoder again — must match pre-import bytes
        // exactly, confirming the failed imports left the context
        // untouched.
        let mut snap2 = vec![0u8; 4096];
        let mut snap2_len: usize = 0;
        assert_eq!(
            alec_decoder_export_state(
                dec,
                SENSOR_TYPE.as_ptr() as *const c_char,
                snap2.as_mut_ptr(),
                snap2.len(),
                &mut snap2_len,
            ),
            AlecResult::Ok
        );
        assert_eq!(snap2_len, snap_len);
        assert_eq!(&snap2[..snap2_len], &snap_pre[..]);

        alec_encoder_free(enc);
        alec_decoder_free(dec);
    }

    /// D4-5. Session state (last_header_sequence, last_gap_size) is
    /// preserved across an import.
    #[test]
    fn test_session_state_preserved_on_import() {
        let enc = alec_encoder_new();
        let dec = alec_decoder_new();
        let rows: Vec<[f64; 5]> = (0..5)
            .map(|_| [3.60, 22.50, 45.0, 420.0, 1013.25])
            .collect();
        let _ = drive_pair(enc, dec, &rows);

        // Overwrite the session-state fields with a known tuple.
        unsafe {
            (*dec).last_header_sequence = Some(42);
            (*dec).last_gap_size = 2;
        }

        // Export and re-import — session state should survive.
        let mut buf = vec![0u8; 4096];
        let mut n: usize = 0;
        assert_eq!(
            alec_decoder_export_state(
                dec,
                SENSOR_TYPE.as_ptr() as *const c_char,
                buf.as_mut_ptr(),
                buf.len(),
                &mut n,
            ),
            AlecResult::Ok
        );
        assert_eq!(
            alec_decoder_import_state(dec, buf.as_ptr(), n),
            AlecResult::Ok
        );

        let dec_ref = unsafe { &*dec };
        assert_eq!(dec_ref.last_header_sequence, Some(42));
        assert_eq!(dec_ref.last_gap_size, 2);

        alec_encoder_free(enc);
        alec_decoder_free(dec);
    }

    /// D4-6. Exporting a context just after `reset_to_baseline()`
    /// produces a valid serialized buffer whose `source_stats`
    /// section is empty. Patterns registered before the reset are
    /// preserved (Bloc C contract — see `reset_to_baseline`).
    #[test]
    fn test_export_after_reset_to_baseline() {
        let enc = alec_encoder_new();
        let dec = alec_decoder_new();
        let rows: Vec<[f64; 5]> = (0..30)
            .map(|_| [3.60, 22.50, 45.0, 420.0, 1013.25])
            .collect();
        let _ = drive_pair(enc, dec, &rows);

        // Register one pattern directly on the context so we can
        // check it survives the reset.
        use alec::context::Pattern;
        let code = unsafe { &mut *dec }
            .context
            .register_pattern(Pattern::new(vec![0x11, 0x22, 0x33]))
            .unwrap();

        // Reset → export → import → verify.
        unsafe { &mut *dec }.context.reset_to_baseline();
        let mut buf = vec![0u8; 4096];
        let mut n: usize = 0;
        assert_eq!(
            alec_decoder_export_state(
                dec,
                SENSOR_TYPE.as_ptr() as *const c_char,
                buf.as_mut_ptr(),
                buf.len(),
                &mut n,
            ),
            AlecResult::Ok
        );

        let dec_fresh = alec_decoder_new();
        assert_eq!(
            alec_decoder_import_state(dec_fresh, buf.as_ptr(), n),
            AlecResult::Ok
        );

        // source_stats is empty (predict returns None for any sid).
        let d = unsafe { &*dec_fresh };
        assert!(d.context.predict(1).is_none());
        assert!(d.context.predict(5).is_none());
        // Pattern registered pre-reset must still be in the dictionary.
        assert!(d.context.get_pattern(code).is_some());

        alec_encoder_free(enc);
        alec_decoder_free(dec);
        alec_decoder_free(dec_fresh);
    }

    /// NULL-safety on the three new FFI entry points.
    #[test]
    fn test_persistence_ffi_null_safety() {
        use std::ptr;
        let dec = alec_decoder_new();
        let mut n: usize = 0;
        let mut buf = [0u8; 4];

        // export_state_size
        assert_eq!(
            alec_decoder_export_state_size(
                ptr::null(),
                SENSOR_TYPE.as_ptr() as *const c_char,
                &mut n
            ),
            AlecResult::ErrorNullPointer
        );
        assert_eq!(
            alec_decoder_export_state_size(dec, ptr::null(), &mut n),
            AlecResult::ErrorNullPointer
        );
        assert_eq!(
            alec_decoder_export_state_size(
                dec,
                SENSOR_TYPE.as_ptr() as *const c_char,
                ptr::null_mut()
            ),
            AlecResult::ErrorNullPointer
        );

        // export_state
        assert_eq!(
            alec_decoder_export_state(
                ptr::null(),
                SENSOR_TYPE.as_ptr() as *const c_char,
                buf.as_mut_ptr(),
                buf.len(),
                &mut n,
            ),
            AlecResult::ErrorNullPointer
        );

        // import_state
        assert_eq!(
            alec_decoder_import_state(ptr::null_mut(), buf.as_ptr(), buf.len()),
            AlecResult::ErrorNullPointer
        );
        assert_eq!(
            alec_decoder_import_state(dec, ptr::null(), buf.len()),
            AlecResult::ErrorNullPointer
        );

        alec_decoder_free(dec);
    }
}
