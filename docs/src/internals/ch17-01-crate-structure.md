# Crate Structure

The simulation system is split across five crates in the `sim/` directory of the workspace.

## Crate map

### `spice-rs` (sim/spice-rs/)

The core SPICE engine. Contains:

- **`parser`** — SPICE netlist parser. Handles R, C, L, V, I, D, M, Q, J, E, G, F, H, T device lines plus `.MODEL`, `.OP`, `.TRAN`, `.DC`, `.AC`, `.TF`, `.SENS`, `.PZ`, `.OPTIONS`, `.IC`, `.NODESET`, and `.END`.
- **`circuit`** — Circuit topology: nodes, branches, device instances, node-name-to-equation-number mapping. Matches ngspice's `CKTcircuit` (topology portion).
- **`mna`** — Modified Nodal Analysis matrix system. Wraps the Markowitz sparse matrix with TRANSLATE (external-to-internal node remapping), element caching, and RHS vectors.
- **`solver`** — `SimState` (mutable simulation state) and `ni_iter()` (Newton-Raphson iteration loop, port of ngspice `NIiter`).
- **`analysis/`** — Analysis engines: `dc` (operating point, DC sweep), `transient`, `ac`, `tf`, `sens`, `pz`.
- **`device/`** — Device models: `resistor`, `capacitor`, `inductor`, `vsource`, `isource`, `diode`, `mosfet1`, `mosfet2`, `mosfet3`, `bsim3`, `bsim4`, `bjt`, `jfet`, `vcvs`, `vccs`, `ccvs`, `cccs`, `tline`, `mutual_inductor`.
- **`integration`** — Numerical integration (`ni_com_cof`, `ni_integrate`, `ckt_terr`): trapezoidal rule companion model computation.
- **`state`** — `StateVectors`: arena-allocated per-device state with 8 history levels.
- **`config`** — `SimConfig`: simulation options (tolerances, temperature, iteration limits).
- **`runner`** — High-level entry point: `run_netlist(text) -> HashMap<String, f64>`.

### `sparse-rs` (sim/sparse-rs/)

Pure Rust sparse direct solver. Two independent backends:

- **`klu/`** — Gilbert-Peierls LU factorization with BTF decomposition and AMD column ordering. Port of SuiteSparse KLU.
- **`markowitz/`** — Markowitz pivoting with diagonal preference. Port of Sparse 1.3 (Kundert, 1988). Arena-based linked-list matrix with u32 indices.

Also provides `CscMatrix` (compressed sparse column) as the interchange format for KLU.

### `ngspice-ffi` (sim/ngspice-ffi/)

Safe Rust wrapper around `libngspice` (shared library). Provides `NgSpice::new()`, `load_circuit()`, `command()`, and vector extraction. Used exclusively by the eval harness — never by the simulation engine itself. Includes a global mutex since ngspice is not thread-safe.

### `spice-eval` (sim/spice-eval/)

Validation harness that runs test circuits through both spice-rs and ngspice (via `ngspice-ffi`), then compares results. Contains the test circuit manifest (`eval/manifest.toml`) and 224 test circuits organized by complexity layer.

### `spice-rs-wasm` (sim/spice-rs-wasm/)

`wasm-bindgen` bindings exposing spice-rs to the browser. Depends on `spice-rs` and the `ferrite-schematic-render` crate. Built with `wasm-pack` for integration into the Ferrite UI.

## Dependency graph

```
spice-eval
├── spice-rs
│   └── sparse-rs
└── ngspice-ffi
      └── libngspice (C, linked at build time)

spice-rs-wasm
├── spice-rs
│   └── sparse-rs
├── ferrite-schematic-render
└── ferrite-data-model
```

The critical dependency is `spice-rs → sparse-rs`. The simulation engine calls into the Markowitz solver through `MnaSystem`, which wraps `MarkowitzMatrix`. The KLU backend is available but not used by default in the MNA path — it is used directly by `sparse-eval` for cross-validation.

`spice-eval` depends on both `spice-rs` and `ngspice-ffi`, running the same netlist through both engines and comparing output. It never links the two engines together — they share only the netlist text.

## Companion crate: `sparse-eval` (sim/sparse-eval/)

Benchmarks `sparse-rs` against SuiteSparse C (via FFI). Contains binaries for KLU comparison (`solver-compare`), AMD ordering comparison (`amd-compare`), and the main benchmark suite (`sparse-eval`). Links against SuiteSparse C libraries at build time via a `build.rs` script.
