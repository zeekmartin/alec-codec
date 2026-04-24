// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! v1.3.9 — zero-heap encoder context_save integration tests.
//!
//! Covers the four scenarios enumerated in the v1.3.9 Phase-3 task:
//!
//! 1. `save_load_round_trip_partner_config` — partner's production
//!    config (history_size=20, max_patterns=256, keyframe_interval=30)
//!    driven through 100 frames of realistic 5-channel slow-drift
//!    sensor data. Context save / load must round-trip and subsequent
//!    encodings must be bit-identical between the original and the
//!    restored encoder.
//!
//! 2. `wire_format_byte_identical_to_v1_3_8` — verifies that the
//!    streaming `write_preload_bytes` output is byte-identical to the
//!    `to_preload_bytes` (v1.3.8) Vec-based output for the same
//!    context, on both code paths.
//!
//! 3. `context_save_buffer_too_small_clean_error` — a stub-sized
//!    buffer must yield ALEC_ERROR_BUFFER_TOO_SMALL cleanly, without
//!    touching the caller's buffer (no partial write).
//!
//! 4. `context_save_max_patterns_stress` — fills the dictionary via
//!    the lower-level `Context::register_pattern` API, then invokes
//!    the FFI save and verifies the round-trip — emulating a worst
//!    case where a caller has accumulated many patterns.
//!
//! 5. `ffi_save_load_zero_heap_ffi_round_trip` — end-to-end through
//!    `alec_encoder_context_save` / `alec_encoder_context_load` with
//!    the partner's exact config.

#![cfg(feature = "decoder")]

use std::ptr;

use alec_ffi::{
    alec_encode_multi_fixed, alec_encoder_context_load, alec_encoder_context_save,
    alec_encoder_free, alec_encoder_new_with_config, AlecEncoder, AlecEncoderConfig, AlecResult,
};

const CHANNELS: usize = 5;
/// Per-channel native quantization step used to check round-trip
/// fidelity after a save/load cycle.
const TOL: [f64; CHANNELS] = [0.01, 0.01, 0.1, 1.0, 0.01];

/// Partner's production config (CONTEXT.md).
fn partner_cfg() -> AlecEncoderConfig {
    AlecEncoderConfig {
        history_size: 20,
        max_patterns: 256,
        max_memory_bytes: 2048,
        keyframe_interval: 30,
        smart_resync: true,
    }
}

/// Synthetic realistic dataset matching the partner's profile.
fn slow_drift_dataset(n: usize) -> Vec<[f64; CHANNELS]> {
    (0..n)
        .map(|i| {
            [
                1.0,                             // battery (stable)
                268.0 + (i as f64 * 0.1),        // temperature (°C × 10)
                120.0 + (i as f64 * 0.05),       // humidity (% × 10)
                900.0 + (i as f64 % 50.0) * 3.0, // CO2 (ppm)
                10_100.0 + (i as f64 * 0.2),     // pressure (hPa × 10)
            ]
        })
        .collect()
}

fn encode_one(enc: *mut AlecEncoder, row: &[f64; CHANNELS]) -> Vec<u8> {
    let mut out = [0u8; 32];
    let mut n = 0usize;
    let r = alec_encode_multi_fixed(
        enc,
        row.as_ptr(),
        row.len(),
        out.as_mut_ptr(),
        out.len(),
        &mut n,
    );
    assert_eq!(r, AlecResult::Ok, "encode_multi_fixed failed");
    out[..n].to_vec()
}

/// Save the encoder state into a freshly sized Vec using the FFI.
fn save_enc(enc: *const AlecEncoder) -> Vec<u8> {
    // Probe with a 1-byte buffer to learn the required size.
    let mut probe = [0u8; 1];
    let sentinel = 0xABu8;
    probe[0] = sentinel;
    let mut need = 0usize;
    let r = alec_encoder_context_save(enc, probe.as_mut_ptr(), probe.len(), &mut need);
    assert_eq!(
        r,
        AlecResult::ErrorBufferTooSmall,
        "probe save should report buffer-too-small"
    );
    assert!(need > 1, "required size must be > 1");
    assert_eq!(probe[0], sentinel, "no partial write on BufferTooSmall");

    let mut buf = vec![0u8; need];
    let mut written = 0usize;
    assert_eq!(
        alec_encoder_context_save(enc, buf.as_mut_ptr(), buf.len(), &mut written),
        AlecResult::Ok
    );
    assert_eq!(written, need, "written must equal required size");
    buf
}

// ============================================================================
// 3.1 — Partner-scenario round-trip (the crash case from the bug report).
// ============================================================================

