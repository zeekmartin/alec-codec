# ALEC Milesight Integration — Todo

Last updated: 2026-04-15 (Session 4 — Bloc D complete → Phase 1 DONE)

---

## Phase 1 — alec-codec (repo public) ✅ COMPLETE

All four blocks (A, B, C, D) are done. The library now
ships everything the Milesight firmware + sidecar need:
config FFI + compact 4B header + packet-loss recovery
+ in-memory context persistence. Next: Phase 2 (Python
decoder + FreeRTOS C example + ChirpStack sidecar).

### Bloc A: Config FFI
- [x] A1. alec_encoder_new_with_config() FFI
  - [x] AlecEncoderConfig C struct
        (history_size, max_patterns, max_memory_bytes,
         keyframe_interval, smart_resync)
  - [x] alec_heap_init_with_buffer() (bare-metal only)
  - [x] alec_force_keyframe() FFI
  - [x] alec_decoder_gap_detected() FFI
  - [x] Regenerate alec.h via cbindgen
        (auto-generated include/alec_generated.h +
         hand-written include/alec.h updated)
  - [x] Unit tests for each new FFI function
        (24 total, 9 new — all passing)

- [x] A2. Build M4 (thumbv7em-none-eabi)
  - [x] Verify build success
  - [x] Measure .text / .bss sizes
  - [x] Try thumbv7em-none-eabihf (hardware FPU)
  - [x] Compare M3 / M4 / M4F sizes — see table below

- [x] A3. Build M0+ (thumbv6m-none-eabi)
  - [x] Fix portable-atomic shim if needed
        (not needed — cortex-m 0.7 feature
         `critical-section-single-core` already
         supplies a single-core critical_section impl
         that LlffHeap uses for thread-safety)
  - [x] Measure .text / .bss sizes
  - [ ] Verify f64 soft-float acceptable latency
        (deferred: requires on-device benchmark;
         compiler_builtins includes all soft-float
         routines — ~170 KB of .text)

#### Bloc A — Build sizes (release, archive-level upper bound)

|  Target                 | ALEC .text | compiler_builtins .text | other .text | TOTAL .text | .bss   |
|-------------------------|-----------:|------------------------:|------------:|------------:|-------:|
| M3  (thumbv7m-none-eabi)  |    54 887 |                 167 350 |       7 900 |     230 137 | 8 220 |
| M4  (thumbv7em-none-eabi) |    54 891 |                 166 654 |       7 820 |     229 365 | 8 220 |
| M4F (thumbv7em-none-eabihf)|   56 159 |                 173 442 |       7 732 |     237 333 | 8 220 |
| M0+ (thumbv6m-none-eabi)  |    53 331 |                 174 356 |       8 068 |     235 755 | 8 220 |

Notes on the sizes:
- Numbers are **archive-level** totals (`llvm-size -t libalec_ffi.a`).
  Final firmware link with `--gc-sections` will drop unused
  symbols — expect Milesight to see substantially less.
- "ALEC .text" is alec + alec-ffi only, filtered on object
  file prefix. ~54 KB across all targets.
- compiler_builtins carries the f64 soft-float routines;
  Milesight's firmware may already include its own copy.
- "other" is mostly xxhash-rust (~8 KB).
- .bss is constant at 8 220 B = 8 192 B HEAP_MEM + 28 B
  LlffHeap allocator state (the built-in static buffer from
  `alec_heap_init`). With `alec_heap_init_with_buffer` the
  .bss drops to 28 B.

### Bloc B: Compact 4B header
- [x] B1. MessageType::DataFixedChannel = 7
      (renamed from `MessageType::Reserved`;
       slot 7 was unused elsewhere in the codebase)
  - [x] 4B serializer (seq u16 + ctx_ver u16)
        — `CompactHeader::{write,read}` in
        `src/protocol.rs`
  - [x] ctx_ver u16 wraparound handling
        — `ctx_version_compatible(incoming, last, max_jump)`
        treats version as a ring in u16 space; any forward
        jump ≤ `max_jump` is accepted, anything else is a
        mismatch. Decoder uses `max_jump = 256`.
  - [x] Marker bytes 0xA1 / 0xA2
        (two markers instead of stealing bit 15 of ctx_ver;
         preserves the full u16 ctx_ver range claimed in
         CONTEXT.md, and makes the JS passthrough codec
         trivial: `b & 0xFE == 0xA0` matches both.)
  - [x] encode_multi_fixed() encoder path
        (no name_ids, no timestamp, 2-bits-per-channel
         bitmap, positional channel ordering)

