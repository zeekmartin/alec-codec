# ALEC Comparison Benchmarks

Python-based benchmarks comparing ALEC with general-purpose compression codecs.

## Quick Start

```bash
# Install dependencies
pip install -r requirements.txt

# Generate sample data (if not present in data/samples/)
python generate_samples.py

# Run full benchmark suite
python benchmark_comparison.py

# Run focused benchmark on variable data (recommended)
python benchmark_variable.py
```

## Scripts

| Script | Purpose |
|--------|---------|
| `generate_samples.py` | Generate realistic IoT sensor data |
| `benchmark_comparison.py` | Compare ALEC vs gzip/zlib/lz4/zstd |
| `benchmark_variable.py` | Focused test on variable data (best for demos) |

## Key Results

On variable IoT data (SmartGrid current, 8.7% unchanged readings):

| Condition | ALEC | gzip | Advantage |
|-----------|------|------|-----------|
| Cold start | 10.9x | 5.1x | **+113%** |
| With preload | 22.1x | 8.0x | **+177%** |

## Output

Results are saved to:
- `../../results/figures/` - PNG plots
- `../../results/benchmark_results.json` - Raw data
- `../../results/BENCHMARK_RESULTS.md` - Markdown summary

## Note on ALEC Simulation

These benchmarks use a Python simulator that models ALEC's encoding behavior:
- Delta encoding with varint
- Pattern dictionary lookup
- Context evolution

For production benchmarks, use the Rust implementation:
```bash
cargo bench --bench encoding
```
