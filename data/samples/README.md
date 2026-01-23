# ALEC Sample Datasets

Realistic IoT sensor data for benchmarking ALEC compression.

## Datasets

| File | Sensors | Samples | Interval | Duration |
|------|---------|---------|----------|----------|
| `hvac_24h.csv` | temperature, humidity | 1,440 | 1 min | 24 hours |
| `smartgrid_24h.csv` | voltage, current, power_factor | 8,640 | 10 sec | 24 hours |
| `industrial_1h.csv` | vibration, rpm | 3,600 | 1 sec | 1 hour |

## Data Characteristics

| Dataset | Column | Unchanged % | Unique Values | Pattern |
|---------|--------|-------------|---------------|---------|
| HVAC | temperature | 78% | 23 | Stable with daily cycle |
| HVAC | humidity | 60% | 17 | Inverse correlation with temp |
| SmartGrid | voltage | 82% | 21 | Very stable (grid regulated) |
| SmartGrid | current | **8.7%** | 673 | Variable (load dependent) |
| SmartGrid | power_factor | 100% | 1 | Constant |
| Industrial | vibration | 90% | 21 | Stable with event spikes |

## Regenerating Data

```bash
cd ../benches/comparison
python generate_samples.py
```

## Notes

- Data is synthetic but modeled on real IoT patterns
- The `current` column in SmartGrid is the most challenging (highly variable)
- Use these datasets to compare ALEC's performance across different data types
