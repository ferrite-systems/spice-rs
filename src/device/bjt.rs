//! BJT (Gummel-Poon) — port of ngspice bjt/bjtload.c.
//!
//! DC path first. Transient capacitances added later.

use crate::device::Device;
use crate::error::SimError;
use crate::integration::ni_integrate;
use crate::mna::MnaSystem;
use crate::mode::*;
use crate::state::StateVectors;

const BJT_NUM_STATES: usize = 33;

// State offsets — must match ngspice bjtdefs.h exactly
const VBE: usize = 0;
const VBC: usize = 1;
// const VBCX: usize = 2;  // not used without intCollResist
// const VRCI: usize = 3;  // not used without intCollResist
const CC: usize = 4;
const CB: usize = 5;
const GPI: usize = 6;
const GMU: usize = 7;
const GM: usize = 8;
const GO: usize = 9;
const QBE: usize = 10;
const CQBE: usize = 11;
const QBC: usize = 12;
const CQBC: usize = 13;
const QSUB: usize = 14;
const CQSUB: usize = 15;
const QBX: usize = 16;
const CQBX: usize = 17;
const GX: usize = 18;
const CEXBC: usize = 19;
const GEQCB: usize = 20;
const GCSUB: usize = 21;
const GEQBX: usize = 22;
const VSUB: usize = 23;
const CDSUB: usize = 24;
const GDSUB: usize = 25;

use crate::constants::{CHARGE, BOLTZ, KoverQ, REFTEMP};
const MAX_EXP_ARG: f64 = 709.0;

/// BJT model parameters.
#[derive(Debug, Clone)]
pub struct BjtModel {
    pub bjt_type: i32,   // +1 NPN, -1 PNP
    pub is_: f64,        // saturation current
    pub bf: f64,         // forward beta
    pub nf: f64,         // forward emission coeff
    pub br: f64,         // reverse beta
    pub nr: f64,         // reverse emission coeff
    pub ise: f64,        // B-E leakage
    pub ne: f64,         // B-E leakage emission
    pub isc: f64,        // B-C leakage
    pub nc: f64,         // B-C leakage emission
    pub vaf: f64,        // forward Early voltage (0 = infinite)
    pub var: f64,        // reverse Early voltage (0 = infinite)
    pub ikf: f64,        // forward roll-off (0 = infinite)
    pub ikr: f64,        // reverse roll-off (0 = infinite)
    pub rb: f64,         // base resistance
    pub rbm: f64,        // minimum base resistance
    pub re: f64,         // emitter resistance
    pub rc: f64,         // collector resistance
    pub cje: f64,        // B-E junction cap
    pub vje: f64,        // B-E junction potential
    pub mje: f64,        // B-E junction exponent
    pub cjc: f64,        // B-C junction cap
    pub vjc: f64,        // B-C junction potential
    pub mjc: f64,        // B-C junction exponent
    pub xcjc: f64,       // B-C cap fraction to internal
    pub cjs: f64,        // substrate cap
    pub vjs: f64,        // substrate junction potential
    pub mjs: f64,        // substrate junction exponent
    pub tf: f64,         // forward transit time
    pub tr: f64,         // reverse transit time
    pub xtf: f64,        // transit time bias coefficient
    pub vtf: f64,        // transit time VBC factor
    pub itf: f64,        // transit time high-current
    pub eg: f64,         // energy gap
    pub xtb: f64,        // IS temperature exponent
    pub fc: f64,         // forward cap depletion
    pub ptf: f64,        // excess phase (degrees)
    pub tnom: f64,       // nominal temperature (K)
    pub tnom_given: bool,
    // Computed during temperature:
    pub excess_phase_factor: f64,
}

impl Default for BjtModel {
    fn default() -> Self {
        Self {
            bjt_type: 1, // NPN
            is_: 1e-16, bf: 100.0, nf: 1.0, br: 1.0, nr: 1.0,
            ise: 0.0, ne: 1.5, isc: 0.0, nc: 2.0,
            vaf: 0.0, var: 0.0, ikf: 0.0, ikr: 0.0,
            rb: 0.0, rbm: 0.0, re: 0.0, rc: 0.0,
            cje: 0.0, vje: 0.75, mje: 0.33,
            cjc: 0.0, vjc: 0.75, mjc: 0.33, xcjc: 1.0,
            cjs: 0.0, vjs: 0.75, mjs: 0.0,
            tf: 0.0, tr: 0.0, xtf: 0.0, vtf: 0.0, itf: 0.0,
            eg: 1.11, xtb: 0.0, fc: 0.5, ptf: 0.0,
            tnom: 300.15,
            tnom_given: false,
            excess_phase_factor: 0.0,
        }
    }
}

/// BJT device instance.
#[derive(Debug)]
pub struct Bjt {
    name: String,
    // External nodes
    c_node: usize,
    b_node: usize,
    e_node: usize,
    s_node: usize,
    // Internal nodes
    cp_node: usize, // collector prime (or c_node if RC=0)
    bp_node: usize, // base prime (or b_node if RB=0)
    ep_node: usize, // emitter prime (or e_node if RE=0)
    // Model
    model: BjtModel,
    area: f64,
    m: f64,
    // Temperature-corrected
    t_is: f64,           // BJTtSatCur: area * IS * factor (used for vcrit, etc.)
    t_be_sat_cur: f64,   // BJTBEtSatCur: B-E junction saturation current
    t_bc_sat_cur: f64,   // BJTBCtSatCur: B-C junction saturation current
    t_ise: f64,
    t_isc: f64,
    t_bf: f64,
    t_br: f64,
    t_be_pot: f64,
    t_bc_pot: f64,
    t_be_cap: f64,
    t_bc_cap: f64,
    t_dep_cap_be: f64,
    t_dep_cap_bc: f64,
    t_sub_cap: f64,       // substrate cap (temperature-corrected)
    t_sub_pot: f64,       // substrate junction potential (temperature-corrected)
    // Polynomial coefficients for junction cap above depletion (bjttemp.c:316-333)
    tf1: f64,
    tf2: f64,
    tf3: f64,
    tf4: f64, // depCapCoeff * tBCpot
    tf5: f64,
    tf6: f64,
    tf7: f64,
    be_vcrit: f64,
    bc_vcrit: f64,
    rc_cond: f64,
    rb_cond: f64, // 1/RB (if RB > 0)
    re_cond: f64,
    inv_vaf: f64, // 1/VAF (0 if VAF not given)
    inv_var: f64,
    inv_ikf: f64,
    inv_ikr: f64,
    // Last computed values (for parity checking)
    last_gpi: f64,
    last_gmu: f64,
    last_go: f64,
    last_gm: f64,
    last_vbe: f64,
    last_vbc: f64,
    pre_vbe: f64,
    pre_vbc: f64,
    // Device initial conditions (from .IC node voltages or instance params)
    ic_vbe: f64,
    ic_vce: f64,
    ic_vbe_given: bool,
    ic_vce_given: bool,
    // Temperature
    temp: f64,
    // OFF flag (device starts in off state during DC OP)
    pub off: bool,
    // State
    state_offset: usize,
    pub ag: [f64; 7],
    pub order: usize,
    pub delta: f64,
    pub delta_old1: f64,
}

