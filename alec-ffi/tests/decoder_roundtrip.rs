// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! v1.3.6 decoder FFI integration tests.
//!
//! Exercises the new decoder FFI surface end-to-end:
//! * `alec_decoder_new_with_config`
//! * `alec_decoder_reset`
//! * `alec_decode_multi_fixed` with the v1.3.6 outputs
//!   (`num_channels_out`, `sequence_out`, `is_keyframe_out`)
//! * `alec_decoder_context_save` / `alec_decoder_context_load`
//!
//! The round-trip drives 5 channels (battery, temperature, humidity,
//! CO2, pressure) with realistic slow drift, then verifies decoded
//! values match the input within native sensor LSB tolerance.

#![cfg(feature = "decoder")]

use std::ptr;

use alec_ffi::{
    alec_decode_multi_fixed, alec_decoder_context_load, alec_decoder_context_save,
    alec_decoder_free, alec_decoder_new_with_config, alec_decoder_reset, alec_encode_multi_fixed,
    alec_encoder_free, alec_encoder_new_with_config, AlecDecoder, AlecEncoder, AlecEncoderConfig,
    AlecResult,
};

const CHANNELS: usize = 5;
/// Native quantization step per channel (battery V, temp °C, humidity %,
/// CO2 ppm, pressure hPa). Used as the round-trip tolerance — the
/// codec is effectively lossless at this granularity.
const TOLERANCE: [f64; CHANNELS] = [0.01, 0.01, 0.1, 1.0, 0.01];

/// 50-row synthetic EM500-CO2-style slow-drift dataset.
///
/// Battery decays slowly (10mV every ~16 frames); temperature, humidity
/// and pressure walk small amounts; CO2 ramps. Matches the steady-state
/// regime documented in docs/CONTEXT.md.
fn synthetic_dataset() -> Vec<[f64; CHANNELS]> {
    (0..50)
        .map(|i| {
            let battery = 3.60 - 0.01 * ((i / 16) as f64);
            let temperature = 22.50 + if i % 8 < 4 { 0.01 } else { 0.0 };
            let humidity = 45.0 + 0.1 * ((i / 7) % 3) as f64;
            let co2 = 420.0 + (i / 3) as f64;
            let pressure = 1013.25 + 0.01 * ((i / 11) % 3) as f64;
            [battery, temperature, humidity, co2, pressure]
        })
        .collect()
}

/// Build an encoder + decoder pair with matched config.
fn matched_pair(keyframe_interval: u32) -> (*mut AlecEncoder, *mut AlecDecoder) {
    let cfg = AlecEncoderConfig {
        history_size: 0,
        max_patterns: 0,
        max_memory_bytes: 0,
        keyframe_interval,
        smart_resync: true,
        num_channels: 0,
    };
    let enc = alec_encoder_new_with_config(&cfg);
    let dec = alec_decoder_new_with_config(&cfg);
    assert!(!enc.is_null() && !dec.is_null());
    (enc, dec)
}

/// Encode + decode one frame, capturing the new frame-level outputs.
struct DecodedFrame {
    values: [f64; CHANNELS],
    sequence: u16,
    keyframe: bool,
    num_channels: usize,
    wire: Vec<u8>,
}

fn encode_decode_one(
    enc: *mut AlecEncoder,
    dec: *mut AlecDecoder,
    row: &[f64; CHANNELS],
) -> DecodedFrame {
    let mut wire = [0u8; 64];
    let mut wire_len = 0usize;
    let r = alec_encode_multi_fixed(
        enc,
        row.as_ptr(),
        row.len(),
        wire.as_mut_ptr(),
        wire.len(),
        &mut wire_len,
    );
    assert_eq!(r, AlecResult::Ok, "encode failed");

    let mut values = [0f64; CHANNELS];
    let mut num: usize = 0;
    let mut seq: u16 = 0;
    let mut keyframe = false;
    let r = alec_decode_multi_fixed(
        dec,
        wire.as_ptr(),
        wire_len,
        values.as_mut_ptr(),
        values.len(),
        &mut num,
        &mut seq,
        &mut keyframe,
    );
    assert_eq!(r, AlecResult::Ok, "decode failed");

    DecodedFrame {
        values,
        sequence: seq,
        keyframe,
        num_channels: num,
        wire: wire[..wire_len].to_vec(),
    }
}

fn assert_within_tolerance(expected: &[f64; CHANNELS], actual: &[f64; CHANNELS], frame: usize) {
    for ch in 0..CHANNELS {
        let diff = (expected[ch] - actual[ch]).abs();
        assert!(
            diff <= TOLERANCE[ch] + 1e-9,
            "frame {} ch {}: expected {}, got {} (diff {} > tol {})",
            frame,
            ch,
            expected[ch],
            actual[ch],
            diff,
            TOLERANCE[ch]
        );
    }
}

