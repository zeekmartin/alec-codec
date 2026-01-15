//! Decoder module
//!
//! This module handles decoding binary messages back into data values
//! using the shared context for decompression.

use crate::context::Context;
use crate::error::{DecodeError, Result};
use crate::protocol::{DecodedData, EncodedMessage, EncodingType};

/// Decoder for ALEC messages
#[derive(Debug, Clone)]
pub struct Decoder {
    /// Whether to verify checksum (reserved for future use)
    _verify_checksum: bool,
    /// Last decoded sequence number (for gap detection)
    last_sequence: Option<u32>,
}

impl Decoder {
    /// Create a new decoder
    pub fn new() -> Self {
        Self {
            _verify_checksum: false,
            last_sequence: None,
        }
    }

    /// Create decoder with checksum verification
    pub fn with_checksum_verification() -> Self {
        Self {
            _verify_checksum: true,
            last_sequence: None,
        }
    }

    /// Decode a message
    pub fn decode(&mut self, message: &EncodedMessage, context: &Context) -> Result<DecodedData> {
        // Check for sequence gaps
        if let Some(last_seq) = self.last_sequence {
            let expected = last_seq.wrapping_add(1);
            if message.header.sequence != expected {
                // Sequence gap detected (could log warning)
                // For now, just continue
            }
        }
        self.last_sequence = Some(message.header.sequence);

        // Parse payload
        let payload = &message.payload;
        if payload.is_empty() {
            return Err(DecodeError::BufferTooShort {
                needed: 1,
                available: 0,
            }
            .into());
        }

        // Decode source ID (varint)
        let (source_id, offset) = self.decode_varint(payload)?;

        if offset >= payload.len() {
            return Err(DecodeError::BufferTooShort {
                needed: offset + 1,
                available: payload.len(),
            }
            .into());
        }

        // Decode encoding type
        let encoding_byte = payload[offset];
        let encoding_type = EncodingType::from_u8(encoding_byte)
            .ok_or(DecodeError::UnknownEncodingType(encoding_byte))?;

        // Decode value based on encoding type
        let value = self.decode_value(encoding_type, &payload[offset + 1..], source_id, context)?;

        Ok(DecodedData::new(
            source_id,
            message.header.timestamp as u64,
            value,
            message.header.priority,
        ))
    }

    /// Decode from raw bytes
    pub fn decode_bytes(&mut self, bytes: &[u8], context: &Context) -> Result<DecodedData> {
        let message = EncodedMessage::from_bytes(bytes).ok_or(DecodeError::InvalidHeader)?;
        self.decode(&message, context)
    }

    /// Decode a varint from the buffer
    fn decode_varint(&self, buffer: &[u8]) -> Result<(u32, usize)> {
        let mut result: u32 = 0;
        let mut shift = 0;
        let mut offset = 0;

        loop {
            if offset >= buffer.len() {
                return Err(DecodeError::BufferTooShort {
                    needed: offset + 1,
                    available: buffer.len(),
                }
                .into());
            }

            let byte = buffer[offset];
            result |= ((byte & 0x7F) as u32) << shift;
            offset += 1;

            if byte & 0x80 == 0 {
                break;
            }

            shift += 7;
            if shift >= 32 {
                return Err(DecodeError::MalformedMessage {
                    offset,
                    reason: "Varint too long".to_string(),
                }
                .into());
            }
        }

        Ok((result, offset))
    }

    /// Decode value based on encoding type
    fn decode_value(
        &self,
        encoding_type: EncodingType,
        data: &[u8],
        source_id: u32,
        context: &Context,
    ) -> Result<f64> {
        match encoding_type {
            EncodingType::Raw64 => self.decode_raw64(data),
            EncodingType::Raw32 => self.decode_raw32(data),
            EncodingType::Delta8 => self.decode_delta8(data, source_id, context),
            EncodingType::Delta16 => self.decode_delta16(data, source_id, context),
            EncodingType::Delta32 => self.decode_delta32(data, source_id, context),
            EncodingType::Repeated => self.decode_repeated(source_id, context),
            EncodingType::Interpolated => self.decode_interpolated(source_id, context),
            EncodingType::Pattern => self.decode_pattern(data, context),
            EncodingType::PatternDelta => self.decode_pattern_delta(data, context),
            EncodingType::Multi => Err(DecodeError::MalformedMessage {
                offset: 0,
                reason: "Multi encoding should use decode_multi".to_string(),
            }
            .into()),
        }
    }