impl Bjt {
    pub fn new(
        name: impl Into<String>,
        c: usize, b: usize, e: usize, s: usize,
        model: BjtModel, area: f64,
    ) -> Self {
        Self {
            name: name.into(),
            c_node: c, b_node: b, e_node: e, s_node: s,
            cp_node: c, bp_node: b, ep_node: e,
            model, area, m: 1.0,
            t_is: 0.0, t_be_sat_cur: 0.0, t_bc_sat_cur: 0.0,
            t_ise: 0.0, t_isc: 0.0,
            t_bf: 0.0, t_br: 0.0,
            t_be_pot: 0.0, t_bc_pot: 0.0,
            t_be_cap: 0.0, t_bc_cap: 0.0,
            t_dep_cap_be: 0.0, t_dep_cap_bc: 0.0, t_sub_cap: 0.0, t_sub_pot: 0.0,
            tf1: 0.0, tf2: 0.0, tf3: 0.0, tf4: 0.0, tf5: 0.0, tf6: 0.0, tf7: 0.0,
            be_vcrit: 0.0, bc_vcrit: 0.0,
            rc_cond: 0.0, rb_cond: 0.0, re_cond: 0.0,
            inv_vaf: 0.0, inv_var: 0.0, inv_ikf: 0.0, inv_ikr: 0.0,
            last_gpi: 0.0, last_gmu: 0.0, last_go: 0.0, last_gm: 0.0,
            last_vbe: 0.0, last_vbc: 0.0, pre_vbe: 0.0, pre_vbc: 0.0,
            ic_vbe: 0.0, ic_vce: 0.0, ic_vbe_given: false, ic_vce_given: false,
            temp: REFTEMP, off: false,
            state_offset: 0,
            ag: [0.0; 7],
            order: 1,
            delta: 0.0,
            delta_old1: 0.0,
        }
    }

    pub fn set_internal_nodes(&mut self, cp: usize, bp: usize, ep: usize) {
        self.cp_node = cp;
        self.bp_node = bp;
        self.ep_node = ep;
    }
}

impl Device for Bjt {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn name(&self) -> &str { &self.name }

    /// BJTgetic (bjtgetic.c): propagate .IC node voltages to device ICs.
    fn setic(&mut self, rhs: &[f64]) {
        if !self.ic_vbe_given {
            self.ic_vbe = rhs[self.b_node] - rhs[self.e_node];
        }
        if !self.ic_vce_given {
            self.ic_vce = rhs[self.c_node] - rhs[self.e_node];
        }
    }

    fn setup(&mut self, states: &mut StateVectors) -> usize {
        self.state_offset = states.allocate(BJT_NUM_STATES);
        BJT_NUM_STATES
    }

    fn setup_matrix(&mut self, mna: &mut MnaSystem) {
        let (c, b, e) = (self.c_node, self.b_node, self.e_node);
        let (cp, bp, ep) = (self.cp_node, self.bp_node, self.ep_node);

        // TSTALLOC order — must match ngspice bjtsetup.c exactly for TRANSLATE parity.
        // Off-diagonals first (bjtsetup.c order)
        mna.make_element(c, cp);    // collCollCX (CX=C)
        mna.make_element(b, bp);    // baseBasePrime
        mna.make_element(e, ep);    // emitEmitPrime
        mna.make_element(cp, c);    // collCXCol
        mna.make_element(cp, bp);   // colPrimeBasePrime
        mna.make_element(cp, ep);   // colPrimeEmitPrime
        mna.make_element(bp, b);    // basePrimeBase
        mna.make_element(bp, cp);   // basePrimeColPrime
        mna.make_element(bp, ep);   // basePrimeEmitPrime
        mna.make_element(ep, e);    // emitPrimeEmit
        mna.make_element(ep, cp);   // emitPrimeColPrime
        mna.make_element(ep, bp);   // emitPrimeBasePrime
        // Diagonals
        mna.make_element(c, c);     // colCol
        mna.make_element(b, b);     // baseBase
        mna.make_element(e, e);     // emitEmit
        mna.make_element(cp, cp);   // colPrimeColPrime (+collCXcollCX since CX=C)
        mna.make_element(bp, bp);   // basePrimeBasePrime
        mna.make_element(ep, ep);   // emitPrimeEmitPrime
        // base-collector cross (for gm stamp / geqbx)
        mna.make_element(b, cp);
        mna.make_element(cp, b);
        // Substrate matrix elements (bjtsetup.c:520-529)
        let s = self.s_node;
        let subst_con = if self.model.bjt_type == 1 { cp } else { bp };
        mna.make_element(s, s);           // substSubst
        mna.make_element(subst_con, s);   // substConSubst
        mna.make_element(s, subst_con);   // substSubstCon
    }

    fn temperature(&mut self, temp: f64, global_tnom: f64) {
        bjt_temp(self, temp, global_tnom);
    }

    fn load(
        &mut self,
        mna: &mut MnaSystem,
        states: &mut StateVectors,
        mode: Mode,
        _src_fact: f64,
        gmin: f64,
        noncon: &mut bool,
    ) -> Result<(), SimError> {
        bjt_load(self, mna, states, mode, gmin, noncon)
    }

    fn conductances(&self) -> Vec<(&str, f64)> {
        vec![
            ("gpi", self.last_gpi), ("gmu", self.last_gmu),
            ("go", self.last_go), ("gm", self.last_gm),
        ]
    }

    fn limited_voltages(&self) -> Vec<(&str, f64)> {
        vec![
            ("vbe", self.last_vbe), ("vbc", self.last_vbc),
            ("pre_vbe", self.pre_vbe), ("pre_vbc", self.pre_vbc),
        ]
    }

    /// Port of BJTconvTest (bjtconv.c) — per-device convergence check.
    /// NOTE: BJT uses CKTrhsOld (rhs_old_val) not CKTrhs (rhs_val).
    fn conv_test(&self, mna: &MnaSystem, states: &StateVectors, reltol: f64, abstol: f64) -> bool {
        let tp = self.model.bjt_type as f64;
        let so = self.state_offset;

        // BJTconvTest uses CKTrhsOld (previous iteration solution)
        let vbe = tp * (mna.rhs_old_val(self.bp_node) - mna.rhs_old_val(self.ep_node));
        let vbc = tp * (mna.rhs_old_val(self.bp_node) - mna.rhs_old_val(self.cp_node));

        let delvbe = vbe - states.get(0, so + VBE);
        let delvbc = vbc - states.get(0, so + VBC);

        let cc = states.get(0, so + CC);
        let cb = states.get(0, so + CB);
        let gm = states.get(0, so + GM);
        let go = states.get(0, so + GO);
        let gmu = states.get(0, so + GMU);
        let gpi = states.get(0, so + GPI);

        let cchat = cc + (gm + go) * delvbe - (go + gmu) * delvbc;
        let cbhat = cb + gpi * delvbe + gmu * delvbc;

        // Check collector current convergence
        let tol = reltol * f64::max(cchat.abs(), cc.abs()) + abstol;
        if (cchat - cc).abs() > tol {
            return false;
        }

        // Check base current convergence
        let tol = reltol * f64::max(cbhat.abs(), cb.abs()) + abstol;
        if (cbhat - cb).abs() > tol {
            return false;
        }

        true
    }

