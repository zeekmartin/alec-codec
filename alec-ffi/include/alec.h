/**
 * ALEC - Adaptive Lazy Evolving Compression
 * Copyright (c) 2025 David Martin Venti
 *
 * Dual-licensed under AGPL-3.0 and Commercial License.
 * See LICENSE file for details.
 *
 * C/C++ bindings for ALEC compression library.
 *
 * Example usage:
 *
 *     #include "alec.h"
 *
 *     AlecEncoder* enc = alec_encoder_new();
 *     uint8_t output[256];
 *     size_t output_len;
 *
 *     AlecResult res = alec_encode_value(
 *         enc, 22.5, 0, NULL,
 *         output, sizeof(output), &output_len
 *     );
 *
 *     if (res == ALEC_OK) {
 *         // Use compressed data in output[0..output_len]
 *     }
 *
 *     alec_encoder_free(enc);
 */

#ifndef ALEC_H
#define ALEC_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ============================================================================
 * Types
 * ============================================================================ */

/**
 * Opaque encoder handle.
 *
 * Created with alec_encoder_new(), freed with alec_encoder_free().
 * Do not access internal fields directly.
 */
typedef struct AlecEncoder AlecEncoder;

/**
 * Opaque decoder handle.
 *
 * Created with alec_decoder_new(), freed with alec_decoder_free().
 * Do not access internal fields directly.
 */
typedef struct AlecDecoder AlecDecoder;

/**
 * Result codes for ALEC functions.
 */
typedef enum {
    /** Operation completed successfully */
    ALEC_OK = 0,
    /** Invalid input data provided */
    ALEC_ERROR_INVALID_INPUT = 1,
    /** Output buffer is too small */
    ALEC_ERROR_BUFFER_TOO_SMALL = 2,
    /** Encoding operation failed */
    ALEC_ERROR_ENCODING_FAILED = 3,
    /** Decoding operation failed */
    ALEC_ERROR_DECODING_FAILED = 4,
    /** Null pointer was provided */
    ALEC_ERROR_NULL_POINTER = 5,
    /** Invalid UTF-8 string */
    ALEC_ERROR_INVALID_UTF8 = 6,
    /** File I/O error */
    ALEC_ERROR_FILE_IO = 7,
    /** Context version mismatch */
    ALEC_ERROR_VERSION_MISMATCH = 8,
    /** Corrupt or malformed context-state data (bad magic, bad CRC,
     *  truncated buffer, unknown format version). Produced by
     *  alec_decoder_import_state. */
    ALEC_ERROR_CORRUPT_DATA = 9,
} AlecResult;

/**
 * Runtime configuration for a new ALEC encoder.
 *
 * Mirrors the Milesight-integration defaults:
 *   - history_size:       20
 *   - max_patterns:       256
 *   - max_memory_bytes:   2048
 *   - keyframe_interval:  50
 *   - smart_resync:       true
 *   - num_channels:       0       (v1.3.9 — disables pre-warm)
 *
 * Pass a NULL pointer to alec_encoder_new_with_config() to use all defaults.
 * Any numeric field set to 0 is also replaced by its default, except
 * keyframe_interval and num_channels where 0 is a valid value (disables
 * periodic keyframes / disables the v1.3.9 pre-warm respectively).
 */
