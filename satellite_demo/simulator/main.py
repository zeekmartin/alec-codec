#!/usr/bin/env python3
"""
ALEC Satellite IoT Demo — Maritime Cargo Monitoring Simulator

Simulates 6 satellite IoT sensor streams for a 30-day North Atlantic cargo voyage.
Generates realistic data patterns and ALEC compression metrics for Grafana dashboard.

Sensors:
  1. Temperature (°C)       — Cargo hold, diurnal cycle
  2. Pressure (hPa)         — Atmospheric, very stable
  3. Vibration (g)          — Hull/engine, chaotic
  4. GPS Latitude (°N)      — Monotonic route progression
  5. Humidity (%RH)         — Cargo hold, correlated with temp
  6. Cathodic Voltage (mV)  — Ultra-stable with anomaly spikes

ALEC compression is simulated based on delta encoding characteristics:
  - Δ=0 → 2 bits, |Δ|<8 → 6 bits, |Δ|<64 → 10 bits, |Δ|<512 → 14 bits, else → 32 bits
  - Context warm-up: first ~20 messages are cold start, then ratio improves
  - Stable sensors → tiny deltas → extreme compression
  - Chaotic sensors → larger deltas → still good but lower ratio
"""

import asyncio
import json
import logging
import math
import os
import struct
import time
from collections import deque
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
from fastapi import FastAPI
from fastapi.responses import PlainTextResponse
import uvicorn

logging.basicConfig(
    level=getattr(logging, os.getenv("LOG_LEVEL", "INFO").upper()),
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
)
logger = logging.getLogger("alec-satellite-sim")

# =============================================================================
# Configuration
# =============================================================================

PROFILE_DIR = Path(os.getenv("PROFILE_DIR", "/app/profiles"))
PROFILE_NAME = os.getenv("SENSOR_PROFILE", "maritime")
# Simulated time acceleration: 1 real second = N simulated seconds
TIME_ACCEL = float(os.getenv("TIME_ACCEL", "60"))  # 1 min real = 1 hour sim


# =============================================================================
# ALEC Compression Simulator
# =============================================================================