    fn model_params(&self) -> Vec<(&str, f64)> {
        let m = &self.model;
        vec![
            ("bf", m.bf), ("nf", m.nf), ("br", m.br), ("nr", m.nr),
            ("rb", m.rb), ("re", m.re), ("rc", m.rc),
            ("vaf", m.vaf), ("var", m.var), ("ikf", m.ikf), ("ikr", m.ikr),
            ("ne", m.ne), ("nc", m.nc),
        ]
    }

    /// AC small-signal load — port of bjtacld.c.
    /// Stamps the BJT small-signal equivalent circuit into the complex matrix.
    /// Must be in `impl Device for Bjt`, NOT `impl Bjt`.
    fn ac_load(
        &mut self,
        mna: &mut MnaSystem,
        states: &crate::state::StateVectors,
        omega: f64,
    ) -> Result<(), SimError> {
        let m = &self.model;
        let so = self.state_offset;

        let gcpr = self.rc_cond;  // BJTtcollectorConduct
        let gepr = self.re_cond;  // BJTtemitterConduct
        let gpi = states.get(0, so + GPI);
        let gmu = states.get(0, so + GMU);
        let mut gm = states.get(0, so + GM);
        let go = states.get(0, so + GO);
        let gx = states.get(0, so + GX);

        // Excess phase (bjtacld.c:56-63)
        let mut xgm = 0.0;
        let td = m.excess_phase_factor;
        if td != 0.0 {
            let arg = td * omega;
            gm = gm + go;
            xgm = -gm * arg.sin();
            gm = gm * arg.cos() - go;
        }

        // Capacitive susceptances (bjtacld.c:64-70)
        let xcpi = states.get(0, so + CQBE) * omega;   // capbe * omega
        let xcmu = states.get(0, so + CQBC) * omega;   // capbc * omega
        let xcbx = states.get(0, so + CQBX) * omega;   // capbx * omega
        let xcsub = states.get(0, so + CQSUB) * omega; // capsub * omega
        let xcmcb = states.get(0, so + CEXBC) * omega;  // geqcb * omega

        let (c, b, e) = (self.c_node, self.b_node, self.e_node);
        let (cp, bp, ep) = (self.cp_node, self.bp_node, self.ep_node);
        let s = self.s_node;
        // Substrate connection: colPrime for NPN, basePrime for PNP
        let subst_con = if m.bjt_type == 1 { cp } else { bp };

        // Real stamps — diagonal (bjtacld.c:72-83)
        mna.stamp(c, c, gcpr);                                     // colColPtr
        mna.stamp(b, b, gx);                                       // baseBasePtr
        mna.stamp(e, e, gepr);                                     // emitEmitPtr
        mna.stamp(cp, cp, gmu + go + gcpr);                        // colPrimeColPrimePtr + collCXcollCXPtr
        mna.stamp(bp, bp, gx + gpi + gmu);                         // basePrimeBasePrimePtr
        mna.stamp(ep, ep, gpi + gepr + gm + go);                   // emitPrimeEmitPrimePtr

        // Imaginary stamps — diagonal (bjtacld.c:74,78-83)
        mna.stamp_imag(b, b, xcbx);                                // baseBasePtr+1
        mna.stamp_imag(cp, cp, xcmu + xcbx);                       // colPrimeColPrimePtr+1
        mna.stamp_imag(subst_con, subst_con, xcsub);               // substConSubstConPtr+1
        mna.stamp_imag(bp, bp, xcpi + xcmu + xcmcb);               // basePrimeBasePrimePtr+1
        mna.stamp_imag(ep, ep, xcpi + xgm);                        // emitPrimeEmitPrimePtr+1

        // Real stamps — off-diagonal (bjtacld.c:84-101)
        mna.stamp(c, cp, -gcpr);                                   // collCollCXPtr
        mna.stamp(b, bp, -gx);                                     // baseBasePrimePtr
        mna.stamp(e, ep, -gepr);                                   // emitEmitPrimePtr
        mna.stamp(cp, c, -gcpr);                                   // collCXCollPtr
        mna.stamp(cp, bp, -gmu + gm);                              // colPrimeBasePrimePtr
        mna.stamp(cp, ep, -gm - go);                               // colPrimeEmitPrimePtr
        mna.stamp(bp, b, -gx);                                     // basePrimeBasePtr
        mna.stamp(bp, cp, -gmu);                                   // basePrimeColPrimePtr
        mna.stamp(bp, ep, -gpi);                                   // basePrimeEmitPrimePtr
        mna.stamp(ep, e, -gepr);                                   // emitPrimeEmitPtr
        mna.stamp(ep, cp, -go);                                    // emitPrimeColPrimePtr
        mna.stamp(ep, bp, -gpi - gm);                              // emitPrimeBasePrimePtr

        // Imaginary stamps — off-diagonal (bjtacld.c:89-101)
        mna.stamp_imag(cp, bp, -xcmu + xgm);                       // colPrimeBasePrimePtr+1
        mna.stamp_imag(cp, ep, -xgm);                              // colPrimeEmitPrimePtr+1
        mna.stamp_imag(bp, cp, -xcmu - xcmcb);                     // basePrimeColPrimePtr+1
        mna.stamp_imag(bp, ep, -xcpi);                              // basePrimeEmitPrimePtr+1
        mna.stamp_imag(ep, cp, xcmcb);                              // emitPrimeColPrimePtr+1
        mna.stamp_imag(ep, bp, -xcpi - xgm - xcmcb);               // emitPrimeBasePrimePtr+1

        // Substrate stamps — imaginary (bjtacld.c:102-104)
        mna.stamp_imag(s, s, xcsub);                                // substSubstPtr+1
        mna.stamp_imag(subst_con, s, -xcsub);                       // substConSubstPtr+1
        mna.stamp_imag(s, subst_con, -xcsub);                       // substSubstConPtr+1

        // Base-colPrime cross capacitance — imaginary (bjtacld.c:105-106)
        mna.stamp_imag(b, cp, -xcbx);                               // baseColPrimePtr+1
        mna.stamp_imag(cp, b, -xcbx);                               // colPrimeBasePtr+1

        Ok(())
    }
}

