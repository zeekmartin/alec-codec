#ifndef ALEC_GENERATED_H
#define ALEC_GENERATED_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Default history size per source (validated on 99-message EM500-CO2 dataset).
 */
#define ALEC_DEFAULT_HISTORY_SIZE 20

/**
 * Default maximum number of patterns retained in the dictionary.
 */
#define ALEC_DEFAULT_MAX_PATTERNS 256

/**
 * Default maximum memory budget for the context (bytes).
 */
#define ALEC_DEFAULT_MAX_MEMORY_BYTES 2048

/**
 * Default keyframe interval (messages between forced Raw32 keyframes).
 */
#define ALEC_DEFAULT_KEYFRAME_INTERVAL 50

/**
 * Default for smart-resync via LoRaWAN downlink.
 */
#define ALEC_DEFAULT_SMART_RESYNC true

/**
 * Result codes for ALEC FFI functions
 */
typedef enum AlecResult {
  /**
   * Operation completed successfully
   */
  Ok = 0,
  /**
   * Invalid input data provided
   */
  ErrorInvalidInput = 1,
  /**
   * Output buffer is too small
   */
  ErrorBufferTooSmall = 2,
  /**
   * Encoding operation failed
   */
  ErrorEncodingFailed = 3,
  /**
   * Decoding operation failed
   */
  ErrorDecodingFailed = 4,
  /**
   * Null pointer was provided
   */
  ErrorNullPointer = 5,
  /**
   * Invalid UTF-8 string
   */
  ErrorInvalidUtf8 = 6,
  /**
   * File I/O error
   */
  ErrorFileIo = 7,
  /**
   * Context version mismatch
   */
  ErrorVersionMismatch = 8,
} AlecResult;

/**
 * Opaque decoder handle
 *
 * Created with `alec_decoder_new()`, freed with `alec_decoder_free()`.
 * Do not access internal fields directly.
 */
typedef struct AlecDecoder AlecDecoder;

/**
 * Opaque encoder handle
 *
 * Created with `alec_encoder_new()`, freed with `alec_encoder_free()`.
 * Do not access internal fields directly.
 */
typedef struct AlecEncoder AlecEncoder;

/**
 * Runtime configuration for a new ALEC encoder.
 *
 * Mirrors the Milesight-integration defaults (history=20,
 * patterns=256, memory=2048B, keyframe=50, smart_resync=true).
 *
 * Pass a NULL pointer to `alec_encoder_new_with_config` to use all
 * defaults. Any field set to 0 is also replaced by its default, so
 * callers can opt in to a single override while keeping the rest.
 */
typedef struct AlecEncoderConfig {
  /**
   * Per-source history window size. Default: 20.
   */
  uint32_t history_size;
  /**
   * Maximum patterns retained in the context dictionary. Default: 256.
   */
  uint32_t max_patterns;
  /**
   * Maximum memory budget for the context in bytes. Default: 2048.
   */
  uint32_t max_memory_bytes;
  /**
   * Interval (in messages) between forced Raw32 keyframes. Default: 50.
   * Set to 0 to disable periodic keyframes.
   */
  uint32_t keyframe_interval;
  /**
   * If true, the encoder honours downlink-driven resync requests
   * (via `alec_force_keyframe`). Default: true.
   */
  bool smart_resync;
} AlecEncoderConfig;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Get the ALEC library version string
 *
 * # Returns
 *
 * A null-terminated string containing the version (e.g., "1.0.0").
 * The returned pointer is valid for the lifetime of the program.
 *
 * # Example (C)
 *
 * ```c
 * printf("ALEC version: %s\n", alec_version());
 * ```
 */
const char *alec_version(void);

/**
 * Convert a result code to a human-readable string
 *
 * # Arguments
 *
 * * `result` - The result code to convert
 *
 * # Returns
 *
 * A null-terminated string describing the result.
 * The returned pointer is valid for the lifetime of the program.
 */
const char *alec_result_to_string(enum AlecResult result);

/**
 * Create a new ALEC encoder
 *
 * # Returns
 *
 * A pointer to a new encoder, or NULL on allocation failure.
 * The encoder must be freed with `alec_encoder_free()` when no longer needed.
 *
 * # Example (C)
 *
 * ```c
 * AlecEncoder* enc = alec_encoder_new();
 * if (enc == NULL) {
 *     // Handle allocation failure
 * }
 * // ... use encoder ...
 * alec_encoder_free(enc);
 * ```
 */
struct AlecEncoder *alec_encoder_new(void);

