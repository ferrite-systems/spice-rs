# Validation & Eval

spice-rs validates correctness by running test circuits through both spice-rs and ngspice and comparing every output value. This is not unit testing — it is full-pipeline comparison against a reference implementation.

The validation infrastructure consists of:

- **224 test circuits** organized by complexity layer (L1 through L6+)
- **The spice-eval harness** that runs both engines and compares results
- **Divergence reporting** that identifies the exact device and parameter where the engines disagree
- **Trace export** for deep investigation of transient waveform divergences

The standard: if a circuit produces different output than ngspice (beyond tolerance), that is a bug in spice-rs. Not a "different but valid" result — a bug. Fix the code, not the tolerance.

## Chapters

- [Test Circuits](ch20-01-test-circuits.md) — the 224 test circuits organized by category
- [Divergence Reports](ch20-02-divergence-reports.md) — reading and using divergence reports
- [Trace Export](ch20-03-trace-export.md) — deep investigation with per-timestep JSON traces
- [Current Status](ch20-04-status.md) — parity status and known issues
