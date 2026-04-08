use crate::device::Device;
use crate::error::SimError;
use crate::mna::MnaSystem;
use crate::mode::Mode;
use crate::state::StateVectors;

/// VCCS (G element) — Voltage-Controlled Current Source.
/// Port of ngspice vccs/vccsload.c.
///
/// I(pos→neg) = gm * (V(cont_pos) - V(cont_neg))
///
/// No branch equation needed. Stamps transconductance directly:
/// ```text
/// G[pos, cont_pos] += gm    G[pos, cont_neg] -= gm
/// G[neg, cont_pos] -= gm    G[neg, cont_neg] += gm
/// ```
#[derive(Debug)]
pub struct Vccs {
    name: String,
    pos_node: usize,
    neg_node: usize,
    cont_pos: usize,
    cont_neg: usize,
    gm: f64,
}

impl Vccs {
    pub fn new(
        name: impl Into<String>,
        pos_node: usize,
        neg_node: usize,
        cont_pos: usize,
        cont_neg: usize,
        gm: f64,
    ) -> Self {
        Self { name: name.into(), pos_node, neg_node, cont_pos, cont_neg, gm }
    }
}

impl Device for Vccs {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn name(&self) -> &str { &self.name }
    fn setup_matrix(&mut self, mna: &mut crate::mna::MnaSystem) {
        mna.make_element(self.pos_node, self.cont_pos);
        mna.make_element(self.pos_node, self.cont_neg);
        mna.make_element(self.neg_node, self.cont_pos);
        mna.make_element(self.neg_node, self.cont_neg);
    }
    fn load(&mut self, mna: &mut MnaSystem, _states: &mut StateVectors, _mode: Mode, _src_fact: f64, _gmin: f64, _noncon: &mut bool) -> Result<(), SimError> {
        let p = self.pos_node;
        let n = self.neg_node;
        let cp = self.cont_pos;
        let cn = self.cont_neg;

        // vccsload.c:34-37
        mna.stamp(p, cp, self.gm);
        mna.stamp(p, cn, -self.gm);
        mna.stamp(n, cp, -self.gm);
        mna.stamp(n, cn, self.gm);
        Ok(())
    }

    fn ac_load(&mut self, mna: &mut MnaSystem, _states: &crate::state::StateVectors, _omega: f64) -> Result<(), SimError> {
        mna.stamp(self.pos_node, self.cont_pos, self.gm);
        mna.stamp(self.pos_node, self.cont_neg, -self.gm);
        mna.stamp(self.neg_node, self.cont_pos, -self.gm);
        mna.stamp(self.neg_node, self.cont_neg, self.gm);
        Ok(())
    }

    /// Port of VCCSpzLoad from vccspzld.c — same as regular load.
    fn pz_load(&mut self, mna: &mut MnaSystem, _s_re: f64, _s_im: f64) -> Result<(), SimError> {
        mna.stamp(self.pos_node, self.cont_pos, self.gm);
        mna.stamp(self.pos_node, self.cont_neg, -self.gm);
        mna.stamp(self.neg_node, self.cont_pos, -self.gm);
        mna.stamp(self.neg_node, self.cont_neg, self.gm);
        Ok(())
    }
}