typedef struct AlecEncoderConfig {
    /** Per-source history window size. Default: 20. */
    uint32_t history_size;
    /** Maximum patterns retained in the context dictionary. Default: 256. */
    uint32_t max_patterns;
    /** Maximum memory budget for the context in bytes. Default: 2048. */
    uint32_t max_memory_bytes;
    /**
     * Interval (in messages) between forced Raw32 keyframes. Default: 50.
     * Set to 0 to disable periodic keyframes.
     */
    uint32_t keyframe_interval;
    /**
     * If true, the encoder honours downlink-driven resync requests
     * (via alec_force_keyframe). Default: true.
     */
    bool smart_resync;
    /**
     * **v1.3.9 addition.** Number of fixed channels to pre-allocate at
     * encoder-creation time. When > 0, alec_encoder_new_with_config()
     * immediately allocates one SourceStats entry per channel
     * (ids 1..=num_channels, matching the fixed_channel_source_id
     * convention), so the first call to alec_encode_multi_fixed()
     * performs ZERO heap allocations.
     *
     * Set to the number of channels your firmware uses (5 for the
     * Milesight EM500-CO2). 0 disables the pre-warm and keeps the
     * pre-v1.3.9 behaviour — first encode allocates on demand.
     *
     * Upper bound: 64. Values > 64 are silently clamped to 0.
     *
     * IMPORTANT: for constrained-heap MCUs (≤ 8 KB) set this to the
     * actual channel count. Allocating per-channel state at init
     * time — when the heap is guaranteed fresh — prevents OOM panics
     * during first encode on heaps partially consumed by the
     * integrator's application.
     */
    uint32_t num_channels;
} AlecEncoderConfig;

/* ============================================================================
 * Version and Utility Functions
 * ============================================================================ */

/**
 * Get the ALEC library version string.
 *
 * @return Null-terminated version string (e.g., "1.0.0").
 *         Valid for the lifetime of the program.
 */
const char* alec_version(void);

/**
 * Convert a result code to a human-readable string.
 *
 * @param result The result code to convert.
 * @return Null-terminated description string.
 *         Valid for the lifetime of the program.
 */
const char* alec_result_to_string(AlecResult result);

/* ============================================================================
 * Encoder Functions
 * ============================================================================ */

/**
 * Create a new ALEC encoder.
 *
 * @return Pointer to new encoder, or NULL on allocation failure.
 *         Must be freed with alec_encoder_free().
 */
AlecEncoder* alec_encoder_new(void);

/**
 * Create a new encoder with checksum enabled.
 *
 * Checksums add integrity verification but increase message size.
 *
 * @return Pointer to new encoder, or NULL on allocation failure.
 */
AlecEncoder* alec_encoder_new_with_checksum(void);

/**
 * Create a new ALEC encoder with a custom configuration.
 *
 * Mirrors the Milesight integration requirements: the caller specifies
 * history_size, max_patterns, max_memory_bytes, keyframe_interval and
 * smart_resync. See AlecEncoderConfig for defaults.
 *
 * @param config Pointer to an AlecEncoderConfig. If NULL, all defaults
 *               are used. Numeric fields set to 0 are replaced by their
 *               default (except keyframe_interval, where 0 disables
 *               periodic keyframes).
 *
 * @return Pointer to a new encoder, or NULL on allocation failure.
 *         Must be freed with alec_encoder_free().
 */
AlecEncoder* alec_encoder_new_with_config(const AlecEncoderConfig* config);

/**
 * Force the next encode call to emit a keyframe (Raw32 for all channels).
 *
 * Intended to be called from a LoRaWAN downlink handler receiving the
 * 0xFF resync command from the server-side sidecar. The keyframe is
 * emitted by the next call to alec_encode_multi_fixed (marker 0xA2,
 * Raw32 for every channel).
 *
 * No-op if encoder is NULL or if the encoder was configured with
 * smart_resync = false.
 *
 * Most integrators will prefer alec_downlink_handler, which parses a
 * raw LoRaWAN downlink payload and applies the right action.
 *
 * @param encoder Encoder handle.
 */
void alec_force_keyframe(AlecEncoder* encoder);

/**
 * Parse a raw LoRaWAN downlink payload and apply the right action
 * to the encoder.
 *
 * Defined commands (first byte of @p data):
 *
 *     0xFF  Request immediate keyframe. The encoder's next
 *           alec_encode_multi_fixed call emits marker 0xA2 and Raw32
 *           for every channel.
 *
 * Any other byte is treated as an unknown command and leaves the
 * encoder state unchanged (returns ALEC_ERROR_INVALID_INPUT). Bytes
 * after byte 0 are currently reserved and ignored.
 *
 * Worst-case drift after a packet loss:
 *   - Without smart resync: keyframe_interval × uplink_period
 *     (e.g. 50 × 10 min ≈ 8h on EM500-CO2)
 *   - With smart resync + 0xFF: 1 × uplink_period (next uplink is
 *     a keyframe).
 *
 * @param encoder Encoder handle.
 * @param data    Downlink payload bytes (the raw LoRaWAN FRMPayload).
 * @param len     Length of data in bytes.
 *
 * @return ALEC_OK on a recognized command;
 *         ALEC_ERROR_NULL_POINTER if encoder or data is NULL;
 *         ALEC_ERROR_INVALID_INPUT for an empty payload or unknown
 *         command byte.
 */
