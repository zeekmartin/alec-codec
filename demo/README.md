# ALEC Demo Infrastructure

Complete demonstration environment for the ALEC (Adaptive Lossless Entropy Codec) ecosystem, including data simulation, anomaly injection, and real-time monitoring.

## Overview

This demo validates the ALEC Gateway and Complexity modules by:
1. Generating realistic correlated sensor data (15 agricultural IoT sensors)
2. Processing data through the ALEC pipeline (encoding, entropy analysis)
3. Computing complexity metrics (H_tot, C, R)
4. Visualizing everything in real-time via Grafana dashboards

## Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│    Simulator    │────▶│     Gateway     │────▶│   Complexity    │
│   (15 sensors)  │     │  (ALEC codec)   │     │   (H, C, R)     │
└─────────────────┘     └─────────────────┘     └─────────────────┘
        │                       │                       │
        │                       │                       │
        ▼                       ▼                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                         Prometheus                               │
│                    (metrics collection)                          │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                          Grafana                                 │
│                    (visualization)                               │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────┐
│    Injection    │ ◀── Manual anomaly injection for testing
│    Service      │
└─────────────────┘
```

## Quick Start

### Prerequisites

- Docker Engine 20.10+
- Docker Compose 2.0+
- 4GB RAM available

### Launch the Demo

```bash
# Start all services
docker compose up -d

# View logs
docker compose logs -f

# Stop everything
docker compose down
```

### Access Points

| Service    | URL                          | Credentials      |
|------------|------------------------------|------------------|
| Grafana    | http://localhost:3000        | admin / admin    |
| Prometheus | http://localhost:9090        |                  |
| Simulator  | http://localhost:8080        |                  |
| Gateway    | http://localhost:8081        |                  |
| Complexity | http://localhost:8082        |                  |
| Injection  | http://localhost:8084        |                  |

## Components

### Simulator (`/simulator`)

Generates realistic correlated sensor data using latent variable modeling.

**Sensors (Agricultural Profile):**
- Temperature, Humidity, Dewpoint
- Pressure, Altitude
- Luminosity, UV Index
- Wind Speed, Wind Direction
- Soil Moisture, Soil Temperature
- pH, Conductivity
- CO2, O2

**Latent Variables:**
- `weather`: Slow pressure system changes
- `daily_cycle`: 24-hour solar patterns
- `seasonal`: Long-term seasonal trends
- `gusts`: Sporadic wind events
- `irrigation`: Periodic watering events

**API Endpoints:**
- `GET /metrics` - Prometheus metrics
- `GET /readings` - Current sensor values (JSON)
- `GET /sensors` - List sensor configurations
- `GET /health` - Health check
- `GET /status` - Simulator status

### Injection Service (`/injection`)

REST API for injecting test anomalies into sensor data.

**Injection Types:**
- `noise` - Increase sensor noise (factor: multiplier)
- `spike` - Add sudden value jump (magnitude: offset)
- `drift` - Gradual value drift (rate: units/second)
- `dropout` - Random data loss (probability: 0-1)

**API Endpoints:**
- `POST /inject/{sensor_id}/{type}` - Apply injection
- `DELETE /inject/{sensor_id}` - Clear injection
- `POST /reset` - Clear all injections
- `GET /status` - Current injection state
- `GET /health` - Health check

**Example:**
```bash
# Add noise to temperature sensor
curl -X POST "http://localhost:8084/inject/sensor_01/noise?factor=3.0"

# Add spike to humidity sensor
curl -X POST "http://localhost:8084/inject/sensor_02/spike?magnitude=20"

# Clear all injections
curl -X POST "http://localhost:8084/reset"
```

### Grafana Dashboard

Pre-configured dashboard (`alec-demo.json`) with:

**Panels:**
- **Cluster Status**: Active sensors, gateway/complexity health
- **Sensor Time Series**: Real-time multi-sensor plot
- **Entropy Gauge**: H_tot with color thresholds
- **Complexity Gauge**: C metric visualization
- **Robustness Indicator**: R with HEALTHY/ATTENTION/CRITICAL zones
- **Entropy Over Time**: H_tot trend
- **Complexity Over Time**: C trend
- **Per-Sensor Entropy**: Breakdown by sensor
- **Correlation Heatmap**: Sensor correlation matrix
- **Anomaly Detection**: Active anomaly alerts

## Dataset Generation

For offline dataset generation (testing, benchmarks):

```bash
cd simulator

# Generate 1 hour of data at 1Hz
python generate_dataset.py \
  --profile profiles/agricultural.json \
  --output ../data/test_1h.csv \
  --duration 3600 \
  --rate 1.0

# With anomaly injection
python generate_dataset.py \
  --profile profiles/agricultural.json \
  --output ../data/test_anomaly.csv \
  --duration 1800 \
  --inject-spike sensor_01:15:300:60 \
  --inject-drift sensor_05:0.05:600:300
```

## Metrics Reference

### Simulator Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `alec_sensor_value` | gauge | Current sensor reading |
| `alec_sensor_quality` | gauge | Data quality (0-1) |
| `alec_latent_variable` | gauge | Latent variable value |
| `alec_injection_active` | gauge | Injection state |
| `alec_active_sensors` | gauge | Active sensor count |

### Gateway Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `alec_gateway_bytes_in` | counter | Input bytes |
| `alec_gateway_bytes_out` | counter | Output bytes |
| `alec_gateway_compression_ratio` | gauge | Current ratio |
| `alec_gateway_encoding_time_ms` | histogram | Encode latency |

### Complexity Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `alec_entropy_total` | gauge | H_tot entropy |
| `alec_complexity` | gauge | Complexity C |
| `alec_robustness` | gauge | Robustness R |
| `alec_entropy_per_sensor` | gauge | Per-sensor H |

## Development

### Adding New Sensor Profiles

Create a JSON file in `simulator/profiles/`:

```json
{
  "name": "My Profile",
  "description": "Custom sensor setup",
  "version": "1.0.0",
  "sensors": [
    {
      "id": "sensor_01",
      "type": "temperature",
      "unit": "°C",
      "base": 20.0,
      "min": -10.0,
      "max": 50.0,
      "noise_std": 0.5,
      "correlates": ["sensor_02"],
      "latent_weights": {"weather": 5.0, "daily_cycle": 3.0}
    }
  ],
  "latent_variables": {
    "weather": "Weather pattern changes",
    "daily_cycle": "24-hour cycle"
  }
}
```

### Building Individual Services

```bash
# Simulator only
docker build -t alec-simulator ./simulator

# Injection service only
docker build -t alec-injection ./injection
```

## Troubleshooting

**Services not starting:**
```bash
docker compose logs simulator
docker compose logs gateway
```

**Grafana shows no data:**
1. Check Prometheus targets: http://localhost:9090/targets
2. Verify simulator is running: http://localhost:8080/health
3. Check datasource: Grafana > Connections > Data sources

**High memory usage:**
Reduce retention in `prometheus/prometheus.yml`:
```yaml
command:
  - '--storage.tsdb.retention.time=1d'
```

## License

Part of the ALEC codec project. See root LICENSE file.