/**
 * Create a new encoder with checksum enabled
 *
 * # Returns
 *
 * A pointer to a new encoder with checksum enabled, or NULL on failure.
 */
struct AlecEncoder *alec_encoder_new_with_checksum(void);

/**
 * Create a new ALEC encoder with a custom configuration.
 *
 * Mirrors the Milesight integration requirements: the caller specifies
 * `history_size`, `max_patterns`, `max_memory_bytes`, `keyframe_interval`
 * and `smart_resync`. See `AlecEncoderConfig` for defaults.
 *
 * # Arguments
 *
 * * `config` - Pointer to an `AlecEncoderConfig`. If NULL, all defaults
 *              are used. Numeric fields set to 0 are replaced by their
 *              default (except `keyframe_interval`, where 0 disables
 *              periodic keyframes).
 *
 * # Returns
 *
 * A pointer to a new encoder, or NULL on allocation failure.
 * Must be freed with `alec_encoder_free()`.
 */
struct AlecEncoder *alec_encoder_new_with_config(const struct AlecEncoderConfig *config);

/**
 * Force the next encode call to emit a keyframe (Raw32 for all channels).
 *
 * Intended to be called from a LoRaWAN downlink handler receiving the
 * 0xFF resync command from the server-side sidecar. The flag is
 * consumed by the fixed-channel encode path (Bloc B/C); until that
 * path lands, calling this only sets the internal flag.
 *
 * No-op if `encoder` is NULL or if the encoder was configured with
 * `smart_resync = false`.
 *
 * # Arguments
 *
 * * `encoder` - Encoder handle.
 */
void alec_force_keyframe(struct AlecEncoder *encoder);

/**
 * Free an encoder
 *
 * # Arguments
 *
 * * `encoder` - Encoder to free. May be NULL (no-op).
 *
 * # Safety
 *
 * The encoder must not be used after calling this function.
 */
void alec_encoder_free(struct AlecEncoder *encoder);

/**
 * Encode a single floating-point value
 *
 * # Arguments
 *
 * * `encoder` - Encoder handle (must not be NULL)
 * * `value` - The value to encode
 * * `timestamp` - Timestamp for the value (can be 0 if not used)
 * * `source_id` - Source identifier string (null-terminated, can be NULL)
 * * `output` - Output buffer for encoded data
 * * `output_capacity` - Size of output buffer in bytes
 * * `output_len` - Pointer to store actual encoded length
 *
 * # Returns
 *
 * `ALEC_OK` on success, error code otherwise.
 */
enum AlecResult alec_encode_value(struct AlecEncoder *encoder,
                                  double value,
                                  uint64_t timestamp,
                                  const char *source_id,
                                  uint8_t *output,
                                  uintptr_t output_capacity,
                                  uintptr_t *output_len);

/**
 * Encode multiple values with adaptive per-channel compression.
 *
 * Each channel is independently classified (P1–P5) and encoded using the
 * optimal strategy (Repeated, Delta8, Delta16, etc.). P5 channels are
 * excluded from the output frame but their context is still updated.
 *
 * # Arguments
 *
 * * `encoder` - Encoder handle
 * * `values` - Array of f64 values to encode (one per channel)
 * * `value_count` - Number of channels
 * * `timestamps` - Per-channel timestamps (array of uint64_t), or NULL to
 *   use 0 for all channels
 * * `source_ids` - Per-channel source identifier strings (array of
 *   `const char*`), or NULL for automatic index-based IDs
 * * `priorities` - Per-channel priority overrides (1–5), or NULL for
 *   classifier-assigned priorities
 * * `output` - Output buffer for encoded data
 * * `output_capacity` - Size of output buffer in bytes
 * * `output_len` - Pointer to store actual encoded length
 *
 * # Returns
 *
 * `ALEC_OK` on success, error code otherwise.
 */
enum AlecResult alec_encode_multi(struct AlecEncoder *encoder,
                                  const double *values,
                                  uintptr_t value_count,
                                  const uint64_t *timestamps,
                                  const char *const *source_ids,
                                  const uint8_t *priorities,
                                  uint8_t *output,
                                  uintptr_t output_capacity,
                                  uintptr_t *output_len);

/**
 * Save encoder context to a file (for preload generation)
 *
 * # Arguments
 *
 * * `encoder` - Encoder handle
 * * `path` - File path (null-terminated string)
 * * `sensor_type` - Sensor type identifier (null-terminated string)
 *
 * # Returns
 *
 * `ALEC_OK` on success, error code otherwise.
 */
