//! Regression tests for protocol header v2 changes:
//! - Timestamp stored as seconds (÷1000) instead of truncated milliseconds
//! - Sequence field u16 instead of u32
//! - context_version serialized as u24 (3 bytes)
//! - encode_raw() uses context.version() instead of hardcoded 0

use alec::{
    Classifier, Context, Encoder, MessageHeader, MessageType, Priority, RawData,
};

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