    /// Decode raw f64
    fn decode_raw64(&self, data: &[u8]) -> Result<f64> {
        if data.len() < 8 {
            return Err(DecodeError::BufferTooShort {
                needed: 8,
                available: data.len(),
            }
            .into());
        }
        let bytes: [u8; 8] = data[..8].try_into().unwrap();
        Ok(f64::from_be_bytes(bytes))
    }

    /// Decode raw f32
    fn decode_raw32(&self, data: &[u8]) -> Result<f64> {
        if data.len() < 4 {
            return Err(DecodeError::BufferTooShort {
                needed: 4,
                available: data.len(),
            }
            .into());
        }
        let bytes: [u8; 4] = data[..4].try_into().unwrap();
        Ok(f32::from_be_bytes(bytes) as f64)
    }

    /// Decode delta8
    fn decode_delta8(&self, data: &[u8], source_id: u32, context: &Context) -> Result<f64> {
        if data.is_empty() {
            return Err(DecodeError::BufferTooShort {
                needed: 1,
                available: 0,
            }
            .into());
        }

        let prediction =
            context
                .predict(source_id)
                .ok_or_else(|| DecodeError::MalformedMessage {
                    offset: 0,
                    reason: "No prediction available for delta decoding".to_string(),
                })?;

        let delta = data[0] as i8;
        let scale = context.scale_factor() as f64;
        let decoded = prediction.value + (delta as f64 / scale);

        Ok(decoded)
    }

    /// Decode delta16
    fn decode_delta16(&self, data: &[u8], source_id: u32, context: &Context) -> Result<f64> {
        if data.len() < 2 {
            return Err(DecodeError::BufferTooShort {
                needed: 2,
                available: data.len(),
            }
            .into());
        }

        let prediction =
            context
                .predict(source_id)
                .ok_or_else(|| DecodeError::MalformedMessage {
                    offset: 0,
                    reason: "No prediction available for delta decoding".to_string(),
                })?;

        let delta = i16::from_be_bytes([data[0], data[1]]);
        let scale = context.scale_factor() as f64;
        let decoded = prediction.value + (delta as f64 / scale);

        Ok(decoded)
    }

    /// Decode delta32
    fn decode_delta32(&self, data: &[u8], source_id: u32, context: &Context) -> Result<f64> {
        if data.len() < 4 {
            return Err(DecodeError::BufferTooShort {
                needed: 4,
                available: data.len(),
            }
            .into());
        }

        let prediction =
            context
                .predict(source_id)
                .ok_or_else(|| DecodeError::MalformedMessage {
                    offset: 0,
                    reason: "No prediction available for delta decoding".to_string(),
                })?;

        let delta = i32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let scale = context.scale_factor() as f64;
        let decoded = prediction.value + (delta as f64 / scale);

        Ok(decoded)
    }

    /// Decode repeated (same as last value)
    fn decode_repeated(&self, source_id: u32, context: &Context) -> Result<f64> {
        context.last_value(source_id).ok_or_else(|| {
            DecodeError::MalformedMessage {
                offset: 0,
                reason: "No previous value for repeated decoding".to_string(),
            }
            .into()
        })
    }

    /// Decode interpolated (exact prediction)
    fn decode_interpolated(&self, source_id: u32, context: &Context) -> Result<f64> {
        let prediction =
            context
                .predict(source_id)
                .ok_or_else(|| DecodeError::MalformedMessage {
                    offset: 0,
                    reason: "No prediction available for interpolated decoding".to_string(),
                })?;
        Ok(prediction.value)
    }

    /// Decode pattern reference
    fn decode_pattern(&self, data: &[u8], context: &Context) -> Result<f64> {
        let (pattern_id, _) = self.decode_varint(data)?;

        let pattern = context
            .get_pattern(pattern_id)
            .ok_or(DecodeError::UnknownPattern { pattern_id })?;

        pattern.value.ok_or_else(|| {
            DecodeError::MalformedMessage {
                offset: 0,
                reason: "Pattern has no numeric value".to_string(),
            }
            .into()
        })
    }

