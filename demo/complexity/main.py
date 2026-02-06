#!/usr/bin/env python3
"""
ALEC Demo - Complexity Service

Calculates entropy, complexity, and robustness metrics from sensor data.
Implements Quantitative Complexity Theory (QCT) concepts for anomaly detection.

Metrics exposed:
    - alec_entropy_per_sensor: Per-sensor Shannon entropy (H_i)
    - alec_entropy_total: Total entropy (H_tot = sum of H_i)
    - alec_complexity: Complexity metric (C) based on correlations
    - alec_robustness: System robustness (R) - distance to critical point
    - alec_information_total: Total information (I_tot = H_tot + C)
    - alec_delta_information: Rate of change of I_tot (ΔI_tot)
    - alec_anomaly_score: Per-sensor anomaly score (0-1)
"""

import asyncio
import os
import time
from collections import deque
from datetime import datetime
from typing import Dict, List, Optional
from contextlib import asynccontextmanager

import httpx
import numpy as np
from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from prometheus_client import Gauge, generate_latest, CONTENT_TYPE_LATEST
from starlette.responses import Response
import uvicorn


# =============================================================================
# Configuration
# =============================================================================

SIMULATOR_URL = os.environ.get("SIMULATOR_URL", "http://simulator:8080")
POLL_INTERVAL = float(os.environ.get("POLL_INTERVAL", "1.0"))
WINDOW_SIZE = int(os.environ.get("WINDOW_SIZE", "100"))
SHORT_WINDOW = int(os.environ.get("SHORT_WINDOW", "20"))
ENTROPY_BINS = int(os.environ.get("ENTROPY_BINS", "20"))
C_CRITICAL_PERCENTILE = 95
WARMUP_SAMPLES = 50


# =============================================================================
# Prometheus Metrics (defined ONCE)
# =============================================================================

# Per-sensor entropy
entropy_per_sensor = Gauge(
    'alec_entropy_per_sensor',
    'Shannon entropy for individual sensor',
    ['sensor_id']
)

# Per-sensor anomaly score
anomaly_score = Gauge(
    'alec_anomaly_score',
    'Anomaly score for sensor (0-1)',
    ['sensor_id']
)

# Aggregate metrics
entropy_total = Gauge('alec_entropy_total', 'Total entropy H_tot')
complexity_metric = Gauge('alec_complexity', 'Complexity metric C')
robustness_metric = Gauge('alec_robustness', 'System robustness R (0-1)')
information_total = Gauge('alec_information_total', 'Total information I_tot')
delta_information = Gauge('alec_delta_information', 'Rate of change of I_tot')
correlation_mean = Gauge('alec_correlation_mean', 'Mean absolute correlation')
anomaly_count = Gauge('alec_anomaly_count', 'Number of anomalous sensors')
sensors_active = Gauge('alec_complexity_sensors_active', 'Number of active sensors')
samples_collected = Gauge('alec_complexity_samples', 'Samples in window')


# =============================================================================
# Complexity Calculator
# =============================================================================

