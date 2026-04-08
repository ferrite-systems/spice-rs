//! MOSFET Level 3 (semi-empirical) — port of ngspice mos3/mos3load.c.
//!
//! Adds THETA (mobility modulation), ETA (static feedback), KAPPA (saturation
//! field factor), DELTA (narrow width) compared to Level 1. Uses the same
//! Meyer cap model and body diode caps as Level 1/2.

use crate::device::Device;
use crate::error::SimError;
use crate::mna::MnaSystem;
use crate::mode::*;
use crate::state::StateVectors;

/// Number of state variables per MOSFET Level 3 instance.
/// Same layout as MOS1/MOS2: vbd, vbs, vgs, vds, capgs, qgs, cqgs, capgd, qgd, cqgd,
/// capgb, qgb, cqgb, qbd, cqbd, qbs, cqbs.
const MOS3_NUM_STATES: usize = 17;

/// EPSSIL — ngspice hardcoded constant for silicon permittivity.
const EPSSIL: f64 = 11.7 * 8.854214871e-12;

/// MOSFET Level 3 model parameters — parsed from .MODEL NMOS/PMOS LEVEL=3.
#[derive(Debug, Clone)]
pub struct Mos3Model {
    pub mos_type: i32,     // +1 = NMOS, -1 = PMOS
    pub vto: f64,          // Threshold voltage (V)
    pub kp: f64,           // Transconductance parameter (A/V^2)
    pub gamma: f64,        // Body effect coefficient (V^0.5)
    pub phi: f64,          // Surface potential (V)
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
    // Level 3 specific parameters
    pub eta: f64,          // Static feedback
    pub theta: f64,        // Mobility modulation
    pub kappa: f64,        // Saturation field factor
    pub delta: f64,        // Narrow width effect (input)
    pub narrow_factor: f64, // Computed: delta * pi/2 * EPSSIL / oxideCapFactor
    pub nfs: f64,          // Fast surface state density (1/cm^2)
    pub vmax: f64,         // Maximum drift velocity (m/s)
    pub xj: f64,           // Junction depth (m)
    pub alpha: f64,        // Alpha parameter
    pub xd: f64,           // Depletion layer width coefficient (computed)
    pub surface_mobility: f64, // UO in cm^2/Vs
    // Mask adjustment parameters
    pub length_adjust: f64, // XL — mask adjustment to length
    pub width_narrow: f64,  // WD — width narrow
    pub width_adjust: f64,  // XW — mask adjustment to width
    pub delvt0: f64,        // DELVTO — adjustment to calculated VTO
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
    pub nsub_given: bool,
    pub nfs_given: bool,
}

impl Default for Mos3Model {
    fn default() -> Self {
        Self {
            mos_type: 1, // NMOS
            vto: 0.0, kp: 2e-5, gamma: 0.0, phi: 0.6,
            rd: 0.0, rs: 0.0, cbd: 0.0, cbs: 0.0, is_: 1e-14, pb: 0.8,
            cgso: 0.0, cgdo: 0.0, cgbo: 0.0,
            cj: 0.0, mj: 0.5, cjsw: 0.0, mjsw: 0.33,
            tox: 0.0, ld: 0.0, u0: 600.0, fc: 0.5,
            nss: 0.0, nsub: 0.0, tpg: 1, rsh: 0.0, js: 0.0,
            tnom: 300.15, // 27°C
            tnom_given: false,
            oxide_cap_factor: 0.0,
            // Level 3 specific defaults (from mos3set.c)
            eta: 0.0,
            theta: 0.0,
            kappa: 0.2,
            delta: 0.0,
            narrow_factor: 0.0,
            nfs: 0.0,
            vmax: 0.0,
            xj: 0.0,
            alpha: 0.0,
            xd: 0.0,
            surface_mobility: 600.0,
            length_adjust: 0.0,
            width_narrow: 0.0,
            width_adjust: 0.0,
            delvt0: 0.0,
            vto_given: false, kp_given: false, gamma_given: false,
            phi_given: false, u0_given: false,
            cbd_given: false, cbs_given: false, cj_given: false, cjsw_given: false,
            nsub_given: false, nfs_given: false,
        }
    }
}

/// MOSFET Level 3 device instance.
#[derive(Debug)]
pub struct Mosfet3 {
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
    model: Mos3Model,
    w: f64,
    l: f64,
    m: f64, // parallel multiplier
    // Temperature-corrected parameters
    t_vto: f64,
    t_kp: f64,
    t_phi: f64,
    t_surf_mob: f64,        // temperature-corrected surface mobility
    t_is: f64,
    t_is_density: f64,      // temperature-corrected saturation current density
    t_bulk_pot: f64,
    vbi: f64, // built-in potential
    drain_conductance: f64,
    source_conductance: f64,
    source_vcrit: f64,
    drain_vcrit: f64,
    saved_von: f64,
    saved_vdsat: f64,
    // Bulk cap temperature-corrected
    t_cbd: f64,
    t_cbs: f64,
    t_cj: f64,
    t_cjsw: f64,
    t_dep_cap: f64,
    // Zero-bias caps
    cbd_zero: f64,
    cbdsw_zero: f64,
    cbs_zero: f64,
    cbssw_zero: f64,
    f2d: f64, f3d: f64, f4d: f64,
    f2s: f64, f3s: f64, f4s: f64,
    // Last computed values
    last_gm: f64,
    last_gds: f64,
    last_gbd: f64,
    last_gbs: f64,
    last_gmbs: f64,
    last_cd: f64,     // MOS3cd: equivalent drain current
    last_cbs: f64,    // MOS3cbs
    last_cbd: f64,    // MOS3cbd
    last_vgs: f64,
    last_vds: f64,
    last_vbs: f64,
    pre_vgs: f64,
    pre_vds: f64,
    pre_vbs: f64,
    s1_vbs: f64, s1_vgs: f64, s1_vds: f64,
    s2_vbs: f64, s2_vgs: f64, s2_vds: f64,
    // Device initial conditions (from .IC node voltages or instance params)
    ic_vds: f64, ic_vgs: f64, ic_vbs: f64,
    ic_vds_given: bool, ic_vgs_given: bool, ic_vbs_given: bool,
    /// MOS3mode: +1 if VDS >= 0 (normal), -1 if reversed.
    mode_sign: i32,
    /// Bulk-drain junction capacitance — stored for AC.
    ac_capbd: f64,
    /// Bulk-source junction capacitance — stored for AC.
    ac_capbs: f64,
    // Instance area/perim for drain/source junctions
    drain_area: f64,
    source_area: f64,
    drain_perim: f64,
    source_perim: f64,
    drain_squares: f64,
    source_squares: f64,
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

impl Mosfet3 {
    /// Charge state offsets for LTE truncation.
    /// Same as MOS1/MOS2: only truncate qgs, qgd, qgb — NOT qbd, qbs.
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
        model: Mos3Model, w: f64, l: f64, m: f64,
    ) -> Self {
        Self {
            name: name.into(),
            d_node: d, g_node: g, s_node: s, b_node: b,
            dp_node: d, sp_node: s,
            model, w, l, m,
            t_vto: 0.0, t_kp: 0.0, t_phi: 0.0, t_surf_mob: 0.0,
            t_is: 0.0, t_is_density: 0.0,
            t_bulk_pot: 0.0, vbi: 0.0,
            drain_conductance: 0.0, source_conductance: 0.0,
            source_vcrit: 0.0, drain_vcrit: 0.0,
            saved_von: 0.0, saved_vdsat: 0.0,
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
            drain_area: 0.0,
            source_area: 0.0,
            drain_perim: 0.0,
            source_perim: 0.0,
            drain_squares: 0.0,
            source_squares: 0.0,
            temp: REFTEMP,
            state_offset: 0,
            delta: 0.0,
            delta_old1: 1.0,
            ag: [0.0; 7],
            order: 1,
        }
    }
}