    /// Decode pattern with delta adjustment
    fn decode_pattern_delta(&self, data: &[u8], context: &Context) -> Result<f64> {
        let (pattern_id, offset) = self.decode_varint(data)?;

        if offset >= data.len() {
            return Err(DecodeError::BufferTooShort {
                needed: offset + 1,
                available: data.len(),
            }
            .into());
        }

        let pattern = context
            .get_pattern(pattern_id)
            .ok_or(DecodeError::UnknownPattern { pattern_id })?;

        let base_value = pattern.value.ok_or_else(|| DecodeError::MalformedMessage {
            offset: 0,
            reason: "Pattern has no numeric value".to_string(),
        })?;

        let delta = data[offset] as i8;
        let scale = context.scale_factor() as f64;

        Ok(base_value + (delta as f64 / scale))
    }

    /// Decode multi-value message
    pub fn decode_multi(
        &mut self,
        message: &EncodedMessage,
        _context: &Context,
    ) -> Result<Vec<(u16, f64)>> {
        let payload = &message.payload;

        // Source ID
        let (_source_id, mut offset) = self.decode_varint(payload)?;

        // Encoding type (should be Multi)
        if offset >= payload.len() {
            return Err(DecodeError::BufferTooShort {
                needed: offset + 1,
                available: payload.len(),
            }
            .into());
        }

        let encoding = payload[offset];
        offset += 1;

        if encoding != EncodingType::Multi as u8 {
            return Err(DecodeError::MalformedMessage {
                offset: offset - 1,
                reason: "Expected Multi encoding".to_string(),
            }
            .into());
        }

        // Count
        if offset >= payload.len() {
            return Err(DecodeError::BufferTooShort {
                needed: offset + 1,
                available: payload.len(),
            }
            .into());
        }

        let count = payload[offset] as usize;
        offset += 1;

        let mut values = Vec::with_capacity(count);

        for _ in 0..count {
            // Name ID (2 bytes)
            if offset + 2 > payload.len() {
                return Err(DecodeError::BufferTooShort {
                    needed: offset + 2,
                    available: payload.len(),
                }
                .into());
            }
            let name_id = u16::from_be_bytes([payload[offset], payload[offset + 1]]);
            offset += 2;

            // Value encoding type
            if offset >= payload.len() {
                return Err(DecodeError::BufferTooShort {
                    needed: offset + 1,
                    available: payload.len(),
                }
                .into());
            }
            let value_encoding = payload[offset];
            offset += 1;

            // Value (assuming Raw32 for now)
            if value_encoding == EncodingType::Raw32 as u8 {
                if offset + 4 > payload.len() {
                    return Err(DecodeError::BufferTooShort {
                        needed: offset + 4,
                        available: payload.len(),
                    }
                    .into());
                }
                let bytes: [u8; 4] = payload[offset..offset + 4].try_into().unwrap();
                let value = f32::from_be_bytes(bytes) as f64;
                offset += 4;
                values.push((name_id, value));
            } else {
                return Err(DecodeError::UnknownEncodingType(value_encoding).into());
            }
        }

        Ok(values)
    }

    /// Reset decoder state
    pub fn reset(&mut self) {
        self.last_sequence = None;
    }

