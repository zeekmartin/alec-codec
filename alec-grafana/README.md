# ALEC Grafana Monitoring Stack

Complete monitoring stack for ALEC (Adaptive Lossless Entropy Codec) with Prometheus and Grafana.

## Overview

This directory contains a Docker Compose stack for visualizing ALEC metrics:

- **ALEC Exporter**: Prometheus exporter replaying sensor datasets
- **Prometheus**: Time-series database for metrics
- **Grafana**: Visualization dashboards

## Quick Start

```bash
# Start the stack
./scripts/demo.sh start

# Open Grafana at http://localhost:3000
# Username: admin
# Password: admin

# Stop the stack
./scripts/demo.sh stop
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Docker Compose Stack                                           │
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐      │
│  │    ALEC      │    │  Prometheus  │    │   Grafana    │      │
│  │   Exporter   │───>│              │───>│              │      │
│  │   :9100      │    │   :9090      │    │   :3000      │      │
│  │              │    │              │    │              │      │
│  │  CSV Replay  │    │  15s scrape  │    │  Dashboards  │      │
│  └──────────────┘    └──────────────┘    └──────────────┘      │
│         │                   │                   │               │
│         ▼                   ▼                   ▼               │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐      │
│  │   datasets/  │    │ prometheus_  │    │  grafana_    │      │
│  │   (volume)   │    │    data      │    │    data      │      │
│  └──────────────┘    └──────────────┘    └──────────────┘      │
└─────────────────────────────────────────────────────────────────┘
```

## Services

### ALEC Exporter (Port 9100)

Prometheus exporter that replays CSV sensor datasets through the ALEC metrics pipeline.

- **Metrics endpoint**: http://localhost:9100/metrics
- **Health check**: http://localhost:9100/health
- **Status (JSON)**: http://localhost:9100/status

Default configuration:
- Replays `farm_normal_24h.csv` at 60x speed
- Loops continuously

### Prometheus (Port 9090)

Time-series database collecting ALEC metrics.

- **Web UI**: http://localhost:9090
- **Scrape interval**: 5 seconds for ALEC, 15 seconds for Prometheus itself
- **Retention**: 7 days

### Grafana (Port 3000)

Visualization platform with pre-configured dashboards.

- **Web UI**: http://localhost:3000
- **Credentials**: admin/admin
- **Auto-provisioned**: Prometheus datasource and ALEC dashboards

## Dashboards

### ALEC Overview

Main monitoring dashboard with:

1. **System Health**: Status indicators and gauges
   - Resilience Zone (Healthy/Warning/Critical)
   - Resilience Index (R) gauge
   - Total Correlation, Joint Entropy, Payload Entropy stats
   - Samples processed counter

2. **Entropy Metrics Over Time**: Time series graphs
   - Signal entropy metrics (TC, H_joint, Sum H)
   - Resilience Index with threshold zones

3. **Complexity Analysis**: Baseline and deviation metrics
   - Baseline status (Learning/Locked)
   - Baseline progress gauge
   - Z-scores for all metrics (TC, H_bytes, R, H_joint)

4. **Per-Channel Analysis**: Channel-level breakdown
   - Per-channel entropy time series
   - Entropy distribution pie chart
   - Criticality ranking bar gauge

5. **Anomaly Events**: Event detection
   - Event rate time series
   - Total events by type and severity

## Directory Structure

```
alec-grafana/
├── docker-compose.yml          # Main compose file
├── README.md                   # This file
├── prometheus/
│   └── prometheus.yml          # Prometheus configuration
├── grafana/
│   ├── provisioning/
│   │   ├── datasources/
│   │   │   └── prometheus.yml  # Auto-configure Prometheus
│   │   └── dashboards/
│   │       └── default.yml     # Dashboard provisioning
│   └── dashboards/
│       └── alec-overview.json  # ALEC Overview dashboard
└── scripts/
    └── demo.sh                 # Demo runner script
```

## Demo Script

The `scripts/demo.sh` script provides easy management:

```bash
# Start the stack
./scripts/demo.sh start

# Stop the stack
./scripts/demo.sh stop

# Restart the stack
./scripts/demo.sh restart

# Show status
./scripts/demo.sh status

# View logs (all services)
./scripts/demo.sh logs

# View logs (specific service)
./scripts/demo.sh logs grafana

# Clean up everything (including volumes)
./scripts/demo.sh clean
```

## Configuration

### Using Different Datasets

Edit `docker-compose.yml` to change the replayed dataset:

```yaml
alec-exporter:
  command:
    - "--port=9100"
    - "--csv=/app/datasets/manufacturing/factory_normal_24h.csv"
    - "--speed=30.0"
    - "--loop-replay=true"
```

Available datasets (from alec-testdata):
- `agriculture/farm_normal_24h.csv`
- `manufacturing/factory_normal_24h.csv`
- `satellite/tracker_normal_24h.csv`
- And more in `../alec-testdata/datasets/`

### Custom Grafana Configuration

Add environment variables in `docker-compose.yml`:

```yaml
grafana:
  environment:
    - GF_SECURITY_ADMIN_PASSWORD=mysecretpassword
    - GF_USERS_ALLOW_SIGN_UP=true
```

### Adding Alert Rules

1. Create alert rules YAML in `prometheus/rules/`
2. Uncomment the `rule_files` section in `prometheus/prometheus.yml`
3. Restart Prometheus

## Troubleshooting

### Services Not Starting

```bash
# Check logs
./scripts/demo.sh logs

# Verify Docker is running
docker ps

# Check port conflicts
netstat -tulpn | grep -E '3000|9090|9100'
```

### No Data in Grafana

1. Check Prometheus targets: http://localhost:9090/targets
2. Verify exporter is running: http://localhost:9100/health
3. Check datasource in Grafana: Configuration > Data Sources

### Dashboard Not Loading

1. Refresh the browser
2. Check Grafana logs: `./scripts/demo.sh logs grafana`
3. Verify provisioning: Grafana > Dashboards > Browse > ALEC folder

## Requirements

- Docker 20.10+
- Docker Compose 2.0+ (or docker-compose 1.29+)
- ~500MB disk space
- Ports 3000, 9090, 9100 available

## License

Dual-licensed under AGPL-3.0 and Commercial License.
See [LICENSE](../LICENSE) for details.
