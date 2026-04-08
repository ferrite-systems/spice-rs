# API Reference

spice-rs can be used in three ways:

1. **Rust library** -- direct integration via `spice_rs` crate
2. **WebAssembly module** -- browser and Node.js usage via `spice-rs-wasm`
3. **sparse-rs** -- standalone sparse matrix solver (KLU and Markowitz backends)

All three share the same simulation core. The WASM module wraps the Rust API with JSON serialization. sparse-rs is an independent crate that can be used without the SPICE engine.

## In this chapter

- [Rust API](ch16-01-rust-api.md) -- `spice_rs::runner` functions and `Circuit`/`SimConfig` types
- [WASM API](ch16-02-wasm-api.md) -- `SimulationEngine` class for JavaScript
- [sparse-rs API](ch16-03-sparse-rs.md) -- KLU and Markowitz sparse solver interfaces
