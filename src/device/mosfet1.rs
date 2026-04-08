//! MOSFET Level 1 (Shichman-Hodges) — port of ngspice mos1/mos1load.c.
//!
//! DC-only path first (Phase 4.0). Transient capacitances added later.

use crate::device::Device;
use crate::error::SimError;
use crate::mna::MnaSystem;
use crate::mode::*;
use crate::state::StateVectors;

/// Number of state variables per MOSFET instance.
const MOS1_NUM_STATES: usize = 17;

/// MOSFET Level 1 model parameters — parsed from .MODEL NMOS/PMOS.
#[derive(Debug, Clone)]
pub struct Mos1Model {
    pub mos_type: i32,     // +1 = NMOS, -1 = PMOS
    pub vto: f64,          // Threshold voltage (V)
    pub kp: f64,           // Transconductance parameter (A/V^2)
    pub gamma: f64,        // Body effect coefficient (V^0.5)
    pub phi: f64,          // Surface potential (V)
    pub lambda: f64,       // Channel-length modulation (1/V)
    pub rd: f64,           // Drain resistance (ohm)
    pub rs: f64,           // Source resistance (ohm)
    pub cbd: f64,          // Drain-bulk capacitance (F)
    pub cbs: f64,          // Source-bulk capacitance (F)
    pub is_: f64,          // Bulk junction saturation current (A)
    pub pb: f64,           // Bulk junction potential (V)
    pub cgso: f64,         // Gate-source overlap cap (F/m)
    pub cgdo: f64,         // Gate-drain overlap cap (F/m)
    pub cgbo: f64,         // Gate-bulk overlap cap (F/m)
    pub cj: f64,           // Bottom junction cap (F/m^2)
    pub mj: f64,           // Bottom grading coefficient
    pub cjsw: f64,         // Sidewall junction cap (F/m)
    pub mjsw: f64,         // Sidewall grading coefficient
    pub tox: f64,          // Oxide thickness (m)
    pub ld: f64,           // Lateral diffusion (m)
    pub u0: f64,           // Surface mobility (cm^2/Vs)
    pub fc: f64,           // Forward cap depletion coefficient
    pub nss: f64,          // Surface state density (1/cm^2)
    pub nsub: f64,         // Substrate doping (1/cm^3)
    pub tpg: i32,          // Gate type: 1=opposite, -1=same, 0=Al
    pub rsh: f64,          // Sheet resistance (ohm/sq)
    pub js: f64,           // Saturation current density (A/m^2)
    pub tnom: f64,         // Nominal temperature (K)
    pub tnom_given: bool,
    // Derived from TOX
    pub oxide_cap_factor: f64,
    // "given" flags for auto-computation
    pub vto_given: bool,
    pub kp_given: bool,
    pub gamma_given: bool,
    pub phi_given: bool,
    pub u0_given: bool,
    pub cbd_given: bool,
    pub cbs_given: bool,
    pub cj_given: bool,
    pub cjsw_given: bool,
}

impl Default for Mos1Model {
    fn default() -> Self {
        Self {
            mos_type: 1, // NMOS
            vto: 0.0, kp: 2e-5, gamma: 0.0, phi: 0.6, lambda: 0.0,
            rd: 0.0, rs: 0.0, cbd: 0.0, cbs: 0.0, is_: 1e-14, pb: 0.8,
            cgso: 0.0, cgdo: 0.0, cgbo: 0.0,
            cj: 0.0, mj: 0.5, cjsw: 0.0, mjsw: 0.5,
            tox: 0.0, ld: 0.0, u0: 600.0, fc: 0.5,
            nss: 0.0, nsub: 0.0, tpg: 1, rsh: 0.0, js: 0.0,
            tnom: 300.15, // 27°C
            tnom_given: false,
            oxide_cap_factor: 0.0,
            vto_given: false, kp_given: false, gamma_given: false,
            phi_given: false, u0_given: false,
            cbd_given: false, cbs_given: false, cj_given: false, cjsw_given: false,
        }
    }
}

/// MOSFET Level 1 device instance.
#[derive(Debug)]
pub struct Mosfet1 {
    name: String,
    // External nodes
    d_node: usize,
    g_node: usize,
    s_node: usize,
    b_node: usize,
    // Internal nodes (may equal external if no parasitic R)
    dp_node: usize,
    sp_node: usize,
    // Model
    model: Mos1Model,
    w: f64,
    l: f64,
    m: f64, // parallel multiplier
    // Temperature-corrected parameters
    t_vto: f64,
    t_kp: f64,
    t_phi: f64,
    t_is: f64,
    t_bulk_pot: f64,
    vbi: f64, // built-in potential
    beta: f64,
    drain_conductance: f64,
    source_conductance: f64,
    source_vcrit: f64,
    drain_vcrit: f64,
    oxide_cap: f64,
    saved_von: f64,  // MOS1von: saved for next iteration's voltage limiting
    // Bulk cap temperature-corrected
    t_cbd: f64,
    t_cbs: f64,
    t_cj: f64,
    t_cjsw: f64,
    t_dep_cap: f64,
    // Zero-bias caps (from mos1temp.c:218-289)
    cbd_zero: f64,   // czbd: total drain-bulk zero-bias cap
    cbdsw_zero: f64,  // czbdsw: drain-bulk sidewall zero-bias cap
    cbs_zero: f64,   // czbs: total source-bulk zero-bias cap
    cbssw_zero: f64,  // czbssw: source-bulk sidewall zero-bias cap
    f2d: f64, f3d: f64, f4d: f64,
    f2s: f64, f3s: f64, f4s: f64,
    // Last computed values (for parity checking and NEWCONV)
    last_gm: f64,
    last_gds: f64,
    last_gbd: f64,
    last_gbs: f64,
    last_gmbs: f64,
    last_cd: f64,     // MOS1cd: mode * cdrain - cbd
    last_cbs: f64,    // MOS1cbs
    last_cbd: f64,    // MOS1cbd
    last_vgs: f64,
    last_vds: f64,
    last_vbs: f64,
    // Pre-limiting voltages (raw from rhs_old, before fetlim/pnjlim)
    pre_vgs: f64,
    pre_vds: f64,
    pre_vbs: f64,
    // State1/state2 for predictor comparison
    s1_vbs: f64, s1_vgs: f64, s1_vds: f64,
    s2_vbs: f64, s2_vgs: f64, s2_vds: f64,
    // Device initial conditions (from .IC node voltages or instance params)
    ic_vds: f64,
    ic_vgs: f64,
    ic_vbs: f64,
    ic_vds_given: bool,
    ic_vgs_given: bool,
    ic_vbs_given: bool,
    /// MOS1mode: +1 if VDS >= 0 (normal), -1 if reversed.
    /// Stored during load() for use by ac_load().
    mode_sign: i32,
    /// Bulk-drain junction capacitance — stored for AC.
    ac_capbd: f64,
    /// Bulk-source junction capacitance — stored for AC.
    ac_capbs: f64,
    // Temperature
    temp: f64,
    // State
    state_offset: usize,
    // Integration
    pub delta: f64,
    pub delta_old1: f64,
    pub ag: [f64; 7],
    pub order: usize,
}

