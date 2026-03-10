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

### encode_multi() — adaptive compression (context-aware)

**Status:** Planned — v1.3
**Priority:** High
**Discovered during:** Nordic nRF9151 NB-IoT demo (March 2026)

**Problem:**
`alec_encode_multi()` unconditionally uses Raw32 encoding for every value.
The context (BTreeMap patterns, EMA, last_value) is populated via `observe()`
after each call but never consulted during encoding. The adaptive compression
path (delta, repeated, interpolated) only exists in `alec_encode_value()`.

**Impact:**
- 5-channel payload: always 99 bytes (5 × 19B headers + raw values)
- No compression benefit regardless of message count
- Workaround: call `alec_encode_value()` per channel and concatenate —
  but this multiplies per-message header overhead (5 × 13B = 65B headers)

**Requested change:**
Implement context-aware encoding in `encode_multi()` matching the decision
tree in `encode()`: Repeated → Delta8 → Delta16 → Delta32 → Raw32 → Raw64.
Per-channel context must be keyed by channel index or source_id.

---

### Protocol header overhead reduction for multi-channel payloads

**Status:** Planned — v1.3
**Priority:** Medium

**Problem:**
Each `alec_encode_value()` call produces a standalone message with:
- 13-byte fixed header (version, type, priority, sequence, timestamp,
  context_version)
- 1-byte source_id varint
- 1-byte encoding type
- value bytes

For a 5-channel device sending temp/rh/pressure/ts/seq, the per-message
overhead is 5 × ~15B = 75B of headers for ~20B of actual sensor data.

**Requested change:**
Design a compact multi-value frame format that shares a single header
across all channels in one transmission. Target: ≤20B total for a
5-channel payload once context is warm, vs current 99B.

---

## [v1.4] — Pattern & Interpolated Encoding

### Pattern encoding (EncodingType::Pattern = 0x20)

**Status:** Planned
**Priority:** High

EncodingType::Pattern and PatternDelta exist in the enum and wire format
but the encoder decision tree never selects them. The encoder always falls
through to Raw32/Raw64 when Delta32 is insufficient.

Pattern encoding would match repeating signal signatures in the dictionary
(e.g. day/night temperature cycles, periodic consumption curves) and encode
the entire pattern as a 1-2 byte dictionary reference.

**Expected gain:** 80-95% compression on periodic real-world signals.

**Blocked by:** dictionary population logic in Context, pattern matching
in the encoder decision tree.

### Interpolated encoding (EncodingType::Interpolated = 0x31)

**Status:** Planned
**Priority:** Medium

Interpolated encodes 0 bytes when the actual value matches the EMA
prediction within tolerance. Currently the encoder checks for exact
Repeated match only — it never evaluates prediction accuracy.

**Expected gain:** significant reduction on smooth sensor curves
(temperature drift, pressure trends) where EMA converges quickly.

**Blocked by:** tolerance threshold design (fixed vs adaptive per channel).

---

## Platform Notes

### Zephyr allocator — alignment requirement (resolved in v1.2.4)

- `k_malloc` on Zephyr returns 4-byte aligned memory by default.
- alec-ffi v1.2.3 and earlier passed only `layout.size()` to `k_malloc`,
  discarding alignment — causing rc=5 on Cortex-M33 targets.
- Fixed in v1.2.4 via `k_aligned_alloc`.
- Minimum viable `CONFIG_HEAP_MEM_POOL_SIZE` on nRF9151: 65536 bytes (64 KB)
  out of 211 KB available NS zone (~30% — comfortable).
