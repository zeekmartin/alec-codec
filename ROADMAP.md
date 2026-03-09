# ALEC Codec — Roadmap

## v1.3 Planned Features

### Configurable encoder context size (`alec_encoder_new_with_options`)

**Status:** Planned — v1.3
**Priority:** Medium
**Discovered during:** Nordic nRF9151 NB-IoT validation (Zephyr RTOS, March 2026)

**Problem:**
`alec_encoder_new()` provides no way to cap the encoder's internal memory footprint (dictionary + prediction tables). On MCUs with limited heap (<64 KB), the internal context allocation silently fails, returning a valid encoder pointer but causing `alec_encode_multi()` to fail with `rc=5` (null pointer).

Validated RAM requirements by target:

| SoC | Total RAM | 64 KB heap | Viable |
|-----|-----------|------------|--------|
| nRF9151 | 211 KB NS | 30% | Comfortable |
| nRF9160 | 256 KB | 25% | Comfortable |
| STM32L4 | 128 KB | 50% | Tight |
| STM32L0 | 20 KB | — | Not viable |

**Proposed API addition (C FFI):**

```c
typedef struct {
    size_t max_context_bytes;  // 0 = default (no cap)
    uint8_t max_channels;      // 0 = default
} AlecEncoderOptions;

AlecEncoder *alec_encoder_new_with_options(const AlecEncoderOptions *opts);
```

**Expected behavior:**

- `max_context_bytes` caps internal heap usage.
- If requested size is below functional minimum, return NULL with documented minimum.
- Document minimum viable `max_context_bytes` per channel count.

**Workaround (current):** Set `CONFIG_HEAP_MEM_POOL_SIZE=65536` in `prj.conf` (Zephyr). Valid on nRF9151/nRF9160. Not viable on STM32L0/SAMD21 class devices.

## Platform Notes

### Zephyr allocator — alignment requirement (resolved in v1.2.4)

- `k_malloc` on Zephyr returns 4-byte aligned memory by default.
- alec-ffi v1.2.3 and earlier passed only `layout.size()` to `k_malloc`,
  discarding alignment — causing rc=5 on Cortex-M33 targets.
- Fixed in v1.2.4 via `k_aligned_alloc`.
- Minimum viable `CONFIG_HEAP_MEM_POOL_SIZE` on nRF9151: 65536 bytes (64 KB)
  out of 211 KB available NS zone (~30% — comfortable).