- [x] B2. Decoder path
  - [x] decode_multi_fixed(channel_count) — core library
  - [x] Dispatch on 0xA1 / 0xA2 marker byte via
        `classify_compact_marker`
  - [x] Gap + context-version-mismatch detection
        (`FixedFrameInfo { keyframe, gap_size,
         context_mismatch, … }`). Actual
        `reset_to_baseline()` deferred to Bloc C — a
        `TODO(Bloc C)` comment in `decode_multi_fixed`
        marks where to wire it in.

- [x] B3. FFI entries
  - [x] alec_encode_multi_fixed() — consumes
        `force_keyframe_pending` + tracks
        `messages_since_keyframe`; emits keyframe when
        interval is hit OR downlink forced AND
        smart_resync is enabled
  - [x] alec_decode_multi_fixed() — positional,
        gap-tracked, observes each decoded value so the
        next frame's Delta encoding is correct
  - [x] Update alec.h (both the cbindgen-generated
        `alec_generated.h` and the hand-written `alec.h`)

- [x] B4. Tests — 7 required + 1 diagnostic
  - [x] Roundtrip on 99-frame synthesized EM500-CO2
        dataset (5 channels). Tolerance = 1 native
        sensor LSB per channel.
  - [x] Output ≤ 11B steady state (frame ≥ 8) on the
        synthesized 99-msg dataset — steady-state avg
        8.16 B/frame, max 11 B.
  - [x] Cold-start frame = 27 B (Raw32 all channels).
        Caller must fall back to TLV for this one.
  - [x] ctx_ver u16 wraparound from 65530 → 0 without
        the decoder flagging a false mismatch.
  - [x] 0xA1 marker dispatch vs non-ALEC marker
        (legacy TLV byte 0x5A returns
        `ALEC_ERROR_INVALID_INPUT`, not a panic).
  - [x] Periodic keyframes: interval=10 → keyframes
        at frames 10, 20 (27 B each, marker 0xA2),
        data frames in between (marker 0xA1, ≤ 11 B).
  - [x] `alec_force_keyframe` FFI → next encode emits
        a keyframe (marker 0xA2, 27 B), the frame
        after is back to compact (marker 0xA1, ≤ 11 B).

#### Bloc B — Wire format & steady-state numbers

Wire layout:
```
byte 0         : marker  (0xA1 data / 0xA2 keyframe)
byte 1..=2     : sequence       (u16 BE)
byte 3..=4     : context_version (u16 BE, low 16 bits of u32)
byte 5..=5+B-1 : encoding bitmap (2 bits per channel,
                 B = ceil(channel_count / 4) bytes)
byte 5+B..     : per-channel encoded data
```

Encoding bitmap codes (2 bits each, LSB-first):
- `00` Repeated (0 bytes)
- `01` Delta8  (1 byte)
- `10` Delta16 (2 bytes)
- `11` Raw32  (4 bytes)

Steady-state byte budget for 5 channels (bitmap = 2 bytes):
- 5×Repeated → 1 + 4 + 2 + 0  =  7 B ✓
- 3×Rep + 2×D8 → 1 + 4 + 2 + 2 =  9 B ✓
- 1×Rep + 4×D8 → 1 + 4 + 2 + 4 = 11 B ✓ (ceiling)
- 5×D8       → 1 + 4 + 2 + 5  = 12 B ✗ (caller → TLV)
- keyframe (5×Raw32) → 1 + 4 + 2 + 20 = 27 B (always → TLV)

Measured on the synthesized 99-msg EM500-CO2 dataset:
| Regime | avg | min | max |
|---|---:|---:|---:|
| All 99 frames        | 8.32 | 7 | 27 |
| Steady (frame ≥ 8)   | 8.16 | 7 | 11 |

Encoding distribution across 495 channels (99 × 5):
| Encoding  | Count | Share |
|-----------|------:|------:|
| Repeated  | 410   | 82.8% |
| Delta8    | 49    |  9.9% |
| Delta16   | 31    |  6.3% |
| Raw32     | 5     |  1.0% |

