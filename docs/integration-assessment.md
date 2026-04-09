# ALEC Integration Assessment Report

> **Modules assessed:** alec-gateway (metrics module) · alec-complexity
> **Date:** 2026-04-09
> **Target platforms:** NibiaaPlax Edge (Jetson), FAVORIOT IoT Cloud, Wireless Logic application layer

---

## Executive Summary

**alec-gateway** (metrics module, 2,684 LOC) and **alec-complexity** (3,693 LOC) are well-structured, stateful Rust libraries with clean public APIs and full JSON serialization via serde. Both are `std`-only, with nalgebra as the heaviest transitive dependency (gateway metrics only). The codebase is integration-friendly: inputs are simple JSON structs, outputs are serializable snapshots, and state is encapsulated in single engine structs. A single experienced Rust developer could ship a REST microservice in ~3 days and reach full multi-pattern coverage in ~6 weeks.

---

## Module Inventory

| Property | alec-complexity | alec-gateway (metrics) |
|---|---|---|
| Version | 0.1.0-alpha | 0.1.0-alpha |
| Lines of code | 3,693 | 4,410 (2,684 metrics) |
| Source files | 12 | 16 (8 metrics) |
| Dependencies | serde, serde_json | alec (core), thiserror, nalgebra 0.33, serde, serde_json |
| Heavy deps | None | **nalgebra 0.33** (linear algebra, covariance matrices) |
| no_std | No (HashMap, Vec, String) | No (HashMap, VecDeque, SystemTime) |
| Serde support | All pub types derive Serialize/Deserialize | All snapshot types (feature-gated) |
| Release rlib size | 2.3 MB | 2.0 MB |
| Test coverage | 35+ integration tests | Comprehensive unit + integration |
| Entry point | `ComplexityEngine::process(&mut self, &InputSnapshot) -> Option<ComplexitySnapshot>` | `MetricsEngine::observe_frame(&mut self, &[u8], u64) -> Option<MetricsSnapshot>` |

---

## Per-Pattern Analysis

### 1. REST API / JSON Microservice

| Attribute | Assessment |
|---|---|
| **Effort** | **3–4 days** |
| **Dependencies** | `axum` 0.7 + `tokio` 1.0 (or `actix-web` 4.0), `serde_json` (already present) |
| **Constraints** | Both engines are `&mut self` — need `Arc<Mutex<Engine>>` or one engine per worker. Not async-native but computation is fast (<1ms per call), so `spawn_blocking` is unnecessary for typical loads. |
| **Recommended for** | FAVORIOT IoT Cloud, any HTTP-capable platform, quickest path to partner demos |

**Interface sketch:**

```
POST /complexity/process
  Body: GenericInput (JSON)
  Response: ComplexitySnapshot (JSON)

POST /gateway/observe
  Body: { "channel_id": "temp", "value": 23.5, "timestamp_ms": 1706000000000 }
  Response: 200 OK

POST /gateway/flush
  Body: { "current_time_ms": 1706000000000 }
  Response: MetricsSnapshot (JSON)

GET  /complexity/baseline
  Response: { "state": "locked", "progress": 1.0, "stats": {...} }

POST /complexity/reset
POST /gateway/reset

GET  /health
```

**Notes:** GenericInput already has `from_json()` / `to_json()`. ComplexitySnapshot already has `to_json()` / `from_json()`. The JSON surface is essentially done — the work is just wiring HTTP routing.

---

### 2. Python Bindings (PyO3)

| Attribute | Assessment |
|---|---|
| **Effort** | **5–7 days** |
| **Dependencies** | `pyo3` 0.22, `maturin` (build tool), `serde-pyobject` or manual conversions |
| **Constraints** | nalgebra compiles fine under PyO3. Main work is mapping Rust structs to Python dicts/dataclasses. GIL management: engine is `&mut self`, so hold GIL during `process()` — fine since computation is <1ms. Need to decide: return Python dicts or custom Python classes. |
| **Recommended for** | Data science teams, Jupyter notebooks, FastAPI microservices, NibiaaPlax Edge with Python ML pipelines |

**Interface sketch:**

```python
import alec_analytics

# Complexity
engine = alec_analytics.ComplexityEngine(config={
    "enabled": True,
    "baseline": {"build_time_ms": 300000}
})

snapshot = engine.process({
    "timestamp_ms": 1706000000000,
    "h_bytes": 6.5,
    "tc": 2.3,
    "channels": [{"id": "temp", "h": 3.2}]
})

print(snapshot.events)          # List[dict]
print(snapshot.z_scores)        # dict or None
print(snapshot.to_json())       # str

# Gateway metrics
gw = alec_analytics.MetricsEngine(config={...})
gw.observe_sample("temp", 23.5, 1706000000000)
snap = gw.observe_frame(frame_bytes, current_time_ms)
```