AlecResult alec_downlink_handler(
    AlecEncoder* encoder,
    const uint8_t* data,
    size_t len
);

/**
 * Free an encoder.
 *
 * @param encoder Encoder to free. May be NULL (no-op).
 *
 * @warning Do not use the encoder after calling this function.
 */
void alec_encoder_free(AlecEncoder* encoder);

/**
 * Encode a single floating-point value.
 *
 * @param encoder        Encoder handle (must not be NULL).
 * @param value          The value to encode.
 * @param timestamp      Timestamp for the value (can be 0 if not used).
 * @param source_id      Source identifier string (null-terminated, can be NULL).
 * @param output         Output buffer for encoded data.
 * @param output_capacity Size of output buffer in bytes.
 * @param output_len     Pointer to store actual encoded length.
 *
 * @return ALEC_OK on success, error code otherwise.
 */
AlecResult alec_encode_value(
    AlecEncoder* encoder,
    double value,
    uint64_t timestamp,
    const char* source_id,
    uint8_t* output,
    size_t output_capacity,
    size_t* output_len
);

/**
 * Encode multiple values with adaptive per-channel compression.
 *
 * Each channel is independently classified and encoded using the optimal
 * strategy (Repeated, Delta8, Delta16, Raw32, Raw64). A single shared
 * header (13 bytes) covers all channels, amortising the header cost.
 *
 * P5 (disposable) channels are excluded from the output frame but their
 * context is still updated for future predictions.
 *
 * @param encoder         Encoder handle.
 * @param values          Array of f64 values (one per channel).
 * @param value_count     Number of channels.
 * @param timestamps      Per-channel timestamps (array of uint64_t),
 *                        or NULL to use 0 for all channels.
 * @param source_ids      Per-channel source identifier strings (array of
 *                        const char*), or NULL for automatic index-based IDs.
 * @param priorities      Per-channel priority overrides (1-5),
 *                        or NULL for classifier-assigned priorities.
 * @param output          Output buffer for encoded data.
 * @param output_capacity Size of output buffer in bytes.
 * @param output_len      Pointer to store actual encoded length.
 *
 * @return ALEC_OK on success, error code otherwise.
 */
AlecResult alec_encode_multi(
    AlecEncoder* encoder,
    const double* values,
    size_t value_count,
    const uint64_t* timestamps,
    const char* const* source_ids,
    const uint8_t* priorities,
    uint8_t* output,
    size_t output_capacity,
    size_t* output_len
);

/**
 * Save encoder context to a file.
 *
 * This allows saving a trained context for later use as a preload.
 * Both encoder and decoder must use the same context for correct
 * decompression.
 *
 * @param encoder     Encoder handle.
 * @param path        File path (null-terminated string).
 * @param sensor_type Sensor type identifier (null-terminated string).
 *
 * @return ALEC_OK on success, error code otherwise.
 */
AlecResult alec_encoder_save_context(
    AlecEncoder* encoder,
    const char* path,
    const char* sensor_type
);

/**
 * Load encoder context from a preload file.
 *
 * This enables instant optimal compression by loading a pre-trained
 * context.
 *
 * @param encoder Encoder handle.
 * @param path    File path to preload (null-terminated string).
 *
 * @return ALEC_OK on success, error code otherwise.
 */
AlecResult alec_encoder_load_context(
    AlecEncoder* encoder,
    const char* path
);

/**
 * Get the current context version.
 *
 * The context version changes when the encoder learns new patterns.
 * Encoder and decoder must have matching versions for correct
 * decompression.
 *
 * @param encoder Encoder handle.
 *
 * @return Context version number, or 0 if encoder is NULL.
 */