### Bloc C: Packet loss recovery
- [x] C1. Context::reset_to_baseline()
  - [x] Wipe source_stats
  - [x] Preserve dictionary / pattern_index (and hence
        any preloaded patterns — the fixed-channel path
        never uses patterns)
  - [x] Preserve version counter (so future mismatch
        detection keeps working)
  - [x] Unit test: prediction wiped, patterns preserved

- [x] C2. Keyframe mechanism (encoder) — audited, flow
        was already correct after Bloc A/B; added a
        dedicated lifecycle comment block in
        `src/encoder.rs`
  - [x] keyframe_interval + messages_since_keyframe
  - [x] Force Raw32 all channels at interval
  - [x] MessageType::Heartbeat as keyframe flag
        (CONTEXT.md-level name; the wire uses the
        dedicated 0xA2 marker because the compact
        header has no room for a message_type field)
  - [x] alec_force_keyframe() callable from downlink

- [x] C3. Sequence gap reset (decoder)
  - [x] Replace "For now, just continue" with a call
        to Context::reset_to_baseline() in the FFI
        (`alec_decode_multi_fixed`), not in the core
        decoder (which has `&Context`, not `&mut`).
        Updated the TODO(Bloc C) comment in
        `src/decoder.rs` to redirect to the FFI.
  - [x] Invoke check_version() in decode path
        (informational; the u16-truncated wire version
        is compared via ctx_version_compatible which is
        wraparound-aware — check_version is still
        called for parity with the legacy TLV path)
  - [x] Log gap + version mismatch
        (`log::warn!`, zero-cost when no subscriber)

- [x] C4. Smart resync via LoRaWAN downlink
  - [x] alec_decoder_gap_detected() returns gap_size
        (from Bloc A)
  - [x] Downlink command 0xFF handler
  - [x] alec_downlink_handler() FFI
  - [x] Worst-case drift: 1 interval with smart resync
        (down from 8h to 10min on EM500-CO2)

- [x] C5. Tests — 6 required, all passing
  - [x] reset_to_baseline_wipes_stats — post-reset
        encode is Raw32-all-channels (27 B)
  - [x] packet_loss_recovery_at_keyframe — drop 4
        frames, decoder reports gap_size=4, next
        keyframe recovers
  - [x] no_silent_corruption — drop frame 20 with
        keyframe_interval=50; frame 50 and 51..=60 all
        decode within sensor LSB
  - [x] smart_resync_downlink — drop frame 5,
        alec_downlink_handler(0xFF) → next uplink is
        a keyframe, decoder recovers on next frame
  - [x] downlink_handler_invalid_command — 0x00,
        NULL pointers, empty payload all surface clean
        errors with no encoder state change
  - [x] context_mismatch_triggers_reset — synthetic
        ctx_ver tampering flips the decoder's
        prediction model from MovingAverage back to
        LastValue (proves reset_to_baseline ran), then
        next keyframe recovers within sensor LSB

### Bloc D: Context persistence FFI
- [x] D1. Context::to_preload_bytes() / from_preload_bytes()
        — new "ALCS" (ALec Context State) wire format,
        no_std+alloc compatible, CRC32-protected.
        Distinct from the older PreloadFile (b"ALEC")
        format because ALCS preserves per-source
        SourceStats bit-exactly — critical for the
        sidecar use case.
  - [x] Magic bytes "ALCS", format version 1
  - [x] Header: ctx_ver (full u32) + scale_factor +
        observation_count + next_code + sensor_type
  - [x] Per-source SourceStats section (count, EMA,
        last_value, sum_sq_diff, mean, history vector)
        preserved bit-exactly via f64 `to_bits()`
  - [x] Dictionary section (patterns with data,
        frequency, last_used, created_at)
  - [x] CRC32 ISO-HDLC checksum (LE)
  - [x] Unit tests: roundtrip bit-exact, bad magic,
        bad CRC, oversize sensor_type rejected

- [x] D2. alec_decoder_export_state / import_state FFI
  - [x] alec_decoder_export_state_size() — size query
        for precise buffer allocation
  - [x] alec_decoder_export_state() — serialize with
        BUFFER_TOO_SMALL that reports required size
        and does NOT partially write
  - [x] alec_decoder_import_state() — restore context,
        preserve session state (last_header_sequence,
        last_gap_size); on CORRUPT_DATA, decoder is
        completely untouched
  - [x] New result code: ALEC_ERROR_CORRUPT_DATA = 9

