// ALEC Gateway - Integration Tests
//
// This file contains comprehensive integration tests for the ALEC Gateway crate.
// The tests are organized into categories:
// 1. Channel Management (10 tests)
// 2. Push/Flush (10 tests)
// 3. Frame (10 tests)
// 4. Gateway (10+ tests)

use alec_gateway::{
    Aggregator, Channel, ChannelConfig, ChannelData, ChannelManager, Frame, FrameBuilder,
    FrameParseError, Gateway, GatewayConfig, GatewayError,
};

// ============================================================================
// Channel Management Tests (10 tests)
// ============================================================================

#[test]
fn test_channel_creation_basic() {
    let channel = Channel::new("sensor_1", ChannelConfig::default()).unwrap();
    assert_eq!(channel.id, "sensor_1");
    assert!(channel.is_empty());
    assert_eq!(channel.pending(), 0);
}

#[test]
fn test_channel_creation_with_priority() {
    let config = ChannelConfig::with_priority(5);
    let channel = Channel::new("high_priority", config).unwrap();
    assert_eq!(channel.config.priority, 5);
}

#[test]
fn test_channel_manager_add_remove() {
    let mut manager = ChannelManager::new(10);

    // Add
    manager.add("ch1", ChannelConfig::default()).unwrap();
    assert!(manager.contains("ch1"));
    assert_eq!(manager.count(), 1);

    // Remove
    let removed = manager.remove("ch1").unwrap();
    assert_eq!(removed.id, "ch1");
    assert!(!manager.contains("ch1"));
    assert_eq!(manager.count(), 0);
}

#[test]
fn test_channel_manager_duplicate_channel_error() {
    let mut manager = ChannelManager::new(10);
    manager.add("ch1", ChannelConfig::default()).unwrap();

    let result = manager.add("ch1", ChannelConfig::default());
    assert!(matches!(result, Err(GatewayError::ChannelAlreadyExists(_))));
}

#[test]
fn test_channel_manager_max_channels_limit() {
    let mut manager = ChannelManager::new(3);
    manager.add("ch1", ChannelConfig::default()).unwrap();
    manager.add("ch2", ChannelConfig::default()).unwrap();
    manager.add("ch3", ChannelConfig::default()).unwrap();

    let result = manager.add("ch4", ChannelConfig::default());
    assert!(matches!(
        result,
        Err(GatewayError::MaxChannelsReached { max: 3 })
    ));
}

#[test]
fn test_channel_manager_channel_not_found_error() {
    let manager = ChannelManager::new(10);
    let result = manager.get("nonexistent");
    assert!(matches!(result, Err(GatewayError::ChannelNotFound(_))));
}

#[test]
fn test_channel_manager_list_channels() {
    let mut manager = ChannelManager::new(10);
    manager.add("alpha", ChannelConfig::default()).unwrap();
    manager.add("beta", ChannelConfig::default()).unwrap();
    manager.add("gamma", ChannelConfig::default()).unwrap();

    let ids: Vec<_> = manager.list().collect();
    assert_eq!(ids.len(), 3);
    assert!(ids.contains(&&"alpha".to_string()));
    assert!(ids.contains(&&"beta".to_string()));
    assert!(ids.contains(&&"gamma".to_string()));
}

#[test]
fn test_channel_manager_iter() {
    let mut manager = ChannelManager::new(10);
    manager.add("ch1", ChannelConfig::default()).unwrap();
    manager.add("ch2", ChannelConfig::default()).unwrap();

    let count = manager.iter().count();
    assert_eq!(count, 2);
}

#[test]
fn test_channel_manager_is_empty() {
    let mut manager = ChannelManager::new(10);
    assert!(manager.is_empty());

    manager.add("ch1", ChannelConfig::default()).unwrap();
    assert!(!manager.is_empty());
}

#[test]
fn test_channel_manager_total_pending() {
    let mut manager = ChannelManager::new(10);
    manager.add("ch1", ChannelConfig::default()).unwrap();
    manager.add("ch2", ChannelConfig::default()).unwrap();

    manager.get_mut("ch1").unwrap().push(1.0, 1000).unwrap();
    manager.get_mut("ch1").unwrap().push(2.0, 2000).unwrap();
    manager.get_mut("ch2").unwrap().push(3.0, 3000).unwrap();

    assert_eq!(manager.total_pending(), 3);
}

