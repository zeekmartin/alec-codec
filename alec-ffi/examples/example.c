/**
 * ALEC C Example - Basic Usage
 *
 * This example demonstrates how to use the ALEC compression library from C.
 *
 * Compile with:
 *   gcc -o example example.c -I../include -L../../target/release -lalec -lpthread -ldl -lm
 *
 * Or with static linking:
 *   gcc -o example example.c -I../include ../../target/release/libalec.a -lpthread -ldl -lm
 */

#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include "alec.h"

/* Simulated temperature sensor readings */
static const double temperature_readings[] = {
    22.1, 22.2, 22.1, 22.3, 22.2, 22.4, 22.3, 22.5,
    22.4, 22.6, 22.5, 22.7, 22.6, 22.5, 22.4, 22.3
};

static const size_t NUM_READINGS = sizeof(temperature_readings) / sizeof(temperature_readings[0]);

/**
 * Demonstrate basic encoding of sensor values
 */
int example_basic_encoding(void) {
    printf("\n=== Basic Encoding Example ===\n");

    /* Create encoder */
    AlecEncoder* enc = alec_encoder_new();
    if (!enc) {
        fprintf(stderr, "Failed to create encoder\n");
        return 1;
    }

    /* Buffer for compressed output */
    uint8_t compressed[256];
    size_t compressed_len;
    size_t total_compressed = 0;

    /* Encode each reading */
    for (size_t i = 0; i < NUM_READINGS; i++) {
        AlecResult res = alec_encode_value(
            enc,
            temperature_readings[i],
            i * 1000,  /* timestamp in ms */
            "temp_sensor_1",
            compressed,
            sizeof(compressed),
            &compressed_len
        );

        if (res != ALEC_OK) {
            fprintf(stderr, "Encoding failed: %s\n", alec_result_to_string(res));
            alec_encoder_free(enc);
            return 1;
        }

        total_compressed += compressed_len;
        printf("  Value %.1f -> %zu bytes\n", temperature_readings[i], compressed_len);
    }

    /* Calculate statistics */
    size_t original_size = NUM_READINGS * sizeof(double);
    double ratio = (1.0 - (double)total_compressed / original_size) * 100;

    printf("\nResults:\n");
    printf("  Original size:   %zu bytes (%zu values x %zu bytes)\n",
           original_size, NUM_READINGS, sizeof(double));
    printf("  Compressed size: %zu bytes\n", total_compressed);
    printf("  Compression:     %.1f%%\n", ratio);
    printf("  Context version: %u\n", alec_encoder_context_version(enc));

    /* Cleanup */
    alec_encoder_free(enc);
    return 0;
}

/**
 * Demonstrate multi-value encoding
 */
int example_multi_encoding(void) {
    printf("\n=== Multi-Value Encoding Example ===\n");

    AlecEncoder* enc = alec_encoder_new();
    if (!enc) {
        fprintf(stderr, "Failed to create encoder\n");
        return 1;
    }

    uint8_t compressed[512];
    size_t compressed_len;

    /* Encode all readings at once */
    AlecResult res = alec_encode_multi(
        enc,
        temperature_readings,
        NUM_READINGS,
        0,  /* timestamp */
        "temp_sensor_batch",
        compressed,
        sizeof(compressed),
        &compressed_len
    );

    if (res != ALEC_OK) {
        fprintf(stderr, "Multi-encoding failed: %s\n", alec_result_to_string(res));
        alec_encoder_free(enc);
        return 1;
    }

    size_t original_size = NUM_READINGS * sizeof(double);
    double ratio = (1.0 - (double)compressed_len / original_size) * 100;

    printf("  Original size:   %zu bytes (%zu values)\n", original_size, NUM_READINGS);
    printf("  Compressed size: %zu bytes\n", compressed_len);
    printf("  Compression:     %.1f%%\n", ratio);

    alec_encoder_free(enc);
    return 0;
}

/**
 * Demonstrate checksum usage
 */
int example_with_checksum(void) {
    printf("\n=== Encoding with Checksum Example ===\n");

    AlecEncoder* enc = alec_encoder_new_with_checksum();
    if (!enc) {
        fprintf(stderr, "Failed to create encoder\n");
        return 1;
    }

    uint8_t compressed[256];
    size_t compressed_len;

    AlecResult res = alec_encode_value(
        enc,
        22.5,
        1234567890,
        "checksum_test",
        compressed,
        sizeof(compressed),
        &compressed_len
    );

    if (res != ALEC_OK) {
        fprintf(stderr, "Encoding with checksum failed: %s\n", alec_result_to_string(res));
        alec_encoder_free(enc);
        return 1;
    }

    printf("  Encoded with checksum: %zu bytes\n", compressed_len);
    printf("  (Checksum adds ~4 bytes for integrity verification)\n");

    alec_encoder_free(enc);
    return 0;
}

/**
 * Main entry point
 */
int main(void) {
    printf("ALEC C Bindings Example\n");
    printf("Version: %s\n", alec_version());
    printf("========================\n");

    int result = 0;

    result |= example_basic_encoding();
    result |= example_multi_encoding();
    result |= example_with_checksum();

    if (result == 0) {
        printf("\n=== All examples completed successfully ===\n");
    }

    return result;
}
