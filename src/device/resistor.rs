use crate::device::Device;
use crate::error::SimError;
use crate::mna::MnaSystem;
use crate::mode::Mode;
use crate::state::StateVectors;

/// Resistor device — port of ngspice res/resload.c.
///
/// Stamps a 2x2 conductance matrix:
/// ```text
/// G[pos,pos] += G    G[pos,neg] -= G
/// G[neg,pos] -= G    G[neg,neg] += G
/// ```
#[derive(Debug)]
pub struct Resistor {
    name: String,
    pos_node: usize,
    neg_node: usize,
    conductance: f64,
    /// AC conductance — differs from DC conductance when `ac=` is specified.
    /// Port of ngspice RESacConduct / RESacResist.
    ac_conductance: f64,
}

impl Resistor {
    pub fn new(name: impl Into<String>, pos_node: usize, neg_node: usize, resistance: f64, ac_resistance: Option<f64>) -> Self {
        let conductance = 1.0 / resistance;
        // Port of restemp.c: if RESacresGiven, compute separate ac conductance;
        // otherwise ac_conductance = conductance.
        let ac_conductance = match ac_resistance {
            Some(ac_res) => 1.0 / ac_res,
            None => conductance,
        };
        Self {
            name: name.into(),
            pos_node,
            neg_node,
            conductance,
            ac_conductance,
        }
    }
}

impl Device for Resistor {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn name(&self) -> &str { &self.name }

    fn setup_matrix(&mut self, mna: &mut crate::mna::MnaSystem) {
        let p = self.pos_node; let n = self.neg_node;
        mna.make_element(p, p); mna.make_element(n, n);
        mna.make_element(p, n); mna.make_element(n, p);
    }

    fn load(
        &mut self,
        mna: &mut MnaSystem,
        _states: &mut StateVectors,
        _mode: Mode,
        _src_fact: f64,
        _gmin: f64,
        _noncon: &mut bool,
    ) -> Result<(), SimError> {
        // Stamp conductance (resload.c:27-31)
        let g = self.conductance;
        mna.stamp(self.pos_node, self.pos_node, g);
        mna.stamp(self.neg_node, self.neg_node, g);
        mna.stamp(self.pos_node, self.neg_node, -g);
        mna.stamp(self.neg_node, self.pos_node, -g);
        Ok(())
    }

    fn sensitivity_params(&self) -> Vec<(String, u32)> {
        // Instance params: "!" prefix = principal (just device name)
        // Only include the principal resistance; model params like r, rsh, tc1, tc2
        // are not perturbable in our simplified model.
        vec![
            ("!resistance".to_string(), 1),  // principal: just device name
        ]
    }

    fn get_param(&self, id: u32) -> Option<f64> {
        match id {
            1 | 100 => Some(1.0 / self.conductance), // resistance
            _ => None,
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        match id {
            1 | 100 => self.conductance = 1.0 / value, // resistance
            _ => {}
        }
    }

    fn load_into_dense(
        &mut self,
        collector: &mut crate::analysis::sens::DenseStampCollector<'_>,
        _states: &mut StateVectors,
        _mode: Mode,
        _src_fact: f64,
        _gmin: f64,
        _noncon: &mut bool,
    ) {
        let g = self.conductance;
        collector.stamp(self.pos_node, self.pos_node, g);
        collector.stamp(self.neg_node, self.neg_node, g);
        collector.stamp(self.pos_node, self.neg_node, -g);
        collector.stamp(self.neg_node, self.pos_node, -g);
    }

    /// Port of RESacload from resload.c:44-71.
    /// Uses AC conductance when `ac=` was specified on the resistor instance.
    fn ac_load(
        &mut self,
        mna: &mut MnaSystem,
        _states: &crate::state::StateVectors,
        _omega: f64,
    ) -> Result<(), SimError> {
        // resload.c:59-60: if (here->RESacresGiven) g = here->RESacConduct;
        // else g = here->RESconduct;
        let g = self.ac_conductance;
        mna.stamp(self.pos_node, self.pos_node, g);
        mna.stamp(self.neg_node, self.neg_node, g);
        mna.stamp(self.pos_node, self.neg_node, -g);
        mna.stamp(self.neg_node, self.pos_node, -g);
        Ok(())
    }

    /// Port of RESpzLoad from respzld.c.
    /// Uses AC conductance when `ac=` was specified on the resistor instance.
    fn pz_load(
        &mut self,
        mna: &mut MnaSystem,
        _s_re: f64,
        _s_im: f64,
    ) -> Result<(), SimError> {
        // respzld.c:36-37: if (here->RESacresGiven) g = here->RESacConduct;
        // else g = here->RESconduct;
        let g = self.ac_conductance;
        mna.stamp(self.pos_node, self.pos_node, g);
        mna.stamp(self.neg_node, self.neg_node, g);
        mna.stamp(self.pos_node, self.neg_node, -g);
        mna.stamp(self.neg_node, self.pos_node, -g);
        Ok(())
    }
}
