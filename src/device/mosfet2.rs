//! MOSFET Level 2 (Grove-Frohman) ��� port of ngspice mos2/mos2load.c.
//!
//! Adds velocity saturation (VMAX), narrow channel (DELTA), short channel (ETA/XJ),
//! and improved subthreshold behavior compared to Level 1.

use crate::device::Device;
use crate::error::SimError;
use crate::mna::MnaSystem;
use crate::mode::*;
use crate::state::StateVectors;

/// Number of state variables per MOSFET Level 2 instance.
/// Same layout as MOS1: vbd, vbs, vgs, vds, capgs, qgs, cqgs, capgd, qgd, cqgd,
/// capgb, qgb, cqgb, qbd, cqbd, qbs, cqbs.
const MOS2_NUM_STATES: usize = 17;

/// EPSSIL — ngspice hardcoded constant for silicon permittivity.
const EPSSIL: f64 = 11.7 * 8.854214871e-12;

/// sig1/sig2 tables for quartic root finding (mos2load.c:19-20).
const SIG1: [f64; 4] = [1.0, -1.0, 1.0, -1.0];
const SIG2: [f64; 4] = [1.0, 1.0, -1.0, -1.0];

/// MOSFET Level 2 model parameters — parsed from .MODEL NMOS/PMOS LEVEL=2.
#[derive(Debug, Clone)]
pub struct Mos2Model {
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
    // Level 2 specific parameters
    pub nfs: f64,          // Fast surface state density (1/cm^2)
    pub delta: f64,        // Narrow channel width effect
    pub uexp: f64,         // Critical field exponent
    pub ucrit: f64,        // Critical electric field (V/cm)
    pub vmax: f64,         // Maximum drift velocity (m/s)
    pub xj: f64,           // Junction depth (m)
    pub neff: f64,         // Channel charge coefficient
    pub xd: f64,           // Depletion layer width (computed)
    pub surface_mobility: f64, // UO in cm^2/Vs
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

impl Default for Mos2Model {
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
            // Level 2 specific defaults (from mos2set.c)
            nfs: 0.0,
            delta: 0.0,
            uexp: 0.0,
            ucrit: 1.0e4, // default from mos2set.c:82
            vmax: 0.0,
            xj: 0.0,
            neff: 1.0,
            xd: 0.0,
            surface_mobility: 600.0,
            vto_given: false, kp_given: false, gamma_given: false,
            phi_given: false, u0_given: false,
            cbd_given: false, cbs_given: false, cj_given: false, cjsw_given: false,
            nsub_given: false, nfs_given: false,
        }
    }
}

/// MOSFET Level 2 device instance.
#[derive(Debug)]
pub struct Mosfet2 {
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
    model: Mos2Model,
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
    oxide_cap: f64,         // OxideCap = oxideCapFactor * L_eff * w * m
    unscaled_oxide_cap: f64, // unscaled = oxideCapFactor * L_eff * w (no m)
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
    last_cd: f64,     // MOS2cd: equivalent drain current
    last_cbs: f64,    // MOS2cbs
    last_cbd: f64,    // MOS2cbd
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
    /// MOS2mode: +1 if VDS >= 0 (normal), -1 if reversed.
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

impl Mosfet2 {
    /// Charge state offsets for LTE truncation.
    /// Same as MOS1: only truncate qgs, qgd, qgb — NOT qbd, qbs.
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
        model: Mos2Model, w: f64, l: f64, m: f64,
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
            oxide_cap: 0.0, unscaled_oxide_cap: 0.0,
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

// State offsets — identical to MOS1
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

impl Device for Mosfet2 {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn name(&self) -> &str { &self.name }

    /// MOS2getic (mos2ic.c): propagate .IC node voltages to device ICs.
    fn setic(&mut self, rhs: &[f64]) {
        if !self.ic_vbs_given { self.ic_vbs = rhs[self.b_node] - rhs[self.s_node]; }
        if !self.ic_vds_given { self.ic_vds = rhs[self.d_node] - rhs[self.s_node]; }
        if !self.ic_vgs_given { self.ic_vgs = rhs[self.g_node] - rhs[self.s_node]; }
    }

    fn setup(&mut self, states: &mut StateVectors) -> usize {
        self.state_offset = states.allocate(MOS2_NUM_STATES);
        MOS2_NUM_STATES
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
        mos2_temp(self, temp, tnom);
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
        mos2_load(self, mna, states, mode, gmin, noncon)
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

    /// Port of MOS2convTest (mos2conv.c) — per-device convergence check.
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
            ("lambda", m.lambda), ("rd", m.rd), ("rs", m.rs),
            ("cgso", m.cgso), ("cgdo", m.cgdo), ("cgbo", m.cgbo),
            ("tox", m.tox), ("ld", m.ld),
            ("nfs", m.nfs), ("delta", m.delta), ("uexp", m.uexp),
            ("ucrit", m.ucrit), ("vmax", m.vmax), ("xj", m.xj),
            ("neff", m.neff),
        ]
    }