/// 50-frame round trip with periodic keyframes at interval 10.
/// Every decoded frame must match the input within sensor LSB.
#[test]
fn roundtrip_50_frames_5_channels() {
    let (enc, dec) = matched_pair(10);
    let data = synthetic_dataset();

    let mut keyframes_seen = 0usize;
    for (i, row) in data.iter().enumerate() {
        let frame = encode_decode_one(enc, dec, row);
        assert_within_tolerance(row, &frame.values, i);

        // num_channels_out always equals the frame's channel count.
        assert_eq!(frame.num_channels, CHANNELS);

        // Wire-level sanity: marker matches the keyframe flag.
        let marker = frame.wire[0];
        if frame.keyframe {
            assert_eq!(marker, 0xA2, "keyframe marker mismatch at frame {}", i);
            keyframes_seen += 1;
        } else {
            assert_eq!(marker, 0xA1, "data marker mismatch at frame {}", i);
        }

        // Sequence number from the decoder matches the wire sequence.
        let wire_seq = u16::from_be_bytes([frame.wire[1], frame.wire[2]]);
        assert_eq!(frame.sequence, wire_seq);
    }

    // Frame 0 (cold start: all Raw32) and the periodic keyframes at
    // wire-sequence multiples of 10 (frames at indices 10, 20, 30, 40)
    // must be marked as keyframes. Frame 0 is encoded as a data frame
    // (0xA1) but is materially Raw32 — only the marker counts here.
    assert!(
        keyframes_seen >= 4,
        "expected at least 4 keyframes (one every 10 frames after cold start), saw {}",
        keyframes_seen
    );

    alec_encoder_free(enc);
    alec_decoder_free(dec);
}

/// First frame must be flagged as the data frame's "cold start" Raw32
/// (marker 0xA1, no keyframe flag), and the periodic keyframes that
/// follow must be flagged as keyframes. Checks the keyframe interval
/// works deterministically with the new is_keyframe_out output.
#[test]
fn keyframe_flag_at_periodic_interval() {
    let (enc, dec) = matched_pair(10);
    let row = [3.6, 22.5, 45.0, 420.0, 1013.25];

    let mut keyframe_indices = Vec::new();
    for i in 0..25 {
        let frame = encode_decode_one(enc, dec, &row);
        if frame.keyframe {
            keyframe_indices.push(i);
        }
    }

    // With keyframe_interval=10 and a stable signal, the encoder
    // emits keyframes at indices 10 and 20 (counter-based).
    assert_eq!(keyframe_indices, vec![10, 20]);

    alec_encoder_free(enc);
    alec_decoder_free(dec);
}

/// Sequence numbers should be monotonically increasing across the run.
#[test]
fn sequence_numbers_monotonic() {
    let (enc, dec) = matched_pair(50);
    let row = [3.6, 22.5, 45.0, 420.0, 1013.25];

    let mut prev: Option<u16> = None;
    for _ in 0..30 {
        let frame = encode_decode_one(enc, dec, &row);
        if let Some(p) = prev {
            assert_eq!(
                frame.sequence,
                p.wrapping_add(1),
                "sequence numbers must be contiguous"
            );
        }
        prev = Some(frame.sequence);
    }

    alec_encoder_free(enc);
    alec_decoder_free(dec);
}

