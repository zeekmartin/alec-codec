// Tests for v1.3 encode_multi_adaptive() — adaptive per-channel compression
// with shared header and priority-based inclusion.

use alec::classifier::Classifier;
use alec::context::Context;
use alec::protocol::{ChannelInput, EncodingType, MessageHeader, Priority, RawData};
use alec::{Decoder, Encoder};

/// Build 5-channel inputs with slight drift from a base.
/// source_id can be anything — the encoder uses name_id as context key for multi.
fn make_channels(base: &[f64; 5], drift: &[f64; 5]) -> Vec<ChannelInput> {
    (0..5)
        .map(|i| ChannelInput {
            name_id: i as u8,
            source_id: (i as u32) + 1, // caller's logical id (not used for multi context)
            value: base[i] + drift[i],
        })
        .collect()
}

/// Warm up context with `rounds` identical observations per channel.
/// Uses `i as u32` as source_id (= name_id), matching encode_multi_adaptive convention.
fn warmup(ctx: &mut Context, base: &[f64; 5], rounds: usize) {
    for r in 0..rounds {
        for (i, &val) in base.iter().enumerate() {
            let rd = RawData::with_source(i as u32, val, r as u64);
            ctx.observe(&rd);
        }
    }
}

#[test]
fn test_encode_multi_adaptive() {
    let mut encoder = Encoder::new();
    let classifier = Classifier::default();
    let mut context = Context::new();

    let base: [f64; 5] = [22.5, 65.0, 1013.25, 3.3, 48.0];

    // Warm up with 20 rounds of identical values
    warmup(&mut context, &base, 20);

    // Encode with moderate drift (>1% relative) so classifier assigns P3/P4, not P5
    let drift: [f64; 5] = [0.5, -1.5, 15.0, 0.0, 1.0];
    let channels = make_channels(&base, &drift);

    let (message, _classifications) =
        encoder.encode_multi_adaptive(&channels, 100, &context, &classifier);

    // Parse the multi payload to check encoding types
    let payload = &message.payload;

    // Skip: source_id varint (1B for 0), Multi tag (1B), count (1B)
    let mut pos = 0;
    // source_id varint
    while pos < payload.len() && payload[pos] & 0x80 != 0 {
        pos += 1;
    }
    pos += 1; // end of varint

    assert_eq!(
        payload[pos],
        EncodingType::Multi as u8,
        "Expected Multi tag"
    );
    pos += 1;

    let count = payload[pos];
    pos += 1;

    println!("Included channels: {}", count);

    let mut saw_delta_or_repeated = false;
    for ch_idx in 0..count {
        // name_id (1B)
        let name_id = payload[pos];
        pos += 1;

        // encoding type (1B)
        let enc_byte = payload[pos];
        pos += 1;

        let enc_type = EncodingType::from_u8(enc_byte).unwrap();
        let value_size = enc_type.typical_size();
        pos += value_size;

        println!(
            "  ch[{}] name_id={} encoding={:?} value_bytes={}",
            ch_idx, name_id, enc_type, value_size
        );

        match enc_type {
            EncodingType::Delta8 | EncodingType::Delta16 | EncodingType::Repeated => {
                saw_delta_or_repeated = true;
            }
            _ => {}
        }
    }

    assert!(
        saw_delta_or_repeated,
        "Expected at least one Delta8/Delta16/Repeated after warmup, but all were raw"
    );
}

#[test]
fn test_encode_multi_p5_suppression() {
    let mut encoder = Encoder::new();
    let classifier = Classifier::default();
    let mut context = Context::new();

    let base: [f64; 5] = [22.5, 65.0, 1013.25, 3.3, 48.0];

    // Warm up so classifier has predictions
    warmup(&mut context, &base, 20);

    // Channels with *no* change → classifier should assign P5 (BelowMinimumDelta)
    let channels = make_channels(&base, &[0.0001, 0.0001, 0.0001, 0.0001, 0.0001]);

    let (message, classifications) =
        encoder.encode_multi_adaptive(&channels, 200, &context, &classifier);

    // Count how many were classified P5
    let p5_count = classifications
        .iter()
        .filter(|c| c.priority == Priority::P5Disposable)
        .count();

    println!("P5 count: {} / {}", p5_count, classifications.len());

    // Parse count from payload
    let payload = &message.payload;
    let mut pos = 0;
    while pos < payload.len() && payload[pos] & 0x80 != 0 {
        pos += 1;
    }
    pos += 1; // end of varint
    pos += 1; // Multi tag
    let included_count = payload[pos] as usize;

    println!("Included in frame: {}", included_count);
    println!("Total channels: {}", channels.len());

    // P5 channels should be excluded from the frame
    assert!(
        included_count < channels.len() || p5_count == 0,
        "P5 channels should be excluded from frame, but included_count={} total={}",
        included_count,
        channels.len()
    );

    // If there were P5 channels, verify fewer are included
    if p5_count > 0 {
        assert_eq!(
            included_count,
            channels.len() - p5_count,
            "Included count should be total minus P5 count"
        );
    }
}