    /// Get last decoded sequence number
    pub fn last_sequence(&self) -> Option<u32> {
        self.last_sequence
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classifier::Classifier;
    use crate::encoder::Encoder;
    use crate::protocol::{MessageHeader, RawData};

    #[test]
    fn test_roundtrip_raw() {
        let mut encoder = Encoder::new();
        let mut decoder = Decoder::new();
        let classifier = Classifier::default();
        let context = Context::new();

        let original = RawData::new(42.5, 12345);
        let classification = classifier.classify(&original, &context);
        let message = encoder.encode(&original, &classification, &context);
        let decoded = decoder.decode(&message, &context).unwrap();

        assert!((decoded.value - original.value).abs() < 0.001);
        assert_eq!(decoded.source_id, original.source_id);
    }

    #[test]
    fn test_roundtrip_delta() {
        let mut encoder = Encoder::new();
        let mut decoder = Decoder::new();
        let classifier = Classifier::default();
        let mut ctx_encoder = Context::new();
        let mut ctx_decoder = Context::new();

        // Build context on both sides
        for i in 0..10 {
            let data = RawData::new(20.0 + i as f64 * 0.1, i as u64);
            ctx_encoder.observe(&data);
            ctx_decoder.observe(&data);
        }

        // Encode value close to prediction
        let original = RawData::new(21.05, 100);
        let classification = classifier.classify(&original, &ctx_encoder);
        let message = encoder.encode(&original, &classification, &ctx_encoder);

        // Verify delta encoding was used
        assert!(matches!(
            message.encoding_type(),
            Some(EncodingType::Delta8) | Some(EncodingType::Delta16)
        ));

        let decoded = decoder.decode(&message, &ctx_decoder).unwrap();
        assert!((decoded.value - original.value).abs() < 0.01);
    }

    #[test]
    fn test_roundtrip_repeated() {
        let mut encoder = Encoder::new();
        let mut decoder = Decoder::new();
        let classifier = Classifier::default();
        let mut ctx_encoder = Context::new();
        let mut ctx_decoder = Context::new();

        // Observe same value
        let data = RawData::new(42.0, 0);
        ctx_encoder.observe(&data);
        ctx_decoder.observe(&data);

        // Encode same value again
        let original = RawData::new(42.0, 1);
        let classification = classifier.classify(&original, &ctx_encoder);
        let message = encoder.encode(&original, &classification, &ctx_encoder);

        assert_eq!(message.encoding_type(), Some(EncodingType::Repeated));

        let decoded = decoder.decode(&message, &ctx_decoder).unwrap();
        assert!((decoded.value - original.value).abs() < 0.001);
    }

    #[test]
    fn test_roundtrip_multi() {
        let mut encoder = Encoder::new();
        let mut decoder = Decoder::new();
        let context = Context::new();

        let values = vec![(1, 22.5), (2, 65.0), (3, 1013.25)];

        let message = encoder.encode_multi(
            &values,
            42,
            12345,
            crate::protocol::Priority::P3Normal,
            &context,
        );

        let decoded = decoder.decode_multi(&message, &context).unwrap();

        assert_eq!(decoded.len(), values.len());
        for ((orig_id, orig_val), (dec_id, dec_val)) in values.iter().zip(decoded.iter()) {
            assert_eq!(orig_id, dec_id);
            assert!((orig_val - dec_val).abs() < 0.01);
        }
    }

    #[test]
    fn test_varint_roundtrip() {
        let decoder = Decoder::new();

        // Test various values
        let test_values = vec![0, 1, 127, 128, 255, 256, 16383, 16384, 100000];

        for value in test_values {
            let mut buffer = Vec::new();
            // Encode
            let mut v = value;
            while v >= 0x80 {
                buffer.push((v as u8 & 0x7F) | 0x80);
                v >>= 7;
            }
            buffer.push(v as u8);

            // Decode
            let (decoded, _) = decoder.decode_varint(&buffer).unwrap();
            assert_eq!(decoded, value);
        }
    }

    #[test]
    fn test_decode_invalid_encoding() {
        let mut decoder = Decoder::new();
        let context = Context::new();

        // Create a message with invalid encoding type
        let message = EncodedMessage::new(
            MessageHeader::default(),
            vec![0x00, 0xFF], // source_id=0, invalid encoding=0xFF
        );

        let result = decoder.decode(&message, &context);
        assert!(result.is_err());
    }

    #[test]
    fn test_sequence_tracking() {
        let mut decoder = Decoder::new();
        let context = Context::new();
        let mut encoder = Encoder::new();
        let classifier = Classifier::default();

        let data = RawData::new(42.0, 0);
        let classification = classifier.classify(&data, &context);

        // Decode first message
        let msg1 = encoder.encode(&data, &classification, &context);
        decoder.decode(&msg1, &context).unwrap();
        assert_eq!(decoder.last_sequence(), Some(0));

        // Decode second message
        let msg2 = encoder.encode(&data, &classification, &context);
        decoder.decode(&msg2, &context).unwrap();
        assert_eq!(decoder.last_sequence(), Some(1));
    }
}
