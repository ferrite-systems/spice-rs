# Data Flow

The simulation pipeline from netlist text to results follows ngspice's structure: parse, setup, temperature, analyze, extract.

## The pipeline

```
                    ┌─────────────────┐
  netlist text ───▶ │  parse_netlist() │ ──▶ ParseResult
                    └─────────────────┘
                              │
                    ┌─────────▼─────────┐
                    │  circuit.setup()   │  allocate state vectors
                    └───────────────────┘
                              │
                    ┌─────────▼──────────────────────┐
                    │  resolve_coupled_inductors()    │  link K ↔ L
                    └────────────────────────────────┘
                              │
                    ┌─────────▼──────────────┐
                    │  circuit.temperature()  │  temp-dependent params
                    └────────────────────────┘
                              │
                    ┌─────────▼──────────────────┐
                    │  analysis engine             │
                    │  (dc_operating_point /       │
                    │   transient / ac_analysis /  │
                    │   dc_sweep / ...)            │
                    └──────────────────────────────┘
                              │
                    ┌─────────▼──────────────────┐
                    │  extract_node_values()      │  solution → HashMap
                    └────────────────────────────┘
```

## Step-by-step

### 1. Parse: `parse_netlist(text) -> ParseResult`

**Source:** `sim/spice-rs/src/parser.rs`

The parser reads each line of the netlist and builds the circuit topology:

- Device lines (R, C, L, V, I, D, M, Q, J, E, G, F, H, T) create device instances and allocate nodes via `circuit.node(name)`. Each call to `circuit.node()` either returns an existing equation number or assigns a new one (monotonic, matching ngspice's `CKTmkVolt`/`CKTlinkEq`).
- `.MODEL` lines populate model parameter structs.
- Analysis directives (`.OP`, `.TRAN`, `.DC`, `.AC`, etc.) set the `Analysis` enum.
- `.OPTIONS` set tolerance overrides.
- `.IC` and `.NODESET` store node initial conditions.

The parser returns a `ParseResult` containing the `Circuit`, the `Analysis`, model parameters, coupled inductor specs (`K` elements), and option overrides.

### 2. Setup: `circuit.setup()`

**Source:** `sim/spice-rs/src/circuit.rs`

Calls `device.setup(&mut states)` on every device. Each device allocates contiguous state vector slots (e.g., a MOSFET allocates 17 slots for voltages, charges, and currents). After all devices have allocated, `states.finalize()` resizes all 8 history arrays to the total state count. This matches ngspice's `CKTsetup` → `DEVsetup` loop.

### 3. Coupled inductors: `resolve_coupled_inductors()`

**Source:** `sim/spice-rs/src/parser.rs`

Links `K` elements (mutual coupling) to their referenced `L` elements by name lookup. Each `MutualInductor` gets references to the two `Inductor` instances and the coupling coefficient. This must happen after setup but before temperature.

### 4. Temperature: `circuit.temperature(&config)`

**Source:** `sim/spice-rs/src/circuit.rs`

Calls `device.temperature(temp, tnom)` on every device. Devices compute temperature-dependent parameters: junction potentials, saturation currents, mobility, threshold voltage shifts. This matches ngspice's `CKTtemp` → `DEVtemperature` loop.

### 5. Analysis engine

The analysis engine creates a `SimState` (containing the `MnaSystem` and convergence state), calls `device.setup_matrix(&mut mna)` to pre-allocate matrix elements, then runs the appropriate analysis.

**DC operating point** (`sim/spice-rs/src/analysis/dc.rs`):
```
SimState::new(size)
  → device.setup_matrix(&mut mna)    // TSTALLOC: register matrix elements
  → apply_nodesets()                  // .NODESET initial guess
  → ni_iter() with MODEDCOP          // NR loop (direct)
  → if fails: dynamic gmin stepping  // CKTnumGminSteps
  → if fails: source stepping        // CKTnumSrcSteps
```

**Transient** (`sim/spice-rs/src/analysis/transient.rs`):
```
dc_operating_point_tran()            // initial DC OP (MODETRANOP)
  → time loop:
      rotate state vectors
      compute integration coefficients (ni_com_cof)
      ni_iter() with MODETRAN
      truncation error check (ckt_terr)
      accept or reject step
      adjust timestep
```

**AC analysis** (`sim/spice-rs/src/analysis/ac.rs`):
```
dc_operating_point()                 // bias point
  → frequency loop:
      clear matrix
      ac_load() on all devices       // stamp G + jwC
      factor + solve complex system
      extract complex node voltages
```

### 6. Extract: `extract_node_values()`

**Source:** `sim/spice-rs/src/runner.rs`

Maps equation numbers back to node names. Voltage nodes become `v(name)`, branch current nodes keep their name (e.g., `v1#branch`). Ground (equation 0) is skipped. The solution vector used is `rhs_old`, matching ngspice's `CKTrhsOld` output convention.

## The NR iteration loop

`ni_iter()` is the innermost loop and the performance-critical path. Each iteration:

1. `mna.clear()` — zero all matrix elements and RHS.
2. `device.pre_load()` — inductor flux computation (first pass).
3. `device.load()` — stamp conductances and currents into the matrix and RHS.
4. Add `diag_gmin` to matrix diagonal (if nonzero).
5. `mna.solve()` — Markowitz LU factorization (or refactorization) + forward/backward substitution.
6. Convergence check: `|new - old| < reltol * max(|new|, |old|) + abstol`.
7. If converged and `NEWCONV`: run `device.conv_test()` for additional per-device checks.
8. Swap `rhs` and `rhs_old` for the next iteration.

This is a direct port of ngspice's `NIiter` in `niiter.c`.