// ============================================================================
// Push/Flush Tests (10 tests)
// ============================================================================

#[test]
fn test_channel_push_single_value() {
    let mut channel = Channel::new("test", ChannelConfig::default()).unwrap();
    channel.push(22.5, 1000).unwrap();
    assert_eq!(channel.pending(), 1);
}

#[test]
fn test_channel_push_multiple_values() {
    let mut channel = Channel::new("test", ChannelConfig::default()).unwrap();
    for i in 0..10 {
        channel
            .push(20.0 + i as f64 * 0.1, i as u64 * 1000)
            .unwrap();
    }
    assert_eq!(channel.pending(), 10);
}

#[test]
fn test_channel_buffer_full_error() {
    let config = ChannelConfig::with_buffer_size(3);
    let mut channel = Channel::new("test", config).unwrap();

    channel.push(1.0, 1000).unwrap();
    channel.push(2.0, 2000).unwrap();
    channel.push(3.0, 3000).unwrap();

    let result = channel.push(4.0, 4000);
    assert!(matches!(result, Err(GatewayError::BufferFull(_))));
}

#[test]
fn test_channel_flush_empty() {
    let mut channel = Channel::new("test", ChannelConfig::default()).unwrap();
    let data = channel.flush().unwrap();
    assert!(data.is_empty());
}

#[test]
fn test_channel_flush_with_data() {
    let mut channel = Channel::new("test", ChannelConfig::default()).unwrap();
    channel.push(22.5, 1000).unwrap();
    channel.push(22.6, 2000).unwrap();

    let data = channel.flush().unwrap();
    assert!(!data.is_empty());
    assert_eq!(channel.pending(), 0);
}

#[test]
fn test_channel_flush_clears_buffer() {
    let mut channel = Channel::new("test", ChannelConfig::default()).unwrap();
    channel.push(22.5, 1000).unwrap();

    assert_eq!(channel.pending(), 1);
    channel.flush().unwrap();
    assert_eq!(channel.pending(), 0);
}

#[test]
fn test_channel_clear_buffer() {
    let mut channel = Channel::new("test", ChannelConfig::default()).unwrap();
    channel.push(22.5, 1000).unwrap();
    channel.push(22.6, 2000).unwrap();

    channel.clear_buffer();
    assert_eq!(channel.pending(), 0);
}

#[test]
fn test_channel_context_version_updates() {
    let mut channel = Channel::new("test", ChannelConfig::default()).unwrap();
    let initial_version = channel.context_version();

    channel.push(22.5, 1000).unwrap();
    channel.flush().unwrap();

    // Context version should increase after observing values
    assert!(channel.context_version() >= initial_version);
}

#[test]
fn test_channel_reset_sequence() {
    let mut channel = Channel::new("test", ChannelConfig::default()).unwrap();
    channel.push(22.5, 1000).unwrap();
    channel.flush().unwrap();

    channel.reset_sequence();
    // Should not panic
}

#[test]
fn test_channel_flush_produces_valid_alec_data() {
    let mut channel = Channel::new("test", ChannelConfig::default()).unwrap();

    // Push multiple values to build context
    for i in 0..5 {
        channel.push(20.0 + i as f64, i as u64 * 1000).unwrap();
    }

    let data = channel.flush().unwrap();

    // Data should be non-empty and reasonable size
    assert!(!data.is_empty());
    assert!(data.len() < 1000); // Should be compressed
}

// ============================================================================
// Frame Tests (10 tests)
// ============================================================================

#[test]
fn test_frame_new() {
    let frame = Frame::new();
    assert_eq!(frame.version, Frame::VERSION);
    assert!(frame.is_empty());
    assert_eq!(frame.channel_count(), 0);
}

#[test]
fn test_frame_add_channel() {
    let mut frame = Frame::new();
    frame.add_channel("temp".to_string(), vec![1, 2, 3]);

    assert_eq!(frame.channel_count(), 1);
    assert!(!frame.is_empty());
}

#[test]
fn test_frame_serialize_deserialize_roundtrip() {
    let mut frame = Frame::new();
    frame.add_channel("temp".to_string(), vec![1, 2, 3, 4, 5]);
    frame.add_channel("humid".to_string(), vec![10, 20, 30]);
    frame.add_channel("pressure".to_string(), vec![100, 101, 102, 103]);

    let bytes = frame.to_bytes();
    let parsed = Frame::from_bytes(&bytes).unwrap();

    assert_eq!(frame, parsed);
}

