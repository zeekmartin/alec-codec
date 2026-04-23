// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! v1.3.7 encoder context save/load integration tests.
//!
//! Covers the new buffer-based encoder-state APIs
//! (`alec_encoder_context_save` / `alec_encoder_context_load`) that
//! unblock the firmware fix for oversize-frame discard on MCUs
//! without a filesystem.
//!
//! Test matrix:
//! 1. Save-then-load produces an encoder that continues emitting the
//!    exact same wire bytes as a reference encoder that ran without
//!    interruption.
//! 2. Full encoder + decoder round-trip across a save/load boundary
//!    on both sides.
//! 3. Discard-and-restore: simulate "frame too big" by rolling back
//!    the encoder, forcing a keyframe, and checking the decoder
//!    catches up without visible corruption.
//! 4. Buffer too small returns `ALEC_ERROR_BUFFER_TOO_SMALL` (and
//!    does not partial-write).
//! 5. NULL-pointer safety on both entry points.

#![cfg(feature = "decoder")]

use std::ptr;

use alec_ffi::{
    alec_decode_multi_fixed, alec_decoder_free, alec_decoder_new_with_config,
    alec_encode_multi_fixed, alec_encoder_context_load, alec_encoder_context_save,
    alec_encoder_free, alec_encoder_new_with_config, alec_force_keyframe, AlecEncoder,
    AlecEncoderConfig, AlecResult,
};

const CHANNELS: usize = 5;
/// Native quantization step per channel (V, °C, %, ppm, hPa).
const TOLERANCE: [f64; CHANNELS] = [0.01, 0.01, 0.1, 1.0, 0.01];

fn cfg(keyframe_interval: u32) -> AlecEncoderConfig {
    AlecEncoderConfig {
        history_size: 0,
        max_patterns: 0,
        max_memory_bytes: 0,
        keyframe_interval,
        smart_resync: true,
    }
}

/// Encode one row on `enc` and return the wire bytes.
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
    assert_eq!(r, AlecResult::Ok);
    out[..n].to_vec()
}

fn assert_within_tolerance(expected: &[f64; CHANNELS], actual: &[f64; CHANNELS], label: &str) {
    for ch in 0..CHANNELS {
        let diff = (expected[ch] - actual[ch]).abs();
        assert!(
            diff <= TOLERANCE[ch] + 1e-9,
            "{} ch {}: expected {}, got {} (diff {} > tol {})",
            label,
            ch,
            expected[ch],
            actual[ch],
            diff,
            TOLERANCE[ch]
        );
    }
}

/// Slow-drift reference dataset.
fn dataset(n: usize) -> Vec<[f64; CHANNELS]> {
    (0..n)
        .map(|i| {
            [
                3.60 - 0.01 * ((i / 20) as f64),
                22.50 + if i % 8 < 4 { 0.01 } else { 0.0 },
                45.0 + 0.1 * ((i / 7) % 3) as f64,
                420.0 + (i / 3) as f64,
                1013.25 + 0.01 * ((i / 11) % 3) as f64,
            ]
        })
        .collect()
}

/// Helper: save encoder state into a freshly-sized buffer. Returns
/// the bytes exactly.
fn save_enc(enc: *const AlecEncoder) -> Vec<u8> {
    // Probe for required size with a 1-byte stub.
    let mut probe = [0u8; 1];
    let mut need = 0usize;
    let r = alec_encoder_context_save(enc, probe.as_mut_ptr(), probe.len(), &mut need);
    assert_eq!(r, AlecResult::ErrorBufferTooSmall);
    assert!(need > 1);
    assert_eq!(probe[0], 0, "no partial write on BufferTooSmall");

    let mut buf = vec![0u8; need];
    let mut written = 0usize;
    assert_eq!(
        alec_encoder_context_save(enc, buf.as_mut_ptr(), buf.len(), &mut written),
        AlecResult::Ok
    );
    assert_eq!(written, need);
    buf
}

// ==========================================================================
// Task 3.1 — Save/load continuity: replaying an encoder from a snapshot
// produces bit-identical wire bytes to an encoder that ran uninterrupted.
// ==========================================================================

