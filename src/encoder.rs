//! Encoder module
//!
//! This module handles encoding data into compact binary messages
//! using various encoding strategies based on context and classification.

use crate::classifier::Classification;
use crate::context::Context;
use crate::protocol::{
    EncodedMessage, EncodingType, MessageHeader, MessageType, Priority, RawData,
};

/// Encoder for ALEC messages
#[derive(Debug, Clone)]
pub struct Encoder {
    /// Next sequence number
    sequence: u32,
    /// Whether to include checksum (reserved for future use)
    _include_checksum: bool,
}

impl Encoder {
    /// Create a new encoder
    pub fn new() -> Self {
        Self {
            sequence: 0,
            _include_checksum: false,
        }
    }

    /// Create encoder with checksum enabled
    pub fn with_checksum() -> Self {
        Self {
            sequence: 0,
            _include_checksum: true,
        }
    }

    /// Get current sequence number
    pub fn sequence(&self) -> u32 {
        self.sequence
    }

    /// Reset sequence number
    pub fn reset_sequence(&mut self) {
        self.sequence = 0;
    }

    /// Encode data with classification into a message
    pub fn encode(
        &mut self,
        data: &RawData,
        classification: &Classification,
        context: &Context,
    ) -> EncodedMessage {
        // Check for invalid values
        if data.value.is_nan() || data.value.is_infinite() {
            // Fall back to raw encoding for invalid values
            return self.encode_raw(data, classification.priority);
        }

        // Choose encoding based on context
        let (encoding_type, encoded_value) = self.choose_encoding(data, context);

        // Build payload
        let mut payload = Vec::new();
        
        // Source ID (varint encoding)
        self.encode_varint(data.source_id, &mut payload);
        
        // Encoding type
        payload.push(encoding_type as u8);
        
        // Encoded value
        payload.extend(encoded_value);

        // Build header
        let header = MessageHeader {
            version: crate::PROTOCOL_VERSION,
            message_type: MessageType::Data,
            priority: classification.priority,
            sequence: self.next_sequence(),
            timestamp: (data.timestamp & 0xFFFFFFFF) as u32,
            context_version: context.version(),
        };

        EncodedMessage::new(header, payload)
    }

    /// Encode as raw (fallback)
    fn encode_raw(&mut self, data: &RawData, priority: Priority) -> EncodedMessage {
        let mut payload = Vec::new();
        
        // Source ID
        self.encode_varint(data.source_id, &mut payload);
        
        // Encoding type
        payload.push(EncodingType::Raw64 as u8);
        
        // Raw value
        payload.extend_from_slice(&data.value.to_be_bytes());

        let header = MessageHeader {
            version: crate::PROTOCOL_VERSION,
            message_type: MessageType::Data,
            priority,
            sequence: self.next_sequence(),
            timestamp: (data.timestamp & 0xFFFFFFFF) as u32,
            context_version: 0,
        };

        EncodedMessage::new(header, payload)
    }

    /// Choose the best encoding for this value
    fn choose_encoding(&self, data: &RawData, context: &Context) -> (EncodingType, Vec<u8>) {
        // Check if value matches last value exactly (repeated) — MOST COMPACT
        if let Some(last) = context.last_value(data.source_id) {
            if (data.value - last).abs() < f64::EPSILON {
                return (EncodingType::Repeated, vec![]);
            }
        }

        // Try to get prediction for delta encoding
        if let Some(prediction) = context.predict(data.source_id) {
            let delta = data.value - prediction.value;
            let scale = context.scale_factor() as f64;
            let scaled_delta = (delta * scale).round();

            // Check if delta fits in i8
            if scaled_delta >= i8::MIN as f64 && scaled_delta <= i8::MAX as f64 {
                let delta_i8 = scaled_delta as i8;
                return (EncodingType::Delta8, vec![delta_i8 as u8]);
            }

            // Check if delta fits in i16
            if scaled_delta >= i16::MIN as f64 && scaled_delta <= i16::MAX as f64 {
                let delta_i16 = scaled_delta as i16;
                return (EncodingType::Delta16, delta_i16.to_be_bytes().to_vec());
            }

            // Check if delta fits in i32
            if scaled_delta >= i32::MIN as f64 && scaled_delta <= i32::MAX as f64 {
                let delta_i32 = scaled_delta as i32;
                return (EncodingType::Delta32, delta_i32.to_be_bytes().to_vec());
            }
        }

        // Check if value can fit in f32 without significant loss
        let as_f32 = data.value as f32;
        if (as_f32 as f64 - data.value).abs() < 0.0001 {
            return (EncodingType::Raw32, as_f32.to_be_bytes().to_vec());
        }

        // Fall back to raw f64
        (EncodingType::Raw64, data.value.to_be_bytes().to_vec())
    }