impl Mosfet1 {
    /// Charge state offsets for LTE truncation.
    /// ngspice mos1trun.c only truncates qgs, qgd, qgb — NOT qbd, qbs.
    pub fn qcap_offsets(&self) -> [usize; 3] {
        [self.state_offset + QGS, self.state_offset + QGD, self.state_offset + QGB]
    }

    pub fn set_internal_nodes(&mut self, dp: usize, sp: usize) {
        self.dp_node = dp;
        self.sp_node = sp;
    }

    pub fn new(
        name: impl Into<String>,
        d: usize, g: usize, s: usize, b: usize,
        model: Mos1Model, w: f64, l: f64, m: f64,
    ) -> Self {
        Self {
            name: name.into(),
            d_node: d, g_node: g, s_node: s, b_node: b,
            dp_node: d, sp_node: s, // default: shorted
            model, w, l, m,
            t_vto: 0.0, t_kp: 0.0, t_phi: 0.0, t_is: 0.0,
            t_bulk_pot: 0.0, vbi: 0.0, beta: 0.0,
            drain_conductance: 0.0, source_conductance: 0.0,
            source_vcrit: 0.0, drain_vcrit: 0.0, oxide_cap: 0.0, saved_von: 0.0,
            t_cbd: 0.0, t_cbs: 0.0, t_cj: 0.0, t_cjsw: 0.0,
            t_dep_cap: 0.0,
            cbd_zero: 0.0, cbdsw_zero: 0.0, cbs_zero: 0.0, cbssw_zero: 0.0,
            f2d: 0.0, f3d: 0.0, f4d: 0.0,
            f2s: 0.0, f3s: 0.0, f4s: 0.0,
            last_gm: 0.0, last_gds: 0.0, last_gbd: 0.0, last_gbs: 0.0, last_gmbs: 0.0,
            last_cd: 0.0, last_cbs: 0.0, last_cbd: 0.0,
            last_vgs: 0.0, last_vds: 0.0, last_vbs: 0.0,
            pre_vgs: 0.0, pre_vds: 0.0, pre_vbs: 0.0,
            s1_vbs: 0.0, s1_vgs: 0.0, s1_vds: 0.0,
            s2_vbs: 0.0, s2_vgs: 0.0, s2_vds: 0.0,
            ic_vds: 0.0, ic_vgs: 0.0, ic_vbs: 0.0,
            ic_vds_given: false, ic_vgs_given: false, ic_vbs_given: false,
            mode_sign: 1,
            ac_capbd: 0.0,
            ac_capbs: 0.0,
            temp: REFTEMP,
            state_offset: 0,
            delta: 0.0,
            delta_old1: 1.0,
            ag: [0.0; 7],
            order: 1,
        }
    }
}

// State offsets
const VBD: usize = 0;
const VBS: usize = 1;
const VGS: usize = 2;
const VDS: usize = 3;
const CAPGS: usize = 4;
const QGS: usize = 5;
const CQGS: usize = 6;
const CAPGD: usize = 7;
const QGD: usize = 8;
const CQGD: usize = 9;
const CAPGB: usize = 10;
const QGB: usize = 11;
const CQGB: usize = 12;
const QBD: usize = 13;
const CQBD: usize = 14;
const QBS: usize = 15;
const CQBS: usize = 16;

// Physics constants
use crate::constants::{CHARGE, BOLTZ, KoverQ, REFTEMP};
const MAX_EXP_ARG: f64 = 709.0;

impl Device for Mosfet1 {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn name(&self) -> &str { &self.name }

    /// MOS1getic (mos1ic.c): propagate .IC node voltages to device ICs.
    /// Uses EXTERNAL nodes (dNode, gNode, sNode, bNode) not internal (dNodePrime, sNodePrime).
    fn setic(&mut self, rhs: &[f64]) {
        if !self.ic_vbs_given {
            self.ic_vbs = rhs[self.b_node] - rhs[self.s_node];
        }
        if !self.ic_vds_given {
            self.ic_vds = rhs[self.d_node] - rhs[self.s_node];
        }
        if !self.ic_vgs_given {
            self.ic_vgs = rhs[self.g_node] - rhs[self.s_node];
        }
    }

    fn setup(&mut self, states: &mut StateVectors) -> usize {
        self.state_offset = states.allocate(MOS1_NUM_STATES);
        MOS1_NUM_STATES
    }

    fn setup_matrix(&mut self, mna: &mut MnaSystem) {
        // Create internal nodes if parasitic resistances exist
        // For now, dp=d and sp=s (no parasitic R handling yet — added with RD/RS)

        // 22 TSTALLOC elements matching mos1set.c:188-213
        let (d, g, s, b) = (self.d_node, self.g_node, self.s_node, self.b_node);
        let (dp, sp) = (self.dp_node, self.sp_node);

        mna.make_element(d, d);
        mna.make_element(g, g);
        mna.make_element(s, s);
        mna.make_element(b, b);
        mna.make_element(dp, dp);
        mna.make_element(sp, sp);

        mna.make_element(d, dp);
        mna.make_element(g, b);
        mna.make_element(g, dp);
        mna.make_element(g, sp);
        mna.make_element(s, sp);
        mna.make_element(b, dp);
        mna.make_element(b, sp);
        mna.make_element(dp, sp);
        mna.make_element(dp, d);
        mna.make_element(b, g);
        mna.make_element(dp, g);
        mna.make_element(sp, g);
        mna.make_element(sp, s);
        mna.make_element(dp, b);
        mna.make_element(sp, b);
        mna.make_element(sp, dp);
    }

    fn temperature(&mut self, temp: f64, tnom: f64) {
        mos1_temp(self, temp, tnom);
    }

    fn load(
        &mut self,
        mna: &mut MnaSystem,
        states: &mut StateVectors,
        mode: crate::mode::Mode,
        _src_fact: f64,
        gmin: f64,
        noncon: &mut bool,
    ) -> Result<(), SimError> {
        mos1_load(self, mna, states, mode, gmin, noncon)
    }

    fn conductances(&self) -> Vec<(&str, f64)> {
        vec![
            ("gm", self.last_gm), ("gds", self.last_gds),
            ("gbd", self.last_gbd), ("gbs", self.last_gbs),
        ]
    }

    fn stored_currents(&self) -> Vec<(&str, f64)> {
        vec![
            ("cd", self.last_cd), ("cbs", self.last_cbs), ("cbd", self.last_cbd),
        ]
    }

