# ALEC Milesight Integration — Todo

Last updated: 2026-04-14 (Session 2 — Bloc B complete)

---

## Phase 1 — alec-codec (repo public)

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
- [ ] C1. Context::reset_to_baseline()
  - [ ] Wipe source_stats + pattern_index
  - [ ] Preserve preloaded patterns

- [ ] C2. Keyframe mechanism (encoder)
  - [ ] keyframe_interval + messages_since_keyframe
  - [ ] Force Raw32 all channels at interval
  - [ ] MessageType::Heartbeat as keyframe flag
  - [ ] alec_force_keyframe() callable from downlink

- [ ] C3. Sequence gap reset (decoder)
  - [ ] Replace "For now, just continue"
        with reset_to_baseline()
  - [ ] Invoke check_version() in decode path
  - [ ] Log gap + version mismatch

- [ ] C4. Smart resync via LoRaWAN downlink
  - [ ] alec_decoder_gap_detected() returns gap_size
  - [ ] Downlink command 0xFF handler
  - [ ] alec_downlink_handler() FFI
  - [ ] Worst-case drift: 1 interval with smart resync

- [ ] C5. Tests
  - [ ] Encode 100 frames, drop frame 20
  - [ ] Verify corruption frames 20→keyframe
  - [ ] Verify recovery at keyframe N=50
  - [ ] Verify immediate recovery with smart resync
  - [ ] No silent corruption on any path

### Bloc D: Context persistence FFI
- [ ] D1. Context::to_preload_bytes()
- [ ] D1. Context::from_preload_bytes()
- [ ] D2. alec_decoder_export_state() FFI
- [ ] D2. alec_decoder_import_state() FFI
- [ ] D3. Update alec.h
- [ ] D3. Round-trip binary tests

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
