use crate::device::Device;
use crate::error::SimError;
use crate::integration::ni_integrate;
use crate::mna::MnaSystem;
use crate::mode::*;
use crate::state::StateVectors;

/// Number of state variables: flux + voltage-equivalent.
const IND_NUM_STATES: usize = 2;

/// Inductor device — port of ngspice ind/indload.c.
///
/// Uses a branch equation (like voltage source) with flux integration.
/// State layout: state[offset] = flux, state[offset+1] = voltage-equivalent.
///
/// The load is split into two phases to support mutual inductors:
/// - `pre_load()`: first pass (indload.c:41-49) — compute state0[flux] = L/m * i_branch
/// - `load()`: third pass (indload.c:84-153) — predictor, integrate, stamp
///
/// Between these phases, mutual inductors add their flux contributions.
#[derive(Debug)]
pub struct Inductor {
    name: String,
    pos_node: usize,
    neg_node: usize,
    branch_eq: usize,
    inductance: f64,
    ic: Option<f64>,
    state_offset: usize,
    pub ag: [f64; 7],
    pub order: usize,
}

impl Inductor {
    pub fn new(
        name: impl Into<String>,
        pos_node: usize,
        neg_node: usize,
        branch_eq: usize,
        inductance: f64,
    ) -> Self {
        Self {
            name: name.into(),
            pos_node,
            neg_node,
            branch_eq,
            inductance,
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

    pub fn flux_offset(&self) -> usize {
        self.state_offset
    }

    pub fn branch_eq(&self) -> usize {
        self.branch_eq
    }

    pub fn inductance(&self) -> f64 {
        self.inductance
    }

    pub fn ic(&self) -> Option<f64> {
        self.ic
    }
}

impl Device for Inductor {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self, states: &mut StateVectors) -> usize {
        self.state_offset = states.allocate(IND_NUM_STATES);
        IND_NUM_STATES
    }

    fn setup_matrix(&mut self, mna: &mut crate::mna::MnaSystem) {
        // Pre-allocate all elements (ngspice INDsetup TSTALLOC)
        mna.make_element(self.pos_node, self.branch_eq);
        mna.make_element(self.neg_node, self.branch_eq);
        mna.make_element(self.branch_eq, self.pos_node);
        mna.make_element(self.branch_eq, self.neg_node);
        mna.make_element(self.branch_eq, self.branch_eq);
    }

    /// First pass of INDload (indload.c:41-49): compute flux from branch current.
    ///
    /// This runs BEFORE mutual inductors, so the flux starts with just L*i.
    /// Mutual inductors then add their contribution (MUTfactor * i_other).
    fn pre_load(
        &mut self,
        mna: &mut MnaSystem,
        states: &mut StateVectors,
        mode: Mode,
    ) {
        let flux = self.state_offset;

        // indload.c:41: if(!(ckt->CKTmode & (MODEDC|MODEINITPRED)))
        if !mode.is(MODEDC) && !mode.is(MODEINITPRED) {
            if mode.is(MODEUIC) && mode.is(MODEINITTRAN) {
                // indload.c:42-44: flux = INDinduct * INDinitCond
                // Note: ngspice uses INDinduct/m but we don't support m yet
                let ic = self.ic.unwrap_or(0.0);
                states.set(0, flux, self.inductance * ic);
            } else {
                // indload.c:46-48: flux = INDinduct * i_branch
                // Note: ngspice uses INDinduct/m but we don't support m yet
                let i_branch = mna.rhs_old_val(self.branch_eq);
                states.set(0, flux, self.inductance * i_branch);
            }
        }
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
        let b = self.branch_eq;
        let flux = self.state_offset;

        // Third pass of INDload (indload.c:84-153).
        // At this point, state0[flux] contains L*i + mutual contributions.

        if mode.is(MODEDC) {
            // DC: inductor is short circuit (indload.c:93-95)
            // req = 0, veq = 0 → only topology stamps below
        } else {
            // indload.c:99-110: predictor / init copies
            if mode.is(MODEINITPRED) {
                // indload.c:100-101: state0[flux] = state1[flux]
                let f1 = states.get(1, flux);
                states.set(0, flux, f1);
            } else {
                if mode.is(MODEINITTRAN) {
                    // indload.c:105-106: state1[flux] = state0[flux]
                    let f0 = states.get(0, flux);
                    states.set(1, flux, f0);
                }
            }

            // indload.c:112-113: NIintegrate
            let newmind = self.inductance; // INDinduct/m, m=1
            let (req, veq) = ni_integrate(&self.ag, states, newmind, flux, self.order);

            // indload.c:117: rhs[brEq] += veq
            mna.stamp_rhs(b, veq);

            // indload.c:142-145: copy state1[volt] = state0[volt] at INITTRAN
            if mode.is(MODEINITTRAN) {
                let volt = self.state_offset + 1;
                let v0 = states.get(0, volt);
                states.set(1, volt, v0);
            }

            // indload.c:151: ibrIbrPtr -= req
            mna.stamp(b, b, -req);
        }

        // Stamp branch current topology (always, indload.c:147-150)
        mna.stamp(p, b, 1.0);
        mna.stamp(n, b, -1.0);
        mna.stamp(b, p, 1.0);
        mna.stamp(b, n, -1.0);

        Ok(())
    }

    /// Port of INDacLoad from indacld.c.
    /// Stamps: topology +-1 in real, -omega*L in imaginary on branch diagonal.
    fn ac_load(
        &mut self,
        mna: &mut MnaSystem,
        _states: &crate::state::StateVectors,
        omega: f64,
    ) -> Result<(), SimError> {
        let val = omega * self.inductance;
        let p = self.pos_node;
        let n = self.neg_node;
        let b = self.branch_eq;

        mna.stamp(p, b, 1.0);
        mna.stamp(n, b, -1.0);
        mna.stamp(b, p, 1.0);
        mna.stamp(b, n, -1.0);
        mna.stamp_imag(b, b, -val);
        Ok(())
    }

    /// Port of INDpzLoad from indpzld.c.
    /// Topology stamps +-1, branch diagonal gets -s*L.
    fn pz_load(
        &mut self,
        mna: &mut MnaSystem,
        s_re: f64,
        s_im: f64,
    ) -> Result<(), SimError> {
        let val = self.inductance; // ind / m (m=1)
        let p = self.pos_node;
        let n = self.neg_node;
        let b = self.branch_eq;

        mna.stamp(p, b, 1.0);
        mna.stamp(n, b, -1.0);
        mna.stamp(b, p, 1.0);
        mna.stamp(b, n, -1.0);
        mna.stamp(b, b, -(val * s_re));
        mna.stamp_imag(b, b, -(val * s_im));
        Ok(())
    }
}

const MODEUIC: u32 = 0x10000;