    fn limited_voltages(&self) -> Vec<(&str, f64)> {
        vec![
            ("vgs", self.last_vgs), ("vds", self.last_vds), ("vbs", self.last_vbs),
            ("pre_vgs", self.pre_vgs), ("pre_vds", self.pre_vds), ("pre_vbs", self.pre_vbs),
            ("s1_vbs", self.s1_vbs), ("s1_vgs", self.s1_vgs), ("s1_vds", self.s1_vds),
            ("s2_vbs", self.s2_vbs), ("s2_vgs", self.s2_vgs), ("s2_vds", self.s2_vds),
        ]
    }

    /// Port of MOS1convTest (mos1conv.c) — per-device convergence check.
    ///
    /// ngspice defines NEWCONV (macros.h:19), so CKTconvTest calls MOS1convTest
    /// after basic node convergence passes. This checks predicted drain current
    /// and body current against stored values from the last load() call.
    fn conv_test(&self, mna: &MnaSystem, states: &StateVectors, reltol: f64, abstol: f64) -> bool {
        let tp = self.model.mos_type as f64;
        let so = self.state_offset;

        // mos1conv.c reads from CKTrhs (NEW solution, not CKTrhsOld)
        let vbs = tp * (mna.rhs_val(self.b_node) - mna.rhs_val(self.sp_node));
        let vgs = tp * (mna.rhs_val(self.g_node) - mna.rhs_val(self.sp_node));
        let vds = tp * (mna.rhs_val(self.dp_node) - mna.rhs_val(self.sp_node));
        let vbd = vbs - vds;
        let vgd = vgs - vds;
        let vgdo = states.get(0, so + VGS) - states.get(0, so + VDS);

        let delvbs = vbs - states.get(0, so + VBS);
        let delvbd = vbd - states.get(0, so + VBD);
        let delvgs = vgs - states.get(0, so + VGS);
        let delvds = vds - states.get(0, so + VDS);
        let delvgd = vgd - vgdo;

        // mos1conv.c:57-71
        let cdhat = if self.mode_sign >= 0 {
            self.last_cd
                - self.last_gbd * delvbd
                + self.last_gmbs * delvbs
                + self.last_gm * delvgs
                + self.last_gds * delvds
        } else {
            self.last_cd
                - (self.last_gbd - self.last_gmbs) * delvbd
                - self.last_gm * delvgd
                + self.last_gds * delvds
        };

        // mos1conv.c:72-76
        let cbhat = self.last_cbs + self.last_cbd
            + self.last_gbd * delvbd
            + self.last_gbs * delvbs;

        // mos1conv.c:80-82 — drain current convergence
        let tol = reltol * cdhat.abs().max(self.last_cd.abs()) + abstol;
        if (cdhat - self.last_cd).abs() >= tol {
            return false;
        }

        // mos1conv.c:87-90 — body current convergence
        let cb = self.last_cbs + self.last_cbd;
        let tol = reltol * cbhat.abs().max(cb.abs()) + abstol;
        if (cbhat - cb).abs() > tol {
            return false;
        }

        true
    }

    fn model_params(&self) -> Vec<(&str, f64)> {
        let m = &self.model;
        vec![
            ("vto", m.vto), ("kp", m.kp), ("gamma", m.gamma), ("phi", m.phi),
            ("lambda", m.lambda), ("rd", m.rd), ("rs", m.rs),
            ("cgso", m.cgso), ("cgdo", m.cgdo), ("cgbo", m.cgbo),
            ("tox", m.tox), ("ld", m.ld),
        ]
    }

    /// Port of MOS1acLoad from mos1acld.c.
    fn ac_load(
        &mut self,
        mna: &mut MnaSystem,
        states: &crate::state::StateVectors,
        omega: f64,
    ) -> Result<(), SimError> {
        let (xnrm, xrev): (f64, f64) = if self.mode_sign < 0 { (0.0, 1.0) } else { (1.0, 0.0) };

        // Meyer's capacitances — port of mos1acld.c:50-71
        let eff_length = self.l - 2.0 * self.model.ld;
        let gate_source_overlap = self.model.cgso * self.m * self.w;
        let gate_drain_overlap = self.model.cgdo * self.m * self.w;
        let gate_bulk_overlap = self.model.cgbo * self.m * eff_length;

        let so = self.state_offset;
        let capgs = states.get(0, so + CAPGS) + states.get(0, so + CAPGS) + gate_source_overlap;
        let capgd = states.get(0, so + CAPGD) + states.get(0, so + CAPGD) + gate_drain_overlap;
        let capgb = states.get(0, so + CAPGB) + states.get(0, so + CAPGB) + gate_bulk_overlap;

        let xgs = capgs * omega;
        let xgd = capgd * omega;
        let xgb = capgb * omega;
        let xbd = self.ac_capbd * omega;
        let xbs = self.ac_capbs * omega;

        let g = self.g_node;
        let b = self.b_node;
        let dp = self.dp_node;
        let sp = self.sp_node;
        let d = self.d_node;
        let s = self.s_node;

        let gm = self.last_gm;
        let gds = self.last_gds;
        let gbd = self.last_gbd;
        let gbs = self.last_gbs;
        let gmbs = self.last_gmbs;

        // Imaginary stamps (mos1acld.c:77-90)
        mna.stamp_imag(g, g, xgd + xgs + xgb);
        mna.stamp_imag(b, b, xgb + xbd + xbs);
        mna.stamp_imag(dp, dp, xgd + xbd);
        mna.stamp_imag(sp, sp, xgs + xbs);
        mna.stamp_imag(g, b, -xgb);
        mna.stamp_imag(g, dp, -xgd);
        mna.stamp_imag(g, sp, -xgs);
        mna.stamp_imag(b, g, -xgb);
        mna.stamp_imag(b, dp, -xbd);
        mna.stamp_imag(b, sp, -xbs);
        mna.stamp_imag(dp, g, -xgd);
        mna.stamp_imag(dp, b, -xbd);
        mna.stamp_imag(sp, g, -xgs);
        mna.stamp_imag(sp, b, -xbs);

        // Real stamps (mos1acld.c:91-113)
        mna.stamp(d, d, self.drain_conductance);
        mna.stamp(s, s, self.source_conductance);
        mna.stamp(b, b, gbd + gbs);
        mna.stamp(dp, dp, self.drain_conductance + gds + gbd + xrev * (gm + gmbs));
        mna.stamp(sp, sp, self.source_conductance + gds + gbs + xnrm * (gm + gmbs));
        mna.stamp(d, dp, -self.drain_conductance);
        mna.stamp(s, sp, -self.source_conductance);
        mna.stamp(b, dp, -gbd);
        mna.stamp(b, sp, -gbs);
        mna.stamp(dp, d, -self.drain_conductance);
        mna.stamp(dp, g, (xnrm - xrev) * gm);
        mna.stamp(dp, b, -gbd + (xnrm - xrev) * gmbs);
        mna.stamp(dp, sp, -(gds + xnrm * (gm + gmbs)));
        mna.stamp(sp, g, -(xnrm - xrev) * gm);
        mna.stamp(sp, s, -self.source_conductance);
        mna.stamp(sp, b, -(gbs + (xnrm - xrev) * gmbs));
        mna.stamp(sp, dp, -(gds + xrev * (gm + gmbs)));

        Ok(())
    }
}

