# Implementation Plan: Protocol Header V2 — 3 Fixes (v1.3.1)

## Files Touched (8 files)

| File | Nature of Change |
|------|-----------------|
| `src/protocol.rs` | MessageHeader struct (sequence u32→u16), SIZE 13→10, to_bytes/from_bytes rewritten, doc comment |
| `src/encoder.rs` | Encoder.sequence u32→u16, next_sequence() return u16, encode_raw() takes &Context, 4 timestamp sites ÷1000, MessageBuilder.sequence() takes u16 |
| `src/decoder.rs` | Decoder.last_sequence Option<u32>→Option<u16>, last_sequence() return type, wrapping_add type |
| `tests/protocol_header_v2.rs` | **NEW** — 8 regression tests (written first, must fail before fixes) |
| `tests/multi_adaptive.rs` | Line 218: assertion message "26B" → "20B" |
| `CHANGELOG.md` | Add [1.3.1] - 2026-03-10 entry |
| `Cargo.toml` | version "1.3.0" → "1.3.1" |
| `alec-ffi/Cargo.toml` | version "1.3.0" → "1.3.1" |

## Byte Layout Change

```
BEFORE (13 bytes, SIZE=13):
  [0]       header byte (version 2b | type 3b | priority 3b)
  [1..5]    sequence       u32  4 bytes BE
  [5..9]    timestamp      u32  4 bytes BE
  [9..13]   context_ver    u32  4 bytes BE

AFTER (10 bytes, SIZE=10):
  [0]       header byte (version 2b | type 3b | priority 3b)
  [1..3]    sequence       u16  2 bytes BE
  [3..7]    timestamp      u32  4 bytes BE
  [7..10]   context_ver    u24  3 bytes BE (stored as u32, serialized as 3B)
```

Saving: 3 bytes per frame.

## Execution Order

### Phase 1: Write failing regression tests

Create `tests/protocol_header_v2.rs` with 8 tests. They must compile and FAIL on current code (SIZE==13, sequence is u32, timestamp is truncated ms, encode_raw context_version==0).

Tests:
1. **test_timestamp_seconds_not_ms** — RawData ts=1_741_234_567_000 ms → header.timestamp == 1_741_234_567 (seconds)
2. **test_timestamp_no_49day_wrap** — 50 days in ms (4_320_000_000) → header.timestamp == 4_320_000 (seconds)
3. **test_sequence_u16_rollover** — 65,536 encode() calls → sequence wraps to 0, then 1
4. **test_sequence_2_bytes_in_header** — MessageHeader::SIZE == 10, sequence at bytes[1..3]
5. **test_context_version_u24_range** — context_version=0x00ABCDEF roundtrips through serialize/deserialize
6. **test_context_version_3_bytes_in_header** — bytes[7..10] == [0x00, 0x00, 0xFF] for cv=255
7. **test_header_roundtrip_all_fields** — full roundtrip: version=1, Sync, P2, seq=60000u16, ts=1_741_234_567, cv=0x00AABBCC
8. **test_encode_raw_context_version_not_zero** — NaN value → encode_raw → context_version != 0

### Phase 2: Fix 1 — Timestamp ÷1000

**encoder.rs** — 4 timestamp construction sites:
- Line 249: `(data.timestamp & 0xFFFFFFFF) as u32` → `(data.timestamp / 1000) as u32`
- Line 274: same
- Line 379: `(timestamp & 0xFFFFFFFF) as u32` → `(timestamp / 1000) as u32`
- Line 480: same

**encoder.rs** — encode_raw() context_version fix:
- Line 257: change signature to add `context: &Context` parameter
- Line 275: `context_version: 0` → `context_version: context.version()`
- Line 225 (call site): add `context` argument

### Phase 3: Fix 2 — Sequence u32 → u16

**protocol.rs:**
- Line 240: `pub sequence: u32` → `pub sequence: u16`
- Line 261: `pub const SIZE: usize = 13` → `pub const SIZE: usize = 10` (combined with Fix 3)
- Line 283: `bytes[1..5].copy_from_slice(&self.sequence.to_be_bytes())` → `bytes[1..3]`
- Line 284: `bytes[5..9]` → `bytes[3..7]` (timestamp shifts left by 2)
- Line 285: context_version serialization → u24 at bytes[7..10] (combined with Fix 3)
- Line 299: `u32::from_be_bytes` → `u16::from_be_bytes([bytes[1], bytes[2]])`
- Line 300: timestamp → `u32::from_be_bytes([bytes[3], bytes[4], bytes[5], bytes[6]])`
- Line 301: context_version → `u32::from_be_bytes([0, bytes[7], bytes[8], bytes[9]])`
- Line 230 doc: `13 bytes` → `10 bytes`

**encoder.rs:**
- Line 59: `sequence: u32` → `sequence: u16`
- Line 111: `pub fn sequence(&self) -> u32` → `-> u16`
- Line 338: `fn next_sequence(&mut self) -> u32` → `-> u16`
- Line 539: `pub fn sequence(mut self, seq: u32)` → `seq: u16`

**decoder.rs:**
- Line 25: `last_sequence: Option<u32>` → `Option<u16>`
- Line 472: `pub fn last_sequence(&self) -> Option<u32>` → `-> Option<u16>`

### Phase 4: Fix 3 — context_version u24 serialization

**protocol.rs to_bytes():**
```rust
let cv = self.context_version & 0x00FFFFFF;
bytes[7..10].copy_from_slice(&cv.to_be_bytes()[1..]);
```

**protocol.rs from_bytes():**
```rust
let context_version = u32::from_be_bytes([0, bytes[7], bytes[8], bytes[9]]);
```

Field type stays `pub context_version: u32` — only wire format changes.

### Phase 5: Update existing tests

- **protocol.rs** existing tests: sequence values (12345, 42) fit u16, auto-OK. SIZE refs auto-update.
- **tests/multi_adaptive.rs:218**: change string `"26B"` → `"20B"` in assertion message
- **encoder.rs** tests: `.sequence(42)` fits u16. SIZE refs auto-update.

### Phase 6: Verify

1. `cargo test --release` — all tests pass (existing 178 + 8 new)
2. `cargo clippy -- -D warnings` — zero warnings

### Phase 7: Changelog & version bump

**CHANGELOG.md** — prepend:
```
## [1.3.1] - 2026-03-10

### Fixed
- Timestamp stored as Unix seconds (÷1000) instead of truncated milliseconds — fixes silent wrap every 49 days
- encode_raw() now uses context.version() instead of hardcoded 0

### Changed
- MessageHeader::sequence reduced from u32 to u16 (-2B per frame)
- MessageHeader::context_version serialized as u24 (-1B per frame)
- MessageHeader::SIZE reduced from 13 to 10
- Total header saving: 3B per frame
```

**Cargo.toml**: `version = "1.3.0"` → `"1.3.1"`
**alec-ffi/Cargo.toml**: `version = "1.3.0"` → `"1.3.1"`

### Phase 8: Commit & push

Single commit to branch `claude/alec-no-std-support-2iXfW`.
