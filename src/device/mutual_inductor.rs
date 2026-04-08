use crate::device::Device;
use crate::error::SimError;
use crate::mna::MnaSystem;
use crate::mode::*;
use crate::state::StateVectors;

/// Mutual inductor — port of ngspice mutual inductor (ind/indload.c:52-82, mutsetup.c, muttemp.c).
///
/// Couples two inductors L1 and L2 with coupling coefficient k.
/// The mutual inductance M = k * sqrt(|L1 * L2|) (muttemp.c:56).
///
/// In ngspice, INDload handles both inductors and mutual inductors in one function
/// with three loops: (1) inductors compute flux, (2) mutual inductors add flux +
/// stamp matrix, (3) inductors integrate + stamp. In spice-rs, we split this:
/// - Inductor::pre_load() = loop 1
/// - MutualInductor::pre_load() = loop 2 flux part
/// - MutualInductor::load() = loop 2 matrix stamp (indload.c:80-81)
/// - Inductor::load() = loop 3
///
/// Since mutual inductors have type_order 30 (after inductors at 29), the pre_load
/// sequence is: all inductors compute flux, then all mutual inductors add cross-flux.
/// The load sequence doesn't matter for correctness since inductors already have
/// full flux by then.
#[derive(Debug)]
pub struct MutualInductor {
    name: String,
    coupling: f64,
    /// Mutual inductance factor: k * sqrt(|L1 * L2|) — computed in temperature().
    factor: f64,
    /// Inductance values of L1 and L2 (for computing factor).
    ind1_value: f64,
    ind2_value: f64,
    /// Branch equation of inductor 1.
    ind1_branch: usize,
    /// Branch equation of inductor 2.
    ind2_branch: usize,
    /// State offset of inductor 1's flux (INDflux).
    ind1_flux_offset: usize,
    /// State offset of inductor 2's flux (INDflux).
    ind2_flux_offset: usize,
    /// Initial condition of inductor 1 (for UIC).
    ind1_ic: Option<f64>,
    /// Initial condition of inductor 2 (for UIC).
    ind2_ic: Option<f64>,
    /// Integration coefficients — shared with inductors (set by transient engine).
    pub ag: [f64; 7],
}

impl MutualInductor {
    pub fn new(
        name: impl Into<String>,
        coupling: f64,
        ind1_value: f64,
        ind2_value: f64,
        ind1_branch: usize,
        ind2_branch: usize,
        ind1_flux_offset: usize,
        ind2_flux_offset: usize,
        ind1_ic: Option<f64>,
        ind2_ic: Option<f64>,
    ) -> Self {
        // Compute factor immediately (same as muttemp.c:56)
        let factor = coupling * (ind1_value * ind2_value).abs().sqrt();
        Self {
            name: name.into(),
            coupling,
            factor,
            ind1_value,
            ind2_value,
            ind1_branch,
            ind2_branch,
            ind1_flux_offset,
            ind2_flux_offset,
            ind1_ic,
            ind2_ic,
            ag: [0.0; 7],
        }
    }
}

const MODEUIC: u32 = 0x10000;

impl Device for MutualInductor {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn name(&self) -> &str {
        &self.name
    }

    fn setup_matrix(&mut self, mna: &mut MnaSystem) {
        // mutsetup.c:63-64: allocate cross-coupling elements
        mna.make_element(self.ind1_branch, self.ind2_branch);
        mna.make_element(self.ind2_branch, self.ind1_branch);
    }

    fn temperature(&mut self, _temp: f64, _tnom: f64) {
        // muttemp.c:56: MUTfactor = MUTcoupling * sqrt(fabs(ind1 * ind2))
        self.factor = self.coupling * (self.ind1_value * self.ind2_value).abs().sqrt();
    }

    /// Mutual flux contribution — port of indload.c:60-78.
    ///
    /// Runs during the pre_load pass, AFTER inductors have set flux = L*i.
    /// Adds the cross-coupling flux: flux1 += M*i2, flux2 += M*i1.
    fn pre_load(
        &mut self,
        mna: &mut MnaSystem,
        states: &mut StateVectors,
        mode: Mode,
    ) {
        // indload.c:61: if(!(ckt->CKTmode & (MODEDC|MODEINITPRED)))
        if !mode.is(MODEDC) && !mode.is(MODEINITPRED) {
            if mode.is(MODEUIC) && mode.is(MODEINITTRAN) {
                // indload.c:64-68: UIC initial conditions
                // flux1 += MUTfactor * ind2_ic
                // flux2 += MUTfactor * ind1_ic
                let ic2 = self.ind2_ic.unwrap_or(0.0);
                let ic1 = self.ind1_ic.unwrap_or(0.0);
                let f1 = states.get(0, self.ind1_flux_offset);
                states.set(0, self.ind1_flux_offset, f1 + self.factor * ic2);
                let f2 = states.get(0, self.ind2_flux_offset);
                states.set(0, self.ind2_flux_offset, f2 + self.factor * ic1);
            } else {
                // indload.c:71-78: normal mutual flux contribution
                // flux1 += MUTfactor * i_branch2
                // flux2 += MUTfactor * i_branch1
                let i1 = mna.rhs_old_val(self.ind1_branch);
                let i2 = mna.rhs_old_val(self.ind2_branch);
                let f1 = states.get(0, self.ind1_flux_offset);
                states.set(0, self.ind1_flux_offset, f1 + self.factor * i2);
                let f2 = states.get(0, self.ind2_flux_offset);
                states.set(0, self.ind2_flux_offset, f2 + self.factor * i1);
            }
        }
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
        // indload.c:80-81: matrix stamp (unconditional)
        // *(muthere->MUTbr1br2Ptr) -= muthere->MUTfactor*ckt->CKTag[0];
        // *(muthere->MUTbr2br1Ptr) -= muthere->MUTfactor*ckt->CKTag[0];
        let stamp_val = -(self.factor * self.ag[0]);
        mna.stamp(self.ind1_branch, self.ind2_branch, stamp_val);
        mna.stamp(self.ind2_branch, self.ind1_branch, stamp_val);

        Ok(())
    }

    /// Port of MUTacLoad from mutacld.c.
    /// Stamps: -omega*factor into imaginary part of branch cross-coupling.
    fn ac_load(
        &mut self,
        mna: &mut MnaSystem,
        _states: &crate::state::StateVectors,
        omega: f64,
    ) -> Result<(), SimError> {
        let val = omega * self.factor;
        mna.stamp_imag(self.ind1_branch, self.ind2_branch, -val);
        mna.stamp_imag(self.ind2_branch, self.ind1_branch, -val);
        Ok(())
    }
}