    /// Port of MOS2acLoad from mos2acld.c.
    fn ac_load(
        &mut self,
        mna: &mut MnaSystem,
        states: &crate::state::StateVectors,
        omega: f64,
    ) -> Result<(), SimError> {
        let (xnrm, xrev): (f64, f64) = if self.mode_sign < 0 { (0.0, 1.0) } else { (1.0, 0.0) };

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

        // Imaginary stamps (mos2acld.c:75-88)
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

        // Real stamps (mos2acld.c:89-111)
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

/// Temperature preprocessing — port of mos2temp.c.
fn mos2_temp(dev: &mut Mosfet2, temp: f64, global_tnom: f64) {
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

    let leff = dev.l - 2.0 * m.ld;

    // Oxide capacitance — mos2temp.c:65-66
    let oxide_cap_factor = if m.tox > 0.0 { 3.9 * 8.854214871e-12 / m.tox } else { m.oxide_cap_factor };
    dev.unscaled_oxide_cap = oxide_cap_factor * leff * dev.w;
    dev.oxide_cap = dev.unscaled_oxide_cap * dev.m;

    // Transconductance + mobility temperature correction: mos2temp.c:214-215
    let ratio4 = ratio * ratio.sqrt();
    dev.t_kp = m.kp / ratio4;
    dev.t_surf_mob = m.surface_mobility / ratio4;

    // Phi temperature correction (mos2temp.c:216-217)
    let phio = (m.phi - pbfact1) / fact1;
    dev.t_phi = fact2 * phio + pbfact;

    // Vbi and Vto (mos2temp.c:218-224)
    dev.vbi = m.vto - m.mos_type as f64 * (m.gamma * m.phi.sqrt())
        + 0.5 * (eg_nom - eg) + m.mos_type as f64 * 0.5 * (dev.t_phi - m.phi);
    dev.t_vto = dev.vbi + m.mos_type as f64 * m.gamma * dev.t_phi.sqrt();

    // Saturation current (mos2temp.c:225-228)
    dev.t_is = m.is_ * (-eg / vt + eg_nom / vt_nom).exp();
    dev.t_is_density = m.js * (-eg / vt + eg_nom / vt_nom).exp();

    // Bulk junction potential (mos2temp.c:229-248)
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

    // Vcrit (mos2temp.c:252-266)
    if dev.t_is_density == 0.0 || dev.drain_area == 0.0 || dev.source_area == 0.0 {
        dev.source_vcrit = vt * (vt / (2.0_f64.sqrt() * dev.m * dev.t_is)).ln();
        dev.drain_vcrit = dev.source_vcrit;
    } else {
        dev.drain_vcrit = vt * (vt / (2.0_f64.sqrt() * dev.m * dev.t_is_density * dev.drain_area)).ln();
        dev.source_vcrit = vt * (vt / (2.0_f64.sqrt() * dev.m * dev.t_is_density * dev.source_area)).ln();
    }

    // Drain/source conductances (mos2temp.c:169-206)
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

    // Bulk junction zero-bias caps (mos2temp.c:267-338)
    // Drain side
    let czbd = if m.cbd_given {
        dev.t_cbd * dev.m
    } else if m.cj > 0.0 {
        dev.t_cj * dev.drain_area * dev.m
    } else {
        0.0
    };
    let czbdsw = if m.cjsw > 0.0 {
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
    } else if m.cj > 0.0 {
        dev.t_cj * dev.source_area * dev.m
    } else {
        0.0
    };
    let czbssw = if m.cjsw > 0.0 {
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

/// Main load function — port of mos2load.c.
/// This is the complex Level 2 (Grove-Frohman) model with velocity saturation,
/// narrow channel, short channel, and subthreshold effects.
fn mos2_load(
    dev: &mut Mosfet2,
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

    let eff_length = dev.l - 2.0 * mdl.ld;

    // Saturation currents (mos2load.c:128-138)
    let (drain_sat_cur, source_sat_cur);
    if dev.t_is_density == 0.0 || dev.drain_area == 0.0 || dev.source_area == 0.0 {
        drain_sat_cur = dev.m * dev.t_is;
        source_sat_cur = dev.m * dev.t_is;
    } else {
        drain_sat_cur = dev.m * dev.t_is_density * dev.drain_area;
        source_sat_cur = dev.m * dev.t_is_density * dev.source_area;
    }

    let gate_source_overlap = mdl.cgso * dev.m * dev.w;
    let gate_drain_overlap = mdl.cgdo * dev.m * dev.w;
    let gate_bulk_overlap = mdl.cgbo * dev.m * eff_length;
    let beta = dev.t_kp * dev.w * dev.m / eff_length;
    let unscaled_oxide_cap = mdl.oxide_cap_factor * eff_length * dev.w;
    let oxide_cap = unscaled_oxide_cap * dev.m;

    // 1. Voltage recovery (mos2load.c:190-381)
    let (mut vbs, mut vgs, mut vds);
    let mut check = true;

    if mode.is(MODEINITJCT) && !mode.is(MODEUIC) {
        // mos2load.c:367-381
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
            vbs = tp * (mna.rhs_old_val(b) - mna.rhs_old_val(sp));
            vgs = tp * (mna.rhs_old_val(g) - mna.rhs_old_val(sp));
            vds = tp * (mna.rhs_old_val(dp) - mna.rhs_old_val(sp));
        }

        dev.pre_vgs = vgs;
        dev.pre_vds = vds;
        dev.pre_vbs = vbs;

        // Voltage limiting (mos2load.c:338-361)
        let old_vgs = states.get(0, so + VGS);
        let old_vds = states.get(0, so + VDS);
        let old_vbs = states.get(0, so + VBS);
        let old_vbd = states.get(0, so + VBD);
        let vgd = vgs - vds;
        let vgdo = old_vgs - old_vds;
        // mos2load.c:241: vbd computed BEFORE limiting — used in pnjlim path below
        let vbd_pre = vbs - vds;

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

        // pnjlim overwrites Check (passed by pointer in ngspice)
        // mos2load.c:355-361
        if vds >= 0.0 {
            let (new_vbs, chk) = dev_pnjlim(vbs, old_vbs, vt, dev.source_vcrit);
            vbs = new_vbs;
            check = chk;
        } else {
            // mos2load.c:360: vbd here is from line 241 (pre-limiting), NOT recomputed
            let (new_vbd, chk) = dev_pnjlim(vbd_pre, old_vbd, vt, dev.drain_vcrit);
            vbs = new_vbd + vds;
            check = chk;
        }
    }

    let vbd = vbs - vds;
    let vgd = vgs - vds;
    let vgb = vgs - vbs;

    // 2. Diode currents (mos2load.c:397-412)
    let (mut gbs, mut cbs_val);
    if vbs <= -3.0 * vt {
        gbs = gmin;
        cbs_val = gmin * vbs - source_sat_cur;
    } else {
        let evbs = f64::min(MAX_EXP_ARG, vbs / vt).exp();
        gbs = source_sat_cur * evbs / vt + gmin;
        cbs_val = source_sat_cur * (evbs - 1.0) + gmin * vbs;
    }

    let (mut gbd, mut cbd_val);
    if vbd <= -3.0 * vt {
        gbd = gmin;
        cbd_val = gmin * vbd - drain_sat_cur;
    } else {
        let evbd = f64::min(MAX_EXP_ARG, vbd / vt).exp();
        gbd = drain_sat_cur * evbd / vt + gmin;
        cbd_val = drain_sat_cur * (evbd - 1.0) + gmin * vbd;
    }

    // 3. Mode determination (mos2load.c:414-420)
    let ds_mode: i32 = if vds >= 0.0 { 1 } else { -1 };

    // 4. Level 2 drain current — moseq2 (mos2load.c:421-1010)
    let (mut cdrain, mut gm, mut gds_val, mut gmbs);
    {
        let lvbs = if ds_mode == 1 { vbs } else { vbd };
        let lvds = ds_mode as f64 * vds;
        let lvgs = if ds_mode == 1 { vgs } else { vgd };
        let phi_min_vbs = dev.t_phi - lvbs;

        // Compute sarg1, dsrgdb, d2sdb2 (mos2load.c:571-582)
        let (sarg1, dsrgdb, d2sdb2);
        let mut sphi;
        let mut sphi3;
        if lvbs <= 0.0 {
            sarg1 = phi_min_vbs.sqrt();
            dsrgdb = -0.5 / sarg1;
            d2sdb2 = 0.5 * dsrgdb / phi_min_vbs;
            sphi = 0.0;
            sphi3 = 0.0;
        } else {
            sphi = dev.t_phi.sqrt();
            sphi3 = dev.t_phi * sphi;
            sarg1 = sphi / (1.0 + 0.5 * lvbs / dev.t_phi);
            let tmp = sarg1 / sphi3;
            dsrgdb = -0.5 * sarg1 * tmp;
            d2sdb2 = -dsrgdb * tmp;
        }

        // barg, dbrgdb, d2bdb2 (mos2load.c:583-594)
        let (barg, dbrgdb, d2bdb2);
        if (lvbs - lvds) <= 0.0 {
            barg = (phi_min_vbs + lvds).sqrt();
            dbrgdb = -0.5 / barg;
            d2bdb2 = 0.5 * dbrgdb / (phi_min_vbs + lvds);
        } else {
            sphi = dev.t_phi.sqrt();
            sphi3 = dev.t_phi * sphi;
            barg = sphi / (1.0 + 0.5 * (lvbs - lvds) / dev.t_phi);
            let tmp = barg / sphi3;
            dbrgdb = -0.5 * barg * tmp;
            d2bdb2 = -dbrgdb * tmp;
        }

        // Narrow channel effect (mos2load.c:600-604)
        let factor = 0.125 * mdl.delta * 2.0 * std::f64::consts::PI * EPSSIL
            / unscaled_oxide_cap * eff_length;
        let eta = 1.0 + factor;
        let vbin = dev.vbi * tp + factor * phi_min_vbs;

        // Short channel effect (mos2load.c:606-657)
        let (gamasd, mut dgddvb, mut dgdvds, mut dgddb2);
        if mdl.gamma > 0.0 || mdl.nsub > 0.0 {
            let xwd = mdl.xd * barg;
            let xws = mdl.xd * sarg1;

            let mut argss = 0.0;
            let mut argsd = 0.0;
            let mut dbargs = 0.0;
            let mut dbargd = 0.0;
            dgdvds = 0.0;
            dgddb2 = 0.0;
            let mut argxs = 0.0;
            let mut argxd = 0.0;
            let mut args = 0.0;
            let mut argd = 0.0;

            if mdl.xj > 0.0 {
                let tmp = 2.0 / mdl.xj;
                argxs = 1.0 + xws * tmp;
                argxd = 1.0 + xwd * tmp;
                args = argxs.sqrt();
                argd = argxd.sqrt();
                let tmp = 0.5 * mdl.xj / eff_length;
                argss = tmp * (args - 1.0);
                argsd = tmp * (argd - 1.0);
            }
            gamasd = mdl.gamma * (1.0 - argss - argsd);
            let dbxwd = mdl.xd * dbrgdb;
            let dbxws = mdl.xd * dsrgdb;
            if mdl.xj > 0.0 {
                let tmp = 0.5 / eff_length;
                dbargs = tmp * dbxws / args;
                dbargd = tmp * dbxwd / argd;
                let dasdb2 = -mdl.xd * (d2sdb2 + dsrgdb * dsrgdb
                    * mdl.xd / (mdl.xj * argxs))
                    / (eff_length * args);
                let daddb2 = -mdl.xd * (d2bdb2 + dbrgdb * dbrgdb
                    * mdl.xd / (mdl.xj * argxd))
                    / (eff_length * argd);
                dgddb2 = -0.5 * mdl.gamma * (dasdb2 + daddb2);
            }
            dgddvb = -mdl.gamma * (dbargs + dbargd);
            if mdl.xj > 0.0 {
                let ddxwd = -dbxwd;
                dgdvds = -mdl.gamma * 0.5 * ddxwd / (eff_length * argd);
            }
        } else {
            gamasd = mdl.gamma;
            dgddvb = 0.0;
            dgdvds = 0.0;
            dgddb2 = 0.0;
        }

        // von, vth, vdsat (mos2load.c:658-660)
        let mut von = vbin + gamasd * sarg1;
        let vth = von;
        let mut vdsat = 0.0;

        // NFS / subthreshold (mos2load.c:661-683)
        let mut xn = 0.0;
        let mut argg = 0.0;
        let vgst;
        let mut dodvds = 0.0;
        let mut dxndvd = 0.0;
        let mut dxndvb = 0.0;
        let mut gds_out;

        let goto_line1050; // flag for cutoff

        if mdl.nfs != 0.0 && oxide_cap != 0.0 {
            let cfs = CHARGE * mdl.nfs * 1e4; // cm^2/m^2
            let cdonco = -(gamasd * dsrgdb + dgddvb * sarg1) + factor;

            xn = 1.0 + cfs / unscaled_oxide_cap
                * dev.w * eff_length + cdonco;

            let tmp = vt * xn;
            von = von + tmp;
            argg = 1.0 / tmp;
            vgst = lvgs - von;
            goto_line1050 = false;
        } else {
            vgst = lvgs - von;
            if lvgs <= vbin {
                // cutoff
                gds_out = 0.0;
                cdrain = 0.0;
                gm = 0.0;
                gmbs = 0.0;
                dev.saved_von = tp * von;
                dev.saved_vdsat = tp * vdsat;
                dev.last_cd = ds_mode as f64 * cdrain - cbd_val;
                dev.last_gm = gm;
                dev.last_gds = gds_out;
                dev.last_gmbs = gmbs;
                goto_line1050 = true;

                // Jump to state saving and stamp
                // We need to continue the computation below
                // Use a block structure to handle goto

                // We'll handle this with a flag
            } else {
                goto_line1050 = false;
            }
        }

        if !goto_line1050 {
            // More useful quantities (mos2load.c:689-702)
            let sarg3 = sarg1 * sarg1 * sarg1;
            let sbiarg = dev.t_bulk_pot.sqrt();
            let mut gammad = gamasd;
            let mut dgdvbs = dgddvb;
            let body = barg * barg * barg - sarg3;
            let gdbdv = 2.0 * gammad * (barg * barg * dbrgdb - sarg1 * sarg1 * dsrgdb);
            let mut dodvbs = -factor + dgdvbs * sarg1 + gammad * dsrgdb;

            if mdl.nfs != 0.0 {
                if oxide_cap != 0.0 {
                    dxndvb = 2.0 * dgdvbs * dsrgdb + gammad * d2sdb2 + dgddb2 * sarg1;
                    dodvbs = dodvbs + vt * dxndvb;
                    dxndvd = dgdvds * dsrgdb;
                    dodvds = dgdvds * sarg1 + vt * dxndvd;
                }
            }

            // Effective mobility (mos2load.c:706-723)
            let (ufact, ueff, dudvgs, dudvds, dudvbs);
            if oxide_cap > 0.0 {
                let udenom = vgst;
                let tmp = mdl.ucrit * 100.0 /* cm/m */ * EPSSIL / mdl.oxide_cap_factor;
                if udenom > tmp {
                    ufact = (mdl.uexp * (tmp / udenom).ln()).exp();
                    ueff = mdl.surface_mobility * 1e-4 * ufact;
                    dudvgs = -ufact * mdl.uexp / udenom;
                    dudvds = 0.0;
                    dudvbs = mdl.uexp * ufact * dodvbs / vgst;
                } else {
                    ufact = 1.0;
                    ueff = mdl.surface_mobility * 1e-4;
                    dudvgs = 0.0;
                    dudvds = 0.0;
                    dudvbs = 0.0;
                }
            } else {
                ufact = 1.0;
                ueff = mdl.surface_mobility * 1e-4;
                dudvgs = 0.0;
                dudvds = 0.0;
                dudvbs = 0.0;
            }

            // Saturation voltage (mos2load.c:728-825)
            let mut vgsx = lvgs;
            gammad = gamasd / eta;
            dgdvbs = dgddvb;
            let (mut dsdvgs, mut dsdvbs);

            if mdl.nfs != 0.0 && oxide_cap != 0.0 {
                vgsx = f64::max(lvgs, von);
            }

            if gammad > 0.0 {
                let gammd2 = gammad * gammad;
                let argv = (vgsx - vbin) / eta + phi_min_vbs;
                if argv <= 0.0 {
                    vdsat = 0.0;
                    dsdvgs = 0.0;
                    dsdvbs = 0.0;
                } else {
                    let arg1 = (1.0 + 4.0 * argv / gammd2).sqrt();
                    vdsat = (vgsx - vbin) / eta + gammd2 * (1.0 - arg1) / 2.0;
                    vdsat = f64::max(vdsat, 0.0);
                    dsdvgs = (1.0 - 1.0 / arg1) / eta;
                    dsdvbs = (gammad * (1.0 - arg1) + 2.0 * argv / (gammad * arg1))
                        / eta * dgdvbs + 1.0 / arg1 + factor * dsdvgs;
                }
            } else {
                vdsat = (vgsx - vbin) / eta;
                vdsat = f64::max(vdsat, 0.0);
                dsdvgs = 1.0;
                dsdvbs = 0.0;
            }

            // Velocity saturation — Baum's theory (mos2load.c:756-825)
            if mdl.vmax > 0.0 {
                let gammd2 = gammad * gammad;
                let v1 = (vgsx - vbin) / eta + phi_min_vbs;
                let v2 = phi_min_vbs;
                let xv = mdl.vmax * eff_length / ueff;
                let a1 = gammad / 0.75;
                let b1 = -2.0 * (v1 + xv);
                let c1 = -2.0 * gammad * xv;
                let d1 = 2.0 * v1 * (v2 + xv) - v2 * v2 - 4.0 / 3.0 * gammad * sarg3;
                let a = -b1;
                let b_val = a1 * c1 - 4.0 * d1;
                let c = -d1 * (a1 * a1 - 4.0 * b1) - c1 * c1;
                let r = -a * a / 3.0 + b_val;
                let s_val = 2.0 * a * a * a / 27.0 - a * b_val / 3.0 + c;
                let r3 = r * r * r;
                let s2 = s_val * s_val;
                let p = s2 / 4.0 + r3 / 27.0;
                let p0 = p.abs();
                let p2 = p0.sqrt();

                let y3;
                if p < 0.0 {
                    let ro = (s2 / 4.0 + p0).sqrt();
                    let ro = (ro.ln() / 3.0).exp();
                    let fi = (-2.0 * p2 / s_val).atan();
                    y3 = 2.0 * ro * (fi / 3.0).cos() - a / 3.0;
                } else {
                    let p3_val = -s_val / 2.0 + p2;
                    let p3_val = (p3_val.abs().ln() / 3.0).exp();
                    let p4_val = -s_val / 2.0 - p2;
                    let p4_val = (p4_val.abs().ln() / 3.0).exp();
                    y3 = p3_val + p4_val - a / 3.0;
                }

                let mut iknt = 0;
                let a3 = (a1 * a1 / 4.0 - b1 + y3).sqrt();
                let b3 = (y3 * y3 / 4.0 - d1).sqrt();
                let mut a4 = [0.0_f64; 4];
                let mut b4 = [0.0_f64; 4];
                let mut x4 = [0.0_f64; 8];
                let mut poly4 = [0.0_f64; 8];

                for i in 0..4 {
                    a4[i] = a1 / 2.0 + SIG1[i] * a3;
                    b4[i] = y3 / 2.0 + SIG2[i] * b3;
                    let delta4 = a4[i] * a4[i] / 4.0 - b4[i];
                    if delta4 < 0.0 { continue; }
                    let tmp = delta4.sqrt();
                    x4[iknt] = -a4[i] / 2.0 + tmp;
                    iknt += 1;
                    x4[iknt] = -a4[i] / 2.0 - tmp;
                    iknt += 1;
                }

                let mut jknt = 0;
                let mut xvalid = 0.0;
                for j in 0..iknt {
                    if x4[j] <= 0.0 { continue; }
                    poly4[j] = x4[j] * x4[j] * x4[j] * x4[j]
                        + a1 * x4[j] * x4[j] * x4[j];
                    poly4[j] = poly4[j] + b1 * x4[j] * x4[j] + c1 * x4[j] + d1;
                    if poly4[j].abs() > 1.0e-6 { continue; }
                    jknt += 1;
                    if jknt <= 1 { xvalid = x4[j]; }
                    if x4[j] > xvalid { continue; }
                    xvalid = x4[j];
                }
                if jknt > 0 {
                    vdsat = xvalid * xvalid - phi_min_vbs;
                }
            }

            // Effective channel length (mos2load.c:829-900)
            let xlamda = mdl.lambda;
            let (mut dldvgs, mut dldvds, mut dldvbs);
            let mut clfact;

            let mut bsarg = 0.0;
            let mut dbsrdb = 0.0;
            let mut bodys = 0.0;
            let mut gdbdvs = 0.0;

            if lvds != 0.0 {
                gammad = gamasd;
                if (lvbs - vdsat) <= 0.0 {
                    bsarg = (vdsat + phi_min_vbs).sqrt();
                    dbsrdb = -0.5 / bsarg;
                } else {
                    sphi = dev.t_phi.sqrt();
                    sphi3 = dev.t_phi * sphi;
                    bsarg = sphi / (1.0 + 0.5 * (lvbs - vdsat) / dev.t_phi);
                    dbsrdb = -0.5 * bsarg * bsarg / sphi3;
                }
                bodys = bsarg * bsarg * bsarg - sarg3;
                gdbdvs = 2.0 * gammad * (bsarg * bsarg * dbsrdb - sarg1 * sarg1 * dsrgdb);

                let mut xlamda_eff = xlamda;

                if mdl.vmax <= 0.0 {
                    if mdl.nsub == 0.0 || xlamda > 0.0 {
                        // goto line610
                        dldvgs = 0.0;
                        dldvds = 0.0;
                        dldvbs = 0.0;
                    } else {
                        let argv = (lvds - vdsat) / 4.0;
                        let sargv = (1.0 + argv * argv).sqrt();
                        let arg1 = (argv + sargv).sqrt();
                        let xlfact = mdl.xd / (eff_length * lvds);
                        xlamda_eff = xlfact * arg1;
                        let dldsat = lvds * xlamda_eff / (8.0 * sargv);
                        dldvgs = dldsat * dsdvgs;
                        dldvds = -xlamda_eff + dldsat;
                        dldvbs = dldsat * dsdvbs;
                    }
                } else {
                    let argv = (vgsx - vbin) / eta - vdsat;
                    let xdv = mdl.xd / (mdl.neff as f64).sqrt();
                    let xlv = mdl.vmax * xdv / (2.0 * ueff);
                    let vqchan = argv - gammad * bsarg;
                    let dqdsat = -1.0 + gammad * dbsrdb;
                    let vl = mdl.vmax * eff_length;
                    let dfunds = vl * dqdsat - ueff * vqchan;
                    let dfundg = (vl - ueff * vdsat) / eta;
                    let dfundb = -vl * (1.0 + dqdsat - factor / eta)
                        + ueff * (gdbdvs - dgdvbs * bodys / 1.5) / eta;
                    dsdvgs = -dfundg / dfunds;
                    dsdvbs = -dfundb / dfunds;

                    if mdl.nsub == 0.0 || xlamda > 0.0 {
                        dldvgs = 0.0;
                        dldvds = 0.0;
                        dldvbs = 0.0;
                    } else {
                        let argv2 = lvds - vdsat;
                        let argv2 = f64::max(argv2, 0.0);
                        let xls = (xlv * xlv + argv2).sqrt();
                        let dldsat = xdv / (2.0 * xls);
                        let xlfact = xdv / (eff_length * lvds);
                        xlamda_eff = xlfact * (xls - xlv);
                        let dldsat = dldsat / eff_length;
                        dldvgs = dldsat * dsdvgs;
                        dldvds = -xlamda_eff + dldsat;
                        dldvbs = dldsat * dsdvbs;
                    }
                }

                // Limit channel shortening (mos2load.c:886-900)
                let xwb = mdl.xd * sbiarg;
                let xld = eff_length - xwb;
                clfact = 1.0 - xlamda_eff * lvds;
                dldvds = -xlamda_eff - dldvds;
                let xleff = eff_length * clfact;
                let deltal = xlamda_eff * lvds * eff_length;
                let xwb_limit = if mdl.nsub == 0.0 { 0.25e-6 } else { xwb };
                if xleff < xwb_limit {
                    let xleff = xwb_limit / (1.0 + (deltal - xld) / xwb_limit);
                    clfact = xleff / eff_length;
                    let dfact = xleff * xleff / (xwb_limit * xwb_limit);
                    dldvgs = dfact * dldvgs;
                    dldvds = dfact * dldvds;
                    dldvbs = dfact * dldvbs;
                }
            } else {
                // line610
                dldvgs = 0.0;
                dldvds = 0.0;
                dldvbs = 0.0;
                clfact = 1.0;
            }

            // Effective beta (mos2load.c:904)
            let beta1 = beta * ufact / clfact;

            // Test for mode of operation (mos2load.c:908-998)
            gammad = gamasd;
            dgdvbs = dgddvb;

            if lvds <= 1.0e-10 {
                // Small VDS (mos2load.c:910-926)
                if lvgs <= von {
                    if mdl.nfs == 0.0 || oxide_cap == 0.0 {
                        gds_out = 0.0;
                        cdrain = 0.0;
                        gm = 0.0;
                        gmbs = 0.0;
                    } else {
                        gds_out = beta1 * (von - vbin - gammad * sarg1)
                            * (argg * (lvgs - von)).exp();
                        cdrain = 0.0;
                        gm = 0.0;
                        gmbs = 0.0;
                    }
                } else {
                    gds_out = beta1 * (lvgs - vbin - gammad * sarg1);
                    cdrain = 0.0;
                    gm = 0.0;
                    gmbs = 0.0;
                }
            } else if (mdl.nfs != 0.0 && oxide_cap != 0.0 && lvgs <= von)
                || (mdl.nfs == 0.0 && lvgs <= vbin)
            {
                // Above threshold test / subthreshold (mos2load.c:928-966)
                if mdl.nfs == 0.0 && lvgs <= vbin {
                    // doneval: all zeros
                    gds_out = 0.0;
                    cdrain = 0.0;
                    gm = 0.0;
                    gmbs = 0.0;
                } else if lvgs > von {
                    // goto line900 — this shouldn't happen due to condition
                    unreachable!();
                } else {
                    // Subthreshold region (mos2load.c:939-966)
                    if vdsat <= 0.0 {
                        gds_out = 0.0;
                        if lvgs > vth {
                            cdrain = 0.0;
                            gm = 0.0;
                            gmbs = 0.0;
                        } else {
                            cdrain = 0.0;
                            gm = 0.0;
                            gmbs = 0.0;
                        }
                    } else {
                        let vdson = f64::min(vdsat, lvds);
                        let mut barg_sub = barg;
                        let mut dbrgdb_sub = dbrgdb;
                        let mut body_sub = body;
                        let mut gdbdv_sub = gdbdv;

                        if lvds > vdsat {
                            barg_sub = bsarg;
                            dbrgdb_sub = dbsrdb;
                            body_sub = bodys;
                            gdbdv_sub = gdbdvs;
                        }

                        let cdson = beta1 * ((von - vbin - eta * vdson * 0.5) * vdson
                            - gammad * body_sub / 1.5);
                        let didvds = beta1 * (von - vbin - eta * vdson - gammad * barg_sub);
                        let mut gdson = -cdson * dldvds / clfact
                            - beta1 * dgdvds * body_sub / 1.5;
                        if lvds < vdsat {
                            gdson = gdson + didvds;
                        }
                        let mut gbson = -cdson * dldvbs / clfact + beta1
                            * (dodvbs * vdson + factor * vdson
                                - dgdvbs * body_sub / 1.5 - gdbdv_sub);
                        if lvds > vdsat {
                            gbson = gbson + didvds * dsdvbs;
                        }
                        let expg = (argg * (lvgs - von)).exp();
                        cdrain = cdson * expg;
                        let gmw = cdrain * argg;
                        gm = gmw;
                        if lvds > vdsat {
                            gm = gmw + didvds * dsdvgs * expg;
                        }
                        let tmp = gmw * (lvgs - von) / xn;
                        gds_out = gdson * expg - gm * dodvds - tmp * dxndvd;
                        gmbs = gbson * expg - gm * dodvbs - tmp * dxndvb;
                    }
                }
            } else {
                // line900: above threshold (mos2load.c:968-994)
                if lvds <= vdsat {
                    // Linear region (mos2load.c:969-980)
                    cdrain = beta1 * ((lvgs - vbin - eta * lvds / 2.0) * lvds
                        - gammad * body / 1.5);
                    let arg1 = cdrain * (dudvgs / ufact - dldvgs / clfact);
                    gm = arg1 + beta1 * lvds;
                    let arg1 = cdrain * (dudvds / ufact - dldvds / clfact);
                    gds_out = arg1 + beta1 * (lvgs - vbin - eta * lvds
                        - gammad * barg - dgdvds * body / 1.5);
                    let arg1 = cdrain * (dudvbs / ufact - dldvbs / clfact);
                    gmbs = arg1 - beta1 * (gdbdv + dgdvbs * body / 1.5 - factor * lvds);
                } else {
                    // Saturation region (mos2load.c:981-994)
                    cdrain = beta1 * ((lvgs - vbin - eta * vdsat / 2.0) * vdsat
                        - gammad * bodys / 1.5);
                    let arg1 = cdrain * (dudvgs / ufact - dldvgs / clfact);
                    gm = arg1 + beta1 * vdsat + beta1 * (lvgs - vbin - eta * vdsat
                        - gammad * bsarg) * dsdvgs;
                    gds_out = -cdrain * dldvds / clfact - beta1 * dgdvds * bodys / 1.5;
                    let arg1 = cdrain * (dudvbs / ufact - dldvbs / clfact);
                    gmbs = arg1 - beta1 * (gdbdvs + dgdvbs * bodys / 1.5 - factor * vdsat)
                        + beta1 * (lvgs - vbin - eta * vdsat - gammad * bsarg) * dsdvbs;
                }
            }

            // Save von/vdsat (mos2load.c:1012-1013)
            dev.saved_von = tp * von;
            dev.saved_vdsat = tp * vdsat;

            // CD = mode * cdrain - cbd (mos2load.c:1017)
            dev.last_cd = ds_mode as f64 * cdrain - cbd_val;

            gds_val = gds_out;
        } else {
            // goto_line1050 was true (cutoff path)
            cdrain = 0.0;
            gm = 0.0;
            gds_val = 0.0;
            gmbs = 0.0;
            dev.saved_von = tp * von;
            dev.saved_vdsat = tp * vdsat;
            dev.last_cd = ds_mode as f64 * cdrain - cbd_val;
        }
    }

    // 5. Bulk junction depletion capacitances (mos2load.c:1019-1174)
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

        // Integrate bulk caps (mos2load.c:1151-1174)
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

    // 6. Convergence check (mos2load.c:1181-1187)
    // ngspice: if ( (off == 0) || (!(mode & (MODEINITFIX|MODEINITSMSIG))) )
    // Since we don't track the OFF flag, this is always true.
    if check {
        *noncon = true;
    }

    // 7. Save state (mos2load.c:1188-1191)
    states.set(0, so + VBS, vbs);
    states.set(0, so + VBD, vbd);
    states.set(0, so + VGS, vgs);
    states.set(0, so + VDS, vds);

    // 8. Meyer gate capacitances (mos2load.c:1196-1279)
    let (mut gcgs, mut gcgd, mut gcgb) = (0.0, 0.0, 0.0);
    let (mut ceqgs, mut ceqgd, mut ceqgb) = (0.0, 0.0, 0.0);

    if mode.is(MODETRAN) || mode.is(MODETRANOP) || mode.is(MODEINITSMSIG) {
        // Meyer uses the UNSCALED von/vdsat from moseq2, not the type-scaled saved_von/saved_vdsat.
        // In C, the outer-scope `von` variable is passed directly to DEVqmeyer.
        // saved_von = tp * von, saved_vdsat = tp * vdsat, so we divide back by tp.
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

        // Charge computation (mos2load.c:1251-1278)
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

        // Integration (mos2load.c:1287-1316)
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

    dev.last_gm = gm; dev.last_gds = gds_val; dev.last_gbd = gbd; dev.last_gbs = gbs;
    dev.last_gmbs = gmbs;
    dev.mode_sign = ds_mode;
    dev.last_vgs = vgs; dev.last_vds = vds; dev.last_vbs = vbs;

    // 9. RHS stamps (mos2load.c:1324-1346)
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

    // 10. Matrix stamps (mos2load.c:1363-1388)
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
