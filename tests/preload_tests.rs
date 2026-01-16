// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Integration tests for ALEC preload file system
//!
//! These tests verify the save/load roundtrip functionality
//! and version synchronization for preload files.

use alec::context::{Context, ContextConfig, EvolutionConfig, Pattern, VersionCheckResult};
use alec::protocol::RawData;
use tempfile::tempdir;

/// Helper to create a trained context with patterns (evolution disabled)
fn create_trained_context() -> Context {
    // Disable evolution to prevent pattern pruning during test
    let config = ContextConfig {
        evolution: EvolutionConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut ctx = Context::with_config(config);

    // Register some patterns
    ctx.register_pattern(Pattern::new(vec![0x00, 0x01, 0x02]))
        .unwrap();
    ctx.register_pattern(Pattern::new(vec![0x10, 0x20, 0x30, 0x40]))
        .unwrap();
    ctx.register_pattern(Pattern::new(vec![0xFF])).unwrap();

    // Train with some observations
    for i in 0..100 {
        let value = 20.0 + (i as f64 * 0.1);
        ctx.observe(&RawData::new(value, i as u64));
    }

    ctx
}

#[test]
fn test_save_load_roundtrip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.alec-context");

    // Create and save a trained context
    let original_ctx = create_trained_context();
    let original_pattern_count = original_ctx.pattern_count();
    let original_version = original_ctx.context_version();

    original_ctx
        .save_to_file(&path, "temperature")
        .expect("Failed to save context");

    // Verify file was created
    assert!(path.exists(), "Preload file should exist");

    // Load the context back
    let loaded_ctx = Context::load_from_file(&path).expect("Failed to load context");

    // Verify the loaded context matches
    assert_eq!(
        loaded_ctx.pattern_count(),
        original_pattern_count,
        "Pattern count should match"
    );
    assert_eq!(
        loaded_ctx.context_version(),
        original_version,
        "Version should match"
    );

    // Verify patterns can be found
    let code = loaded_ctx.find_pattern(&[0x00, 0x01, 0x02]);
    assert!(code.is_some(), "Pattern should be found in loaded context");
}

#[test]
fn test_version_match_detection() {
    let ctx = create_trained_context();
    let version = ctx.context_version();

    // Test matching version
    let result = ctx.check_version(version);
    assert!(result.is_match(), "Same version should match");
    assert_eq!(result, VersionCheckResult::Match);

    // Test mismatching version
    let result = ctx.check_version(version + 1);
    assert!(!result.is_match(), "Different version should not match");

    match result {
        VersionCheckResult::Mismatch { expected, actual } => {
            assert_eq!(expected, version);
            assert_eq!(actual, version + 1);
        }
        _ => panic!("Expected mismatch result"),
    }
}

#[test]
fn test_version_mismatch_detection() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.alec-context");

    // Create and save context with version 0
    let mut ctx1 = Context::new();
    ctx1.register_pattern(Pattern::new(vec![0x42])).unwrap();
    ctx1.save_to_file(&path, "test").unwrap();

    // Load and modify the context (incrementing version)
    let mut ctx2 = Context::load_from_file(&path).unwrap();
    ctx2.observe(&RawData::new(42.0, 0));
    let new_version = ctx2.context_version();

    // Original version from file should differ from modified context
    let original = Context::load_from_file(&path).unwrap();
    assert_ne!(
        original.context_version(),
        new_version,
        "Versions should differ after modification"
    );
}

#[test]
fn test_corrupt_file_detection() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("corrupt.alec-context");

    // Create a valid preload file
    let ctx = create_trained_context();
    ctx.save_to_file(&path, "test").unwrap();

    // Read the file and corrupt it
    let mut data = std::fs::read(&path).unwrap();
    if data.len() > 70 {
        // Corrupt some data in the dictionary section
        data[70] ^= 0xFF;
    }
    std::fs::write(&path, &data).unwrap();

    // Try to load - should fail with checksum error
    let result = Context::load_from_file(&path);
    assert!(result.is_err(), "Corrupt file should fail to load");
}

