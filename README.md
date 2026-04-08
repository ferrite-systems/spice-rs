# spice-rs

A faithful port of [ngspice](https://ngspice.sourceforge.io/) to pure Rust — no C dependencies, no FFI. Compiles to WebAssembly for browser-based circuit simulation.

**[Read the docs](https://ferrite-systems.github.io/spice-rs/)**

## What this is

spice-rs is an experiment in using LLMs to port a large, complex C codebase to Rust — and in discovering what it actually takes to make that work.

ngspice is roughly 500,000 lines of C: decades of numerical algorithms, device physics models, and solver heuristics accumulated since Berkeley SPICE in 1973. We wanted to know if an LLM-assisted workflow could faithfully translate this code — not approximate it, not rewrite it from textbook equations, but produce a Rust implementation that matches ngspice at machine precision, device model by device model, timestep by timestep.

The answer: yes, but only with rigorous evaluation infrastructure. The LLM reads ngspice's C and produces plausible Rust translations, but "plausible" is not "correct." Getting to bit-identical required an **evaluation harness** that pinpoints exactly where the Rust output diverges from ngspice — which device, which parameter, which Newton-Raphson iteration, which matrix entry. The human designs testing infrastructure and diagnoses root causes; the LLM does the high-volume translation. Neither could do this alone.

## Status

- **199 / 226** validation circuits passing against ngspice (abs=0.01, rel=0.01)
- **176** of those are bit-identical to ngspice output
- Covers: resistors, capacitors, inductors, diodes, MOSFETs (Level 1/2/3, BSIM3), BJTs, JFETs, coupled inductors, transmission lines, all source types
- Analysis modes: `.OP`, `.TRAN`, `.DC`, `.AC`, `.TF`, `.SENS`, `.PZ`

## The Ferrite context

spice-rs is a core component of [Ferrite](https://github.com/ferrite-systems), an open-source EDA platform built on a text-first philosophy using [KDL](https://kdl.dev) as its native file format. A pure-Rust engine means simulation runs anywhere Rust compiles: natively in the desktop editor, in CI pipelines, and in the browser via WebAssembly.

## Documentation

The docs site at **[ferrite-systems.github.io/spice-rs](https://ferrite-systems.github.io/spice-rs/)** is an interactive textbook — every circuit diagram, simulation result, and waveform is computed live in your browser via the same WASM-compiled engine that passes the validation suite. No server, no pre-rendered images.

The site covers:
- **Learn SPICE** — From Kirchhoff's laws to BSIM3, with interactive simulations
- **Reference** — Netlist syntax, device models, simulation options, API
- **Internals** — Architecture, porting process, sparse-rs solver, validation methodology

## License

spice-rs is licensed under **GPL-3.0-or-later**.

The algorithms are derived from ngspice (Modified BSD) and SuiteSparse (BSD-3-Clause / LGPL-2.1). See [NOTICES](NOTICES) for full upstream attribution, license texts, and the ngspice contributor list.
