/// Trace flags — control stderr debug output without env vars.
/// Set via spice-eval CLI flags (e.g. `--trace=step,lte,bp`).
#[derive(Debug, Clone, Default)]
pub struct TraceFlags {
    /// Print accepted step info (SR_ACCEPT). Value = max steps to print (0 = off).
    pub step: usize,
    /// Print breakpoint state at each step (BP_STATE).
    pub bp: bool,
    /// Print LTE truncation results per step (SR_LTE).
    pub lte: bool,
    /// Print full RHS/solution per NR iteration (SR_NR).
    pub nr_dump: bool,
    /// Print compact NR solution (small circuits only).
    pub nr: bool,
    /// Print matrix diagonals pre-gmin (small circuits only).
    pub stamp: bool,
    /// Print TRANSLATE mapping during MNA setup.
    pub translate: bool,
    /// Print voltage limiter inputs/outputs.
    pub limiter: bool,
    /// Enable profiling (NR snapshot collection for diverge-deep).
    pub profile: bool,
}

/// Simulation configuration — matches ngspice CKTcircuit tolerance/option fields.
#[derive(Debug, Clone)]
pub struct SimConfig {
    /// Relative tolerance (CKTreltol, default 1e-3).
    pub reltol: f64,
    /// Absolute voltage tolerance (CKTvoltTol, default 1e-6 V).
    pub volt_tol: f64,
    /// Absolute current tolerance (CKTabstol, default 1e-12 A).
    pub abs_tol: f64,
    /// Minimum conductance (CKTgmin, default 1e-12 S).
    pub gmin: f64,
    /// Shunt conductance (CKTgshunt, default 0).
    pub gshunt: f64,
    /// Temperature in Kelvin (CKTtemp, default 300.15 = 27C).
    pub temp: f64,
    /// Nominal temperature in Kelvin (CKTnomTemp, default 300.15).
    pub tnom: f64,
    /// Maximum DC iterations (itl1, default 100).
    pub dc_max_iter: usize,
    /// Maximum DC transfer curve iterations (itl2, default 50).
    pub dc_trcv_max_iter: usize,
    /// Number of gmin steps (default 1 = dynamic_gmin).
    pub num_gmin_steps: usize,
    /// Gmin stepping factor (default 10).
    pub gmin_factor: f64,
    /// Number of source steps (default 1 = gillespie_src).
    pub num_src_steps: usize,
    /// Trace/debug flags — set via CLI, NOT env vars.
    pub trace: TraceFlags,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            reltol: 1e-3,
            volt_tol: 1e-6,
            abs_tol: 1e-12,
            gmin: 1e-12,
            gshunt: 0.0,
            temp: 300.15,
            tnom: 300.15,
            dc_max_iter: 100,
            dc_trcv_max_iter: 50,
            num_gmin_steps: 1,
            gmin_factor: 10.0,
            num_src_steps: 1,
            trace: TraceFlags::default(),
        }
    }
}
