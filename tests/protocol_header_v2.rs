//! Regression tests for protocol header v2 changes:
//! - Timestamp stored as seconds (÷1000) instead of truncated milliseconds
//! - Sequence field u16 instead of u32
//! - context_version serialized as u24 (3 bytes)
//! - encode_raw() uses context.version() instead of hardcoded 0
//! - name_id serialized as u8 instead of u16 in multi-channel frame

use alec::{
    Classifier, Context, Encoder, MessageHeader, MessageType, Priority, RawData,
};
use alec::protocol::{ChannelInput, EncodingType};

#[test]
fn test_timestamp_seconds_not_ms() {
    let mut encoder = Encoder::new();
    let classifier = Classifier::default();
    let context = Context::new();

    // March 2025 in milliseconds
    let timestamp_ms: u64 = 1_741_234_567_000;
    let data = RawData::new(22.5, timestamp_ms);
    let classification = classifier.classify(&data, &context);
    let message = encoder.encode(&data, &classification, &context);

    // Should be seconds, not truncated ms
    assert_eq!(message.header.timestamp, 1_741_234_567u32);
    assert_ne!(
        message.header.timestamp,
        (1_741_234_567_000u64 & 0xFFFFFFFF) as u32
    );
}

#[test]
fn test_timestamp_no_49day_wrap() {
    let mut encoder = Encoder::new();
    let classifier = Classifier::default();
    let context = Context::new();

    // 50 days in milliseconds
    let timestamp_ms: u64 = 50 * 24 * 3600 * 1000; // 4_320_000_000
    let data = RawData::new(22.5, timestamp_ms);
    let classification = classifier.classify(&data, &context);
    let message = encoder.encode(&data, &classification, &context);

    // 50 days in seconds = 4_320_000
    assert_eq!(message.header.timestamp, 4_320_000u32);
}

#[test]
fn test_sequence_u16_rollover() {
    let mut encoder = Encoder::new();
    let classifier = Classifier::default();
    let context = Context::new();

    let data = RawData::new(22.5, 0);
    let classification = classifier.classify(&data, &context);

    // Burn through 65,535 calls (sequences 0..65535)
    for _ in 0..65_535 {
        encoder.encode(&data, &classification, &context);
    }

    // The 65,536th call should have sequence 65535
    let msg = encoder.encode(&data, &classification, &context);
    assert_eq!(msg.header.sequence, 65_535);

    // The 65,537th call wraps to 0
    let msg = encoder.encode(&data, &classification, &context);
    assert_eq!(msg.header.sequence, 0);

    // The 65,538th call is 1
    let msg = encoder.encode(&data, &classification, &context);
    assert_eq!(msg.header.sequence, 1);
}

#[test]
fn test_sequence_2_bytes_in_header() {
    let header = MessageHeader {
        version: 1,
        message_type: MessageType::Data,
        priority: Priority::P3Normal,
        sequence: 0x1234,
        timestamp: 0,
        context_version: 0,
    };

    let bytes = header.to_bytes();
    assert_eq!(bytes.len(), MessageHeader::SIZE);
    assert_eq!(MessageHeader::SIZE, 10);

    // Sequence occupies bytes[1..3] (2 bytes, big-endian)
    assert_eq!(bytes[1], 0x12);
    assert_eq!(bytes[2], 0x34);
}

#[test]
fn test_context_version_u24_range() {
    let header = MessageHeader {
        version: 1,
        message_type: MessageType::Data,
        priority: Priority::P3Normal,
        sequence: 0,
        timestamp: 0,
        context_version: 0x00ABCDEF,
    };

    let bytes = header.to_bytes();
    let restored = MessageHeader::from_bytes(&bytes).unwrap();
    assert_eq!(restored.context_version, 0x00ABCDEF);
}

#[test]
fn test_context_version_3_bytes_in_header() {
    let header = MessageHeader {
        version: 1,
        message_type: MessageType::Data,
        priority: Priority::P3Normal,
        sequence: 0,
        timestamp: 0,
        context_version: 255,
    };

    let bytes = header.to_bytes();
    // context_version at bytes[7..10] as u24 big-endian
    assert_eq!(&bytes[7..10], &[0x00, 0x00, 0xFF]);
    assert_eq!(MessageHeader::SIZE, 10);
}

#[test]
fn test_header_roundtrip_all_fields() {
    let header = MessageHeader {
        version: 1,
        message_type: MessageType::Sync,
        priority: Priority::P2Important,
        sequence: 60_000,
        timestamp: 1_741_234_567,
        context_version: 0x00AABBCC,
    };

    let bytes = header.to_bytes();
    let restored = MessageHeader::from_bytes(&bytes).unwrap();

    assert_eq!(restored.version, 1);
    assert_eq!(restored.message_type, MessageType::Sync);
    assert_eq!(restored.priority, Priority::P2Important);
    assert_eq!(restored.sequence, 60_000);
    assert_eq!(restored.timestamp, 1_741_234_567);
    assert_eq!(restored.context_version, 0x00AABBCC);
}