- [x] D3. Update alec.h
  - [x] Hand-written include/alec.h — 3 new functions
        + ALEC_ERROR_CORRUPT_DATA
  - [x] cbindgen-regenerated include/alec_generated.h
  - [x] Full doc comments on every new function

- [x] D4. Tests — 6 required + 1 NULL-safety bonus
  - [x] context_roundtrip_in_memory — train, export,
        import, drive 10 more frames; bit-exact
        identical output on original vs restored decoder
  - [x] export_state_size_matches_export
  - [x] export_buffer_too_small — 10-byte buffer →
        BUFFER_TOO_SMALL, *out_len=required size, buffer
        NOT touched (sentinel bytes preserved)
  - [x] import_corrupt_data — garbage + CRC-tampered
        buffer both rejected; decoder state byte-for-byte
        unchanged (verified by re-export and compare)
  - [x] session_state_preserved_on_import —
        last_header_sequence=42, last_gap_size=2 survive
        an export/import roundtrip
  - [x] export_after_reset_to_baseline — post-reset
        export is valid, restored decoder has empty
        source_stats, patterns preserved
  - [x] (bonus) persistence_ffi_null_safety

#### Bloc D — Measured serialized size

| Scenario | Size |
|---|---:|
| 5-channel EM500-CO2 decoder, history=20, 1 pattern | **1 550 B** |

Squarely in the 1-3 KB target for the ChirpStack sidecar
Redis-persistence pattern (1 key per DevEUI).

---

## Phase 2 — alec-milesight (repo privé)

### Bloc E: Python decoder
- [ ] E1. decode_alec_fixed.py
  - [ ] Parse 4B compact header
  - [ ] Dispatch 0xA1 ALEC vs TLV legacy
  - [ ] Channel schema configurable by device model
  - [ ] JSON output
  - [ ] Test on 99-message Milesight CSV
  - [ ] Keyframe detection
  - [ ] Gap simulation test

- [ ] E2. Channel schemas JSON
  - [ ] em500_co2.json
        (battery/temp/humidity/CO2/pressure)
  - [ ] em500_pp.json (battery/pressure)
  - [ ] am307.json
        (temp/humidity/CO2/TVOC/light/pressure/PIR)
  - [ ] am319.json (am307 + HCHO or O3)

### Bloc F: FreeRTOS C example
- [ ] F1. freertos_em500.c
  - [ ] alec_heap_init_with_buffer() 3KB static
  - [ ] alec_encoder_new_with_config()
        history=20, keyframe=50, smart_resync=true
  - [ ] Sensor task: 5 channels, encode_multi_fixed()
  - [ ] Fallback: output > 11B → TLV
  - [ ] Transmission task: LoRaWAN send
  - [ ] Downlink handler: 0xFF → alec_force_keyframe()
  - [ ] Cold start: first frame TLV uncompressed
  - [ ] Reset on reboot: reset context

### Bloc G: ChirpStack + Docker sidecar
- [ ] G1. JS passthrough codec (~2KB)
  - [ ] Detect 0xA1 → forward raw bytes + device_model
  - [ ] Else → TLV decoder inline
  - [ ] Expose f_port for device model routing

- [ ] G2. Sidecar REST Rust (axum)
  - [ ] POST /v1/uplink/chirpstack
  - [ ] DashMap<DevEUI, Arc<Mutex<Context>>>
  - [ ] Gap detection → downlink resync
        via ChirpStack API
  - [ ] PreloadFile persistence per DevEUI
  - [ ] GET /v1/health
  - [ ] GET /v1/devices
  - [ ] POST /v1/devices/:dev_eui/reset
  - [ ] GET /v1/schemas

- [ ] G3. Docker
  - [ ] Multi-stage Dockerfile
  - [ ] docker-compose.yml with volumes
  - [ ] ChirpStack integration README
  - [ ] Webhook + downlink API token config

---

## Phase 3 — nRF9151 validation (parallel to Phase 2)