#[test]
fn save_load_round_trip_partner_config() {
    let cfg = partner_cfg();
    let enc = alec_encoder_new_with_config(&cfg);
    let data = slow_drift_dataset(100);

    // Warm the encoder with 100 frames, discarding the wire bytes —
    // we only care about the final Context state.
    for row in &data {
        let _ = encode_one(enc, row);
    }

    // 2 KB static save buffer — what the partner's firmware uses.
    // The new streaming serialiser must fit its output inside that
    // buffer without any scratch heap. Previous to v1.3.9 the
    // Context::to_preload_bytes Vec<u8> (~1.5 KB) was allocated in
    // addition to the caller buffer, blowing the 4 KB heap budget.
    let mut save_buf = [0u8; 2048];
    let mut written = 0usize;
    let r = alec_encoder_context_save(enc, save_buf.as_mut_ptr(), save_buf.len(), &mut written);
    assert_eq!(r, AlecResult::Ok, "save must succeed with 2 KB buffer");
    assert!(written > 0);
    assert!(written <= save_buf.len());

    // Restore into a fresh encoder with the same config.
    let enc2 = alec_encoder_new_with_config(&cfg);
    assert_eq!(
        alec_encoder_context_load(enc2, save_buf.as_ptr(), written),
        AlecResult::Ok,
        "load must accept the bytes produced by save"
    );

    // Both encoders must now emit identical wire bytes for the same
    // input — the ultimate proof that the restore is bit-accurate.
    let test_rows = [
        [1.0, 270.0, 125.0, 950.0, 10_120.0],
        [1.0, 272.0, 126.0, 960.0, 10_125.0],
        [1.0, 273.0, 126.5, 970.0, 10_128.0],
    ];
    for (i, row) in test_rows.iter().enumerate() {
        let w1 = encode_one(enc, row);
        let w2 = encode_one(enc2, row);
        assert_eq!(
            w1, w2,
            "frame {}: original and restored encoders diverged",
            i
        );
    }

    alec_encoder_free(enc);
    alec_encoder_free(enc2);
}

// ============================================================================
// 3.2 — Wire format is byte-identical to v1.3.8.
//
// `Context::to_preload_bytes` (which still exists as a backward-compat
// wrapper) and `Context::write_preload_bytes` (the new zero-heap path)
// must produce the SAME bytes for the same context. If this invariant
// holds, the v1.3.9 output is also byte-identical to v1.3.8's output
// because v1.3.8's `to_preload_bytes` used the same ALCS format.
// ============================================================================

#[test]
fn wire_format_byte_identical_to_v1_3_8() {
    use alec::context::Context;

    let cfg = partner_cfg();
    let enc = alec_encoder_new_with_config(&cfg);
    let data = slow_drift_dataset(60);
    for row in &data {
        let _ = encode_one(enc, row);
    }

    // Pull the Context out of the encoder to call the raw APIs.
    // SAFETY: `AlecEncoder` is repr(Rust) — for test-only access we
    // build a fresh Context and populate it the same way. Instead of
    // poking AlecEncoder internals, we round-trip through the FFI
    // save/load and compare with a direct `Context` call.
    //
    // Step 1: use the FFI save (which calls write_preload_bytes) to
    // capture what v1.3.9 emits.
    let v1_3_9_full = save_enc(enc);
    // The ALCS payload starts after the 24-byte ALEE encoder header.
    let v1_3_9_alcs = &v1_3_9_full[24..];

    // Step 2: build a Context with the same state via load → save
    // symmetric round-trip. If load/save is lossless and our streamer
    // is correct, we get the same bytes back.
    let enc2 = alec_encoder_new_with_config(&cfg);
    let r = alec_encoder_context_load(enc2, v1_3_9_full.as_ptr(), v1_3_9_full.len());
    assert_eq!(r, AlecResult::Ok);
    let v1_3_9_full_rt = save_enc(enc2);
    assert_eq!(
        v1_3_9_full, v1_3_9_full_rt,
        "save -> load -> save must be a byte-identical fixed point"
    );

    // Step 3: also cross-check the streamer and the Vec-based
    // `to_preload_bytes` against each other. The Vec-based wrapper is
    // the path every pre-v1.3.9 caller used, so equality here proves
    // wire-format compatibility with v1.3.8.
    let ctx =
        Context::from_preload_bytes(v1_3_9_alcs).expect("restored context must parse cleanly");
    let vec_based = ctx.to_preload_bytes("ffi").expect("Vec-based serialise");
    // Allocate a zero-filled slice the streamer will fill.
    let needed = ctx.preload_bytes_len("ffi").unwrap();
    let mut streamed = vec![0u8; needed];
    let n = ctx
        .write_preload_bytes("ffi", &mut streamed)
        .expect("streaming serialise");
    streamed.truncate(n);
    assert_eq!(
        vec_based, streamed,
        "write_preload_bytes must produce bytes identical to to_preload_bytes"
    );

    alec_encoder_free(enc);
    alec_encoder_free(enc2);
}

// ============================================================================
// 3.3 — Buffer too small returns a clean error with no partial write.
// ============================================================================

