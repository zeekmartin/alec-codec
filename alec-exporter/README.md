# ALEC Exporter

Prometheus exporter for ALEC (Adaptive Lossless Entropy Codec) metrics.

## Overview

ALEC Exporter exposes ALEC metrics in Prometheus format, enabling integration with monitoring systems like Grafana. It supports:

- **Real-time metrics**: Export live ALEC metrics from gateway processing
- **Dataset replay**: Replay pre-generated CSV datasets for demos and testing
- **HTTP endpoints**: Standard Prometheus `/metrics` endpoint plus health checks

## Installation

### From Source

```bash
cargo install --path .
```

### Docker

```bash
docker build -t alec-exporter .
```

## Usage

### Basic Usage

```bash
# Start exporter on default port (9100)
alec-exporter

# Custom port
alec-exporter --port 9090

# With dataset replay
alec-exporter --csv /path/to/dataset.csv --speed 10.0
```

### Command Line Options

| Option | Default | Description |
|--------|---------|-------------|
| `-p, --port` | 9100 | HTTP port to listen on |
| `-c, --csv` | None | CSV file to replay |
| `-s, --speed` | 1.0 | Replay speed multiplier |
| `-l, --loop-replay` | true | Loop the dataset |
| `--log-level` | info | Log level (trace, debug, info, warn, error) |

### HTTP Endpoints

| Endpoint | Description |
|----------|-------------|
| `/` | HTML landing page with documentation |
| `/metrics` | Prometheus metrics (text format) |
| `/health` | Health check (returns "OK") |
| `/ready` | Readiness check |
| `/status` | JSON status with replay info |

## Metrics

### Core Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `alec_resilience_index` | Gauge | Resilience Index (R), 0-1 |
| `alec_resilience_zone` | Gauge | Zone: 0=Healthy, 1=Warning, 2=Critical |
| `alec_total_correlation_bits` | Gauge | Total Correlation (TC) in bits |
| `alec_joint_entropy_bits` | Gauge | Joint Entropy (H_joint) in bits |
| `alec_payload_entropy_bits` | Gauge | Payload Entropy (H_bytes) in bits |
| `alec_sum_entropy_bits` | Gauge | Sum of channel entropies |

### Per-Channel Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `alec_channel_entropy_bits` | Gauge | channel | Per-channel entropy |
| `alec_channel_criticality` | Gauge | channel | Criticality ranking |

### Complexity Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `alec_baseline_progress` | Gauge | Baseline learning progress (0-1) |
| `alec_baseline_locked` | Gauge | Baseline state (1=locked) |
| `alec_zscore_tc` | Gauge | Z-score for TC |
| `alec_zscore_h_joint` | Gauge | Z-score for H_joint |
| `alec_zscore_h_bytes` | Gauge | Z-score for H_bytes |
| `alec_zscore_r` | Gauge | Z-score for R |
| `alec_delta_tc` | Gauge | Delta from baseline for TC |
| `alec_delta_h_joint` | Gauge | Delta from baseline for H_joint |
| `alec_delta_h_bytes` | Gauge | Delta from baseline for H_bytes |
| `alec_delta_r` | Gauge | Delta from baseline for R |

### Event Counters

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `alec_anomaly_events_total` | Counter | event_type, severity | Total anomaly events |

### Exporter Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `alec_exporter_samples_total` | Gauge | Total samples processed |
| `alec_exporter_replay_position` | Gauge | Current replay position |
| `alec_exporter_replay_total_samples` | Gauge | Total samples in dataset |
| `alec_exporter_replay_speed` | Gauge | Replay speed multiplier |

## Example Prometheus Config

```yaml
scrape_configs:
  - job_name: 'alec'
    scrape_interval: 5s
    static_configs:
      - targets: ['localhost:9100']
```

## Features

- `replay` (default): Enable dataset replay functionality
- To disable replay: `cargo build --no-default-features`

## Dataset Format

CSV files should have the following format:

```csv
timestamp_ms,sensor1,sensor2,sensor3
1000,25.0,60.0,1.5
2000,25.5,61.0,1.4
3000,26.0,62.0,1.6
```

- First column must be `timestamp_ms`
- Subsequent columns are sensor values
- Empty values represent missing data

## Integration with Grafana

See the [alec-grafana](../alec-grafana) directory for a complete monitoring stack with pre-built dashboards.

## License

Dual-licensed under AGPL-3.0 and Commercial License.
See [LICENSE](../LICENSE) for details.