#[test]
fn save_load_replays_identical_wire_bytes() {
    let rows = dataset(30);

    // Reference: one encoder runs the full sequence.
    let cfg0 = cfg(10);
    let ref_enc = alec_encoder_new_with_config(&cfg0);
    let ref_frames: Vec<Vec<u8>> = rows.iter().map(|r| encode_one(ref_enc, r)).collect();
    alec_encoder_free(ref_enc);

    // Split: encode the first 10 frames on `enc_a`, snapshot it,
    // restore into `enc_b`, then keep encoding the remaining 20 frames.
    let enc_a = alec_encoder_new_with_config(&cfg0);
    for (i, row) in rows.iter().take(10).enumerate() {
        let wire = encode_one(enc_a, row);
        assert_eq!(wire, ref_frames[i], "pre-snapshot frame {} diverged", i);
    }

    let snap = save_enc(enc_a);
    alec_encoder_free(enc_a);

    let enc_b = alec_encoder_new_with_config(&cfg0);
    assert_eq!(
        alec_encoder_context_load(enc_b, snap.as_ptr(), snap.len()),
        AlecResult::Ok
    );

    // Continue encoding. Frames must match the reference bit-for-bit.
    for i in 10..30 {
        let wire = encode_one(enc_b, &rows[i]);
        assert_eq!(
            wire, ref_frames[i],
            "frame {}: replay diverged from reference",
            i
        );
    }

    alec_encoder_free(enc_b);
}

// ==========================================================================
// Task 3.2 — Full encoder+decoder round-trip across save/load on both sides.
// ==========================================================================

#[test]
fn encoder_and_decoder_survive_joint_save_load() {
    let rows = dataset(25);
    let cfg0 = cfg(10);

    // Drive 10 frames through a matched encoder+decoder pair.
    let enc = alec_encoder_new_with_config(&cfg0);
    let dec = alec_decoder_new_with_config(&cfg0);

    for row in &rows[..10] {
        let wire = encode_one(enc, row);
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
        assert_eq!(r, AlecResult::Ok);
    }

    // Snapshot both sides.
    let enc_snap = save_enc(enc);
    // Decoder context save (from v1.3.6) — tiny shape so a fixed cap works.
    let mut dec_snap = vec![0u8; 4096];
    let mut dec_len = 0usize;
    assert_eq!(
        alec_ffi::alec_decoder_context_save(
            dec,
            dec_snap.as_mut_ptr(),
            dec_snap.len(),
            &mut dec_len
        ),
        AlecResult::Ok
    );
    dec_snap.truncate(dec_len);

    alec_encoder_free(enc);
    alec_decoder_free(dec);

    // Fresh pair, restore both contexts, keep encoding+decoding.
    let enc2 = alec_encoder_new_with_config(&cfg0);
    let dec2 = alec_decoder_new_with_config(&cfg0);
    assert_eq!(
        alec_encoder_context_load(enc2, enc_snap.as_ptr(), enc_snap.len()),
        AlecResult::Ok
    );
    assert_eq!(
        alec_ffi::alec_decoder_context_load(dec2, dec_snap.as_ptr(), dec_snap.len()),
        AlecResult::Ok
    );

    for (i, row) in rows[10..].iter().enumerate() {
        let wire = encode_one(enc2, row);
        let mut v = [0f64; CHANNELS];
        let mut num = 0usize;
        assert_eq!(
            alec_decode_multi_fixed(
                dec2,
                wire.as_ptr(),
                wire.len(),
                v.as_mut_ptr(),
                v.len(),
                &mut num,
                ptr::null_mut(),
                ptr::null_mut(),
            ),
            AlecResult::Ok
        );
        assert_within_tolerance(row, &v, &format!("post-restore frame {}", 10 + i));
    }

    alec_encoder_free(enc2);
    alec_decoder_free(dec2);
}

// ==========================================================================
// Task 3.3 — The headline discard-and-restore pattern used by the firmware:
//   encode frame → exceeds ceiling → restore context → force keyframe →
//   encode keyframe → verify decoder can decode all subsequent frames.
// ==========================================================================