**Notes:** Recommend `maturin develop` for local iteration, `maturin build --release` for wheels. Cross-compile for aarch64-linux (Jetson) with `cross` or `zig` linker.

---

### 3. Node.js Bindings (napi-rs)

| Attribute | Assessment |
|---|---|
| **Effort** | **5–7 days** |
| **Dependencies** | `napi` 2.x, `napi-derive` 2.x, `napi-build`, `@napi-rs/cli` (JS side) |
| **Constraints** | Same struct mapping challenge as PyO3. napi-rs has good serde integration — can return JSON strings or JS objects via `napi::bindgen_prelude::Object`. nalgebra compiles fine. Node is single-threaded so `&mut self` ownership is natural. |
| **Recommended for** | Wireless Logic application layer, any Node/TS backend, Electron-based dashboards |

**Interface sketch:**

```typescript
import { ComplexityEngine, MetricsEngine } from '@alec/analytics';

const engine = new ComplexityEngine({
  enabled: true,
  baseline: { buildTimeMs: 300000 }
});

const snapshot = engine.process({
  timestampMs: 1706000000000,
  hBytes: 6.5,
  tc: 2.3,
  channels: [{ id: "temp", h: 3.2 }]
});

console.log(snapshot.events);
console.log(snapshot.toJson());
```

**Notes:** napi-rs produces prebuilt binaries per platform. Publish to npm with `@napi-rs/cli`. Can target linux-arm64-gnu for Jetson.

---

### 4. Docker Microservice

| Attribute | Assessment |
|---|---|
| **Effort** | **2–3 days** (assuming REST server from pattern 1 exists) |
| **Dependencies** | Multi-stage Dockerfile, `rust:1.70-slim` builder, `debian:bookworm-slim` or `alpine` runtime. For MQTT variant: add `rumqttc` 0.24. |
| **Constraints** | Cross-compilation for `aarch64-unknown-linux-gnu` (Jetson) requires `cross` or buildx. Stateful — baseline state persists in memory; need volume mount or export/import API for baseline persistence across restarts. |
| **Recommended for** | NibiaaPlax Edge (Jetson), any containerized edge/cloud deployment |

**Interface sketch:**

```dockerfile
FROM rust:1.70-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p alec-service

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/alec-service /usr/local/bin/
EXPOSE 8080
ENV ALEC_CONFIG=/etc/alec/config.json
CMD ["alec-service"]
```

```yaml
# docker-compose.yml
services:
  alec-analytics:
    image: alec-analytics:latest
    ports: ["8080:8080"]
    volumes:
      - ./config.json:/etc/alec/config.json
      - baseline-data:/var/lib/alec
    deploy:
      resources:
        limits:
          memory: 64M
```

**Notes:** The `export_baseline()` / `import_baseline()` methods on ComplexityEngine already support JSON persistence — wire these to a volume-mounted file for crash recovery.

---

### 5. WebAssembly (WASM) Module

| Attribute | Assessment |
|---|---|
| **Effort** | **7–10 days** |
| **Dependencies** | `wasm-bindgen` 0.2, `wasm-pack`, `serde-wasm-bindgen`, `getrandom/js` feature, `console_error_panic_hook` |
| **Constraints** | **nalgebra is the main risk.** It compiles to WASM but produces large binaries (~1-2 MB for nalgebra alone). `alec-complexity` (no nalgebra) is straightforward. `alec-gateway` metrics would need nalgebra compiled to WASM — feasible but heavy. `SystemTime` must be replaced with `js_sys::Date::now()` or a passed-in timestamp (already the case for both engines). No filesystem access — baseline persistence via JS callbacks or IndexedDB. |
| **Recommended for** | Browser dashboards, Cloudflare Workers, serverless edge (only alec-complexity for size-sensitive targets) |

**Interface sketch:**

```javascript
import init, { WasmComplexityEngine } from '@alec/analytics-wasm';

await init();
const engine = new WasmComplexityEngine({ enabled: true });

const snapshot = engine.process(JSON.stringify({
  timestamp_ms: Date.now(),
  h_bytes: 6.5,
  channels: [{ id: "temp", h: 3.2 }]
}));

const result = JSON.parse(snapshot);
```

**Feasibility matrix:**

| Component | WASM feasible | Est. .wasm size |
|---|---|---|
| alec-complexity (standalone) | Yes, straightforward | ~300-500 KB |
| alec-gateway metrics (with nalgebra) | Yes, but heavy | ~1.5-2.5 MB |
| alec-gateway core (no metrics) | Yes | ~200-400 KB |

