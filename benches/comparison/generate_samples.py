#!/usr/bin/env python3
"""
ALEC Sample Data Generator
Generates realistic IoT sensor data for benchmarking

Patterns modeled:
- HVAC: Temperature stays stable for long periods, slow drift
- SmartGrid: Voltage with small fluctuations around nominal
- Industrial: Vibration with periodic maintenance spikes
"""

import csv
import math
import random
from pathlib import Path
from datetime import datetime, timedelta

def generate_hvac_data(duration_hours: int = 24, interval_seconds: int = 60) -> list:
    """
    Generate realistic HVAC temperature and humidity data.
    
    Characteristics:
    - Temperature: Very stable (22°C ± 0.5°C), slow daily cycle
    - Humidity: More variable (45% ± 5%), follows temperature inversely
    - Long periods of IDENTICAL values (key for ALEC's advantage)
    """
    samples = []
    num_samples = (duration_hours * 3600) // interval_seconds
    
    base_temp = 22.0
    base_humidity = 45.0
    
    current_temp = base_temp
    current_humidity = base_humidity
    
    start_time = datetime(2024, 1, 15, 0, 0, 0)
    
    for i in range(num_samples):
        timestamp = start_time + timedelta(seconds=i * interval_seconds)
        hour = timestamp.hour + timestamp.minute / 60.0
        
        # Daily temperature cycle (warmer afternoon)
        daily_cycle = 0.5 * math.sin((hour - 6) * math.pi / 12)
        
        # Temperature changes are RARE and SMALL
        # 80% of the time: no change at all
        # 15% of the time: ±0.1°C
        # 5% of the time: ±0.2°C (thermostat adjustment)
        rand = random.random()
        if rand < 0.80:
            temp_change = 0.0
        elif rand < 0.95:
            temp_change = random.choice([-0.1, 0.1])
        else:
            temp_change = random.choice([-0.2, 0.2])
        
        target_temp = base_temp + daily_cycle
        current_temp = current_temp * 0.95 + target_temp * 0.05 + temp_change
        current_temp = round(current_temp, 1)  # Sensor precision
        
        # Humidity inversely correlated, more variable
        rand = random.random()
        if rand < 0.60:
            hum_change = 0.0
        elif rand < 0.85:
            hum_change = random.choice([-1, 1])
        else:
            hum_change = random.uniform(-2, 2)
        
        target_humidity = base_humidity - (current_temp - base_temp) * 3
        current_humidity = current_humidity * 0.9 + target_humidity * 0.1 + hum_change
        current_humidity = round(max(30, min(70, current_humidity)), 0)
        
        samples.append({
            'timestamp': timestamp.isoformat(),
            'temperature': current_temp,
            'humidity': int(current_humidity)
        })
    
    return samples


def generate_smartgrid_data(duration_hours: int = 24, interval_seconds: int = 10) -> list:
    """
    Generate realistic smart grid voltage/current data.
    
    Characteristics:
    - Voltage: Very stable around 230V (±2V), rare fluctuations
    - Current: Variable based on load, but often stable
    - Power factor: Usually constant ~0.95
    """
    samples = []
    num_samples = (duration_hours * 3600) // interval_seconds
    
    base_voltage = 230.0
    base_current = 15.0
    
    current_voltage = base_voltage
    current_current = base_current
    
    start_time = datetime(2024, 1, 15, 0, 0, 0)
    
    for i in range(num_samples):
        timestamp = start_time + timedelta(seconds=i * interval_seconds)
        hour = timestamp.hour + timestamp.minute / 60.0
        
        # Load profile: higher during day, peaks at 7-9 and 18-21
        if 7 <= hour <= 9 or 18 <= hour <= 21:
            load_factor = 1.3
        elif 9 <= hour <= 18:
            load_factor = 1.1
        elif 0 <= hour <= 6:
            load_factor = 0.7
        else:
            load_factor = 0.9
        
        # Voltage: very stable, rare small changes
        # Grid voltage is regulated - 90% identical readings
        rand = random.random()
        if rand < 0.90:
            voltage_change = 0.0
        elif rand < 0.98:
            voltage_change = random.choice([-0.1, 0.1, -0.2, 0.2])
        else:
            voltage_change = random.uniform(-1, 1)
        
        current_voltage = base_voltage + voltage_change
        current_voltage = round(current_voltage, 1)
        
        # Current: follows load profile, more variable
        target_current = base_current * load_factor
        rand = random.random()
        if rand < 0.70:
            current_change = 0.0
        else:
            current_change = random.uniform(-0.5, 0.5)
        
        current_current = current_current * 0.8 + target_current * 0.2 + current_change
        current_current = round(max(0.5, current_current), 2)
        
        samples.append({
            'timestamp': timestamp.isoformat(),
            'voltage': current_voltage,
            'current': current_current,
            'power_factor': 0.95
        })
    
    return samples