enum AlecResult alec_encoder_save_context(struct AlecEncoder *encoder,
                                          const char *path,
                                          const char *sensor_type);

/**
 * Load encoder context from a preload file
 *
 * # Arguments
 *
 * * `encoder` - Encoder handle
 * * `path` - File path to preload (null-terminated string)
 *
 * # Returns
 *
 * `ALEC_OK` on success, error code otherwise.
 */
enum AlecResult alec_encoder_load_context(struct AlecEncoder *encoder, const char *path);

/**
 * Get the current context version
 *
 * # Arguments
 *
 * * `encoder` - Encoder handle
 *
 * # Returns
 *
 * The context version number, or 0 if encoder is NULL.
 */
uint32_t alec_encoder_context_version(const struct AlecEncoder *encoder);

/**
 * Create a new ALEC decoder
 *
 * # Returns
 *
 * A pointer to a new decoder, or NULL on allocation failure.
 * The decoder must be freed with `alec_decoder_free()` when no longer needed.
 */
struct AlecDecoder *alec_decoder_new(void);

/**
 * Create a new decoder with checksum verification enabled
 *
 * # Returns
 *
 * A pointer to a new decoder with checksum enabled, or NULL on failure.
 */
struct AlecDecoder *alec_decoder_new_with_checksum(void);

/**
 * Check whether the most recent decode detected a sequence gap.
 *
 * The server-side sidecar uses this to decide whether to issue a
 * resync downlink (0xFF) to the device. The gap size is the number
 * of missing frames between the previous `last_sequence` and the
 * current one, clipped to 255.
 *
 * # Arguments
 *
 * * `decoder`      - Decoder handle.
 * * `out_gap_size` - Out parameter receiving the gap size (may be NULL).
 *
 * # Returns
 *
 * `true` if the most recent multi-frame decode observed missing
 * frames (gap > 0). `false` if no gap, if no decode has been
 * performed yet, or if `decoder` is NULL.
 */
bool alec_decoder_gap_detected(const struct AlecDecoder *decoder, uint8_t *out_gap_size);

/**
 * Free a decoder
 *
 * # Arguments
 *
 * * `decoder` - Decoder to free. May be NULL (no-op).
 */
void alec_decoder_free(struct AlecDecoder *decoder);

/**
 * Decode compressed data to a single value
 *
 * # Arguments
 *
 * * `decoder` - Decoder handle
 * * `input` - Compressed input data
 * * `input_len` - Length of input data
 * * `value` - Pointer to store decoded value
 * * `timestamp` - Pointer to store decoded timestamp (can be NULL)
 *
 * # Returns
 *
 * `ALEC_OK` on success, error code otherwise.
 */
enum AlecResult alec_decode_value(struct AlecDecoder *decoder,
                                  const uint8_t *input,
                                  uintptr_t input_len,
                                  double *value,
                                  uint64_t *timestamp);

/**
 * Decode compressed data to multiple values
 *
 * # Arguments
 *
 * * `decoder` - Decoder handle
 * * `input` - Compressed input data
 * * `input_len` - Length of input data
 * * `values` - Output buffer for decoded values
 * * `values_capacity` - Maximum number of values that can be stored
 * * `values_count` - Pointer to store actual number of decoded values
 *
 * # Returns
 *
 * `ALEC_OK` on success, error code otherwise.
 */
enum AlecResult alec_decode_multi(struct AlecDecoder *decoder,
                                  const uint8_t *input,
                                  uintptr_t input_len,
                                  double *values,
                                  uintptr_t values_capacity,
                                  uintptr_t *values_count);

/**
 * Load decoder context from a preload file
 *
 * # Arguments
 *
 * * `decoder` - Decoder handle
 * * `path` - File path to preload (null-terminated string)
 *
 * # Returns
 *
 * `ALEC_OK` on success, error code otherwise.
 */
enum AlecResult alec_decoder_load_context(struct AlecDecoder *decoder, const char *path);

/**
 * Get the current decoder context version
 *
 * # Arguments
 *
 * * `decoder` - Decoder handle
 *
 * # Returns
 *
 * The context version number, or 0 if decoder is NULL.
 */
uint32_t alec_decoder_context_version(const struct AlecDecoder *decoder);

