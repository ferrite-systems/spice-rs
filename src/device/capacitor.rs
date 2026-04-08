use crate::device::Device;
use crate::error::SimError;
use crate::integration::ni_integrate;
use crate::mna::MnaSystem;
use crate::mode::*;
use crate::state::StateVectors;

/// Number of state variables per capacitor instance: charge (qcap) + current (ccap).
const CAP_NUM_STATES: usize = 2;

/// Capacitor device — port of ngspice cap/capload.c.
///
/// State layout: state[offset] = charge (qcap), state[offset+1] = current (ccap).
#[derive(Debug)]
pub struct Capacitor {
    name: String,
    pos_node: usize,
    neg_node: usize,
    capacitance: f64,
    ic: Option<f64>,
    state_offset: usize,
    /// Integration coefficients (set by transient engine before load).
    pub ag: [f64; 7],
    pub order: usize,
}

impl Capacitor {
    pub fn new(
        name: impl Into<String>,
        pos_node: usize,
        neg_node: usize,
        capacitance: f64,
    ) -> Self {
        Self {
            name: name.into(),
            pos_node,
            neg_node,
            capacitance,
            ic: None,
            state_offset: 0,
            ag: [0.0; 7],
            order: 1,
        }
    }

    pub fn with_ic(mut self, ic: f64) -> Self {
        self.ic = Some(ic);
        self
    }

    pub fn qcap(&self) -> usize {
        self.state_offset
    }

    pub fn pos_node(&self) -> usize { self.pos_node }
    pub fn neg_node(&self) -> usize { self.neg_node }
}

impl Device for Capacitor {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn name(&self) -> &str { &self.name }

    fn setup_matrix(&mut self, mna: &mut crate::mna::MnaSystem) {
        let p = self.pos_node; let n = self.neg_node;
        mna.make_element(p, p); mna.make_element(n, n);
        mna.make_element(p, n); mna.make_element(n, p);
    }

    /// CAPgetic (capgetic.c): if no device-level IC, read from .IC node voltages.
    fn setic(&mut self, rhs: &[f64]) {
        if self.ic.is_none() {
            let v = rhs[self.pos_node] - rhs[self.neg_node];
            if v != 0.0 {
                self.ic = Some(v);
            }
        }
    }

    fn setup(&mut self, states: &mut StateVectors) -> usize {
        self.state_offset = states.allocate(CAP_NUM_STATES);
        CAP_NUM_STATES
    }

    fn load(
        &mut self,
        mna: &mut MnaSystem,
        states: &mut StateVectors,
        mode: Mode,
        _src_fact: f64,
        _gmin: f64,
        _noncon: &mut bool,
    ) -> Result<(), SimError> {
        let p = self.pos_node;
        let n = self.neg_node;
        let qcap = self.state_offset;

        if mode.is(MODETRAN) || mode.is(MODEAC) || mode.is(MODETRANOP) {
            // Determine voltage across capacitor (capload.c:56-61)
            // capload.c:42-44: MODEDC (not MODEDCOP) — MODEDC is a mask including MODETRANOP
            let cond1 = (mode.is(MODEDC) && mode.is(MODEINITJCT))
                || (mode.is(MODEUIC) && mode.is(MODEINITTRAN));

            let vcap = if cond1 {
                self.ic.unwrap_or(0.0)
            } else {
                mna.rhs_old_val(p) - mna.rhs_old_val(n)
            };

            if mode.is(MODETRAN) || mode.is(MODEAC) {
                // Charge state (capload.c:62-76)
                if mode.is(MODEINITPRED) {
                    // Use previous step's charge
                    let q1 = states.get(1, qcap);
                    states.set(0, qcap, q1);
                } else {
                    states.set(0, qcap, self.capacitance * vcap);
                    if mode.is(MODEINITTRAN) {
                        // Initialize history
                        let q0 = states.get(0, qcap);
                        states.set(1, qcap, q0);
                    }
                }

                // Integrate: charge → current → companion model (capload.c:77-79)
                let (geq, ceq) = ni_integrate(&self.ag, states, self.capacitance, qcap, self.order);

                // Stamp companion model (capload.c:103-108)
                mna.stamp(p, p, geq);
                mna.stamp(n, n, geq);
                mna.stamp(p, n, -geq);
                mna.stamp(n, p, -geq);
                mna.stamp_rhs(p, -ceq);
                mna.stamp_rhs(n, ceq);
            }
        } else {
            // DC mode: just store charge (capload.c:109-110)
            let vcap = mna.rhs_old_val(p) - mna.rhs_old_val(n);
            states.set(0, qcap, self.capacitance * vcap);
        }

        Ok(())
    }

    fn truncate(&self, _states: &StateVectors) -> f64 {
        // Will be called via ckt_terr from the transient engine
        // The actual CKTterr call happens in the transient loop
        f64::INFINITY
    }

    /// Port of CAPacLoad from capacld.c.
    /// Stamps omega*C into the imaginary part of the complex matrix.
    fn ac_load(
        &mut self,
        mna: &mut MnaSystem,
        _states: &StateVectors,
        omega: f64,
    ) -> Result<(), SimError> {
        let val = omega * self.capacitance;
        let p = self.pos_node;
        let n = self.neg_node;
        mna.stamp_imag(p, p, val);
        mna.stamp_imag(n, n, val);
        mna.stamp_imag(p, n, -val);
        mna.stamp_imag(n, p, -val);
        Ok(())
    }

    /// Port of CAPpzLoad from cappzld.c.
    /// Stamps s*C into the complex matrix: real part gets C*s_re, imag gets C*s_im.
    /// m (multiplier) defaults to 1.0 in our simplified model.
    fn pz_load(
        &mut self,
        mna: &mut MnaSystem,
        s_re: f64,
        s_im: f64,
    ) -> Result<(), SimError> {
        let val = self.capacitance; // m=1.0
        let p = self.pos_node;
        let n = self.neg_node;
        mna.stamp(p, p, val * s_re);
        mna.stamp_imag(p, p, val * s_im);
        mna.stamp(n, n, val * s_re);
        mna.stamp_imag(n, n, val * s_im);
        mna.stamp(p, n, -(val * s_re));
        mna.stamp_imag(p, n, -(val * s_im));
        mna.stamp(n, p, -(val * s_re));
        mna.stamp_imag(n, p, -(val * s_im));
        Ok(())
    }
}

// Mode flag not defined in mode.rs yet — add it
pub const MODEUIC: u32 = 0x10000;
