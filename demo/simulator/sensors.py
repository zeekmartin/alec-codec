"""
ALEC Demo - Sensor Configuration
Agricultural IoT scenario with 15 correlated sensors.
"""

from dataclasses import dataclass, field
from typing import List, Optional, Dict, Any
import json

@dataclass
class SensorConfig:
    """Configuration for a single sensor."""
    id: str
    type: str
    unit: str
    base: float
    min_val: float
    max_val: float
    noise_std: float = 0.5
    correlates: List[str] = field(default_factory=list)
    derived_from: List[str] = field(default_factory=list)
    latent_weights: Dict[str, float] = field(default_factory=dict)

# Agricultural IoT sensor configuration
SENSORS = [
    SensorConfig(
        id="sensor_01",
        type="temperature",
        unit="°C",
        base=20.0,
        min_val=-10.0,
        max_val=45.0,
        noise_std=0.3,
        correlates=["sensor_02", "sensor_05"],
        latent_weights={"weather": 8.0, "daily_cycle": 5.0, "seasonal": 3.0}
    ),
    SensorConfig(
        id="sensor_02",
        type="humidity",
        unit="%",
        base=60.0,
        min_val=20.0,
        max_val=100.0,
        noise_std=2.0,
        correlates=["sensor_01", "sensor_03"],
        latent_weights={"weather": -12.0, "daily_cycle": -3.0, "seasonal": 5.0}
    ),
    SensorConfig(
        id="sensor_03",
        type="dewpoint",
        unit="°C",
        base=12.0,
        min_val=-15.0,
        max_val=30.0,
        noise_std=0.2,
        derived_from=["sensor_01", "sensor_02"],
        latent_weights={"weather": 4.0, "daily_cycle": 2.0}
    ),
    SensorConfig(
        id="sensor_04",
        type="pressure",
        unit="hPa",
        base=1013.0,
        min_val=980.0,
        max_val=1050.0,
        noise_std=0.5,
        correlates=["sensor_05"],
        latent_weights={"weather": -5.0, "seasonal": 2.0}
    ),
    SensorConfig(
        id="sensor_05",
        type="altitude",
        unit="m",
        base=450.0,
        min_val=0.0,
        max_val=2000.0,
        noise_std=0.1,
        correlates=["sensor_04", "sensor_01"],
        latent_weights={"weather": 0.5}  # Altitude is mostly static
    ),
    SensorConfig(
        id="sensor_06",
        type="luminosity",
        unit="lux",
        base=500.0,
        min_val=0.0,
        max_val=100000.0,
        noise_std=50.0,
        correlates=["sensor_01"],
        latent_weights={"daily_cycle": 400.0, "weather": 100.0}
    ),
    SensorConfig(
        id="sensor_07",
        type="uv_index",
        unit="-",
        base=3.0,
        min_val=0.0,
        max_val=11.0,
        noise_std=0.2,
        correlates=["sensor_06"],
        latent_weights={"daily_cycle": 3.0, "weather": 1.0}
    ),
    SensorConfig(
        id="sensor_08",
        type="wind_speed",
        unit="m/s",
        base=5.0,
        min_val=0.0,
        max_val=30.0,
        noise_std=1.0,
        correlates=["sensor_09"],
        latent_weights={"weather": 3.0, "gusts": 5.0}
    ),
    SensorConfig(
        id="sensor_09",
        type="wind_direction",
        unit="°",
        base=180.0,
        min_val=0.0,
        max_val=360.0,
        noise_std=15.0,
        correlates=["sensor_08"],
        latent_weights={"weather": 30.0, "gusts": 20.0}
    ),
    SensorConfig(
        id="sensor_10",
        type="soil_moisture",
        unit="%",
        base=40.0,
        min_val=10.0,
        max_val=80.0,
        noise_std=1.5,
        correlates=["sensor_02", "sensor_11"],
        latent_weights={"weather": -5.0, "seasonal": 8.0, "irrigation": 15.0}
    ),
    SensorConfig(
        id="sensor_11",
        type="soil_temperature",
        unit="°C",
        base=18.0,
        min_val=5.0,
        max_val=35.0,
        noise_std=0.2,
        correlates=["sensor_01", "sensor_10"],
        latent_weights={"weather": 3.0, "daily_cycle": 2.0, "seasonal": 4.0}
    ),
    SensorConfig(
        id="sensor_12",
        type="ph",
        unit="-",
        base=6.5,
        min_val=4.0,
        max_val=9.0,
        noise_std=0.1,
        correlates=["sensor_10"],
        latent_weights={"irrigation": -0.3, "seasonal": 0.2}
    ),
    SensorConfig(
        id="sensor_13",
        type="conductivity",
        unit="µS/cm",
        base=500.0,
        min_val=100.0,
        max_val=2000.0,
        noise_std=20.0,
        correlates=["sensor_12"],
        latent_weights={"irrigation": -50.0, "seasonal": 30.0}
    ),
    SensorConfig(
        id="sensor_14",
        type="co2",
        unit="ppm",
        base=400.0,
        min_val=350.0,
        max_val=2000.0,
        noise_std=10.0,
        correlates=["sensor_01", "sensor_06"],
        latent_weights={"daily_cycle": -20.0, "weather": 15.0}
    ),
    SensorConfig(
        id="sensor_15",
        type="o2",
        unit="%",
        base=21.0,
        min_val=18.0,
        max_val=23.0,
        noise_std=0.1,
        correlates=["sensor_14"],
        latent_weights={"daily_cycle": 0.3, "weather": -0.2}
    ),
]


def get_sensor_by_id(sensor_id: str) -> Optional[SensorConfig]:
    """Get sensor configuration by ID."""
    for sensor in SENSORS:
        if sensor.id == sensor_id:
            return sensor
    return None


def get_sensors_dict() -> Dict[str, SensorConfig]:
    """Get all sensors as a dictionary keyed by ID."""
    return {s.id: s for s in SENSORS}


def to_json(sensors: List[SensorConfig] = SENSORS) -> str:
    """Convert sensors to JSON string."""
    data = []
    for s in sensors:
        data.append({
            "id": s.id,
            "type": s.type,
            "unit": s.unit,
            "base": s.base,
            "min": s.min_val,
            "max": s.max_val,
            "noise_std": s.noise_std,
            "correlates": s.correlates,
            "derived_from": s.derived_from,
            "latent_weights": s.latent_weights
        })
    return json.dumps(data, indent=2)


def save_profile(filename: str, sensors: List[SensorConfig] = SENSORS):
    """Save sensor profile to JSON file."""
    with open(filename, 'w') as f:
        f.write(to_json(sensors))


if __name__ == "__main__":
    # Print sensor configuration
    print(f"Agricultural IoT Sensor Configuration")
    print(f"=====================================")
    print(f"Total sensors: {len(SENSORS)}")
    print()
    for s in SENSORS:
        print(f"{s.id}: {s.type} ({s.unit})")
        print(f"  Base: {s.base}, Range: [{s.min_val}, {s.max_val}]")
        if s.correlates:
            print(f"  Correlates with: {', '.join(s.correlates)}")
        if s.derived_from:
            print(f"  Derived from: {', '.join(s.derived_from)}")
        print()
