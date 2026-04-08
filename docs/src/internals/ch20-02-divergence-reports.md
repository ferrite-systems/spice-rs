# Divergence Reports

When a test circuit fails, the eval harness provides several levels of diagnostic output to identify the root cause.

## Basic failure output

In the default summary table, a failed circuit shows:

```
│ [L3] NMOS Level 1 DC                               │  FAIL    │   2.341e-03 │   1.872e-02 │
│   ! v(drain)             sr=   4.98123  ng=   4.97889  abs=2.341e-03  rel=4.704e-04
│     v(gate)              sr=   3.00000  ng=   3.00000  abs=0.000e+00  rel=0.000e+00
│     v(source)            sr=   0.00000  ng=   0.00000  abs=0.000e+00  rel=0.000e+00
```

Each row shows:
- `!` flag if this node exceeds tolerance
- Node name
- `sr=` spice-rs value
- `ng=` ngspice value
- `abs=` absolute error
- `rel=` relative error

This immediately tells you which node is wrong and by how much.

## Diverge mode: `--diverge`

```bash
cargo run --release --bin spice-eval -- --diverge="Circuit Name"
```

For **transient circuits**, this mode runs both engines with full waveform capture and identifies the first timepoint where divergence exceeds tolerance:

```
Divergence analysis: [L5] CMOS Inverter Tran
First divergence at t=1.234e-06 (step 47 of 200)
  v(out)   sr=2.4531   ng=2.4489   abs=4.2e-03   rel=1.7e-03

Per-device state at divergence point:
  M1 (NMOS): Ids=1.23e-03  Vgs=3.000  Vth=0.700  gm=2.46e-03  gds=1.23e-05
  M2 (PMOS): Ids=-1.23e-03 Vgs=-2.000 Vth=-0.700 gm=2.46e-03  gds=1.23e-05

Slope comparison (dv/dt at divergence):
  v(out)   sr_slope=1.23e+06   ng_slope=1.22e+06   ratio=1.008
```

The per-device state shows the internal device variables (Ids, Vgs, Vth, gm, gds) at the divergent timestep. Comparing these between engines pinpoints which device is computing differently.

The slope comparison detects cases where the waveforms agree on value but are diverging in trend — a precursor to larger errors at later timesteps.

For **DC circuits**, diverge mode shows the full operating point comparison with all device parameters.

## Diverge-deep mode: `--diverge-deep`

```bash
cargo run --release --bin spice-eval -- --diverge-deep="Circuit Name"
```

This is the most detailed diagnostic. It enables profiling on both engines and compares at the NR-iteration level:

```
NR iteration comparison at t=1.234e-06, iter=3:
  RHS before solve (device stamps):
    eq[1] sr= 1.23456789e-03  ng= 1.23456789e-03  diff=0.000e+00
    eq[2] sr=-4.56789012e-04  ng=-4.56789013e-04  diff=1.000e-12  ← first diff
    eq[3] sr= 0.00000000e+00  ng= 0.00000000e+00  diff=0.000e+00

  Solution after solve:
    eq[1] sr= 2.99999999      ng= 3.00000000       diff=1.000e-08
    eq[2] sr= 1.23456780      ng= 1.23456789       diff=9.000e-08  ← amplified

  Per-device conductances:
    M1: gm=2.460000e-03 gds=1.230000e-05   (match)
    M2: gm=2.460001e-03 gds=1.230000e-05   (gm diff=1e-09)

  Per-device stored currents:
    M1: Ids=1.23456e-03 Ibs=-1.23e-14 Ibd=-4.56e-14
    M2: Ids=-1.23456e-03 Ibs=1.23e-14 Ibd=4.56e-14

  noncon: sr=0  ng=0
```

This output identifies exactly which NR iteration first shows a divergence, whether the divergence originates in the device stamps (RHS before solve) or the solver (solution after solve), and which device is responsible.

**Interpreting diverge-deep output:**

1. **RHS matches but solution diverges** — solver bug (pivot ordering, factorization error).
2. **RHS diverges at a specific equation** — the device stamping that equation is computing differently. Cross-reference the equation number with the circuit's node map to identify the device.
3. **Device conductance diverges** — the device model is computing a different linearization. Compare against the ngspice C code for that device's load function.
4. **Device stored current diverges** — the device model's I-V evaluation is different. Often caused by a voltage limiting difference or a missed mode flag check.

## Parameter check mode: `--check-params`

```bash
cargo run --release --bin spice-eval -- --check-params
```

Compares parsed model parameters (not simulation results) between spice-rs and ngspice:

```
Parameter check: [L3] BSIM3 NMOS DC
  M1 model parameters:
    VTH0    sr= 0.4376260  ng= 0.4376260  MATCH
    K1      sr= 0.5613000  ng= 0.5613000  MATCH
    K2      sr=-0.0861000  ng=-0.0861000  MATCH
    K3      sr= 80.000000  ng= 80.000000  MATCH
    TOXE    sr= 0.0000000  ng= 1.000e-08  MISMATCH ← parser bug!
```

A parameter mismatch means the parser is not passing the value through to the device. This is the highest-impact failure mode: the device operates with a wrong parameter, producing wrong results that look like a model accuracy problem. Fix the parser before debugging the model.

## Translate check mode: `--check-translate`

```bash
cargo run --release --bin spice-eval -- --check-translate
```

Compares the TRANSLATE external-to-internal node mapping. If these differ, the Markowitz solver sees the matrix in a different order, potentially choosing different pivots:

```
Translate check: [L3] NMOS Level 1 DC
  ext→int mapping:
    ext[1]=int[1]  ext[2]=int[2]  ext[3]=int[3]  ext[4]=int[4]
  ngspice: [1, 2, 3, 4]
  spice-rs: [1, 2, 3, 4]
  MATCH
```

TRANSLATE mismatches are subtle: they don't cause wrong answers directly, but they can cause different convergence behavior because the pivot ordering changes.

## Debugging workflow

When a circuit fails:

1. **Run `--check-params`** to rule out parser bugs.
2. **Run `--diverge`** to find where the divergence starts and which device.
3. **Run `--diverge-deep`** to find the exact NR iteration and whether it's a stamp or solver issue.
4. **Read the ngspice C code** for the identified device/function and compare against the Rust translation.
5. **Fix and re-run.** Verify the fix doesn't regress other circuits.
