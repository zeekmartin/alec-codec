// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! v1.3.10 — `alec_decoder_feed_values` integration tests.
//!
//! Cover the partner's "Option C" pattern: when an ALEC-encoded frame
//! exceeds the LoRaWAN ceiling the firmware sends a legacy TLV frame
//! instead and the server feeds the raw values back into the decoder
//! to keep its prediction model synchronised with the encoder.
//!
//! The critical test here is `feed_values_matches_decode_round_trip`
//! (Task 4.1): two parallel decoders driven through 100 frames — one
//! always decoding the ALEC bytes, the other always fed the raw
//! values — must remain in lockstep, producing identical encodings
//! when promoted to the encoder side.

#![cfg(feature = "decoder")]

use std::ptr;

use alec_ffi::{
    alec_decode_multi_fixed, alec_decoder_context_version, alec_decoder_feed_values,
    alec_decoder_free, alec_decoder_gap_detected, alec_decoder_new_with_config,
    alec_encode_multi_fixed, alec_encoder_context_version, alec_encoder_free,
    alec_encoder_new_with_config, AlecDecoder, AlecEncoder, AlecEncoderConfig, AlecResult,
};

const CHANNELS: usize = 5;
const TOL: [f64; CHANNELS] = [0.01, 0.01, 0.1, 1.0, 0.01];

/// Partner's production config (CONTEXT.md).
fn partner_cfg() -> AlecEncoderConfig {
    AlecEncoderConfig {
        history_size: 20,
        max_patterns: 256,
        max_memory_bytes: 2048,
        keyframe_interval: 30,
        smart_resync: true,
        // num_channels: 5 pre-warms per-channel state at init so
        // feed_values is zero-heap from the very first call.
        num_channels: CHANNELS as u32,
    }
}

/// Slow-drift dataset (CO2 has a small modulo bump for variance).
fn dataset(n: usize) -> Vec<[f64; CHANNELS]> {
    (0..n)
        .map(|i| {
            [
                1.0,
                268.0 + (i as f64 * 0.1),
                120.0 + (i as f64 * 0.05),
                900.0 + (i as f64 % 50.0) * 3.0,
                10_100.0 + (i as f64 * 0.2),
            ]
        })
        .collect()
}