#[test]
fn test_invalid_magic_bytes() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("invalid.alec-context");

    // Write a file with invalid magic bytes
    let mut data = vec![0u8; 100];
    data[0..4].copy_from_slice(b"BADM"); // Wrong magic
    std::fs::write(&path, &data).unwrap();

    let result = Context::load_from_file(&path);
    assert!(result.is_err(), "Invalid magic should fail to load");
}

#[test]
fn test_empty_context_roundtrip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("empty.alec-context");

    // Save an empty context
    let ctx = Context::new();
    ctx.save_to_file(&path, "empty").unwrap();

    // Load it back
    let loaded = Context::load_from_file(&path).unwrap();
    assert_eq!(loaded.pattern_count(), 0);
    assert_eq!(loaded.context_version(), 0);
}

#[test]
fn test_large_pattern_dictionary() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("large.alec-context");

    // Create context with many patterns
    let mut ctx = Context::new();
    for i in 0..1000 {
        let pattern = vec![(i & 0xFF) as u8, ((i >> 8) & 0xFF) as u8];
        ctx.register_pattern(Pattern::new(pattern)).unwrap();
    }

    ctx.save_to_file(&path, "large_test").unwrap();
    let loaded = Context::load_from_file(&path).unwrap();

    assert_eq!(loaded.pattern_count(), 1000);
}

#[test]
fn test_sensor_type_preserved() {
    use alec::context::PreloadFile;

    let dir = tempdir().unwrap();
    let path = dir.path().join("sensor.alec-context");

    let ctx = create_trained_context();
    ctx.save_to_file(&path, "soil_moisture_v2").unwrap();

    // Read the raw preload file to check sensor type
    let data = std::fs::read(&path).unwrap();
    let preload = PreloadFile::from_bytes(&data).unwrap();

    assert_eq!(preload.sensor_type, "soil_moisture_v2");
}

#[test]
fn test_preload_file_metadata() {
    use alec::context::PreloadFile;

    let ctx = create_trained_context();
    let preload = PreloadFile::from_context(&ctx, "temperature");

    assert_eq!(preload.format_version, 1);
    assert_eq!(preload.context_version, ctx.context_version());
    assert_eq!(preload.sensor_type, "temperature");
    assert!(preload.created_timestamp > 0);
    assert_eq!(preload.training_samples, ctx.observation_count());
}

#[test]
fn test_context_version_increments() {
    let mut ctx = Context::new();
    let initial_version = ctx.context_version();

    // Observe some data - version should increment
    ctx.observe(&RawData::new(42.0, 0));
    assert!(
        ctx.context_version() > initial_version,
        "Version should increment on observe"
    );

    // Register pattern - version should increment
    let version_after_observe = ctx.context_version();
    ctx.register_pattern(Pattern::new(vec![0x42])).unwrap();
    assert!(
        ctx.context_version() > version_after_observe,
        "Version should increment on pattern registration"
    );
}

#[test]
fn test_loaded_context_can_encode() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("encode_test.alec-context");

    // Create and train a context
    let mut ctx = create_trained_context();
    ctx.register_pattern(Pattern::numeric(25.0)).unwrap();
    ctx.save_to_file(&path, "test").unwrap();

    // Load and verify it can still be used
    let loaded = Context::load_from_file(&path).unwrap();

    // Should be able to find patterns
    let code = loaded.find_pattern(&25.0_f64.to_be_bytes());
    assert!(code.is_some());

    // Should be able to get patterns by code
    let pattern = loaded.get_pattern(code.unwrap());
    assert!(pattern.is_some());
}

#[test]
fn test_multiple_save_load_cycles() {
    let dir = tempdir().unwrap();

    let mut ctx = create_trained_context();

    for i in 0..5 {
        let path = dir.path().join(format!("cycle_{}.alec-context", i));

        // Save current state
        ctx.save_to_file(&path, "cycle_test").unwrap();

        // Load it back - loaded context has default config (evolution enabled)
        // but since we're just saving/loading without many observations, patterns survive
        ctx = Context::load_from_file(&path).unwrap();

        // Add more data (not enough to trigger evolution with interval=100)
        ctx.observe(&RawData::new(30.0 + i as f64, i as u64));
    }

    // Should still have original patterns (3 from create_trained_context)
    assert_eq!(ctx.pattern_count(), 3);
}
