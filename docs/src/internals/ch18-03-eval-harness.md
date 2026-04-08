# Eval Harness

The `spice-eval` crate (`sim/spice-eval/`) is the validation backbone of the port. It runs test circuits through both spice-rs and ngspice (via FFI), compares results, and reports divergences.

**Source:** `sim/spice-eval/src/main.rs`

## Architecture

```
spice-eval
├── src/main.rs          — CLI, comparison logic, report formatting
└── eval/
    ├── manifest.toml    — test circuit registry (name, file, tolerances)
    ├── results.json     — last run results (for regression detection)
    ├── dc/              — DC operating point circuits
    ├── tran/            — transient circuits
    ├── ac/              — AC analysis circuits
    └── ...
```

The harness initializes ngspice via `NgSpice::new()`, verifies the build tag (`ferrite-build-NNN`), and runs a constants parity check (a trivial diode circuit) to catch fundamental constant mismatches early.

For each test circuit, it:
1. Runs `spice_rs::runner::run_netlist(netlist)` to get spice-rs results.
2. Runs the same netlist through ngspice via FFI to get reference results.
3. Compares per-node values using the tolerance from the manifest.

## Tolerance rules

Standard tolerance for all tests: **abs=0.01, rel=0.01**. Layer 1 (passive) tests use tighter tolerances (abs=1e-9, rel=1e-6) since passives should be bit-identical.

The rule is absolute: **never loosen tolerances to make a test pass.** If a test fails, that's a bug in spice-rs. Fix the bug. The tolerance can only be tightened (to verify higher precision).

## CLI modes

### Summary mode (default)

```bash
cargo run --release --bin spice-eval
```

Runs all 224 circuits and prints a table:

```
┌─────────────────────────────────────────────────────┬──────────┬─────────────┬─────────────┐
│ Circuit                                             │  Status  │  Max AbsErr │  Max RelErr │
├───��──────────────────────────���──────────────────────��──────────┼─────���───────┼─────────────┤
│ [L1] Single Resistor                                │  PASS    │   0.000e+00 │   0.000e+00 │
│ [L3] Diode Forward DC                               │  PASS    │   1.421e-14 │   2.027e-14 │
│ ...                                                 │          │             │             │
└──────────────────────────────────────────��──────────┴──────────┴───────��─────┴─────────────┘

200 passed, 3 failed, 3 errors, 0 skipped
```

Status values:
- **PASS** — all nodes within tolerance
- **FAIL** — at least one node exceeds tolerance
- **ERROR** — spice-rs panicked or returned an error
- **NG-ERR** — ngspice failed (not a spice-rs problem)

### Filter mode

```bash
cargo run --release --bin spice-eval -- --filter=mosfet
```

Run only circuits whose name matches the substring (case-insensitive).

### Detail mode

```bash
cargo run --release --bin spice-eval -- --detail
```

Show per-node comparison for all circuits, not just failures.

### Diverge mode

```bash
cargo run --release --bin spice-eval -- --diverge="Circuit Name"
```

For transient circuits: run both engines, find the first timepoint where divergence exceeds tolerance, and show per-node comparison at that point. For DC: show full operating point comparison with per-device state.

### Diverge-deep mode

```bash
cargo run --release --bin spice-eval -- --diverge-deep="Circuit Name"
```

Per-NR-iteration comparison. Runs both engines with profiling enabled and compares:
- RHS vectors before solve (device stamp output)
- Solution vectors after solve
- Per-device conductances
- Per-device stored currents (Ids, Ibs, Ibd for MOSFETs)
- Per-device limited voltages
- Noncon flags

This is the most detailed diagnostic mode. It identifies the exact NR iteration where the engines diverge and which device is responsible.

### Profile mode

```bash
cargo run --release --bin spice-eval -- --profile="Circuit Name"
```

Capture NR iteration snapshots from spice-rs for a single circuit. Shows the convergence trajectory.

### Parameter check mode

```bash
cargo run --release --bin spice-eval -- --check-params
```

Compare parsed model parameters between spice-rs and ngspice. This catches parser bugs where a model parameter (e.g., BSIM3's `VTH0`) is not reaching the device correctly. Parser bugs are the highest-impact failures — they masquerade as model accuracy problems but are trivially fixable.

### Translate check mode

```bash
cargo run --release --bin spice-eval -- --check-translate
```

Compare the TRANSLATE external-to-internal node remapping between both engines. Node ordering must match ngspice exactly — different internal ordering produces different pivot sequences in the Markowitz solver, which can produce different (though numerically equivalent) results.

## Regression detection

When run without `--filter`, the harness saves `results.json` and compares against the previous run. If any previously-passing circuit now fails, it prints a regression warning. This prevents fixing one device model from silently breaking another.

## Test circuit manifest

The `manifest.toml` file registers each circuit:

```toml
[[circuit]]
name = "[L3] Diode Forward DC"
file = "dc/L3_diode_fwd.cir"
[circuit.tolerances]
abs = 0.01
rel = 0.01
```

Circuits are organized by complexity layer (L1-L6+). See [Test Circuits](ch20-01-test-circuits.md) for the full inventory.
