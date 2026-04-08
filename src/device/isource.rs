use crate::device::Device;
use crate::error::SimError;
use crate::mna::MnaSystem;
use crate::mode::*;
use crate::state::StateVectors;
use crate::waveform::Waveform;

/// Current source — port of ngspice isrc/isrcload.c.
///
/// Stamps current directly into RHS: RHS[pos] += I, RHS[neg] -= I.
#[derive(Debug)]
pub struct CurrentSource {
    name: String,
    pub pos_node: usize,
    pub neg_node: usize,
    waveform: Waveform,
    /// Transient sim parameters (set by engine before load).
    pub time: f64,
    pub step: f64,
    pub final_time: f64,
    /// AC magnitude (from netlist `AC mag [phase]`).
    pub ac_mag: f64,
    /// AC phase in degrees.
    pub ac_phase_deg: f64,
}

impl CurrentSource {
    pub fn new(
        name: impl Into<String>,
        pos_node: usize,
        neg_node: usize,
        waveform: Waveform,
    ) -> Self {
        Self {
            name: name.into(),
            pos_node,
            neg_node,
            waveform,
            time: 0.0,
            step: 1.0,
            final_time: 1.0,
            ac_mag: 0.0,
            ac_phase_deg: 0.0,
        }
    }
}

impl Device for CurrentSource {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn name(&self) -> &str {
        &self.name
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
        // Evaluate waveform (isrcload.c:47-54)
        // ngspice: if (MODEDCOP | MODEDCTRANCURVE) && dcGiven => use dcValue * srcFact
        // For MODETRANOP, also use DC value (with source stepping)
        let mut value = if mode.is(MODEDCOP) || mode.is(MODEDCTRANCURVE) {
            self.waveform.dc_value()
        } else if mode.is(MODETRANOP) {
            self.waveform.dc_value()
        } else {
            let t = self.time;
            self.waveform.eval(t, self.step, self.final_time)
        };

        // Source stepping (isrcload.c:390-405)
        if mode.is(MODETRANOP) {
            value *= src_fact;
        }

        // Stamp RHS
        mna.stamp_rhs(self.pos_node, value);
        mna.stamp_rhs(self.neg_node, -value);

        Ok(())
    }

    /// Port of ISRCacLoad from isrcacld.c.
    /// Stamps AC current into real and imaginary RHS.
    fn ac_load(
        &mut self,
        mna: &mut MnaSystem,
        _states: &crate::state::StateVectors,
        _omega: f64,
    ) -> Result<(), SimError> {
        let radians = self.ac_phase_deg * std::f64::consts::PI / 180.0;
        let ac_real = self.ac_mag * radians.cos();
        let ac_imag = self.ac_mag * radians.sin();

        mna.stamp_rhs(self.pos_node, ac_real);
        mna.stamp_rhs(self.neg_node, -ac_real);
        mna.stamp_irhs(self.pos_node, ac_imag);
        mna.stamp_irhs(self.neg_node, -ac_imag);

        Ok(())
    }
}

impl CurrentSource {
    pub fn next_breakpoint(&self, time: f64, step: f64, final_time: f64, min_break: f64) -> Option<f64> {
        self.waveform.next_breakpoint(time, step, final_time, min_break)
    }

    /// Get the DC value — matches ngspice ISRCdcValue.
    pub fn dc_value(&self) -> f64 {
        self.waveform.dc_value()
    }

    /// Set the DC value — for DC sweep, replaces the waveform DC value.
    /// Matches ngspice: `here->ISRCdcValue = value` in dctrcurv.c.
    pub fn set_dc_value(&mut self, value: f64) {
        self.waveform.set_dc_value(value);
    }
}