#[test]
fn discard_and_restore_pattern() {
    let cfg0 = cfg(10_000); // no periodic keyframes interfering
    let enc = alec_encoder_new_with_config(&cfg0);
    let dec = alec_decoder_new_with_config(&cfg0);

    // Warm up on a stable signal so the encoder's prediction is settled.
    let stable = [3.60_f64, 22.50, 45.0, 420.0, 1013.25];
    for _ in 0..8 {
        let wire = encode_one(enc, &stable);
        let mut v = [0f64; CHANNELS];
        let mut num = 0usize;
        assert_eq!(
            alec_decode_multi_fixed(
                dec,
                wire.as_ptr(),
                wire.len(),
                v.as_mut_ptr(),
                v.len(),
                &mut num,
                ptr::null_mut(),
                ptr::null_mut(),
            ),
            AlecResult::Ok
        );
    }
    eprintln!("[discard-restore] warm-up done, decoder is in sync with encoder");

    // === The critical section ===
    //
    // Snapshot the encoder, then encode a frame whose wire output we
    // pretend "exceeds the LoRaWAN ceiling" and must be discarded.
    let snap = save_enc(enc);
    eprintln!(
        "[discard-restore] snapshotted encoder state: {} bytes (magic ALEE)",
        snap.len()
    );

    let wild = [5.99_f64, 999.0, 99.9, 9_000.0, 9_999.99];
    let discarded = encode_one(enc, &wild);
    eprintln!(
        "[discard-restore] encoded 'oversize' frame: {} bytes - DISCARDING (not sent to decoder)",
        discarded.len()
    );

    // Simulate discard — we just never send `discarded` to the decoder.
    // The encoder's internal prediction state has already been polluted
    // by the wild reading; we need to roll it back.
    assert_eq!(
        alec_encoder_context_load(enc, snap.as_ptr(), snap.len()),
        AlecResult::Ok
    );
    eprintln!("[discard-restore] restored encoder from snapshot");

    // Force the next frame to be a keyframe so the decoder resyncs
    // deterministically (also exercises the force_keyframe_pending
    // field we save/restore).
    alec_force_keyframe(enc);
    eprintln!("[discard-restore] armed force_keyframe for next uplink");

    // Now continue with the legitimate next reading (back on the stable
    // signal — the wild frame is pretended-never-sent).
    let next_row = stable;
    let wire = encode_one(enc, &next_row);
    assert_eq!(
        wire[0], 0xA2,
        "next frame after restore+force_keyframe must be a keyframe"
    );
    eprintln!(
        "[discard-restore] encoded keyframe: {} bytes, marker 0xA2",
        wire.len()
    );

    let mut v = [0f64; CHANNELS];
    let mut num = 0usize;
    let mut keyframe = false;
    assert_eq!(
        alec_decode_multi_fixed(
            dec,
            wire.as_ptr(),
            wire.len(),
            v.as_mut_ptr(),
            v.len(),
            &mut num,
            ptr::null_mut(),
            &mut keyframe,
        ),
        AlecResult::Ok
    );
    assert!(keyframe);
    assert_within_tolerance(&next_row, &v, "keyframe after discard");
    eprintln!("[discard-restore] decoder decoded keyframe, values match input");

    // And every frame after that must decode cleanly on the stable
    // signal, using compact Delta/Repeated encodings — the prediction
    // state is healthy on both sides again.
    for i in 0..10 {
        let wire = encode_one(enc, &stable);
        let mut v = [0f64; CHANNELS];
        let mut num = 0usize;
        assert_eq!(
            alec_decode_multi_fixed(
                dec,
                wire.as_ptr(),
                wire.len(),
                v.as_mut_ptr(),
                v.len(),
                &mut num,
                ptr::null_mut(),
                ptr::null_mut(),
            ),
            AlecResult::Ok,
            "frame {} after restore failed to decode",
            i
        );
        assert_within_tolerance(&stable, &v, &format!("post-restore steady frame {}", i));
    }
    eprintln!("[discard-restore] 10 steady-state frames decoded cleanly after restore - OK");

    alec_encoder_free(enc);
    alec_decoder_free(dec);
}

// ==========================================================================
// Task 3.4 — Buffer-too-small.
// ==========================================================================

