// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Empirical proof that `alec_encoder_context_save()` allocates
//! **zero** bytes on the heap when it writes into a caller-provided
//! buffer (v1.3.9 streaming serialiser).
//!
//! We install a counting global allocator (`TrackingAllocator`) that
//! delegates to `std::alloc::System` and records every `alloc`
//! call. The test:
//!
//!   1. Builds an encoder + saves a baseline snapshot of the counter.
//!   2. Warms up the encoder (which DOES allocate — these allocations
//!      are persistent state, not the thing we're measuring).
//!   3. Resets the counter to zero.
//!   4. Calls `alec_encoder_context_save` with a pre-sized buffer.
//!   5. Asserts that the counter is still zero.
//!
//! This is an integration test (its own crate-binary) so the global
//! allocator swap does not leak into the other test binaries.

#![cfg(feature = "decoder")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use alec_ffi::{
    alec_encode_multi_fixed, alec_encoder_context_save, alec_encoder_free,
    alec_encoder_new_with_config, AlecEncoderConfig, AlecResult,
};

struct TrackingAllocator;

/// Total bytes requested since the tracker was armed.
static BYTES: AtomicUsize = AtomicUsize::new(0);
/// Number of `alloc` calls since the tracker was armed.
static CALLS: AtomicUsize = AtomicUsize::new(0);
/// When `false`, the allocator still delegates to System but stops
/// counting — used to ignore the (substantial) allocations performed
/// by rustc test harness / print machinery / encoder warm-up.
static TRACKING: AtomicBool = AtomicBool::new(false);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACKING.load(Ordering::Relaxed) {
            BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            CALLS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if TRACKING.load(Ordering::Relaxed) {
            BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            CALLS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if TRACKING.load(Ordering::Relaxed) {
            // Count net growth only.
            if new_size > layout.size() {
                BYTES.fetch_add(new_size - layout.size(), Ordering::Relaxed);
            }
            CALLS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

fn partner_cfg() -> AlecEncoderConfig {
    AlecEncoderConfig {
        history_size: 20,
        max_patterns: 256,
        max_memory_bytes: 2048,
        keyframe_interval: 30,
        smart_resync: true,
    }
}

#[test]
fn alec_encoder_context_save_is_zero_heap() {
    // 1. Build encoder + warm up — NOT tracked.
    let enc = alec_encoder_new_with_config(&partner_cfg());
    let mut wire = [0u8; 32];
    let mut wl = 0usize;
    for i in 0..100 {
        let row = [
            1.0_f64,
            268.0 + (i as f64 * 0.1),
            120.0 + (i as f64 * 0.05),
            900.0 + (i as f64 % 50.0) * 3.0,
            10_100.0 + (i as f64 * 0.2),
        ];
        let r = alec_encode_multi_fixed(
            enc,
            row.as_ptr(),
            row.len(),
            wire.as_mut_ptr(),
            wire.len(),
            &mut wl,
        );
        assert_eq!(r, AlecResult::Ok);
    }

    // 2. Pre-size the save buffer BEFORE arming the tracker — the
    //    `vec![0u8; need]` below would otherwise count against us.
    //    On the firmware this is a static `[u8; 2048]`, no allocation
    //    at all. We use a heap-allocated `Vec` here only because this
    //    is a hosted test.
    let mut save_buf = vec![0u8; 2048];
    let mut written = 0usize;

    // 3. Arm the tracker.
    BYTES.store(0, Ordering::Relaxed);
    CALLS.store(0, Ordering::Relaxed);
    TRACKING.store(true, Ordering::Relaxed);

    // 4. Perform the save — the thing under test.
    let r = alec_encoder_context_save(enc, save_buf.as_mut_ptr(), save_buf.len(), &mut written);

    // 5. Disarm the tracker BEFORE any assertions (assertion panic
    //    paths allocate; they'd pollute the reading).
    TRACKING.store(false, Ordering::Relaxed);
    let observed_bytes = BYTES.load(Ordering::Relaxed);
    let observed_calls = CALLS.load(Ordering::Relaxed);

    assert_eq!(r, AlecResult::Ok);
    assert!(written > 0);
    assert_eq!(
        observed_calls, 0,
        "alec_encoder_context_save allocated {} times ({} bytes) — expected 0",
        observed_calls, observed_bytes,
    );
    assert_eq!(
        observed_bytes, 0,
        "alec_encoder_context_save allocated {} bytes — expected 0",
        observed_bytes,
    );

    alec_encoder_free(enc);
}
