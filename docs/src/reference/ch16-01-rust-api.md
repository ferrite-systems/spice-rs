# Rust API

Add spice-rs as a dependency:

```toml
[dependencies]
spice-rs = { path = "../sim/spice-rs" }
```

## High-level runner functions

All runner functions accept a netlist string and return a `Result`.

### `run_netlist`

```rust
pub fn run_netlist(netlist: &str) -> Result<(HashMap<String, f64>, Analysis), String>
```

Runs whatever analysis the netlist specifies (`.OP`, `.TRAN`, `.AC`, `.DC`, `.SENS`, `.TF`, `.PZ`). Returns node voltages/branch currents as a HashMap, plus an `Analysis` enum indicating which analysis ran.

For `.TRAN`, the returned HashMap contains the values at the **last** timepoint.

### `run_netlist_tran_waveform`

```rust
pub fn run_netlist_tran_waveform(netlist: &str)
    -> Result<(Vec<String>, TransientResult), String>
```

Runs transient analysis and returns full waveforms. The `Vec<String>` contains signal names. `TransientResult` has:

- `times: Vec<f64>` -- time points
- `values: Vec<Vec<f64>>` -- one row per timepoint, one column per signal
- `accepted: usize` -- number of accepted timesteps
- `rejected: usize` -- number of rejected timesteps

### `run_netlist_dc_sweep`

```rust
pub fn run_netlist_dc_sweep(netlist: &str) -> Result<DcSweepWaveform, String>
```

Runs a `.DC` sweep analysis. `DcSweepWaveform` has:

- `sweep_values: Vec<f64>` -- swept parameter values
- `signals: HashMap<String, Vec<f64>>` -- signal name to waveform
- `names: Vec<String>` -- signal names

### `run_netlist_ac`

```rust
pub fn run_netlist_ac(netlist: &str) -> Result<AcWaveform, String>
```

Runs `.AC` analysis. `AcWaveform` has:

- `frequencies: Vec<f64>` -- frequency points
- `signals_re: HashMap<String, Vec<f64>>` -- real part
- `signals_im: HashMap<String, Vec<f64>>` -- imaginary part
- `names: Vec<String>` -- signal names

### `run_netlist_params`

```rust
pub fn run_netlist_params(netlist: &str)
    -> Result<Vec<(String, Vec<(String, f64)>)>, String>
```

Returns device operating-point parameters after DC analysis. Each entry is `(device_name, [(param_name, value), ...])`.

### `run_netlist_dc_op_profiled`

```rust
pub fn run_netlist_dc_op_profiled(netlist: &str)
    -> Result<(HashMap<String, f64>, Vec<NrSnapshot>), String>
```

Runs DC operating point and returns NR iteration snapshots for debugging convergence.

## Complete example

```rust
use spice_rs::runner::run_netlist;

fn main() {
    let netlist = "\
Voltage Divider
V1 vdd 0 DC 3.3
R1 vdd mid 10K
R2 mid 0 10K
.OP
.END
";

    match run_netlist(netlist) {
        Ok((voltages, _analysis)) => {
            for (node, value) in &voltages {
                println!("{}: {:.4} V", node, value);
            }
            // Output:
            //   vdd: 3.3000 V
            //   mid: 1.6500 V
        }
        Err(e) => eprintln!("Simulation error: {}", e),
    }
}
```

## Transient example

```rust
use spice_rs::runner::run_netlist_tran_waveform;

fn main() {
    let netlist = "\
RC Step Response
V1 in 0 PULSE(0 1 0 1N 1N 10U 20U)
R1 in out 1K
C1 out 0 1N
.TRAN 10N 20U
.END
";

    let (names, result) = run_netlist_tran_waveform(netlist).unwrap();
    println!("Signals: {:?}", names);
    println!("Timepoints: {}", result.times.len());
    println!("Accepted: {}, Rejected: {}", result.accepted, result.rejected);

    // Access the voltage at the last timepoint
    let last = result.values.last().unwrap();
    for (i, name) in names.iter().enumerate() {
        println!("{}: {:.6}", name, last[i]);
    }
}
```
