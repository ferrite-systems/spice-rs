# Device Trait

All circuit components implement the `Device` trait, defined in `sim/spice-rs/src/device/mod.rs`. This trait matches ngspice's `SPICEdev` function pointer table — one set of callbacks that the simulator invokes at defined points during setup and analysis.

## The trait

```rust
pub trait Device: std::fmt::Debug + Any {
    fn name(&self) -> &str;

    // --- Setup phase ---
    fn setup(&mut self, states: &mut StateVectors) -> usize { 0 }
    fn setup_matrix(&mut self, mna: &mut MnaSystem) {}
    fn setic(&mut self, rhs: &[f64]) {}
    fn temperature(&mut self, temp: f64, tnom: f64) {}

    // --- NR iteration ---
    fn pre_load(&mut self, mna: &mut MnaSystem, states: &mut StateVectors, mode: Mode) {}
    fn load(&mut self, mna: &mut MnaSystem, states: &mut StateVectors,
            mode: Mode, src_fact: f64, gmin: f64, noncon: &mut bool) -> Result<(), SimError>;
    fn conv_test(&self, mna: &MnaSystem, states: &StateVectors,
                 reltol: f64, abstol: f64) -> bool { true }

    // --- Transient ---
    fn truncate(&self, states: &StateVectors) -> f64 { f64::INFINITY }
    fn accept(&mut self, states: &StateVectors) {}

    // --- AC ---
    fn ac_load(&mut self, mna: &mut MnaSystem, states: &StateVectors,
               omega: f64) -> Result<(), SimError> { Ok(()) }

    // --- Pole-Zero ---
    fn pz_load(&mut self, mna: &mut MnaSystem,
               s_re: f64, s_im: f64) -> Result<(), SimError> { Ok(()) }

    // ... diagnostic methods omitted for brevity
}
```

## Method lifecycle

### Setup phase (called once before simulation)

**`setup(states)`** — Allocate state vector slots. A MOSFET Level 1 allocates 17 slots for terminal voltages, gate charges (Meyer model), junction charges, and their associated currents. Returns the base offset. Matches ngspice `DEVsetup` state allocation.

**`setup_matrix(mna)`** — Pre-allocate matrix elements via `mna.make_element(row, col)`. This returns a `MatElt` handle (a u32 arena index) that the device caches. During `load()`, the device stamps values using these cached handles. This matches ngspice's `TSTALLOC` macros in each device's setup function. All matrix elements must be allocated here — the Markowitz solver's structure is fixed after the first factorization.

**`setic(rhs)`** — Read initial conditions from the RHS vector (which holds `.IC` node voltages at this point). Capacitors read `rhs[pos] - rhs[neg]` into their initial condition. Port of ngspice `DEVsetic` (e.g., `CAPgetic`).

**`temperature(temp, tnom)`** — Compute temperature-dependent parameters. For a diode: junction potential, saturation current, transit time scaling. For a MOSFET: threshold voltage, mobility, junction parameters. Called once before simulation, matches ngspice `DEVtemperature`.

### NR iteration (called every Newton-Raphson iteration)

**`pre_load(mna, states, mode)`** — Called for ALL devices before the main `load()` loop. Only inductors override this: they compute `state0[flux] = L * i_branch` so that mutual inductors can add cross-coupling contributions during their `load()` call. This two-pass pattern matches ngspice's `INDload` first pass.

**`load(mna, states, mode, src_fact, gmin, noncon)`** — The main device evaluation. This is where the physics happens. The device must:

1. Read terminal voltages from `mna.rhs_old_val(node_eq)`.
2. Apply voltage limiting (for nonlinear devices) and set `noncon = true` if limiting was active.
3. Evaluate device equations: compute currents, conductances, charges.
4. Stamp the companion model into the matrix and RHS.

The `mode` parameter controls behavior: `MODEINITJCT` for junction initialization, `MODEINITFIX`/`MODEINITFLOAT` for the nodeset ipass mechanism, `MODETRAN` for transient (with integration), `MODEDCOP` for DC operating point. The `src_fact` parameter scales independent sources during source stepping (0 to 1). The `gmin` parameter is the per-device minimum conductance, stepped during gmin stepping.

**`conv_test(mna, states, reltol, abstol)`** — Port of ngspice's `NEWCONV` per-device convergence test. After the global convergence check passes, this method recomputes device currents using the NEW solution vector (`mna.rhs`) and compares against the stored values from the last `load()`. Returns false if the device-level check fails, forcing another NR iteration. Implemented for diodes, BJTs, MOSFETs, and JFETs.

### Transient callbacks

**`truncate(states)`** — Compute the maximum safe timestep based on local truncation error (LTE) for this device's charge states. Returns `f64::INFINITY` if the device imposes no constraint. The transient engine takes the minimum across all devices. Port of ngspice `DEVtrunc`.

**`accept(states)`** — Called when a timestep is accepted. Devices can update internal bookkeeping.

### AC small-signal

**`ac_load(mna, states, omega)`** — Stamp the linearized small-signal model into the complex MNA matrix. Conductances go into the real part (`mna.stamp()`), susceptances (omega * C) go into the imaginary part (`mna.stamp_imag()`). AC sources stamp their stimulus into `mna.stamp_rhs()` and `mna.stamp_irhs()`. Called once per frequency point after the DC bias point is established.

## The stamp pattern

Devices do not know about the sparse solver. They interact with the MNA system through a small API:

```rust
// During setup_matrix():
let drain_drain = mna.make_element(drain_eq, drain_eq);  // cache handle

// During load():
mna.stamp_elt(drain_drain, gds);      // add gds to (drain, drain) element
mna.stamp_rhs(drain_eq, -ids);        // add -ids to drain RHS
```

The `make_element()` call during setup registers the (row, col) pair in the sparse matrix and returns a handle. The `stamp_elt()` call during load adds a value to the element using the cached handle — this is O(1), no hash lookup. The `stamp()` convenience method takes (row, col) and does the lookup, but most hot-path code uses the cached handles.

This separation means devices are completely decoupled from the solver backend. They stamp values; the MNA system manages the matrix.

## Device inventory

| Prefix | Device | Source | States |
|--------|--------|--------|--------|
| R | Resistor | `resistor.rs` | 0 |
| C | Capacitor | `capacitor.rs` | 2 |
| L | Inductor | `inductor.rs` | 2 |
| K | Mutual Inductor | `mutual_inductor.rs` | 0 |
| V | Voltage Source | `vsource.rs` | 0 |
| I | Current Source | `isource.rs` | 0 |
| D | Diode | `diode.rs` | 5 |
| M (Level 1) | MOSFET Level 1 | `mosfet1.rs` | 17 |
| M (Level 2) | MOSFET Level 2 | `mosfet2.rs` | 17 |
| M (Level 3) | MOSFET Level 3 | `mosfet3.rs` | 17 |
| M (BSIM3) | BSIM3v3 | `bsim3.rs` | 17 |
| M (BSIM4) | BSIM4 | `bsim4.rs` | 17 |
| Q | BJT | `bjt.rs` | 13 |
| J | JFET | `jfet.rs` | 7 |
| E | VCVS | `vcvs.rs` | 0 |
| G | VCCS | `vccs.rs` | 0 |
| F | CCCS | `cccs.rs` | 0 |
| H | CCVS | `ccvs.rs` | 0 |
| T | Transmission Line | `tline.rs` | 6 |
