#!/usr/bin/env python3
"""
ALEC Demo - Injection Service

FastAPI service for injecting noise, spikes, drift, and dropouts
into sensor data streams for demo purposes.

Endpoints:
    POST /inject/noise    - Add gaussian noise
    POST /inject/spike    - Single spike on sensor
    POST /inject/drift    - Progressive drift
    POST /inject/dropout  - Simulate signal loss
    POST /reset           - Clear all injections
    GET  /status          - Current injection state
    GET  /health          - Health check
"""

import asyncio
import os
import time
from datetime import datetime, timedelta
from enum import Enum
from typing import Dict, List, Optional
from contextlib import asynccontextmanager

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel, Field
import uvicorn


# Configuration
SIMULATOR_URL = os.environ.get("SIMULATOR_URL", "http://simulator:8081")


class NoiseLevel(str, Enum):
    LOW = "low"
    MEDIUM = "medium"
    HIGH = "high"


class InjectionType(str, Enum):
    NOISE = "noise"
    SPIKE = "spike"
    DRIFT = "drift"
    DROPOUT = "dropout"


# Request models
class NoiseParams(BaseModel):
    level: NoiseLevel = NoiseLevel.MEDIUM
    duration: int = Field(default=60, ge=1, le=3600, description="Duration in seconds")
    sensors: Optional[List[str]] = Field(default=None, description="Target sensors (None = all)")

    @property
    def sigma(self) -> float:
        """Get noise standard deviation based on level."""
        return {"low": 0.5, "medium": 1.5, "high": 3.0}[self.level.value]


class SpikeParams(BaseModel):
    sensor: str = Field(default="random", description="Sensor ID or 'random'")
    magnitude: float = Field(default=5.0, ge=1.0, le=20.0, description="Spike multiplier")
    direction: Optional[int] = Field(default=None, description="1 for up, -1 for down, None for random")


class DriftParams(BaseModel):
    sensor: str = Field(..., description="Target sensor ID")
    rate: float = Field(default=0.1, ge=0.01, le=1.0, description="Drift rate (units per second)")
    duration: int = Field(default=120, ge=10, le=3600, description="Duration in seconds")
    direction: int = Field(default=1, ge=-1, le=1, description="1 for positive, -1 for negative")


class DropoutParams(BaseModel):
    sensor: str = Field(default="random", description="Sensor ID or 'random'")
    duration: int = Field(default=30, ge=5, le=300, description="Duration in seconds")


# Response models
class InjectionResponse(BaseModel):
    success: bool
    message: str
    injection_id: str
    details: dict


class StatusResponse(BaseModel):
    active_injections: int
    injections: List[dict]
    uptime_seconds: float


class HealthResponse(BaseModel):
    status: str
    timestamp: str
    version: str


# Injection state management
class InjectionState:
    """Manages active injections."""

    def __init__(self):
        self.injections: Dict[str, dict] = {}
        self.start_time = time.time()
        self._counter = 0
        self._lock = asyncio.Lock()

    async def add_injection(
        self,
        injection_type: InjectionType,
        params: dict,
        duration: Optional[int] = None,
    ) -> str:
        """Add a new injection and return its ID."""
        async with self._lock:
            self._counter += 1
            injection_id = f"{injection_type.value}_{self._counter}_{int(time.time())}"

            self.injections[injection_id] = {
                "id": injection_id,
                "type": injection_type.value,
                "params": params,
                "started_at": datetime.utcnow().isoformat(),
                "expires_at": (
                    (datetime.utcnow() + timedelta(seconds=duration)).isoformat()
                    if duration else None
                ),
                "active": True,
            }

            # Schedule expiration if duration is set
            if duration:
                asyncio.create_task(self._expire_injection(injection_id, duration))

            return injection_id

    async def _expire_injection(self, injection_id: str, duration: int):
        """Expire an injection after the specified duration."""
        await asyncio.sleep(duration)
        async with self._lock:
            if injection_id in self.injections:
                self.injections[injection_id]["active"] = False

    async def remove_injection(self, injection_id: str) -> bool:
        """Remove an injection by ID."""
        async with self._lock:
            if injection_id in self.injections:
                del self.injections[injection_id]
                return True
            return False

    async def clear_all(self) -> int:
        """Clear all injections. Returns count of cleared injections."""
        async with self._lock:
            count = len(self.injections)
            self.injections.clear()
            return count

    def get_active(self) -> List[dict]:
        """Get list of active injections."""
        return [
            inj for inj in self.injections.values()
            if inj.get("active", False)
        ]

    def get_all(self) -> List[dict]:
        """Get all injections."""
        return list(self.injections.values())

    @property
    def uptime(self) -> float:
        """Get uptime in seconds."""
        return time.time() - self.start_time