uint32_t alec_encoder_context_version(const AlecEncoder* encoder);

/* ============================================================================
 * Encoder context save / load with RAM buffers (v1.3.7)
 *
 * Buffer-based mirror of alec_decoder_context_save / _context_load, callable
 * from MCUs that have no filesystem. The saved blob captures:
 *   - The full Context (per-channel prediction, patterns, version).
 *   - The encoder sequence counter.
 *   - The pending force-keyframe flag and messages-since-keyframe counter.
 *
 * Intended use (firmware fix for "oversize frame discard"):
 *
 *     snapshot = alec_encoder_context_save(enc, buf, cap, &len)
 *     wire     = alec_encode_multi_fixed(enc, …)
 *     if (wire_len > LORAWAN_CEILING) {
 *         alec_encoder_context_load(enc, buf, len);   // roll back
 *         alec_force_keyframe(enc);                   // next frame = keyframe
 *     }
 *
 * Wire layout:
 *
 *     byte 0..=3    : magic "ALEE"
 *     byte 4        : format version (= 1)
 *     byte 5        : flags (bit 0: force_keyframe_pending)
 *     byte 6..=7    : encoder sequence (u16 big-endian)
 *     byte 8..=11   : messages_since_keyframe (u32 big-endian)
 *     byte 12..=15  : ALCS payload length  (u32 big-endian)
 *     byte 16..=23  : header xxh64         (u64 big-endian)
 *     byte 24..     : ALCS-wrapped Context (own internal CRC32)
 *
 * Typical size: 24 bytes of header + 1–2 KB of Context payload.
 * ============================================================================ */

/**
 * Save the encoder's runtime state to a caller-provided RAM buffer.
 *
 * On success `*written` reports the number of bytes written. On
 * ALEC_ERROR_BUFFER_TOO_SMALL `*written` reports the required size
 * and `buf` is NOT modified (no partial write).
 *
 * Static configuration (keyframe_interval, smart_resync, checksum flag)
 * is NOT saved — those are properties of the encoder instance.
 *
 * @param enc     Encoder handle.
 * @param buf     Destination buffer.
 * @param buf_cap Size of `buf` in bytes.
 * @param written Out: bytes written (on success) or required size
 *                (on ALEC_ERROR_BUFFER_TOO_SMALL).
 *
 * @return ALEC_OK on success;
 *         ALEC_ERROR_BUFFER_TOO_SMALL if `buf_cap` is too small
 *         (`*written` reports the required size, `buf` is unchanged);
 *         ALEC_ERROR_NULL_POINTER for a NULL required pointer;
 *         ALEC_ERROR_ENCODING_FAILED for an internal serialization error.
 */
AlecResult alec_encoder_context_save(
    const AlecEncoder* enc,
    uint8_t* buf,
    size_t buf_cap,
    size_t* written
);

/**
 * Restore encoder state from bytes produced by alec_encoder_context_save.
 *
 * On success the encoder's context, sequence counter, force-keyframe
 * flag and messages-since-keyframe counter are overwritten atomically.
 * Static configuration is preserved.
 *
 * On ALEC_ERROR_CORRUPT_DATA the encoder is NOT modified — callers can
 * safely retry.
 *
 * @param enc     Encoder handle.
 * @param buf     Input bytes.
 * @param buf_len Length of `buf` in bytes.
 *
 * @return ALEC_OK on success;
 *         ALEC_ERROR_NULL_POINTER for a NULL required pointer;
 *         ALEC_ERROR_CORRUPT_DATA if `buf` cannot be parsed (bad magic,
 *         unknown format version, header xxh64 mismatch, length mismatch,
 *         or malformed ALCS payload).
 */
AlecResult alec_encoder_context_load(
    AlecEncoder* enc,
    const uint8_t* buf,
    size_t buf_len
);

/* ============================================================================
 * Decoder Functions
 * ============================================================================ */

