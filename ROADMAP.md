# ALEC Codec — Roadmap

## v1.3.5 — Constrained network support (in progress)

Target: validation on embedded hardware.

- [x] Compact fixed-channel wire format (4B header)
- [x] Multi-arch bare-metal (M3 / M4 / M4F / M0+)
- [x] Periodic keyframe + sequence gap recovery
- [x] LoRaWAN downlink smart resync (`0xFF` command)
- [x] In-memory context persistence (ALCS format)
- [ ] Hardware validation (Cortex-M embedded target)
- [ ] Python decoder reference implementation
- [ ] Docker sidecar reference implementation

## v1.4.0 — Pattern encoding

Target: 3–4 B/frame on periodic signals.

Pattern and Interpolated encoding are currently defined
in the encoding enum but not selected by the decision
tree. v1.4 will activate them for signals with repeating
daily/weekly cycles (environmental monitoring, energy
metering).

- [ ] Pattern detection (FFT / autocorrelation)
- [ ] Shared pattern dictionary encoder/decoder sync
- [ ] Pattern encoding selected by `choose_encoding()`
- [ ] Convergence: minimum 24h data at 10-min interval
- [ ] Server-side pattern learning + device preload
      via downlink

Expected compression: ~3–4 B/frame on signals with
strong periodic components (CO2, temperature,
humidity day/night cycles).

## v1.5.0 — Bitmap optimization

Target: 6 B floor (vs current 7 B).

- [ ] 1-bit bitmap (Repeated / Delta8 only)
- [ ] Reduces bitmap overhead 2 B → 1 B
- [ ] Fallback rate increases for high-variance signals

## Beyond

- MQTT-native sidecar
- Python / Node.js bindings (PyO3 / napi-rs)
- WASM decoder for browser / serverless

---

## Platform notes

### Zephyr allocator — alignment requirement (resolved in v1.2.4)

- `k_malloc` on Zephyr returns 4-byte aligned memory by default.
- `alec-ffi` v1.2.3 and earlier passed only `layout.size()` to
  `k_malloc`, discarding alignment — causing `rc=5` on Cortex-M33
  targets.
- Fixed in v1.2.4 via `k_aligned_alloc`.
- Minimum viable `CONFIG_HEAP_MEM_POOL_SIZE` on nRF9151: 65536 bytes
  (64 KB) out of 211 KB available NS zone (~30 % — comfortable).

### Historical notes (shipped in v1.3.x)

The following items from earlier roadmap drafts have
landed in the v1.3.x line and are no longer tracked as
open work:

- Configurable encoder context (`alec_encoder_new_with_config`,
  `AlecEncoderConfig`) — shipped in v1.3.5.
- Adaptive `encode_multi()` — shipped in v1.3.1
  (`encode_multi_adaptive`); v1.3.5 adds the compact
  fixed-channel variant (`encode_multi_fixed`).
- Compact multi-channel frame format — shipped in v1.3.5
  (1 marker + 4 header + 2 bitmap + per-channel data,
  steady-state ~8 B for 5 channels).
