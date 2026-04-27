// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Decoder module
//!
//! This module handles decoding binary messages back into data values
//! using the shared context for decompression.

#[cfg(not(feature = "std"))]
use alloc::{string::ToString, vec::Vec};

use crate::context::Context;
use crate::encoder::{fixed_bitmap_bytes, FixedEncoding};
use crate::error::{DecodeError, Result};
use crate::protocol::{
    classify_compact_marker, ctx_version_compatible, CompactHeader, DecodedData, EncodedMessage,
    EncodingType,
};

/// Maximum forward jump of the u16 context_version tolerated by the
/// fixed-channel decoder before flagging a version mismatch. The
/// encoder increments the version once per channel observation, so
/// 256 leaves comfortable slack for 50-ish channels × several frames
/// while still catching large skips.
const FIXED_CTX_MAX_JUMP: u16 = 256;

/// Outcome of a successful `decode_multi_fixed` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedFrameInfo {
    /// True if the frame was a keyframe (marker 0xA2).
    pub keyframe: bool,
    /// Sequence number from the compact header.
    pub sequence: u16,
    /// Low-u16 context version from the compact header.
    pub context_version: u16,
    /// Number of frames missing between the previous decode and this
    /// one, clipped to 255 (0 means contiguous / first decode).
    pub gap_size: u8,
    /// True if the header's context_version looks incompatible with
    /// the locally tracked one (likely desync). Bloc C will act on
    /// this by calling `context.reset_to_baseline()`.
    pub context_mismatch: bool,
}

/// Decoder for ALEC messages
#[derive(Debug, Clone)]
pub struct Decoder {
    /// Whether to verify checksum on incoming messages
    verify_checksum: bool,
    /// Last decoded sequence number (for gap detection)
    last_sequence: Option<u16>,
    /// Sequence observed on the most recent fixed-channel frame.
    last_fixed_sequence: Option<u16>,
    /// Context version observed on the most recent fixed-channel frame.
    last_fixed_ctx_version: Option<u16>,
}

impl Decoder {
    /// Create a new decoder
    pub fn new() -> Self {
        Self {
            verify_checksum: false,
            last_sequence: None,
            last_fixed_sequence: None,
            last_fixed_ctx_version: None,
        }
    }

    /// Create decoder with checksum verification enabled
    pub fn with_checksum_verification() -> Self {
        Self {
            verify_checksum: true,
            last_sequence: None,
            last_fixed_sequence: None,
            last_fixed_ctx_version: None,
        }
    }