#[test]
fn test_frame_size_calculation() {
    let mut frame = Frame::new();
    let initial_size = frame.size();
    assert_eq!(initial_size, 2); // version + count

    frame.add_channel("t".to_string(), vec![1, 2]);
    // 2 (header) + 1 (id_len) + 1 (id "t") + 2 (data_len) + 2 (data) = 8
    assert_eq!(frame.size(), 8);
}

#[test]
fn test_frame_max_size_enforcement() {
    let mut builder = FrameBuilder::new(20);

    // First channel should fit
    assert!(builder.try_add("a".to_string(), vec![1, 2, 3]));

    // Second channel should not fit (would exceed 20 bytes)
    assert!(!builder.try_add("bbbbbbbbbb".to_string(), vec![4, 5, 6, 7, 8, 9, 10]));
}

#[test]
fn test_frame_multi_channel() {
    let mut frame = Frame::new();
    for i in 0..5 {
        frame.add_channel(format!("ch{}", i), vec![i as u8; 10]);
    }

    assert_eq!(frame.channel_count(), 5);

    let bytes = frame.to_bytes();
    let parsed = Frame::from_bytes(&bytes).unwrap();
    assert_eq!(parsed.channel_count(), 5);
}

#[test]
fn test_frame_get_channel() {
    let mut frame = Frame::new();
    frame.add_channel("temp".to_string(), vec![22, 23, 24]);
    frame.add_channel("humid".to_string(), vec![65, 66]);

    let temp = frame.get_channel("temp").unwrap();
    assert_eq!(temp.data, vec![22, 23, 24]);

    let humid = frame.get_channel("humid").unwrap();
    assert_eq!(humid.data, vec![65, 66]);

    assert!(frame.get_channel("nonexistent").is_none());
}

#[test]
fn test_frame_parse_error_too_short() {
    let result = Frame::from_bytes(&[1]);
    assert!(matches!(result, Err(FrameParseError::TooShort)));
}

#[test]
fn test_frame_parse_error_unsupported_version() {
    let result = Frame::from_bytes(&[99, 0]);
    assert!(matches!(
        result,
        Err(FrameParseError::UnsupportedVersion(99))
    ));
}

#[test]
fn test_frame_builder_remaining() {
    let mut builder = FrameBuilder::new(100);
    let initial = builder.remaining();

    builder.try_add("test".to_string(), vec![1, 2, 3]);
    assert!(builder.remaining() < initial);
}

// ============================================================================
// Gateway Tests (12 tests)
// ============================================================================

#[test]
fn test_gateway_new_default() {
    let gateway = Gateway::new();
    assert_eq!(gateway.channel_count(), 0);
    assert_eq!(gateway.total_pending(), 0);
    assert_eq!(gateway.max_frame_size(), 242);
}

#[test]
fn test_gateway_with_custom_config() {
    let config = GatewayConfig {
        max_frame_size: 100,
        max_channels: 5,
        enable_checksums: false,
    };
    let gateway = Gateway::with_config(config);
    assert_eq!(gateway.max_frame_size(), 100);
}

#[test]
fn test_gateway_full_workflow() {
    let mut gateway = Gateway::new();

    // Add channels
    gateway
        .add_channel("temp", ChannelConfig::with_priority(1))
        .unwrap();
    gateway
        .add_channel("humid", ChannelConfig::with_priority(2))
        .unwrap();

    // Push values
    gateway.push("temp", 22.5, 1000).unwrap();
    gateway.push("temp", 22.6, 2000).unwrap();
    gateway.push("humid", 65.0, 1000).unwrap();

    assert_eq!(gateway.total_pending(), 3);

    // Flush
    let frame = gateway.flush().unwrap();
    assert_eq!(frame.channel_count(), 2);
    assert_eq!(gateway.total_pending(), 0);
}