/// Drop a frame mid-sequence and verify the decoder recovers
/// gracefully (no panic, decode returns Ok or a recoverable error,
/// next keyframe re-syncs).
#[test]
fn gap_recovery_at_next_keyframe() {
    let (enc, dec) = matched_pair(10);
    let row = [3.60, 22.50, 45.0, 420.0, 1013.25];

    // Encode 21 frames; capture all wire bytes.
    let mut wires = Vec::new();
    let mut buf = [0u8; 64];
    for _ in 0..=20 {
        let mut n = 0usize;
        assert_eq!(
            alec_encode_multi_fixed(
                enc,
                row.as_ptr(),
                row.len(),
                buf.as_mut_ptr(),
                buf.len(),
                &mut n
            ),
            AlecResult::Ok
        );
        wires.push(buf[..n].to_vec());
    }
    assert_eq!(wires[10][0], 0xA2, "frame 10 must be a keyframe");
    assert_eq!(wires[20][0], 0xA2, "frame 20 must be a keyframe");

    // Decode frames 0..=14 normally, skip 15..=18 (4-frame gap),
    // then resume at frame 19 + keyframe at frame 20.
    let mut values = [0f64; CHANNELS];
    let mut num = 0usize;
    let mut seq = 0u16;
    let mut keyframe = false;
    for w in &wires[..=14] {
        let r = alec_decode_multi_fixed(
            dec,
            w.as_ptr(),
            w.len(),
            values.as_mut_ptr(),
            values.len(),
            &mut num,
            &mut seq,
            &mut keyframe,
        );
        assert_eq!(r, AlecResult::Ok);
    }

    // Frame 19: gap of 4 → decode returns Ok but the decoder
    // internally calls reset_to_baseline().
    let r19 = alec_decode_multi_fixed(
        dec,
        wires[19].as_ptr(),
        wires[19].len(),
        values.as_mut_ptr(),
        values.len(),
        &mut num,
        &mut seq,
        &mut keyframe,
    );
    assert_eq!(r19, AlecResult::Ok);
    assert!(!keyframe, "frame 19 is a data frame");
    assert_eq!(seq, 19);

    // Frame 20 is a real keyframe — decode must succeed AND values
    // must match the input within sensor LSB.
    let r20 = alec_decode_multi_fixed(
        dec,
        wires[20].as_ptr(),
        wires[20].len(),
        values.as_mut_ptr(),
        values.len(),
        &mut num,
        &mut seq,
        &mut keyframe,
    );
    assert_eq!(r20, AlecResult::Ok);
    assert!(keyframe, "frame 20 must be flagged as keyframe");
    assert_eq!(seq, 20);
    assert_within_tolerance(&row, &values, 20);

    alec_encoder_free(enc);
    alec_decoder_free(dec);
}

