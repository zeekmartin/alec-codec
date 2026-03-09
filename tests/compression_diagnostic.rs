//! Diagnostic test: why does encode_value() always return Raw32
//! even after 20+ messages of slowly drifting sensor data?
//!
//! Run with: cargo test compression_diagnostic -- --nocapture

use alec::classifier::Classifier;
use alec::context::Context;
use alec::encoder::Encoder;
use alec::protocol::{EncodingType, RawData};

/// Decode a varint from a byte slice, returning (value, bytes_consumed).
fn decode_varint(data: &[u8]) -> Option<(u32, usize)> {
    let mut result: u32 = 0;
    let mut shift = 0;
    for (i, &byte) in data.iter().enumerate() {
        result |= ((byte & 0x7F) as u32) << shift;
        if byte & 0x80 == 0 {
            return Some((result, i + 1));
        }
        shift += 7;
        if shift >= 35 {
            return None; // overflow
        }
    }
    None
}

/// Extract the encoding type from a payload by properly skipping the varint source_id.
fn extract_encoding_type(payload: &[u8]) -> Option<EncodingType> {
    let (_, varint_len) = decode_varint(payload)?;
    let enc_byte = *payload.get(varint_len)?;
    EncodingType::from_u8(enc_byte)
}

#[test]
fn compression_diagnostic() {
    let mut encoder = Encoder::new();
    let classifier = Classifier::default();
    let mut context = Context::new();

    // Hash "temp" the same way alec-ffi does: xxh64 mod 127 + 1 (1-byte varint)
    let source_id = (xxhash_rust::xxh64::xxh64(b"temp", 0) % 127 + 1) as u32;

    let drift_pattern: [f64; 10] = [8.0, -6.0, 10.0, -4.0, 7.0, -9.0, 5.0, -3.0, 11.0, -8.0];
    let mut value = 2400.0_f64;
    let mut timestamp = 0_u64;
    let mut prev_value = value;

    // Counters per encoding type
    let mut raw64_count = 0u32;
    let mut raw32_count = 0u32;
    let mut delta8_count = 0u32;
    let mut delta16_count = 0u32;
    let mut delta32_count = 0u32;
    let mut repeated_count = 0u32;
    let mut interpolated_count = 0u32;
    let mut pattern_count_enc = 0u32;
    let mut other_count = 0u32;
    let mut first_non_raw32: Option<usize> = None;

    println!();
    println!("=============================================================================");
    println!(
        "  ALEC COMPRESSION DIAGNOSTIC — 50 messages, source_id=\"temp\" (hash={:#010x})",
        source_id
    );
    println!("  scale_factor = {}", context.scale_factor());
    println!("=============================================================================");
    println!();

    for i in 0..50 {
        let delta_input = drift_pattern[i % drift_pattern.len()];
        if i > 0 {
            value += delta_input;
        }

        let raw_data = RawData::with_source(source_id, value, timestamp);

        // ── Prediction BEFORE encoding ──────────────────────────────
        let prediction = context.predict(source_id);
        let last_val = context.last_value(source_id);

        let ema_pred = prediction.as_ref().map(|p| p.value);
        let ema_conf = prediction.as_ref().map(|p| p.confidence);
        let pred_delta = ema_pred.map(|p| value - p);

        // Compute what choose_encoding would compute
        let scale = context.scale_factor() as f64;
        let scaled_delta = pred_delta.map(|d| {
            let raw = d * scale;
            if raw >= 0.0 {
                raw + 0.5
            } else {
                raw - 0.5
            }
        });
        let fits_i8 = scaled_delta
            .map(|sd| sd >= i8::MIN as f64 && sd <= i8::MAX as f64)
            .unwrap_or(false);
        let fits_i16 = scaled_delta
            .map(|sd| sd >= i16::MIN as f64 && sd <= i16::MAX as f64)
            .unwrap_or(false);

        // ── Classify ────────────────────────────────────────────────
        let classification = classifier.classify(&raw_data, &context);

        // ── Encode ──────────────────────────────────────────────────
        let message = encoder.encode(&raw_data, &classification, &context);
        let encoded = message.to_bytes();
        // Use both: the now-fixed encoding_type() and our manual parser for cross-check
        let encoding = message.encoding_type();
        let encoding_manual = extract_encoding_type(&message.payload);
        assert_eq!(
            encoding, encoding_manual,
            "encoding_type() disagrees with manual varint parse at message {}",
            i
        );

        // ── Observe (update context AFTER encoding, like FFI does) ─
        context.observe(&raw_data);

        // ── Bookkeeping ─────────────────────────────────────────────
        let enc_type_str = match encoding {
            Some(EncodingType::Raw64) => {
                raw64_count += 1;
                "Raw64"
            }
            Some(EncodingType::Raw32) => {
                raw32_count += 1;
                "Raw32"
            }
            Some(EncodingType::Delta8) => {
                delta8_count += 1;
                "Delta8"
            }
            Some(EncodingType::Delta16) => {
                delta16_count += 1;
                "Delta16"
            }
            Some(EncodingType::Delta32) => {
                delta32_count += 1;
                "Delta32"
            }
            Some(EncodingType::Repeated) => {
                repeated_count += 1;
                "Repeated"
            }
            Some(EncodingType::Interpolated) => {
                interpolated_count += 1;
                "Interpolated"
            }
            Some(EncodingType::Pattern) | Some(EncodingType::PatternDelta) => {
                pattern_count_enc += 1;
                "Pattern"
            }
            _ => {
                other_count += 1;
                "???"
            }
        };

        if first_non_raw32.is_none() && encoding != Some(EncodingType::Raw32) {
            first_non_raw32 = Some(i);
        }

        let input_delta = if i == 0 { 0.0 } else { value - prev_value };

        // ── Print ───────────────────────────────────────────────────
        println!("--- Message {} ---", i);
        println!("  INPUT:");
        println!("    value          = {:.1}", value);
        println!("    delta_from_prev= {:+.1}", input_delta);
        println!("    timestamp      = {}", timestamp);
        println!("  CLASSIFICATION:");
        println!("    reason         = {:?}", classification.reason);
        println!("    priority       = {:?}", classification.priority);
        println!("    class.delta    = {:.4}", classification.delta);
        println!("    confidence     = {:.4}", classification.confidence);
        println!("  PREDICTION (before encode):");
        println!("    last_value     = {:?}", last_val);
        println!("    ema_prediction = {:?}", ema_pred);
        println!("    ema_confidence = {:?}", ema_conf);
        println!("    pred_delta     = {:?}", pred_delta);
        println!("    scaled_delta   = {:?}  (scale={})", scaled_delta, scale);
        println!(
            "    fits_i8?       = {}   fits_i16? = {}",
            fits_i8, fits_i16
        );
        println!("  OUTPUT:");
        println!("    encoding       = {}", enc_type_str);
        println!("    total_bytes    = {}", encoded.len());
        println!("    payload_hex    = {}", hex_dump(&message.payload));
        println!(
            "    ratio_vs_f64   = {:.2}x",
            8.0 / message.payload.len().max(1) as f64
        );
        println!("  CONTEXT (after observe):");
        println!("    pattern_count  = {}", context.pattern_count());
        println!("    context_ver    = {}", context.version());
        println!("    observations   = {}", context.observation_count());
        println!();

        prev_value = value;
        timestamp += 5000;
    }

    // ── Summary ─────────────────────────────────────────────────────
    println!("=============================================================================");
    println!("  SUMMARY (50 messages)");
    println!("=============================================================================");
    println!("  Raw64        : {}", raw64_count);
    println!("  Raw32        : {}", raw32_count);
    println!("  Delta8       : {}", delta8_count);
    println!("  Delta16      : {}", delta16_count);
    println!("  Delta32      : {}", delta32_count);
    println!("  Repeated     : {}", repeated_count);
    println!("  Interpolated : {}", interpolated_count);
    println!("  Pattern      : {}", pattern_count_enc);
    println!("  Other/???    : {}", other_count);
    println!();
    match first_non_raw32 {
        Some(idx) => println!("  First non-Raw32 at message #{}", idx),
        None => println!("  *** ALL 50 messages used Raw32 — no compression ever kicked in ***"),
    }
    println!("=============================================================================");
}

fn hex_dump(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ")
}