#[test]
fn test_gateway_priority_ordering() {
    let mut gateway = Gateway::new();

    // Add channels with different priorities (lower = higher priority)
    gateway
        .add_channel("low", ChannelConfig::with_priority(200))
        .unwrap();
    gateway
        .add_channel("high", ChannelConfig::with_priority(1))
        .unwrap();
    gateway
        .add_channel("medium", ChannelConfig::with_priority(100))
        .unwrap();

    gateway.push("low", 1.0, 1000).unwrap();
    gateway.push("high", 2.0, 1000).unwrap();
    gateway.push("medium", 3.0, 1000).unwrap();

    let frame = gateway.flush().unwrap();

    // High priority should be first in frame
    assert_eq!(frame.channels[0].id, "high");
    assert_eq!(frame.channels[1].id, "medium");
    assert_eq!(frame.channels[2].id, "low");
}

#[test]
fn test_gateway_push_multi() {
    let mut gateway = Gateway::new();
    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();

    let values = vec![(22.5, 1000), (22.6, 2000), (22.7, 3000)];
    gateway.push_multi("temp", &values).unwrap();

    assert_eq!(gateway.pending("temp").unwrap(), 3);
}

#[test]
fn test_gateway_flush_specific_channels() {
    let mut gateway = Gateway::new();
    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();
    gateway
        .add_channel("humid", ChannelConfig::default())
        .unwrap();
    gateway
        .add_channel("pressure", ChannelConfig::default())
        .unwrap();

    gateway.push("temp", 22.5, 1000).unwrap();
    gateway.push("humid", 65.0, 1000).unwrap();
    gateway.push("pressure", 1013.25, 1000).unwrap();

    // Only flush temp and pressure
    let frame = gateway.flush_channels(&["temp", "pressure"]).unwrap();

    assert_eq!(frame.channel_count(), 2);
    assert!(frame.get_channel("temp").is_some());
    assert!(frame.get_channel("pressure").is_some());
    assert!(frame.get_channel("humid").is_none());

    // Humid should still have pending data
    assert_eq!(gateway.pending("humid").unwrap(), 1);
}

#[test]
fn test_gateway_clear_all() {
    let mut gateway = Gateway::new();
    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();
    gateway
        .add_channel("humid", ChannelConfig::default())
        .unwrap();

    gateway.push("temp", 22.5, 1000).unwrap();
    gateway.push("humid", 65.0, 1000).unwrap();

    assert_eq!(gateway.total_pending(), 2);

    gateway.clear_all();

    assert_eq!(gateway.total_pending(), 0);
}

#[test]
fn test_gateway_has_pending_data() {
    let mut gateway = Gateway::new();
    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();

    assert!(!gateway.has_pending_data());

    gateway.push("temp", 22.5, 1000).unwrap();

    assert!(gateway.has_pending_data());

    gateway.flush().unwrap();

    assert!(!gateway.has_pending_data());
}

#[test]
fn test_gateway_set_max_frame_size() {
    let mut gateway = Gateway::new();
    assert_eq!(gateway.max_frame_size(), 242);

    gateway.set_max_frame_size(100);
    assert_eq!(gateway.max_frame_size(), 100);
}

#[test]
fn test_gateway_channels_list() {
    let mut gateway = Gateway::new();
    gateway
        .add_channel("alpha", ChannelConfig::default())
        .unwrap();
    gateway
        .add_channel("beta", ChannelConfig::default())
        .unwrap();

    let channels = gateway.channels();
    assert_eq!(channels.len(), 2);
    assert!(channels.contains(&"alpha".to_string()));
    assert!(channels.contains(&"beta".to_string()));
}

#[test]
fn test_gateway_has_channel() {
    let mut gateway = Gateway::new();
    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();

    assert!(gateway.has_channel("temp"));
    assert!(!gateway.has_channel("humid"));
}

#[test]
fn test_gateway_remove_channel() {
    let mut gateway = Gateway::new();
    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();
    gateway
        .add_channel("humid", ChannelConfig::default())
        .unwrap();

    gateway.remove_channel("temp").unwrap();

    assert_eq!(gateway.channel_count(), 1);
    assert!(!gateway.has_channel("temp"));
    assert!(gateway.has_channel("humid"));
}

// ============================================================================
// Additional Integration Tests (8+ tests)
// ============================================================================

#[test]
fn test_aggregator_empty_manager() {
    let config = GatewayConfig::default();
    let aggregator = Aggregator::new(config);
    let mut manager = ChannelManager::new(10);

    let frame = aggregator.aggregate(&mut manager).unwrap();
    assert!(frame.is_empty());
}

