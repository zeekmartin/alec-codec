# ALEC — Milesight Integration Context

## Partner
Milesight IoT — EM500-CO2 sensor (STM32WLE5CCU6)
Contact: Stephen (PM) + Lin (R&D engineer)
Repo privé intégration: alec-milesight (à créer)

## Hardware target
- MCU: STM32WLE5CCU6 — Cortex-M3 (primary), M4, M0+
- OS: FreeRTOS
- RAM libre: 4.5 KB
- Flash: coordonné en interne par Milesight
- Toolchain: GCC ARM 9.3.1
- Target Rust: thumbv7em-none-eabi (M4 first),
  thumbv7m-none-eabi (M3), thumbv6m-none-eabi (M0+)

## Constraints
- LoRaWAN hard payload ceiling: 11 bytes
- Channels: 5 fixed (battery, temperature, humidity,
  CO2, pressure) — order fixed at compile time
- No device timestamp — server applies on receipt
- history_size: 20 (validated on 99-message dataset)
- Keyframe interval: 50 (every ~8h at 10min interval)

## Benchmark results (99 messages EM500-CO2 real data)
- Milesight TLV current: 18.0 B/message
- ALEC v1.3.1 (current header): 17.1 B → −5%
- ALEC fixed-channel 4B header: 6.1 B → −66%
- Battery estimate: 8.5 years → 13.9 years
- Encoding distribution: 58% Repeated, 42% Delta8
- history_size 100/50/10: identical results

## Wire format
- Marker byte: 0xA1 = ALEC fixed-channel frame
- Header: 4 bytes (seq u16 BE + ctx_ver u16 BE)
- No name_ids, no timestamp in header
- Fallback: if encoded > 11B → existing TLV format
- Cold start: first frame always TLV (uncompressed)
- Keyframe: MessageType::Heartbeat repurposed,
  forces Raw32 for all channels

## Packet loss recovery
- Periodic keyframe every N=50 transmissions
- Sequence gap detection → reset_to_baseline()
- Smart resync: downlink 0xFF → alec_force_keyframe()
- Worst-case drift: 8h (keyframe) or 10min (smart resync)

## Server-side architecture
- Network server: ChirpStack (confirmed by Milesight)
- Decoder pattern: JS passthrough codec (~2KB)
  + Docker sidecar (Rust/axum)
- No WASM in NS codec functions (all three NS sandbox JS)
- 1 encoder → 1 decoder per DevEUI
- Context persistence: PreloadFile::to_bytes() per DevEUI
- Smart resync: sidecar detects gap → downlink via
  ChirpStack API → encoder sends keyframe immediately

## Delivery strategy
Deliver progressively in 3 milestones:
1. M4 build + Python decoder (unblocks Lin immediately)
2. Full firmware package (compact header + fallback +
   keyframe + multi-arch)
3. ChirpStack sidecar + Docker + smart resync

## Key architectural decisions
- Channel mapping lives in sidecar, NOT in decoder
- Priority P1-P5 has NO role in recovery — ignore it
- MessageType::Heartbeat repurposed for keyframe signal
- context_version truncated to u16 (65535 increments,
  wraparound handled in decoder)
- AlecEncoderConfig exposes: history_size, max_patterns,
  max_memory_bytes, keyframe_interval, smart_resync

## Files of interest
- src/encoder.rs: encode_multi_adaptive, choose_encoding,
  encode_multi_fixed (Bloc B), keyframe lifecycle block
  (Bloc C)
- src/decoder.rs: last_sequence, decode_multi_fixed
  (Bloc B), FixedFrameInfo { keyframe, gap_size,
  context_mismatch }
- src/protocol.rs: MessageHeader, MessageType::Heartbeat,
  MessageType::DataFixedChannel, CompactHeader,
  classify_compact_marker, ctx_version_compatible
- src/context/mod.rs: check_version(), version increment,
  reset_to_baseline() (Bloc C)
- src/context/preload.rs: PreloadFile::to_bytes/from_bytes
- src/sync.rs: SyncState::Diverged, check_sync_needed()
- src/recovery.rs: CircuitBreaker (exists, not wired)
- alec-ffi/src/lib.rs: alec_encoder_new(),
  alec_encode_multi_fixed, alec_decode_multi_fixed,
  alec_force_keyframe, alec_downlink_handler, HEAP_MEM

## Wire format corrections (Bloc B findings)
- Dual markers: 0xA1 = data frame, 0xA2 = keyframe
  (two dedicated markers instead of stealing bit 15 of
   ctx_ver; preserves the full u16 ctx_ver range so
   wraparound at 65535→0 is detectable by the decoder)
- JS passthrough codec dispatch: `b & 0xFE == 0xA0`
  matches both markers in one comparison
- Encoding bitmap: 2 bits per channel
  (4 options: Repeated / Delta8 / Delta16 / Raw32 —
   no Delta32 in the fixed wire format)
- Bitmap size: 2 bytes for 5 channels
- Actual wire for 5 channels:
  1B marker + 4B header + 2B bitmap + per-ch data
- Steady-state avg: 8.16 B/frame (NOT the 6.1 B of the
  earlier benchmark — the 6.1 B figure did not model
  the 2-byte bitmap overhead)
- Cold-start / keyframe frames for 5 channels = 27 B
  (Raw32 ×5 + overhead) → always falls back to TLV for
  that specific frame
- Max steady-state frame = 11 B (= ceiling) when
  1×Repeated + 4×Delta8; 5 Delta8 → 12 B → TLV
- Fallback to TLV: compare encoded length against 11 B;
  on mismatch emit the legacy TLV frame instead

## Packet loss recovery flow (Bloc C)
- Encoder side (FFI owns the decision):
  * messages_since_keyframe counter, reset to 1 (not 0)
    on every keyframe so keyframes land at frames N,
    2N, 3N … with interval N
  * force_keyframe_pending flag, set by
    alec_force_keyframe() or by alec_downlink_handler()
    when it sees byte 0 == 0xFF
  * keyframe = (counter >= interval) OR
              (force_flag && smart_resync)
- Decoder side (FFI applies the policy):
  * decode_multi_fixed reports FixedFrameInfo with
    keyframe, gap_size, context_mismatch
  * FFI calls Context::reset_to_baseline() iff:
       (gap_size > 0 || context_mismatch) && !keyframe
  * Keyframes never trigger a reset — their Raw32
    payload fully rebuilds the per-channel prediction
    state on its own (the FFI observes each decoded
    value after the decode)
- Context::reset_to_baseline():
  * Wipes source_stats (per-channel EMA, last_value,
    history, variance)
  * Preserves dictionary + pattern_index (keeps
    preloaded patterns) — in the fixed-channel path
    these are never used anyway
  * Preserves version counter (keeps future mismatch
    detection working)
- Downlink protocol:
  * Byte 0 == 0xFF → "request immediate keyframe" →
    alec_force_keyframe()
  * Any other first byte → ALEC_ERROR_INVALID_INPUT,
    encoder state unchanged
  * Bytes after byte 0 reserved, currently ignored
- Worst-case drift after a dropped uplink:
  * Without smart resync (sidecar disabled):
    keyframe_interval × uplink_period
    (default 50 × 10 min ≈ 8 h on EM500-CO2)
  * With smart resync (sidecar detects gap and sends
    0xFF downlink): 1 × uplink_period (10 min)
- Logging: `log::warn!` at the FFI layer on every gap
  or ctx-mismatch event; no-op if no subscriber is
  installed (zero-cost on embedded)