---

### 6. MQTT-Native Integration

| Attribute | Assessment |
|---|---|
| **Effort** | **4–5 days** |
| **Dependencies** | `rumqttc` 0.24 (async MQTT 3.1.1/5 client), `tokio` 1.0, `serde_json` (already present) |
| **Constraints** | Must define topic schema conventions. Engine is synchronous — run in a `tokio::task::spawn_blocking` or dedicate a thread. QoS selection matters for edge: QoS 1 for events, QoS 0 for periodic snapshots. Need reconnection/backoff logic (rumqttc handles this). |
| **Recommended for** | FAVORIOT IoT Cloud (MQTT-native), any MQTT broker deployment, edge-to-cloud pipelines |

**Interface sketch:**

```
Subscribe: alec/{device_id}/sensor/+/data
  Payload: { "value": 23.5, "timestamp_ms": 1706000000000 }

Publish:  alec/{device_id}/complexity/snapshot
  Payload: ComplexitySnapshot (JSON)

Publish:  alec/{device_id}/complexity/event
  Payload: ComplexityEvent (JSON, on anomaly only)

Publish:  alec/{device_id}/metrics/snapshot
  Payload: MetricsSnapshot (JSON)

Subscribe: alec/{device_id}/control
  Payload: { "command": "reset" | "export_baseline" | "import_baseline", ... }
```

**Notes:** This is the most natural fit for IoT platforms. Combine with Docker (pattern 4) for a fully self-contained edge appliance.

---

## Summary Table

| Pattern | Effort | Key Dependencies | Recommended For |
|---|---|---|---|
| REST microservice | **3-4 days** | axum, tokio | FAVORIOT, quick demos, any HTTP platform |
| Python bindings | **5-7 days** | pyo3, maturin | Data science, Jupyter, Jetson ML pipelines |
| Node.js bindings | **5-7 days** | napi-rs | Wireless Logic, TS backends |
| Docker microservice | **2-3 days** (+REST) | Docker, cross | NibiaaPlax Edge (Jetson), cloud deploy |
| WASM module | **7-10 days** | wasm-bindgen, wasm-pack | Browser dashboards, serverless |
| MQTT-native | **4-5 days** | rumqttc, tokio | FAVORIOT, pure message bus IoT |

**Total for all 6 patterns: ~26-36 developer-days (~6-8 weeks)**

---

## Additional Questions

### Minimal interface a third party must implement

```
// Minimum viable integration (complexity only):
1. Produce a JSON object: { "timestamp_ms": u64, "h_bytes": f64 }
2. POST it or publish it
3. Consume the ComplexitySnapshot JSON response

// Full integration (gateway + complexity):
1. Register channels by name
2. Push (channel_id, value, timestamp_ms) tuples
3. Trigger flush periodically
4. Consume MetricsSnapshot and/or ComplexitySnapshot
```

That's it. Both engines accept simple numeric inputs and return self-contained JSON snapshots. No callbacks, no streaming protocols, no schema negotiation required.

### Blocking dependencies for no_std / embedded

| Blocker | Module | Severity | Mitigation |
|---|---|---|---|
| `HashMap` / `VecDeque` | Both | **Hard** | Replace with `heapless` or `BTreeMap` — significant refactor |
| `String` / `Vec` | Both | **Hard** | Requires `alloc` at minimum |
| `nalgebra` 0.33 | Gateway metrics | **Medium** | nalgebra supports `no_std` + `alloc`, but increases binary size |
| `serde_json` | Both | **Hard** | No no_std support; replace with `serde-json-core` or `postcard` |
| `thiserror` | Gateway | **Easy** | Replace with manual `Display`/`Error` impls |
| `SystemTime` | Gateway | **Easy** | Already uses passed-in timestamps in metrics API |

**Verdict:** True `no_std` bare-metal is **not feasible without major refactoring** (~15-20 days). `no_std + alloc` (e.g., Cortex-M with heap) is achievable in ~10 days but questionable ROI. For embedded Jetson/RPi (which run Linux), this is irrelevant — standard `std` builds work fine on those targets.

### Public API surface readiness

| Aspect | Status | Action needed |
|---|---|---|
| Struct/method naming | Consistent, idiomatic Rust | None |
| Serde derives | All pub types | None |
| JSON round-trip | `to_json()` / `from_json()` on all snapshots | None |
| Error types | `thiserror`-based in gateway, `String` in complexity | Unify to typed errors in complexity (~1 day) |
| Builder pattern | `GenericInput` has fluent builder | None |
| Config defaults | `Default` impls on all config types | None |
| Versioning | Both `0.1.0-alpha` | Bump to `0.1.0` for partner release |
| Documentation | Doc comments present, examples in lib.rs | Add per-method examples (~2 days) |
| Baseline persistence | `export_baseline()` / `import_baseline()` exist | None |
| MSRV | 1.70 | Reasonable, no issues |