# Global state
state = InjectionState()


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Application lifespan handler."""
    print("Injection service starting...")
    yield
    print("Injection service shutting down...")


# FastAPI app
app = FastAPI(
    title="ALEC Injection Service",
    description="API for injecting noise and anomalies into sensor data for demo purposes",
    version="1.0.0",
    lifespan=lifespan,
)

# CORS middleware
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


@app.get("/health", response_model=HealthResponse)
async def health_check():
    """Health check endpoint."""
    return HealthResponse(
        status="healthy",
        timestamp=datetime.utcnow().isoformat(),
        version="1.0.0",
    )


@app.get("/status", response_model=StatusResponse)
async def get_status():
    """Get current injection status."""
    return StatusResponse(
        active_injections=len(state.get_active()),
        injections=state.get_all(),
        uptime_seconds=state.uptime,
    )


@app.post("/inject/noise", response_model=InjectionResponse)
async def inject_noise(params: NoiseParams):
    """
    Add gaussian noise to sensor readings.

    - **level**: low (σ=0.5), medium (σ=1.5), high (σ=3.0)
    - **duration**: How long to apply noise (seconds)
    - **sensors**: List of sensor IDs to affect (None = all)
    """
    injection_id = await state.add_injection(
        InjectionType.NOISE,
        {
            "level": params.level.value,
            "sigma": params.sigma,
            "sensors": params.sensors,
        },
        duration=params.duration,
    )

    return InjectionResponse(
        success=True,
        message=f"Noise injection started ({params.level.value}, σ={params.sigma})",
        injection_id=injection_id,
        details={
            "level": params.level.value,
            "sigma": params.sigma,
            "duration": params.duration,
            "sensors": params.sensors or "all",
        },
    )


@app.post("/inject/spike", response_model=InjectionResponse)
async def inject_spike(params: SpikeParams):
    """
    Inject a single spike anomaly.

    - **sensor**: Target sensor ID or 'random'
    - **magnitude**: Spike multiplier (1.0-20.0)
    - **direction**: 1 (up), -1 (down), None (random)
    """
    import random

    direction = params.direction or random.choice([-1, 1])
    sensor = params.sensor if params.sensor != "random" else f"sensor_{random.randint(1, 15):02d}"

    injection_id = await state.add_injection(
        InjectionType.SPIKE,
        {
            "sensor": sensor,
            "magnitude": params.magnitude,
            "direction": direction,
        },
        duration=1,  # Spikes are instantaneous
    )

    return InjectionResponse(
        success=True,
        message=f"Spike injected on {sensor} (magnitude: {params.magnitude}x, direction: {'+' if direction > 0 else '-'})",
        injection_id=injection_id,
        details={
            "sensor": sensor,
            "magnitude": params.magnitude,
            "direction": direction,
        },
    )


@app.post("/inject/drift", response_model=InjectionResponse)
async def inject_drift(params: DriftParams):
    """
    Inject progressive drift on a sensor.

    - **sensor**: Target sensor ID
    - **rate**: Drift rate in units per second
    - **duration**: How long to apply drift (seconds)
    - **direction**: 1 (positive drift), -1 (negative drift)
    """
    injection_id = await state.add_injection(
        InjectionType.DRIFT,
        {
            "sensor": params.sensor,
            "rate": params.rate * params.direction,
            "duration": params.duration,
        },
        duration=params.duration,
    )

    return InjectionResponse(
        success=True,
        message=f"Drift injection started on {params.sensor} (rate: {params.rate * params.direction}/s for {params.duration}s)",
        injection_id=injection_id,
        details={
            "sensor": params.sensor,
            "rate": params.rate * params.direction,
            "duration": params.duration,
        },
    )


@app.post("/inject/dropout", response_model=InjectionResponse)
async def inject_dropout(params: DropoutParams):
    """
    Simulate sensor signal dropout (NaN values).

    - **sensor**: Target sensor ID or 'random'
    - **duration**: Dropout duration in seconds
    """
    import random

    sensor = params.sensor if params.sensor != "random" else f"sensor_{random.randint(1, 15):02d}"

    injection_id = await state.add_injection(
        InjectionType.DROPOUT,
        {
            "sensor": sensor,
            "duration": params.duration,
        },
        duration=params.duration,
    )

    return InjectionResponse(
        success=True,
        message=f"Dropout injection started on {sensor} for {params.duration}s",
        injection_id=injection_id,
        details={
            "sensor": sensor,
            "duration": params.duration,
        },
    )


@app.post("/reset")
async def reset_injections():
    """Clear all active injections."""
    count = await state.clear_all()
    return {
        "success": True,
        "message": f"Cleared {count} injection(s)",
        "cleared_count": count,
    }


@app.delete("/inject/{injection_id}")
async def remove_injection(injection_id: str):
    """Remove a specific injection by ID."""
    removed = await state.remove_injection(injection_id)
    if removed:
        return {"success": True, "message": f"Injection {injection_id} removed"}
    raise HTTPException(status_code=404, detail=f"Injection {injection_id} not found")


if __name__ == "__main__":
    port = int(os.environ.get("PORT", "8084"))
    uvicorn.run(
        "main:app",
        host="0.0.0.0",
        port=port,
        reload=os.environ.get("DEBUG", "false").lower() == "true",
    )