    /// Check if checksum verification is enabled
    pub fn checksum_verification_enabled(&self) -> bool {
        self.verify_checksum
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

    /// Decode from raw bytes (with optional checksum verification)
    pub fn decode_bytes(&mut self, bytes: &[u8], context: &Context) -> Result<DecodedData> {
        let message = if self.verify_checksum {
            EncodedMessage::from_bytes_with_checksum(bytes)?
        } else {
            EncodedMessage::from_bytes(bytes).ok_or(DecodeError::InvalidHeader)?
        };
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

    /// Decode multi-value message.
    ///
    /// Handles all per-channel encoding types (Raw64, Raw32, Delta8, Delta16,
    /// Delta32, Repeated, Interpolated). The `name_id` of each channel is used
    /// as its `source_id` (cast to `u32`) for context-dependent decodings.
    pub fn decode_multi(
        &mut self,
        message: &EncodedMessage,
        context: &Context,
    ) -> Result<Vec<(u8, f64)>> {
        let payload = &message.payload;

        // Source ID (frame-level, ignored for per-channel decode)
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
            // Name ID (1 byte)
            if offset >= payload.len() {
                return Err(DecodeError::BufferTooShort {
                    needed: offset + 1,
                    available: payload.len(),
                }
                .into());
            }
            let name_id = payload[offset];
            offset += 1;

            // Value encoding type
            if offset >= payload.len() {
                return Err(DecodeError::BufferTooShort {
                    needed: offset + 1,
                    available: payload.len(),
                }
                .into());
            }
            let value_encoding_byte = payload[offset];
            offset += 1;

            let enc_type = EncodingType::from_u8(value_encoding_byte)
                .ok_or(DecodeError::UnknownEncodingType(value_encoding_byte))?;

            // Use name_id as per-channel source_id (matches encoder convention)
            let ch_source_id = name_id as u32;

            let value = match enc_type {
                EncodingType::Raw64 => {
                    let v = self.decode_raw64(&payload[offset..])?;
                    offset += 8;
                    v
                }
                EncodingType::Raw32 => {
                    let v = self.decode_raw32(&payload[offset..])?;
                    offset += 4;
                    v
                }
                EncodingType::Delta8 => {
                    let v = self.decode_delta8(&payload[offset..], ch_source_id, context)?;
                    offset += 1;
                    v
                }
                EncodingType::Delta16 => {
                    let v = self.decode_delta16(&payload[offset..], ch_source_id, context)?;
                    offset += 2;
                    v
                }
                EncodingType::Delta32 => {
                    let v = self.decode_delta32(&payload[offset..], ch_source_id, context)?;
                    offset += 4;
                    v
                }
                EncodingType::Repeated => {
                    self.decode_repeated(ch_source_id, context)?
                    // 0 extra bytes
                }
                EncodingType::Interpolated => {
                    self.decode_interpolated(ch_source_id, context)?
                    // 0 extra bytes
                }
                _ => {
                    return Err(DecodeError::MalformedMessage {
                        offset,
                        reason: "Unsupported encoding in multi frame".to_string(),
                    }
                    .into());
                }
            };

            values.push((name_id, value));
        }

        Ok(values)
    }

    /// Reset decoder state
    pub fn reset(&mut self) {
        self.last_sequence = None;
        self.last_fixed_sequence = None;
        self.last_fixed_ctx_version = None;
    }

    /// Get last decoded sequence number
    pub fn last_sequence(&self) -> Option<u16> {
        self.last_sequence
    }

    /// Last sequence decoded via `decode_multi_fixed` (separate tracker
    /// from the legacy multi-frame path so the two can coexist in the
    /// same `Decoder`).
    pub fn last_fixed_sequence(&self) -> Option<u16> {
        self.last_fixed_sequence
    }

    /// Manually advance the fixed-path sequence and context-version
    /// trackers as if `decode_multi_fixed` had just run on a frame
    /// carrying the supplied wire-header values.
    ///
    /// Used by the v1.3.10 FFI `alec_decoder_feed_values` path so the
    /// server side can keep its decoder synchronised with the encoder
    /// when the device sent a legacy TLV frame instead of an ALEC
    /// frame (the encoder still advanced its sequence + ctx_version
    /// for the discarded frame; the decoder needs to do the same to
    /// avoid spurious gap-detection on the next real ALEC frame).
    ///
    /// Note: this does NOT touch the per-channel prediction state —
    /// the caller is responsible for `Context::observe`-ing each
    /// value, mirroring the post-decode loop in
    /// `alec_decode_multi_fixed`.
    pub fn record_fixed_frame(&mut self, sequence: u16, context_version: u16) {
        self.last_fixed_sequence = Some(sequence);
        self.last_fixed_ctx_version = Some(context_version);
    }

    // ========================================================================
    // Bloc B — Compact fixed-channel decoder (Milesight EM500-CO2)
    // ========================================================================
    /// Decode a fixed-channel frame produced by `Encoder::encode_multi_fixed`.
    ///
    /// # Arguments
    ///
    /// * `input` - Wire bytes, starting at the marker byte.
    /// * `channel_count` - Number of channels in the frame. Must match
    ///   the value used when encoding (the wire format does NOT
    ///   self-describe the channel count).
    /// * `context` - Shared context (for delta predictions). Not
    ///   mutated — the caller is responsible for `observe()`-ing each
    ///   decoded value.
    /// * `output` - Destination slice; must have length >= `channel_count`.
    ///
    /// # Returns
    ///
    /// A [`FixedFrameInfo`] describing whether the frame was a keyframe,
    /// its sequence number, observed gap and version-mismatch flag.
    /// The decoded values are written positionally to `output[..channel_count]`.
    pub fn decode_multi_fixed(
        &mut self,
        input: &[u8],
        channel_count: usize,
        context: &Context,
        output: &mut [f64],
    ) -> Result<FixedFrameInfo> {
        if channel_count == 0 {
            return Err(DecodeError::MalformedMessage {
                offset: 0,
                reason: "channel_count must be > 0".to_string(),
            }
            .into());
        }
        if output.len() < channel_count {
            return Err(DecodeError::BufferTooShort {
                needed: channel_count,
                available: output.len(),
            }
            .into());
        }

        // 1 marker + 4 header + bitmap + data
        let bitmap_bytes = fixed_bitmap_bytes(channel_count);
        let min_size = 1 + CompactHeader::SIZE + bitmap_bytes;
        if input.len() < min_size {
            return Err(DecodeError::BufferTooShort {
                needed: min_size,
                available: input.len(),
            }
            .into());
        }

        // Marker dispatch.
        let marker = input[0];
        let keyframe = match classify_compact_marker(marker) {
            Some(kf) => kf,
            None => {
                return Err(DecodeError::MalformedMessage {
                    offset: 0,
                    reason: "not a fixed-channel ALEC frame".to_string(),
                }
                .into());
            }
        };

        // Compact header.
        let header = CompactHeader::read(&input[1..1 + CompactHeader::SIZE])?;

        // Gap detection (clamped to u8).
        let gap_size = match self.last_fixed_sequence {
            Some(prev) => {
                let diff = header.sequence.wrapping_sub(prev);
                if diff == 0 {
                    0
                } else {
                    diff.saturating_sub(1).min(u8::MAX as u16) as u8
                }
            }
            None => 0,
        };

        // Context-version mismatch detection (wraparound-aware).
        // Keyframes are self-sufficient: the decoder must accept them
        // even if the version looks wildly off — that is the whole
        // point of a keyframe. So we never flag mismatch on a keyframe.
        let context_mismatch = if keyframe {
            false
        } else {
            match self.last_fixed_ctx_version {
                Some(last) => {
                    !ctx_version_compatible(header.context_version, last, FIXED_CTX_MAX_JUMP)
                }
                None => false,
            }
        };

        // Bitmap spans bitmap_bytes; after that comes the per-channel data.
        let bitmap_start = 1 + CompactHeader::SIZE;
        let data_start = bitmap_start + bitmap_bytes;

        // Parse each channel's encoding from the bitmap, 2 bits LSB-first per byte.
        // Use a stack-allocated scratch array (bounded at 64 channels
        // to mirror the encoder's upper limit).
        if channel_count > 64 {
            return Err(DecodeError::MalformedMessage {
                offset: 0,
                reason: "channel_count > 64 not supported by fixed wire format".to_string(),
            }
            .into());
        }
        let mut encodings: [FixedEncoding; 64] = [FixedEncoding::Repeated; 64];
        for i in 0..channel_count {
            let bits = (input[bitmap_start + i / 4] >> ((i % 4) * 2)) & 0b11;
            encodings[i] = FixedEncoding::from_bits(bits);
        }

        // Confirm data length is exactly what we expect.
        let data_bytes: usize = (0..channel_count).map(|i| encodings[i].byte_size()).sum();
        let expected_total = data_start + data_bytes;
        if input.len() < expected_total {
            return Err(DecodeError::BufferTooShort {
                needed: expected_total,
                available: input.len(),
            }
            .into());
        }

        // Decode each channel positionally.
        let mut cursor = data_start;
        for i in 0..channel_count {
            let source_id = crate::encoder::Encoder::fixed_channel_source_id(i);
            let enc = encodings[i];
            let value = match enc {
                FixedEncoding::Repeated => match context.last_value(source_id) {
                    Some(v) => v,
                    None => {
                        return Err(DecodeError::MalformedMessage {
                            offset: cursor,
                            reason: "Repeated encoding without prior value".to_string(),
                        }
                        .into());
                    }
                },
                FixedEncoding::Delta8 => {
                    let delta_byte = input[cursor] as i8;
                    cursor += 1;
                    self.apply_delta(delta_byte as f64, source_id, context)?
                }
                FixedEncoding::Delta16 => {
                    let b0 = input[cursor];
                    let b1 = input[cursor + 1];
                    cursor += 2;
                    let d = i16::from_be_bytes([b0, b1]);
                    self.apply_delta(d as f64, source_id, context)?
                }
                FixedEncoding::Raw32 => {
                    let bytes = [
                        input[cursor],
                        input[cursor + 1],
                        input[cursor + 2],
                        input[cursor + 3],
                    ];
                    cursor += 4;
                    f32::from_be_bytes(bytes) as f64
                }
            };
            output[i] = value;
        }
        debug_assert_eq!(cursor, expected_total);

        // Update the fixed-path tracking.
        self.last_fixed_sequence = Some(header.sequence);
        self.last_fixed_ctx_version = Some(header.context_version);

        // Recovery policy (wired by the FFI layer, not here, because
        // this function only has `&Context` — it surfaces the flags
        // and the caller applies them). The FFI's `alec_decode_multi_fixed`
        // calls `context.reset_to_baseline()` when
        //     (gap_size > 0 || context_mismatch) && !keyframe
        // so the next keyframe (marker 0xA2, Raw32) can fully re-seed
        // the per-channel prediction state without silent corruption.
        // See alec-ffi/src/lib.rs::alec_decode_multi_fixed.

        Ok(FixedFrameInfo {
            keyframe,
            sequence: header.sequence,
            context_version: header.context_version,
            gap_size,
            context_mismatch,
        })
    }

    /// Apply a scaled integer delta to the prediction for `source_id`.
    ///
    /// Mirrors the inverse of the encoder's `choose_encoding` /
    /// `encode_multi_fixed` logic.
    fn apply_delta(&self, scaled_delta: f64, source_id: u32, context: &Context) -> Result<f64> {
        let prediction = match context.predict(source_id) {
            Some(p) => p,
            None => {
                return Err(DecodeError::MalformedMessage {
                    offset: 0,
                    reason: "Delta encoding without prediction".to_string(),
                }
                .into());
            }
        };
        let scale = context.scale_factor() as f64;
        let delta = scaled_delta / scale;
        Ok(prediction.value + delta)
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

        let values: Vec<(u8, f64)> = vec![(1, 22.5), (2, 65.0), (3, 1013.25)];

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

    #[test]
    fn test_checksum_encode_decode_roundtrip() {
        let mut encoder = Encoder::with_checksum();
        let mut decoder = Decoder::with_checksum_verification();
        let classifier = Classifier::default();
        let context = Context::new();

        let data = RawData::new(42.5, 12345);
        let classification = classifier.classify(&data, &context);
        let bytes = encoder.encode_to_bytes(&data, &classification, &context);

        let decoded = decoder.decode_bytes(&bytes, &context).unwrap();
        assert!((decoded.value - data.value).abs() < 0.001);
        assert_eq!(decoded.source_id, data.source_id);
    }

    #[test]
    fn test_checksum_corruption_decode_fails() {
        use crate::error::AlecError;

        let mut encoder = Encoder::with_checksum();
        let mut decoder = Decoder::with_checksum_verification();
        let classifier = Classifier::default();
        let context = Context::new();

        let data = RawData::new(42.5, 12345);
        let classification = classifier.classify(&data, &context);
        let mut bytes = encoder.encode_to_bytes(&data, &classification, &context);

        // Corrupt a byte in the middle
        bytes[5] ^= 0xFF;

        let result = decoder.decode_bytes(&bytes, &context);
        assert!(matches!(
            result,
            Err(AlecError::Decode(
                crate::error::DecodeError::InvalidChecksum { .. }
            ))
        ));
    }

    #[test]
    fn test_no_checksum_still_works() {
        let mut encoder = Encoder::new();
        let mut decoder = Decoder::new();
        let classifier = Classifier::default();
        let context = Context::new();

        let data = RawData::new(123.456, 999);
        let classification = classifier.classify(&data, &context);
        let bytes = encoder.encode_to_bytes(&data, &classification, &context);

        let decoded = decoder.decode_bytes(&bytes, &context).unwrap();
        assert!((decoded.value - data.value).abs() < 0.001);
    }
}