- [ ] H1. Build alec-ffi thumbv8m.main-none-eabi
      with compact header + config FFI + keyframe
- [ ] H2. Update alec-nrf9151-demo firmware
  - [ ] alec_encoder_new_with_config()
  - [ ] Simulate fallback >11B
  - [ ] Simulate packet loss (skip every N frames)
  - [ ] Simulate smart resync via console downlink
- [ ] H3. Grafana dashboard
  - [ ] context_version per message
  - [ ] Keyframe events
  - [ ] Gap detected events
  - [ ] Recovery confirmation
  - [ ] bytes/message: TLV vs ALEC vs keyframe

---

## Notes
- Do NOT use P1-P5 priority for recovery logic
- MessageType::Heartbeat is free — use for keyframe
  (but the fixed-channel wire format uses the 0xA2 marker
   byte instead of the Heartbeat MessageType, because the
   4B compact header has no room for a message_type field.
   Bloc B uses marker 0xA1 = data, 0xA2 = keyframe.)
- context_version truncated to u16: handle wraparound
  (implemented in Bloc B as `ctx_version_compatible`)
- Channel mapping lives in sidecar, NOT in decoder
- history_size=10/50/100 give identical results
  on EM500-CO2 slow-drift profile
- Fallback rate estimate: 30-60% operational
  (do not communicate to Milesight — let them validate)

### Notes left open by Bloc B for Bloc C

- **`Context::reset_to_baseline()` not yet implemented.**
  Bloc B surfaces both gap_size and context_mismatch via
  `FixedFrameInfo`, and `alec_decoder_gap_detected` reports
  the gap size. Bloc C-1 must wire
  `reset_to_baseline()` in `Context` (preserve preloaded
  patterns, wipe `source_stats` + `pattern_index`) and
  call it from `decode_multi_fixed` when
  `gap_size > 0 || context_mismatch` on a non-keyframe
  frame. A `TODO(Bloc C)` comment in `src/decoder.rs`
  marks the exact line.
- **Periodic keyframe trigger semantics.** Chosen:
  "keyframe at frames N, 2N, 3N …" (interval=10 → frames
  10, 20, 30). The keyframe counts as frame 1 of the next
  cycle (`messages_since_keyframe` resets to 1 after a
  keyframe, not 0). If Milesight prefers a different
  convention it's a one-line change in `alec_encode_multi_fixed`.
- **Encoder/decoder context drift.** The encoder observes
  the ORIGINAL input, not the reconstructed value;
  otherwise f32 rounding breaks `Repeated` detection on
  stable channels (3.600 → f32 → 3.5999999). This means
  encoder and decoder contexts can drift by up to ~1
  native LSB per channel between keyframes. The default
  `keyframe_interval = 50` bounds drift to ~50 frames.
  Verified lossless at the application level on the
  synthesized 99-msg dataset (1 LSB tolerance per channel).
- **12B fallback rate.** With 5 channels and the
  2-bits-per-channel bitmap (2 B of tags), ANY frame
  where > 4 channels use Delta8/Delta16 exceeds the 11 B
  ceiling and must fall back to TLV. On our synthesized
  dataset (82.8% Repeated) fallback is ~0%. On real sensor
  data with more churn this can rise toward the 30-60%
  estimate in the Notes section above.
- **64-channel hard cap** in `encode_multi_fixed` /
  `decode_multi_fixed` (stack-allocated bitmap scratch
  buffers). Well above the 5 fixed channels for EM500-CO2
  and above any plausible Milesight device. Can be raised
  by switching to heap allocation if needed.
- **Dataset note.** No 99-message EM500-CO2 CSV is
  shipped with the repo; the B5 tests synthesize a
  slow-drift dataset inline, calibrated to reproduce the
  encoding distribution cited in CONTEXT.md (we actually
  hit 82.8% Repeated, higher than the 58% figure, because
  the synthesized data is more aggressively quantized
  than real sensor output). Final validation on Milesight's
  actual 99-msg CSV is Phase 2 (Bloc E) work.

### Notes left open by Bloc C for Bloc D