impl Bjt {
    /// Returns state offsets for charges used in LTE truncation (bjttrunc.c).
    /// ngspice BJTtrunc checks: qbe, qbc, qsub (when CJS given), qbx.
    pub fn qcap_offsets(&self) -> Vec<usize> {
        let so = self.state_offset;
        let mut offsets = vec![so + QBE, so + QBC];
        // Substrate cap — always include for consistency with ngspice
        offsets.push(so + QSUB);
        offsets.push(so + QBX);
        offsets
    }
}

fn bjt_temp(dev: &mut Bjt, temp: f64, global_tnom: f64) {
    dev.temp = temp;
    // Excess phase factor (bjttemp.c:56-57) — compute before immutable borrow of model
    dev.model.excess_phase_factor = (dev.model.ptf / (180.0 / std::f64::consts::PI)) * dev.model.tf;
    let m = &dev.model;
    let tnom = if m.tnom_given { m.tnom } else { global_tnom };
    // ngspice: vt = BJTtemp * CONSTKoverQ (bjttemp.c:149)
    // Must use precomputed KoverQ for FP parity: temp*(BOLTZ/CHARGE) != (BOLTZ*temp)/CHARGE
    let vt = temp * KoverQ;
    let vt_nom = KoverQ * tnom; // ngspice: vtnom = CONSTKoverQ * BJTtnom (bjttemp.c:44)
    let ratio = temp / tnom;
    let eg = 1.16 - 7.02e-4 * temp * temp / (temp + 1108.0);
    let eg_nom = 1.16 - 7.02e-4 * tnom * tnom / (tnom + 1108.0);

    // IS temperature scaling (bjttemp.c:162-168, tlev=0)
    // BJTtempExpIS = XTI (default 3), BJTenergyGap = EG (default 1.11)
    let ratlog = ratio.ln();
    let ratio1 = ratio - 1.0;
    let xti = 3.0; // default; TODO: parse XTI from model
    let factlog = ratio1 * m.eg / vt + xti * ratlog;
    let factor = factlog.exp();
    // ngspice: BJTarea * BJTsatCur * factor (bjttemp.c:168)
    dev.t_is = dev.area * m.is_ * factor;
    // BEtSatCur and BCtSatCur: when BEsatCur/BCsatCur not given, both = tSatCur,
    // BUT then BCtSatCur *= areab unconditionally (bjttemp.c:198-202).
    // areab defaults to area (bjtsetup.c:414-416), subs defaults to VERTICAL.
    dev.t_be_sat_cur = dev.t_is; // bjttemp.c:173
    dev.t_bc_sat_cur = dev.t_is * dev.area; // bjttemp.c:179 + 198-199: *= areab (=area)
    // Leakage currents scale with factlog/emission_coeff (bjttemp.c:246-249)
    let bfactor = (ratlog * m.xtb).exp(); // bjttemp.c:232
    // ngspice: BJTarea * BJTleakBEcurrent * exp(factlog/NE) / bfactor (bjttemp.c:246-249)
    dev.t_ise = dev.area * m.ise * (factlog / m.ne).exp() / bfactor;
    // ngspice: BJTleakBCcurrent * exp(factlog/NC) / bfactor, then *= areab
    // areab defaults to area (bjtsetup.c:414-416), subs defaults to VERTICAL
    dev.t_isc = dev.area * m.isc * (factlog / m.nc).exp() / bfactor;

    // BF/BR temperature scaling (bjttemp.c:231-243, tlev=0)
    dev.t_bf = m.bf * bfactor;
    dev.t_br = m.br * bfactor;

    // Junction potentials temperature scaling (bjttemp.c:149-162, tlevc=0)
    // FP eval order must match C exactly: arg = -egfet/(2*CONSTboltz*temp) + ...
    let fact1 = tnom / REFTEMP;
    let fact2 = temp / REFTEMP;
    let arg = -eg / (2.0 * BOLTZ * temp)
        + 1.1150877 / (BOLTZ * (REFTEMP + REFTEMP));
    let pbfact = -2.0 * vt * (1.5 * fact2.ln() + CHARGE * arg);
    let arg1 = -eg_nom / (2.0 * BOLTZ * tnom)
        + 1.1150877 / (BOLTZ * (REFTEMP + REFTEMP));
    let pbfact1 = -2.0 * vt_nom * (1.5 * fact1.ln() + CHARGE * arg1);

    // BE junction potential & cap (bjttemp.c:261-275, tlevc=0)
    // FP eval order must match C exactly: cap = cje/denom, then *=numer, then *=area
    let pbo_be = (m.vje - pbfact1) / fact1;
    let gmaold_be = (m.vje - pbo_be) / pbo_be;
    dev.t_be_cap = m.cje
        / (1.0 + m.mje * (4e-4 * (tnom - REFTEMP) - gmaold_be));
    dev.t_be_pot = fact2 * pbo_be + pbfact;
    let gmanew_be = (dev.t_be_pot - pbo_be) / pbo_be;
    dev.t_be_cap *= 1.0 + m.mje * (4e-4 * (temp - REFTEMP) - gmanew_be);
    dev.t_be_cap *= dev.area; // bjttemp.c:275

    // BC junction potential & cap (bjttemp.c:277-294, tlevc=0)
    let pbo_bc = (m.vjc - pbfact1) / fact1;
    let gmaold_bc = (m.vjc - pbo_bc) / pbo_bc;
    dev.t_bc_cap = m.cjc
        / (1.0 + m.mjc * (4e-4 * (tnom - REFTEMP) - gmaold_bc));
    dev.t_bc_pot = fact2 * pbo_bc + pbfact;
    let gmanew_bc = (dev.t_bc_pot - pbo_bc) / pbo_bc;
    dev.t_bc_cap *= 1.0 + m.mjc * (4e-4 * (temp - REFTEMP) - gmanew_bc);
    dev.t_bc_cap *= dev.area; // bjttemp.c:291-294 (areab for VERTICAL, area for LATERAL)

    // Substrate cap temperature correction (bjttemp.c:296-313, tlevc=0)
    // FP eval order must match C: capSub/denom, then *=numer, then *=areac
    let pbo_sub = (m.vjs - pbfact1) / fact1;
    let gmaold_sub = (m.vjs - pbo_sub) / pbo_sub;
    dev.t_sub_cap = m.cjs
        / (1.0 + m.mjs * (4e-4 * (tnom - REFTEMP) - gmaold_sub));
    dev.t_sub_pot = fact2 * pbo_sub + pbfact;
    let gmanew_sub = (dev.t_sub_pot - pbo_sub) / pbo_sub;
    dev.t_sub_cap *= 1.0 + m.mjs * (4e-4 * (temp - REFTEMP) - gmanew_sub);
    dev.t_sub_cap *= dev.area; // bjttemp.c:310-313 (areac for VERTICAL, areab for LATERAL)

    dev.t_dep_cap_be = m.fc * dev.t_be_pot;
    dev.t_dep_cap_bc = m.fc * dev.t_bc_pot;

    // Polynomial coefficients for junction cap above depletion (bjttemp.c:315-333)
    let xfc = (1.0 - m.fc).ln();
    dev.tf1 = dev.t_be_pot * (1.0 - ((1.0 - m.mje) * xfc).exp()) / (1.0 - m.mje);
    dev.tf4 = m.fc * dev.t_bc_pot;
    dev.tf5 = dev.t_bc_pot * (1.0 - ((1.0 - m.mjc) * xfc).exp()) / (1.0 - m.mjc);
    dev.tf2 = ((1.0 + m.mje) * xfc).exp();
    dev.tf3 = 1.0 - m.fc * (1.0 + m.mje);
    dev.tf6 = ((1.0 + m.mjc) * xfc).exp();
    dev.tf7 = 1.0 - m.fc * (1.0 + m.mjc);

    // Critical voltages
    dev.be_vcrit = vt * (vt / (2.0_f64.sqrt() * dev.t_is)).ln();
    dev.bc_vcrit = dev.be_vcrit;

    // Conductances
    if m.rc > 0.0 { dev.rc_cond = dev.area / m.rc; }
    if m.rb > 0.0 { dev.rb_cond = dev.area / m.rb; }
    if m.re > 0.0 { dev.re_cond = dev.area / m.re; }

    // Inverse Early/roll-off (0 if not given)
    dev.inv_vaf = if m.vaf > 0.0 { 1.0 / m.vaf } else { 0.0 };
    dev.inv_var = if m.var > 0.0 { 1.0 / m.var } else { 0.0 };
    // ngspice: tinvRollOffF = 1/(ikf * ...) / area (bjttemp.c:88-90)
    dev.inv_ikf = if m.ikf > 0.0 { 1.0 / (m.ikf * dev.area) } else { 0.0 };
    dev.inv_ikr = if m.ikr > 0.0 { 1.0 / (m.ikr * dev.area) } else { 0.0 };
}