/// Temperature preprocessing — port of mos1temp.c.
fn mos1_temp(dev: &mut Mosfet1, temp: f64, global_tnom: f64) {
    dev.temp = temp;
    let m = &dev.model;
    // Use global TNOM (.OPTIONS TNOM) if model TNOM wasn't explicitly given (mos1temp.c:41-42)
    let tnom = if m.tnom_given { m.tnom } else { global_tnom };
    // ngspice: vt = MOS1temp * CONSTKoverQ (mos1temp.c:135)
    // Must use precomputed KoverQ for FP parity: temp*(BOLTZ/CHARGE) != (BOLTZ*temp)/CHARGE
    let vt = temp * KoverQ;
    let vt_nom = tnom * KoverQ; // ngspice: vtnom = MOS1tnom * CONSTKoverQ (mos1temp.c:46)
    let ratio = temp / tnom;
    let fact1 = tnom / REFTEMP;
    let fact2 = temp / REFTEMP;

    let eg_nom = 1.16 - 7.02e-4 * tnom * tnom / (tnom + 1108.0);
    let eg = 1.16 - 7.02e-4 * temp * temp / (temp + 1108.0);

    let kt1 = BOLTZ * tnom;
    let pbfact1 = -2.0 * vt_nom * (1.5 * fact1.ln() + CHARGE * (-eg_nom / (kt1 + kt1) + 1.1150877 / (BOLTZ * (REFTEMP + REFTEMP))));
    let kt = BOLTZ * temp;
    let pbfact = -2.0 * vt * (1.5 * fact2.ln() + CHARGE * (-eg / (kt + kt) + 1.1150877 / (BOLTZ * (REFTEMP + REFTEMP))));

    // Effective channel length
    let leff = dev.l - 2.0 * m.ld;
    dev.beta = m.kp * dev.w / leff;

    // Oxide capacitance
    // ngspice mos1temp.c:64 uses hardcoded 3.9 * 8.854214871e-12 / tox
    // NOT CONSTepsSiO2 from const.h (which derives eps0 from mu0*c^2).
    // The literal 8.854214871e-12 is the old CODATA eps0 value, different
    // from 1/(mu0*c^2)=8.854187817e-12. Must match ngspice exactly.
    let oxide_cap_factor = if m.tox > 0.0 { 3.9 * 8.854214871e-12 / m.tox } else { 0.0 };
    dev.oxide_cap = oxide_cap_factor * leff * dev.m * dev.w;

    // Transconductance temperature correction: KP / (T/Tnom)^1.5
    let ratio4 = ratio * ratio.sqrt();
    dev.t_kp = m.kp / ratio4;
    dev.beta = dev.t_kp * dev.w / leff;

    // Phi temperature correction
    let phio = (m.phi - pbfact1) / fact1;
    dev.t_phi = fact2 * phio + pbfact;

    // Vto temperature correction
    dev.vbi = m.vto - m.mos_type as f64 * (m.gamma * m.phi.sqrt())
        + 0.5 * (eg_nom - eg) + m.mos_type as f64 * 0.5 * (dev.t_phi - m.phi);
    dev.t_vto = dev.vbi + m.mos_type as f64 * m.gamma * dev.t_phi.sqrt();

    // Saturation current
    dev.t_is = m.is_ * ((-eg / vt + eg_nom / vt_nom).exp());

    // Bulk junction potential
    let pbo = (m.pb - pbfact1) / fact1;
    let gmaold = (m.pb - pbo) / pbo;
    let capfact = 1.0 / (1.0 + m.mj * (4e-4 * (tnom - REFTEMP) - gmaold));
    dev.t_cbd = m.cbd * capfact;
    dev.t_cbs = m.cbs * capfact;
    dev.t_cj = m.cj * capfact;

    let capfact_sw = 1.0 / (1.0 + m.mjsw * (4e-4 * (tnom - REFTEMP) - gmaold));
    dev.t_cjsw = m.cjsw * capfact_sw;

    dev.t_bulk_pot = fact2 * pbo + pbfact;
    let gmanew = (dev.t_bulk_pot - pbo) / pbo;

    let capfact2 = 1.0 + m.mj * (4e-4 * (temp - REFTEMP) - gmanew);
    dev.t_cbd *= capfact2;
    dev.t_cbs *= capfact2;
    dev.t_cj *= capfact2;
    let capfact2_sw = 1.0 + m.mjsw * (4e-4 * (temp - REFTEMP) - gmanew);
    dev.t_cjsw *= capfact2_sw;

    dev.t_dep_cap = m.fc * dev.t_bulk_pot;

    // Bulk junction zero-bias caps and forward-bias polynomial coefficients (mos1temp.c:218-289)
    // Drain side
    let czbd = if m.cbd_given { dev.t_cbd * dev.m } else if m.cj_given { dev.t_cj * dev.m * 0.0 /* drainArea default 0 */ } else { 0.0 };
    let czbdsw = if m.cjsw_given { dev.t_cjsw * 0.0 * dev.m /* drainPerimeter default 0 */ } else { 0.0 };
    dev.cbd_zero = czbd;
    dev.cbdsw_zero = czbdsw;

    if czbd != 0.0 || czbdsw != 0.0 {
        let arg = 1.0 - m.fc;
        let sarg = (-m.mj * arg.ln()).exp();
        let sargsw = (-m.mjsw * arg.ln()).exp();
        dev.f2d = czbd * (1.0 - m.fc * (1.0 + m.mj)) * sarg / arg
            + czbdsw * (1.0 - m.fc * (1.0 + m.mjsw)) * sargsw / arg;
        dev.f3d = czbd * m.mj * sarg / arg / dev.t_bulk_pot
            + czbdsw * m.mjsw * sargsw / arg / dev.t_bulk_pot;
        dev.f4d = czbd * dev.t_bulk_pot * (1.0 - arg * sarg) / (1.0 - m.mj)
            + czbdsw * dev.t_bulk_pot * (1.0 - arg * sargsw) / (1.0 - m.mjsw)
            - dev.f3d / 2.0 * (dev.t_dep_cap * dev.t_dep_cap)
            - dev.t_dep_cap * dev.f2d;
    }

    // Source side
    let czbs = if m.cbs_given { dev.t_cbs * dev.m } else if m.cj_given { dev.t_cj * dev.m * 0.0 /* sourceArea default 0 */ } else { 0.0 };
    let czbssw = if m.cjsw_given { dev.t_cjsw * 0.0 * dev.m /* sourcePerimeter default 0 */ } else { 0.0 };
    dev.cbs_zero = czbs;
    dev.cbssw_zero = czbssw;

    if czbs != 0.0 || czbssw != 0.0 {
        let arg = 1.0 - m.fc;
        let sarg = (-m.mj * arg.ln()).exp();
        let sargsw = (-m.mjsw * arg.ln()).exp();
        dev.f2s = czbs * (1.0 - m.fc * (1.0 + m.mj)) * sarg / arg
            + czbssw * (1.0 - m.fc * (1.0 + m.mjsw)) * sargsw / arg;
        dev.f3s = czbs * m.mj * sarg / arg / dev.t_bulk_pot
            + czbssw * m.mjsw * sargsw / arg / dev.t_bulk_pot;
        dev.f4s = czbs * dev.t_bulk_pot * (1.0 - arg * sarg) / (1.0 - m.mj)
            + czbssw * dev.t_bulk_pot * (1.0 - arg * sargsw) / (1.0 - m.mjsw)
            - dev.f3s / 2.0 * (dev.t_dep_cap * dev.t_dep_cap)
            - dev.t_dep_cap * dev.f2s;
    }

    // Critical voltages
    dev.source_vcrit = vt * (vt / (2.0_f64.sqrt() * dev.m * dev.t_is)).ln();
    dev.drain_vcrit = dev.source_vcrit;

    // Drain/source conductances (mos1set.c:140-165)
    if m.rd > 0.0 {
        dev.drain_conductance = dev.m / m.rd;
    } else if m.rsh > 0.0 {
        dev.drain_conductance = dev.m / (m.rsh * 1.0); // NRD default = 1
    }
    if m.rs > 0.0 {
        dev.source_conductance = dev.m / m.rs;
    } else if m.rsh > 0.0 {
        dev.source_conductance = dev.m / (m.rsh * 1.0); // NRS default = 1
    }
}

