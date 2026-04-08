# Architecture

spice-rs is a faithful port of ngspice's core simulation engine in Rust. Every algorithm — Newton-Raphson convergence, device model evaluation, timestep control, sparse factorization — is translated directly from the ngspice C source code, preserving the same logic, the same control flow, and in many cases the same variable names.

The architecture mirrors ngspice's structure:

- **Parser** reads SPICE netlists into a circuit topology.
- **Circuit builder** allocates nodes, branches, and device instances.
- **MNA system** assembles the Modified Nodal Analysis matrix and RHS vectors.
- **Device models** stamp conductances and currents into the MNA matrix each Newton-Raphson iteration.
- **NR solver** (`ni_iter`) drives the load-factor-solve loop until convergence.
- **Analysis engines** orchestrate DC operating point, transient, AC, DC sweep, sensitivity, transfer function, and pole-zero analyses.
- **Sparse solver** (in the separate `sparse-rs` crate) handles LU factorization and back-substitution.

The solver layer is decoupled: `sparse-rs` provides two independent backends (KLU and Markowitz), both ported from their respective C reference implementations. spice-rs uses the Markowitz backend by default, matching ngspice's default solver.

## Key design decisions

**One `load()` method per device.** ngspice uses a single `DEVload` function pointer per device type that handles all modes (DC, transient, AC initialization). spice-rs follows this pattern exactly. Devices check `mode` flags internally to determine behavior. This avoids the split `stamp_dc`/`stamp_transient` problem that plagued an earlier version.

**Persistent MNA matrix.** The matrix is created once (`spCreate`), elements are allocated during setup (`spGetElement`), and values are cleared/restamped each NR iteration. The matrix is never moved, copied, or rebuilt. This matches ngspice's architecture and is critical for Markowitz solver correctness (the pivot ordering is cached).

**State vectors with history.** Device state (charges, fluxes, junction voltages) is stored in flat arrays with 8 history levels, matching ngspice's `CKTstates[0..7]`. The arrays are rotated between timesteps using O(1) pointer swaps.

## Chapters in this section

- [Crate Structure](ch17-01-crate-structure.md) — the five crates and their dependency graph
- [Data Flow](ch17-02-data-flow.md) — the simulation pipeline from netlist text to results
- [Device Trait](ch17-03-device-trait.md) — how components plug into the simulator
- [State Management](ch17-04-state-management.md) — state vectors, history, and numerical integration
