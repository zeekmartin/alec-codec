# Implementation Plan: v1.3 — encode_multi() Adaptive Compression

## Problem Statement

Current `encode_multi()` (encoder.rs:344–384) uses a **naive flat format**: 1 shared header (13B) + source_id varint (1B) + Multi encoding tag (1B) + count (1B), then **per-channel**: 2B name_id + 1B encoding tag + 4B Raw32 = **7 bytes per channel, always Raw32, no context, no classification**.

For 5 channels: 13 + 1 + 1 + 1 + (5 × 7) = **51 bytes**. Five separate `encode_value()` calls at best-case (Repeated) = 5 × 15 = 75B, so current multi saves on headers but wastes on encoding.

The goal: shared header + adaptive per-channel encoding + priority-based inclusion = **13B + ~1–2B per warm channel** in the common IoT case (slow drift / hold).

## Wire Format v1.3

```
[Header 13B] [source_id varint 1B] [0x40 Multi] [count 1B] [channel entries...]

Per-channel entry:
  [name_id 2B BE] [priority 3 bits | encoding_type 5 bits = 1B] [value 0–8B]

Encoding type codes (already defined, fit in 5 bits):
  0x00 Raw64(8B) 0x01 Raw32(4B) 0x10 Delta8(1B) 0x11 Delta16(2B)
  0x12 Delta32(4B) 0x30 Repeated(0B) 0x31 Interpolated(0B)
```

**Key design decision**: Pack priority (3 bits) and encoding type (5 bits) into a single byte per channel. This replaces the current encoding-type-only byte, and all existing EncodingType values fit in 5 bits (max = 0x40 = 64, but we remap inside multi to use 0–31 range).

Actually — simpler approach: keep encoding_type as a full byte (already defined, decoder expects it), and add a **1-byte priority prefix per channel** only when priorities are non-uniform. But even simpler: the priority is a per-*frame* concept at the header level and channels either appear or don't. Let's follow the spec literally:

**Revised**: Keep existing Multi tag byte. The `count` byte now counts only *included* channels (P5 excluded). Each channel entry has:
- `name_id` (2B) — identifies the channel
- `encoding_type` (1B) — full byte, same codes as single-value
- `value` (0–8B) — depends on encoding type

P5 channels: context updated (observe), but NOT written to payload.
P4 channels: included only if total frame size stays under a configurable cap (default: 127B, fits in one BLE ATT_MTU).

## Files to Change

### 1. `src/encoder.rs` — Replace `encode_multi()`

Current signature:
```rust
pub fn encode_multi(
    &mut self,
    values: &[(u16, f64)],
    source_id: u32,
    timestamp: u64,
    priority: Priority,
    context: &Context,
) -> EncodedMessage
```

New signature:
```rust
pub fn encode_multi_adaptive(
    &mut self,
    channels: &[ChannelInput],
    timestamp: u64,
    context: &Context,
    classifier: &Classifier,
) -> EncodedMessage
```

Where `ChannelInput` is a new struct:
```rust
pub struct ChannelInput {
    pub name_id: u16,
    pub source_id: u32,
    pub value: f64,
}
```

**Algorithm**:
1. For each channel, build a `RawData` with the channel's `source_id` and classify it
2. Sort channels by priority (P1 first)
3. Build payload:
   - source_id varint (0 for multi — frame-level, not channel-level)
   - 0x40 (Multi tag)
   - count byte (placeholder, filled after filtering)
   - For each channel where priority != P5:
     - `name_id` (2B BE)
     - Call `choose_encoding()` with channel's source_id + context → get encoding_type + encoded_value
     - Write encoding_type byte + encoded_value
   - If P4 and frame would exceed cap, stop including P4 channels
4. Fill count byte with actual included count
5. Return EncodedMessage with shared header

Keep old `encode_multi()` unchanged for backward compatibility.

### 2. `src/decoder.rs` — Update `decode_multi()`

The per-channel encoding type byte is already there in the current format. The decoder already reads `encoding_type` per channel — but currently only handles Raw32. Extend the match to handle all encoding types using the existing `decode_value()` infrastructure.

