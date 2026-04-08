use crate::device::Device;
use crate::error::SimError;
use crate::mna::MnaSystem;
use crate::mode::*;
use crate::state::StateVectors;
use crate::waveform::Waveform;

/// Voltage source — port of ngspice vsrc/vsrcload.c.
///
/// Uses a branch equation: V(pos) - V(neg) = value
///
/// Supports DC, PULSE, SINE, PWL waveforms.
#[derive(Debug)]
pub struct VoltageSource {
    name: String,
    pub pos_node: usize,
    pub neg_node: usize,
    pub branch_eq: usize,
    waveform: Waveform,
    /// Transient sim parameters (set by engine before load).
    pub time: f64,
    pub step: f64,
    pub final_time: f64,
    /// Breakpoint guard — only register new breakpoint when time >= break_time.
    /// Matches ngspice VSRCbreak_time (vsrcdefs.h:53, vsrcacct.c:94).
    pub break_time: f64,
    /// AC magnitude (from netlist `AC mag [phase]`).
    pub ac_mag: f64,
    /// AC phase in degrees.
    pub ac_phase_deg: f64,
}

impl VoltageSource {
    pub fn new(
        name: impl Into<String>,
        pos_node: usize,
        neg_node: usize,
        branch_eq: usize,
        voltage: f64,
    ) -> Self {
        Self {
            name: name.into(),
            pos_node,
            neg_node,
            branch_eq,
            waveform: Waveform::Dc(voltage),
            time: 0.0,
            step: 1.0,
            final_time: 1.0,
            break_time: -1.0, // vsrcset.c:34
            ac_mag: 0.0,
            ac_phase_deg: 0.0,
        }
    }

    pub fn with_waveform(
        name: impl Into<String>,
        pos_node: usize,
        neg_node: usize,
        branch_eq: usize,
        waveform: Waveform,
    ) -> Self {
        Self {
            name: name.into(),
            pos_node,
            neg_node,
            branch_eq,
            waveform,
            time: 0.0,
            step: 1.0,
            final_time: 1.0,
            break_time: -1.0, // vsrcset.c:34
            ac_mag: 0.0,
            ac_phase_deg: 0.0,
        }
    }
}

impl Device for VoltageSource {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn name(&self) -> &str { &self.name }

    fn setup_matrix(&mut self, mna: &mut crate::mna::MnaSystem) {
        let p = self.pos_node; let n = self.neg_node; let b = self.branch_eq;
        mna.make_element(p, b); mna.make_element(n, b);
        mna.make_element(b, p); mna.make_element(b, n);
    }

    fn load(
        &mut self,
        mna: &mut MnaSystem,
        _states: &mut StateVectors,
        mode: Mode,
        src_fact: f64,
        _gmin: f64,
        _noncon: &mut bool,
    ) -> Result<(), SimError> {
        let b = self.branch_eq;
        let p = self.pos_node;
        let n = self.neg_node;

        // Branch equation topology (vsrcload.c:44-72)
        mna.stamp(b, p, 1.0);
        mna.stamp(b, n, -1.0);
        mna.stamp(p, b, 1.0);
        mna.stamp(n, b, -1.0);

        // Evaluate waveform (vsrcload.c:74-88)
        // ngspice: if (MODEDCOP | MODEDCTRANCURVE) && dcGiven => use dcValue
        // For MODETRANOP, also use DC value (with source stepping)
        let mut value = if mode.is(MODEDCOP) || mode.is(MODEDCTRANCURVE) {
            self.waveform.dc_value()
        } else if mode.is(MODETRANOP) {
            self.waveform.dc_value()
        } else {
            let t = self.time;
            self.waveform.eval(t, self.step, self.final_time)
        };

        // Source stepping (vsrcload.c:445-456)
        // ngspice: for MODEDCOP|MODEDCTRANCURVE, value = dcValue * srcFact
        // But srcFact is only < 1.0 during source stepping convergence;
        // during DC sweep it's always 1.0.
        if mode.is(MODETRANOP) {
            value *= src_fact;
        }

        mna.stamp_rhs(b, value);

        Ok(())
    }

    /// Port of VSRCacLoad from vsrcacld.c.
    /// Stamps topology +-1 in real, AC stimulus in RHS.
    fn ac_load(
        &mut self,
        mna: &mut MnaSystem,
        _states: &crate::state::StateVectors,
        _omega: f64,
    ) -> Result<(), SimError> {
        let b = self.branch_eq;
        let p = self.pos_node;
        let n = self.neg_node;

        // Topology (same as DC)
        mna.stamp(p, b, 1.0);
        mna.stamp(n, b, -1.0);
        mna.stamp(b, p, 1.0);
        mna.stamp(b, n, -1.0);

        // AC stimulus: convert mag/phase to real/imag
        // Port of vsrctemp.c:68-70
        let radians = self.ac_phase_deg * std::f64::consts::PI / 180.0;
        let ac_real = self.ac_mag * radians.cos();
        let ac_imag = self.ac_mag * radians.sin();

        mna.stamp_rhs(b, ac_real);
        mna.stamp_irhs(b, ac_imag);

        Ok(())
    }

    /// Port of VSRCpzLoad from vsrcpzld.c.
    /// For a DC source: shorts nodes via branch equation.
    /// For an AC source: pos/neg to branch coupling + branch diagonal =1.
    fn pz_load(
        &mut self,
        mna: &mut MnaSystem,
        _s_re: f64,
        _s_im: f64,
    ) -> Result<(), SimError> {
        let b = self.branch_eq;
        let p = self.pos_node;
        let n = self.neg_node;

        let is_ac = self.ac_mag != 0.0;
        if !is_ac {
            // DC source — short circuit the nodes (vsrcpzld.c:30-35)
            mna.stamp(p, b, 1.0);
            mna.stamp(n, b, -1.0);
            mna.stamp(b, p, 1.0);
            mna.stamp(b, n, -1.0);
        } else {
            // AC source — (vsrcpzld.c:38-43)
            // The branch equation is made independent: diag=1
            mna.stamp(p, b, 1.0);
            mna.stamp(n, b, -1.0);
            mna.stamp(b, b, 1.0);
        }
        Ok(())
    }
}

impl VoltageSource {
    /// Compute the next waveform breakpoint time from current time.
    pub fn next_breakpoint(&self, time: f64, step: f64, final_time: f64, min_break: f64) -> Option<f64> {
        self.waveform.next_breakpoint(time, step, final_time, min_break)
    }

    /// Get the DC value — matches ngspice VSRCdcValue.
    pub fn dc_value(&self) -> f64 {
        self.waveform.dc_value()
    }

    /// Set the DC value — for DC sweep, replaces the waveform DC value.
    /// Matches ngspice: `here->VSRCdcValue = value` in dctrcurv.c.
    pub fn set_dc_value(&mut self, value: f64) {
        self.waveform.set_dc_value(value);
    }
}