class ALECCompressionSimulator:
    """
    Simulates ALEC's delta-based compression for F32 sensor values.

    Based on the actual ALEC codec delta encoding strategy:
      - Δ = 0:       2 bits  (same value flag)
      - |Δ| < 8:     6 bits  (tiny delta)
      - |Δ| < 64:    10 bits (small delta)
      - |Δ| < 512:   14 bits (medium delta)
      - |Δ| >= 512:  32 bits (full value, anomaly)

    We quantize F32 to integer deltas using the sensor's noise floor.
    """

    def __init__(self, num_channels: int = 6):
        self.num_channels = num_channels
        self.context_age = 0  # messages processed (warm-up tracker)
        self.prev_values: dict[str, float] = {}
        self.prev_quantized: dict[str, int] = {}
        self.total_raw_bytes = 0
        self.total_compressed_bytes = 0
        self.total_gzip_bytes = 0
        self.per_channel_raw: dict[str, int] = {}
        self.per_channel_compressed: dict[str, int] = {}
        self.per_channel_ratio: dict[str, float] = {}

    def _quantize(self, value: float, resolution: float) -> int:
        """Quantize a float value to an integer based on sensor resolution."""
        return int(round(value / resolution))

    def _delta_bits(self, delta: int) -> int:
        """Estimate bits needed for a delta value using ALEC encoding."""
        abs_d = abs(delta)
        if abs_d == 0:
            return 2   # same-value flag
        elif abs_d < 8:
            return 6   # tiny delta: 1 sign + 3 value + 2 overhead
        elif abs_d < 64:
            return 10  # small delta
        elif abs_d < 512:
            return 14  # medium delta
        else:
            return 32  # full value transmission

    def process_frame(
        self,
        readings: dict[str, float],
        resolutions: dict[str, float],
    ) -> dict:
        """
        Process one transmission frame with all sensor readings.

        Returns per-channel and aggregate compression stats.
        """
        self.context_age += 1

        # Raw payload: 6 sensors × 4 bytes (F32) + 2 byte header = 26 bytes
        # But spec says ~120 bytes per LoRaWAN frame (includes LoRaWAN overhead,
        # sensor IDs, timestamps, checksums)
        raw_bytes_per_sensor = 20  # 4 F32 + 4 timestamp + 4 sensor_id + 8 overhead
        frame_raw = len(readings) * raw_bytes_per_sensor

        frame_compressed_bits = 0
        # Frame header: version(8) + channel_count(8) + timestamp_delta(16)
        frame_compressed_bits += 32

        channel_stats = {}

        for sensor_id, value in readings.items():
            resolution = resolutions.get(sensor_id, 0.01)
            quantized = self._quantize(value, resolution)

            if sensor_id in self.prev_quantized:
                delta = quantized - self.prev_quantized[sensor_id]
            else:
                delta = 99999  # Force full value on first reading

            # Cold start penalty: first N messages have less context
            if self.context_age < 5:
                # Very cold: send full values
                bits = 32
            elif self.context_age < 20:
                # Warming up: deltas are less efficient
                bits = self._delta_bits(delta)
                bits = min(32, int(bits * 1.3))  # 30% overhead during warm-up
            else:
                bits = self._delta_bits(delta)

            # Channel overhead: id(4 bits) + length(4 bits)
            channel_bits = bits + 8

            self.prev_quantized[sensor_id] = quantized
            self.prev_values[sensor_id] = value

            # Per-channel stats
            channel_bytes = max(1, (channel_bits + 7) // 8)
            self.per_channel_raw[sensor_id] = raw_bytes_per_sensor
            self.per_channel_compressed[sensor_id] = channel_bytes

            if channel_bytes > 0:
                self.per_channel_ratio[sensor_id] = raw_bytes_per_sensor / channel_bytes
            else:
                self.per_channel_ratio[sensor_id] = raw_bytes_per_sensor

            frame_compressed_bits += channel_bits

            channel_stats[sensor_id] = {
                "delta": delta,
                "bits": bits,
                "channel_bytes": channel_bytes,
                "is_anomaly": abs(delta) >= 512,
            }

        # Total frame compressed size
        frame_compressed = max(4, (frame_compressed_bits + 7) // 8)

        # Gzip simulation: on small payloads, gzip adds overhead
        # gzip header is ~18 bytes + deflate overhead
        gzip_simulated = max(frame_raw, int(frame_raw * 0.92) + 18)
        if frame_raw < 150:
            # On small payloads gzip is worse
            gzip_simulated = frame_raw + 18

        self.total_raw_bytes += frame_raw
        self.total_compressed_bytes += frame_compressed
        self.total_gzip_bytes += gzip_simulated

        return {
            "frame_raw": frame_raw,
            "frame_compressed": frame_compressed,
            "frame_gzip": gzip_simulated,
            "frame_ratio": frame_raw / max(1, frame_compressed),
            "channels": channel_stats,
            "context_age": self.context_age,
        }


# =============================================================================
# ALEC Complexity Simulator
# =============================================================================

class ComplexityMonitor:
    """
    Simulates ALEC Complexity anomaly detection.

    Tracks rolling statistics per sensor and flags structural breaks
    when z-scores exceed thresholds.
    """

    def __init__(self, window_size: int = 60, z_warn: float = 2.0, z_crit: float = 3.0):
        self.window_size = window_size
        self.z_warn = z_warn
        self.z_crit = z_crit
        self.buffers: dict[str, deque] = {}
        self.anomaly_events: list[dict] = []
        self.total_anomalies = 0
        self.complexity_index = 0.0

    def update(self, sensor_id: str, value: float) -> dict:
        """Update with new reading, return anomaly status."""
        if sensor_id not in self.buffers:
            self.buffers[sensor_id] = deque(maxlen=self.window_size)

        buf = self.buffers[sensor_id]
        result = {"z_score": 0.0, "severity": "normal", "is_anomaly": False}

        if len(buf) >= 20:
            arr = np.array(buf)
            mean = np.mean(arr)
            std = np.std(arr)

            if std > 1e-10:
                z = abs(value - mean) / std
                result["z_score"] = float(z)

                if z >= self.z_crit:
                    result["severity"] = "critical"
                    result["is_anomaly"] = True
                    self.total_anomalies += 1
                    self.anomaly_events.append({
                        "sensor_id": sensor_id,
                        "value": value,
                        "z_score": float(z),
                        "severity": "critical",
                        "time": time.time(),
                    })
                elif z >= self.z_warn:
                    result["severity"] = "warning"

        buf.append(value)
        return result

    def compute_complexity(self) -> float:
        """
        Compute overall complexity index.

        Based on cross-channel entropy variation.
        Higher when sensors diverge from expected patterns.
        """
        if len(self.buffers) < 2:
            return 0.0

        # Compute per-channel coefficient of variation
        cvs = []
        for buf in self.buffers.values():
            if len(buf) >= 10:
                arr = np.array(buf)
                mean = np.mean(arr)
                std = np.std(arr)
                if abs(mean) > 1e-10:
                    cvs.append(std / abs(mean))

        if not cvs:
            return 0.0

        # Complexity = weighted variance of CVs (high when sensors behave differently)
        cv_arr = np.array(cvs)
        self.complexity_index = float(np.std(cv_arr) * 100)
        return self.complexity_index


# =============================================================================
# Maritime Sensor Simulator
# =============================================================================

class MaritimeSensorSimulator:
    """
    Generates realistic maritime cargo monitoring sensor data.

    Simulates a 30-day North Atlantic crossing with:
    - Weather systems passing through
    - Diurnal temperature cycles
    - Engine vibration + sea state
    - GPS route progression
    - Cathodic voltage anomalies
    """

    def __init__(self, profile_path: Path):
        self.sensors: list[dict] = []
        self.start_time = time.time()
        self.rng = np.random.default_rng(42)
        self.update_count = 0

        # Latent variable states
        self.weather_state = 0.0
        self.sea_state = 0.2
        self.engine_rpm_norm = 0.0
        self.gust_state = 0.0

        # Anomaly scheduling for cathodic voltage
        self.anomaly_schedule: list[dict] = []
        self.active_anomaly: dict | None = None

        self._load_profile(profile_path)
        self._schedule_anomalies()

    def _load_profile(self, path: Path):
        with open(path) as f:
            profile = json.load(f)
        self.sensors = profile["sensors"]
        self.profile = profile
        logger.info(f"Loaded maritime profile: {len(self.sensors)} sensors")

    def _schedule_anomalies(self):
        """Pre-schedule cathodic voltage anomaly events."""
        # Schedule anomalies at specific simulated time offsets
        # These occur at random intervals during the "voyage"
        self.anomaly_schedule = [
            {"sim_offset": 1800, "magnitude": 120, "duration": 8},   # After 30min sim (~30h voyage)
            {"sim_offset": 4200, "magnitude": 180, "duration": 12},  # After 70min sim
            {"sim_offset": 7200, "magnitude": 95, "duration": 6},    # After 120min sim
        ]
        logger.info(f"Scheduled {len(self.anomaly_schedule)} cathodic anomaly events")

    def _update_latent_variables(self, elapsed_sim: float):
        """Update latent driving variables based on simulated elapsed time."""
        sim_hours = elapsed_sim / 3600
        sim_days = sim_hours / 24

        # Weather: slow sinusoidal + random walk (pressure system passing every ~5 days)
        weather_cycle = math.sin(2 * math.pi * sim_days / 4.5) * 0.6
        self.weather_state += self.rng.normal(0, 0.003)
        self.weather_state = np.clip(self.weather_state + weather_cycle * 0.01, -1, 1)

        # Diurnal cycle
        self.diurnal = math.sin(2 * math.pi * sim_hours / 24 - math.pi / 2)

        # Sea state: correlated with weather, but with its own dynamics
        target_sea = abs(self.weather_state) * 0.7 + 0.15
        self.sea_state += (target_sea - self.sea_state) * 0.02
        self.sea_state += self.rng.normal(0, 0.01)
        self.sea_state = np.clip(self.sea_state, 0.05, 1.0)

        # Engine vibration: baseline + random
        self.engine_rpm_norm = 0.5 + self.rng.normal(0, 0.05)
        self.engine_rpm_norm = np.clip(self.engine_rpm_norm, 0.2, 0.9)

        # Gusts: sporadic
        if self.rng.random() < 0.03:
            self.gust_state = self.rng.uniform(0.3, 1.0)
        else:
            self.gust_state *= 0.85

        # Voyage progress: linear 0→1 over 30 simulated days
        self.voyage_progress = min(1.0, sim_days / 30.0)

    def _check_anomalies(self, elapsed_real: float) -> float:
        """Check if a cathodic anomaly should be active. Returns magnitude or 0."""
        # Use real elapsed time for scheduling (so anomalies happen during demo)
        for sched in self.anomaly_schedule:
            trigger = sched["sim_offset"]
            duration = sched["duration"]
            if trigger <= elapsed_real < trigger + duration:
                # Active anomaly: ramp up then down
                progress = (elapsed_real - trigger) / duration
                # Bell curve shape
                envelope = math.exp(-((progress - 0.5) ** 2) / 0.08)
                return sched["magnitude"] * envelope
        return 0.0

    def generate_readings(self) -> dict[str, float]:
        """Generate one set of sensor readings."""
        self.update_count += 1
        elapsed_real = time.time() - self.start_time
        elapsed_sim = elapsed_real * TIME_ACCEL

        self._update_latent_variables(elapsed_sim)

        readings = {}

        for sensor in self.sensors:
            sid = sensor["id"]
            base = sensor["base"]
            noise_std = sensor["noise_std"]
            weights = sensor.get("latent_weights", {})

            value = base

            # Apply latent variable contributions
            if "weather" in weights:
                value += weights["weather"] * self.weather_state
            if "diurnal" in weights:
                value += weights["diurnal"] * self.diurnal
            if "sea_state" in weights:
                value += weights["sea_state"] * self.sea_state
            if "engine" in weights:
                value += weights["engine"] * self.engine_rpm_norm
            if "gusts" in weights:
                value += weights["gusts"] * self.gust_state

            # Special pattern handling
            pattern = sensor.get("pattern", "")

            if pattern == "monotonic_route":
                # GPS latitude: great circle from Rotterdam (51.9°N) to Halifax (44.6°N)
                # Goes north first (up to ~55°N) then south
                t = self.voyage_progress
                lat = 51.9 + 3.5 * math.sin(math.pi * t) - 7.3 * t
                value = lat + self.rng.normal(0, noise_std)

            elif pattern == "chaotic_engine":
                # Vibration: base engine + sea state bursts + random
                engine_vib = 0.2 + 0.15 * self.engine_rpm_norm
                sea_vib = 0.3 * self.sea_state ** 2
                burst = 0.0
                if self.rng.random() < 0.05:  # 5% chance of vibration burst
                    burst = self.rng.uniform(0.2, 0.8)
                value = engine_vib + sea_vib + burst + self.rng.normal(0, noise_std)

            elif pattern == "ultra_stable_anomaly":
                # Cathodic voltage: very stable baseline with scheduled anomalies
                anomaly_mag = self._check_anomalies(elapsed_real)
                value = base + self.rng.normal(0, noise_std)
                if anomaly_mag > 0:
                    # Anomaly pushes voltage toward -850 (less negative = problem)
                    value += anomaly_mag
            else:
                # Standard: base + latent contributions + noise
                value += self.rng.normal(0, noise_std)

            # Clamp to valid range
            value = np.clip(value, sensor["min"], sensor["max"])
            readings[sid] = float(value)

        return readings

    def get_resolutions(self) -> dict[str, float]:
        """Get quantization resolution for each sensor (based on noise floor)."""
        resolutions = {}
        for sensor in self.sensors:
            # Resolution ~= noise_std / 2 (captures meaningful changes)
            resolutions[sensor["id"]] = sensor["noise_std"] / 2
        return resolutions


# =============================================================================
# FastAPI Application
# =============================================================================

app = FastAPI(
    title="ALEC Satellite IoT Simulator",
    description="Maritime cargo monitoring simulation for ALEC compression demo",
    version="1.0.0",
)

simulator: MaritimeSensorSimulator | None = None
compressor: ALECCompressionSimulator | None = None
complexity: ComplexityMonitor | None = None


@app.on_event("startup")
async def startup():
    global simulator, compressor, complexity

    profile_path = PROFILE_DIR / f"{PROFILE_NAME}.json"
    if not profile_path.exists():
        logger.error(f"Profile not found: {profile_path}")
        raise RuntimeError(f"Profile not found: {profile_path}")

    simulator = MaritimeSensorSimulator(profile_path)
    compressor = ALECCompressionSimulator(num_channels=6)
    complexity = ComplexityMonitor(window_size=60)

    logger.info("Satellite IoT simulator started")
    logger.info(f"Time acceleration: {TIME_ACCEL}x (1 real second = {TIME_ACCEL} sim seconds)")


@app.get("/health")
async def health():
    return {"status": "healthy", "service": "alec-satellite-sim"}


@app.get("/metrics", response_class=PlainTextResponse)
async def metrics():
    """Prometheus metrics endpoint — called every scrape interval."""
    if simulator is None or compressor is None or complexity is None:
        return PlainTextResponse("# simulator not ready\n", status_code=503)

    readings = simulator.generate_readings()
    resolutions = simulator.get_resolutions()
    comp_result = compressor.process_frame(readings, resolutions)

    lines = []

    # =========================================================================
    # Sensor Values
    # =========================================================================
    lines.append("# HELP sat_sensor_value Current sensor reading (F32)")
    lines.append("# TYPE sat_sensor_value gauge")
    for sensor in simulator.sensors:
        sid = sensor["id"]
        val = readings[sid]
        lines.append(
            f'sat_sensor_value{{sensor_id="{sid}",sensor_type="{sensor["type"]}",'
            f'unit="{sensor["unit"]}",label="{sensor["label"]}"}} {val:.6f}'
        )

    # =========================================================================
    # ALEC Compression Metrics
    # =========================================================================

    # Per-channel compression ratio
    lines.append("# HELP sat_compression_ratio ALEC compression ratio per sensor (raw/compressed)")
    lines.append("# TYPE sat_compression_ratio gauge")
    for sid, ratio in compressor.per_channel_ratio.items():
        stype = next(s["type"] for s in simulator.sensors if s["id"] == sid)
        lines.append(
            f'sat_compression_ratio{{sensor_id="{sid}",sensor_type="{stype}"}} {ratio:.2f}'
        )

    # Per-channel raw bytes
    lines.append("# HELP sat_raw_bytes Raw bytes per sensor per frame")
    lines.append("# TYPE sat_raw_bytes gauge")
    for sid, raw in compressor.per_channel_raw.items():
        lines.append(f'sat_raw_bytes{{sensor_id="{sid}"}} {raw}')

    # Per-channel compressed bytes
    lines.append("# HELP sat_compressed_bytes ALEC compressed bytes per sensor per frame")
    lines.append("# TYPE sat_compressed_bytes gauge")
    for sid, comp in compressor.per_channel_compressed.items():
        lines.append(f'sat_compressed_bytes{{sensor_id="{sid}"}} {comp}')

    # Frame-level stats
    lines.append("# HELP sat_frame_raw_bytes Total raw frame size in bytes")
    lines.append("# TYPE sat_frame_raw_bytes gauge")
    lines.append(f'sat_frame_raw_bytes {comp_result["frame_raw"]}')

    lines.append("# HELP sat_frame_compressed_bytes ALEC compressed frame size in bytes")
    lines.append("# TYPE sat_frame_compressed_bytes gauge")
    lines.append(f'sat_frame_compressed_bytes {comp_result["frame_compressed"]}')

    lines.append("# HELP sat_frame_gzip_bytes Simulated gzip frame size (for comparison)")
    lines.append("# TYPE sat_frame_gzip_bytes gauge")
    lines.append(f'sat_frame_gzip_bytes {comp_result["frame_gzip"]}')

    lines.append("# HELP sat_frame_compression_ratio Frame-level compression ratio")
    lines.append("# TYPE sat_frame_compression_ratio gauge")
    lines.append(f'sat_frame_compression_ratio {comp_result["frame_ratio"]:.2f}')

    # Cumulative bytes saved
    lines.append("# HELP sat_total_raw_bytes_total Cumulative raw bytes transmitted")
    lines.append("# TYPE sat_total_raw_bytes_total counter")
    lines.append(f"sat_total_raw_bytes_total {compressor.total_raw_bytes}")

    lines.append("# HELP sat_total_compressed_bytes_total Cumulative ALEC compressed bytes")
    lines.append("# TYPE sat_total_compressed_bytes_total counter")
    lines.append(f"sat_total_compressed_bytes_total {compressor.total_compressed_bytes}")

    lines.append("# HELP sat_total_gzip_bytes_total Cumulative gzip bytes (comparison)")
    lines.append("# TYPE sat_total_gzip_bytes_total counter")
    lines.append(f"sat_total_gzip_bytes_total {compressor.total_gzip_bytes}")

    lines.append("# HELP sat_bytes_saved_total Cumulative bytes saved by ALEC (raw - compressed)")
    lines.append("# TYPE sat_bytes_saved_total counter")
    bytes_saved = compressor.total_raw_bytes - compressor.total_compressed_bytes
    lines.append(f"sat_bytes_saved_total {bytes_saved}")

    lines.append("# HELP sat_avg_compression_ratio Average compression ratio across all sensors")
    lines.append("# TYPE sat_avg_compression_ratio gauge")
    if compressor.per_channel_ratio:
        avg_ratio = sum(compressor.per_channel_ratio.values()) / len(compressor.per_channel_ratio)
    else:
        avg_ratio = 1.0
    lines.append(f"sat_avg_compression_ratio {avg_ratio:.2f}")

    # Context warm-up indicator
    lines.append("# HELP sat_context_age Number of frames processed (context maturity)")
    lines.append("# TYPE sat_context_age counter")
    lines.append(f"sat_context_age {compressor.context_age}")

    # Active sensor count
    lines.append("# HELP sat_active_sensors Number of active sensors")
    lines.append("# TYPE sat_active_sensors gauge")
    lines.append(f"sat_active_sensors {len(simulator.sensors)}")

    # =========================================================================
    # Gzip vs ALEC comparison (key demo metric)
    # =========================================================================
    lines.append("# HELP sat_gzip_overhead_bytes How much larger gzip is vs raw (gzip - raw)")
    lines.append("# TYPE sat_gzip_overhead_bytes gauge")
    gzip_overhead = comp_result["frame_gzip"] - comp_result["frame_raw"]
    lines.append(f"sat_gzip_overhead_bytes {gzip_overhead}")

    # =========================================================================
    # ALEC Complexity / Anomaly Detection
    # =========================================================================

    # Update complexity monitor
    anomaly_results = {}
    for sid, val in readings.items():
        anomaly_results[sid] = complexity.update(sid, val)

    complexity_idx = complexity.compute_complexity()

    lines.append("# HELP sat_complexity_index ALEC Complexity index (structural deviation)")
    lines.append("# TYPE sat_complexity_index gauge")
    lines.append(f"sat_complexity_index {complexity_idx:.4f}")

    lines.append("# HELP sat_anomaly_z_score Per-sensor z-score from baseline")
    lines.append("# TYPE sat_anomaly_z_score gauge")
    for sid, result in anomaly_results.items():
        lines.append(f'sat_anomaly_z_score{{sensor_id="{sid}"}} {result["z_score"]:.4f}')

    lines.append("# HELP sat_anomaly_detected Binary anomaly flag per sensor (0=normal, 1=anomaly)")
    lines.append("# TYPE sat_anomaly_detected gauge")
    for sid, result in anomaly_results.items():
        flag = 1 if result["is_anomaly"] else 0
        lines.append(f'sat_anomaly_detected{{sensor_id="{sid}"}} {flag}')

    lines.append("# HELP sat_anomalies_total Total anomaly events detected")
    lines.append("# TYPE sat_anomalies_total counter")
    lines.append(f"sat_anomalies_total {complexity.total_anomalies}")

    # Per-channel delta bits (shows how ALEC encodes each sensor)
    lines.append("# HELP sat_delta_bits Bits used for delta encoding per sensor")
    lines.append("# TYPE sat_delta_bits gauge")
    for sid, ch in comp_result["channels"].items():
        lines.append(f'sat_delta_bits{{sensor_id="{sid}"}} {ch["bits"]}')

    # Simulated voyage progress
    lines.append("# HELP sat_voyage_progress Simulated voyage progress (0-1)")
    lines.append("# TYPE sat_voyage_progress gauge")
    lines.append(f"sat_voyage_progress {simulator.voyage_progress:.4f}")

    # Latent variables (for debug/insight)
    lines.append("# HELP sat_latent_variable Latent driving variable value")
    lines.append("# TYPE sat_latent_variable gauge")
    lines.append(f'sat_latent_variable{{name="weather"}} {simulator.weather_state:.6f}')
    lines.append(f'sat_latent_variable{{name="sea_state"}} {simulator.sea_state:.6f}')
    lines.append(f'sat_latent_variable{{name="diurnal"}} {simulator.diurnal:.6f}')
    lines.append(f'sat_latent_variable{{name="engine"}} {simulator.engine_rpm_norm:.6f}')
    lines.append(f'sat_latent_variable{{name="voyage_progress"}} {simulator.voyage_progress:.6f}')

    return "\n".join(lines) + "\n"


@app.get("/status")
async def status():
    """Current simulator status."""
    if simulator is None:
        return {"status": "not_ready"}

    elapsed = time.time() - simulator.start_time
    return {
        "status": "running",
        "profile": PROFILE_NAME,
        "sensors": len(simulator.sensors),
        "updates": simulator.update_count,
        "elapsed_real_seconds": elapsed,
        "elapsed_sim_hours": elapsed * TIME_ACCEL / 3600,
        "voyage_progress": f"{simulator.voyage_progress * 100:.1f}%",
        "context_age": compressor.context_age if compressor else 0,
        "total_bytes_saved": (compressor.total_raw_bytes - compressor.total_compressed_bytes) if compressor else 0,
        "anomalies_detected": complexity.total_anomalies if complexity else 0,
    }


if __name__ == "__main__":
    uvicorn.run(
        "main:app",
        host="0.0.0.0",
        port=8080,
        log_level="info",
        reload=False,
    )