/// PNJ limiter — wrapper around shared pnjlim in limiting.rs.
fn pnjlim(vnew: f64, vold: f64, vt: f64, vcrit: f64) -> (f64, bool) {
    let mut check = false;
    let result = crate::device::limiting::pnjlim(vnew, vold, vt, vcrit, &mut check);
    (result, check)
}

fn bjt_load(
    dev: &mut Bjt,
    mna: &mut MnaSystem,
    states: &mut StateVectors,
    mode: Mode,
    gmin: f64,
    noncon: &mut bool,
) -> Result<(), SimError> {
    let m = &dev.model;
    let tp = m.bjt_type as f64;
    let vt = dev.temp * KoverQ; // bjtload.c:148: vt = BJTtemp * KoverQ
    let so = dev.state_offset;

    let (c, b, e) = (dev.c_node, dev.b_node, dev.e_node);
    let (cp, bp, ep) = (dev.cp_node, dev.bp_node, dev.ep_node);

    // 1. Voltage recovery
    let (mut vbe, mut vbc);

    // Substrate connection node: colPrime for NPN (vertical), basePrime for PNP (lateral)
    let subst_con = if m.bjt_type == 1 { cp } else { bp };
    let mut vsub;

    if mode.is(MODEINITJCT) && mode.is(MODETRANOP) && mode.is(MODEUIC) {
        // bjtload.c:249-255: UIC — use icVBE/icVCE from .IC node voltages
        let tp = m.bjt_type as f64;
        vbe = tp * dev.ic_vbe;
        let vce = tp * dev.ic_vce;
        vbc = vbe - vce;
        vsub = 0.0;
    } else if mode.is(MODEINITJCT) && !dev.off {
        // bjtload.c:256-261: vbe = BJTtVcrit for on device (off==0)
        vbe = dev.be_vcrit;
        vbc = 0.0;
        vsub = 0.0;
    } else if mode.is(MODEINITJCT) || (mode.is(MODEINITFIX) && dev.off) {
        // bjtload.c:262-268: off device at INITJCT, or off device at INITFIX
        vbe = 0.0;
        vbc = 0.0;
        vsub = 0.0;
    } else if mode.is(MODEINITTRAN) {
        // bjtload.c:233-247: MODEINITTRAN — read vbe/vbc from state1 (already sign-adjusted),
        // read vsub from rhs_old. NO pnjlim limiting, NO xfact extrapolation.
        vbe = states.get(1, so + VBE);
        vbc = states.get(1, so + VBC);
        vsub = mna.rhs_old_val(dev.s_node) - mna.rhs_old_val(subst_con);
        dev.pre_vbe = vbe;
        dev.pre_vbc = vbc;
    } else {
        // MODEINITFIX, MODEINITFLOAT, MODEINITPRED
        // (bjtload.c:269-427)
        if mode.is(MODEINITPRED) {
            // Predictor step (bjtload.c:271-312)
            // NOTE: BJT only does predictor for MODEINITPRED, NOT MODEINITTRAN
            // (unlike MOSFET which includes both). bjtload.c:271 checks only MODEINITPRED.
            let xfact = dev.delta / dev.delta_old1;
            states.set(0, so + VBE, states.get(1, so + VBE));
            vbe = (1.0 + xfact) * states.get(1, so + VBE) - xfact * states.get(2, so + VBE);
            states.set(0, so + VBC, states.get(1, so + VBC));
            vbc = (1.0 + xfact) * states.get(1, so + VBC) - xfact * states.get(2, so + VBC);
            states.set(0, so + VSUB, states.get(1, so + VSUB));
            // Copy remaining state from state1 → state0 (bjtload.c:289-312)
            states.set(0, so + CC, states.get(1, so + CC));
            states.set(0, so + CB, states.get(1, so + CB));
            states.set(0, so + GPI, states.get(1, so + GPI));
            states.set(0, so + GMU, states.get(1, so + GMU));
            states.set(0, so + GM, states.get(1, so + GM));
            states.set(0, so + GO, states.get(1, so + GO));
            states.set(0, so + GX, states.get(1, so + GX));
        } else {
            // General iteration (bjtload.c:318-322)
            vbe = tp * (mna.rhs_old_val(bp) - mna.rhs_old_val(ep));
            vbc = tp * (mna.rhs_old_val(bp) - mna.rhs_old_val(cp));
        }
        // Compute vsub (bjtload.c:340-342): ttype=BJTtype*BJTsubs, always +1 for defaults
        vsub = mna.rhs_old_val(dev.s_node) - mna.rhs_old_val(subst_con);
        dev.pre_vbe = vbe;
        dev.pre_vbc = vbc;

        // Voltage limiting (bjtload.c:412-425) — only for general iteration, NOT MODEINITTRAN
        let old_vbe = states.get(0, so + VBE);
        let old_vbc = states.get(0, so + VBC);
        let (new_vbe, check1) = pnjlim(vbe, old_vbe, vt, dev.be_vcrit);
        vbe = new_vbe;
        let (new_vbc, check2) = pnjlim(vbc, old_vbc, vt, dev.bc_vcrit);
        vbc = new_vbc;
        // Substrate limiting (bjtload.c:418-425): vcrit=50 when subSatCur not given
        let old_vsub = states.get(0, so + VSUB);
        let (new_vsub, check3) = pnjlim(vsub, old_vsub, vt, 50.0);
        vsub = new_vsub;
        // bjtload.c:835: suppress noncon at MODEINITFIX if device is off
        if (check1 || check2 || check3) && (!mode.is(MODEINITFIX) || !dev.off) {
            *noncon = true;
        }
    }

    // 2. Junction currents (gmin = CKTgmin, passed from solver)
    let vtn_f = vt * m.nf;
    let vtn_r = vt * m.nr;

    // B-E diode (NO gmin here — gmin goes on leakage term)
    // ngspice uses BJTBEtSatCur (bjtload.c:435-441)
    let (cbe, gbe) = if vbe > -3.0 * vtn_f {
        let evbe = f64::min(MAX_EXP_ARG, vbe / vtn_f).exp();
        (dev.t_be_sat_cur * (evbe - 1.0),
         dev.t_be_sat_cur * evbe / vtn_f)
    } else {
        let arg = 3.0 * vtn_f / (vbe * std::f64::consts::E);
        let arg3 = arg * arg * arg;
        (-dev.t_be_sat_cur * (1.0 + arg3),
         dev.t_be_sat_cur * 3.0 * arg3 / vbe)
    };

    // B-E leakage (gmin added here per bjtload.c:455-456)
    let (cben, gben) = if dev.t_ise > 0.0 {
        let vtn_e = vt * m.ne;
        if vbe > -3.0 * vtn_e {
            let evbe = f64::min(MAX_EXP_ARG, vbe / vtn_e).exp();
            (dev.t_ise * (evbe - 1.0) + gmin * vbe,
             dev.t_ise * evbe / vtn_e + gmin)
        } else {
            let arg = 3.0 * vtn_e / (vbe * std::f64::consts::E);
            let arg3 = arg * arg * arg;
            (-dev.t_ise * (1.0 + arg3) + gmin * vbe,
             dev.t_ise * 3.0 * arg3 / vbe + gmin)
        }
    } else {
        // No ISE: gmin still goes here (bjtload.c:455-456 runs unconditionally)
        (gmin * vbe, gmin)
    };

    // B-C diode (NO gmin — goes on leakage)
    // ngspice uses BJTBCtSatCur (bjtload.c:465-471)
    let (cbc, gbc) = if vbc > -3.0 * vtn_r {
        let evbc = f64::min(MAX_EXP_ARG, vbc / vtn_r).exp();
        (dev.t_bc_sat_cur * (evbc - 1.0),
         dev.t_bc_sat_cur * evbc / vtn_r)
    } else {
        let arg = 3.0 * vtn_r / (vbc * std::f64::consts::E);
        let arg3 = arg * arg * arg;
        (-dev.t_bc_sat_cur * (1.0 + arg3),
         dev.t_bc_sat_cur * 3.0 * arg3 / vbc)
    };

    // B-C leakage (gmin added here per bjtload.c:485-486)
    let (cbcn, gbcn) = if dev.t_isc > 0.0 {
        let vtn_c = vt * m.nc;
        if vbc > -3.0 * vtn_c {
            let evbc = f64::min(MAX_EXP_ARG, vbc / vtn_c).exp();
            (dev.t_isc * (evbc - 1.0) + gmin * vbc,
             dev.t_isc * evbc / vtn_c + gmin)
        } else {
            let arg = 3.0 * vtn_c / (vbc * std::f64::consts::E);
            let arg3 = arg * arg * arg;
            (-dev.t_isc * (1.0 + arg3) + gmin * vbc,
             dev.t_isc * 3.0 * arg3 / vbc + gmin)
        }
    } else {
        (gmin * vbc, gmin)
    };

    // 3. Base charge (Early + high-current roll-off)
    let q1 = 1.0 / (1.0 - dev.inv_vaf * vbc - dev.inv_var * vbe);
    let q2 = dev.inv_ikf * cbe + dev.inv_ikr * cbc;
    let (qb, dqb_dvbe, dqb_dvbc);
    if dev.inv_ikf == 0.0 && dev.inv_ikr == 0.0 {
        qb = q1;
        dqb_dvbe = q1 * q1 * dev.inv_var;
        dqb_dvbc = q1 * q1 * dev.inv_vaf;
    } else {
        let arg = f64::max(0.0, 1.0 + 4.0 * q2);
        let sqarg = arg.sqrt();
        qb = q1 * (1.0 + sqarg) / 2.0;
        let dq2_dvbe = dev.inv_ikf * gbe;
        let dq2_dvbc = dev.inv_ikr * gbc;
        dqb_dvbe = q1 * (q1 * dev.inv_var * (1.0 + sqarg) / 2.0 + dq2_dvbe / sqarg);
        dqb_dvbc = q1 * (q1 * dev.inv_vaf * (1.0 + sqarg) / 2.0 + dq2_dvbc / sqarg);
    }

    // 4. Transport current (bjtload.c:595-614)
    // cex = cbe (NOT cbe/qb — the /qb happens in the go/gm formulas)
    let cex = cbe;
    let gex = gbe;
    let mut cc_val = (cex - cbc) / qb - cbc / dev.t_br - cbcn;
    let mut cb_val = cbe / dev.t_bf + cben + cbc / dev.t_br + cbcn;

    // 5. Conductances (bjtload.c:630-633)
    let mut gpi = gbe / dev.t_bf + gben;
    let mut gmu = gbc / dev.t_br + gbcn;
    let go = (gbc + (cex - cbc) * dqb_dvbc / qb) / qb;
    let gm_val = (gex - (cex - cbc) * dqb_dvbe / qb) / qb - go;

    // Base resistance conductance
    let gx = if m.rb > 0.0 { dev.rb_cond } else { 0.0 };

    // 6. Charge storage (bjtload.c:634-857)
    // Gated on: MODETRAN|MODEAC|MODEDCTRANCURVE or (MODETRANOP&&MODEUIC)
    let mut geqcb = 0.0;
    let mut geqbx = 0.0;
    let mut gcsub = 0.0;
    let cdsub = gmin * vsub; // bjtload.c:505: cdsub = CKTgmin*vsub when subSatCur not given
    let gdsub = gmin;       // bjtload.c:504: gdsub = CKTgmin when subSatCur not given

    if mode.is(MODETRAN) || mode.is(MODEAC)
        || mode.is(MODEDCTRANCURVE)
        || (mode.is(MODETRANOP) && mode.is(MODEUIC))
        || mode.is(MODEINITSMSIG)
    {
        let tf = m.tf;
        let tr = m.tr;
        let czbe = dev.t_be_cap;
        let pe = dev.t_be_pot;
        let xme = m.mje;
        let cdis = m.xcjc; // baseFractionBCcap
        let ctot = dev.t_bc_cap;
        let czbc = ctot * cdis;
        let czbx = ctot - czbc;
        let pc = dev.t_bc_pot;
        let xmc = m.mjc;
        let fcpe = dev.t_dep_cap_be;
        let czsub = dev.t_sub_cap;
        let ps = dev.t_sub_pot;
        let xms = m.mjs;
        let xtf = m.xtf;
        let ovtf = m.vtf;
        let xjtf = if m.itf > 0.0 { m.itf * dev.area } else { 0.0 };

        // Transit time bias effects (bjtload.c:658-678)
        // When tf != 0 && vbe > 0, ngspice modifies cbe and gbe IN-PLACE.
        // These modified values are used for the QBE charge and capbe below.
        // We shadow them here to match ngspice's behavior exactly.
        let mut cbe_q = cbe;  // cbe for charge computation
        let mut gbe_q = gbe;  // gbe for charge computation
        if tf != 0.0 && vbe > 0.0 {
            let mut argtf = 0.0;
            let mut arg2 = 0.0;
            let mut arg3 = 0.0;
            if xtf != 0.0 {
                argtf = xtf;
                if ovtf != 0.0 {
                    argtf = argtf * (vbc * ovtf).exp();
                }
                arg2 = argtf;
                if xjtf != 0.0 {
                    let temp = cbe / (cbe + xjtf);
                    argtf = argtf * temp * temp;
                    arg2 = argtf * (3.0 - temp - temp);
                }
                arg3 = cbe * argtf * ovtf;
            }
            // bjtload.c:675-677: modifies cbe, gbe in-place for charge storage
            cbe_q = cbe * (1.0 + argtf) / qb;
            gbe_q = (gbe * (1.0 + arg2) - cbe_q * dqb_dvbe) / qb;
            geqcb = tf * (arg3 - cbe_q * dqb_dvbc) / qb;
        }

        // BE charge and capacitance (bjtload.c:679-693)
        // Uses cbe_q/gbe_q (which are the modified cbe/gbe when tf!=0 && vbe>0)
        let capbe;
        if vbe < fcpe {
            let arg = 1.0 - vbe / pe;
            let sarg = (-xme * arg.ln()).exp();
            states.set(0, so + QBE, tf * cbe_q + pe * czbe * (1.0 - arg * sarg) / (1.0 - xme));
            capbe = tf * gbe_q + czbe * sarg;
        } else {
            let f1 = dev.tf1;
            let f2 = dev.tf2;
            let f3 = dev.tf3;
            let czbef2 = czbe / f2;
            states.set(0, so + QBE,
                tf * cbe_q + czbe * f1 + czbef2 * (f3 * (vbe - fcpe) + (xme / (pe + pe)) * (vbe * vbe - fcpe * fcpe)));
            capbe = tf * gbe_q + czbef2 * (f3 + xme * vbe / pe);
        }

        // BC charge and capacitance (bjtload.c:694-709)
        let fcpc = dev.tf4;
        let f1_bc = dev.tf5;
        let f2_bc = dev.tf6;
        let f3_bc = dev.tf7;
        let capbc;
        if vbc < fcpc {
            let arg = 1.0 - vbc / pc;
            let sarg = (-xmc * arg.ln()).exp();
            states.set(0, so + QBC, tr * cbc + pc * czbc * (1.0 - arg * sarg) / (1.0 - xmc));
            capbc = tr * gbc + czbc * sarg;
        } else {
            let czbcf2 = czbc / f2_bc;
            states.set(0, so + QBC,
                tr * cbc + czbc * f1_bc + czbcf2 * (f3_bc * (vbc - fcpc) + (xmc / (pc + pc)) * (vbc * vbc - fcpc * fcpc)));
            capbc = tr * gbc + czbcf2 * (f3_bc + xmc * vbc / pc);
        }

        // BX charge and capacitance (bjtload.c:710-721)
        // vbx = type * (V(base) - V(colPrime))
        let vbx_charge = tp * (mna.rhs_old_val(b) - mna.rhs_old_val(cp));
        let capsub;
        let capbx;
        if vbx_charge < fcpc {
            let arg = 1.0 - vbx_charge / pc;
            let sarg = (-xmc * arg.ln()).exp();
            states.set(0, so + QBX, pc * czbx * (1.0 - arg * sarg) / (1.0 - xmc));
            capbx = czbx * sarg;
        } else {
            let czbxf2 = czbx / f2_bc;
            states.set(0, so + QBX,
                czbx * f1_bc + czbxf2 * (f3_bc * (vbx_charge - fcpc) + (xmc / (pc + pc)) * (vbx_charge * vbx_charge - fcpc * fcpc)));
            capbx = czbxf2 * (f3_bc + xmc * vbx_charge / pc);
        }

        // Substrate charge and capacitance (bjtload.c:722-732)
        if vsub < 0.0 {
            let arg = 1.0 - vsub / ps;
            let sarg = (-xms * arg.ln()).exp();
            states.set(0, so + QSUB, ps * czsub * (1.0 - arg * sarg) / (1.0 - xms));
            capsub = czsub * sarg;
        } else {
            states.set(0, so + QSUB, vsub * czsub * (1.0 + xms * vsub / (2.0 * ps)));
            capsub = czsub * (1.0 + xms * vsub / ps);
        }

        // Transient integration (bjtload.c:746-828)
        // Guard: line 746-747: if(!(MODETRANOP)||(!(MODEUIC)))
        if !mode.is(MODETRANOP) || !mode.is(MODEUIC) {
            // Store small-signal parameters for AC analysis (bjtload.c:748-779)
            // During MODEINITSMSIG, store cap values to state0 and skip stamping.
            if mode.is(MODEINITSMSIG) {
                states.set(0, so + CQBE, capbe);
                states.set(0, so + CQBC, capbc);
                states.set(0, so + CQSUB, capsub);
                states.set(0, so + CQBX, capbx);
                states.set(0, so + CEXBC, geqcb);
                // Also save conductances to state0 (bjtload.c:858-878 happens normally,
                // but ngspice does 'continue' here skipping stamps. We store and return.)
                states.set(0, so + GPI, gpi);
                states.set(0, so + GMU, gmu);
                states.set(0, so + GM, gm_val);
                states.set(0, so + GO, go);
                states.set(0, so + GX, gx);
                return Ok(());
            }

            // Copy charges to state1 at MODEINITTRAN (bjtload.c:791-802)
            if mode.is(MODEINITTRAN) {
                states.set(1, so + QBE, states.get(0, so + QBE));
                states.set(1, so + QBC, states.get(0, so + QBC));
                states.set(1, so + QBX, states.get(0, so + QBX));
                states.set(1, so + QSUB, states.get(0, so + QSUB));
            }

            // NIintegrate for qbe → updates gpi, cb (bjtload.c:803-808)
            let (geq_be, _ceq_be) = ni_integrate(&dev.ag, states, capbe, so + QBE, dev.order);
            geqcb = geqcb * dev.ag[0]; // bjtload.c:805
            gpi += geq_be; // bjtload.c:807
            cb_val += states.get(0, so + CQBE); // bjtload.c:808

            // NIintegrate for qbc → updates gmu, cb, cc (bjtload.c:809-813)
            let (geq_bc, _ceq_bc) = ni_integrate(&dev.ag, states, capbc, so + QBC, dev.order);
            gmu += geq_bc; // bjtload.c:811
            cb_val += states.get(0, so + CQBC); // bjtload.c:812
            cc_val -= states.get(0, so + CQBC); // bjtload.c:813

            // Copy cqbe/cqbc to state1 at MODEINITTRAN (bjtload.c:820-826)
            if mode.is(MODEINITTRAN) {
                states.set(1, so + CQBE, states.get(0, so + CQBE));
                states.set(1, so + CQBC, states.get(0, so + CQBC));
            }
        }

        // Charge storage for c-s and b-x junctions (bjtload.c:845-856)
        // Outside the (!(MODETRANOP)||!(MODEUIC)) guard, but inside MODETRAN|MODEAC
        if mode.is(MODETRAN) || mode.is(MODEAC) {
            let (geq_sub, _) = ni_integrate(&dev.ag, states, capsub, so + QSUB, dev.order);
            gcsub = geq_sub;
            let (geq_bx, _) = ni_integrate(&dev.ag, states, capbx, so + QBX, dev.order);
            geqbx = geq_bx;

            if mode.is(MODEINITTRAN) {
                states.set(1, so + CQBX, states.get(0, so + CQBX));
                states.set(1, so + CQSUB, states.get(0, so + CQSUB));
            }
        }
    }

    dev.last_gpi = gpi;
    dev.last_gmu = gmu;
    dev.last_go = go;
    dev.last_gm = gm_val;
    dev.last_vbe = vbe;
    dev.last_vbc = vbc;

    // 7. Save state (bjtload.c:858-878)
    states.set(0, so + VBE, vbe);
    states.set(0, so + VBC, vbc);
    states.set(0, so + VSUB, vsub);
    states.set(0, so + CC, cc_val);
    states.set(0, so + CB, cb_val);
    states.set(0, so + GPI, gpi);
    states.set(0, so + GMU, gmu);
    states.set(0, so + GM, gm_val);
    states.set(0, so + GO, go);
    states.set(0, so + GX, gx);
    states.set(0, so + GEQCB, geqcb);
    states.set(0, so + GCSUB, gcsub);
    states.set(0, so + GEQBX, geqbx);
    states.set(0, so + GDSUB, gdsub);
    states.set(0, so + CDSUB, cdsub);

    // 8. RHS stamps (bjtload.c:890-914)
    // Substrate connection node
    let subst_con = if m.bjt_type == 1 { cp } else { bp };
    let ttype = 1.0; // BJTtype * BJTsubs, always +1 for defaults
    let geqsub = gcsub + gdsub;
    let ceqsub = ttype * (states.get(0, so + CQSUB) + cdsub - vsub * geqsub);
    let vbx = tp * (mna.rhs_old_val(b) - mna.rhs_old_val(cp));
    let ceqbx = tp * (states.get(0, so + CQBX) - vbx * geqbx);
    let ceqbe = tp * (cc_val + cb_val - vbe * (gm_val + go + gpi) + vbc * (go - geqcb));
    let ceqbc = tp * (-cc_val + vbe * (gm_val + go) - vbc * (gmu + go));

    mna.stamp_rhs(b, -ceqbx);                          // 907: baseNode += -ceqbx
    mna.stamp_rhs(cp, ceqbx + ceqbc);                  // 908-909: colPrimeNode += ceqbx+ceqbc
    mna.stamp_rhs(subst_con, ceqsub);                  // 910: substConNode += ceqsub
    mna.stamp_rhs(bp, -ceqbe - ceqbc);                 // 911-912: basePrimeNode += -ceqbe-ceqbc
    mna.stamp_rhs(ep, ceqbe);                           // 913: emitPrimeNode += ceqbe
    mna.stamp_rhs(dev.s_node, -ceqsub);                // 914: substNode += -ceqsub

    // 9. Matrix stamps — port of bjtload.c:941-968
    mna.stamp(c, c, dev.rc_cond);                          // collectorConduct
    mna.stamp(b, b, gx + geqbx);                          // gx+geqbx
    mna.stamp(e, e, dev.re_cond);                          // emitterConduct
    mna.stamp(cp, cp, gmu + go + geqbx + dev.rc_cond);    // gmu+go+geqbx+collectorConduct (CX=C)
    mna.stamp(subst_con, subst_con, geqsub);              // substCon diagonal
    mna.stamp(bp, bp, gx + gpi + gmu + geqcb);            // gx+gpi+gmu+geqcb
    mna.stamp(ep, ep, gpi + dev.re_cond + gm_val + go);   // gpi+emitCond+gm+go

    // Off-diagonal resistor terms
    mna.stamp(c, cp, -dev.rc_cond);
    mna.stamp(b, bp, -gx);
    mna.stamp(e, ep, -dev.re_cond);
    mna.stamp(cp, c, -dev.rc_cond);
    // Transistor cross-coupling (bjtload.c:953-965)
    mna.stamp(cp, bp, -gmu + gm_val);               // 953
    mna.stamp(cp, ep, -gm_val - go);                 // 954
    mna.stamp(bp, b, -gx);                           // 955
    mna.stamp(bp, cp, -gmu - geqcb);                 // 956: -gmu-geqcb
    mna.stamp(bp, ep, -gpi);                         // 957
    mna.stamp(ep, e, -dev.re_cond);                  // 958
    mna.stamp(ep, cp, -go + geqcb);                  // 959: -go+geqcb
    mna.stamp(ep, bp, -gpi - gm_val - geqcb);       // 960: -gpi-gm-geqcb
    // Substrate off-diagonal stamps (bjtload.c:961-963)
    mna.stamp(dev.s_node, dev.s_node, geqsub);      // 961: substSubst += geqsub
    mna.stamp(subst_con, dev.s_node, -geqsub);      // 962: substConSubst += -geqsub
    mna.stamp(dev.s_node, subst_con, -geqsub);      // 963: substSubstCon += -geqsub
    // External base to colPrime capacitance stamps (bjtload.c:964-965)
    mna.stamp(b, cp, -geqbx);                       // 964: baseColPrime += -geqbx
    mna.stamp(cp, b, -geqbx);                       // 965: colPrimeBase += -geqbx

    Ok(())
}
