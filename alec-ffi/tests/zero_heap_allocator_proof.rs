// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Empirical heap-behaviour tests for the v1.3.9 encoder FFI:
//!
//! * `alec_encoder_context_save_is_zero_heap` — streaming save
//!   performs zero allocations (regression guard on the v1.3.9 fix).
//! * `alec_encoder_context_load_peak_heap_ok` — load does **not**
//!   briefly require `old + new` Context heap (Q1 fix). Peak heap in
//!   flight during the load stays within ~1 × Context rather than
//!   ~2 ×.
//! * `alec_encode_multi_fixed_prewarm_is_zero_heap` — with
//!   `AlecEncoderConfig.num_channels > 0`, the first
//!   `alec_encode_multi_fixed` performs zero allocations (Q4 fix).
//! * `alec_encode_multi_fixed_without_prewarm_still_works` —
//!   backward compatibility: `num_channels = 0` keeps the legacy
//!   on-demand allocation behaviour.
//!
//! We install a counting global allocator (`TrackingAllocator`) that
//! delegates to `std::alloc::System` and records every `alloc` /
//! `dealloc` so that both total allocations AND **in-flight bytes**
//! (balance at any point) can be asserted.
//!
//! This is an integration test (its own crate-binary) so the global
//! allocator swap does not leak into the other test binaries.

#![cfg(feature = "decoder")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

use alec_ffi::{
    alec_encode_multi_fixed, alec_encoder_context_load, alec_encoder_context_save,
    alec_encoder_free, alec_encoder_new_with_config, AlecEncoderConfig, AlecResult,
};

/// Serializes the four tests in this file.
///
/// The tracking machinery below uses a single global `TRACKED_TID` —
/// only ONE thread at a time can be in the "measurement" section. If
/// two tests run in parallel on two different threads, their
/// overlapping `arm()`/`disarm()` calls fight over `TRACKED_TID` and
/// produce flaky zero-allocation readings (seen on some CI runners).
///
/// Acquiring this mutex at the top of every test body serialises the
/// four tests against each other while still letting the rest of the
/// workspace's integration tests run in parallel (`cargo test`
/// parallelism is file-level and across files, not within a single
/// test binary once this mutex is held).
static TRACKER_LOCK: Mutex<()> = Mutex::new(());

fn tracker_lock() -> MutexGuard<'static, ()> {
    // `Mutex` poisoning doesn't matter for our purposes — we only
    // care about mutual exclusion, not state consistency.
    TRACKER_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// When non-zero, contains the thread-id (as u64) of the single
/// thread currently measuring. Allocations from other threads (the
/// test harness) are ignored. Works in conjunction with `TRACKER_LOCK`
/// above: the lock ensures only one thread is ever in-section, the
/// tid check filters out stray allocations from the test harness
/// (e.g. future panic-message formatting on OTHER threads).
static TRACKED_TID: AtomicU64 = AtomicU64::new(0);

fn current_tid_u64() -> u64 {
    // `ThreadId::as_u64()` is unstable; hash the Debug id via the
    // fixed-seed `DefaultHasher` so we get a deterministic u64.
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    std::thread::current().id().hash(&mut h);
    h.finish()
}

struct TrackingAllocator;

/// Total bytes requested since the tracker was armed (tracked thread only).
static BYTES: AtomicUsize = AtomicUsize::new(0);
/// Number of `alloc` calls since the tracker was armed (tracked thread only).
static CALLS: AtomicUsize = AtomicUsize::new(0);
/// Net bytes currently in flight (alloc − dealloc) — tracked thread only.
static IN_FLIGHT: AtomicIsize = AtomicIsize::new(0);
/// Largest `IN_FLIGHT` value seen while armed.
static PEAK_IN_FLIGHT: AtomicIsize = AtomicIsize::new(0);

#[inline]
fn is_tracked_thread() -> bool {
    let tid = TRACKED_TID.load(Ordering::Relaxed);
    tid != 0 && tid == current_tid_u64()
}

