#!/usr/bin/env python3
"""
ALEC Demo - Simulator Service

Real-time sensor data generation with Prometheus metrics exposition.
Generates correlated agricultural IoT sensor data using latent variables.
"""

import asyncio
import json
import logging
import os
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import numpy as np
from fastapi import FastAPI, HTTPException
from fastapi.responses import PlainTextResponse
import uvicorn

# Configure logging
logging.basicConfig(
    level=getattr(logging, os.getenv("LOG_LEVEL", "INFO").upper()),
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"
)
logger = logging.getLogger("alec-simulator")

# =============================================================================
# Configuration
# =============================================================================

SENSOR_PROFILE = os.getenv("SENSOR_PROFILE", "agricultural")
GENERATION_RATE = float(os.getenv("GENERATION_RATE", "1.0"))
PROFILE_DIR = Path(os.getenv("PROFILE_DIR", "/app/profiles"))

# =============================================================================
# Data Models
# =============================================================================

@dataclass
class SensorConfig:
    """Configuration for a single sensor."""
    id: str
    type: str
    unit: str
    base: float
    min_val: float
    max_val: float
    noise_std: float
    correlates: list[str] = field(default_factory=list)
    latent_weights: dict[str, float] = field(default_factory=dict)
    derived_from: list[str] = field(default_factory=list)

@dataclass
class SensorReading:
    """A single sensor reading."""
    sensor_id: str
    sensor_type: str
    value: float
    unit: str
    timestamp: float
    quality: float = 1.0

# =============================================================================
# Latent Variable Generator
# =============================================================================

class LatentVariableGenerator:
    """Generates correlated latent variables for realistic sensor data."""

    def __init__(self, seed: int | None = None):
        self.rng = np.random.default_rng(seed)
        self.latent_vars: dict[str, float] = {}
        self.latent_history: dict[str, list[float]] = {}
        self.start_time = time.time()

        # Initialize latent variable states
        self._init_latent_states()

    def _init_latent_states(self):
        """Initialize latent variable internal states."""
        self.weather_state = self.rng.uniform(-1, 1)
        self.gust_state = 0.0
        self.irrigation_state = 0.0
        self.irrigation_cooldown = 0

    def update(self) -> dict[str, float]:
        """Update all latent variables based on elapsed time."""
        elapsed = time.time() - self.start_time

        # Weather: slow random walk (changes over hours)
        self.weather_state += self.rng.normal(0, 0.01)
        self.weather_state = np.clip(self.weather_state, -1, 1)
        self.latent_vars["weather"] = self.weather_state

        # Daily cycle: sinusoidal pattern
        hours = elapsed / 3600
        daily_phase = (hours % 24) / 24 * 2 * np.pi
        self.latent_vars["daily_cycle"] = np.sin(daily_phase - np.pi / 2)

        # Seasonal: very slow sinusoidal (simulated at 100x speed for demo)
        days = elapsed / 86400 * 100  # 100x speed
        seasonal_phase = (days % 365) / 365 * 2 * np.pi
        self.latent_vars["seasonal"] = np.sin(seasonal_phase)

        # Gusts: sporadic bursts
        if self.rng.random() < 0.02:  # 2% chance per update
            self.gust_state = self.rng.uniform(0.5, 1.0)
        else:
            self.gust_state *= 0.9  # Decay
        self.latent_vars["gusts"] = self.gust_state

        # Irrigation: periodic events
        if self.irrigation_cooldown > 0:
            self.irrigation_cooldown -= 1
            self.irrigation_state *= 0.95
        elif self.rng.random() < 0.005:  # Occasional irrigation
            self.irrigation_state = 1.0
            self.irrigation_cooldown = 60  # 60 updates cooldown
        self.latent_vars["irrigation"] = self.irrigation_state

        return self.latent_vars

# =============================================================================
# Sensor Simulator
# =============================================================================

