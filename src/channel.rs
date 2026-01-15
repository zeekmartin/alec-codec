// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.


//! Channel abstraction module
//!
//! This module provides traits and implementations for communication channels
//! between emitters and receivers.

use crate::error::{ChannelError, Result};
use crate::protocol::EncodedMessage;
use std::collections::VecDeque;
use std::time::Duration;

/// Statistics about channel usage
#[derive(Debug, Clone, Default)]
pub struct ChannelMetrics {
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Total messages sent
    pub messages_sent: u64,
    /// Total messages received
    pub messages_received: u64,
    /// Average latency in milliseconds
    pub latency_avg_ms: f32,
    /// Error rate (0.0-1.0)
    pub error_rate: f32,
    /// Estimated available bandwidth in bytes/sec
    pub bandwidth_available: u32,
}

/// Trait for communication channels
pub trait Channel {
    /// Send a message through the channel
    fn send(&mut self, message: EncodedMessage) -> Result<()>;

    /// Receive a message (blocking with timeout)
    fn receive(&mut self, timeout: Duration) -> Result<EncodedMessage>;

    /// Check if the channel is available
    fn is_available(&self) -> bool;

    /// Get channel metrics
    fn metrics(&self) -> ChannelMetrics;

    /// Close the channel
    fn close(&mut self);
}

/// A simple in-memory channel for testing and local communication
#[derive(Debug)]
pub struct MemoryChannel {
    /// Outgoing messages (send buffer)
    tx_buffer: VecDeque<EncodedMessage>,
    /// Incoming messages (receive buffer)
    rx_buffer: VecDeque<EncodedMessage>,
    /// Maximum buffer size
    max_buffer_size: usize,
    /// Whether the channel is open
    is_open: bool,
    /// Metrics
    metrics: ChannelMetrics,
}

impl MemoryChannel {
    /// Create a new memory channel
    pub fn new() -> Self {
        Self {
            tx_buffer: VecDeque::new(),
            rx_buffer: VecDeque::new(),
            max_buffer_size: 1000,
            is_open: true,
            metrics: ChannelMetrics::default(),
        }
    }

    /// Create with custom buffer size
    pub fn with_buffer_size(max_size: usize) -> Self {
        Self {
            tx_buffer: VecDeque::with_capacity(max_size),
            rx_buffer: VecDeque::with_capacity(max_size),
            max_buffer_size: max_size,
            is_open: true,
            metrics: ChannelMetrics::default(),
        }
    }

    /// Push a message to the receive buffer (simulate receiving)
    pub fn push_incoming(&mut self, message: EncodedMessage) {
        self.rx_buffer.push_back(message);
    }

    /// Pop a message from the send buffer (simulate transmission)
    pub fn pop_outgoing(&mut self) -> Option<EncodedMessage> {
        self.tx_buffer.pop_front()
    }

    /// Get number of pending outgoing messages
    pub fn pending_outgoing(&self) -> usize {
        self.tx_buffer.len()
    }

    /// Get number of pending incoming messages
    pub fn pending_incoming(&self) -> usize {
        self.rx_buffer.len()
    }

    /// Transfer all messages from this channel's TX to another channel's RX
    pub fn transfer_to(&mut self, other: &mut MemoryChannel) {
        while let Some(msg) = self.tx_buffer.pop_front() {
            other.rx_buffer.push_back(msg);
        }
    }
}

impl Default for MemoryChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl Channel for MemoryChannel {
    fn send(&mut self, message: EncodedMessage) -> Result<()> {
        if !self.is_open {
            return Err(ChannelError::Disconnected {
                reason: "Channel is closed".to_string(),
            }
            .into());
        }

        if self.tx_buffer.len() >= self.max_buffer_size {
            return Err(ChannelError::BufferFull.into());
        }

        let msg_size = message.len();
        self.tx_buffer.push_back(message);

        self.metrics.bytes_sent += msg_size as u64;
        self.metrics.messages_sent += 1;

        Ok(())
    }

    fn receive(&mut self, _timeout: Duration) -> Result<EncodedMessage> {
        if !self.is_open {
            return Err(ChannelError::Disconnected {
                reason: "Channel is closed".to_string(),
            }
            .into());
        }

        match self.rx_buffer.pop_front() {
            Some(msg) => {
                self.metrics.bytes_received += msg.len() as u64;
                self.metrics.messages_received += 1;
                Ok(msg)
            }
            None => Err(ChannelError::Timeout { timeout_ms: 0 }.into()),
        }
    }

    fn is_available(&self) -> bool {
        self.is_open
    }

    fn metrics(&self) -> ChannelMetrics {
        self.metrics.clone()
    }

    fn close(&mut self) {
        self.is_open = false;
    }
}

/// A channel pair for bidirectional communication
#[derive(Debug)]
pub struct ChannelPair {
    /// Emitter to Receiver channel
    pub emitter_to_receiver: MemoryChannel,
    /// Receiver to Emitter channel
    pub receiver_to_emitter: MemoryChannel,
}

impl ChannelPair {
    /// Create a new channel pair
    pub fn new() -> Self {
        Self {
            emitter_to_receiver: MemoryChannel::new(),
            receiver_to_emitter: MemoryChannel::new(),
        }
    }

