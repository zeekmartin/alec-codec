# ALEC Milesight Integration — Todo

Last updated: 2026-04-14

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
- [ ] B1. MessageType::DataFixedChannel = 7
  - [ ] 4B serializer (seq u16 + ctx_ver u16)
  - [ ] ctx_ver u16 wraparound handling
  - [ ] Marker byte 0xA1
  - [ ] encode_multi_fixed() encoder path
        (no name_ids, no timestamp)

- [ ] B2. Decoder path
  - [ ] alec_decode_multi_fixed(channel_count)
  - [ ] Dispatch on 0xA1 marker byte

- [ ] B3. FFI entries
  - [ ] alec_encode_multi_fixed()
  - [ ] alec_decode_multi_fixed()
  - [ ] Update alec.h

- [ ] B4. Tests
  - [ ] Roundtrip property-based test
  - [ ] Output ≤ 11B steady state on 99-msg CSV
  - [ ] Cold start → TLV fallback first frame
  - [ ] ctx_ver wraparound at 65535
  - [ ] 0xA1 marker dispatch vs TLV

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
- context_version truncated to u16: handle wraparound
- Channel mapping lives in sidecar, NOT in decoder
- history_size=10/50/100 give identical results
  on EM500-CO2 slow-drift profile
- Fallback rate estimate: 30-60% operational
  (do not communicate to Milesight — let them validate)