class SensorSimulator:
    """Simulates correlated sensor readings."""

    def __init__(self, profile_path: Path):
        self.sensors: dict[str, SensorConfig] = {}
        self.latent_gen = LatentVariableGenerator()
        self.current_values: dict[str, float] = {}
        self.injection_state: dict[str, Any] = {}

        self._load_profile(profile_path)
        self._init_values()

    def _load_profile(self, profile_path: Path):
        """Load sensor profile from JSON."""
        try:
            with open(profile_path) as f:
                profile = json.load(f)

            for sensor_data in profile.get("sensors", []):
                config = SensorConfig(
                    id=sensor_data["id"],
                    type=sensor_data["type"],
                    unit=sensor_data["unit"],
                    base=sensor_data["base"],
                    min_val=sensor_data["min"],
                    max_val=sensor_data["max"],
                    noise_std=sensor_data.get("noise_std", 0.1),
                    correlates=sensor_data.get("correlates", []),
                    latent_weights=sensor_data.get("latent_weights", {}),
                    derived_from=sensor_data.get("derived_from", [])
                )
                self.sensors[config.id] = config

            logger.info(f"Loaded {len(self.sensors)} sensors from profile")

        except Exception as e:
            logger.error(f"Failed to load profile: {e}")
            raise

    def _init_values(self):
        """Initialize sensor values to base values."""
        for sensor_id, config in self.sensors.items():
            self.current_values[sensor_id] = config.base

    def generate_reading(self, sensor_id: str) -> SensorReading:
        """Generate a single sensor reading."""
        config = self.sensors[sensor_id]
        latent_vars = self.latent_gen.latent_vars

        # Base value with latent variable contributions
        value = config.base
        for latent_name, weight in config.latent_weights.items():
            if latent_name in latent_vars:
                value += weight * latent_vars[latent_name]

        # Add noise
        value += np.random.normal(0, config.noise_std)

        # Apply injection effects
        value, quality = self._apply_injections(sensor_id, value)

        # Clamp to valid range
        value = np.clip(value, config.min_val, config.max_val)

        # Store current value for correlation
        self.current_values[sensor_id] = value

        return SensorReading(
            sensor_id=sensor_id,
            sensor_type=config.type,
            value=value,
            unit=config.unit,
            timestamp=time.time(),
            quality=quality
        )

    def _apply_injections(self, sensor_id: str, value: float) -> tuple[float, float]:
        """Apply any active injection effects."""
        quality = 1.0

        if sensor_id in self.injection_state:
            state = self.injection_state[sensor_id]

            if state.get("type") == "noise":
                factor = state.get("factor", 2.0)
                value += np.random.normal(0, self.sensors[sensor_id].noise_std * factor)
                quality = 0.8

            elif state.get("type") == "spike":
                magnitude = state.get("magnitude", 10.0)
                value += magnitude
                quality = 0.5

            elif state.get("type") == "drift":
                rate = state.get("rate", 0.1)
                elapsed = time.time() - state.get("start_time", time.time())
                value += rate * elapsed
                quality = 0.7

            elif state.get("type") == "dropout":
                if np.random.random() < state.get("probability", 0.3):
                    value = np.nan
                    quality = 0.0

        return value, quality

    def generate_all_readings(self) -> list[SensorReading]:
        """Generate readings for all sensors."""
        self.latent_gen.update()
        return [self.generate_reading(sid) for sid in self.sensors]

    def inject(self, sensor_id: str, injection_type: str, **kwargs):
        """Apply an injection effect to a sensor."""
        if sensor_id not in self.sensors:
            raise ValueError(f"Unknown sensor: {sensor_id}")

        self.injection_state[sensor_id] = {
            "type": injection_type,
            "start_time": time.time(),
            **kwargs
        }
        logger.info(f"Injection applied: {sensor_id} -> {injection_type}")

    def clear_injection(self, sensor_id: str):
        """Clear injection effect from a sensor."""
        if sensor_id in self.injection_state:
            del self.injection_state[sensor_id]
            logger.info(f"Injection cleared: {sensor_id}")

    def clear_all_injections(self):
        """Clear all injection effects."""
        self.injection_state.clear()
        logger.info("All injections cleared")

# =============================================================================
# Complexity Metrics Calculators
# =============================================================================

from collections import deque

class EntropyCalculator:
    """Calculate Shannon entropy on a rolling window of sensor values."""

    def __init__(self, window_size: int = 100, num_bins: int = 20):
        self.window_size = window_size
        self.num_bins = num_bins
        self.buffers: dict[str, deque] = {}

    def update(self, sensor_id: str, value: float) -> None:
        """Add a new value to the sensor's buffer."""
        if np.isnan(value):
            return
        if sensor_id not in self.buffers:
            self.buffers[sensor_id] = deque(maxlen=self.window_size)
        self.buffers[sensor_id].append(value)

    def get_entropy(self, sensor_id: str) -> float:
        """Calculate Shannon entropy for a sensor."""
        if sensor_id not in self.buffers or len(self.buffers[sensor_id]) < 10:
            return 0.0

        values = np.array(self.buffers[sensor_id])

        # Discretize into bins
        hist, _ = np.histogram(values, bins=self.num_bins, density=True)
        hist = hist[hist > 0]  # Remove zero bins

        if len(hist) == 0:
            return 0.0

        # Calculate Shannon entropy: H = -Σ p(x) * log2(p(x))
        # Normalize histogram to get probabilities
        probs = hist / hist.sum()
        entropy = -np.sum(probs * np.log2(probs + 1e-10))

        return float(entropy)

    def get_total_entropy(self) -> float:
        """Get sum of all sensor entropies."""
        return sum(self.get_entropy(sid) for sid in self.buffers)