**Summary:** The API is ~85% ship-ready. Main gaps: unify error types in complexity module, add rustdoc examples, bump version.

### Jetson/Edge Resource Estimation (8 channels, 60s window)

| Resource | Estimate | Notes |
|---|---|---|
| **Binary size** (release, stripped, LTO) | **~3-4 MB** | Both engines + axum REST server. Without REST: ~2 MB |
| **Baseline RAM** (idle, no data) | **~200 KB** | Engine structs + config + empty buffers |
| **Working RAM** (8ch x 60s x 10 samples/s) | **~2-4 MB** | SlidingWindow: 8 x 600 samples x 16 bytes = 75 KB. Covariance matrix: 8x8 x 8 bytes = 512 bytes. Snapshot JSON buffers: ~5-10 KB. Dominant cost: per-channel VecDeque + serde serialization buffers |
| **Peak RAM** (during covariance computation) | **~5-6 MB** | nalgebra DMatrix allocation for 8x600 matrix + covariance |
| **CPU per process() call** | **<1 ms** | On Jetson Orin (ARM Cortex-A78AE). Dominated by covariance matrix computation |
| **CPU per flush cycle** | **~2-5 ms** | ALEC encoding + metrics computation + complexity analysis |

**Conclusion:** Runs comfortably on Jetson Nano (4 GB RAM) or even Raspberry Pi 4. No GPU required. Memory footprint is negligible relative to available resources on any Jetson variant.

---

## Recommended Integration Roadmap

```
Week 1:  REST microservice (Pattern 1)
         +-- Ship as standalone binary
         +-- Validate with partner using curl / Postman
         +-- Unify error types in alec-complexity

Week 2:  Docker microservice (Pattern 4)
         +-- Multi-arch Dockerfile (amd64 + arm64)
         +-- Baseline persistence via volume mount
         +-- Deploy to Jetson for NibiaaPlax Edge demo

Week 3:  MQTT-native (Pattern 6)
         +-- Topic schema design with FAVORIOT
         +-- Standalone MQTT binary
         +-- Compose with Docker for edge appliance

Week 4-5: Python bindings (Pattern 2)
         +-- PyO3 wrapper crate
         +-- maturin CI for wheel builds
         +-- Jupyter notebook example for data science teams

Week 6-7: Node.js bindings (Pattern 3)
         +-- napi-rs wrapper crate
         +-- npm package with prebuilt binaries
         +-- TypeScript type definitions

Week 8+:  WASM (Pattern 5) — only if browser/serverless demand exists
         +-- alec-complexity only (skip nalgebra dependency)
         +-- wasm-pack + npm publish
```

**Rationale:** REST first because it's the fastest to ship and universally consumable. Docker immediately after because it's just packaging. MQTT third because it's the native IoT pattern. Language bindings follow based on partner demand. WASM last — it's the most effort with the narrowest use case.

---

## Open Questions and Risks

1. **AGPL-3.0 license** — This is the biggest non-technical blocker. Third-party platforms integrating these modules must comply with AGPL copyleft or obtain a commercial license. This affects all 6 patterns. Clarify dual-licensing terms before partner engagement.

2. **Alpha versioning** — Both modules are `0.1.0-alpha`. Partners need a stability commitment. Recommend bumping to `0.1.0` with a documented API stability policy (e.g., "snapshot format stable, config may evolve").

3. **nalgebra binary weight in WASM** — If browser deployment becomes a priority, consider extracting a lightweight covariance computation that avoids the full nalgebra dependency (~50 lines of manual matrix math for <=32 channels).

4. **Baseline cold-start** — ComplexityEngine requires 5 minutes (default) of data before producing meaningful output. Partners need to understand this warm-up period. Consider offering a "preloaded baseline" feature for known sensor profiles.

5. **No async API** — Both engines are synchronous `&mut self`. This is fine for all patterns (wrap in Mutex or dedicate a thread), but a future async interface could simplify the MQTT pattern.

6. **Missing observability** — No `log` or `tracing` instrumentation in either module. For production edge deployments, partners will want structured logging. Estimate ~1 day to add behind a feature flag.

---

*Report generated from source analysis of alec-codec workspace. All effort estimates assume a single experienced Rust developer with familiarity in the relevant ecosystem (PyO3, napi-rs, wasm-pack, etc.)*