// State offsets — identical to MOS1/MOS2
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

use crate::constants::{CHARGE, BOLTZ, KoverQ, REFTEMP};
const MAX_EXP_ARG: f64 = 709.0;

impl Device for Mosfet3 {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn name(&self) -> &str { &self.name }

    fn setic(&mut self, rhs: &[f64]) {
        if !self.ic_vbs_given { self.ic_vbs = rhs[self.b_node] - rhs[self.s_node]; }
        if !self.ic_vds_given { self.ic_vds = rhs[self.d_node] - rhs[self.s_node]; }
        if !self.ic_vgs_given { self.ic_vgs = rhs[self.g_node] - rhs[self.s_node]; }
    }

    fn setup(&mut self, states: &mut StateVectors) -> usize {
        self.state_offset = states.allocate(MOS3_NUM_STATES);
        MOS3_NUM_STATES
    }

    fn setup_matrix(&mut self, mna: &mut MnaSystem) {
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
        mos3_temp(self, temp, tnom);
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
        mos3_load(self, mna, states, mode, gmin, noncon)
    }

    fn conductances(&self) -> Vec<(&str, f64)> {
        vec![
            ("gm", self.last_gm), ("gds", self.last_gds),
            ("gbd", self.last_gbd), ("gbs", self.last_gbs),
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

    /// Port of MOS3convTest (mos3conv.c) — per-device convergence check.
    /// Identical to MOS1convTest — all three MOSFET levels use the same test.
    fn conv_test(&self, mna: &MnaSystem, states: &StateVectors, reltol: f64, abstol: f64) -> bool {
        let tp = self.model.mos_type as f64;
        let so = self.state_offset;

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

        let cbhat = self.last_cbs + self.last_cbd
            + self.last_gbd * delvbd
            + self.last_gbs * delvbs;

        let tol = reltol * cdhat.abs().max(self.last_cd.abs()) + abstol;
        if (cdhat - self.last_cd).abs() >= tol {
            return false;
        }

        let cb = self.last_cbs + self.last_cbd;
        let tol = reltol * cbhat.abs().max(cb.abs()) + abstol;
        if (cbhat - cb).abs() > tol {
            return false;
        }

        true
    }

    fn stored_currents(&self) -> Vec<(&str, f64)> {
        vec![
            ("cd", self.last_cd), ("cbs", self.last_cbs), ("cbd", self.last_cbd),
        ]
    }

    fn model_params(&self) -> Vec<(&str, f64)> {
        let m = &self.model;
        vec![
            ("vto", m.vto), ("kp", m.kp), ("gamma", m.gamma), ("phi", m.phi),
            ("rd", m.rd), ("rs", m.rs),
            ("cgso", m.cgso), ("cgdo", m.cgdo), ("cgbo", m.cgbo),
            ("tox", m.tox), ("ld", m.ld),
            ("eta", m.eta), ("theta", m.theta), ("kappa", m.kappa),
            ("delta", m.delta), ("nfs", m.nfs), ("vmax", m.vmax), ("xj", m.xj),
        ]
    }

    /// Port of MOS3acLoad from mos3acld.c.
    fn ac_load(
        &mut self,
        mna: &mut MnaSystem,
        states: &crate::state::StateVectors,
        omega: f64,
    ) -> Result<(), SimError> {
        let (xnrm, xrev): (f64, f64) = if self.mode_sign < 0 { (0.0, 1.0) } else { (1.0, 0.0) };

        let eff_width = self.w - 2.0 * self.model.width_narrow + self.model.width_adjust;
        let eff_length = self.l - 2.0 * self.model.ld + self.model.length_adjust;
        let gate_source_overlap = self.model.cgso * self.m * eff_width;
        let gate_drain_overlap = self.model.cgdo * self.m * eff_width;
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

        // Imaginary stamps (mos3acld.c:84-93)
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

        // Real stamps (mos3acld.c:98-120)
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

/// Temperature preprocessing — port of mos3temp.c.
fn mos3_temp(dev: &mut Mosfet3, temp: f64, global_tnom: f64) {
    dev.temp = temp;
    let m = &dev.model;
    let tnom = if m.tnom_given { m.tnom } else { global_tnom };
    let vt = temp * KoverQ;
    let vt_nom = tnom * KoverQ;
    let ratio = temp / tnom;
    let fact1 = tnom / REFTEMP;
    let fact2 = temp / REFTEMP;

    let eg_nom = 1.16 - 7.02e-4 * tnom * tnom / (tnom + 1108.0);
    let eg = 1.16 - 7.02e-4 * temp * temp / (temp + 1108.0);

    let kt1 = BOLTZ * tnom;
    let pbfact1 = -2.0 * vt_nom * (1.5 * fact1.ln() + CHARGE * (-eg_nom / (kt1 + kt1) + 1.1150877 / (BOLTZ * (REFTEMP + REFTEMP))));
    let kt = BOLTZ * temp;
    let pbfact = -2.0 * vt * (1.5 * fact2.ln() + CHARGE * (-eg / (kt + kt) + 1.1150877 / (BOLTZ * (REFTEMP + REFTEMP))));

    // Transconductance + mobility temperature correction: mos3temp.c:214-216
    let ratio4 = ratio * ratio.sqrt();
    dev.t_kp = m.kp / ratio4;
    dev.t_surf_mob = m.surface_mobility / ratio4;

    // Phi temperature correction (mos3temp.c:217-218)
    let phio = (m.phi - pbfact1) / fact1;
    dev.t_phi = fact2 * phio + pbfact;

    // Vbi and Vto (mos3temp.c:219-226)
    // Note: mos3temp.c line 220 adds model->MOS3delvt0
    dev.vbi = m.delvt0 + m.vto - m.mos_type as f64 * (m.gamma * m.phi.sqrt())
        + 0.5 * (eg_nom - eg) + m.mos_type as f64 * 0.5 * (dev.t_phi - m.phi);
    dev.t_vto = dev.vbi + m.mos_type as f64 * m.gamma * dev.t_phi.sqrt();

    // Saturation current (mos3temp.c:227-230)
    dev.t_is = m.is_ * (-eg / vt + eg_nom / vt_nom).exp();
    dev.t_is_density = m.js * (-eg / vt + eg_nom / vt_nom).exp();

    // Bulk junction potential (mos3temp.c:231-250)
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

    // Vcrit (mos3temp.c:253-267)
    if dev.t_is_density == 0.0 || dev.drain_area == 0.0 || dev.source_area == 0.0 {
        dev.source_vcrit = vt * (vt / (2.0_f64.sqrt() * dev.m * dev.t_is)).ln();
        dev.drain_vcrit = dev.source_vcrit;
    } else {
        dev.drain_vcrit = vt * (vt / (2.0_f64.sqrt() * dev.m * dev.t_is_density * dev.drain_area)).ln();
        dev.source_vcrit = vt * (vt / (2.0_f64.sqrt() * dev.m * dev.t_is_density * dev.source_area)).ln();
    }

    // Drain/source conductances (mos3temp.c:159-196)
    if m.rd > 0.0 {
        dev.drain_conductance = dev.m / m.rd;
    } else if m.rsh > 0.0 && dev.drain_squares != 0.0 {
        dev.drain_conductance = dev.m / (m.rsh * dev.drain_squares);
    } else {
        dev.drain_conductance = 0.0;
    }
    if m.rs > 0.0 {
        dev.source_conductance = dev.m / m.rs;
    } else if m.rsh > 0.0 && dev.source_squares != 0.0 {
        dev.source_conductance = dev.m / (m.rsh * dev.source_squares);
    } else {
        dev.source_conductance = 0.0;
    }

    // Bulk junction zero-bias caps (mos3temp.c:268-339)
    // Drain side
    let czbd = if m.cbd_given {
        dev.t_cbd * dev.m
    } else if m.cj_given {
        dev.t_cj * dev.drain_area * dev.m
    } else {
        0.0
    };
    let czbdsw = if m.cjsw_given {
        dev.t_cjsw * dev.drain_perim * dev.m
    } else {
        0.0
    };
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
    let czbs = if m.cbs_given {
        dev.t_cbs * dev.m
    } else if m.cj_given {
        dev.t_cj * dev.source_area * dev.m
    } else {
        0.0
    };
    let czbssw = if m.cjsw_given {
        dev.t_cjsw * dev.source_perim * dev.m
    } else {
        0.0
    };
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
                if vnew >= vtox {
                    if -delv > vtstlo { vnew = vold - vtstlo; }
                } else {
                    vnew = f64::max(vnew, vto + 2.0);
                }
            } else {
                if delv >= vtsthi { vnew = vold + vtsthi; }
            }
        } else {
            if delv <= 0.0 {
                vnew = f64::max(vnew, vto - 0.5);
            } else {
                vnew = f64::min(vnew, vto + 4.0);
            }
        }
    } else {
        if delv <= 0.0 {
            if -delv > vtsthi { vnew = vold - vtsthi; }
        } else {
            let vtemp = vto + 0.5;
            if vnew <= vtemp {
                if delv > vtstlo { vnew = vold + vtstlo; }
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

/// P-N junction voltage limiter — wrapper around shared pnjlim in limiting.rs.
fn dev_pnjlim(vnew: f64, vold: f64, vt: f64, vcrit: f64) -> (f64, bool) {
    let mut check = false;
    let result = crate::device::limiting::pnjlim(vnew, vold, vt, vcrit, &mut check);
    (result, check)
}

/// Meyer capacitance model — port of DEVqmeyer (devsup.c:674-738).
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

/// Main load function — port of mos3load.c.
/// This is the Level 3 (semi-empirical) model with THETA mobility modulation,
/// ETA static feedback, KAPPA saturation field, and DELTA narrow width.
fn mos3_load(
    dev: &mut Mosfet3,
    mna: &mut MnaSystem,
    states: &mut StateVectors,
    mode: Mode,
    gmin: f64,
    noncon: &mut bool,
) -> Result<(), SimError> {
    let mdl = &dev.model;
    let tp = mdl.mos_type as f64;
    let vt = dev.temp * KoverQ;
    let so = dev.state_offset;

    let (d, g, s, b) = (dev.d_node, dev.g_node, dev.s_node, dev.b_node);
    let (dp, sp) = (dev.dp_node, dev.sp_node);

    // mos3load.c:130-131
    let eff_width = dev.w - 2.0 * mdl.width_narrow + mdl.width_adjust;
    let eff_length = dev.l - 2.0 * mdl.ld + mdl.length_adjust;

    // Saturation currents (mos3load.c:135-145)
    let (drain_sat_cur, source_sat_cur);
    if dev.t_is_density == 0.0 || dev.drain_area == 0.0 || dev.source_area == 0.0 {
        drain_sat_cur = dev.m * dev.t_is;
        source_sat_cur = dev.m * dev.t_is;
    } else {
        drain_sat_cur = dev.m * dev.t_is_density * dev.drain_area;
        source_sat_cur = dev.m * dev.t_is_density * dev.source_area;
    }

    // mos3load.c:146-155
    let gate_source_overlap = mdl.cgso * dev.m * eff_width;
    let gate_drain_overlap = mdl.cgdo * dev.m * eff_width;
    let gate_bulk_overlap = mdl.cgbo * dev.m * eff_length;
    let beta = dev.t_kp * dev.m * eff_width / eff_length;
    let oxide_cap = mdl.oxide_cap_factor * eff_length * dev.m * eff_width;

    // 1. Voltage recovery (mos3load.c:196-397)
    let (mut vbs, mut vgs, mut vds);
    let mut check = true;

    if mode.is(MODEINITJCT) && !mode.is(MODEUIC) {
        // mos3load.c:382-396
        vds = tp * dev.ic_vds;
        vgs = tp * dev.ic_vgs;
        vbs = tp * dev.ic_vbs;
        if vds == 0.0 && vgs == 0.0 && vbs == 0.0
            && (mode.is(MODETRAN) || mode.is(MODEDCOP) || mode.is(MODEDCTRANCURVE)
                || !mode.is(MODEUIC))
        {
            vbs = -1.0;
            vgs = tp * dev.t_vto;
            vds = 0.0;
        }
    } else if mode.is(MODEINITJCT) && mode.is(MODEUIC) {
        vds = tp * dev.ic_vds;
        vgs = tp * dev.ic_vgs;
        vbs = tp * dev.ic_vbs;
    } else {
        if mode.is(MODEINITPRED) || mode.is(MODEINITTRAN) {
            // Predictor step (mos3load.c:211-229)
            let xfact = dev.delta / dev.delta_old1;
            states.set(0, so + VBS, states.get(1, so + VBS));
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
            // General iteration (mos3load.c:235-243)
            vbs = tp * (mna.rhs_old_val(b) - mna.rhs_old_val(sp));
            vgs = tp * (mna.rhs_old_val(g) - mna.rhs_old_val(sp));
            vds = tp * (mna.rhs_old_val(dp) - mna.rhs_old_val(sp));
        }

        dev.pre_vgs = vgs;
        dev.pre_vds = vds;
        dev.pre_vbs = vbs;

        // Voltage limiting (mos3load.c:339-376)
        let old_vgs = states.get(0, so + VGS);
        let old_vds = states.get(0, so + VDS);
        let old_vbs = states.get(0, so + VBS);
        let old_vbd = states.get(0, so + VBD);
        let vgd = vgs - vds;
        let vgdo = old_vgs - old_vds;
        // mos3load.c:253: vbd computed BEFORE limiting — used in pnjlim path below
        let vbd_pre = vbs - vds;

        let von = tp * dev.saved_von;

        if old_vds >= 0.0 {
            vgs = dev_fetlim(vgs, old_vgs, von);
            vds = vgs - vgd;
            vds = dev_limvds(vds, old_vds);
            // mos3load.c:357: vgd = vgs - vds (recompute after limvds)
        } else {
            let vgd_new = dev_fetlim(vgd, vgdo, von);
            vds = vgs - vgd_new;
            vds = -dev_limvds(-vds, -old_vds);
            vgs = vgd_new + vds;
        }

        // pnjlim overwrites Check (passed by pointer in ngspice)
        // mos3load.c:367-375
        if vds >= 0.0 {
            let (new_vbs, chk) = dev_pnjlim(vbs, old_vbs, vt, dev.source_vcrit);
            vbs = new_vbs;
            check = chk;
        } else {
            // mos3load.c:372: vbd here is from line 253 (pre-limiting), NOT recomputed
            let (new_vbd, chk) = dev_pnjlim(vbd_pre, old_vbd, vt, dev.drain_vcrit);
            vbs = new_vbd + vds;
            check = chk;
        }
    }

    // Temporary trace removed

    let vbd = vbs - vds;
    let vgd = vgs - vds;
    let vgb = vgs - vbs;

    // 2. Diode currents (mos3load.c:414-433)
    let (mut gbs, mut cbs_val);
    if vbs <= -3.0 * vt {
        let arg = 3.0 * vt / (vbs * std::f64::consts::E);
        let arg = arg * arg * arg;
        cbs_val = -source_sat_cur * (1.0 + arg) + gmin * vbs;
        gbs = source_sat_cur * 3.0 * arg / vbs + gmin;
    } else {
        let evbs = f64::min(MAX_EXP_ARG, vbs / vt).exp();
        gbs = source_sat_cur * evbs / vt + gmin;
        cbs_val = source_sat_cur * (evbs - 1.0) + gmin * vbs;
    }

    let (mut gbd, mut cbd_val);
    if vbd <= -3.0 * vt {
        let arg = 3.0 * vt / (vbd * std::f64::consts::E);
        let arg = arg * arg * arg;
        cbd_val = -drain_sat_cur * (1.0 + arg) + gmin * vbd;
        gbd = drain_sat_cur * 3.0 * arg / vbd + gmin;
    } else {
        let evbd = f64::min(MAX_EXP_ARG, vbd / vt).exp();
        gbd = drain_sat_cur * evbd / vt + gmin;
        cbd_val = drain_sat_cur * (evbd - 1.0) + gmin * vbd;
    }

    // 3. Mode determination (mos3load.c:438-444)
    let ds_mode: i32 = if vds >= 0.0 { 1 } else { -1 };

    // 4. Level 3 drain current — moseq3 (mos3load.c:446-867)
    let (mut cdrain, mut gm, mut gds_val, mut gmbs);
    cdrain = 0.0; gm = 0.0; gds_val = 0.0; gmbs = 0.0;
    {
        let coeff0 = 0.0631353e0;
        let coeff1 = 0.8013292e0;
        let coeff2 = -0.01110777e0;

        let lvbs = if ds_mode == 1 { vbs } else { vbd };
        let lvds = ds_mode as f64 * vds;
        let lvgs = if ds_mode == 1 { vgs } else { vgd };

        let mut vdsat = 0.0;
        let oneoverxl = 1.0 / eff_length;
        let eta = mdl.eta * 8.15e-22 / (mdl.oxide_cap_factor
            * eff_length * eff_length * eff_length);

        // Square root term (mos3load.c:566-577)
        let (phibs, sqphbs, dsqdvb);
        if lvbs <= 0.0 {
            let p = dev.t_phi - lvbs;
            let sq = p.sqrt();
            phibs = p;
            sqphbs = sq;
            dsqdvb = -0.5 / sq;
        } else {
            let sqphis = dev.t_phi.sqrt();
            let sqphs3 = dev.t_phi * sqphis;
            let sq = sqphis / (1.0 + lvbs / (dev.t_phi + dev.t_phi));
            sqphbs = sq;
            phibs = sq * sq;
            dsqdvb = -phibs / (sqphs3 + sqphs3);
        }

        // Short channel effect factor (mos3load.c:581-600)
        let (fshort, dfsdvb);
        if mdl.xj != 0.0 && mdl.xd != 0.0 {
            let wps = mdl.xd * sqphbs;
            let oneoverxj = 1.0 / mdl.xj;
            let xjonxl = mdl.xj * oneoverxl;
            let djonxj = mdl.ld * oneoverxj;
            let wponxj = wps * oneoverxj;
            let wconxj = coeff0 + coeff1 * wponxj + coeff2 * wponxj * wponxj;
            let arga = wconxj + djonxj;
            let argc = wponxj / (1.0 + wponxj);
            let argb = (1.0 - argc * argc).sqrt();
            fshort = 1.0 - xjonxl * (arga * argb - djonxj);
            let dwpdvb = mdl.xd * dsqdvb;
            let dadvb = (coeff1 + coeff2 * (wponxj + wponxj)) * dwpdvb * oneoverxj;
            let dbdvb = -argc * argc * (1.0 - argc) * dwpdvb / (argb * wps);
            dfsdvb = -xjonxl * (dadvb * argb + arga * dbdvb);
        } else {
            fshort = 1.0;
            dfsdvb = 0.0;
        }

        // Body effect (mos3load.c:603-611)
        let gammas = mdl.gamma * fshort;
        let fbodys = 0.5 * gammas / (sqphbs + sqphbs);
        let fbody = fbodys + mdl.narrow_factor / eff_width;
        let onfbdy = 1.0 / (1.0 + fbody);
        let dfbdvb = -fbodys * dsqdvb / sqphbs + fbodys * dfsdvb / fshort;
        let qbonco = gammas * sqphbs + mdl.narrow_factor * phibs / eff_width;
        let dqbdvb = gammas * dsqdvb + mdl.gamma * dfsdvb * sqphbs
            - mdl.narrow_factor / eff_width;

        // Static feedback effect (mos3load.c:615)
        let vbix = dev.vbi * tp - eta * lvds;

        // Threshold voltage (mos3load.c:619-621)
        let vth = vbix + qbonco;
        let dvtdvd = -eta;
        let dvtdvb = dqbdvb;

        // Joint weak/strong inversion (mos3load.c:625-647)
        let mut von = vth;
        let mut xn = 0.0;
        let mut dxndvb = 0.0;
        let mut dvodvd = 0.0;
        let mut dvodvb = 0.0;

        let cutoff; // true if we skip directly to innerline1000

        if mdl.nfs != 0.0 {
            let csonco = CHARGE * mdl.nfs * 1e4 * eff_length * eff_width * dev.m / oxide_cap;
            let cdonco = qbonco / (phibs + phibs);
            xn = 1.0 + csonco + cdonco;
            von = vth + vt * xn;
            dxndvb = dqbdvb / (phibs + phibs) - qbonco * dsqdvb / (phibs * sqphbs);
            dvodvd = dvtdvd;
            dvodvb = dvtdvb + vt * dxndvb;
            cutoff = false;
        } else {
            // Cutoff region (mos3load.c:640-646)
            if lvgs <= von {
                cutoff = true;
            } else {
                cutoff = false;
            }
        }

        if !cutoff {
            // Device is on (mos3load.c:651)
            let vgsx = f64::max(lvgs, von);

            // Mobility modulation by gate voltage (mos3load.c:655-660)
            let onfg = 1.0 + mdl.theta * (vgsx - vth);
            let fgate = 1.0 / onfg;
            let us = dev.t_surf_mob * 1e-4 * fgate;
            let dfgdvg = -mdl.theta * fgate * fgate;
            let dfgdvd = -dfgdvg * dvtdvd;
            let dfgdvb = -dfgdvg * dvtdvb;

            // Saturation voltage (mos3load.c:664-679)
            vdsat = (vgsx - vth) * onfbdy;
            let (dvsdvg, dvsdvb, dvsdvd);
            if mdl.vmax <= 0.0 {
                dvsdvg = onfbdy;
                dvsdvd = -dvsdvg * dvtdvd;
                dvsdvb = -dvsdvg * dvtdvb - vdsat * dfbdvb * onfbdy;
            } else {
                let vdsc = eff_length * mdl.vmax / us;
                let onvdsc = 1.0 / vdsc;
                let arga = (vgsx - vth) * onfbdy;
                let argb = (arga * arga + vdsc * vdsc).sqrt();
                vdsat = arga + vdsc - argb;
                let dvsdga = (1.0 - arga / argb) * onfbdy;
                dvsdvg = dvsdga - (1.0 - vdsc / argb) * vdsc * dfgdvg * onfg;
                dvsdvd = -dvsdvg * dvtdvd;
                dvsdvb = -dvsdvg * dvtdvb - arga * dvsdga * dfbdvb;
            }

            // Current factors in linear region (mos3load.c:683-684)
            let vdsx = f64::min(ds_mode as f64 * vds, vdsat);

            let mut my_cdrain;
            let mut my_gm;
            let mut my_gds;
            let mut my_gmbs;

            if vdsx == 0.0 {
                // line900: special case vds = 0 (mos3load.c:857-866)
                let beta_fg = beta * fgate;
                my_cdrain = 0.0;
                my_gm = 0.0;
                my_gds = beta_fg * (vgsx - vth);
                my_gmbs = 0.0;
                if mdl.nfs != 0.0 && lvgs < von {
                    my_gds *= ((lvgs - von) / (vt * xn)).exp();
                }
            } else {
                // mos3load.c:685-686
                let cdo = vgsx - vth - 0.5 * (1.0 + fbody) * vdsx;
                let dcodvb = -dvtdvb - 0.5 * dfbdvb * vdsx;

                // Normalized drain current (mos3load.c:690-694)
                let cdnorm = cdo * vdsx;
                my_gm = vdsx;
                if (ds_mode as f64 * vds) > vdsat {
                    my_gds = -dvtdvd * vdsx;
                } else {
                    my_gds = vgsx - vth - (1.0 + fbody + dvtdvd) * vdsx;
                }
                my_gmbs = dcodvb * vdsx;

                // Drain current without velocity saturation (mos3load.c:698-703)
                let cd1 = beta * cdnorm;
                let beta_fg = beta * fgate;
                my_cdrain = beta_fg * cdnorm;
                my_gm = beta_fg * my_gm + dfgdvg * cd1;
                my_gds = beta_fg * my_gds + dfgdvd * cd1;
                my_gmbs = beta_fg * my_gmbs + dfgdvb * cd1;

                // Velocity saturation factor (mos3load.c:707-723)
                // These variables are declared at moseq3 scope in C and
                // persist into the channel length modulation section.
                let mut fdrain = 0.0;
                let mut onvdsc = 0.0;
                let mut dfddvg = 0.0;
                let mut dfddvd = 0.0;
                let mut dfddvb = 0.0;
                if mdl.vmax > 0.0 {
                    let vdsc = eff_length * mdl.vmax / us;
                    onvdsc = 1.0 / vdsc;
                    fdrain = 1.0 / (1.0 + vdsx * onvdsc);
                    let fd2 = fdrain * fdrain;
                    let arga = fd2 * vdsx * onvdsc * onfg;
                    dfddvg = -dfgdvg * arga;
                    dfddvd = if (ds_mode as f64 * vds) > vdsat {
                        -dfgdvd * arga
                    } else {
                        -dfgdvd * arga - fd2 * onvdsc
                    };
                    dfddvb = -dfgdvb * arga;

                    // Drain current (mos3load.c:718-722)
                    my_gm = fdrain * my_gm + dfddvg * my_cdrain;
                    my_gds = fdrain * my_gds + dfddvd * my_cdrain;
                    my_gmbs = fdrain * my_gmbs + dfddvb * my_cdrain;
                    my_cdrain = fdrain * my_cdrain;
                    // beta = beta_fg * fdrain; (used for gds0 path only)
                }

                // Channel length modulation (mos3load.c:727-827)
                let mut gds0 = 0.0;
                if (ds_mode as f64 * vds) <= vdsat {
                    // mos3load.c:728-743
                    if mdl.vmax > 0.0 || mdl.alpha == 0.0 {
                        // goto line700
                    } else {
                        let arga = (ds_mode as f64 * vds) / vdsat;
                        let mut delxl = (mdl.kappa * mdl.alpha * vdsat / 8.0).sqrt();
                        let mut dldvd = 4.0 * delxl * arga * arga * arga / vdsat;
                        let arga2 = arga * arga;
                        let arga4 = arga2 * arga2;
                        delxl *= arga4;
                        let ddldvg = 0.0;
                        let mut ddldvd = -dldvd;
                        let ddldvb = 0.0;

                        // Punch through (mos3load.c:799-809)
                        if delxl > 0.5 * eff_length {
                            delxl = eff_length - (eff_length * eff_length / (4.0 * delxl));
                            let arga_pt = 4.0 * (eff_length - delxl) * (eff_length - delxl)
                                / (eff_length * eff_length);
                            ddldvd = ddldvd * arga_pt;
                            dldvd = dldvd * arga_pt;
                            // ddldvg and ddldvb are 0
                        }

                        // Saturation region (mos3load.c:813-824)
                        let dlonxl = delxl * oneoverxl;
                        let xlfact = 1.0 / (1.0 - dlonxl);

                        let cd1 = my_cdrain;
                        my_cdrain = my_cdrain * xlfact;
                        let diddl = my_cdrain / (eff_length - delxl);
                        my_gm = my_gm * xlfact + diddl * ddldvg;
                        my_gmbs = my_gmbs * xlfact + diddl * ddldvb;
                        gds0 = diddl * ddldvd;
                        my_gm = my_gm + gds0 * dvsdvg;
                        my_gmbs = my_gmbs + gds0 * dvsdvb;
                        my_gds = my_gds * xlfact + diddl * dldvd + gds0 * dvsdvd;
                    }
                } else {
                    // vds > vdsat (mos3load.c:746-808)
                    if mdl.vmax > 0.0 {
                        if mdl.alpha == 0.0 {
                            // goto line700
                        } else {
                            // mos3load.c:748-782
                            // Use onvdsc and dfddvg/dfddvd/dfddvb from velocity
                            // saturation block — must match C eval order exactly.
                            let cdsat = my_cdrain;
                            let mut gdsat = cdsat * (1.0 - fdrain) * onvdsc;
                            gdsat = f64::max(1.0e-12, gdsat);
                            let gdoncd = gdsat / cdsat;
                            let gdonfd = gdsat / (1.0 - fdrain);
                            let gdonfg = gdsat * onfg;
                            let dgdvg = gdoncd * my_gm - gdonfd * dfddvg + gdonfg * dfgdvg;
                            let dgdvd = gdoncd * my_gds - gdonfd * dfddvd + gdonfg * dfgdvd;
                            let dgdvb = gdoncd * my_gmbs - gdonfd * dfddvb + gdonfg * dfgdvb;

                            let emax = mdl.kappa * cdsat * oneoverxl / gdsat;
                            let emoncd = emax / cdsat;
                            let emongd = emax / gdsat;
                            let demdvg = emoncd * my_gm - emongd * dgdvg;
                            let demdvd = emoncd * my_gds - emongd * dgdvd;
                            let demdvb = emoncd * my_gmbs - emongd * dgdvb;

                            let arga = 0.5 * emax * mdl.alpha;
                            let argc = mdl.kappa * mdl.alpha;
                            let argb = (arga * arga + argc * ((ds_mode as f64 * vds) - vdsat)).sqrt();
                            let mut delxl = argb - arga;
                            let (mut dldvd, dldem);
                            if argb != 0.0 {
                                dldvd = argc / (argb + argb);
                                dldem = 0.5 * (arga / argb - 1.0) * mdl.alpha;
                            } else {
                                dldvd = 0.0;
                                dldem = 0.0;
                            }
                            let mut ddldvg = dldem * demdvg;
                            let mut ddldvd = dldem * demdvd - dldvd;
                            let mut ddldvb = dldem * demdvb;

                            // Punch through (mos3load.c:799-809)
                            if delxl > 0.5 * eff_length {
                                delxl = eff_length - (eff_length * eff_length / (4.0 * delxl));
                                let arga_pt = 4.0 * (eff_length - delxl) * (eff_length - delxl)
                                    / (eff_length * eff_length);
                                ddldvg = ddldvg * arga_pt;
                                ddldvd = ddldvd * arga_pt;
                                ddldvb = ddldvb * arga_pt;
                                dldvd = dldvd * arga_pt;
                            }

                            let dlonxl = delxl * oneoverxl;
                            let xlfact = 1.0 / (1.0 - dlonxl);

                            my_cdrain = my_cdrain * xlfact;
                            let diddl = my_cdrain / (eff_length - delxl);
                            my_gm = my_gm * xlfact + diddl * ddldvg;
                            my_gmbs = my_gmbs * xlfact + diddl * ddldvb;
                            gds0 = diddl * ddldvd;
                            my_gm = my_gm + gds0 * dvsdvg;
                            my_gmbs = my_gmbs + gds0 * dvsdvb;
                            my_gds = my_gds * xlfact + diddl * dldvd + gds0 * dvsdvd;
                        }
                    } else {
                        // vmax <= 0 (mos3load.c:783-795 — line510)
                        let mut delxl = (mdl.kappa * mdl.alpha
                            * ((ds_mode as f64 * vds) - vdsat + (vdsat / 8.0))).sqrt();
                        let mut dldvd = 0.5 * delxl / ((ds_mode as f64 * vds) - vdsat + (vdsat / 8.0));
                        let mut ddldvd = -dldvd;
                        let mut ddldvg = 0.0;
                        let mut ddldvb = 0.0;

                        // Punch through (mos3load.c:799-809)
                        if delxl > 0.5 * eff_length {
                            delxl = eff_length - (eff_length * eff_length / (4.0 * delxl));
                            let arga_pt = 4.0 * (eff_length - delxl) * (eff_length - delxl)
                                / (eff_length * eff_length);
                            ddldvg = ddldvg * arga_pt;
                            ddldvd = ddldvd * arga_pt;
                            ddldvb = ddldvb * arga_pt;
                            dldvd = dldvd * arga_pt;
                        }

                        let dlonxl = delxl * oneoverxl;
                        let xlfact = 1.0 / (1.0 - dlonxl);

                        my_cdrain = my_cdrain * xlfact;
                        let diddl = my_cdrain / (eff_length - delxl);
                        my_gm = my_gm * xlfact + diddl * ddldvg;
                        my_gmbs = my_gmbs * xlfact + diddl * ddldvb;
                        gds0 = diddl * ddldvd;
                        my_gm = my_gm + gds0 * dvsdvg;
                        my_gmbs = my_gmbs + gds0 * dvsdvb;
                        my_gds = my_gds * xlfact + diddl * dldvd + gds0 * dvsdvd;
                    }
                }

                // line700: finish strong inversion (mos3load.c:832-849)
                if lvgs < von {
                    // Weak inversion
                    let onxn = 1.0 / xn;
                    let ondvt = onxn / vt;
                    let wfact = ((lvgs - von) * ondvt).exp();
                    my_cdrain = my_cdrain * wfact;
                    let gms = my_gm * wfact;
                    let gmw = my_cdrain * ondvt;
                    my_gm = gmw;
                    if (ds_mode as f64 * vds) > vdsat {
                        my_gm = my_gm + gds0 * dvsdvg * wfact;
                    }
                    my_gds = my_gds * wfact + (gms - gmw) * dvodvd;
                    my_gmbs = my_gmbs * wfact + (gms - gmw) * dvodvb
                        - gmw * (lvgs - von) * onxn * dxndvb;
                }
            }

            cdrain = my_cdrain;
            gm = my_gm;
            gds_val = my_gds;
            gmbs = my_gmbs;
        }

        // Save von/vdsat (mos3load.c:876-877)
        dev.saved_von = tp * von;
        dev.saved_vdsat = tp * vdsat;

        // CD = mode * cdrain - cbd (mos3load.c:882)
        dev.last_cd = ds_mode as f64 * cdrain - cbd_val;
        dev.last_gm = gm;
        dev.last_gds = gds_val;
        dev.last_gmbs = gmbs;
        dev.mode_sign = ds_mode;
    }

    // 5. Bulk junction depletion capacitances (mos3load.c:884-1012)
    if mode.is(MODETRAN) || mode.is(MODETRANOP) || mode.is(MODEINITSMSIG) {
        // Source-bulk capacitance
        let (capbs, qbs_val);
        if dev.cbs_zero != 0.0 || dev.cbssw_zero != 0.0 {
            if vbs < dev.t_dep_cap {
                let arg = 1.0 - vbs / dev.t_bulk_pot;
                let (sarg, sargsw);
                if dev.model.mj == dev.model.mjsw {
                    if dev.model.mj == 0.5 {
                        let s = 1.0 / arg.sqrt();
                        sarg = s;
                        sargsw = s;
                    } else {
                        let s = (-dev.model.mj * arg.ln()).exp();
                        sarg = s;
                        sargsw = s;
                    }
                } else {
                    sarg = if dev.model.mj == 0.5 { 1.0 / arg.sqrt() }
                        else { (-dev.model.mj * arg.ln()).exp() };
                    sargsw = if dev.model.mjsw == 0.5 { 1.0 / arg.sqrt() }
                        else { (-dev.model.mjsw * arg.ln()).exp() };
                }
                qbs_val = dev.t_bulk_pot * (dev.cbs_zero * (1.0 - arg * sarg)
                    / (1.0 - dev.model.mj)
                    + dev.cbssw_zero * (1.0 - arg * sargsw)
                    / (1.0 - dev.model.mjsw));
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

        // Drain-bulk capacitance
        let (capbd, qbd_val);
        if dev.cbd_zero != 0.0 || dev.cbdsw_zero != 0.0 {
            if vbd < dev.t_dep_cap {
                let arg = 1.0 - vbd / dev.t_bulk_pot;
                let (sarg, sargsw);
                if dev.model.mj == 0.5 && dev.model.mjsw == 0.5 {
                    let s = 1.0 / arg.sqrt();
                    sarg = s;
                    sargsw = s;
                } else {
                    sarg = if dev.model.mj == 0.5 { 1.0 / arg.sqrt() }
                        else { (-dev.model.mj * arg.ln()).exp() };
                    sargsw = if dev.model.mjsw == 0.5 { 1.0 / arg.sqrt() }
                        else { (-dev.model.mjsw * arg.ln()).exp() };
                }
                qbd_val = dev.t_bulk_pot * (dev.cbd_zero * (1.0 - arg * sarg)
                    / (1.0 - dev.model.mj)
                    + dev.cbdsw_zero * (1.0 - arg * sargsw)
                    / (1.0 - dev.model.mjsw));
                capbd = dev.cbd_zero * sarg + dev.cbdsw_zero * sargsw;
            } else {
                qbd_val = dev.f4d + vbd * (dev.f2d + vbd * dev.f3d / 2.0);
                capbd = dev.f2d + dev.f3d * vbd;
            }
        } else {
            qbd_val = 0.0;
            capbd = 0.0;
        }
        states.set(0, so + QBD, qbd_val);

        // Integrate bulk caps (mos3load.c:1027-1038)
        if mode.is(MODETRAN) {
            use crate::integration::ni_integrate;
            let (geq, _ceq) = ni_integrate(&dev.ag, states, capbd, so + QBD, dev.order);
            gbd += geq;
            cbd_val += states.get(0, so + CQBD);
            dev.last_cd -= states.get(0, so + CQBD);

            let (geq, _ceq) = ni_integrate(&dev.ag, states, capbs, so + QBS, dev.order);
            gbs += geq;
            cbs_val += states.get(0, so + CQBS);
        }
        dev.ac_capbd = capbd;
        dev.ac_capbs = capbs;
    }

    // Save final junction currents for NEWCONV (after integration modifies them)
    dev.last_cbs = cbs_val;
    dev.last_cbd = cbd_val;

    // 6. Convergence check (mos3load.c:1045-1051)
    if check {
        *noncon = true;
    }

    // 7. Save state (mos3load.c:1056-1059)
    states.set(0, so + VBS, vbs);
    states.set(0, so + VBD, vbd);
    states.set(0, so + VGS, vgs);
    states.set(0, so + VDS, vds);

    // 8. Meyer gate capacitances (mos3load.c:1065-1156)
    let (mut gcgs, mut gcgd, mut gcgb) = (0.0, 0.0, 0.0);
    let (mut ceqgs, mut ceqgd, mut ceqgb) = (0.0, 0.0, 0.0);

    if mode.is(MODETRAN) || mode.is(MODETRANOP) || mode.is(MODEINITSMSIG) {
        // Meyer uses the UNSCALED von/vdsat from moseq3, not the type-scaled saved_von/saved_vdsat.
        // In C, the outer-scope `von` variable is passed directly to DEVqmeyer.
        // saved_von = tp * von, so multiply by tp to recover the internal value.
        let von = dev.saved_von * tp;
        let vdsat = dev.saved_vdsat * tp;

        let (mut cap_gs_half, mut cap_gd_half, cap_gb_half);
        if ds_mode > 0 {
            (cap_gs_half, cap_gd_half, cap_gb_half) =
                dev_qmeyer(vgs, vgd, vgb, von, vdsat, dev.t_phi, oxide_cap);
        } else {
            let (cgd, cgs, cgb) =
                dev_qmeyer(vgd, vgs, vgb, von, vdsat, dev.t_phi, oxide_cap);
            cap_gs_half = cgs;
            cap_gd_half = cgd;
            cap_gb_half = cgb;
        }
        states.set(0, so + CAPGS, cap_gs_half);
        states.set(0, so + CAPGD, cap_gd_half);
        states.set(0, so + CAPGB, cap_gb_half);

        let vgs1 = states.get(1, so + VGS);
        let vgd1 = vgs1 - states.get(1, so + VDS);
        let vgb1 = vgs1 - states.get(1, so + VBS);

        let (capgs, capgd, capgb);
        if mode.is(MODETRANOP) || mode.is(MODEINITSMSIG) {
            capgs = 2.0 * states.get(0, so + CAPGS) + gate_source_overlap;
            capgd = 2.0 * states.get(0, so + CAPGD) + gate_drain_overlap;
            capgb = 2.0 * states.get(0, so + CAPGB) + gate_bulk_overlap;
        } else {
            capgs = states.get(0, so + CAPGS) + states.get(1, so + CAPGS) + gate_source_overlap;
            capgd = states.get(0, so + CAPGD) + states.get(1, so + CAPGD) + gate_drain_overlap;
            capgb = states.get(0, so + CAPGB) + states.get(1, so + CAPGB) + gate_bulk_overlap;
        }

        // Charge computation (mos3load.c:1128-1155)
        if mode.is(MODEINITPRED) || mode.is(MODEINITTRAN) {
            let xfact = dev.delta / dev.delta_old1;
            states.set(0, so + QGS,
                (1.0 + xfact) * states.get(1, so + QGS) - xfact * states.get(2, so + QGS));
            states.set(0, so + QGD,
                (1.0 + xfact) * states.get(1, so + QGD) - xfact * states.get(2, so + QGD));
            states.set(0, so + QGB,
                (1.0 + xfact) * states.get(1, so + QGB) - xfact * states.get(2, so + QGB));
        } else if mode.is(MODETRAN) {
            states.set(0, so + QGS, (vgs - vgs1) * capgs + states.get(1, so + QGS));
            states.set(0, so + QGD, (vgd - vgd1) * capgd + states.get(1, so + QGD));
            states.set(0, so + QGB, (vgb - vgb1) * capgb + states.get(1, so + QGB));
        } else {
            // TRANOP
            states.set(0, so + QGS, vgs * capgs);
            states.set(0, so + QGD, vgd * capgd);
            states.set(0, so + QGB, vgb * capgb);
        }

        // Integration (mos3load.c:1162-1194)
        if mode.is(MODEINITTRAN) || !mode.is(MODETRAN) {
            gcgs = 0.0; ceqgs = 0.0;
            gcgd = 0.0; ceqgd = 0.0;
            gcgb = 0.0; ceqgb = 0.0;
        } else {
            if capgs == 0.0 { states.set(0, so + CQGS, 0.0); }
            if capgd == 0.0 { states.set(0, so + CQGD, 0.0); }
            if capgb == 0.0 { states.set(0, so + CQGB, 0.0); }

            use crate::integration::ni_integrate;
            let (g_val, c_val) = ni_integrate(&dev.ag, states, capgs, so + QGS, dev.order);
            gcgs = g_val; ceqgs = c_val;
            let (g_val, c_val) = ni_integrate(&dev.ag, states, capgd, so + QGD, dev.order);
            gcgd = g_val; ceqgd = c_val;
            let (g_val, c_val) = ni_integrate(&dev.ag, states, capgb, so + QGB, dev.order);
            gcgb = g_val; ceqgb = c_val;

            ceqgs = ceqgs - gcgs * vgs + dev.ag[0] * states.get(0, so + QGS);
            ceqgd = ceqgd - gcgd * vgd + dev.ag[0] * states.get(0, so + QGD);
            ceqgb = ceqgb - gcgb * vgb + dev.ag[0] * states.get(0, so + QGB);
        }
    }

    let gm = dev.last_gm;
    let gds_val = dev.last_gds;
    let gmbs = dev.last_gmbs;
    dev.last_gbd = gbd;
    dev.last_gbs = gbs;
    dev.last_vgs = vgs; dev.last_vds = vds; dev.last_vbs = vbs;

    // 9. RHS stamps (mos3load.c:1202-1224)
    let ceqbs = tp * (cbs_val - gbs * vbs);
    let ceqbd = tp * (cbd_val - gbd * vbd);

    let (xnrm, xrev) = if ds_mode >= 0 { (1.0, 0.0) } else { (0.0, 1.0) };

    let cdreq = if ds_mode >= 0 {
        tp * (cdrain - gds_val * vds - gm * vgs - gmbs * vbs)
    } else {
        -tp * (cdrain - gds_val * (-vds) - gm * vgd - gmbs * vbd)
    };

    mna.stamp_rhs(g, -(tp * (ceqgs + ceqgb + ceqgd)));
    mna.stamp_rhs(b, -(ceqbs + ceqbd - tp * ceqgb));
    mna.stamp_rhs(dp, ceqbd - cdreq + tp * ceqgd);
    mna.stamp_rhs(sp, cdreq + ceqbs + tp * ceqgs);

    // 10. Matrix stamps (mos3load.c:1236-1263)
    let rd_cond = dev.drain_conductance;
    let rs_cond = dev.source_conductance;

    mna.stamp(d, d, rd_cond);
    mna.stamp(g, g, gcgd + gcgs + gcgb);
    mna.stamp(s, s, rs_cond);
    mna.stamp(b, b, gbd + gbs + gcgb);
    mna.stamp(dp, dp, rd_cond + gds_val + gbd + xrev * (gm + gmbs) + gcgd);
    mna.stamp(sp, sp, rs_cond + gds_val + gbs + xnrm * (gm + gmbs) + gcgs);

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
    mna.stamp(dp, sp, -gds_val - xnrm * (gm + gmbs));
    mna.stamp(sp, g, -(xnrm - xrev) * gm - gcgs);
    mna.stamp(sp, s, -rs_cond);
    mna.stamp(sp, b, -gbs - (xnrm - xrev) * gmbs);
    mna.stamp(sp, dp, -gds_val - xrev * (gm + gmbs));

    Ok(())
}