/**
 * Create a new ALEC decoder.
 *
 * @return Pointer to new decoder, or NULL on allocation failure.
 *         Must be freed with alec_decoder_free().
 */
AlecDecoder* alec_decoder_new(void);

/**
 * Create a new decoder with checksum verification enabled.
 *
 * @return Pointer to new decoder, or NULL on allocation failure.
 */
AlecDecoder* alec_decoder_new_with_checksum(void);

/**
 * Create a new ALEC decoder with a custom configuration.
 *
 * Mirrors alec_encoder_new_with_config for the decoder side. The
 * AlecEncoderConfig struct is reused because the decoder must run with
 * the same history_size, max_patterns and max_memory_bytes as the
 * matching encoder for the prediction model to stay in sync.
 *
 * keyframe_interval and smart_resync are encoder-only knobs and are
 * accepted but ignored on the decoder side.
 *
 * @param config Pointer to an AlecEncoderConfig. If NULL, all defaults
 *               are used. Numeric fields set to 0 are replaced by their
 *               default.
 *
 * @return Pointer to a new decoder, or NULL on allocation failure.
 *         Must be freed with alec_decoder_free().
 */
AlecDecoder* alec_decoder_new_with_config(const AlecEncoderConfig* config);

/**
 * Reset a decoder to its initial state.
 *
 * Wipes all per-channel prediction state (per-source EMA, last values,
 * history) and clears the session-tracking counters
 * (last_header_sequence, last_gap_size). The next frame the decoder
 * sees should be a keyframe (marker 0xA2) which will re-seed the
 * prediction state from Raw32 values.
 *
 * Use after detecting an unrecoverable desync that the in-band gap
 * recovery can't handle (e.g. the server-side sidecar restarting
 * without a saved context).
 *
 * @param dec Decoder to reset. May be NULL (no-op).
 */
void alec_decoder_reset(AlecDecoder* dec);

/**
 * Free a decoder.
 *
 * @param decoder Decoder to free. May be NULL (no-op).
 */
void alec_decoder_free(AlecDecoder* decoder);

/**
 * Decode compressed data to a single value.
 *
 * @param decoder   Decoder handle.
 * @param input     Compressed input data.
 * @param input_len Length of input data.
 * @param value     Pointer to store decoded value.
 * @param timestamp Pointer to store decoded timestamp (can be NULL).
 *
 * @return ALEC_OK on success, error code otherwise.
 */
AlecResult alec_decode_value(
    AlecDecoder* decoder,
    const uint8_t* input,
    size_t input_len,
    double* value,
    uint64_t* timestamp
);

/**
 * Decode compressed data to multiple values.
 *
 * @param decoder         Decoder handle.
 * @param input           Compressed input data.
 * @param input_len       Length of input data.
 * @param values          Output buffer for decoded values.
 * @param values_capacity Maximum number of values that can be stored.
 * @param values_count    Pointer to store actual number of decoded values.
 *
 * @return ALEC_OK on success, error code otherwise.
 */
AlecResult alec_decode_multi(
    AlecDecoder* decoder,
    const uint8_t* input,
    size_t input_len,
    double* values,
    size_t values_capacity,
    size_t* values_count
);

/**
 * Load decoder context from a preload file.
 *
 * @param decoder Decoder handle.
 * @param path    File path to preload (null-terminated string).
 *
 * @return ALEC_OK on success, error code otherwise.
 */
AlecResult alec_decoder_load_context(
    AlecDecoder* decoder,
    const char* path
);

/**
 * Get the current decoder context version.
 *
 * @param decoder Decoder handle.
 *
 * @return Context version number, or 0 if decoder is NULL.
 */
uint32_t alec_decoder_context_version(const AlecDecoder* decoder);

