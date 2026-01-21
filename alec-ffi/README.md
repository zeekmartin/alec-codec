# ALEC FFI - C/C++ Bindings

C/C++ bindings for the ALEC (Adaptive Lazy Evolving Compression) library.

## Overview

This crate provides a C-compatible FFI layer for ALEC, enabling use from C/C++ firmware and embedded systems.

## Quick Start

### C Example

```c
#include "alec.h"

int main() {
    // Create encoder
    AlecEncoder* enc = alec_encoder_new();

    // Encode a sensor reading
    uint8_t output[256];
    size_t output_len;

    AlecResult res = alec_encode_value(
        enc,
        22.5,           // value
        0,              // timestamp
        "sensor_1",     // source_id
        output,
        sizeof(output),
        &output_len
    );

    if (res == ALEC_OK) {
        printf("Compressed to %zu bytes\n", output_len);
    }

    // Cleanup
    alec_encoder_free(enc);
    return 0;
}
```

## Building

### Build the Library

```bash
cd alec-ffi
cargo build --release
```

This produces:
- `target/release/libalec.a` - Static library
- `target/release/libalec.so` (Linux) / `libalec.dylib` (macOS) - Shared library

### Compile C Code

**With static linking:**
```bash
gcc -o myapp myapp.c \
    -I alec-ffi/include \
    target/release/libalec.a \
    -lpthread -ldl -lm
```

**With dynamic linking:**
```bash
gcc -o myapp myapp.c \
    -I alec-ffi/include \
    -L target/release \
    -lalec \
    -lpthread -ldl -lm

# Set library path before running
export LD_LIBRARY_PATH=target/release:$LD_LIBRARY_PATH
./myapp
```

## API Reference

### Types

| Type | Description |
|------|-------------|
| `AlecEncoder*` | Opaque encoder handle |
| `AlecDecoder*` | Opaque decoder handle |
| `AlecResult` | Result code enum |

### Result Codes

| Code | Value | Description |
|------|-------|-------------|
| `ALEC_OK` | 0 | Success |
| `ALEC_ERROR_INVALID_INPUT` | 1 | Invalid input data |
| `ALEC_ERROR_BUFFER_TOO_SMALL` | 2 | Output buffer too small |
| `ALEC_ERROR_ENCODING_FAILED` | 3 | Encoding failed |
| `ALEC_ERROR_DECODING_FAILED` | 4 | Decoding failed |
| `ALEC_ERROR_NULL_POINTER` | 5 | NULL pointer provided |
| `ALEC_ERROR_INVALID_UTF8` | 6 | Invalid UTF-8 string |
| `ALEC_ERROR_FILE_IO` | 7 | File I/O error |
| `ALEC_ERROR_VERSION_MISMATCH` | 8 | Context version mismatch |

### Encoder Functions

```c
// Create/destroy
AlecEncoder* alec_encoder_new(void);
AlecEncoder* alec_encoder_new_with_checksum(void);
void alec_encoder_free(AlecEncoder* encoder);

// Encoding
AlecResult alec_encode_value(
    AlecEncoder* encoder,
    double value,
    uint64_t timestamp,
    const char* source_id,
    uint8_t* output,
    size_t output_capacity,
    size_t* output_len
);

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

// Context management
AlecResult alec_encoder_save_context(AlecEncoder* encoder, const char* path, const char* sensor_type);
AlecResult alec_encoder_load_context(AlecEncoder* encoder, const char* path);
uint32_t alec_encoder_context_version(const AlecEncoder* encoder);
```

### Decoder Functions

```c
// Create/destroy
AlecDecoder* alec_decoder_new(void);
AlecDecoder* alec_decoder_new_with_checksum(void);
void alec_decoder_free(AlecDecoder* decoder);

// Decoding
AlecResult alec_decode_value(
    AlecDecoder* decoder,
    const uint8_t* input,
    size_t input_len,
    double* value,
    uint64_t* timestamp
);

AlecResult alec_decode_multi(
    AlecDecoder* decoder,
    const uint8_t* input,
    size_t input_len,
    double* values,
    size_t values_capacity,
    size_t* values_count
);

// Context management
AlecResult alec_decoder_load_context(AlecDecoder* decoder, const char* path);
uint32_t alec_decoder_context_version(const AlecDecoder* decoder);
```

### Utility Functions

```c
const char* alec_version(void);
const char* alec_result_to_string(AlecResult result);
```

## Using Preloads

Preloads allow instant optimal compression by loading pre-trained contexts:

```c
// Encoder side
AlecEncoder* enc = alec_encoder_new();
alec_encoder_load_context(enc, "temperature_preload.alec-context");

// Decoder side (must use same preload!)
AlecDecoder* dec = alec_decoder_new();
alec_decoder_load_context(dec, "temperature_preload.alec-context");
```

## Cross-Compilation

### ARM Cortex-M4

```bash
rustup target add thumbv7em-none-eabihf
cargo build --release --target thumbv7em-none-eabihf
```

### ARM Cortex-M0

```bash
rustup target add thumbv6m-none-eabi
cargo build --release --target thumbv6m-none-eabi
```

**Note:** Cross-compilation for `no_std` targets requires additional configuration in the core ALEC library.

## Thread Safety

- Each encoder/decoder instance is **not** thread-safe
- Use separate instances for each thread, or protect access with mutexes
- The library itself has no global state

## Memory Management

- All `*_new()` functions allocate memory
- All `*_free()` functions deallocate memory
- Never use a handle after calling its `*_free()` function
- Passing `NULL` to `*_free()` functions is safe (no-op)

## License

AGPL-3.0 or Commercial License. See LICENSE file for details.
