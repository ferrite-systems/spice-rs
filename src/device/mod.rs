pub mod bjt;
pub mod bsim3;
pub mod bsim4;
pub mod capacitor;
pub mod cccs;
pub mod ccvs;
pub mod diode;
pub mod inductor;
pub mod jfet;
pub mod limiting;
pub mod isource;
pub mod mosfet1;
pub mod mosfet2;
pub mod mosfet3;
pub mod mutual_inductor;
pub mod resistor;
pub mod tline;
pub mod vccs;
pub mod vcvs;
pub mod vsource;

use std::any::Any;

use crate::error::SimError;
use crate::mna::MnaSystem;
use crate::mode::Mode;
use crate::state::StateVectors;

/// Device trait — matches ngspice SPICEdev function pointer table.
///
/// One `load()` method handles ALL modes (DC, transient, AC) by checking
/// `mode` flags internally. This matches ngspice's single DEVload pattern
/// and avoids the split stamp_dc/stamp_transient problem from spice-rs v1.
pub trait Device: std::fmt::Debug + Any {
    /// Upcast to Any for downcasting in transient engine.
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// Device instance name (e.g., "R1", "V1").
    fn name(&self) -> &str;

    /// Allocate state vector slots. Called during circuit setup.
    /// Returns the number of states needed (device stores its base offset).
    fn setup(&mut self, _states: &mut StateVectors) -> usize {
        0
    }

    /// Pre-allocate matrix elements. Called after setup, before first NR iteration.
    /// Matches ngspice TSTALLOC in DEVsetup — ensures all elements exist before factoring.
    fn setup_matrix(&mut self, _mna: &mut crate::mna::MnaSystem) {}

    /// Set initial conditions from node voltages — port of DEVsetic (e.g. CAPgetic).
    /// Called with UIC before DC OP. Devices without device-level ICs read their
    /// IC from the rhs vector (which contains .IC node voltages).
    fn setic(&mut self, _rhs: &[f64]) {}


    /// Temperature-dependent preprocessing (DEVtemperature).
    /// Called once before simulation, and whenever temperature changes.
    fn temperature(&mut self, _temp: f64, _tnom: f64) {}

    /// Pre-load phase — called for ALL devices before the main load() loop.
    ///
    /// Port of the first pass in INDload (indload.c:41-49): inductors compute
    /// state0[flux] = L/m * i_branch here. Mutual inductors then add their
    /// cross-coupling flux contributions in their load() call, which runs
    /// between pre_load() and the inductor's load() (due to type ordering).
    ///
    /// Default: no-op. Only inductors override this.
    fn pre_load(
        &mut self,
        _mna: &mut crate::mna::MnaSystem,
        _states: &mut crate::state::StateVectors,
        _mode: Mode,
    ) {}

    /// Main device evaluation — called every NR iteration (DEVload).
    ///
    /// The device must:
    /// 1. Read node voltages from `mna.rhs_old_val(node_eq)` (previous solution)
    /// 2. Evaluate device equations (I-V, charge, etc.)
    /// 3. Stamp conductance matrix via `mna.stamp(row, col, value)`
    /// 4. Stamp RHS via `mna.stamp_rhs(row, value)`
    /// 5. Set `noncon` to true if voltage limiting was applied
    ///
    /// `mode` determines behavior (JCT init, transient, etc.).
    /// `src_fact` is the source scaling factor (0..1 for source stepping).
    /// `gmin` is the per-device minimum conductance (CKTgmin, stepped during gmin stepping).
    fn load(
        &mut self,
        mna: &mut MnaSystem,
        states: &mut StateVectors,
        mode: Mode,
        src_fact: f64,
        gmin: f64,
        noncon: &mut bool,
    ) -> Result<(), SimError>;

    /// Truncation error — compute maximum safe timestep (DEVtrunc).
    /// Returns f64::INFINITY if no constraint.
    fn truncate(&self, _states: &StateVectors) -> f64 {
        f64::INFINITY
    }