/// Save the decoder context mid-sequence, build a fresh decoder, load
/// the state, and verify subsequent frames decode to the same values
/// as the original decoder. This is the sidecar restart story.
#[test]
fn context_save_load_continues_decoding() {
    let (enc, dec_orig) = matched_pair(10_000); // no periodic keyframes
    let data = synthetic_dataset();

    // Train both encoder and decoder on the first half of the data.
    let split = 25;
    for row in &data[..split] {
        let _ = encode_decode_one(enc, dec_orig, row);
    }

    // Probe the required size with a single-byte stub buffer. The
    // FFI honours the no-partial-write contract: on
    // ErrorBufferTooSmall, `*written` reports the required size and
    // the input buffer is NOT modified.
    let mut probe = [0u8; 1];
    let mut required = 0usize;
    let r = alec_decoder_context_save(dec_orig, probe.as_mut_ptr(), probe.len(), &mut required);
    assert_eq!(r, AlecResult::ErrorBufferTooSmall);
    assert!(required > 1, "expected non-trivial required size");
    assert_eq!(probe[0], 0, "no partial write on BufferTooSmall");

    let mut buf = vec![0u8; required];
    let mut written = 0usize;
    assert_eq!(
        alec_decoder_context_save(dec_orig, buf.as_mut_ptr(), buf.len(), &mut written),
        AlecResult::Ok
    );
    assert_eq!(written, required);

    // Restore into a fresh decoder.
    let dec_new = alec_decoder_new_with_config(ptr::null());
    assert!(!dec_new.is_null());
    assert_eq!(
        alec_decoder_context_load(dec_new, buf.as_ptr(), buf.len()),
        AlecResult::Ok
    );

    // Decode the second half of the data through BOTH decoders and
    // compare bit-exactly. This proves the restored context is
    // operationally equivalent.
    let mut wire = [0u8; 64];
    let mut wire_len = 0usize;
    for (offset, row) in data[split..].iter().enumerate() {
        let i = split + offset;

        assert_eq!(
            alec_encode_multi_fixed(
                enc,
                row.as_ptr(),
                row.len(),
                wire.as_mut_ptr(),
                wire.len(),
                &mut wire_len
            ),
            AlecResult::Ok
        );

        let mut v_orig = [0f64; CHANNELS];
        let mut v_new = [0f64; CHANNELS];
        let mut num = 0usize;
        let mut seq = 0u16;
        let mut keyframe = false;
        assert_eq!(
            alec_decode_multi_fixed(
                dec_orig,
                wire.as_ptr(),
                wire_len,
                v_orig.as_mut_ptr(),
                v_orig.len(),
                &mut num,
                &mut seq,
                &mut keyframe,
            ),
            AlecResult::Ok
        );
        assert_eq!(
            alec_decode_multi_fixed(
                dec_new,
                wire.as_ptr(),
                wire_len,
                v_new.as_mut_ptr(),
                v_new.len(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            ),
            AlecResult::Ok
        );

        for ch in 0..CHANNELS {
            assert_eq!(
                v_orig[ch].to_bits(),
                v_new[ch].to_bits(),
                "frame {} ch {}: restored decoder diverged from original",
                i,
                ch
            );
        }
        // Both decoders also match the input within sensor tolerance.
        assert_within_tolerance(row, &v_orig, i);
    }

    alec_encoder_free(enc);
    alec_decoder_free(dec_orig);
    alec_decoder_free(dec_new);
}

/// `alec_decoder_reset` clears all per-channel prediction state.
/// After a reset, the next decoded frame must be a keyframe to
/// reseed the prediction state — any non-keyframe with delta
/// encoding would error out for lack of a reference value.
#[test]
fn reset_clears_state() {
    let (enc, dec) = matched_pair(10_000); // no periodic keyframes
    let row = [3.6, 22.5, 45.0, 420.0, 1013.25];

    // Warm up.
    for _ in 0..15 {
        let _ = encode_decode_one(enc, dec, &row);
    }

    // Reset.
    alec_decoder_reset(dec);

    // After reset, any subsequent encoded delta-frame from the
    // encoder would fail to decode (no prediction state). The
    // simplest way to recover is to force a keyframe on the encoder
    // side — but we can also re-synchronise both sides with a fresh
    // encoder/decoder pair seeded with a Raw32 frame. For this test,
    // we directly verify the reset-then-decode-keyframe path works:
    // create a fresh encoder, then encode (which produces a Raw32
    // cold-start frame), then decode through the reset decoder.
    let cfg = AlecEncoderConfig {
        history_size: 0,
        max_patterns: 0,
        max_memory_bytes: 0,
        keyframe_interval: 10_000,
        smart_resync: false,
        num_channels: 0,
    };
    let fresh_enc = alec_encoder_new_with_config(&cfg);

    let mut wire = [0u8; 64];
    let mut wire_len = 0usize;
    assert_eq!(
        alec_encode_multi_fixed(
            fresh_enc,
            row.as_ptr(),
            row.len(),
            wire.as_mut_ptr(),
            wire.len(),
            &mut wire_len
        ),
        AlecResult::Ok
    );

    let mut values = [0f64; CHANNELS];
    let mut num = 0usize;
    let mut seq = 0u16;
    let mut keyframe = false;
    assert_eq!(
        alec_decode_multi_fixed(
            dec,
            wire.as_ptr(),
            wire_len,
            values.as_mut_ptr(),
            values.len(),
            &mut num,
            &mut seq,
            &mut keyframe,
        ),
        AlecResult::Ok
    );
    // Decoded values from a Raw32-only frame should match within f32
    // precision (≈1e-7 relative).
    assert_within_tolerance(&row, &values, 0);

    alec_encoder_free(enc);
    alec_encoder_free(fresh_enc);
    alec_decoder_free(dec);
}

/// NULL-safety on the v1.3.6 entry points.
#[test]
fn null_safety_on_new_apis() {
    // alec_decoder_reset(NULL) is a documented no-op.
    alec_decoder_reset(ptr::null_mut());

    // alec_decoder_new_with_config(NULL) uses defaults.
    let dec = alec_decoder_new_with_config(ptr::null());
    assert!(!dec.is_null());

    let mut buf = [0u8; 4];
    let mut n = 0usize;

    assert_eq!(
        alec_decoder_context_save(ptr::null(), buf.as_mut_ptr(), buf.len(), &mut n),
        AlecResult::ErrorNullPointer
    );
    assert_eq!(
        alec_decoder_context_save(dec, ptr::null_mut(), buf.len(), &mut n),
        AlecResult::ErrorNullPointer
    );
    assert_eq!(
        alec_decoder_context_save(dec, buf.as_mut_ptr(), buf.len(), ptr::null_mut()),
        AlecResult::ErrorNullPointer
    );

    assert_eq!(
        alec_decoder_context_load(ptr::null_mut(), buf.as_ptr(), buf.len()),
        AlecResult::ErrorNullPointer
    );
    assert_eq!(
        alec_decoder_context_load(dec, ptr::null(), buf.len()),
        AlecResult::ErrorNullPointer
    );

    // Decode null-safety on the new outputs (output ptrs are optional;
    // input/decoder/values are not).
    let values = [0f64; CHANNELS];
    let frame = [0u8; 16];
    assert_eq!(
        alec_decode_multi_fixed(
            ptr::null_mut(),
            frame.as_ptr(),
            frame.len(),
            values.as_ptr() as *mut f64,
            values.len(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        ),
        AlecResult::ErrorNullPointer
    );
    assert_eq!(
        alec_decode_multi_fixed(
            dec,
            ptr::null(),
            0,
            values.as_ptr() as *mut f64,
            values.len(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        ),
        AlecResult::ErrorNullPointer
    );
    assert_eq!(
        alec_decode_multi_fixed(
            dec,
            frame.as_ptr(),
            frame.len(),
            ptr::null_mut(),
            values.len(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        ),
        AlecResult::ErrorNullPointer
    );

    alec_decoder_free(dec);
}
