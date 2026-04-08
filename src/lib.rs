//! spice-rs: Investigation-first ngspice port in Rust.
//!
//! This crate is a faithful, systematic port of ngspice's core simulation
//! engine. Each subsystem is documented in `docs/` before implementation.
//! See `docs/README.md` for the investigation index and porting status.

pub mod analysis;
pub mod breakpoint;
pub mod circuit;
pub mod config;
pub mod constants;
pub mod device;
pub mod error;
pub mod integration;
pub mod mna;
pub mod mode;
pub mod node;
pub mod parser;
pub mod runner;
pub mod solver;
pub mod state;
pub mod waveform;

// Re-exports for convenience
pub use circuit::Circuit;
pub use config::SimConfig;
pub use error::SimError;
pub use solver::SimState;
