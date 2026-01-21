# ALEC C/C++ Bindings (FFI)

> **Destination**: `alec-codec` repository → `docs/FFI.md`

This document describes how to use ALEC from C and C++ projects.

## Overview

The `alec-ffi` crate provides C-compatible bindings for the ALEC compression library. It produces both static (`libalec_ffi.a`) and shared (`libalec_ffi.so`/`.dylib`) libraries.

## Building

### Prerequisites

- Rust toolchain (1.70+)
- C compiler (gcc, clang, or MSVC)

### Build Commands

```bash
# Clone the repository
git clone https://github.com/zeekmartin/alec-codec.git
cd alec-codec

# Build release libraries
cargo build --release -p alec-ffi

# Output locations:
# - target/release/libalec_ffi.a     (static)
# - target/release/libalec_ffi.so    (shared, Linux)
# - target/release/libalec_ffi.dylib (shared, macOS)
```

### Cross-Compilation

```bash
# For ARM Cortex-M (embedded)
rustup target add thumbv7em-none-eabihf
cargo build --release -p alec-ffi --target thumbv7em-none-eabihf

# For ESP32
cargo build --release -p alec-ffi --target xtensa-esp32-none-elf
```

## Integration

### Header File

Include the header from `alec-ffi/include/alec.h`:

```c
#include "alec.h"
```

### Linking

**GCC/Clang (Linux):**
```bash
gcc -o myapp myapp.c -I/path/to/alec-ffi/include -L/path/to/target/release -lalec_ffi -lpthread -ldl -lm
```

**GCC/Clang (macOS):**
```bash
gcc -o myapp myapp.c -I/path/to/alec-ffi/include -L/path/to/target/release -lalec_ffi
```

**CMake:**
```cmake
add_library(alec_ffi STATIC IMPORTED)
set_target_properties(alec_ffi PROPERTIES
    IMPORTED_LOCATION "${CMAKE_SOURCE_DIR}/lib/libalec_ffi.a"
    INTERFACE_INCLUDE_DIRECTORIES "${CMAKE_SOURCE_DIR}/include"
)
target_link_libraries(myapp PRIVATE alec_ffi pthread dl m)
```

## API Quick Reference

### Encoder

| Function | Description |
|----------|-------------|
| `alec_encoder_new()` | Create encoder with defaults |
| `alec_encoder_new_with_checksum()` | Create encoder with checksum enabled |
| `alec_encoder_free(enc)` | Free encoder |
| `alec_encode_value(enc, value, ts, src, out, cap, &len)` | Encode single value |
| `alec_encode_multi(enc, vals, count, ts, src, out, cap, &len)` | Encode multiple values |
| `alec_encoder_save_context(enc, path, type)` | Save context to file |
| `alec_encoder_load_context(enc, path)` | Load preload file |
| `alec_encoder_context_version(enc)` | Get context version |

### Decoder

| Function | Description |
|----------|-------------|
| `alec_decoder_new()` | Create decoder |
| `alec_decoder_new_with_checksum()` | Create decoder with checksum verification |
| `alec_decoder_free(dec)` | Free decoder |
| `alec_decode_value(dec, in, len, &val, &ts)` | Decode single value |
| `alec_decode_multi(dec, in, len, vals, cap, &count)` | Decode multiple values |
| `alec_decoder_load_context(dec, path)` | Load preload file |

### Utility

| Function | Description |
|----------|-------------|
| `alec_version()` | Get library version string |
| `alec_result_to_string(res)` | Convert error code to string |

### Result Codes

| Code | Value | Meaning |
|------|-------|---------|
| `ALEC_OK` | 0 | Success |
| `ALEC_ERROR_INVALID_INPUT` | 1 | Invalid input data |
| `ALEC_ERROR_BUFFER_TOO_SMALL` | 2 | Output buffer too small |
| `ALEC_ERROR_ENCODING_FAILED` | 3 | Encoding failed |
| `ALEC_ERROR_DECODING_FAILED` | 4 | Decoding failed |
| `ALEC_ERROR_NULL_POINTER` | 5 | NULL pointer provided |
| `ALEC_ERROR_INVALID_UTF8` | 6 | Invalid UTF-8 string |
| `ALEC_ERROR_FILE_IO` | 7 | File I/O error |
| `ALEC_ERROR_VERSION_MISMATCH` | 8 | Context version mismatch |

## Example

```c
#include <stdio.h>
#include "alec.h"

int main(void) {
    // Create encoder and decoder
    AlecEncoder* enc = alec_encoder_new();
    AlecDecoder* dec = alec_decoder_new();

    // Encode a temperature reading
    double temperature = 22.5;
    uint8_t buffer[64];
    size_t encoded_len;

    AlecResult res = alec_encode_value(
        enc,
        temperature,
        0,              // timestamp
        NULL,           // source_id
        buffer,
        sizeof(buffer),
        &encoded_len
    );

    if (res == ALEC_OK) {
        printf("Encoded %zu bytes\n", encoded_len);

        // Decode
        double decoded;
        uint64_t timestamp;

        res = alec_decode_value(dec, buffer, encoded_len, &decoded, &timestamp);
        if (res == ALEC_OK) {
            printf("Decoded: %.2f\n", decoded);
        }
    }

    // Cleanup
    alec_encoder_free(enc);
    alec_decoder_free(dec);

    return 0;
}
```

## Using Preloads

Load a preload file for instant optimal compression:

```c
AlecEncoder* enc = alec_encoder_new();

// Load temperature preload
AlecResult res = alec_encoder_load_context(enc, "temperature.alec-context");
if (res != ALEC_OK) {
    fprintf(stderr, "Failed to load preload: %s\n", alec_result_to_string(res));
}

// Now encoding will use the pre-trained context
// ...
```

## Thread Safety

- Each encoder/decoder instance is **not** thread-safe
- Use separate instances for each thread, or protect with mutexes
- The library itself has no global state

## Memory Usage

Typical memory footprint:
- Encoder: ~2-4 KB (depends on context size)
- Decoder: ~1-2 KB
- Per-value encoding: No additional allocation

## Full Header Documentation

See [`alec-ffi/include/alec.h`](../alec-ffi/include/alec.h) for complete API documentation with detailed parameter descriptions.

## License

ALEC is dual-licensed under AGPL-3.0 and a commercial license. See [LICENSE](../LICENSE) for details.