/* ============================================================================
 * Fixed-channel encode / decode (Milesight EM500-CO2 compact wire format)
 *
 * Wire layout for an ALEC fixed-channel frame:
 *
 *     byte 0         : marker        (0xA1 data, 0xA2 keyframe)
 *     byte 1..=2     : sequence      (u16 big-endian)
 *     byte 3..=4     : ctx_version   (u16 big-endian, low 16 bits of u32)
 *     byte 5..=5+B-1 : encoding bitmap (2 bits per channel,
 *                      B = ceil(channel_count / 4) bytes)
 *     byte 5+B..     : per-channel encoded data
 *
 * Encoding bitmap codes:
 *
 *     00 Repeated    (0 bytes of data)
 *     01 Delta8      (1 byte)
 *     10 Delta16     (2 bytes)
 *     11 Raw32       (4 bytes)
 *
 * The channel count is NOT encoded on the wire — encoder and decoder
 * must agree on it out-of-band (e.g. via the LoRaWAN device model).
 * ============================================================================ */

/**
 * Encode a fixed-channel frame using the compact 4-byte header.
 *
 * If the encoder's `keyframe_interval` has been reached OR
 * `alec_force_keyframe` was called since the last encode (and
 * `smart_resync` is enabled), this frame is emitted as a keyframe
 * (marker 0xA2, Raw32 for every channel). Otherwise a regular data
 * frame (marker 0xA1) is emitted.
 *
 * The caller must compare *output_len against the 11-byte LoRaWAN
 * ceiling. If the encoded frame exceeds 11 bytes (notably cold-start
 * frames and keyframes) the integrator should fall back to a legacy
 * TLV frame for that message.
 *
 * @param encoder         Encoder handle.
 * @param values          Per-channel f64 values (positional).
 * @param channel_count   Number of channels in `values`.
 * @param output          Destination buffer for the wire bytes.
 * @param output_capacity Size of `output` in bytes.
 * @param output_len      Pointer receiving the number of bytes written.
 *
 * @return ALEC_OK on success; ALEC_ERROR_BUFFER_TOO_SMALL if the
 *         encoded frame does not fit; ALEC_ERROR_INVALID_INPUT for
 *         zero channels; ALEC_ERROR_NULL_POINTER for a NULL pointer.
 */
AlecResult alec_encode_multi_fixed(
    AlecEncoder* encoder,
    const double* values,
    size_t channel_count,
    uint8_t* output,
    size_t output_capacity,
    size_t* output_len
);

/**
 * Decode a fixed-channel frame produced by alec_encode_multi_fixed.
 *
 * The wire format does NOT carry the channel count — encoder and decoder
 * must agree on it out-of-band. The decoder uses `max_channels` as both
 * the channel count AND the capacity of `values_out`; on success
 * `*num_channels_out` is set to that same value.
 *
 * On success:
 *   - `values_out[0..*num_channels_out]` receives the decoded values
 *     in channel order.
 *   - `*sequence_out` receives the wire sequence number (compact header).
 *   - `*is_keyframe_out` is `true` iff the frame's marker was `0xA2`.
 *   - The decoder's gap and sequence tracking are updated (see
 *     alec_decoder_gap_detected).
 *
 * `num_channels_out`, `sequence_out` and `is_keyframe_out` are all
 * optional — pass NULL if the caller does not need them.
 *
 * @param dec              Decoder handle.
 * @param frame_data       Raw ALEC wire bytes (as received from
 *                         LoRaWAN/NB-IoT uplink), starting at the marker byte.
 * @param frame_len        Length of the frame in bytes.
 * @param values_out       Destination buffer for decoded channel values
 *                         (one f64 per channel).
 * @param max_channels     Capacity of `values_out` AND number of channels
 *                         in the frame. Must match the encoder's count.
 * @param num_channels_out Out: actual number of channels decoded
 *                         (always equals `max_channels` on success).
 *                         May be NULL.
 * @param sequence_out     Out: sequence number from the compact header.
 *                         May be NULL.
 * @param is_keyframe_out  Out: `true` if the frame was a keyframe (`0xA2`).
 *                         May be NULL.
 *
 * @return ALEC_OK on success; ALEC_ERROR_INVALID_INPUT for a non-ALEC
 *         marker byte or `max_channels == 0`;
 *         ALEC_ERROR_BUFFER_TOO_SMALL if the input is shorter than the
 *         header + bitmap + data bytes;
 *         ALEC_ERROR_DECODING_FAILED for any other decode failure;
 *         ALEC_ERROR_NULL_POINTER for a NULL required pointer
 *         (`dec`, `frame_data`, `values_out`).
 */
