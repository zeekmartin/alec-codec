#!/usr/bin/env python3
"""
ALEC Demo - Correlated Dataset Generator

Generates realistic correlated sensor data for agricultural IoT scenarios.
Uses latent variables to create realistic inter-sensor correlations.

Usage:
    python generate_dataset.py --sensors 15 --samples 3000 --output datasets/baseline.csv
    python generate_dataset.py --sensors 15 --samples 3000 --anomaly-rate 0.02 --output datasets/anomalies.csv
    python generate_dataset.py --sensors 15 --samples 3000 --correlation-matrix datasets/corr.csv
"""

import argparse
import sys
from datetime import datetime, timedelta
from pathlib import Path
from typing import Optional, Tuple, Dict, List
import numpy as np

try:
    import pandas as pd
except ImportError:
    print("Error: pandas is required. Install with: pip install pandas")
    sys.exit(1)

from sensors import SENSORS, SensorConfig


class LatentVariableGenerator:
    """Generates latent variables that drive sensor correlations."""

    def __init__(self, n_samples: int, seed: Optional[int] = None):
        self.n_samples = n_samples
        self.rng = np.random.default_rng(seed)
        self.latent_vars: Dict[str, np.ndarray] = {}
        self._generate_latent_variables()

    def _generate_latent_variables(self):
        """Generate all latent variables."""
        t = np.linspace(0, 1, self.n_samples)

        # Weather patterns (slow changes over hours/days)
        weather_noise = self.rng.normal(0, 0.1, self.n_samples)
        self.latent_vars["weather"] = (
            np.sin(2 * np.pi * t * 2)  # ~2 weather cycles
            + 0.5 * np.sin(2 * np.pi * t * 5)  # Higher frequency component
            + np.cumsum(weather_noise) * 0.01  # Random walk component
        )
        # Normalize to [-1, 1]
        self.latent_vars["weather"] = self._normalize(self.latent_vars["weather"])

        # Daily cycle (follows sun pattern)
        daily_cycles = t * 24  # Assume 24 hours of data per unit
        self.latent_vars["daily_cycle"] = np.sin(2 * np.pi * daily_cycles)

        # Seasonal trend (slow drift)
        self.latent_vars["seasonal"] = np.sin(2 * np.pi * t * 0.5) * 0.5 + t * 0.3

        # Wind gusts (sporadic)
        gusts = self.rng.exponential(0.3, self.n_samples)
        gusts[gusts > 1] = 1
        self.latent_vars["gusts"] = gusts * np.sign(self.rng.normal(0, 1, self.n_samples))

        # Irrigation events (periodic pulses)
        irrigation = np.zeros(self.n_samples)
        irrigation_times = np.linspace(0, self.n_samples, 8, dtype=int)[1:-1]
        for it in irrigation_times:
            start = max(0, it - 50)
            end = min(self.n_samples, it + 100)
            irrigation[start:end] = np.exp(-np.abs(np.arange(end - start) - 50) / 30)
        self.latent_vars["irrigation"] = irrigation

    def _normalize(self, arr: np.ndarray) -> np.ndarray:
        """Normalize array to [-1, 1] range."""
        min_val, max_val = arr.min(), arr.max()
        if max_val - min_val < 1e-10:
            return np.zeros_like(arr)
        return 2 * (arr - min_val) / (max_val - min_val) - 1

    def get(self, name: str) -> np.ndarray:
        """Get a latent variable by name."""
        return self.latent_vars.get(name, np.zeros(self.n_samples))