#[test]
fn test_aggregator_respects_max_size() {
    let mut config = GatewayConfig::default();
    config.max_frame_size = 50;
    let aggregator = Aggregator::new(config);

    let mut manager = ChannelManager::new(20);

    // Add many channels with data
    for i in 0..10 {
        let name = format!("channel_{}", i);
        manager
            .add(&name, ChannelConfig::with_priority(i as u8))
            .unwrap();
        manager
            .get_mut(&name)
            .unwrap()
            .push(i as f64, 1000)
            .unwrap();
    }

    let frame = aggregator.aggregate(&mut manager).unwrap();

    // Frame should respect max size
    assert!(frame.size() <= 50);
}

#[test]
fn test_frame_empty_channel_data() {
    let mut frame = Frame::new();
    frame.add_channel("empty".to_string(), vec![]);

    let bytes = frame.to_bytes();
    let parsed = Frame::from_bytes(&bytes).unwrap();

    assert_eq!(parsed.channel_count(), 1);
    assert!(parsed.channels[0].data.is_empty());
}

#[test]
fn test_gateway_lorawan_config() {
    let config = GatewayConfig::lorawan(0); // DR0
    assert_eq!(config.max_frame_size, 51);

    let config = GatewayConfig::lorawan(4); // DR4
    assert_eq!(config.max_frame_size, 242);
}

#[test]
fn test_channel_config_with_buffer_size() {
    let config = ChannelConfig::with_buffer_size(10);
    let mut channel = Channel::new("test", config).unwrap();

    // Should be able to push 10 values
    for i in 0..10 {
        channel.push(i as f64, i as u64 * 1000).unwrap();
    }

    // 11th should fail
    let result = channel.push(10.0, 10000);
    assert!(matches!(result, Err(GatewayError::BufferFull(_))));
}

#[test]
fn test_frame_large_channel_id() {
    let mut frame = Frame::new();
    let long_id = "a".repeat(200);
    frame.add_channel(long_id.clone(), vec![1, 2, 3]);

    let bytes = frame.to_bytes();
    let parsed = Frame::from_bytes(&bytes).unwrap();

    assert_eq!(parsed.channels[0].id, long_id);
}

#[test]
fn test_gateway_context_version() {
    let mut gateway = Gateway::new();
    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();

    // Context version should be retrievable (starts at 0)
    let version = gateway.channel_context_version("temp").unwrap();
    assert_eq!(version, 0);
}

#[test]
fn test_frame_parse_truncated_data() {
    // Create a valid frame and truncate it
    let mut frame = Frame::new();
    frame.add_channel("test".to_string(), vec![1, 2, 3, 4, 5]);

    let mut bytes = frame.to_bytes();
    bytes.truncate(bytes.len() - 3); // Remove some data

    let result = Frame::from_bytes(&bytes);
    assert!(result.is_err());
}

#[test]
fn test_multiple_flush_cycles() {
    let mut gateway = Gateway::new();
    gateway
        .add_channel("temp", ChannelConfig::default())
        .unwrap();

    // First cycle
    gateway.push("temp", 22.5, 1000).unwrap();
    let frame1 = gateway.flush().unwrap();
    assert_eq!(frame1.channel_count(), 1);

    // Second cycle
    gateway.push("temp", 23.0, 2000).unwrap();
    let frame2 = gateway.flush().unwrap();
    assert_eq!(frame2.channel_count(), 1);

    // Third cycle with no data
    let frame3 = gateway.flush().unwrap();
    assert!(frame3.is_empty());
}

#[test]
fn test_channel_data_struct() {
    let data = ChannelData {
        id: "test".to_string(),
        data: vec![1, 2, 3],
    };
    assert_eq!(data.id, "test");
    assert_eq!(data.data, vec![1, 2, 3]);
}

#[test]
fn test_gateway_error_display() {
    let err = GatewayError::ChannelNotFound("test".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("test"));
    assert!(msg.contains("not found"));
}

#[test]
fn test_frame_builder_is_empty() {
    let mut builder = FrameBuilder::new(100);
    assert!(builder.is_empty());

    builder.try_add("test".to_string(), vec![1, 2, 3]);
    assert!(!builder.is_empty());
}