/// FET voltage limiter — faithful port of DEVfetlim (devsup.c:93-151).
fn dev_fetlim(mut vnew: f64, vold: f64, vto: f64) -> f64 {
    let vtsthi = (2.0 * (vold - vto)).abs() + 2.0;
    let vtstlo = (vold - vto).abs() + 1.0;
    let vtox = vto + 3.5;
    let delv = vnew - vold;

    if vold >= vto {
        if vold >= vtox {
            if delv <= 0.0 {
                // going off
                if vnew >= vtox {
                    if -delv > vtstlo {
                        vnew = vold - vtstlo;
                    }
                } else {
                    vnew = f64::max(vnew, vto + 2.0);
                }
            } else {
                // staying on
                if delv >= vtsthi {
                    vnew = vold + vtsthi;
                }
            }
        } else {
            // middle region
            if delv <= 0.0 {
                vnew = f64::max(vnew, vto - 0.5);
            } else {
                vnew = f64::min(vnew, vto + 4.0);
            }
        }
    } else {
        // off
        if delv <= 0.0 {
            if -delv > vtsthi {
                vnew = vold - vtsthi;
            }
        } else {
            let vtemp = vto + 0.5;
            if vnew <= vtemp {
                if delv > vtstlo {
                    vnew = vold + vtstlo;
                }
            } else {
                vnew = vtemp;
            }
        }
    }
    vnew
}

/// VDS limiter — port of DEVlimvds (devsup.c).
fn dev_limvds(vnew: f64, vold: f64) -> f64 {
    if vold >= 3.5 {
        if vnew > vold { f64::min(vnew, 3.0 * vold + 2.0) }
        else if vnew < 3.5 { f64::max(vnew, 2.0) }
        else { vnew }
    } else {
        if vnew > vold { f64::min(vnew, 4.0) }
        else { f64::max(vnew, -0.5) }
    }
}

/// P-N junction voltage limiter — port of DEVpnjlim (devsup.c).
/// P-N junction voltage limiter — wrapper around shared pnjlim in limiting.rs.
fn dev_pnjlim(vnew: f64, vold: f64, vt: f64, vcrit: f64) -> (f64, bool) {
    let mut check = false;
    let result = crate::device::limiting::pnjlim(vnew, vold, vt, vcrit, &mut check);
    (result, check)
}

/// Meyer capacitance model — port of DEVqmeyer (devsup.c:674-738).
/// Returns (capgs, capgd, capgb) — half of non-constant capacitance.
fn dev_qmeyer(
    vgs: f64, vgd: f64, _vgb: f64, von: f64, mut vdsat: f64,
    phi: f64, cox: f64,
) -> (f64, f64, f64) {
    const MAGIC_VDS: f64 = 0.025;
    let vgst = vgs - von;
    vdsat = f64::max(vdsat, MAGIC_VDS);

    if vgst <= -phi {
        (0.0, 0.0, cox / 2.0)
    } else if vgst <= -phi / 2.0 {
        (0.0, 0.0, -vgst * cox / (2.0 * phi))
    } else if vgst <= 0.0 {
        let capgb = -vgst * cox / (2.0 * phi);
        let mut capgs = vgst * cox / (1.5 * phi) + cox / 3.0;
        let vds = vgs - vgd;
        let capgd = if vds >= vdsat {
            0.0
        } else {
            let vddif = 2.0 * vdsat - vds;
            let vddif1 = vdsat - vds;
            let vddif2 = vddif * vddif;
            let cgd = capgs * (1.0 - vdsat * vdsat / vddif2);
            capgs = capgs * (1.0 - vddif1 * vddif1 / vddif2);
            cgd
        };
        (capgs, capgd, capgb)
    } else {
        let vds = vgs - vgd;
        vdsat = f64::max(vdsat, MAGIC_VDS);
        if vdsat <= vds {
            (cox / 3.0, 0.0, 0.0)
        } else {
            let vddif = 2.0 * vdsat - vds;
            let vddif1 = vdsat - vds;
            let vddif2 = vddif * vddif;
            let capgd = cox * (1.0 - vdsat * vdsat / vddif2) / 3.0;
            let capgs = cox * (1.0 - vddif1 * vddif1 / vddif2) / 3.0;
            (capgs, capgd, 0.0)
        }
    }
}