AlecResult alec_decode_multi_fixed(
    AlecDecoder* dec,
    const uint8_t* frame_data,
    size_t frame_len,
    double* values_out,
    size_t max_channels,
    size_t* num_channels_out,
    uint16_t* sequence_out,
    bool* is_keyframe_out
);

/**
 * Check whether the most recent decode detected a sequence gap.
 *
 * The server-side sidecar uses this to decide whether to issue a resync
 * downlink (0xFF) to the device. The gap size is the number of missing
 * frames between the previous and current sequence, clipped to 255.
 *
 * @param decoder      Decoder handle.
 * @param out_gap_size Out parameter receiving the gap size (may be NULL).
 *
 * @return true if the most recent multi-frame decode observed missing
 *         frames (gap > 0); false if no gap, if no decode has been
 *         performed yet, or if decoder is NULL.
 */
bool alec_decoder_gap_detected(const AlecDecoder* decoder, uint8_t* out_gap_size);

/* ============================================================================
 * Context persistence (Bloc D)
 *
 * Serialize / restore a decoder's Context to a self-contained byte
 * buffer. Intended use: the ChirpStack sidecar periodically exports
 * each DevEUI's decoder context (Redis persistence) and restores it
 * on startup. Typical serialized size is 1-2 KB per DevEUI for a
 * 5-channel EM500-CO2 decoder with history_size = 20.
 *
 * Session state (last_header_sequence, last_gap_size) is NOT
 * serialized — those are transient frame-level trackers that reset
 * naturally on sidecar restart.
 * ============================================================================ */

/**
 * Compute the exact number of bytes alec_decoder_export_state would
 * write. Lets the caller allocate the right-sized buffer up front.
 *
 * @param decoder     Decoder handle.
 * @param sensor_type Null-terminated sensor-type identifier
 *                    (≤ 255 bytes, e.g. "em500-co2").
 * @param out_size    Pointer receiving the required size in bytes.
 *
 * @return ALEC_OK on success; ALEC_ERROR_NULL_POINTER for a NULL
 *         pointer; ALEC_ERROR_INVALID_UTF8 if sensor_type is not
 *         valid UTF-8; ALEC_ERROR_INVALID_INPUT if sensor_type
 *         exceeds 255 bytes.
 */
AlecResult alec_decoder_export_state_size(
    const AlecDecoder* decoder,
    const char* sensor_type,
    size_t* out_size
);

/**
 * Serialize the decoder's context to a caller-provided buffer.
 *
 * @param decoder      Decoder handle.
 * @param sensor_type  Null-terminated sensor-type identifier (≤ 255 bytes).
 * @param out_buf      Destination buffer.
 * @param out_capacity Size of out_buf in bytes.
 * @param out_len      Pointer receiving bytes written (on success)
 *                     or the required size (on ALEC_ERROR_BUFFER_TOO_SMALL).
 *
 * @return ALEC_OK on success; ALEC_ERROR_BUFFER_TOO_SMALL if
 *         out_capacity is too small — *out_len reports the required
 *         size and out_buf is NOT written (no partial write);
 *         ALEC_ERROR_NULL_POINTER / ALEC_ERROR_INVALID_UTF8 /
 *         ALEC_ERROR_INVALID_INPUT otherwise.
 */
AlecResult alec_decoder_export_state(
    const AlecDecoder* decoder,
    const char* sensor_type,
    uint8_t* out_buf,
    size_t out_capacity,
    size_t* out_len
);

/**
 * Restore a decoder's context from bytes produced by
 * alec_decoder_export_state.
 *
 * On success, decoder.context is replaced by the deserialized
 * context. The decoder's session state (last_header_sequence,
 * last_gap_size) is PRESERVED.
 *
 * If the input buffer is corrupted, the decoder is NOT modified in
 * any way — neither the context nor the session state.
 *
 * @param decoder  Decoder handle.
 * @param data     Input bytes produced by alec_decoder_export_state.
 * @param data_len Length of data in bytes.
 *
 * @return ALEC_OK on success; ALEC_ERROR_NULL_POINTER for a NULL
 *         pointer; ALEC_ERROR_CORRUPT_DATA if data cannot be parsed
 *         (bad magic, CRC mismatch, truncation, unknown format version).
 */