    /// Simulate network transfer (move messages between channels)
    pub fn transfer(&mut self) {
        // Move E→R messages
        while let Some(msg) = self.emitter_to_receiver.pop_outgoing() {
            self.emitter_to_receiver.push_incoming(msg);
        }

        // Move R→E messages
        while let Some(msg) = self.receiver_to_emitter.pop_outgoing() {
            self.receiver_to_emitter.push_incoming(msg);
        }
    }
}

impl Default for ChannelPair {
    fn default() -> Self {
        Self::new()
    }
}

/// Lossy channel that simulates packet loss
#[derive(Debug)]
pub struct LossyChannel {
    inner: MemoryChannel,
    loss_rate: f32,
    rng_state: u64,
}

impl LossyChannel {
    /// Create a new lossy channel with given loss rate (0.0-1.0)
    pub fn new(loss_rate: f32) -> Self {
        Self {
            inner: MemoryChannel::new(),
            loss_rate: loss_rate.clamp(0.0, 1.0),
            rng_state: 12345,
        }
    }

    /// Simple PRNG for deterministic testing
    fn next_random(&mut self) -> f32 {
        self.rng_state = self.rng_state.wrapping_mul(1103515245).wrapping_add(12345);
        ((self.rng_state >> 16) & 0x7fff) as f32 / 32767.0
    }
}

impl Channel for LossyChannel {
    fn send(&mut self, message: EncodedMessage) -> Result<()> {
        // Simulate packet loss
        if self.next_random() < self.loss_rate {
            // Message "lost" - still count it in metrics
            self.inner.metrics.bytes_sent += message.len() as u64;
            self.inner.metrics.messages_sent += 1;
            self.inner.metrics.error_rate = self.loss_rate;
            return Ok(()); // Silently drop
        }
        self.inner.send(message)
    }

    fn receive(&mut self, timeout: Duration) -> Result<EncodedMessage> {
        self.inner.receive(timeout)
    }

    fn is_available(&self) -> bool {
        self.inner.is_available()
    }

    fn metrics(&self) -> ChannelMetrics {
        self.inner.metrics()
    }

    fn close(&mut self) {
        self.inner.close()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{MessageHeader, MessageType, Priority};

    fn make_test_message(seq: u32) -> EncodedMessage {
        EncodedMessage::new(
            MessageHeader {
                version: 1,
                message_type: MessageType::Data,
                priority: Priority::P3Normal,
                sequence: seq,
                timestamp: 0,
                context_version: 0,
            },
            vec![0x00, 0x00, 0x42],
        )
    }

    #[test]
    fn test_memory_channel_send_receive() {
        let mut channel = MemoryChannel::new();

        let msg = make_test_message(1);
        channel.send(msg.clone()).unwrap();

        // Message is in TX buffer, not RX
        assert_eq!(channel.pending_outgoing(), 1);
        assert_eq!(channel.pending_incoming(), 0);

        // Simulate transfer
        let outgoing = channel.pop_outgoing().unwrap();
        channel.push_incoming(outgoing);

        // Now message is in RX
        assert_eq!(channel.pending_outgoing(), 0);
        assert_eq!(channel.pending_incoming(), 1);

        // Receive
        let received = channel.receive(Duration::from_secs(1)).unwrap();
        assert_eq!(received.header.sequence, 1);
    }

    #[test]
    fn test_memory_channel_buffer_full() {
        let mut channel = MemoryChannel::with_buffer_size(2);

        channel.send(make_test_message(1)).unwrap();
        channel.send(make_test_message(2)).unwrap();

        // Third message should fail
        let result = channel.send(make_test_message(3));
        assert!(result.is_err());
    }

    #[test]
    fn test_memory_channel_closed() {
        let mut channel = MemoryChannel::new();
        channel.close();

        let result = channel.send(make_test_message(1));
        assert!(result.is_err());
    }

    #[test]
    fn test_channel_pair() {
        let mut pair = ChannelPair::new();

        // Send from emitter side
        pair.emitter_to_receiver.send(make_test_message(1)).unwrap();
        pair.emitter_to_receiver.send(make_test_message(2)).unwrap();

        // Transfer
        while let Some(msg) = pair.emitter_to_receiver.pop_outgoing() {
            pair.emitter_to_receiver.push_incoming(msg);
        }

        // Receive on emitter side (simulating what receiver would see)
        let msg1 = pair
            .emitter_to_receiver
            .receive(Duration::from_secs(1))
            .unwrap();
        let msg2 = pair
            .emitter_to_receiver
            .receive(Duration::from_secs(1))
            .unwrap();

        assert_eq!(msg1.header.sequence, 1);
        assert_eq!(msg2.header.sequence, 2);
    }

    #[test]
    fn test_channel_metrics() {
        let mut channel = MemoryChannel::new();

        let msg = make_test_message(1);
        let msg_size = msg.len();
        channel.send(msg).unwrap();

        let metrics = channel.metrics();
        assert_eq!(metrics.bytes_sent, msg_size as u64);
        assert_eq!(metrics.messages_sent, 1);
    }

    #[test]
    fn test_lossy_channel() {
        let mut channel = LossyChannel::new(0.5); // 50% loss rate

        // Send many messages
        let mut sent = 0;
        for i in 0..100 {
            channel.send(make_test_message(i)).unwrap();
            sent += 1;
        }

        // Count received
        let mut received = 0;
        while channel.inner.pending_outgoing() > 0 {
            let msg = channel.inner.pop_outgoing().unwrap();
            channel.inner.push_incoming(msg);
            received += 1;
        }

        // Should have lost some messages
        assert!(received < sent);
        // But not all
        assert!(received > 0);
    }
}