#[test]
fn buffer_too_small_reports_required_size_and_preserves_buffer() {
    let cfg0 = cfg(10);
    let enc = alec_encoder_new_with_config(&cfg0);
    // Warm the context so there's actual state to serialize.
    for _ in 0..5 {
        let _ = encode_one(enc, &[3.6, 22.5, 45.0, 420.0, 1013.25]);
    }

    let sentinel = 0xABu8;
    let mut tiny = [sentinel; 10]; // smaller than header (24 B)
    let mut need = 0usize;
    let r = alec_encoder_context_save(enc, tiny.as_mut_ptr(), tiny.len(), &mut need);
    assert_eq!(r, AlecResult::ErrorBufferTooSmall);
    assert!(need > tiny.len(), "need must report the full required size");
    for &b in &tiny {
        assert_eq!(b, sentinel, "buffer must not be partially written on error");
    }

    // Retry with the reported capacity — must succeed.
    let mut buf = vec![0u8; need];
    let mut written = 0usize;
    assert_eq!(
        alec_encoder_context_save(enc, buf.as_mut_ptr(), buf.len(), &mut written),
        AlecResult::Ok
    );
    assert_eq!(written, need);

    alec_encoder_free(enc);
}

// ==========================================================================
// Task 3.5 — NULL-pointer safety.
// ==========================================================================

#[test]
fn null_safety_on_encoder_context_save_load() {
    let enc = alec_encoder_new_with_config(ptr::null());
    let mut buf = [0u8; 64];
    let mut n = 0usize;

    // save
    assert_eq!(
        alec_encoder_context_save(ptr::null(), buf.as_mut_ptr(), buf.len(), &mut n),
        AlecResult::ErrorNullPointer
    );
    assert_eq!(
        alec_encoder_context_save(enc, ptr::null_mut(), buf.len(), &mut n),
        AlecResult::ErrorNullPointer
    );
    assert_eq!(
        alec_encoder_context_save(enc, buf.as_mut_ptr(), buf.len(), ptr::null_mut()),
        AlecResult::ErrorNullPointer
    );

    // load
    assert_eq!(
        alec_encoder_context_load(ptr::null_mut(), buf.as_ptr(), buf.len()),
        AlecResult::ErrorNullPointer
    );
    assert_eq!(
        alec_encoder_context_load(enc, ptr::null(), buf.len()),
        AlecResult::ErrorNullPointer
    );

    alec_encoder_free(enc);
}

// ==========================================================================
// Corruption detection.
// ==========================================================================

#[test]
fn load_rejects_corrupt_header_and_preserves_encoder() {
    let cfg0 = cfg(10);
    let enc = alec_encoder_new_with_config(&cfg0);
    for _ in 0..10 {
        let _ = encode_one(enc, &[3.6, 22.5, 45.0, 420.0, 1013.25]);
    }

    // Snapshot the healthy state.
    let snap_ok = save_enc(enc);

    // 1. Too short.
    let tiny = [0u8; 10];
    assert_eq!(
        alec_encoder_context_load(enc, tiny.as_ptr(), tiny.len()),
        AlecResult::ErrorCorruptData
    );

    // 2. Bad magic.
    let mut bad_magic = snap_ok.clone();
    bad_magic[0] = b'X';
    assert_eq!(
        alec_encoder_context_load(enc, bad_magic.as_ptr(), bad_magic.len()),
        AlecResult::ErrorCorruptData
    );

    // 3. Bad format version.
    let mut bad_ver = snap_ok.clone();
    bad_ver[4] = 0xFF;
    assert_eq!(
        alec_encoder_context_load(enc, bad_ver.as_ptr(), bad_ver.len()),
        AlecResult::ErrorCorruptData
    );

    // 4. Header bit flip that the xxh64 must catch.
    let mut bit_flip = snap_ok.clone();
    bit_flip[7] ^= 0x01; // flip a bit in the sequence field
    assert_eq!(
        alec_encoder_context_load(enc, bit_flip.as_ptr(), bit_flip.len()),
        AlecResult::ErrorCorruptData
    );

    // 5. Truncated ALCS payload (length field says more than we have).
    let mut truncated = snap_ok.clone();
    truncated.truncate(snap_ok.len() - 10);
    assert_eq!(
        alec_encoder_context_load(enc, truncated.as_ptr(), truncated.len()),
        AlecResult::ErrorCorruptData
    );

    // Healthy snapshot must still load after all the failed attempts
    // — the failed loads did NOT touch the encoder state.
    assert_eq!(
        alec_encoder_context_load(enc, snap_ok.as_ptr(), snap_ok.len()),
        AlecResult::Ok
    );

    alec_encoder_free(enc);
}