def generate_industrial_data(duration_hours: int = 24, interval_seconds: int = 1) -> list:
    """
    Generate realistic industrial vibration sensor data.
    
    Characteristics:
    - Vibration: Stable during normal operation, spikes during events
    - Very high sample rate, mostly identical values
    - Occasional maintenance/anomaly events
    """
    samples = []
    num_samples = min((duration_hours * 3600) // interval_seconds, 86400)  # Cap at 1 day
    
    base_vibration = 2.5  # mm/s RMS
    current_vibration = base_vibration
    
    start_time = datetime(2024, 1, 15, 0, 0, 0)
    
    # Schedule some events
    events = [
        (3600 * 3, 3600 * 3.5, 8.0),   # Startup event at 3am
        (3600 * 8, 3600 * 8.1, 15.0),  # Anomaly at 8am
        (3600 * 14, 3600 * 14.5, 6.0), # Maintenance at 2pm
    ]
    
    for i in range(num_samples):
        timestamp = start_time + timedelta(seconds=i * interval_seconds)
        elapsed = i * interval_seconds
        
        # Check for events
        event_factor = 1.0
        for start, end, peak in events:
            if start <= elapsed <= end:
                progress = (elapsed - start) / (end - start)
                event_factor = 1.0 + (peak / base_vibration - 1) * math.sin(progress * math.pi)
                break
        
        # Normal operation: 95% identical, 5% tiny variation
        rand = random.random()
        if rand < 0.95:
            noise = 0.0
        else:
            noise = random.uniform(-0.1, 0.1)
        
        current_vibration = base_vibration * event_factor + noise
        current_vibration = round(max(0.1, current_vibration), 2)
        
        samples.append({
            'timestamp': timestamp.isoformat(),
            'vibration': current_vibration,
            'rpm': 1500 if event_factor < 1.5 else int(1500 * (1 + (event_factor - 1) * 0.1))
        })
    
    return samples


def save_csv(data: list, filepath: Path):
    """Save data to CSV file."""
    if not data:
        return
    
    filepath.parent.mkdir(parents=True, exist_ok=True)
    
    with open(filepath, 'w', newline='') as f:
        writer = csv.DictWriter(f, fieldnames=data[0].keys())
        writer.writeheader()
        writer.writerows(data)
    
    print(f"✓ Saved {len(data)} samples to {filepath}")


def analyze_data(data: list, name: str):
    """Print statistics about generated data."""
    if not data:
        return
    
    print(f"\n{'='*50}")
    print(f"Dataset: {name}")
    print(f"{'='*50}")
    print(f"Samples: {len(data)}")
    
    # Analyze each numeric column
    numeric_keys = [k for k in data[0].keys() if k != 'timestamp' and isinstance(data[0][k], (int, float))]
    
    for key in numeric_keys:
        values = [d[key] for d in data]
        
        # Count unchanged values
        unchanged = sum(1 for i in range(1, len(values)) if values[i] == values[i-1])
        unchanged_pct = (unchanged / (len(values) - 1)) * 100 if len(values) > 1 else 0
        
        # Count unique values
        unique = len(set(values))
        
        print(f"\n  {key}:")
        print(f"    Range: {min(values):.2f} - {max(values):.2f}")
        print(f"    Unique values: {unique}")
        print(f"    Unchanged readings: {unchanged_pct:.1f}%")
        
        # Raw size
        raw_bytes = len(values) * 8  # float64
        print(f"    Raw size: {raw_bytes:,} bytes ({raw_bytes/1024:.1f} KB)")


def main():
    output_dir = Path("./data/samples")
    
    print("ALEC Sample Data Generator")
    print("=" * 50)
    
    # Generate HVAC data (24h, 1 reading/minute = 1440 samples)
    print("\nGenerating HVAC data...")
    hvac = generate_hvac_data(duration_hours=24, interval_seconds=60)
    save_csv(hvac, output_dir / "hvac_24h.csv")
    analyze_data(hvac, "HVAC")
    
    # Generate SmartGrid data (24h, 1 reading/10s = 8640 samples)
    print("\nGenerating SmartGrid data...")
    smartgrid = generate_smartgrid_data(duration_hours=24, interval_seconds=10)
    save_csv(smartgrid, output_dir / "smartgrid_24h.csv")
    analyze_data(smartgrid, "SmartGrid")
    
    # Generate Industrial data (1h at 1Hz = 3600 samples)
    print("\nGenerating Industrial data...")
    industrial = generate_industrial_data(duration_hours=1, interval_seconds=1)
    save_csv(industrial, output_dir / "industrial_1h.csv")
    analyze_data(industrial, "Industrial")
    
    print("\n" + "=" * 50)
    print("✓ All sample data generated!")
    print(f"  Output directory: {output_dir.absolute()}")


if __name__ == "__main__":
    main()