AlecResult alec_decoder_import_state(
    AlecDecoder* decoder,
    const uint8_t* data,
    size_t data_len
);

/**
 * Save the decoder's context to a caller-provided buffer.
 *
 * Sensor-type-agnostic wrapper around alec_decoder_export_state.
 * On success `*written` reports the number of bytes written; on
 * ALEC_ERROR_BUFFER_TOO_SMALL `*written` reports the required size
 * and `buf` is NOT modified (no partial write).
 *
 * The output is a self-contained byte buffer (magic `ALCS`, CRC32
 * protected) suitable for Redis/disk persistence. Typical size is
 * 1–2 KB for a 5-channel decoder with history_size = 20.
 *
 * Session state (last_header_sequence, last_gap_size) is NOT
 * serialized — those are transient counters that reset naturally
 * when the sidecar restarts.
 *
 * @param dec     Decoder handle.
 * @param buf     Destination buffer.
 * @param buf_cap Size of `buf` in bytes.
 * @param written Out: bytes written (on success) or required size
 *                (on ALEC_ERROR_BUFFER_TOO_SMALL).
 *
 * @return ALEC_OK on success;
 *         ALEC_ERROR_BUFFER_TOO_SMALL if `buf_cap` is too small
 *         (`*written` reports the required size, `buf` is unchanged);
 *         ALEC_ERROR_NULL_POINTER for a NULL required pointer.
 */
AlecResult alec_decoder_context_save(
    const AlecDecoder* dec,
    uint8_t* buf,
    size_t buf_cap,
    size_t* written
);

/**
 * Restore the decoder's context from bytes produced by
 * alec_decoder_context_save (or alec_decoder_export_state).
 *
 * On success the decoder's context is replaced; session state
 * (last_header_sequence, last_gap_size) is preserved. On
 * ALEC_ERROR_CORRUPT_DATA the decoder is NOT modified.
 *
 * @param dec     Decoder handle.
 * @param buf     Input bytes.
 * @param buf_len Length of `buf` in bytes.
 *
 * @return ALEC_OK on success;
 *         ALEC_ERROR_NULL_POINTER for a NULL required pointer;
 *         ALEC_ERROR_CORRUPT_DATA if `buf` cannot be parsed (bad magic,
 *         CRC mismatch, truncation, unknown format version).
 */
AlecResult alec_decoder_context_load(
    AlecDecoder* dec,
    const uint8_t* buf,
    size_t buf_len
);

/* ============================================================================
 * Bare-metal / RTOS heap initialization
 *
 * Only available when the library is built with the `bare-metal` Cargo
 * feature (e.g. for Cortex-M/FreeRTOS targets). On hosted builds the heap
 * is managed by the system allocator and these entry points are absent.
 * ============================================================================ */

/**
 * Initialize the ALEC heap with the built-in 8 KB static buffer.
 *
 * Call exactly once, before any other ALEC function.
 */
void alec_heap_init(void);

/**
 * Initialize the ALEC heap with a caller-provided buffer.
 *
 * Required on RTOSes (FreeRTOS, Milesight firmware) where the heap region
 * is managed by the integrator and must not be statically embedded in the
 * library itself.
 *
 * @param buf Pointer to the start of the heap region. Must remain valid
 *            for the lifetime of the program. Must be non-NULL.
 * @param len Size of the heap region in bytes. Must be > 0.
 *
 * @note Call exactly once, before any other ALEC function. Do not combine
 *       with alec_heap_init(). No-op if buf is NULL or len == 0.
 */
void alec_heap_init_with_buffer(uint8_t* buf, size_t len);

#ifdef __cplusplus
}
#endif

#endif /* ALEC_H */
