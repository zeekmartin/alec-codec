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
- src/encoder.rs: encode_multi_adaptive, choose_encoding
- src/decoder.rs: last_sequence, gap detection hook (line 52)
- src/protocol.rs: MessageHeader, MessageType::Heartbeat
- src/context/mod.rs: check_version(), version increment
- src/context/preload.rs: PreloadFile::to_bytes/from_bytes
- src/sync.rs: SyncState::Diverged, check_sync_needed()
- src/recovery.rs: CircuitBreaker (exists, not wired)
- alec-ffi/src/lib.rs: alec_encoder_new(), HEAP_MEM
