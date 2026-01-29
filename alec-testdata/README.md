# ALEC Testdata

Realistic test dataset generator for the ALEC ecosystem.

## Overview

ALEC Testdata provides generators for realistic IoT sensor data across multiple industries, with support for:

- **Signal patterns**: Sine waves, noise, steps, random walks, diurnal cycles
- **Anomaly injection**: Sensor failures, spikes, drift, correlation breaks
- **Industry presets**: Agriculture, Satellite, Manufacturing, Smart City, Logistics, Energy

## Installation

```toml
[dependencies]
alec-testdata = "0.1"

# Optional: Integration with ALEC crates
alec-testdata = { version = "0.1", features = ["gateway", "complexity"] }
```

## Quick Start

```rust
use alec_testdata::{Dataset, GeneratorConfig, generate_dataset};
use alec_testdata::industries::agriculture::{AgriculturalScenario, create_farm_sensors};

// Generate a 24-hour agricultural dataset
let config = GeneratorConfig::new()
    .with_duration_hours(24.0)
    .with_sample_interval_secs(60)
    .with_seed(42);

let sensors = create_farm_sensors(AgriculturalScenario::Normal);
let dataset = generate_dataset(&config, &sensors);

// Export to CSV
dataset.to_csv("farm_data.csv").unwrap();

// Export to JSON
dataset.to_json("farm_data.json").unwrap();
```

## Industries

### Agriculture (AgTech)

Field sensors monitoring crop conditions and weather stations.

| Sensor | Unit | Range | Description |
|--------|------|-------|-------------|
| `soil_temp` | °C | 10-35 | Soil temperature |
| `soil_moisture` | % | 20-80 | Soil moisture content |
| `air_temp` | °C | -5-45 | Air temperature |
| `air_humidity` | % | 30-95 | Air humidity |
| `rain_gauge` | mm | 0-50 | Precipitation |
| `solar_radiation` | W/m² | 0-1200 | Solar irradiance |
| `wind_speed` | m/s | 0-25 | Wind speed |
| `leaf_wetness` | binary | 0-1 | Leaf wetness sensor |

Scenarios: `Normal`, `Drought`, `SensorFailure`, `IrrigationCycle`, `FrostEvent`

### Satellite IoT

Remote devices with intermittent connectivity.

| Sensor | Unit | Range | Description |
|--------|------|-------|-------------|
| `battery_voltage` | V | 3.0-4.2 | Battery level |
| `signal_rssi` | dBm | -120 to -60 | Signal strength |
| `gps_lat` | ° | -90-90 | Latitude |
| `gps_lon` | ° | -180-180 | Longitude |
| `internal_temp` | °C | -20-60 | Internal temperature |
| `packet_counter` | count | 0-65535 | Packet sequence |
| `tx_power` | dBm | 10-20 | Transmit power |

Scenarios: `Stationary`, `MovingAsset`, `BatteryCritical`, `SignalLoss`, `GpsDrift`

### Manufacturing (IIoT)

Factory floor sensors on production equipment.

| Sensor | Unit | Range | Description |
|--------|------|-------|-------------|
| `motor_vibration` | g | 0.1-2.0 | Motor vibration |
| `motor_temp` | °C | 40-80 | Motor temperature |
| `pressure_inlet` | bar | 4.5-5.5 | Inlet pressure |
| `pressure_outlet` | bar | 3.0-4.0 | Outlet pressure |
| `flow_rate` | L/min | 100-150 | Flow rate |
| `power_consumption` | kW | 5-15 | Power consumption |
| `product_count` | count | 0-∞ | Products produced |

Scenarios: `NormalShift`, `MachineCycle`, `BearingFailure`, `LeakEvent`, `MotorOverheat`

### Smart City

Urban sensors for traffic and environment.

| Sensor | Unit | Range | Description |
|--------|------|-------|-------------|
| `traffic_count` | vehicles/min | 0-60 | Traffic count |
| `traffic_speed` | km/h | 0-60 | Average speed |
| `air_quality_pm25` | µg/m³ | 5-150 | PM2.5 level |
| `noise_level` | dB | 40-90 | Ambient noise |
| `parking_occupancy` | % | 0-100 | Parking usage |
| `street_light_status` | binary | 0-1 | Light status |
| `pedestrian_count` | count/min | 0-200 | Pedestrian count |