#[test]
fn test_encode_raw_context_version_not_zero() {
    let mut encoder = Encoder::new();
    let classifier = Classifier::default();
    let mut context = Context::new();

    // Warm up context so version > 0
    for i in 0..5 {
        let d = RawData::new(20.0 + i as f64, 1000 * i as u64);
        context.observe(&d);
    }
    assert!(context.version() > 0);

    // Encode a NaN → triggers encode_raw()
    let data = RawData::new(f64::NAN, 5000);
    let classification = classifier.classify(&data, &context);
    let message = encoder.encode(&data, &classification, &context);

    // encode_raw should use context.version(), not 0
    assert_ne!(message.header.context_version, 0);
    assert_eq!(message.header.context_version, context.version());
}

// ─── name_id u8 regression tests ───────────────────────────────────

/// Helper: warm up context for multi-channel tests
fn warmup_multi(ctx: &mut Context, values: &[f64], rounds: usize) {
    for r in 0..rounds {
        for (i, &val) in values.iter().enumerate() {
            let rd = RawData::with_source(i as u32, val, r as u64);
            ctx.observe(&rd);
        }
    }
}

#[test]
fn test_name_id_1_byte_in_frame() {
    let mut encoder = Encoder::new();
    let classifier = Classifier::default();
    let mut context = Context::new();

    let base = [22.5, 65.0, 1013.25];
    warmup_multi(&mut context, &base, 20);

    let channels: Vec<ChannelInput> = (0..3)
        .map(|i| ChannelInput {
            name_id: i as u8,
            source_id: i as u32,
            value: base[i] + 0.5, // slight drift → P3
        })
        .collect();

    let (message, _) =
        encoder.encode_multi_adaptive(&channels, 1000, &context, &classifier);

    let payload = &message.payload;

    // Skip varint source_id + Multi tag + count byte
    let mut pos = 0;
    while pos < payload.len() && payload[pos] & 0x80 != 0 {
        pos += 1;
    }
    pos += 1; // end of varint
    assert_eq!(payload[pos], EncodingType::Multi as u8);
    pos += 1; // Multi tag
    let count = payload[pos] as usize;
    pos += 1; // count

    // Parse each channel: name_id should be exactly 1 byte
    for _ in 0..count {
        let name_id = payload[pos];
        assert!(name_id <= 2, "name_id should be 0, 1, or 2");
        pos += 1; // name_id (1 byte!)

        let enc_byte = payload[pos];
        pos += 1;
        let enc_type = EncodingType::from_u8(enc_byte).unwrap();
        pos += enc_type.typical_size();
    }

    // We should have consumed exactly the whole payload
    assert_eq!(pos, payload.len(), "Payload not fully consumed — name_id is not 1 byte");
}

#[test]
fn test_name_id_max_u8() {
    let mut encoder = Encoder::new();
    let mut decoder = alec::Decoder::new();
    let classifier = Classifier::default();
    let context = Context::new();

    let channels = vec![ChannelInput {
        name_id: 255,
        source_id: 255,
        value: 42.0,
    }];

    let (message, _) =
        encoder.encode_multi_adaptive(&channels, 1000, &context, &classifier);
    let decoded = decoder.decode_multi(&message, &context).unwrap();

    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].0, 255u8);
}

#[test]
fn test_name_id_roundtrip() {
    let mut encoder = Encoder::new();
    let mut decoder = alec::Decoder::new();
    let classifier = Classifier::default();
    let mut context = Context::new();

    let ids: [u8; 5] = [10, 42, 100, 200, 255];
    let values: [f64; 5] = [22.5, 65.0, 1013.25, 3.3, 48.0];

    // Warm up context keyed by name_id
    for r in 0..20 {
        for (i, &val) in values.iter().enumerate() {
            let rd = RawData::with_source(ids[i] as u32, val, r as u64);
            context.observe(&rd);
        }
    }

    // Use large drift (>2% relative) so no channel is classified P5
    let drifts: [f64; 5] = [1.0, 3.0, 30.0, 0.2, 2.0];
    let channels: Vec<ChannelInput> = ids
        .iter()
        .zip(values.iter().zip(drifts.iter()))
        .map(|(&id, (&val, &drift))| ChannelInput {
            name_id: id,
            source_id: id as u32,
            value: val + drift,
        })
        .collect();

    let (message, _) =
        encoder.encode_multi_adaptive(&channels, 2000, &context, &classifier);
    let decoded = decoder.decode_multi(&message, &context).unwrap();

    assert_eq!(decoded.len(), 5);
    for (i, (name_id, _value)) in decoded.iter().enumerate() {
        assert_eq!(*name_id, ids[i], "name_id mismatch at channel {}", i);
    }
}
