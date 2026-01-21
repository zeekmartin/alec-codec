#ifndef ALEC_GENERATED_H
#define ALEC_GENERATED_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

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
                                  const char *_source_id,
                                  uint8_t *output,
                                  uintptr_t output_capacity,
                                  uintptr_t *output_len);

/**
 * Encode multiple values at once
 *
 * # Arguments
 *
 * * `encoder` - Encoder handle
 * * `values` - Array of values to encode
 * * `value_count` - Number of values in the array
 * * `timestamp` - Timestamp for the values
 * * `source_id` - Source identifier string (null-terminated, can be NULL)
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
                                  uint64_t timestamp,
                                  const char *_source_id,
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

#ifdef __cplusplus
} // extern "C"
#endif // __cplusplus

#endif /* ALEC_GENERATED_H */