class ComplexityCalculator:
    """Calculates entropy, complexity, and robustness metrics."""

    def __init__(self, window_size: int = 100, short_window: int = 20):
        self.window_size = window_size
        self.short_window = short_window
        self.data_buffers: Dict[str, deque] = {}
        self.i_tot_history: deque = deque(maxlen=short_window)
        self.c_history: deque = deque(maxlen=200)
        self.c_critical: Optional[float] = None
        self.c_min: Optional[float] = None
        self.baselines: Dict[str, Dict] = {}
        self.warmup_complete = False
        self.sample_count = 0

    def add_sample(self, sensor_id: str, value: float):
        """Add a new sample for a sensor."""
        if sensor_id not in self.data_buffers:
            self.data_buffers[sensor_id] = deque(maxlen=self.window_size)
        self.data_buffers[sensor_id].append(value)

    def calculate_entropy(self, values: np.ndarray, bins: int = 20) -> float:
        """Calculate Shannon entropy for a series of values."""
        if len(values) < 10:
            return 0.0
        
        hist, _ = np.histogram(values, bins=bins, density=True)
        hist = hist[hist > 0]
        if len(hist) == 0:
            return 0.0
        
        # Normalize
        hist = hist / hist.sum()
        entropy = -np.sum(hist * np.log2(hist))
        return float(entropy)

    def calculate_metrics(self) -> Dict:
        """Calculate all complexity metrics."""
        sensor_ids = list(self.data_buffers.keys())
        if not sensor_ids:
            return self._empty_result()

        # Build data matrix
        min_samples = min(len(self.data_buffers[sid]) for sid in sensor_ids)
        if min_samples < 10:
            return self._empty_result()

        data_matrix = np.array([
            list(self.data_buffers[sid])[-min_samples:]
            for sid in sensor_ids
        ])

        self.sample_count += 1

        # 1. Per-sensor entropy (H_i)
        h_sensors = {}
        for i, sid in enumerate(sensor_ids):
            h = self.calculate_entropy(data_matrix[i])
            h_sensors[sid] = h
            entropy_per_sensor.labels(sensor_id=sid).set(h)

        # 2. Total entropy (H_tot)
        h_tot = sum(h_sensors.values())
        entropy_total.set(h_tot)

        # 3. Complexity (C) from correlations
        c_value = 0.0
        corr_mean = 0.0
        if len(sensor_ids) > 1 and min_samples >= 20:
            try:
                corr_matrix = np.corrcoef(data_matrix)
                np.fill_diagonal(corr_matrix, 0)
                corr_matrix = np.nan_to_num(corr_matrix, 0)
                c_value = float(np.sum(np.abs(corr_matrix))) / 2
                corr_mean = float(np.mean(np.abs(corr_matrix)))
            except:
                pass

        complexity_metric.set(c_value)
        correlation_mean.set(corr_mean)
        self.c_history.append(c_value)

        # 4. Total information (I_tot)
        i_tot = h_tot + c_value
        information_total.set(i_tot)
        self.i_tot_history.append(i_tot)

        # 5. Delta I_tot
        delta_i = 0.0
        if len(self.i_tot_history) >= 2:
            delta_i = float(self.i_tot_history[-1] - self.i_tot_history[-2])
        delta_information.set(delta_i)

        # 6. Robustness (R) - learn critical values during warmup
        r_value = 1.0
        if self.sample_count >= WARMUP_SAMPLES:
            if not self.warmup_complete:
                c_arr = np.array(list(self.c_history))
                self.c_critical = float(np.percentile(c_arr, C_CRITICAL_PERCENTILE))
                self.c_min = float(np.min(c_arr))
                self.warmup_complete = True

            if self.c_critical and self.c_critical > self.c_min:
                r_value = (self.c_critical - c_value) / (self.c_critical - self.c_min)
                r_value = float(np.clip(r_value, 0.0, 1.0))

        robustness_metric.set(r_value)

        # 7. Anomaly detection
        anomalies = self._detect_anomalies(data_matrix, sensor_ids)
        n_anomalies = sum(anomalies.values())
        anomaly_count.set(n_anomalies)

        # Update service metrics
        sensors_active.set(len(sensor_ids))
        samples_collected.set(min_samples)

        return {
            'timestamp': datetime.utcnow().isoformat(),
            'sensors': len(sensor_ids),
            'samples': min_samples,
            'h_tot': h_tot,
            'h_sensors': h_sensors,
            'complexity': c_value,
            'correlation_mean': corr_mean,
            'i_tot': i_tot,
            'delta_i_tot': delta_i,
            'robustness': r_value,
            'c_critical': self.c_critical,
            'warmup_complete': self.warmup_complete,
            'anomalies': anomalies
        }

    def _detect_anomalies(self, data_matrix: np.ndarray, sensor_ids: List[str]) -> Dict[str, bool]:
        """Detect anomalies using z-score."""
        anomalies = {}
        
        for i, sid in enumerate(sensor_ids):
            values = data_matrix[i]
            
            # Learn baseline
            if sid not in self.baselines and len(values) >= WARMUP_SAMPLES:
                self.baselines[sid] = {
                    'mean': float(np.mean(values)),
                    'std': float(np.std(values)) + 1e-6
                }
            
            if sid in self.baselines:
                current = values[-1]
                z = abs(current - self.baselines[sid]['mean']) / self.baselines[sid]['std']
                anomalies[sid] = z > 3.0
                score = min(z / 5.0, 1.0)
                anomaly_score.labels(sensor_id=sid).set(score)
            else:
                anomalies[sid] = False
                anomaly_score.labels(sensor_id=sid).set(0.0)
        
        return anomalies

    def _empty_result(self) -> Dict:
        return {
            'timestamp': datetime.utcnow().isoformat(),
            'sensors': 0,
            'samples': 0,
            'h_tot': 0,
            'complexity': 0,
            'robustness': 1.0,
            'warmup_complete': False
        }

    def reset(self):
        """Reset all state."""
        self.data_buffers.clear()
        self.i_tot_history.clear()
        self.c_history.clear()
        self.baselines.clear()
        self.c_critical = None
        self.c_min = None
        self.warmup_complete = False
        self.sample_count = 0