class CorrelationCalculator:
    """Calculate pairwise Pearson correlation between sensors."""

    def __init__(self, window_size: int = 100):
        self.window_size = window_size
        self.buffers: dict[str, deque] = {}

    def update(self, sensor_id: str, value: float) -> None:
        """Add value to buffer."""
        if np.isnan(value):
            return
        if sensor_id not in self.buffers:
            self.buffers[sensor_id] = deque(maxlen=self.window_size)
        self.buffers[sensor_id].append(value)

    def get_correlation(self, sensor_a: str, sensor_b: str) -> float:
        """Calculate Pearson correlation between two sensors."""
        if sensor_a not in self.buffers or sensor_b not in self.buffers:
            return 0.0

        buf_a = self.buffers[sensor_a]
        buf_b = self.buffers[sensor_b]

        # Need same length and minimum samples
        min_len = min(len(buf_a), len(buf_b))
        if min_len < 10:
            return 0.0

        a = np.array(list(buf_a)[-min_len:])
        b = np.array(list(buf_b)[-min_len:])

        # Pearson correlation
        corr = np.corrcoef(a, b)[0, 1]
        return float(corr) if not np.isnan(corr) else 0.0

    def get_correlation_matrix(self) -> dict[tuple, float]:
        """Get all pairwise correlations (upper triangle only)."""
        sensors = sorted(self.buffers.keys())
        matrix = {}
        for i, s_a in enumerate(sensors):
            for s_b in sensors[i + 1:]:
                corr = self.get_correlation(s_a, s_b)
                matrix[(s_a, s_b)] = corr
        return matrix


class AnomalyDetector:
    """Detect anomalies based on z-score deviation."""

    def __init__(self, window_size: int = 100, threshold_sigma: float = 3.0):
        self.window_size = window_size
        self.threshold_sigma = threshold_sigma
        self.buffers: dict[str, deque] = {}

    def update(self, sensor_id: str, value: float) -> None:
        """Add value to buffer."""
        if np.isnan(value):
            return
        if sensor_id not in self.buffers:
            self.buffers[sensor_id] = deque(maxlen=self.window_size)
        self.buffers[sensor_id].append(value)

    def get_anomaly_score(self, sensor_id: str) -> float:
        """
        Returns anomaly score in [0, 1].
        0 = normal, 1 = highly anomalous

        Based on z-score of latest value vs rolling statistics.
        """
        if sensor_id not in self.buffers or len(self.buffers[sensor_id]) < 20:
            return 0.0

        values = np.array(self.buffers[sensor_id])
        latest = values[-1]

        # Exclude latest for statistics
        historical = values[:-1]
        mean = np.mean(historical)
        std = np.std(historical)

        if std < 1e-6:
            return 0.0

        z_score = abs(latest - mean) / std

        # Convert z-score to [0, 1] using sigmoid-like function
        anomaly_score = min(1.0, z_score / (self.threshold_sigma * 2))

        return float(anomaly_score)


def calculate_complexity(correlation_matrix: dict[tuple, float]) -> float:
    """
    Calculate complexity based on correlation structure.

    Complexity increases when correlations are moderate (around 0.5).
    Very low or very high correlations = low complexity.

    C = Σ |r_ij| * (1 - |r_ij|) * 4

    This peaks at |r| = 0.5 and is 0 at |r| = 0 or |r| = 1
    """
    if not correlation_matrix:
        return 0.0

    complexity = 0.0
    for (s_a, s_b), corr in correlation_matrix.items():
        abs_corr = abs(corr)
        # Parabola that peaks at 0.5
        complexity += abs_corr * (1 - abs_corr) * 4

    return float(complexity)


def calculate_robustness(
    complexity: float,
    entropy_total: float,
    num_sensors: int
) -> float:
    """
    Calculate robustness indicator R ∈ [0, 1].

    R approaches 1 when:
    - Complexity is moderate (not too high, not too low)
    - Entropy is stable

    R approaches 0 when:
    - Complexity is very high (fragile system)
    - High entropy (chaotic)
    """
    if num_sensors == 0:
        return 0.0

    # Normalize complexity (assume max ~= num_pairs * 1.0)
    num_pairs = num_sensors * (num_sensors - 1) / 2
    max_complexity = num_pairs  # Maximum when all correlations are 0.5
    c_norm = min(complexity / max_complexity, 1.0) if max_complexity > 0 else 0

    # Normalize entropy (assume max ~= num_sensors * 4 bits)
    max_entropy = num_sensors * 4.0  # 4 bits per sensor max
    h_norm = min(entropy_total / max_entropy, 1.0) if max_entropy > 0 else 0

    # Robustness: high when both are low/moderate
    # Using inverse weighted average
    robustness = 1.0 - (c_norm * 0.6 + h_norm * 0.4)

    return max(0.0, min(1.0, robustness))