    /// Encode a varint (variable-length integer)
    fn encode_varint(&self, value: u32, output: &mut Vec<u8>) {
        let mut v = value;
        while v >= 0x80 {
            output.push((v as u8 & 0x7F) | 0x80);
            v >>= 7;
        }
        output.push(v as u8);
    }

    /// Get next sequence number
    fn next_sequence(&mut self) -> u32 {
        let seq = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);
        seq
    }

    /// Encode multiple values in one message
    pub fn encode_multi(
        &mut self,
        values: &[(u16, f64)], // (name_id, value) pairs
        source_id: u32,
        timestamp: u64,
        priority: Priority,
        context: &Context,
    ) -> EncodedMessage {
        let mut payload = Vec::new();
        
        // Source ID
        self.encode_varint(source_id, &mut payload);
        
        // Multi encoding type
        payload.push(EncodingType::Multi as u8);
        
        // Count
        payload.push(values.len() as u8);
        
        // Each value
        for (name_id, value) in values {
            // Name ID (2 bytes BE)
            payload.extend_from_slice(&name_id.to_be_bytes());
            
            // Simple encoding for multi (just use Raw32 for simplicity)
            payload.push(EncodingType::Raw32 as u8);
            payload.extend_from_slice(&(*value as f32).to_be_bytes());
        }

        let header = MessageHeader {
            version: crate::PROTOCOL_VERSION,
            message_type: MessageType::Data,
            priority,
            sequence: self.next_sequence(),
            timestamp: (timestamp & 0xFFFFFFFF) as u32,
            context_version: context.version(),
        };

        EncodedMessage::new(header, payload)
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating encoded messages manually
pub struct MessageBuilder {
    header: MessageHeader,
    payload: Vec<u8>,
}

impl MessageBuilder {
    /// Create a new message builder
    pub fn new() -> Self {
        Self {
            header: MessageHeader::default(),
            payload: Vec::new(),
        }
    }

    /// Set message type
    pub fn message_type(mut self, msg_type: MessageType) -> Self {
        self.header.message_type = msg_type;
        self
    }

    /// Set priority
    pub fn priority(mut self, priority: Priority) -> Self {
        self.header.priority = priority;
        self
    }

    /// Set sequence number
    pub fn sequence(mut self, seq: u32) -> Self {
        self.header.sequence = seq;
        self
    }

    /// Set timestamp
    pub fn timestamp(mut self, ts: u32) -> Self {
        self.header.timestamp = ts;
        self
    }

    /// Set context version
    pub fn context_version(mut self, version: u32) -> Self {
        self.header.context_version = version;
        self
    }

    /// Set payload
    pub fn payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = payload;
        self
    }

    /// Build the message
    pub fn build(self) -> EncodedMessage {
        EncodedMessage::new(self.header, self.payload)
    }
}

