use crate::device::Device;
use crate::error::SimError;
use crate::mna::MnaSystem;
use crate::mode::Mode;
use crate::state::StateVectors;

/// CCVS (H element) — Current-Controlled Voltage Source.
/// Port of ngspice ccvs/ccvsload.c.
///
/// V(pos) - V(neg) = transresistance * I(controlling_branch)
///
/// Uses a branch equation. Stamps:
/// ```text
/// G[pos, branch] += 1     G[neg, branch] -= 1
/// G[branch, pos] += 1     G[branch, neg] -= 1
/// G[branch, cont_branch] -= transresistance
/// ```
#[derive(Debug)]
pub struct Ccvs {
    name: String,
    pos_node: usize,
    neg_node: usize,
    branch_eq: usize,
    cont_branch: usize,
    transresistance: f64,
}

impl Ccvs {
    pub fn new(
        name: impl Into<String>,
        pos_node: usize,
        neg_node: usize,
        branch_eq: usize,
        cont_branch: usize,
        transresistance: f64,
    ) -> Self {
        Self { name: name.into(), pos_node, neg_node, branch_eq, cont_branch, transresistance }
    }
}

impl Device for Ccvs {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn name(&self) -> &str { &self.name }
    fn setup_matrix(&mut self, mna: &mut crate::mna::MnaSystem) {
        mna.make_element(self.pos_node, self.branch_eq);
        mna.make_element(self.neg_node, self.branch_eq);
        mna.make_element(self.branch_eq, self.pos_node);
        mna.make_element(self.branch_eq, self.neg_node);
        mna.make_element(self.branch_eq, self.cont_branch);
    }
    fn load(&mut self, mna: &mut MnaSystem, _states: &mut StateVectors, _mode: Mode, _src_fact: f64, _gmin: f64, _noncon: &mut bool) -> Result<(), SimError> {
        let p = self.pos_node;
        let n = self.neg_node;
        let b = self.branch_eq;
        let cb = self.cont_branch;

        // ccvsload.c:35-39
        mna.stamp(p, b, 1.0);
        mna.stamp(n, b, -1.0);
        mna.stamp(b, p, 1.0);
        mna.stamp(b, n, -1.0);
        mna.stamp(b, cb, -self.transresistance);
        Ok(())
    }

    fn ac_load(&mut self, mna: &mut MnaSystem, _states: &crate::state::StateVectors, _omega: f64) -> Result<(), SimError> {
        mna.stamp(self.pos_node, self.branch_eq, 1.0);
        mna.stamp(self.neg_node, self.branch_eq, -1.0);
        mna.stamp(self.branch_eq, self.pos_node, 1.0);
        mna.stamp(self.branch_eq, self.neg_node, -1.0);
        mna.stamp(self.branch_eq, self.cont_branch, -self.transresistance);
        Ok(())
    }
}
