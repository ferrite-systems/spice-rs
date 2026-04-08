use crate::device::Device;
use crate::error::SimError;
use crate::mna::MnaSystem;
use crate::mode::Mode;
use crate::state::StateVectors;

/// CCCS (F element) — Current-Controlled Current Source.
/// Port of ngspice cccs/cccsload.c.
///
/// I(pos→neg) = gain * I(controlling_branch)
///
/// The controlling branch must be a voltage source branch equation.
/// Stamps gain from controlling branch to output nodes:
/// ```text
/// G[pos, cont_branch] += gain
/// G[neg, cont_branch] -= gain
/// ```
#[derive(Debug)]
pub struct Cccs {
    name: String,
    pos_node: usize,
    neg_node: usize,
    cont_branch: usize,
    gain: f64,
}

impl Cccs {
    pub fn new(
        name: impl Into<String>,
        pos_node: usize,
        neg_node: usize,
        cont_branch: usize,
        gain: f64,
    ) -> Self {
        Self { name: name.into(), pos_node, neg_node, cont_branch, gain }
    }
}

impl Device for Cccs {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn name(&self) -> &str { &self.name }
    fn setup_matrix(&mut self, mna: &mut crate::mna::MnaSystem) {
        mna.make_element(self.pos_node, self.cont_branch);
        mna.make_element(self.neg_node, self.cont_branch);
    }
    fn load(&mut self, mna: &mut MnaSystem, _states: &mut StateVectors, _mode: Mode, _src_fact: f64, _gmin: f64, _noncon: &mut bool) -> Result<(), SimError> {
        let p = self.pos_node;
        let n = self.neg_node;
        let cb = self.cont_branch;

        // cccsload.c:35-36
        mna.stamp(p, cb, self.gain);
        mna.stamp(n, cb, -self.gain);
        Ok(())
    }

    fn ac_load(&mut self, mna: &mut MnaSystem, _states: &crate::state::StateVectors, _omega: f64) -> Result<(), SimError> {
        mna.stamp(self.pos_node, self.cont_branch, self.gain);
        mna.stamp(self.neg_node, self.cont_branch, -self.gain);
        Ok(())
    }
}
