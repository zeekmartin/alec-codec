# ALEC Satellite IoT Demo — Maritime Cargo Monitoring

Demo dashboard showcasing ALEC compression performance for satellite IoT use cases.
Built for the ECS Technologie AG (GSaaS) sales meeting.

## Scenario

**30-day North Atlantic cargo voyage** (Rotterdam → Halifax) with 6 satellite-connected sensors:

| Sensor | Type | Range | ALEC Ratio | Pattern |
|--------|------|-------|------------|---------|
| Temperature | Cargo hold °C | 18–25 | 15–22× | Slow diurnal cycle |
| Pressure | Atmospheric hPa | 1005–1020 | 18–25× | Very stable |
| Vibration | Hull g-force | 0.01–2.5 | 5–10× | Chaotic (engine + sea) |
| GPS Latitude | Position °N | 44–56 | 12–18× | Monotonic route |
| Humidity | Cargo hold %RH | 55–85 | 10–15× | Cyclical |
| Cathodic Voltage | Protection mV | -1100 to -850 | 20–30× | Ultra-stable + anomalies |

## Quick Start

```bash
cd satellite_demo
docker compose up -d
```

**Access:**
- Grafana: http://localhost:3001 (admin/admin)
- Prometheus: http://localhost:9091
- Simulator status: http://localhost:8085/status

## Dashboard Sections

1. **Overview Stats** — Total bytes saved, avg compression ratio, active sensors, anomaly count
2. **Compression Performance** — Bar chart per sensor, cumulative Raw vs ALEC vs gzip
3. **Frame-Level Comparison** — Per-transmission payload sizes, delta encoding bits
4. **Sensor Data Streams** — 2×3 grid of live sensor values
5. **ALEC Complexity** — Anomaly detection (z-scores, complexity index, event timeline)
6. **Voyage & Environment** — Progress gauge, latent variable drivers

## Key Demo Points

1. **gzip fails on small payloads**: 120-byte LoRaWAN frame becomes 138 bytes with gzip (+15% overhead)
2. **ALEC cold start → warm context**: First 5 frames at ~48 bytes, then drops to ~12 bytes after context builds
3. **Stable sensors = near-zero cost**: Cathodic voltage barely changes → ALEC transmits 2 bits (same-value flag)
4. **Chaotic data still compresses**: Vibration at 5–10× is still far superior to gzip
5. **Anomaly detection is free**: ALEC Complexity catches cathodic voltage spikes without extra sensors or computation

## Architecture

```
Maritime Sensor Simulator (Python/FastAPI)
    ↓ /metrics (Prometheus format)
Prometheus (scrape every 5s)
    ↓
Grafana Dashboard (auto-refresh 10s)
```

## Configuration

Environment variables for the simulator:

| Variable | Default | Description |
|----------|---------|-------------|
| `SENSOR_PROFILE` | `maritime` | Sensor profile JSON name |
| `TIME_ACCEL` | `60` | Time acceleration (1 real sec = 60 sim sec) |
| `LOG_LEVEL` | `info` | Logging level |

## Stopping

```bash
docker compose down
docker compose down -v  # also remove data volumes
```

## Ports

| Service | Port | Purpose |
|---------|------|---------|
| Grafana | 3001 | Dashboard UI |
| Prometheus | 9091 | Metrics storage |
| Simulator | 8085 | Sensor data generator |

Ports are offset from the main demo (3000/9090/8080) to allow running both stacks simultaneously.