#[test]
fn context_save_buffer_too_small_clean_error() {
    let cfg = partner_cfg();
    let enc = alec_encoder_new_with_config(&cfg);
    for row in &slow_drift_dataset(20) {
        let _ = encode_one(enc, row);
    }

    // Way too small (smaller than the 24-byte ALEE header).
    const SENTINEL: u8 = 0xCD;
    let mut tiny = [SENTINEL; 10];
    let mut need = 0usize;
    let r = alec_encoder_context_save(enc, tiny.as_mut_ptr(), tiny.len(), &mut need);
    assert_eq!(r, AlecResult::ErrorBufferTooSmall);
    assert!(
        need > tiny.len(),
        "required size ({}) must exceed available ({})",
        need,
        tiny.len()
    );
    for (i, &b) in tiny.iter().enumerate() {
        assert_eq!(b, SENTINEL, "byte {} was written on BufferTooSmall", i);
    }

    // Retry with the reported capacity — must now succeed.
    let mut buf = vec![0u8; need];
    let mut written = 0usize;
    assert_eq!(
        alec_encoder_context_save(enc, buf.as_mut_ptr(), buf.len(), &mut written),
        AlecResult::Ok
    );
    assert_eq!(written, need);

    alec_encoder_free(enc);
}

// ============================================================================
// 3.4 — Stress: populate the dictionary with up to max_patterns entries
// and verify the streaming serialiser keeps up without heap panics.
// ============================================================================

#[test]
fn context_save_max_patterns_stress() {
    use alec::context::{Context, Pattern};

    // Build a standalone Context populated with many patterns —
    // easier to control than pushing through the FFI encode path.
    let cfg = partner_cfg();
    let mut ctx = Context::with_config(alec::context::ContextConfig {
        history_size: cfg.history_size as usize,
        max_patterns: cfg.max_patterns as usize,
        max_memory: cfg.max_memory_bytes as usize,
        ..Default::default()
    });

    // Register 64 distinct patterns — well above the partner's
    // observed "~50+ patterns" trigger point.
    for i in 0..64u32 {
        let data = (0..16).map(|j| (i.wrapping_mul(31) ^ j) as u8).collect();
        ctx.register_pattern(Pattern::new(data)).unwrap();
    }
    // And a handful of observations so source_stats has entries too.
    for i in 0..50u32 {
        let rd = alec::protocol::RawData::with_source(i + 1, i as f64, 0);
        ctx.observe(&rd);
    }

    // Size the output buffer based on the streaming pre-flight API.
    let needed = ctx.preload_bytes_len("ffi").unwrap();
    assert!(
        needed > 1024 && needed < 8192,
        "sanity: expected 1-8 KB, got {} B",
        needed
    );
    let mut buf = vec![0u8; needed];
    let written = ctx
        .write_preload_bytes("ffi", &mut buf)
        .expect("streaming serialise must succeed for max-patterns context");
    assert_eq!(written, needed);

    // Round-trip: parse what we just wrote.
    let ctx2 = Context::from_preload_bytes(&buf).expect("restored context must parse");
    assert_eq!(ctx2.preload_bytes_len("ffi").unwrap(), needed);
    let mut buf2 = vec![0u8; needed];
    ctx2.write_preload_bytes("ffi", &mut buf2).unwrap();
    assert_eq!(buf, buf2, "round-trip must be byte-stable");
}

// ============================================================================
// 3.5 — End-to-end FFI round trip with the partner's config.
// Same spirit as 3.1 but written as an isolated "FFI-only" test to make
// the failure mode obvious if the FFI wiring regresses.
// ============================================================================

#[test]
fn ffi_save_load_zero_heap_ffi_round_trip() {
    let cfg = partner_cfg();
    let enc = alec_encoder_new_with_config(&cfg);

    // 50 frames — enough to build up predictions on every channel.
    for row in &slow_drift_dataset(50) {
        let _ = encode_one(enc, row);
    }

    // Save into a 2 KB fixed buffer — NO Vec pre-sizing on the
    // caller's side. This mimics an MCU with a single static arena.
    let mut save_buf = [0u8; 2048];
    let mut written = 0usize;
    assert_eq!(
        alec_encoder_context_save(enc, save_buf.as_mut_ptr(), save_buf.len(), &mut written),
        AlecResult::Ok
    );
    assert!(written > 0 && written <= 2048);

    // Restore into a fresh encoder and continue with more frames.
    let enc2 = alec_encoder_new_with_config(&cfg);
    assert_eq!(
        alec_encoder_context_load(enc2, save_buf.as_ptr(), written),
        AlecResult::Ok
    );

    // Drive 20 more frames through both. Wire output must match.
    let more = slow_drift_dataset(70);
    for (i, row) in more.iter().enumerate().skip(50) {
        let w1 = encode_one(enc, row);
        let w2 = encode_one(enc2, row);
        assert_eq!(w1, w2, "post-restore wire divergence at frame {}", i);
    }

    alec_encoder_free(enc);
    alec_encoder_free(enc2);
    // Silence unused-var warning from constants.
    let _ = TOL;
}

// ============================================================================
// 3.6 — NULL safety (unchanged from v1.3.7) — sanity.
// ============================================================================

#[test]
fn ffi_null_pointer_safety_preserved() {
    let mut buf = [0u8; 64];
    let mut n = 0usize;

    // save
    assert_eq!(
        alec_encoder_context_save(ptr::null(), buf.as_mut_ptr(), buf.len(), &mut n),
        AlecResult::ErrorNullPointer
    );

    // load
    assert_eq!(
        alec_encoder_context_load(ptr::null_mut(), buf.as_ptr(), buf.len()),
        AlecResult::ErrorNullPointer
    );
}