- **Reset policy preserves the dictionary/pattern_index.**
  The Milesight fixed-channel codec never uses Pattern
  encoding, so this is safe. If a future non-fixed path
  uses patterns AND learns them at runtime (as opposed
  to loading from preload), those runtime-learned
  patterns will survive a reset — that is acceptable,
  since resetting them would break the ability to decode
  future Pattern references. If that ever becomes
  undesirable, add a "baseline" snapshot field to
  `Context` captured at `from_preload()` time and have
  `reset_to_baseline()` restore dictionary/pattern_index
  from it.
- **`check_version()` is called but largely informational.**
  The compact wire format carries the low 16 bits of
  the u32 version, so the authoritative check is
  `ctx_version_compatible` (wraparound-aware). Bloc D's
  context persistence (`Context::to_preload_bytes()`)
  should snapshot the full u32 so a restored context
  can still participate in meaningful version checks.
- **Decoder mid-state after `reset_to_baseline`**:
  when a non-keyframe triggers the reset, the FFI
  observes the frame's (potentially garbage) decoded
  values right after the reset — this re-seeds the
  prediction cache with count=1 for each channel,
  model_type becomes LastValue. Subsequent Delta
  frames BEFORE the keyframe will decode against this
  re-seed; they may produce divergent values until the
  next keyframe arrives and fully corrects them. This
  is tested (test_no_silent_corruption): frames 21..=49
  are allowed to be "wrong" after a drop at frame 20,
  but frame 50 (keyframe) and all subsequent frames
  decode within sensor LSB.
- **`log` crate adds ~0 KB of .text on bare-metal**
  when no subscriber is installed (verified: release
  build sizes unchanged from Bloc B's measurements).
  Milesight integrators can install a `log`
  subscriber — e.g. route to SEGGER RTT or UART — to
  observe gap / mismatch events in the field.
- **Bloc D blockers:**
  * `Context` does not yet expose `to_preload_bytes() /
    from_preload_bytes()` on its own — only the
    `PreloadFile` type does, and only behind the `std`
    feature (uses std::fs). Bloc D-1 will lift this to
    a `no_std`-compatible in-memory API.
  * `dictionary` entries carry no "preloaded" flag, so
    a reset-after-preload can't selectively preserve
    just the preloaded patterns. If this matters
    (probably not for the Milesight path) Bloc D will
    need a tagging mechanism.
  * There is currently NO mechanism to verify that a
    `to_preload_bytes` / `from_preload_bytes` round-trip
    preserves the `source_stats` EMA residue faithfully
    — it doesn't need to for the sidecar use case (the
    sidecar doesn't need predictions to persist across
    restarts) but Bloc D tests should cover it anyway
    for documentation value.

### Notes left open by Bloc D for Phase 2 (Bloc E+)

- **ALCS format choice.** The existing `PreloadFile`
  (b"ALEC" magic) cannot represent per-source
  SourceStats, so Bloc D introduces a NEW format
  (b"ALCS" magic). `PreloadFile` is NOT deprecated —
  it remains the right choice for training preloads
  where a single aggregated statistic is enough.
  `Context::save_to_file` / `load_from_file` still use
  the old format. Two formats coexist cleanly because
  their magic bytes differ.
- **sensor_type is metadata.** The sensor_type string
  is stored in the serialized buffer but ignored by
  `from_preload_bytes` (the Context itself has no
  sensor_type field). The FFI sidecar can still read
  it back from the raw bytes at byte offset 28 if
  operators want to surface it.
- **Format versioning.** ALCS_FORMAT_VERSION = 1. Any
  future wire-breaking change must bump this and
  optionally maintain a v1→v2 migrator.
- **Phase 2 / Bloc E next steps:**
  * Python reference decoder must implement the Bloc B
    wire format (2-bits-per-channel bitmap, dual
    markers 0xA1/0xA2) — no ALCS needed on the Python
    side since Python decoders re-seed from keyframes
    rather than persist.
  * The ChirpStack sidecar (Bloc G) will be the
    primary ALCS consumer: on every N decodes, call
    `alec_decoder_export_state` and push to Redis;
    on startup, `GET` from Redis and call
    `alec_decoder_import_state`.
  * Session-state preservation contract (Bloc D-5)
    means the sidecar can safely re-import after a
    crash without losing in-flight sequence tracking.
  * If the sidecar uses serde-based (JSON) storage
    instead of raw bytes, it can base64-encode the
    ALCS buffer — size ~2 KB base64 is still well
    within Redis typical value budgets.