Scenarios: `Weekday`, `Weekend`, `Accident`, `Festival`, `PollutionEvent`

### Logistics (Cold Chain)

Temperature-controlled transport and fleet tracking.

| Sensor | Unit | Range | Description |
|--------|------|-------|-------------|
| `cargo_temp` | °C | 2-8 | Cargo temperature |
| `ambient_temp` | °C | -10-40 | Outside temperature |
| `door_status` | binary | 0-1 | Door open/closed |
| `gps_lat` | ° | -90-90 | Latitude |
| `gps_lon` | ° | -180-180 | Longitude |
| `gps_speed` | km/h | 0-120 | Vehicle speed |
| `fuel_level` | % | 10-100 | Fuel level |
| `engine_hours` | h | 0-∞ | Engine runtime |

Scenarios: `NormalRoute`, `MultiStop`, `ColdChainBreach`, `RefrigerationFailure`, `RouteDeviation`, `FuelTheft`

### Energy (Smart Grid)

Power grid monitoring sensors.

| Sensor | Unit | Range | Description |
|--------|------|-------|-------------|
| `voltage_l1` | V | 220-240 | Phase 1 voltage |
| `voltage_l2` | V | 220-240 | Phase 2 voltage |
| `voltage_l3` | V | 220-240 | Phase 3 voltage |
| `current_l1` | A | 0-100 | Phase 1 current |
| `power_factor` | ratio | 0.85-1.0 | Power factor |
| `frequency` | Hz | 49.9-50.1 | Grid frequency |
| `active_power` | kW | 0-50 | Active power |
| `reactive_power` | kVAR | 0-20 | Reactive power |
| `thd` | % | 0-10 | Total harmonic distortion |

Scenarios: `Normal`, `IndustrialLoad`, `PhaseImbalance`, `HarmonicEvent`, `PowerFactorDrop`, `FrequencyDeviation`

## Anomaly Injection

Inject anomalies to test detection systems:

```rust
use alec_testdata::{AnomalyConfig, AnomalyType, SensorConfig};

// Create a sensor with anomaly
let sensor = SensorConfig::new("temp", "°C", 0.0, 100.0, pattern)
    .with_anomaly(AnomalyConfig {
        anomaly_type: AnomalyType::Stuck,
        start_sample: 500,
        duration_samples: Some(200),
    });
```

### Anomaly Types

| Type | Description | Expected Detection |
|------|-------------|-------------------|
| `Stuck` | Value stuck at last reading | STRUCTURE_BREAK |
| `Spike` | Sudden value spike | PAYLOAD_ENTROPY_SPIKE |
| `Drift` | Gradual value drift | COMPLEXITY_SURGE |
| `Decorrelate` | Random noise (breaks correlation) | STRUCTURE_BREAK |
| `Dropout` | Missing values | REDUNDANCY_DROP |
| `Oscillation` | Added oscillation | COMPLEXITY_SURGE |
| `BiasShift` | Constant offset | COMPLEXITY_SURGE |
| `NoiseIncrease` | Increased noise | PAYLOAD_ENTROPY_SPIKE |
| `Clipping` | Value clipping at bounds | STRUCTURE_BREAK |

## Pre-generated Datasets

The `datasets/` directory contains pre-generated CSV files for each industry:

```
datasets/
├── agriculture/
│   ├── farm_normal_24h.csv
│   ├── farm_drought_event.csv
│   └── ...
├── satellite/
├── manufacturing/
├── smart_city/
├── logistics/
└── energy/
```

Each dataset includes a manifest JSON file with metadata and expected metrics.

## Scenario Definitions

The `scenarios/` directory contains JSON definitions for common anomaly patterns:

- `sensor_failure.json` - Stuck sensor
- `gradual_drift.json` - Value drift
- `sudden_spike.json` - Spike event
- `correlation_break.json` - Decorrelation
- `redundancy_loss.json` - Sensor dropout

## Examples

### Generate Dataset

```bash
cargo run --example generate_dataset
```

### Run Through Gateway

```bash
cargo run --example run_through_gateway --features gateway
```

### Complexity Demo

```bash
cargo run --example complexity_demo --features complexity
```

## Features

- `gateway` - Integration with alec-gateway
- `complexity` - Integration with alec-complexity
- `full` - All features enabled

## License

Dual-licensed under AGPL-3.0 and Commercial License.
See [LICENSE](../LICENSE) for details.