# =============================================================================
# Global instances
# =============================================================================

calculator = ComplexityCalculator(WINDOW_SIZE, SHORT_WINDOW)
latest_metrics: Dict = {}
polling_task: Optional[asyncio.Task] = None


# =============================================================================
# Polling loop
# =============================================================================

async def poll_simulator():
    """Poll simulator and update metrics."""
    global latest_metrics
    
    async with httpx.AsyncClient(timeout=5.0) as client:
        while True:
            try:
                # Get metrics from simulator (Prometheus format)
                resp = await client.get(f"{SIMULATOR_URL}/metrics")
                if resp.status_code == 200:
                    text = resp.text
                    
                    # Parse alec_sensor_value lines
                    # Format: alec_sensor_value{sensor_id="sensor_01",...} 9.245924
                    for line in text.split('\n'):
                        if line.startswith('alec_sensor_value{'):
                            try:
                                # Extract sensor_id
                                start = line.find('sensor_id="') + len('sensor_id="')
                                end = line.find('"', start)
                                sensor_id = line[start:end]
                                
                                # Extract value (last part after space)
                                value_str = line.split('}')[1].strip()
                                value = float(value_str)
                                
                                calculator.add_sample(sensor_id, value)
                            except (ValueError, IndexError) as e:
                                continue
                    
                    latest_metrics = calculator.calculate_metrics()
                    
            except Exception as e:
                print(f"Poll error: {e}")
            
            await asyncio.sleep(POLL_INTERVAL)


# =============================================================================
# FastAPI App
# =============================================================================

@asynccontextmanager
async def lifespan(app: FastAPI):
    global polling_task
    polling_task = asyncio.create_task(poll_simulator())
    yield
    if polling_task:
        polling_task.cancel()

app = FastAPI(
    title="ALEC Complexity Service",
    version="1.0.0",
    lifespan=lifespan
)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)


@app.get("/health")
async def health():
    return {"status": "healthy", "service": "complexity"}


@app.get("/metrics")
async def metrics():
    return Response(generate_latest(), media_type=CONTENT_TYPE_LATEST)


@app.get("/status")
async def status():
    return {
        "service": "complexity",
        "simulator_url": SIMULATOR_URL,
        "window_size": WINDOW_SIZE,
        "warmup_complete": calculator.warmup_complete,
        "sensors_tracked": len(calculator.data_buffers),
        "latest_metrics": latest_metrics
    }


@app.post("/reset")
async def reset():
    calculator.reset()
    return {"status": "reset", "message": "All state cleared"}


# =============================================================================
# Main
# =============================================================================

if __name__ == "__main__":
    uvicorn.run(
        app,
        host="0.0.0.0",
        port=8082,
        log_level="info"
    )