impl Default for MessageBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classifier::Classifier;

    #[test]
    fn test_encode_basic() {
        let mut encoder = Encoder::new();
        let classifier = Classifier::default();
        let context = Context::new();
        
        let data = RawData::new(42.0, 0);
        let classification = classifier.classify(&data, &context);
        let message = encoder.encode(&data, &classification, &context);
        
        assert!(!message.is_empty());
        assert!(message.len() < data.raw_size() + MessageHeader::SIZE);
    }

    #[test]
    fn test_encode_with_context() {
        let mut encoder = Encoder::new();
        let classifier = Classifier::default();
        let mut context = Context::new();
        
        // Build context
        for i in 0..10 {
            context.observe(&RawData::new(20.0 + i as f64 * 0.1, i as u64));
        }
        
        // Encode value close to prediction
        let data = RawData::new(21.0, 100);
        let classification = classifier.classify(&data, &context);
        let message = encoder.encode(&data, &classification, &context);
        
        // Should use delta encoding (smaller)
        let encoding = message.encoding_type();
        assert!(matches!(
            encoding,
            Some(EncodingType::Delta8) | Some(EncodingType::Delta16)
        ));
    }

    #[test]
    fn test_encode_repeated() {
        let mut encoder = Encoder::new();
        let classifier = Classifier::default();
        let mut context = Context::new();
        
        // Observe a value
        context.observe(&RawData::new(42.0, 0));
        
        // Encode same value
        let data = RawData::new(42.0, 1);
        let classification = classifier.classify(&data, &context);
        let message = encoder.encode(&data, &classification, &context);
        
        // Should use repeated encoding (very small)
        assert_eq!(message.encoding_type(), Some(EncodingType::Repeated));
    }

    #[test]
    fn test_sequence_increment() {
        let mut encoder = Encoder::new();
        let classifier = Classifier::default();
        let context = Context::new();
        
        let data = RawData::new(42.0, 0);
        let classification = classifier.classify(&data, &context);
        
        let msg1 = encoder.encode(&data, &classification, &context);
        let msg2 = encoder.encode(&data, &classification, &context);
        let msg3 = encoder.encode(&data, &classification, &context);
        
        assert_eq!(msg1.header.sequence, 0);
        assert_eq!(msg2.header.sequence, 1);
        assert_eq!(msg3.header.sequence, 2);
    }

    #[test]
    fn test_encode_nan() {
        let mut encoder = Encoder::new();
        let classifier = Classifier::default();
        let context = Context::new();
        
        let data = RawData::new(f64::NAN, 0);
        let classification = classifier.classify(&data, &context);
        let message = encoder.encode(&data, &classification, &context);
        
        // Should fall back to raw encoding
        assert_eq!(message.encoding_type(), Some(EncodingType::Raw64));
    }

    #[test]
    fn test_encode_multi() {
        let mut encoder = Encoder::new();
        let context = Context::new();
        
        let values = vec![
            (1, 22.5),  // temperature
            (2, 65.0),  // humidity
            (3, 1013.25), // pressure
        ];
        
        let message = encoder.encode_multi(
            &values,
            42,
            12345,
            Priority::P3Normal,
            &context,
        );
        
        assert_eq!(message.encoding_type(), Some(EncodingType::Multi));
    }

    #[test]
    fn test_varint_encoding() {
        let encoder = Encoder::new();
        
        // Small value (1 byte)
        let mut out1 = Vec::new();
        encoder.encode_varint(42, &mut out1);
        assert_eq!(out1.len(), 1);
        
        // Medium value (2 bytes)
        let mut out2 = Vec::new();
        encoder.encode_varint(200, &mut out2);
        assert_eq!(out2.len(), 2);
        
        // Large value
        let mut out3 = Vec::new();
        encoder.encode_varint(100000, &mut out3);
        assert!(out3.len() >= 3);
    }

    #[test]
    fn test_message_builder() {
        let message = MessageBuilder::new()
            .message_type(MessageType::Sync)
            .priority(Priority::P1Critical)
            .sequence(42)
            .timestamp(12345)
            .payload(vec![1, 2, 3])
            .build();
        
        assert_eq!(message.header.message_type, MessageType::Sync);
        assert_eq!(message.header.priority, Priority::P1Critical);
        assert_eq!(message.header.sequence, 42);
        assert_eq!(message.payload, vec![1, 2, 3]);
    }
}