# =============================================================================
# FastAPI Application
# =============================================================================

app = FastAPI(
    title="ALEC Simulator",
    description="Real-time correlated sensor data generator for ALEC demo",
    version="1.0.0"
)

# Global instances
simulator: SensorSimulator | None = None
entropy_calc: EntropyCalculator | None = None
correlation_calc: CorrelationCalculator | None = None
anomaly_detector: AnomalyDetector | None = None

@app.on_event("startup")
async def startup():
    """Initialize the simulator on startup."""
    global simulator, entropy_calc, correlation_calc, anomaly_detector

    profile_path = PROFILE_DIR / f"{SENSOR_PROFILE}.json"
    if not profile_path.exists():
        logger.error(f"Profile not found: {profile_path}")
        raise RuntimeError(f"Profile not found: {profile_path}")

    simulator = SensorSimulator(profile_path)

    # Initialize complexity calculators
    entropy_calc = EntropyCalculator(window_size=100, num_bins=20)
    correlation_calc = CorrelationCalculator(window_size=100)
    anomaly_detector = AnomalyDetector(window_size=100, threshold_sigma=3.0)

    logger.info(f"Simulator started with profile: {SENSOR_PROFILE}")
    logger.info("Complexity metrics calculators initialized")

@app.get("/health")
async def health():
    """Health check endpoint."""
    return {"status": "healthy", "service": "alec-simulator"}

@app.get("/metrics", response_class=PlainTextResponse)
async def metrics():
    """Prometheus metrics endpoint."""
    if simulator is None:
        raise HTTPException(status_code=503, detail="Simulator not initialized")

    readings = simulator.generate_all_readings()
    lines = []

    # Update complexity calculators with new readings
    for r in readings:
        if not np.isnan(r.value):
            entropy_calc.update(r.sensor_id, r.value)
            correlation_calc.update(r.sensor_id, r.value)
            anomaly_detector.update(r.sensor_id, r.value)

    # Sensor value metrics
    lines.append("# HELP alec_sensor_value Current sensor reading value")
    lines.append("# TYPE alec_sensor_value gauge")
    for r in readings:
        if not np.isnan(r.value):
            lines.append(
                f'alec_sensor_value{{sensor_id="{r.sensor_id}",sensor_type="{r.sensor_type}",unit="{r.unit}"}} {r.value:.6f}'
            )

    # Sensor quality metrics
    lines.append("# HELP alec_sensor_quality Data quality indicator (0-1)")
    lines.append("# TYPE alec_sensor_quality gauge")
    for r in readings:
        lines.append(
            f'alec_sensor_quality{{sensor_id="{r.sensor_id}",sensor_type="{r.sensor_type}"}} {r.quality:.2f}'
        )

    # Latent variable metrics
    lines.append("# HELP alec_latent_variable Current latent variable value")
    lines.append("# TYPE alec_latent_variable gauge")
    for name, value in simulator.latent_gen.latent_vars.items():
        lines.append(f'alec_latent_variable{{name="{name}"}} {value:.6f}')

    # Injection state metrics
    lines.append("# HELP alec_injection_active Whether injection is active for sensor")
    lines.append("# TYPE alec_injection_active gauge")
    for sensor_id in simulator.sensors:
        active = 1 if sensor_id in simulator.injection_state else 0
        lines.append(f'alec_injection_active{{sensor_id="{sensor_id}"}} {active}')

    # Active sensor count
    lines.append("# HELP alec_active_sensors Number of active sensors")
    lines.append("# TYPE alec_active_sensors gauge")
    lines.append(f"alec_active_sensors {len(simulator.sensors)}")

    # =========================================================================
    # Complexity Metrics
    # =========================================================================

    # Entropy per sensor
    lines.append("# HELP alec_entropy_per_sensor Shannon entropy per sensor (rolling window)")
    lines.append("# TYPE alec_entropy_per_sensor gauge")
    for sensor_id in simulator.sensors:
        entropy = entropy_calc.get_entropy(sensor_id)
        lines.append(f'alec_entropy_per_sensor{{sensor_id="{sensor_id}"}} {entropy:.6f}')

    # Total entropy
    lines.append("# HELP alec_entropy_total Sum of all sensor entropies")
    lines.append("# TYPE alec_entropy_total gauge")
    total_entropy = entropy_calc.get_total_entropy()
    lines.append(f"alec_entropy_total {total_entropy:.6f}")

    # Correlation matrix (upper triangle only to avoid duplicates)
    lines.append("# HELP alec_correlation Pearson correlation between sensor pairs")
    lines.append("# TYPE alec_correlation gauge")
    corr_matrix = correlation_calc.get_correlation_matrix()
    for (s_a, s_b), corr in corr_matrix.items():
        lines.append(f'alec_correlation{{sensor_a="{s_a}",sensor_b="{s_b}"}} {corr:.6f}')

    # Complexity
    lines.append("# HELP alec_complexity Complexity metric from correlation structure")
    lines.append("# TYPE alec_complexity gauge")
    complexity = calculate_complexity(corr_matrix)
    lines.append(f"alec_complexity {complexity:.6f}")

    # Robustness
    lines.append("# HELP alec_robustness Robustness indicator (0-1)")
    lines.append("# TYPE alec_robustness gauge")
    robustness = calculate_robustness(complexity, total_entropy, len(simulator.sensors))
    lines.append(f"alec_robustness {robustness:.6f}")

    # Anomaly scores
    lines.append("# HELP alec_anomaly_score Anomaly score per sensor (0-1)")
    lines.append("# TYPE alec_anomaly_score gauge")
    for sensor_id in simulator.sensors:
        score = anomaly_detector.get_anomaly_score(sensor_id)
        lines.append(f'alec_anomaly_score{{sensor_id="{sensor_id}"}} {score:.6f}')

    return "\n".join(lines) + "\n"