class DatasetGenerator:
    """Generates correlated sensor datasets."""

    def __init__(
        self,
        sensors: List[SensorConfig],
        n_samples: int = 3000,
        seed: Optional[int] = None,
        start_time: Optional[datetime] = None,
        sample_interval_seconds: int = 1,
    ):
        self.sensors = sensors
        self.n_samples = n_samples
        self.seed = seed
        self.rng = np.random.default_rng(seed)
        self.start_time = start_time or datetime(2026, 1, 1, 0, 0, 0)
        self.sample_interval = timedelta(seconds=sample_interval_seconds)
        self.latent = LatentVariableGenerator(n_samples, seed)

    def generate(self) -> pd.DataFrame:
        """Generate the dataset."""
        # Generate timestamps
        timestamps = [
            self.start_time + i * self.sample_interval
            for i in range(self.n_samples)
        ]

        # Generate sensor data
        data = {"timestamp": timestamps}

        for sensor in self.sensors:
            values = self._generate_sensor_values(sensor)
            col_name = f"{sensor.id}_{sensor.type}"
            data[col_name] = values

        return pd.DataFrame(data)

    def _generate_sensor_values(self, sensor: SensorConfig) -> np.ndarray:
        """Generate values for a single sensor using latent variables."""
        # Start with base value
        values = np.full(self.n_samples, sensor.base)

        # Add contributions from latent variables
        for latent_name, weight in sensor.latent_weights.items():
            latent_values = self.latent.get(latent_name)
            values = values + weight * latent_values

        # Add sensor-specific noise
        noise = self.rng.normal(0, sensor.noise_std, self.n_samples)
        values = values + noise

        # Clip to valid range
        values = np.clip(values, sensor.min_val, sensor.max_val)

        return values

    def inject_anomalies(
        self,
        df: pd.DataFrame,
        anomaly_rate: float = 0.02,
        anomaly_types: Optional[List[str]] = None,
    ) -> Tuple[pd.DataFrame, pd.DataFrame]:
        """
        Inject anomalies into the dataset.

        Returns:
            Tuple of (modified_df, anomaly_log_df)
        """
        if anomaly_types is None:
            anomaly_types = ["spike", "drift", "dropout"]

        df = df.copy()
        anomaly_log = []

        n_anomalies = int(self.n_samples * anomaly_rate)
        sensor_cols = [c for c in df.columns if c != "timestamp"]

        for _ in range(n_anomalies):
            anomaly_type = self.rng.choice(anomaly_types)
            target_col = self.rng.choice(sensor_cols)
            idx = self.rng.integers(0, self.n_samples)

            if anomaly_type == "spike":
                # Single point spike
                magnitude = self.rng.uniform(3, 10) * df[target_col].std()
                direction = self.rng.choice([-1, 1])
                original = df.loc[idx, target_col]
                df.loc[idx, target_col] = original + direction * magnitude
                anomaly_log.append({
                    "timestamp": df.loc[idx, "timestamp"],
                    "sensor": target_col,
                    "type": "spike",
                    "original": original,
                    "modified": df.loc[idx, target_col],
                })

            elif anomaly_type == "drift":
                # Progressive drift over a window
                window = min(100, self.n_samples - idx)
                drift_rate = self.rng.uniform(0.01, 0.05) * df[target_col].std()
                direction = self.rng.choice([-1, 1])
                for i in range(window):
                    if idx + i < self.n_samples:
                        df.loc[idx + i, target_col] += direction * drift_rate * i
                anomaly_log.append({
                    "timestamp": df.loc[idx, "timestamp"],
                    "sensor": target_col,
                    "type": "drift",
                    "duration": window,
                    "rate": drift_rate * direction,
                })

            elif anomaly_type == "dropout":
                # Signal dropout (NaN values)
                window = self.rng.integers(5, 30)
                for i in range(window):
                    if idx + i < self.n_samples:
                        df.loc[idx + i, target_col] = np.nan
                anomaly_log.append({
                    "timestamp": df.loc[idx, "timestamp"],
                    "sensor": target_col,
                    "type": "dropout",
                    "duration": window,
                })

        anomaly_df = pd.DataFrame(anomaly_log)
        return df, anomaly_df


def compute_correlation_matrix(df: pd.DataFrame) -> pd.DataFrame:
    """Compute Pearson correlation matrix for sensor columns."""
    sensor_cols = [c for c in df.columns if c != "timestamp"]
    return df[sensor_cols].corr(method="pearson")


