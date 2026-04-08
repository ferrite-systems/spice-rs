# Trace Export

For deep investigation of transient waveform divergences, the eval harness can export per-timestep data as JSON for offline analysis.

## Usage

```bash
cargo run --release --bin spice-eval -- --trace="Circuit Name" --output=trace.json
```

This runs the named circuit through both engines with full waveform capture and writes a JSON file containing all timestep data.

## Output format

The trace JSON contains:

### Per-timestep node voltages

```json
{
  "spice_rs": {
    "times": [0.0, 1e-9, 2e-9, ...],
    "nodes": {
      "v(out)": [0.0, 0.123, 0.456, ...],
      "v(in)": [0.0, 5.0, 5.0, ...],
      "v1#branch": [0.0, -1.23e-3, -2.34e-3, ...]
    }
  },
  "ngspice": {
    "times": [0.0, 1e-9, 2e-9, ...],
    "nodes": {
      "v(out)": [0.0, 0.123, 0.456, ...],
      "v(in)": [0.0, 5.0, 5.0, ...],
      "v1#branch": [0.0, -1.23e-3, -2.34e-3, ...]
    }
  }
}
```

Both engines produce waveforms at their own internally-chosen timesteps. The harness interpolates to common timepoints for comparison.

### Device state at divergent timesteps

When profiling is enabled, the trace includes per-device state at timesteps where divergence is detected:

```json
{
  "divergence_points": [
    {
      "time": 1.234e-6,
      "step_index": 47,
      "max_abs_err": 4.2e-3,
      "max_rel_err": 1.7e-3,
      "worst_node": "v(out)",
      "device_state": {
        "M1": {
          "Ids": 1.23e-3,
          "Vgs": 3.0,
          "Vth": 0.7,
          "gm": 2.46e-3,
          "gds": 1.23e-5,
          "Qgs": 1.5e-12,
          "Qgd": 0.8e-12
        }
      }
    }
  ]
}
```

The charge values (Qgs, Qgd, etc.) are particularly important for transient divergences, since charge integration errors accumulate over time.

### NR iteration snapshots (with profiling)

When the `--profile` flag or `SPICERS_PROFILE` env var is set, each NR iteration within divergent timesteps includes:

- RHS values before solve (device stamp output)
- Solution values after solve
- Matrix diagonal elements
- Per-device conductances and currents
- Noncon flag state

## Analysis workflow

### Waveform comparison

Load the JSON in Python/Julia/Matlab and plot corresponding signals from both engines:

```python
import json
import matplotlib.pyplot as plt

with open('trace.json') as f:
    data = json.load(f)

sr = data['spice_rs']
ng = data['ngspice']

plt.plot(sr['times'], sr['nodes']['v(out)'], label='spice-rs')
plt.plot(ng['times'], ng['nodes']['v(out)'], label='ngspice')
plt.legend()
plt.show()
```

Visual comparison immediately shows whether the divergence is a DC offset, a timing shift, a slope difference, or a completely wrong waveform.

### Timestep comparison

Compare the timestep sequences to detect integration differences:

```python
sr_dt = [t1 - t0 for t0, t1 in zip(sr['times'], sr['times'][1:])]
ng_dt = [t1 - t0 for t0, t1 in zip(ng['times'], ng['times'][1:])]
```

If spice-rs takes larger timesteps than ngspice, it may be underestimating truncation error. If it takes smaller timesteps, it may be overestimating (less concerning for accuracy, but affects performance).

### Charge tracking

For transient divergences that grow over time, compare the charge states at divergent timesteps. A small error in charge per step accumulates linearly with time. Common causes:

- Integration coefficient computation difference (`ni_com_cof`)
- Charge model evaluation difference (Meyer, BSIM3 charge model)
- Truncation error estimation difference (`ckt_terr`)

## Relationship to other diagnostic modes

| Mode | Scope | Detail level |
|------|-------|-------------|
| Default summary | All circuits, final values only | Low |
| `--detail` | All circuits, per-node comparison | Medium |
| `--diverge` | One circuit, first divergence point | Medium-high |
| `--diverge-deep` | One circuit, per-NR-iteration | Very high |
| `--trace` | One circuit, full waveform export | Full (offline analysis) |

Use the trace export when:
- The divergence is time-dependent and you need to see the full waveform shape
- You want to analyze the divergence in an external tool (plotting, curve fitting)
- The divergence-deep output is too detailed to scan by eye and you need to script the analysis