/**
 * Encode a fixed-channel frame using the compact 4-byte header
 * (Milesight EM500-CO2 wire format).
 *
 * The number of channels is passed explicitly and must match the
 * value used by the peer decoder — the wire format does not carry
 * it. The encoder keeps a positional view of the channels, so
 * `values[i]` is the value for channel index `i`.
 *
 * If `keyframe_interval > 0` and `messages_since_keyframe`
 * has reached that interval, OR if `alec_force_keyframe` was called
 * since the last encode AND `smart_resync` is enabled, this frame
 * is emitted as a **keyframe** (marker 0xA2, Raw32 for every
 * channel). Otherwise a regular data frame (marker 0xA1) is emitted.
 *
 * # Arguments
 *
 * * `encoder`         - Encoder handle (must not be NULL).
 * * `values`          - Per-channel f64 values, positional.
 * * `channel_count`   - Number of channels in `values`.
 * * `output`          - Destination buffer for the wire bytes.
 * * `output_capacity` - Size of `output` in bytes.
 * * `out_len`         - Pointer receiving the number of bytes
 *                       written to `output`.
 *
 * # Returns
 *
 * `ALEC_OK` on success. `ALEC_ERROR_BUFFER_TOO_SMALL` if the
 * encoded frame does not fit in `output`. `ALEC_ERROR_INVALID_INPUT`
 * for zero channels. `ALEC_ERROR_NULL_POINTER` for any required
 * NULL pointer.
 *
 * The caller can detect that the frame must be replaced by the
 * legacy TLV fallback by comparing `*out_len` against the 11-byte
 * LoRaWAN ceiling: if `*out_len > 11`, emit the TLV frame instead.
 */
enum AlecResult alec_encode_multi_fixed(struct AlecEncoder *encoder,
                                        const double *values,
                                        uintptr_t channel_count,
                                        uint8_t *output,
                                        uintptr_t output_capacity,
                                        uintptr_t *out_len);

/**
 * Decode a fixed-channel frame produced by `alec_encode_multi_fixed`.
 *
 * The number of channels is passed explicitly — the wire format does
 * not carry it. Must match the value used by the encoder.
 *
 * On a successful decode:
 *   - `output[..channel_count]` receives the decoded values in channel order.
 *   - The decoder's last-sequence and last-ctx-version are updated.
 *   - The gap size (if any) is available via `alec_decoder_gap_detected`.
 *
 * # Returns
 *
 * * `ALEC_OK`                         on success.
 * * `ALEC_ERROR_INVALID_INPUT`        for zero channels or a non-ALEC marker byte.
 * * `ALEC_ERROR_BUFFER_TOO_SMALL`     if `output_capacity < channel_count`
 *                                     or the input is shorter than the
 *                                     header + bitmap + data bytes.
 * * `ALEC_ERROR_DECODING_FAILED`      for any other decode error.
 * * `ALEC_ERROR_NULL_POINTER`         for a NULL required pointer.
 */
enum AlecResult alec_decode_multi_fixed(struct AlecDecoder *decoder,
                                        const uint8_t *input,
                                        uintptr_t input_len,
                                        uintptr_t channel_count,
                                        double *output,
                                        uintptr_t output_capacity);

extern uint8_t *k_aligned_alloc(uintptr_t align, uintptr_t size);

extern void k_free(uint8_t *ptr);

/**
 * No-op on Zephyr — heap is managed by the RTOS.
 *
 * Provided for API compatibility with bare-metal builds.
 */
void alec_heap_init(void);

/**
 * Initialize the heap allocator. Must be called before any alloc usage.
 *
 * # Safety
 *
 * Must be called exactly once, before any heap allocation.
 */
void alec_heap_init(void);

/**
 * Initialize the heap allocator with a caller-provided buffer.
 *
 * Required on RTOSes (FreeRTOS, Milesight firmware) where the heap
 * region is managed by the integrator and must not be statically
 * embedded in the ALEC library itself.
 *
 * # Arguments
 *
 * * `buf` - Pointer to the start of the heap region. Must remain
 *           valid for the lifetime of the program. Must be non-NULL.
 * * `len` - Size of the heap region in bytes. Must be > 0.
 *
 * # Safety
 *
 * * Must be called exactly once, before any ALEC allocation.
 * * `buf` must point to `len` bytes of writable memory that stays
 *   valid for the lifetime of the process.
 * * This function must not be combined with `alec_heap_init()`.
 * * No-op if `buf` is NULL or `len == 0`.
 */
void alec_heap_init_with_buffer(uint8_t *buf, uintptr_t len);

#ifdef __cplusplus
} // extern "C"
#endif // __cplusplus

#endif /* ALEC_GENERATED_H */