def main():
    parser = argparse.ArgumentParser(
        description="Generate correlated sensor dataset for ALEC demo"
    )
    parser.add_argument(
        "--sensors",
        type=int,
        default=15,
        help="Number of sensors to generate (default: 15)",
    )
    parser.add_argument(
        "--samples",
        type=int,
        default=3000,
        help="Number of samples to generate (default: 3000)",
    )
    parser.add_argument(
        "--output",
        type=str,
        default="datasets/baseline.csv",
        help="Output CSV file path",
    )
    parser.add_argument(
        "--anomaly-rate",
        type=float,
        default=0.0,
        help="Anomaly injection rate (0.0-1.0, default: 0.0)",
    )
    parser.add_argument(
        "--correlation-matrix",
        type=str,
        default=None,
        help="Output file for correlation matrix CSV",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=42,
        help="Random seed for reproducibility (default: 42)",
    )
    parser.add_argument(
        "--interval",
        type=int,
        default=1,
        help="Sample interval in seconds (default: 1)",
    )
    parser.add_argument(
        "--start-time",
        type=str,
        default=None,
        help="Start timestamp (ISO format, default: 2026-01-01T00:00:00)",
    )
    parser.add_argument(
        "--format",
        type=str,
        choices=["csv", "json", "parquet"],
        default="csv",
        help="Output format (default: csv)",
    )
    parser.add_argument(
        "-v", "--verbose",
        action="store_true",
        help="Verbose output",
    )

    args = parser.parse_args()

    # Limit sensors if requested
    sensors = SENSORS[:args.sensors]

    # Parse start time
    start_time = None
    if args.start_time:
        start_time = datetime.fromisoformat(args.start_time)

    if args.verbose:
        print(f"Generating dataset with {len(sensors)} sensors, {args.samples} samples")
        print(f"Seed: {args.seed}, Interval: {args.interval}s")

    # Generate dataset
    generator = DatasetGenerator(
        sensors=sensors,
        n_samples=args.samples,
        seed=args.seed,
        start_time=start_time,
        sample_interval_seconds=args.interval,
    )

    df = generator.generate()

    # Inject anomalies if requested
    anomaly_df = None
    if args.anomaly_rate > 0:
        if args.verbose:
            print(f"Injecting anomalies at rate {args.anomaly_rate}")
        df, anomaly_df = generator.inject_anomalies(df, args.anomaly_rate)

    # Ensure output directory exists
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    # Save dataset
    if args.format == "csv":
        df.to_csv(output_path, index=False)
    elif args.format == "json":
        df.to_json(output_path, orient="records", date_format="iso")
    elif args.format == "parquet":
        df.to_parquet(output_path, index=False)

    if args.verbose:
        print(f"Dataset saved to: {output_path}")
        print(f"Shape: {df.shape}")

    # Save anomaly log if generated
    if anomaly_df is not None and len(anomaly_df) > 0:
        anomaly_path = output_path.with_suffix(".anomalies.csv")
        anomaly_df.to_csv(anomaly_path, index=False)
        if args.verbose:
            print(f"Anomaly log saved to: {anomaly_path}")
            print(f"Total anomalies: {len(anomaly_df)}")

    # Compute and save correlation matrix if requested
    if args.correlation_matrix:
        corr_path = Path(args.correlation_matrix)
        corr_path.parent.mkdir(parents=True, exist_ok=True)
        corr_matrix = compute_correlation_matrix(df)
        corr_matrix.to_csv(corr_path)
        if args.verbose:
            print(f"Correlation matrix saved to: {corr_path}")
            print("\nCorrelation Matrix Summary:")
            print(f"  Strong correlations (|r| > 0.5): {(corr_matrix.abs() > 0.5).sum().sum() // 2 - len(sensors)}")
            print(f"  Moderate correlations (0.3 < |r| < 0.5): {((corr_matrix.abs() > 0.3) & (corr_matrix.abs() <= 0.5)).sum().sum() // 2}")

    # Print summary
    if args.verbose:
        print("\nDataset Summary:")
        print(df.describe().to_string())

    return 0


if __name__ == "__main__":
    sys.exit(main())