The decoder needs a `source_id` per channel for delta/repeated decoding. We'll derive it the same way: `name_id` as implicit source_id for multi frames (or pass through a mapping). Simplest: use `name_id as u32` as the per-channel source_id within multi frames.

### 3. `alec-ffi/src/lib.rs` — New `alec_encode_multi()` signature

Replace current `alec_encode_multi`:
```rust
pub extern "C" fn alec_encode_multi(
    encoder: *mut AlecEncoder,
    values: *const f64,        // was f32, now f64 for consistency
    value_count: usize,
    timestamps: *const u64,    // NEW: per-channel timestamps, or NULL for shared
    source_ids: *const *const c_char, // NEW: array of strings, or NULL
    priorities: *const u8,     // NEW: per-channel priority (1-5), or NULL = all P3
    output: *mut u8,
    output_capacity: usize,
    output_len: *mut usize,
) -> AlecResult
```

Implementation:
- Build `Vec<ChannelInput>` from the C arrays
- Hash each source_id string via `hash_source_id()`
- Call `encoder.encode_multi_adaptive()`
- Observe all channels in context (including P5)

### 4. `alec-ffi/include/alec.h` — Update declaration

Mirror the new Rust signature in C.

### 5. `src/protocol.rs` — Add `ChannelInput` struct

Add:
```rust
pub struct ChannelInput {
    pub name_id: u16,
    pub source_id: u32,
    pub value: f64,
}
```

### 6. Version bumps

- `Cargo.toml` (root): `1.2.4` → `1.3.0`
- `alec-ffi/Cargo.toml`: `1.2.5` → `1.3.0`
- `alec-ffi/src/lib.rs` version string: `"1.2.5\0"` → `"1.3.0\0"`
- `alec-ffi/Cargo.toml` dependency: `version = "1.2"` → `version = "1.3"`

### 7. `CHANGELOG.md` — New `[1.3.0]` section

### 8. Tests (new file: `tests/multi_adaptive.rs`)

**test_encode_multi_adaptive**:
- Warm up context with 20 observations per channel (5 channels, slow drift)
- Encode with `encode_multi_adaptive`
- Verify encoding types are Delta8/Delta16/Repeated, not Raw32

**test_encode_multi_p5_suppression**:
- Set thresholds so some channels classify as P5
- Encode multi
- Verify output count < input count
- Verify P5 channels still observed in context

**test_encode_multi_shared_header**:
- Encode 5 channels via multi vs 5 × encode_value
- Assert multi total < sum of individual totals
- Target: multi should be < 50% of 5 × encode_value after warmup

## Execution Order

1. Add `ChannelInput` to `src/protocol.rs`
2. Add `encode_multi_adaptive()` to `src/encoder.rs`
3. Update `decode_multi()` in `src/decoder.rs` to handle all encoding types
4. Update FFI: `alec-ffi/src/lib.rs` new signature
5. Update `alec-ffi/include/alec.h`
6. Version bumps in both Cargo.toml files + version string
7. Add `tests/multi_adaptive.rs`
8. Update existing FFI test `test_encode_multi` to use new signature
9. `CHANGELOG.md` — add `[1.3.0]` section
10. `cargo test` — all pass
11. Commit, push to `claude/encode-multi-adaptive-v1.3`

## Risks / Open Questions

- **Decoder source_id mapping**: In multi frames, each channel needs a source_id for delta/repeated decode. Using `name_id as u32` is simple but means the decoder's context must have been trained with the same mapping. This is fine because the context is always kept in sync.
- **Backward compatibility**: Old `encode_multi()` is kept as-is. New `encode_multi_adaptive()` produces frames with the same Multi tag (0x40) but per-channel encoding types differ. Old decoders that only handle Raw32 will fail on new frames — this is a **breaking wire format change** for multi, hence the minor version bump to 1.3.
- **P4 cap**: Default 127B matches BLE ATT_MTU. Could be configurable via a new `MultiConfig` struct, but YAGNI for now — hardcode the cap.