    /// Accept a converged timepoint (DEVaccept).
    fn accept(&mut self, _states: &StateVectors) {}

    /// Device-specific convergence test (DEVconvTest / CKTconvTest).
    /// Port of ngspice's NEWCONV per-device convergence check.
    /// Uses the NEW solution (rhs) to compute predicted currents and
    /// compares against stored state0 values from the last load() call.
    /// Returns true if converged, false if not.
    fn conv_test(&self, _mna: &MnaSystem, _states: &StateVectors, _reltol: f64, _abstol: f64) -> bool {
        true
    }

    /// Return parsed model parameter name-value pairs for parity checking.
    /// Values are raw parsed model params (before temperature scaling).
    /// Default empty — passive devices don't need parameter comparison.
    fn model_params(&self) -> Vec<(&str, f64)> { vec![] }

    /// Return current device conductances for parity checking.
    fn conductances(&self) -> Vec<(&str, f64)> { vec![] }

    /// Return stored currents (cd, cbs, cbd) for conv_test parity checking.
    /// For MOSFETs, these correspond to MOS1cd, MOS1cbs, MOS1cbd in ngspice.
    fn stored_currents(&self) -> Vec<(&str, f64)> { vec![] }

    /// Return post-limiting terminal voltages for parity checking.
    fn limited_voltages(&self) -> Vec<(&str, f64)> { vec![] }

    /// Return the list of parameters perturbable for sensitivity analysis.
    /// Each entry is (param_name, param_id). The param_id is used with
    /// get_param/set_param to access the value.
    ///
    /// Port of ngspice's sgen parameter iteration.
    fn sensitivity_params(&self) -> Vec<(String, u32)> { vec![] }

    /// Get a parameter value by ID (for sensitivity analysis).
    fn get_param(&self, _id: u32) -> Option<f64> { None }

    /// Set a parameter value by ID (for sensitivity analysis).
    fn set_param(&mut self, _id: u32, _value: f64) {}

    /// Load device stamps into a dense matrix collector (for sensitivity analysis).
    /// This is equivalent to load() but writes to a DenseStampCollector instead of MnaSystem.
    fn load_into_dense(
        &mut self,
        _collector: &mut crate::analysis::sens::DenseStampCollector<'_>,
        _states: &mut StateVectors,
        _mode: Mode,
        _src_fact: f64,
        _gmin: f64,
        _noncon: &mut bool,
    ) {}

    /// AC small-signal load — stamps the complex (G + jwC) matrix for AC analysis.
    ///
    /// Port of each device's DEVacLoad function. Called once per frequency point.
    /// `omega` is 2*pi*freq (= CKTomega).
    ///
    /// Devices stamp real conductances via `mna.stamp()` and imaginary susceptances
    /// via `mna.stamp_imag()`. Sources stamp AC stimulus into `mna.stamp_rhs()`
    /// and `mna.stamp_irhs()`.
    ///
    /// Default: no-op. Devices without AC behavior don't need to implement this.
    fn ac_load(
        &mut self,
        _mna: &mut MnaSystem,
        _states: &StateVectors,
        _omega: f64,
    ) -> Result<(), SimError> {
        Ok(())
    }

    /// Pole-zero load — stamps the complex matrix for PZ analysis.
    ///
    /// Port of each device's DEVpzLoad function. Called for each trial point s.
    /// `s_re` and `s_im` are the real and imaginary parts of the complex frequency s.
    ///
    /// Resistors/conductance sources stamp real parts.
    /// Capacitors stamp s*C into both real and imaginary parts.
    /// Inductors stamp -s*L into the branch equation.
    /// Voltage sources stamp the coupling equations.
    ///
    /// Default: no-op. Devices without PZ behavior don't need to implement this.
    fn pz_load(
        &mut self,
        _mna: &mut MnaSystem,
        _s_re: f64,
        _s_im: f64,
    ) -> Result<(), SimError> {
        Ok(())
    }
}