#[test]
fn test_encode_multi_shared_header() {
    let mut encoder_multi = Encoder::new();
    let mut encoder_single = Encoder::new();
    let classifier = Classifier::default();
    let mut context = Context::new();

    let base: [f64; 5] = [22.5, 65.0, 1013.25, 3.3, 48.0];

    // Warm up
    warmup(&mut context, &base, 20);

    // Drift >1% relative to base so channels are classified P3/P4, not P5
    let drift: [f64; 5] = [0.5, -1.5, 15.0, 0.1, -1.0];
    let channels = make_channels(&base, &drift);

    // Multi-channel encode: one shared header
    let (multi_msg, _) = encoder_multi.encode_multi_adaptive(&channels, 300, &context, &classifier);
    let multi_bytes = multi_msg.to_bytes();

    // Single-channel encode: 5 separate headers
    let mut single_total = 0usize;
    for ch in &channels {
        let raw = RawData::with_source(ch.source_id, ch.value, 300);
        let cls = classifier.classify(&raw, &context);
        let msg = encoder_single.encode(&raw, &cls, &context);
        single_total += msg.to_bytes().len();
    }

    println!(
        "Multi: {} bytes vs 5x single: {} bytes (ratio: {:.1}%)",
        multi_bytes.len(),
        single_total,
        (multi_bytes.len() as f64 / single_total as f64) * 100.0
    );

    // Multi should be significantly smaller than 5 separate messages
    // 5 × 15B minimum (header+sid+enc) = 75B vs multi = 13B header + ~18B payload
    assert!(
        multi_bytes.len() < single_total,
        "Multi ({}) should be smaller than sum of singles ({})",
        multi_bytes.len(),
        single_total
    );

    // Multi should save substantially (header amortisation + P5 exclusion)
    let saved = single_total - multi_bytes.len();
    println!(
        "Saved: {} bytes ({:.0}%)",
        saved,
        (saved as f64 / single_total as f64) * 100.0
    );
    // At minimum, we save several headers. The exact amount depends on how many
    // channels are included vs excluded as P5.
    assert!(
        saved >= 2 * MessageHeader::SIZE,
        "Should save at least 2 headers (20B), only saved {}",
        saved
    );
}

#[test]
fn test_encode_multi_adaptive_decode_roundtrip() {
    let mut encoder = Encoder::new();
    let mut decoder = Decoder::new();
    let classifier = Classifier::default();
    let mut enc_ctx = Context::new();
    let mut dec_ctx = Context::new();

    let base: [f64; 5] = [22.5, 65.0, 1013.25, 3.3, 48.0];

    // Warm up both contexts identically
    for r in 0..20 {
        for (i, &val) in base.iter().enumerate() {
            let rd = RawData::with_source((i as u32) + 1, val, r as u64);
            enc_ctx.observe(&rd);
            dec_ctx.observe(&rd);
        }
    }

    // Drift >1% relative to base so channels are included (not P5)
    let drift: [f64; 5] = [0.5, -1.5, 15.0, 0.1, -1.0];
    let channels = make_channels(&base, &drift);

    let (message, _) = encoder.encode_multi_adaptive(&channels, 100, &enc_ctx, &classifier);

    // Decode
    let decoded = decoder.decode_multi(&message, &dec_ctx).unwrap();

    assert!(!decoded.is_empty(), "Expected at least one decoded channel");

    // The decoded count may be less than input (P5 excluded), so check what we got
    for (name_id, decoded_val) in &decoded {
        let ch = &channels[*name_id as usize];
        let error = (decoded_val - ch.value).abs();
        println!(
            "ch[{}] expected={:.4} decoded={:.4} error={:.6}",
            name_id, ch.value, decoded_val, error
        );
        assert!(
            error < 0.5,
            "Decode error too large for ch[{}]: {}",
            name_id,
            error
        );
    }
}
