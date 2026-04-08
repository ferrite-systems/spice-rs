# Test Circuits

The eval harness contains 224 test circuits registered in `sim/spice-eval/eval/manifest.toml`. They are organized in six complexity layers, each building on the previous.

## Layer structure

### L1: Single passive components (5 circuits)

The simplest circuits: one device, one analysis.

- Single Resistor — voltage divider, verifies Ohm's law and MNA setup
- 3-Resistor Divider — multi-node DC, verifies node voltage extraction
- Capacitor DC — capacitor as open circuit in DC
- Inductor DC — inductor as short circuit in DC
- VCVS Gain — voltage-controlled voltage source, verifies dependent source stamps

**Tolerance:** abs=1e-9, rel=1e-6. These must be bit-identical.

### L2: Passive combinations (6 circuits)

Multi-component circuits exercising AC and transient analysis engines.

- RC Lowpass AC — frequency sweep, verifies AC analysis pipeline
- RL Highpass AC — frequency sweep with inductor AC model
- RLC Series Resonance — resonant circuit, tests frequency response peak
- RC Step Tran — transient step response, verifies trapezoidal integration
- RL Step Tran — transient with inductor, verifies inductor companion model
- RLC Tran — damped oscillation, tests timestep control and integration accuracy

**Tolerance:** abs=1e-4 to 1e-3, rel=1e-3. Should be near-bit-identical.

### L3: Single nonlinear devices (10 circuits)

One semiconductor device per circuit. Exercises device model load functions, voltage limiting, junction initialization, and temperature processing.

- Diode Forward DC — forward-biased diode, verifies exponential I-V
- Diode Reverse DC — reverse-biased diode, verifies saturation current
- NMOS Level 1 DC — single MOSFET operating point
- NMOS Level 1 Body Effect — body bias sweep
- PMOS Level 1 DC — PMOS sign convention
- BJT NPN DC — bipolar transistor operating point
- BJT PNP DC — PNP sign convention
- JFET N-Channel DC — junction FET
- Diode Tran — diode switching transient
- NMOS Tran — MOSFET transient with Meyer charge model

**Tolerance:** abs=0.01, rel=0.01. Well-conditioned devices achieve machine precision.

### L4: NR algorithm stress tests (6 circuits)

Circuits that stress Newton-Raphson convergence: high-gain configurations, body effect sweeps, stiff systems.

- Body Effect Sweep — MOSFET Vbs sweep, tests NR across region boundaries
- High-Gain Amplifier — common-source with large gain, tests convergence with feedback
- DC Sweep NMOS — Vds sweep across linear/saturation boundary
- Resistor Sweep — parametric DC sweep
- Source Stepping — circuit that requires source stepping to converge
- Gmin Stepping — circuit that requires gmin stepping

**Tolerance:** abs=0.01, rel=0.01.

### L5: Device interactions (5 circuits)

Multiple nonlinear devices interacting. Exercises the NR loop with coupled nonlinearities.

- CMOS Inverter — NMOS + PMOS, the fundamental digital gate
- Differential Pair — two matched MOSFETs with tail current source
- Diode Bridge — four diodes, tests multiple junction initialization
- BJT Amplifier — common-emitter with biasing network
- Cascode — stacked MOSFETs, tests high-impedance node convergence

**Tolerance:** abs=0.01, rel=0.01.

### L6+: Full circuits (192 circuits)

The bulk of the test suite. Includes:

- **BSIM3 circuits** — NMOS/PMOS with BSIM3v3 models, various geometries and bias conditions
- **BSIM4 circuits** — next-generation model (in progress)
- **MOSFET Level 2/3** — legacy models with short-channel effects
- **Complex topologies** — feedback amplifiers, oscillators, bias networks
- **Specialized devices** — transmission lines, mutual inductors, controlled sources
- **Analysis types** — DC sweep, AC, transient, transfer function, sensitivity, pole-zero
- **Edge cases** — zero-valued components, floating nodes with gshunt, temperature variations

**Tolerance:** abs=0.01, rel=0.01 (some AC and precision tests use tighter values).

## Manifest format

Each circuit is registered in `manifest.toml`:

```toml
[[circuit]]
name = "[L3] NMOS Level 1 DC"
file = "dc/L3_nmos_level1.cir"
[circuit.tolerances]
abs = 0.01
rel = 0.01
```

The `file` path is relative to `sim/spice-eval/eval/`. Circuit files are standard SPICE netlists that both spice-rs and ngspice can parse.

## Adding a test circuit

1. Write a SPICE netlist in the appropriate `eval/` subdirectory (dc/, tran/, ac/, etc.).
2. Verify it runs in ngspice: `ngspice -b circuit.cir`.
3. Add an entry to `manifest.toml` with `abs=0.01, rel=0.01` tolerance.
4. Run `cargo run --release --bin spice-eval -- --filter="New Circuit"` to verify.
5. If the circuit exercises well-conditioned passives, tighten the tolerance.

## Design principles

**No golden files.** Test circuits are not compared against stored reference values. They are compared against a live ngspice run. This means updating ngspice (via the vendor submodule) automatically updates the reference.

**Tolerance is a floor, not a target.** The standard tolerance (0.01) is the minimum — most circuits achieve much better accuracy. 176 out of 224 circuits produce bit-identical results. The tolerance exists to handle the few circuits where floating-point non-associativity causes tiny divergences.

**Every device model needs a circuit.** When porting a new device model, create at least one L3 (single device) circuit for it before tackling more complex configurations.