#[inline]
fn account_alloc(n: usize) {
    if is_tracked_thread() {
        BYTES.fetch_add(n, Ordering::Relaxed);
        CALLS.fetch_add(1, Ordering::Relaxed);
        let new_flight = IN_FLIGHT.fetch_add(n as isize, Ordering::Relaxed) + n as isize;
        let peak = PEAK_IN_FLIGHT.load(Ordering::Relaxed);
        if new_flight > peak {
            PEAK_IN_FLIGHT.store(new_flight, Ordering::Relaxed);
        }
    }
}
#[inline]
fn account_dealloc(n: usize) {
    if is_tracked_thread() {
        IN_FLIGHT.fetch_sub(n as isize, Ordering::Relaxed);
    }
}
fn arm() {
    // The zero counters + tid-set must happen in this order so a
    // concurrent other-thread alloc with a stale tid match is impossible.
    BYTES.store(0, Ordering::Relaxed);
    CALLS.store(0, Ordering::Relaxed);
    IN_FLIGHT.store(0, Ordering::Relaxed);
    PEAK_IN_FLIGHT.store(0, Ordering::Relaxed);
    TRACKED_TID.store(current_tid_u64(), Ordering::Relaxed);
}
fn disarm() -> (usize, usize, isize) {
    TRACKED_TID.store(0, Ordering::Relaxed);
    (
        BYTES.load(Ordering::Relaxed),
        CALLS.load(Ordering::Relaxed),
        PEAK_IN_FLIGHT.load(Ordering::Relaxed),
    )
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        account_alloc(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        account_dealloc(layout.size());
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        account_alloc(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // Account the net delta as alloc or dealloc — the
        // per-thread filter is inside `account_alloc` / `_dealloc`.
        if new_size > layout.size() {
            account_alloc(new_size - layout.size());
        } else if layout.size() > new_size {
            account_dealloc(layout.size() - new_size);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

/// Partner's production config (CONTEXT.md). `num_channels = 0`
/// keeps the legacy (pre-v1.3.9) on-demand allocation behaviour for
/// backward-compat assertions.
fn partner_cfg_legacy() -> AlecEncoderConfig {
    AlecEncoderConfig {
        history_size: 20,
        max_patterns: 256,
        max_memory_bytes: 2048,
        keyframe_interval: 30,
        smart_resync: true,
        num_channels: 0,
    }
}

/// Partner's production config with the v1.3.9 pre-warm ON.
fn partner_cfg_prewarmed() -> AlecEncoderConfig {
    AlecEncoderConfig {
        num_channels: 5,
        ..partner_cfg_legacy()
    }
}

const STABLE_ROW: [f64; 5] = [1.0, 268.0, 120.0, 900.0, 10_100.0];

fn encode_one(enc: *mut alec_ffi::AlecEncoder, row: &[f64; 5]) {
    let mut wire = [0u8; 32];
    let mut wl = 0usize;
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

// ---------------------------------------------------------------------------
// v1.3.9 regression guard: streaming `alec_encoder_context_save` is zero-heap.
// ---------------------------------------------------------------------------

#[test]
fn alec_encoder_context_save_is_zero_heap() {
    let _guard = tracker_lock();
    // Warm up WITHOUT the pre-warm feature — this triggers the
    // legacy on-demand allocation path inside the 100-frame loop.
    let enc = alec_encoder_new_with_config(&partner_cfg_legacy());
    for i in 0..100 {
        let row = [
            1.0_f64,
            268.0 + (i as f64 * 0.1),
            120.0 + (i as f64 * 0.05),
            900.0 + (i as f64 % 50.0) * 3.0,
            10_100.0 + (i as f64 * 0.2),
        ];
        encode_one(enc, &row);
    }

    // Pre-size the save buffer BEFORE arming the tracker — the
    // `vec![0u8; need]` below would otherwise count against us. On
    // firmware this is a static `[u8; 2048]`, no allocation at all.
    let mut save_buf = vec![0u8; 2048];
    let mut written = 0usize;

    arm();
    let r = alec_encoder_context_save(enc, save_buf.as_mut_ptr(), save_buf.len(), &mut written);
    let (bytes, calls, _peak) = disarm();

    assert_eq!(r, AlecResult::Ok);
    assert!(written > 0);
    assert_eq!(
        calls, 0,
        "alec_encoder_context_save allocated {} times ({} bytes) — expected 0",
        calls, bytes,
    );
    assert_eq!(bytes, 0);

    alec_encoder_free(enc);
}

// ---------------------------------------------------------------------------
// Q1 — alec_encoder_context_load does NOT briefly require 2× Context heap.
//
// Measures PEAK_IN_FLIGHT bytes during the load call. The v1.3.8
// load path was "build new, then drop old" which had peak ≈ 2 ×
// Context_heap. The v1.3.9 load path is "pre-validate, drop old,
// build new" which has peak ≈ 1 × Context_heap (new-only — the old
// is gone by the time we start rebuilding).
//
// Lower bound on the persistent Context: ~1 300 B for 5 source_stats
// × (SourceStats struct + Vec<f64; 20> history) + BTreeMap node.
// A load that briefly held both would peak ≥ 2 × 1 300 = 2 600 B.
// We assert peak < 1.5 × Context_heap. This gives us a clear
// "never doubles" invariant without over-specifying the exact layout
// (which depends on allocator rounding).
// ---------------------------------------------------------------------------

#[test]
fn alec_encoder_context_load_peak_heap_ok() {
    let _guard = tracker_lock();
    // Build + warm a "source" encoder that we'll snapshot.
    let src = alec_encoder_new_with_config(&partner_cfg_legacy());
    for _ in 0..50 {
        encode_one(src, &STABLE_ROW);
    }
    let mut snap = vec![0u8; 2048];
    let mut snap_len = 0usize;
    assert_eq!(
        alec_encoder_context_save(src, snap.as_mut_ptr(), snap.len(), &mut snap_len),
        AlecResult::Ok
    );
    assert!(snap_len > 0);
    alec_encoder_free(src);

    // Build + warm a SECOND encoder and measure its persistent heap
    // *with the tracker armed* so we have a concrete baseline for
    // what "1 × Context" costs on this host.
    arm();
    let dst = alec_encoder_new_with_config(&partner_cfg_legacy());
    for _ in 0..50 {
        encode_one(dst, &STABLE_ROW);
    }
    let (ctx_bytes, _ctx_calls, ctx_peak) = disarm();
    // Sanity: the persistent context must have actually allocated
    // something, otherwise our comparison against peak_load is
    // meaningless.
    assert!(
        ctx_peak > 500,
        "expected persistent Context heap > 500 B, got peak={}",
        ctx_peak
    );

    // Now: arm the tracker AGAIN, run the load, and check the peak
    // in-flight bytes during the load call stays below ~1.5 × the
    // persistent Context baseline we just measured. A load that
    // briefly held both the old AND the new would peak at ≥ 2×.
    arm();
    let r = alec_encoder_context_load(dst, snap.as_ptr(), snap_len);
    let (_load_bytes, _load_calls, load_peak) = disarm();

    assert_eq!(r, AlecResult::Ok);

    // Hard invariant: peak during load < 2× baseline context heap.
    // Soft invariant: peak during load ≤ 1.25× baseline (leaves
    // slack for allocator metadata / small internal scratch).
    let limit_hard = 2 * ctx_peak;
    let limit_soft = ctx_peak + ctx_peak / 4;
    assert!(
        load_peak < limit_hard,
        "peak heap during context_load ({load_peak} B) reached 2× the \
         persistent Context heap ({ctx_bytes}, peak {ctx_peak} B) — \
         load is briefly doubling the heap (v1.3.8 regression)"
    );
    assert!(
        load_peak <= limit_soft,
        "peak heap during context_load ({load_peak} B) exceeded 1.25× \
         the persistent Context heap ({ctx_peak} B)"
    );

    alec_encoder_free(dst);
}

// ---------------------------------------------------------------------------
// Q4 — pre-warm: first encode_multi_fixed is zero-heap when
//                 AlecEncoderConfig.num_channels = 5.
// ---------------------------------------------------------------------------

#[test]
fn alec_encode_multi_fixed_prewarm_is_zero_heap() {
    let _guard = tracker_lock();
    // Build the encoder with pre-warm ON. All per-channel allocations
    // (5 × SourceStats + 5 × Vec<f64; 20> + BTreeMap node(s)) happen
    // inside `alec_encoder_new_with_config`, BEFORE the tracker is
    // armed. On a partner-style firmware this is call-once-at-boot.
    let enc = alec_encoder_new_with_config(&partner_cfg_prewarmed());

    // Arm the tracker and perform the first-ever encode. If the
    // pre-warm did its job, this performs ZERO heap allocations.
    arm();
    encode_one(enc, &STABLE_ROW);
    let (bytes, calls, _peak) = disarm();

    assert_eq!(
        calls, 0,
        "first encode_multi_fixed after pre-warm allocated {calls} times \
         ({bytes} B) — expected 0"
    );
    assert_eq!(bytes, 0);

    alec_encoder_free(enc);
}

// ---------------------------------------------------------------------------
// Q4 — backward compatibility: num_channels=0 keeps legacy behaviour.
// Without pre-warm the first encode DOES allocate (on-demand). It
// must still succeed — this is the pre-v1.3.9 path unchanged.
// ---------------------------------------------------------------------------

#[test]
fn alec_encode_multi_fixed_without_prewarm_still_works() {
    let _guard = tracker_lock();
    let enc = alec_encoder_new_with_config(&partner_cfg_legacy());

    // With `num_channels = 0` (legacy path) the FIRST encode observes
    // 5 previously-unseen source_ids, which triggers per-channel
    // SourceStats + Vec<f64; 20> + BTreeMap/HashMap node allocations.
    // We only assert that the encoder still produces a valid frame —
    // the exact alloc count is not a stable contract (it depends on
    // the Map implementation, initial HashMap capacity, allocator
    // rounding, and whether the first observe happens to reach an
    // evolution boundary). The Q4 pre-warm test above covers the
    // opposite direction (zero allocs when pre-warmed).
    arm();
    encode_one(enc, &STABLE_ROW);
    let (_bytes, _calls, _peak) = disarm();

    // The contract we DO care about: a second encode on the same
    // encoder, now that all source_ids have been observed once, is
    // still functional. This is the real backward-compat guarantee.
    encode_one(enc, &STABLE_ROW);

    alec_encoder_free(enc);
}