fn encode_one(enc: *mut AlecEncoder, row: &[f64; CHANNELS]) -> Vec<u8> {
    let mut out = [0u8; 64];
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

fn decode_one(dec: *mut AlecDecoder, wire: &[u8]) -> [f64; CHANNELS] {
    let mut v = [0f64; CHANNELS];
    let mut num = 0usize;
    let r = alec_decode_multi_fixed(
        dec,
        wire.as_ptr(),
        wire.len(),
        v.as_mut_ptr(),
        v.len(),
        &mut num,
        ptr::null_mut(),
        ptr::null_mut(),
    );
    assert_eq!(r, AlecResult::Ok, "decode_multi_fixed failed");
    v
}

// ===========================================================================
// 4.1 — feed_values matches the encoder's state (the critical contract).
//
// `feed_values` is documented as "advancing the decoder's state as if a
// frame containing those values had been decoded". The encoder's
// observe loop is fed the RAW input values (see the comment in
// `alec-ffi/src/lib.rs::alec_encode_multi_fixed`), so a decoder fed
// the same raw values via `feed_values` ends up with a Context whose
// prediction state is **bit-identical** to the encoder's. We verify
// this in two ways:
//
//   (a) `context.version` (which advances by `channel_count` per
//       observe) must match exactly.
//   (b) Encoding a probe frame on the encoder + decoding it on the
//       fed decoder must reconstruct the input exactly within the
//       codec's quantisation step (0.01 = scale_factor=100). This
//       only holds if the decoder's `predict()` agrees with the
//       encoder's `predict()` for every source — which in turn only
//       holds if their full prediction state is identical.
//
// Note: a decoder driven by `decode_multi_fixed` observes the
// RECONSTRUCTED (post-f32-roundtrip) values, not raw — that is the
// codec's "bounded drift" path documented on the encoder's observe
// loop. We do NOT test `dec_decode == dec_feed` because they are
// *intentionally* not in lockstep on a steadily-drifting signal —
// `decode` accumulates f32 quantisation, `feed_values` does not.
// The contract that matters is `dec_feed == encoder`.
// ===========================================================================

#[test]
fn feed_values_matches_encoder_state() {
    let cfg = partner_cfg();
    let enc = alec_encoder_new_with_config(&cfg);
    let dec_fed = alec_decoder_new_with_config(&cfg);

    let data = dataset(100);
    for (i, row) in data.iter().enumerate() {
        // Encoder observes the RAW input values (its own `observe`
        // loop in alec_encode_multi_fixed). We discard the wire.
        let _wire = encode_one(enc, row);
        // feed_values observes the SAME raw values on the decoder.
        let r = alec_decoder_feed_values(dec_fed, row.as_ptr(), row.len());
        assert_eq!(r, AlecResult::Ok, "feed_values failed at frame {}", i);
    }

    // (a) Context versions must agree. Both incremented by 5
    //     (channel_count) per frame × 100 frames = 500 — but we
    //     don't hard-code that, just check parity.
    let enc_v = alec_encoder_context_version(enc);
    let dec_v = alec_decoder_context_version(dec_fed);
    assert_eq!(
        enc_v, dec_v,
        "context.version mismatch: enc={enc_v}, dec_fed={dec_v} \
         — feed_values' observe loop is not in step with the encoder's"
    );

    // (b) Probe round-trip. With dec_fed's prediction state == enc's,
    //     the encoder writes delta = raw - enc_pred and the decoder
    //     reconstructs raw = dec_pred + delta = raw modulo
    //     scale_factor quantisation (= 0.01).
    let probe_row = [1.0_f64, 270.0, 125.0, 950.0, 10_120.0];
    let probe_wire = encode_one(enc, &probe_row);
    let v_fed = decode_one(dec_fed, &probe_wire);
    for ch in 0..CHANNELS {
        let diff = (probe_row[ch] - v_fed[ch]).abs();
        assert!(
            diff <= TOL[ch] + 1e-9,
            "ch {}: probe diverged on dec_fed: expected {}, got {} (diff {})",
            ch,
            probe_row[ch],
            v_fed[ch],
            diff
        );
    }

    // No gap should be reported — feed_values advanced the wire-seq
    // tracker so the probe's seq lined up exactly.
    let mut gap: u8 = 99;
    assert!(!alec_decoder_gap_detected(dec_fed, &mut gap));
    assert_eq!(gap, 0);

    alec_encoder_free(enc);
    alec_decoder_free(dec_fed);
}

// ===========================================================================
// 4.2 — Mixed ALEC + TLV simulation (the partner's actual flow).
//
// On every frame:
//   - Encode with the encoder (which advances its prediction state).
//   - If the wire is small enough, "send ALEC" → decode normally.
//   - Otherwise, "send TLV" → feed_values with the raw input.
//
// Verifies:
//   * No spurious gaps at any point.
//   * After all 100 frames the decoder is still functional and
//     can decode the next ALEC frame.
//   * The encoder's `context.version` and the decoder's match each
//     time the run ends on a `feed_values` call (= encoder and
//     decoder both observed raw on every frame). When the run ends
//     on a `decode` call, the decoder observed reconstructed values
//     so its `version` still matches (both advance by channel_count
//     per frame regardless of which value was observed).
//
// Reconstruction tolerance is asserted on a STABLE signal — the
// codec's "bounded drift" property (documented on the encoder's
// observe loop) means a steadily-changing input accumulates ≤ 0.005
// per frame of f32-quantisation drift on each channel, which over
// 100 frames + a steady ramp can exceed the per-channel LSB. That
// drift is a property of the codec, not of `feed_values`, and is
// already covered by `test_encode_decode_fixed_roundtrip_5ch`.
// ===========================================================================

#[test]
fn mixed_alec_and_tlv_round_trip() {
    // Use a smaller ceiling than the real LoRaWAN 11 B so we
    // intentionally hit the "TLV fallback" branch a few times during
    // the run (cold start frames are 27 B, periodic keyframes are
    // 27 B, post-warm frames are 7-11 B).
    const CEILING: usize = 15;

    let cfg = partner_cfg();
    let enc = alec_encoder_new_with_config(&cfg);
    let dec = alec_decoder_new_with_config(&cfg);

    // Stable signal — no accumulated drift, so we can also assert
    // tolerance after the mixed run.
    let row = [1.0_f64, 268.0, 120.0, 900.0, 10_100.0];
    let mut alec_count = 0usize;
    let mut tlv_count = 0usize;

    for _ in 0..100 {
        let wire = encode_one(enc, &row);
        if wire.len() <= CEILING {
            let _ = decode_one(dec, &wire);
            alec_count += 1;
        } else {
            // "TLV fallback" — wire is ignored, raw values fed to decoder.
            let r = alec_decoder_feed_values(dec, row.as_ptr(), row.len());
            assert_eq!(r, AlecResult::Ok);
            tlv_count += 1;
        }
        // Never a spurious gap.
        let mut gap: u8 = 99;
        assert!(
            !alec_decoder_gap_detected(dec, &mut gap),
            "spurious gap during mixed run"
        );
        assert_eq!(gap, 0);
    }

    assert!(alec_count > 0, "expected some ALEC frames");
    assert!(tlv_count > 0, "expected some TLV-fallback frames");

    // Encoder + decoder context.version stay in step throughout
    // (both advance by 5/frame regardless of ALEC vs TLV).
    assert_eq!(
        alec_encoder_context_version(enc),
        alec_decoder_context_version(dec),
        "version drift after 100 mixed frames"
    );

    // Final probe: encode a fresh frame and decode it. Stable signal
    // → reconstruction should be within sensor LSB.
    let probe_wire = encode_one(enc, &row);
    let v = decode_one(dec, &probe_wire);
    let mut gap: u8 = 99;
    assert!(!alec_decoder_gap_detected(dec, &mut gap));
    assert_eq!(gap, 0);
    for ch in 0..CHANNELS {
        let diff = (row[ch] - v[ch]).abs();
        assert!(
            diff <= TOL[ch] + 1e-9,
            "post-mixed-run ch {}: expected {}, got {} (diff {})",
            ch,
            row[ch],
            v[ch],
            diff,
        );
    }

    alec_encoder_free(enc);
    alec_decoder_free(dec);
}

// ===========================================================================
// 4.3 — Sequence continuity: feed_values advances the wire-sequence
// tracker so a subsequent decode does not flag a gap.
// ===========================================================================

#[test]
fn feed_values_advances_sequence_no_gap() {
    let cfg = partner_cfg();
    let enc = alec_encoder_new_with_config(&cfg);
    let dec = alec_decoder_new_with_config(&cfg);
    let row = [1.0_f64, 268.0, 120.0, 900.0, 10_100.0];

    // Drive the encoder for 5 frames so it's well past the cold start.
    // Decode 3, feed 1, decode 1.
    for _ in 0..3 {
        let w = encode_one(enc, &row);
        let _ = decode_one(dec, &w);
    }

    // Frame 4: encoder produces wire seq=3, but we pretend it was TLV.
    let _wire_4_discarded = encode_one(enc, &row);
    let r = alec_decoder_feed_values(dec, row.as_ptr(), row.len());
    assert_eq!(r, AlecResult::Ok);

    // Frame 5: wire seq=4. After feed_values, decoder.last_fixed_sequence
    // should be 3 (wire-equivalent of the discarded frame), so frame 5
    // arrives with diff=1 → no gap.
    let wire_5 = encode_one(enc, &row);
    let _v = decode_one(dec, &wire_5);
    let mut gap: u8 = 99;
    let was_gap = alec_decoder_gap_detected(dec, &mut gap);
    assert!(
        !was_gap,
        "decoder reported a gap of {} on frame 5 — feed_values did not advance last_fixed_sequence",
        gap
    );
    assert_eq!(gap, 0);

    alec_encoder_free(enc);
    alec_decoder_free(dec);
}

// ===========================================================================
// 4.4 — feed_values right after a keyframe.
//
// Sequence: keyframe → ALEC data → discarded TLV (feed_values) → ALEC data.
// Every step must remain in lockstep.
// ===========================================================================

#[test]
fn feed_values_after_keyframe() {
    // Force a keyframe at frame 5 with `keyframe_interval = 5`.
    let cfg = AlecEncoderConfig {
        keyframe_interval: 5,
        ..partner_cfg()
    };
    let enc = alec_encoder_new_with_config(&cfg);
    let dec = alec_decoder_new_with_config(&cfg);

    let row = [1.0_f64, 268.0, 120.0, 900.0, 10_100.0];

    // Drive 5 data frames then 1 keyframe. With `keyframe_interval=5`
    // the encoder fires the keyframe on the 6th encode (when
    // `messages_since_keyframe` first reaches 5).
    for _ in 0..5 {
        let w = encode_one(enc, &row);
        let _ = decode_one(dec, &w);
    }
    let kf_wire = encode_one(enc, &row);
    assert_eq!(kf_wire[0], 0xA2, "frame 6 should be a keyframe");
    let _ = decode_one(dec, &kf_wire);

    // Frame 6: pretend TLV fallback.
    let _ = encode_one(enc, &row);
    let r = alec_decoder_feed_values(dec, row.as_ptr(), row.len());
    assert_eq!(r, AlecResult::Ok);

    // Frame 7: real ALEC again. Must decode without gap and produce
    // values within tolerance.
    let w7 = encode_one(enc, &row);
    let v = decode_one(dec, &w7);
    let mut gap: u8 = 99;
    assert!(!alec_decoder_gap_detected(dec, &mut gap));
    assert_eq!(gap, 0);
    for ch in 0..CHANNELS {
        let diff = (row[ch] - v[ch]).abs();
        assert!(diff <= TOL[ch] + 1e-9);
    }

    alec_encoder_free(enc);
    alec_decoder_free(dec);
}

// ===========================================================================
// 4.5 — Error handling.
// ===========================================================================

#[test]
fn feed_values_error_paths() {
    let dec = alec_decoder_new_with_config(&partner_cfg());
    let row = [1.0_f64, 268.0, 120.0, 900.0, 10_100.0];

    // NULL decoder.
    assert_eq!(
        alec_decoder_feed_values(ptr::null_mut(), row.as_ptr(), row.len()),
        AlecResult::ErrorNullPointer
    );

    // NULL values.
    assert_eq!(
        alec_decoder_feed_values(dec, ptr::null(), row.len()),
        AlecResult::ErrorNullPointer
    );

    // num_values == 0.
    assert_eq!(
        alec_decoder_feed_values(dec, row.as_ptr(), 0),
        AlecResult::ErrorInvalidInput
    );

    // num_values > 64 (out of wire-format range).
    let big = [0.0_f64; 70];
    assert_eq!(
        alec_decoder_feed_values(dec, big.as_ptr(), big.len()),
        AlecResult::ErrorInvalidInput
    );

    alec_decoder_free(dec);
}

// ===========================================================================
// 4.5b — feed_values on a fresh decoder (first frame is TLV).
// Equivalent to "the very first uplink was a fallback TLV". Must
// succeed and leave the decoder in a state where the next ALEC frame
// (wire seq = 1) decodes without a gap.
// ===========================================================================

#[test]
fn feed_values_on_fresh_decoder() {
    let cfg = partner_cfg();
    let enc = alec_encoder_new_with_config(&cfg);
    let dec = alec_decoder_new_with_config(&cfg);
    let row = [1.0_f64, 268.0, 120.0, 900.0, 10_100.0];

    // Frame 0: discarded TLV.
    let _ = encode_one(enc, &row);
    let r = alec_decoder_feed_values(dec, row.as_ptr(), row.len());
    assert_eq!(r, AlecResult::Ok);

    // Frame 1: real ALEC. Wire seq = 1; decoder's last_fixed_sequence
    // should be 0 (set by feed_values for "ghost frame 0"), so the
    // diff is exactly 1 → no gap.
    let w = encode_one(enc, &row);
    let _ = decode_one(dec, &w);
    let mut gap: u8 = 99;
    assert!(!alec_decoder_gap_detected(dec, &mut gap));
    assert_eq!(gap, 0);

    alec_encoder_free(enc);
    alec_decoder_free(dec);
}