@app.get("/readings")
async def get_readings():
    """Get current readings for all sensors as JSON."""
    if simulator is None:
        raise HTTPException(status_code=503, detail="Simulator not initialized")

    readings = simulator.generate_all_readings()
    return {
        "timestamp": time.time(),
        "readings": [
            {
                "sensor_id": r.sensor_id,
                "type": r.sensor_type,
                "value": r.value if not np.isnan(r.value) else None,
                "unit": r.unit,
                "quality": r.quality
            }
            for r in readings
        ]
    }

@app.get("/sensors")
async def list_sensors():
    """List all configured sensors."""
    if simulator is None:
        raise HTTPException(status_code=503, detail="Simulator not initialized")

    return {
        "profile": SENSOR_PROFILE,
        "sensors": [
            {
                "id": s.id,
                "type": s.type,
                "unit": s.unit,
                "range": [s.min_val, s.max_val]
            }
            for s in simulator.sensors.values()
        ]
    }

@app.post("/inject/{sensor_id}/{injection_type}")
async def inject(sensor_id: str, injection_type: str, factor: float = 2.0, magnitude: float = 10.0, rate: float = 0.1, probability: float = 0.3):
    """Apply an injection effect to a sensor."""
    if simulator is None:
        raise HTTPException(status_code=503, detail="Simulator not initialized")

    try:
        simulator.inject(
            sensor_id,
            injection_type,
            factor=factor,
            magnitude=magnitude,
            rate=rate,
            probability=probability
        )
        return {"status": "ok", "sensor_id": sensor_id, "injection": injection_type}
    except ValueError as e:
        raise HTTPException(status_code=404, detail=str(e))

@app.delete("/inject/{sensor_id}")
async def clear_injection(sensor_id: str):
    """Clear injection effect from a sensor."""
    if simulator is None:
        raise HTTPException(status_code=503, detail="Simulator not initialized")

    simulator.clear_injection(sensor_id)
    return {"status": "ok", "sensor_id": sensor_id}

@app.post("/reset")
async def reset():
    """Reset all injections and reinitialize."""
    if simulator is None:
        raise HTTPException(status_code=503, detail="Simulator not initialized")

    simulator.clear_all_injections()
    return {"status": "ok", "message": "All injections cleared"}

@app.get("/status")
async def status():
    """Get current simulator status."""
    if simulator is None:
        raise HTTPException(status_code=503, detail="Simulator not initialized")

    return {
        "status": "running",
        "profile": SENSOR_PROFILE,
        "sensor_count": len(simulator.sensors),
        "active_injections": list(simulator.injection_state.keys()),
        "latent_variables": simulator.latent_gen.latent_vars
    }

# =============================================================================
# Main Entry Point
# =============================================================================

if __name__ == "__main__":
    uvicorn.run(
        "main:app",
        host="0.0.0.0",
        port=8080,
        log_level="info",
        reload=False
    )
