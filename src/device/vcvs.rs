use crate::device::Device;
use crate::error::SimError;
use crate::mna::MnaSystem;
use crate::mode::Mode;
use crate::state::StateVectors;

/// VCVS (E element) — Voltage-Controlled Voltage Source.
/// Port of ngspice vcvs/vcvsload.c.
///
/// V(pos) - V(neg) = gain * (V(cont_pos) - V(cont_neg))
///
/// Uses a branch equation. Stamps:
/// ```text
/// G[pos, branch] += 1     G[neg, branch] -= 1
/// G[branch, pos] += 1     G[branch, neg] -= 1
/// G[branch, cont_pos] -= gain   G[branch, cont_neg] += gain
/// ```
#[derive(Debug)]
pub struct Vcvs {
    name: String,
    pos_node: usize,
    neg_node: usize,
    cont_pos: usize,
    cont_neg: usize,
    branch_eq: usize,
    gain: f64,
}

impl Vcvs {
    pub fn new(
        name: impl Into<String>,
        pos_node: usize,
        neg_node: usize,
        cont_pos: usize,
        cont_neg: usize,
        branch_eq: usize,
        gain: f64,
    ) -> Self {
        Self { name: name.into(), pos_node, neg_node, cont_pos, cont_neg, branch_eq, gain }
    }
}

impl Device for Vcvs {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn name(&self) -> &str { &self.name }
    fn setup_matrix(&mut self, mna: &mut crate::mna::MnaSystem) {
        mna.make_element(self.pos_node, self.branch_eq);
        mna.make_element(self.neg_node, self.branch_eq);
        mna.make_element(self.branch_eq, self.pos_node);
        mna.make_element(self.branch_eq, self.neg_node);
        mna.make_element(self.branch_eq, self.cont_pos);
        mna.make_element(self.branch_eq, self.cont_neg);
    }
    fn load(&mut self, mna: &mut MnaSystem, _states: &mut StateVectors, _mode: Mode, _src_fact: f64, _gmin: f64, _noncon: &mut bool) -> Result<(), SimError> {
        let p = self.pos_node;
        let n = self.neg_node;
        let cp = self.cont_pos;
        let cn = self.cont_neg;
        let b = self.branch_eq;

        // vcvsload.c:34-39
        mna.stamp(p, b, 1.0);
        mna.stamp(n, b, -1.0);
        mna.stamp(b, p, 1.0);
        mna.stamp(b, n, -1.0);
        mna.stamp(b, cp, -self.gain);
        mna.stamp(b, cn, self.gain);
        Ok(())
    }

    /// VCVS acLoad uses the same stamps as the regular load.
    fn ac_load(&mut self, mna: &mut MnaSystem, _states: &crate::state::StateVectors, _omega: f64) -> Result<(), SimError> {
        mna.stamp(self.pos_node, self.branch_eq, 1.0);
        mna.stamp(self.neg_node, self.branch_eq, -1.0);
        mna.stamp(self.branch_eq, self.pos_node, 1.0);
        mna.stamp(self.branch_eq, self.neg_node, -1.0);
        mna.stamp(self.branch_eq, self.cont_pos, -self.gain);
        mna.stamp(self.branch_eq, self.cont_neg, self.gain);
        Ok(())
    }

    /// Port of VCVSpzLoad from vcvspzld.c — same as regular load.
    fn pz_load(&mut self, mna: &mut MnaSystem, _s_re: f64, _s_im: f64) -> Result<(), SimError> {
        mna.stamp(self.pos_node, self.branch_eq, 1.0);
        mna.stamp(self.neg_node, self.branch_eq, -1.0);
        mna.stamp(self.branch_eq, self.pos_node, 1.0);
        mna.stamp(self.branch_eq, self.neg_node, -1.0);
        mna.stamp(self.branch_eq, self.cont_pos, -self.gain);
        mna.stamp(self.branch_eq, self.cont_neg, self.gain);
        Ok(())
    }
}