/// Main load function — port of mos1load.c.
fn mos1_load(
    dev: &mut Mosfet1,
    mna: &mut MnaSystem,
    states: &mut StateVectors,
    mode: Mode,
    gmin: f64,
    noncon: &mut bool,
) -> Result<(), SimError> {
    let m = &dev.model;
    let tp = m.mos_type as f64;
    // ngspice: vt = CONSTKoverQ * MOS1temp (mos1load.c:108)
    let vt = dev.temp * KoverQ;
    let so = dev.state_offset;

    let (d, g, s, b) = (dev.d_node, dev.g_node, dev.s_node, dev.b_node);
    let (dp, sp) = (dev.dp_node, dev.sp_node);

    // 1. Voltage recovery
    let (mut vbs, mut vgs, mut vds);

    if mode.is(MODEINITJCT) && !mode.is(MODEUIC) {
        vbs = if dev.t_is != 0.0 { -1.0 } else { 0.0 };
        vgs = tp * dev.t_vto;
        vds = 0.0;
    } else if mode.is(MODEINITJCT) && mode.is(MODEUIC) {
        // UIC: use IC values — port of mos1load.c:406-416.
        // Device ICs come from MOS1getic (propagated from .IC node voltages)
        // or from IC= on the device line. Default is 0,0,0.
        // The tVto fallback is guarded by (MODETRAN|MODEDCOP|...) || (!MODEUIC),
        // which is FALSE at MODETRANOP|MODEUIC, so vgs stays as icVGS.
        vds = tp * dev.ic_vds;
        vgs = tp * dev.ic_vgs;
        vbs = tp * dev.ic_vbs;
    } else {
        if mode.is(MODEINITPRED) || mode.is(MODEINITTRAN) {
            // Predictor step (mos1load.c:209-237)
            let xfact = dev.delta / dev.delta_old1;
            states.set(0, so + VBS, states.get(1, so + VBS));
            // Save state1/state2 for parity tracing
            dev.s1_vbs = states.get(1, so + VBS);
            dev.s1_vgs = states.get(1, so + VGS);
            dev.s1_vds = states.get(1, so + VDS);
            dev.s2_vbs = states.get(2, so + VBS);
            dev.s2_vgs = states.get(2, so + VGS);
            dev.s2_vds = states.get(2, so + VDS);
            vbs = (1.0 + xfact) * states.get(1, so + VBS) - xfact * states.get(2, so + VBS);
            states.set(0, so + VGS, states.get(1, so + VGS));
            vgs = (1.0 + xfact) * states.get(1, so + VGS) - xfact * states.get(2, so + VGS);
            states.set(0, so + VDS, states.get(1, so + VDS));
            vds = (1.0 + xfact) * states.get(1, so + VDS) - xfact * states.get(2, so + VDS);
            states.set(0, so + VBD, states.get(0, so + VBS) - states.get(0, so + VDS));
        } else {
            // General iteration (mos1load.c:226-234)
            vbs = tp * (mna.rhs_old_val(b) - mna.rhs_old_val(sp));
            vgs = tp * (mna.rhs_old_val(g) - mna.rhs_old_val(sp));
            vds = tp * (mna.rhs_old_val(dp) - mna.rhs_old_val(sp));
        }

        // Save pre-limiting voltages for comparison
        dev.pre_vgs = vgs;
        dev.pre_vds = vds;
        dev.pre_vbs = vbs;

        // Voltage limiting (mos1load.c:241-404)
        let old_vgs = states.get(0, so + VGS);
        let old_vds = states.get(0, so + VDS);
        let old_vbs = states.get(0, so + VBS);
        let old_vbd = states.get(0, so + VBD);
        let vgd = vgs - vds;
        let mut vbd = vbs - vds; // mos1load.c:241 — computed BEFORE limiting
        let vgdo = old_vgs - old_vds;

        let von = tp * dev.saved_von;

        if old_vds >= 0.0 {
            vgs = dev_fetlim(vgs, old_vgs, von);
            vds = vgs - vgd;
            vds = dev_limvds(vds, old_vds);
        } else {
            let vgd_new = dev_fetlim(vgd, vgdo, von);
            vds = vgs - vgd_new;
            vds = -dev_limvds(-vds, -old_vds);
            vgs = vgd_new + vds;
        }

        if vds >= 0.0 {
            let (new_vbs, check) = dev_pnjlim(vbs, old_vbs, vt, dev.source_vcrit);
            vbs = new_vbs;
            vbd = vbs - vds;
            if check { *noncon = true; }
        } else {
            let (new_vbd, check) = dev_pnjlim(vbd, old_vbd, vt, dev.drain_vcrit);
            vbs = new_vbd + vds;
            if check { *noncon = true; }
        }
        let _ = vbd;
    }

    let vbd = vbs - vds;
    let vgd = vgs - vds;

    // 2. Drain-source reversal
    let ds_mode: i32 = if vds >= 0.0 { 1 } else { -1 };

    // 3. Diode currents (bulk-source and bulk-drain) — mos1load.c:439-454
    let (mut gbs, mut cbs_val);
    if vbs <= -3.0 * vt {
        gbs = gmin;
        cbs_val = gmin * vbs - dev.t_is;
    } else {
        let evbs = f64::min(MAX_EXP_ARG, vbs / vt).exp();
        gbs = dev.t_is * evbs / vt + gmin;
        cbs_val = dev.t_is * (evbs - 1.0) + gmin * vbs;
    }

    let (mut gbd, mut cbd_val);
    if vbd <= -3.0 * vt {
        gbd = gmin;
        cbd_val = gmin * vbd - dev.t_is;
    } else {
        let evbd = f64::min(MAX_EXP_ARG, vbd / vt).exp();
        gbd = dev.t_is * evbd / vt + gmin;
        cbd_val = dev.t_is * (evbd - 1.0) + gmin * vbd;
    }

    // 4. Channel current (Shichman-Hodges) — mos1load.c:469-536
    let v_threshold;
    let sarg;
    let v_for_body = if ds_mode > 0 { vbs } else { vbd };

    if v_for_body <= 0.0 {
        sarg = (dev.t_phi - v_for_body).sqrt();
    } else {
        let sp = dev.t_phi.sqrt();
        sarg = f64::max(0.0, sp - v_for_body / (2.0 * sp));
    }
    // mos1load.c:485: von = (tVbi * type) + gamma * sarg
    v_threshold = dev.vbi * tp + m.gamma * sarg;
    let von = v_threshold;
    let v_gate = if ds_mode > 0 { vgs } else { vgd };
    let vgst = v_gate - von;
    let vdsat = f64::max(vgst, 0.0);

    let arg = if sarg <= 0.0 { 0.0 } else { m.gamma / (2.0 * sarg) };

    let vds_eff = vds * ds_mode as f64;
    let beta_eff = dev.beta * dev.m * (1.0 + m.lambda * vds_eff);

    let (cdrain, gm, gds, gmbs);

    if vgst <= 0.0 {
        // Cutoff
        cdrain = 0.0;
        gm = 0.0;
        gds = 0.0;
        gmbs = 0.0;
    } else if vgst <= vds_eff {
        // Saturation
        cdrain = beta_eff * vgst * vgst * 0.5;
        gm = beta_eff * vgst;
        gds = m.lambda * dev.beta * dev.m * vgst * vgst * 0.5;
        gmbs = gm * arg;
    } else {
        // Linear
        cdrain = beta_eff * vds_eff * (vgst - 0.5 * vds_eff);
        gm = beta_eff * vds_eff;
        gds = beta_eff * (vgst - vds_eff) + m.lambda * dev.beta * dev.m * vds_eff * (vgst - 0.5 * vds_eff);
        gmbs = gm * arg;
    }

    // 5. Save von/vdsat and cd — mos1load.c:543-549
    // Save for next iteration's voltage limiting (mos1load.c:543: MOS1von = type * von)
    dev.saved_von = tp * von;
    // MOS1cd = mode * cdrain - cbd (mos1load.c:549) — pre-integration junction cbd
    dev.last_cd = ds_mode as f64 * cdrain - cbd_val;

    // 6. Bulk junction depletion capacitances — mos1load.c:551-713
    // ngspice guard: (MODETRAN | MODETRANOP | MODEINITSMSIG)
    if mode.is(MODETRAN) || mode.is(MODETRANOP) || mode.is(MODEINITSMSIG) {
        // Source-bulk capacitance (mos1load.c:567-617)
        let (capbs, qbs_val);
        if dev.cbs_zero != 0.0 || dev.cbssw_zero != 0.0 {
            if vbs < dev.t_dep_cap {
                let arg = 1.0 - vbs / dev.t_bulk_pot;
                let sarg = if dev.model.mj == 0.5 { 1.0 / arg.sqrt() }
                    else { (-dev.model.mj * arg.ln()).exp() };
                let sargsw = if dev.model.mjsw == 0.5 { 1.0 / arg.sqrt() }
                    else { (-dev.model.mjsw * arg.ln()).exp() };
                qbs_val = dev.t_bulk_pot * (dev.cbs_zero * (1.0 - arg * sarg) / (1.0 - dev.model.mj)
                    + dev.cbssw_zero * (1.0 - arg * sargsw) / (1.0 - dev.model.mjsw));
                capbs = dev.cbs_zero * sarg + dev.cbssw_zero * sargsw;
            } else {
                qbs_val = dev.f4s + vbs * (dev.f2s + vbs * (dev.f3s / 2.0));
                capbs = dev.f2s + dev.f3s * vbs;
            }
        } else {
            qbs_val = 0.0;
            capbs = 0.0;
        }
        states.set(0, so + QBS, qbs_val);

        // Drain-bulk capacitance (mos1load.c:628-674)
        let (capbd, qbd_val);
        if dev.cbd_zero != 0.0 || dev.cbdsw_zero != 0.0 {
            if vbd < dev.t_dep_cap {
                let arg = 1.0 - vbd / dev.t_bulk_pot;
                let sarg = if dev.model.mj == 0.5 { 1.0 / arg.sqrt() }
                    else { (-dev.model.mj * arg.ln()).exp() };
                let sargsw = if dev.model.mjsw == 0.5 { 1.0 / arg.sqrt() }
                    else { (-dev.model.mjsw * arg.ln()).exp() };
                qbd_val = dev.t_bulk_pot * (dev.cbd_zero * (1.0 - arg * sarg) / (1.0 - dev.model.mj)
                    + dev.cbdsw_zero * (1.0 - arg * sargsw) / (1.0 - dev.model.mjsw));
                capbd = dev.cbd_zero * sarg + dev.cbdsw_zero * sargsw;
            } else {
                qbd_val = dev.f4d + vbd * (dev.f2d + vbd * (dev.f3d / 2.0));
                capbd = dev.f2d + dev.f3d * vbd;
            }
        } else {
            qbd_val = 0.0;
            capbd = 0.0;
        }
        states.set(0, so + QBD, qbd_val);

        // Integrate bulk caps in transient (mos1load.c:700-713)
        if mode.is(MODETRAN) {
            use crate::integration::ni_integrate;
            let (geq, _ceq) = ni_integrate(&dev.ag, states, capbd, so + QBD, dev.order);
            gbd += geq;
            cbd_val += states.get(0, so + CQBD);
            // mos1load.c:707: MOS1cd -= cqbd
            dev.last_cd -= states.get(0, so + CQBD);

            let (geq, _ceq) = ni_integrate(&dev.ag, states, capbs, so + QBS, dev.order);
            gbs += geq;
            cbs_val += states.get(0, so + CQBS);
        }
        // Store for AC load
        dev.ac_capbd = capbd;
        dev.ac_capbs = capbs;
    }

    // Save final junction currents/conductances for NEWCONV (after integration)
    dev.last_cbs = cbs_val;
    dev.last_cbd = cbd_val;
    dev.last_gm = gm; dev.last_gds = gds; dev.last_gbd = gbd; dev.last_gbs = gbs;
    dev.last_gmbs = gmbs;
    dev.mode_sign = ds_mode;
    dev.last_vgs = vgs; dev.last_vds = vds; dev.last_vbs = vbs;

    // 7. Save state voltages — mos1load.c:738-741
    states.set(0, so + VBS, vbs);
    states.set(0, so + VBD, vbd);
    states.set(0, so + VGS, vgs);
    states.set(0, so + VDS, vds);

    // 8. Meyer gate capacitances — mos1load.c:750-882
    let (mut gcgs, mut gcgd, mut gcgb) = (0.0, 0.0, 0.0);
    let (mut ceqgs, mut ceqgd, mut ceqgb) = (0.0, 0.0, 0.0);

    let vgb = vgs - vbs;

    // ngspice guard: (MODETRAN | MODETRANOP | MODEINITSMSIG) — mos1load.c:750
    if mode.is(MODETRAN) || mode.is(MODETRANOP) || mode.is(MODEINITSMSIG) {
        let leff = dev.l - 2.0 * m.ld;
        let gate_source_overlap = m.cgso * dev.w * dev.m;
        let gate_drain_overlap = m.cgdo * dev.w * dev.m;
        let gate_bulk_overlap = m.cgbo * leff * dev.m;

        // DEVqmeyer: computes half of non-constant capacitance (mos1load.c:765-777)
        let (mut cap_gs_half, mut cap_gd_half, mut cap_gb_half);
        if ds_mode > 0 {
            (cap_gs_half, cap_gd_half, cap_gb_half) =
                dev_qmeyer(vgs, vgd, vgb, von, vdsat, dev.t_phi, dev.oxide_cap);
        } else {
            // Reverse mode: swap gs/gd args and outputs
            let (cgd, cgs, cgb) =
                dev_qmeyer(vgd, vgs, vgb, von, vdsat, dev.t_phi, dev.oxide_cap);
            cap_gs_half = cgs;
            cap_gd_half = cgd;
            cap_gb_half = cgb;
        }
        states.set(0, so + CAPGS, cap_gs_half);
        states.set(0, so + CAPGD, cap_gd_half);
        states.set(0, so + CAPGB, cap_gb_half);

        // Total capacitance (mos1load.c:781-798)
        let vgs1 = states.get(1, so + VGS);
        let vgd1 = vgs1 - states.get(1, so + VDS);
        let vgb1 = vgs1 - states.get(1, so + VBS);

        let (capgs, capgd, capgb);
        // ngspice: (MODETRANOP | MODEINITSMSIG) — mos1load.c:777
        if mode.is(MODETRANOP) || mode.is(MODEINITSMSIG) {
            capgs = 2.0 * states.get(0, so + CAPGS) + gate_source_overlap;
            capgd = 2.0 * states.get(0, so + CAPGD) + gate_drain_overlap;
            capgb = 2.0 * states.get(0, so + CAPGB) + gate_bulk_overlap;
        } else {
            capgs = states.get(0, so + CAPGS) + states.get(1, so + CAPGS) + gate_source_overlap;
            capgd = states.get(0, so + CAPGD) + states.get(1, so + CAPGD) + gate_drain_overlap;
            capgb = states.get(0, so + CAPGB) + states.get(1, so + CAPGB) + gate_bulk_overlap;
        }

        // Charge computation (mos1load.c:816-843)
        if mode.is(MODEINITPRED) || mode.is(MODEINITTRAN) {
            // Predictor: extrapolate from history using xfact = delta/deltaOld[1]
            // (mos1load.c:211) — same xfact used for both voltage and charge prediction.
            // For MODEINITTRAN, state1==state2==state0 so xfact has no effect.
            let xfact = dev.delta / dev.delta_old1;
            states.set(0, so + QGS,
                (1.0 + xfact) * states.get(1, so + QGS) - xfact * states.get(2, so + QGS));
            states.set(0, so + QGD,
                (1.0 + xfact) * states.get(1, so + QGD) - xfact * states.get(2, so + QGD));
            states.set(0, so + QGB,
                (1.0 + xfact) * states.get(1, so + QGB) - xfact * states.get(2, so + QGB));
        } else if mode.is(MODETRAN) {
            // Forward difference
            states.set(0, so + QGS, (vgs - vgs1) * capgs + states.get(1, so + QGS));
            states.set(0, so + QGD, (vgd - vgd1) * capgd + states.get(1, so + QGD));
            states.set(0, so + QGB, (vgb - vgb1) * capgb + states.get(1, so + QGB));
        } else {
            // TRANOP: Q = V * C
            states.set(0, so + QGS, vgs * capgs);
            states.set(0, so + QGD, vgd * capgd);
            states.set(0, so + QGB, vgb * capgb);
        }

        // Integration (mos1load.c:850-882)
        if mode.is(MODEINITTRAN) || !mode.is(MODETRAN) {
            gcgs = 0.0; ceqgs = 0.0;
            gcgd = 0.0; ceqgd = 0.0;
            gcgb = 0.0; ceqgb = 0.0;
        } else {
            // Zero-cap guards (mos1load.c:863-865)
            if capgs == 0.0 { states.set(0, so + CQGS, 0.0); }
            if capgd == 0.0 { states.set(0, so + CQGD, 0.0); }
            if capgb == 0.0 { states.set(0, so + CQGB, 0.0); }

            use crate::integration::ni_integrate;
            let (g, c) = ni_integrate(&dev.ag, states, capgs, so + QGS, dev.order);
            gcgs = g; ceqgs = c;
            let (g, c) = ni_integrate(&dev.ag, states, capgd, so + QGD, dev.order);
            gcgd = g; ceqgd = c;
            let (g, c) = ni_integrate(&dev.ag, states, capgb, so + QGB, dev.order);
            gcgb = g; ceqgb = c;

            // Correction (mos1load.c:880-885)
            ceqgs = ceqgs - gcgs * vgs + dev.ag[0] * states.get(0, so + QGS);
            ceqgd = ceqgd - gcgd * vgd + dev.ag[0] * states.get(0, so + QGD);
            ceqgb = ceqgb - gcgb * vgb + dev.ag[0] * states.get(0, so + QGB);
        }
    }

    // 7. Current equivalent sources (mos1load.c:894-916)
    let ceqbs = tp * (cbs_val - gbs * vbs);
    let ceqbd = tp * (cbd_val - gbd * vbd);

    let (xnrm, xrev) = if ds_mode >= 0 { (1.0, 0.0) } else { (0.0, 1.0) };

    let cdreq = if ds_mode >= 0 {
        tp * (cdrain - gds * vds - gm * vgs - gmbs * vbs)
    } else {
        -tp * (cdrain - gds * (-vds) - gm * vgd - gmbs * vbd)
    };

    // 8. RHS stamps (mos1load.c:907-916)
    mna.stamp_rhs(g, -(tp * (ceqgs + ceqgb + ceqgd)));
    mna.stamp_rhs(b, -(ceqbs + ceqbd - tp * ceqgb));
    mna.stamp_rhs(dp, ceqbd - cdreq + tp * ceqgd);
    mna.stamp_rhs(sp, cdreq + ceqbs + tp * ceqgs);

    // 9. Matrix stamps (mos1load.c:921-948)
    let rd_cond = dev.drain_conductance;
    let rs_cond = dev.source_conductance;

    // Diagonals
    mna.stamp(d, d, rd_cond);
    mna.stamp(g, g, gcgd + gcgs + gcgb);
    mna.stamp(s, s, rs_cond);
    mna.stamp(b, b, gbd + gbs + gcgb);
    mna.stamp(dp, dp, rd_cond + gds + gbd + xrev * (gm + gmbs) + gcgd);
    mna.stamp(sp, sp, rs_cond + gds + gbs + xnrm * (gm + gmbs) + gcgs);

    // Off-diagonals
    mna.stamp(d, dp, -rd_cond);
    mna.stamp(g, b, -gcgb);
    mna.stamp(g, dp, -gcgd);
    mna.stamp(g, sp, -gcgs);
    mna.stamp(s, sp, -rs_cond);
    mna.stamp(b, g, -gcgb);
    mna.stamp(b, dp, -gbd);
    mna.stamp(b, sp, -gbs);
    mna.stamp(dp, d, -rd_cond);
    mna.stamp(dp, g, (xnrm - xrev) * gm - gcgd);
    mna.stamp(dp, b, -gbd + (xnrm - xrev) * gmbs);
    mna.stamp(dp, sp, -gds - xnrm * (gm + gmbs));
    mna.stamp(sp, g, -(xnrm - xrev) * gm - gcgs);
    mna.stamp(sp, s, -rs_cond);
    mna.stamp(sp, b, -gbs - (xnrm - xrev) * gmbs);
    mna.stamp(sp, dp, -gds - xrev * (gm + gmbs));

    Ok(())
}
