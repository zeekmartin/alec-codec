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
} AlecResult;

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
 * Encode multiple values at once.
 *
 * More efficient than encoding values individually when you have
 * multiple readings from the same timestamp.
 *
 * @param encoder         Encoder handle.
 * @param values          Array of values to encode.
 * @param value_count     Number of values in the array.
 * @param timestamp       Timestamp for the values.
 * @param source_id       Source identifier string (can be NULL).
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
    uint64_t timestamp,
    const char* source_id,
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

#ifdef __cplusplus
}
#endif

#endif /* ALEC_H */
