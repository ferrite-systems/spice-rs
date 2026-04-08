//! BSIM3v3.3.0 MOSFET model — port of ngspice bsim3/b3ld.c, b3temp.c, b3set.c.
//!
//! LEVEL=8 or LEVEL=49 with VERSION=3.3.
//! Uses ngspice's hardcoded constants (EPSOX=3.453133e-11, EPSSI=1.03594e-10, etc.)

use crate::device::Device;
use crate::error::SimError;
use crate::mna::MnaSystem;
use crate::mode::*;
use crate::state::StateVectors;

// ngspice BSIM3 constants (from b3ld.c, b3temp.c, b3set.c)
const MAX_EXP: f64 = 5.834617425e14;
const MIN_EXP: f64 = 1.713908431e-15;
const EXP_THRESHOLD: f64 = 34.0;
const EPSOX: f64 = 3.453133e-11;
const EPSSI: f64 = 1.03594e-10;
const PI: f64 = 3.141592654;
const CHARGE_Q: f64 = 1.60219e-19;
const KB: f64 = 1.3806226e-23;
const KBOQ: f64 = 8.617087e-5; // Kb / q
const DELTA_1: f64 = 0.02;
const DELTA_2: f64 = 0.02;
const DELTA_3: f64 = 0.02;
const DELTA_4: f64 = 0.02;

// From ngspice const.h / cktdefs.h
const CONSTVT0: f64 = 0.025864186389684037; // kT/q at 300.15K
const CONSTROOT2: f64 = 1.4142135623730951;

/// Number of state variables per BSIM3 instance.
const BSIM3_NUM_STATES: usize = 17;

// State vector offsets (from bsim3def.h)
const ST_VBD: usize = 0;
const ST_VBS: usize = 1;
const ST_VGS: usize = 2;
const ST_VDS: usize = 3;
const ST_QB: usize = 4;
const ST_CQB: usize = 5;
const ST_QG: usize = 6;
const ST_CQG: usize = 7;
const ST_QD: usize = 8;
const ST_CQD: usize = 9;
const ST_QBS: usize = 10;
const ST_QBD: usize = 11;
const ST_QCHEQ: usize = 12;
const ST_CQCHEQ: usize = 13;
const ST_QCDUMP: usize = 14;
const ST_CQCDUMP: usize = 15;
const ST_QDEF: usize = 16;

/// BSIM3 size-dependent parameters — computed from model + L/W binning in temperature().
#[derive(Debug, Clone)]
pub struct Bsim3SizeDepParam {
    pub leff: f64,
    pub weff: f64,
    pub leff_cv: f64,
    pub weff_cv: f64,

    // Binned parameters
    pub cdsc: f64, pub cdscb: f64, pub cdscd: f64, pub cit: f64,
    pub nfactor: f64, pub xj: f64, pub vsat: f64, pub at: f64,
    pub a0: f64, pub ags: f64, pub a1: f64, pub a2: f64,
    pub keta: f64, pub nsub: f64, pub npeak: f64, pub ngate: f64,
    pub gamma1: f64, pub gamma2: f64, pub vbx: f64, pub vbi: f64,
    pub vbm: f64, pub vbsc: f64, pub xt: f64, pub phi: f64,
    pub litl: f64, pub k1: f64, pub kt1: f64, pub kt1l: f64,
    pub kt2: f64, pub k2: f64, pub k3: f64, pub k3b: f64,
    pub w0: f64, pub nlx: f64,
    pub dvt0: f64, pub dvt1: f64, pub dvt2: f64,
    pub dvt0w: f64, pub dvt1w: f64, pub dvt2w: f64,
    pub drout: f64, pub dsub: f64,
    pub vth0: f64, pub ua: f64, pub ua1: f64, pub ub: f64,
    pub ub1: f64, pub uc: f64, pub uc1: f64, pub u0: f64,
    pub ute: f64, pub voff: f64, pub vfb: f64, pub delta: f64,
    pub rdsw: f64, pub rds0: f64, pub prwg: f64, pub prwb: f64,
    pub prt: f64, pub eta0: f64, pub etab: f64, pub pclm: f64,
    pub pdibl1: f64, pub pdibl2: f64, pub pdiblb: f64,
    pub pscbe1: f64, pub pscbe2: f64, pub pvag: f64,
    pub wr: f64, pub dwg: f64, pub dwb: f64,
    pub b0: f64, pub b1: f64,
    pub alpha0: f64, pub alpha1: f64, pub beta0: f64,
    // CV model
    pub elm: f64, pub cgsl: f64, pub cgdl: f64,
    pub ckappa: f64, pub cf: f64, pub clc: f64, pub cle: f64,
    pub vfbcv: f64, pub noff: f64, pub voffcv: f64,
    pub acde: f64, pub moin: f64,
    // Pre-calculated
    pub dl: f64, pub dw: f64, pub dlc: f64, pub dwc: f64,
    pub abulk_cv_factor: f64,
    pub cgso: f64, pub cgdo: f64, pub cgbo: f64,
    pub tconst: f64,
    pub u0temp: f64, pub vsattemp: f64,
    pub sqrt_phi: f64, pub phis3: f64,
    pub xdep0: f64, pub sqrt_xdep0: f64,
    pub theta0vb0: f64, pub theta_rout: f64,
    pub cof1: f64, pub cof2: f64, pub cof3: f64, pub cof4: f64,
    pub cdep0: f64, pub vfbzb: f64,
    pub ldeb: f64, pub k1ox: f64, pub k2ox: f64,
}

impl Default for Bsim3SizeDepParam {
    fn default() -> Self {
        Self {
            leff: 0.0, weff: 0.0, leff_cv: 0.0, weff_cv: 0.0,
            cdsc: 0.0, cdscb: 0.0, cdscd: 0.0, cit: 0.0,
            nfactor: 0.0, xj: 0.0, vsat: 0.0, at: 0.0,
            a0: 0.0, ags: 0.0, a1: 0.0, a2: 0.0,
            keta: 0.0, nsub: 0.0, npeak: 0.0, ngate: 0.0,
            gamma1: 0.0, gamma2: 0.0, vbx: 0.0, vbi: 0.0,
            vbm: 0.0, vbsc: 0.0, xt: 0.0, phi: 0.0,
            litl: 0.0, k1: 0.0, kt1: 0.0, kt1l: 0.0,
            kt2: 0.0, k2: 0.0, k3: 0.0, k3b: 0.0,
            w0: 0.0, nlx: 0.0,
            dvt0: 0.0, dvt1: 0.0, dvt2: 0.0,
            dvt0w: 0.0, dvt1w: 0.0, dvt2w: 0.0,
            drout: 0.0, dsub: 0.0,
            vth0: 0.0, ua: 0.0, ua1: 0.0, ub: 0.0,
            ub1: 0.0, uc: 0.0, uc1: 0.0, u0: 0.0,
            ute: 0.0, voff: 0.0, vfb: 0.0, delta: 0.0,
            rdsw: 0.0, rds0: 0.0, prwg: 0.0, prwb: 0.0,
            prt: 0.0, eta0: 0.0, etab: 0.0, pclm: 0.0,
            pdibl1: 0.0, pdibl2: 0.0, pdiblb: 0.0,
            pscbe1: 0.0, pscbe2: 0.0, pvag: 0.0,
            wr: 0.0, dwg: 0.0, dwb: 0.0,
            b0: 0.0, b1: 0.0,
            alpha0: 0.0, alpha1: 0.0, beta0: 0.0,
            elm: 0.0, cgsl: 0.0, cgdl: 0.0,
            ckappa: 0.0, cf: 0.0, clc: 0.0, cle: 0.0,
            vfbcv: 0.0, noff: 0.0, voffcv: 0.0,
            acde: 0.0, moin: 0.0,
            dl: 0.0, dw: 0.0, dlc: 0.0, dwc: 0.0,
            abulk_cv_factor: 0.0,
            cgso: 0.0, cgdo: 0.0, cgbo: 0.0,
            tconst: 0.0,
            u0temp: 0.0, vsattemp: 0.0,
            sqrt_phi: 0.0, phis3: 0.0,
            xdep0: 0.0, sqrt_xdep0: 0.0,
            theta0vb0: 0.0, theta_rout: 0.0,
            cof1: 0.0, cof2: 0.0, cof3: 0.0, cof4: 0.0,
            cdep0: 0.0, vfbzb: 0.0,
            ldeb: 0.0, k1ox: 0.0, k2ox: 0.0,
        }
    }
}

/// BSIM3 model parameters — parsed from .MODEL card.
#[derive(Debug, Clone)]
pub struct Bsim3Model {
    pub mos_type: i32, // +1 = NMOS, -1 = PMOS
    pub mob_mod: i32,
    pub cap_mod: i32,
    pub nqs_mod: i32,
    pub acnqs_mod: i32,
    pub bin_unit: i32,
    pub tox: f64, pub toxm: f64,
    pub cdsc: f64, pub cdscb: f64, pub cdscd: f64, pub cit: f64,
    pub nfactor: f64, pub xj: f64, pub vsat: f64, pub at: f64,
    pub a0: f64, pub ags: f64, pub a1: f64, pub a2: f64,
    pub keta: f64, pub nsub: f64, pub npeak: f64, pub ngate: f64,
    pub gamma1: f64, pub gamma2: f64, pub vbx: f64, pub vbm: f64,
    pub xt: f64, pub k1: f64, pub kt1: f64, pub kt1l: f64,
    pub kt2: f64, pub k2: f64, pub k3: f64, pub k3b: f64,
    pub w0: f64, pub nlx: f64,
    pub dvt0: f64, pub dvt1: f64, pub dvt2: f64,
    pub dvt0w: f64, pub dvt1w: f64, pub dvt2w: f64,
    pub drout: f64, pub dsub: f64,
    pub vth0: f64, pub ua: f64, pub ua1: f64,
    pub ub: f64, pub ub1: f64, pub uc: f64, pub uc1: f64,
    pub u0: f64, pub ute: f64, pub voff: f64,
    pub delta: f64, pub rdsw: f64,
    pub prwg: f64, pub prwb: f64, pub prt: f64,
    pub eta0: f64, pub etab: f64, pub pclm: f64,
    pub pdibl1: f64, pub pdibl2: f64, pub pdiblb: f64,
    pub pscbe1: f64, pub pscbe2: f64, pub pvag: f64,
    pub wr: f64, pub dwg: f64, pub dwb: f64,
    pub b0: f64, pub b1: f64,
    pub alpha0: f64, pub alpha1: f64, pub beta0: f64,
    pub ijth: f64, pub vfb: f64,
    // CV model
    pub elm: f64, pub cgsl: f64, pub cgdl: f64, pub ckappa: f64,
    pub cf: f64, pub vfbcv: f64, pub clc: f64, pub cle: f64,
    pub dwc: f64, pub dlc: f64,
    pub noff: f64, pub voffcv: f64, pub acde: f64, pub moin: f64,
    pub tcj: f64, pub tcjsw: f64, pub tcjswg: f64,
    pub tpb: f64, pub tpbsw: f64, pub tpbswg: f64,
    // Geometry
    pub xl: f64, pub xw: f64,
    // Junction
    pub sheet_resistance: f64,
    pub jct_sat_cur_density: f64,
    pub jct_sidewall_sat_cur_density: f64,
    pub bulk_jct_potential: f64,
    pub sidewall_jct_potential: f64,
    pub gate_sidewall_jct_potential: f64,
    pub unit_area_jct_cap: f64,
    pub unit_length_sidewall_jct_cap: f64,
    pub unit_length_gate_sidewall_jct_cap: f64,
    pub bulk_jct_bot_grading_coeff: f64,
    pub bulk_jct_side_grading_coeff: f64,
    pub bulk_jct_gate_side_grading_coeff: f64,
    pub jct_emission_coeff: f64,
    pub jct_temp_exponent: f64,
    // Binning
    pub lint: f64, pub ll: f64, pub llc: f64, pub lln: f64,
    pub lw: f64, pub lwc: f64, pub lwn: f64, pub lwl: f64, pub lwlc: f64,
    pub wint: f64, pub wl: f64, pub wlc: f64, pub wln: f64,
    pub ww: f64, pub wwc: f64, pub wwn: f64, pub wwl: f64, pub wwlc: f64,
    // Overlap caps
    pub cgdo: f64, pub cgso: f64, pub cgbo: f64,
    pub xpart: f64,
    pub tnom: f64,
    // Pre-calculated (set in temperature)
    pub cox: f64,
    pub vtm: f64,
    pub factor1: f64,
    pub vcrit: f64,
    pub phi_b: f64, pub phi_bsw: f64, pub phi_bswg: f64,
    pub jct_temp_sat_cur_density: f64,
    pub jct_sidewall_temp_sat_cur_density: f64,
    pub unit_area_temp_jct_cap: f64,
    pub unit_length_sidewall_temp_jct_cap: f64,
    pub unit_length_gate_sidewall_temp_jct_cap: f64,
    // Length dependence (l prefix)
    pub l_cdsc: f64, pub l_cdscb: f64, pub l_cdscd: f64, pub l_cit: f64,
    pub l_nfactor: f64, pub l_xj: f64, pub l_vsat: f64, pub l_at: f64,
    pub l_a0: f64, pub l_ags: f64, pub l_a1: f64, pub l_a2: f64,
    pub l_keta: f64, pub l_nsub: f64, pub l_npeak: f64, pub l_ngate: f64,
    pub l_gamma1: f64, pub l_gamma2: f64, pub l_vbx: f64, pub l_vbm: f64,
    pub l_xt: f64, pub l_k1: f64, pub l_kt1: f64, pub l_kt1l: f64,
    pub l_kt2: f64, pub l_k2: f64, pub l_k3: f64, pub l_k3b: f64,
    pub l_w0: f64, pub l_nlx: f64,
    pub l_dvt0: f64, pub l_dvt1: f64, pub l_dvt2: f64,
    pub l_dvt0w: f64, pub l_dvt1w: f64, pub l_dvt2w: f64,
    pub l_drout: f64, pub l_dsub: f64,
    pub l_vth0: f64, pub l_ua: f64, pub l_ua1: f64,
    pub l_ub: f64, pub l_ub1: f64, pub l_uc: f64, pub l_uc1: f64,
    pub l_u0: f64, pub l_ute: f64, pub l_voff: f64,
    pub l_delta: f64, pub l_rdsw: f64,
    pub l_prwg: f64, pub l_prwb: f64, pub l_prt: f64,
    pub l_eta0: f64, pub l_etab: f64, pub l_pclm: f64,
    pub l_pdibl1: f64, pub l_pdibl2: f64, pub l_pdiblb: f64,
    pub l_pscbe1: f64, pub l_pscbe2: f64, pub l_pvag: f64,
    pub l_wr: f64, pub l_dwg: f64, pub l_dwb: f64,
    pub l_b0: f64, pub l_b1: f64,
    pub l_alpha0: f64, pub l_alpha1: f64, pub l_beta0: f64,
    pub l_vfb: f64,
    pub l_elm: f64, pub l_cgsl: f64, pub l_cgdl: f64, pub l_ckappa: f64,
    pub l_cf: f64, pub l_clc: f64, pub l_cle: f64, pub l_vfbcv: f64,
    pub l_acde: f64, pub l_moin: f64, pub l_noff: f64, pub l_voffcv: f64,
    // Width dependence (w prefix)
    pub w_cdsc: f64, pub w_cdscb: f64, pub w_cdscd: f64, pub w_cit: f64,
    pub w_nfactor: f64, pub w_xj: f64, pub w_vsat: f64, pub w_at: f64,
    pub w_a0: f64, pub w_ags: f64, pub w_a1: f64, pub w_a2: f64,
    pub w_keta: f64, pub w_nsub: f64, pub w_npeak: f64, pub w_ngate: f64,
    pub w_gamma1: f64, pub w_gamma2: f64, pub w_vbx: f64, pub w_vbm: f64,
    pub w_xt: f64, pub w_k1: f64, pub w_kt1: f64, pub w_kt1l: f64,
    pub w_kt2: f64, pub w_k2: f64, pub w_k3: f64, pub w_k3b: f64,
    pub w_w0: f64, pub w_nlx: f64,
    pub w_dvt0: f64, pub w_dvt1: f64, pub w_dvt2: f64,
    pub w_dvt0w: f64, pub w_dvt1w: f64, pub w_dvt2w: f64,
    pub w_drout: f64, pub w_dsub: f64,
    pub w_vth0: f64, pub w_ua: f64, pub w_ua1: f64,
    pub w_ub: f64, pub w_ub1: f64, pub w_uc: f64, pub w_uc1: f64,
    pub w_u0: f64, pub w_ute: f64, pub w_voff: f64,
    pub w_delta: f64, pub w_rdsw: f64,
    pub w_prwg: f64, pub w_prwb: f64, pub w_prt: f64,
    pub w_eta0: f64, pub w_etab: f64, pub w_pclm: f64,
    pub w_pdibl1: f64, pub w_pdibl2: f64, pub w_pdiblb: f64,
    pub w_pscbe1: f64, pub w_pscbe2: f64, pub w_pvag: f64,
    pub w_wr: f64, pub w_dwg: f64, pub w_dwb: f64,
    pub w_b0: f64, pub w_b1: f64,
    pub w_alpha0: f64, pub w_alpha1: f64, pub w_beta0: f64,
    pub w_vfb: f64,
    pub w_elm: f64, pub w_cgsl: f64, pub w_cgdl: f64, pub w_ckappa: f64,
    pub w_cf: f64, pub w_clc: f64, pub w_cle: f64, pub w_vfbcv: f64,
    pub w_acde: f64, pub w_moin: f64, pub w_noff: f64, pub w_voffcv: f64,
    // Cross-term dependence (p prefix)
    pub p_cdsc: f64, pub p_cdscb: f64, pub p_cdscd: f64, pub p_cit: f64,
    pub p_nfactor: f64, pub p_xj: f64, pub p_vsat: f64, pub p_at: f64,
    pub p_a0: f64, pub p_ags: f64, pub p_a1: f64, pub p_a2: f64,
    pub p_keta: f64, pub p_nsub: f64, pub p_npeak: f64, pub p_ngate: f64,
    pub p_gamma1: f64, pub p_gamma2: f64, pub p_vbx: f64, pub p_vbm: f64,
    pub p_xt: f64, pub p_k1: f64, pub p_kt1: f64, pub p_kt1l: f64,
    pub p_kt2: f64, pub p_k2: f64, pub p_k3: f64, pub p_k3b: f64,
    pub p_w0: f64, pub p_nlx: f64,
    pub p_dvt0: f64, pub p_dvt1: f64, pub p_dvt2: f64,
    pub p_dvt0w: f64, pub p_dvt1w: f64, pub p_dvt2w: f64,
    pub p_drout: f64, pub p_dsub: f64,
    pub p_vth0: f64, pub p_ua: f64, pub p_ua1: f64,
    pub p_ub: f64, pub p_ub1: f64, pub p_uc: f64, pub p_uc1: f64,
    pub p_u0: f64, pub p_ute: f64, pub p_voff: f64,
    pub p_delta: f64, pub p_rdsw: f64,
    pub p_prwg: f64, pub p_prwb: f64, pub p_prt: f64,
    pub p_eta0: f64, pub p_etab: f64, pub p_pclm: f64,
    pub p_pdibl1: f64, pub p_pdibl2: f64, pub p_pdiblb: f64,
    pub p_pscbe1: f64, pub p_pscbe2: f64, pub p_pvag: f64,
    pub p_wr: f64, pub p_dwg: f64, pub p_dwb: f64,
    pub p_b0: f64, pub p_b1: f64,
    pub p_alpha0: f64, pub p_alpha1: f64, pub p_beta0: f64,
    pub p_vfb: f64,
    pub p_elm: f64, pub p_cgsl: f64, pub p_cgdl: f64, pub p_ckappa: f64,
    pub p_cf: f64, pub p_clc: f64, pub p_cle: f64, pub p_vfbcv: f64,
    pub p_acde: f64, pub p_moin: f64, pub p_noff: f64, pub p_voffcv: f64,
    // Given flags
    pub tnom_given: bool,
    pub k1_given: bool, pub k2_given: bool,
    pub npeak_given: bool, pub ngate_given: bool,
    pub nsub_given: bool, pub xt_given: bool,
    pub vbx_given: bool, pub vfb_given: bool,
    pub vth0_given: bool, pub gamma1_given: bool, pub gamma2_given: bool,
    pub dlc_given: bool, pub dwc_given: bool,
    pub cf_given: bool, pub cgdo_given: bool, pub cgso_given: bool, pub cgbo_given: bool,
}

impl Default for Bsim3Model {
    fn default() -> Self {
        Self {
            mos_type: 1, mob_mod: 1, cap_mod: 3, nqs_mod: 0, acnqs_mod: 0, bin_unit: 1,
            tox: 150.0e-10, toxm: 0.0, // toxm defaults to tox
            cdsc: 2.4e-4, cdscb: 0.0, cdscd: 0.0, cit: 0.0,
            nfactor: 1.0, xj: 0.15e-6, vsat: 8.0e4, at: 3.3e4,
            a0: 1.0, ags: 0.0, a1: 0.0, a2: 1.0,
            keta: -0.047, nsub: 6.0e16, npeak: 1.7e17, ngate: 0.0,
            gamma1: 0.0, gamma2: 0.0, vbx: 0.0, vbm: -3.0,
            xt: 1.55e-7, k1: 0.0, kt1: -0.11, kt1l: 0.0,
            kt2: 0.022, k2: 0.0, k3: 80.0, k3b: 0.0,
            w0: 2.5e-6, nlx: 1.74e-7,
            dvt0: 2.2, dvt1: 0.53, dvt2: -0.032,
            dvt0w: 0.0, dvt1w: 5.3e6, dvt2w: -0.032,
            drout: 0.56, dsub: 0.0, // dsub defaults to drout
            vth0: 0.0, // will be set type-dependent
            ua: 2.25e-9, ua1: 4.31e-9,
            ub: 5.87e-19, ub1: -7.61e-18,
            uc: 0.0, uc1: 0.0, // defaults depend on mobMod
            u0: 0.0, // defaults depend on type
            ute: -1.5, voff: -0.08,
            delta: 0.01, rdsw: 0.0,
            prwg: 0.0, prwb: 0.0, prt: 0.0,
            eta0: 0.08, etab: -0.07, pclm: 1.3,
            pdibl1: 0.39, pdibl2: 0.0086, pdiblb: 0.0,
            pscbe1: 4.24e8, pscbe2: 1.0e-5, pvag: 0.0,
            wr: 1.0, dwg: 0.0, dwb: 0.0,
            b0: 0.0, b1: 0.0,
            alpha0: 0.0, alpha1: 0.0, beta0: 30.0,
            ijth: 0.1, vfb: 0.0,
            elm: 5.0, cgsl: 0.0, cgdl: 0.0, ckappa: 0.6,
            cf: 0.0, vfbcv: -1.0, clc: 0.1e-6, cle: 0.6,
            dwc: 0.0, dlc: 0.0,
            noff: 1.0, voffcv: 0.0, acde: 1.0, moin: 15.0,
            tcj: 0.0, tcjsw: 0.0, tcjswg: 0.0,
            tpb: 0.0, tpbsw: 0.0, tpbswg: 0.0,
            xl: 0.0, xw: 0.0,
            sheet_resistance: 0.0,
            jct_sat_cur_density: 1.0e-4,
            jct_sidewall_sat_cur_density: 0.0,
            bulk_jct_potential: 1.0,
            sidewall_jct_potential: 1.0,
            gate_sidewall_jct_potential: 0.0, // defaults to sidewallJctPotential
            unit_area_jct_cap: 5.0e-4,
            unit_length_sidewall_jct_cap: 5.0e-10,
            unit_length_gate_sidewall_jct_cap: 0.0, // defaults to unitLengthSidewallJctCap
            bulk_jct_bot_grading_coeff: 0.5,
            bulk_jct_side_grading_coeff: 0.33,
            bulk_jct_gate_side_grading_coeff: 0.0, // defaults to sideGrading
            jct_emission_coeff: 1.0,
            jct_temp_exponent: 3.0,
            lint: 0.0, ll: 0.0, llc: 0.0, lln: 1.0,
            lw: 0.0, lwc: 0.0, lwn: 1.0, lwl: 0.0, lwlc: 0.0,
            wint: 0.0, wl: 0.0, wlc: 0.0, wln: 1.0,
            ww: 0.0, wwc: 0.0, wwn: 1.0, wwl: 0.0, wwlc: 0.0,
            cgdo: 0.0, cgso: 0.0, cgbo: 0.0,
            xpart: 0.0,
            tnom: 300.15,
            cox: 0.0, vtm: 0.0, factor1: 0.0, vcrit: 0.0,
            phi_b: 0.0, phi_bsw: 0.0, phi_bswg: 0.0,
            jct_temp_sat_cur_density: 0.0,
            jct_sidewall_temp_sat_cur_density: 0.0,
            unit_area_temp_jct_cap: 0.0,
            unit_length_sidewall_temp_jct_cap: 0.0,
            unit_length_gate_sidewall_temp_jct_cap: 0.0,
            // All L/W/P binning defaults to 0
            l_cdsc: 0.0, l_cdscb: 0.0, l_cdscd: 0.0, l_cit: 0.0,
            l_nfactor: 0.0, l_xj: 0.0, l_vsat: 0.0, l_at: 0.0,
            l_a0: 0.0, l_ags: 0.0, l_a1: 0.0, l_a2: 0.0,
            l_keta: 0.0, l_nsub: 0.0, l_npeak: 0.0, l_ngate: 0.0,
            l_gamma1: 0.0, l_gamma2: 0.0, l_vbx: 0.0, l_vbm: 0.0,
            l_xt: 0.0, l_k1: 0.0, l_kt1: 0.0, l_kt1l: 0.0,
            l_kt2: 0.0, l_k2: 0.0, l_k3: 0.0, l_k3b: 0.0,
            l_w0: 0.0, l_nlx: 0.0,
            l_dvt0: 0.0, l_dvt1: 0.0, l_dvt2: 0.0,
            l_dvt0w: 0.0, l_dvt1w: 0.0, l_dvt2w: 0.0,
            l_drout: 0.0, l_dsub: 0.0,
            l_vth0: 0.0, l_ua: 0.0, l_ua1: 0.0,
            l_ub: 0.0, l_ub1: 0.0, l_uc: 0.0, l_uc1: 0.0,
            l_u0: 0.0, l_ute: 0.0, l_voff: 0.0,
            l_delta: 0.0, l_rdsw: 0.0,
            l_prwg: 0.0, l_prwb: 0.0, l_prt: 0.0,
            l_eta0: 0.0, l_etab: 0.0, l_pclm: 0.0,
            l_pdibl1: 0.0, l_pdibl2: 0.0, l_pdiblb: 0.0,
            l_pscbe1: 0.0, l_pscbe2: 0.0, l_pvag: 0.0,
            l_wr: 0.0, l_dwg: 0.0, l_dwb: 0.0,
            l_b0: 0.0, l_b1: 0.0,
            l_alpha0: 0.0, l_alpha1: 0.0, l_beta0: 0.0,
            l_vfb: 0.0,
            l_elm: 0.0, l_cgsl: 0.0, l_cgdl: 0.0, l_ckappa: 0.0,
            l_cf: 0.0, l_clc: 0.0, l_cle: 0.0, l_vfbcv: 0.0,
            l_acde: 0.0, l_moin: 0.0, l_noff: 0.0, l_voffcv: 0.0,
            w_cdsc: 0.0, w_cdscb: 0.0, w_cdscd: 0.0, w_cit: 0.0,
            w_nfactor: 0.0, w_xj: 0.0, w_vsat: 0.0, w_at: 0.0,
            w_a0: 0.0, w_ags: 0.0, w_a1: 0.0, w_a2: 0.0,
            w_keta: 0.0, w_nsub: 0.0, w_npeak: 0.0, w_ngate: 0.0,
            w_gamma1: 0.0, w_gamma2: 0.0, w_vbx: 0.0, w_vbm: 0.0,
            w_xt: 0.0, w_k1: 0.0, w_kt1: 0.0, w_kt1l: 0.0,
            w_kt2: 0.0, w_k2: 0.0, w_k3: 0.0, w_k3b: 0.0,
            w_w0: 0.0, w_nlx: 0.0,
            w_dvt0: 0.0, w_dvt1: 0.0, w_dvt2: 0.0,
            w_dvt0w: 0.0, w_dvt1w: 0.0, w_dvt2w: 0.0,
            w_drout: 0.0, w_dsub: 0.0,
            w_vth0: 0.0, w_ua: 0.0, w_ua1: 0.0,
            w_ub: 0.0, w_ub1: 0.0, w_uc: 0.0, w_uc1: 0.0,
            w_u0: 0.0, w_ute: 0.0, w_voff: 0.0,
            w_delta: 0.0, w_rdsw: 0.0,
            w_prwg: 0.0, w_prwb: 0.0, w_prt: 0.0,
            w_eta0: 0.0, w_etab: 0.0, w_pclm: 0.0,
            w_pdibl1: 0.0, w_pdibl2: 0.0, w_pdiblb: 0.0,
            w_pscbe1: 0.0, w_pscbe2: 0.0, w_pvag: 0.0,
            w_wr: 0.0, w_dwg: 0.0, w_dwb: 0.0,
            w_b0: 0.0, w_b1: 0.0,
            w_alpha0: 0.0, w_alpha1: 0.0, w_beta0: 0.0,
            w_vfb: 0.0,
            w_elm: 0.0, w_cgsl: 0.0, w_cgdl: 0.0, w_ckappa: 0.0,
            w_cf: 0.0, w_clc: 0.0, w_cle: 0.0, w_vfbcv: 0.0,
            w_acde: 0.0, w_moin: 0.0, w_noff: 0.0, w_voffcv: 0.0,
            p_cdsc: 0.0, p_cdscb: 0.0, p_cdscd: 0.0, p_cit: 0.0,
            p_nfactor: 0.0, p_xj: 0.0, p_vsat: 0.0, p_at: 0.0,
            p_a0: 0.0, p_ags: 0.0, p_a1: 0.0, p_a2: 0.0,
            p_keta: 0.0, p_nsub: 0.0, p_npeak: 0.0, p_ngate: 0.0,
            p_gamma1: 0.0, p_gamma2: 0.0, p_vbx: 0.0, p_vbm: 0.0,
            p_xt: 0.0, p_k1: 0.0, p_kt1: 0.0, p_kt1l: 0.0,
            p_kt2: 0.0, p_k2: 0.0, p_k3: 0.0, p_k3b: 0.0,
            p_w0: 0.0, p_nlx: 0.0,
            p_dvt0: 0.0, p_dvt1: 0.0, p_dvt2: 0.0,
            p_dvt0w: 0.0, p_dvt1w: 0.0, p_dvt2w: 0.0,
            p_drout: 0.0, p_dsub: 0.0,
            p_vth0: 0.0, p_ua: 0.0, p_ua1: 0.0,
            p_ub: 0.0, p_ub1: 0.0, p_uc: 0.0, p_uc1: 0.0,
            p_u0: 0.0, p_ute: 0.0, p_voff: 0.0,
            p_delta: 0.0, p_rdsw: 0.0,
            p_prwg: 0.0, p_prwb: 0.0, p_prt: 0.0,
            p_eta0: 0.0, p_etab: 0.0, p_pclm: 0.0,
            p_pdibl1: 0.0, p_pdibl2: 0.0, p_pdiblb: 0.0,
            p_pscbe1: 0.0, p_pscbe2: 0.0, p_pvag: 0.0,
            p_wr: 0.0, p_dwg: 0.0, p_dwb: 0.0,
            p_b0: 0.0, p_b1: 0.0,
            p_alpha0: 0.0, p_alpha1: 0.0, p_beta0: 0.0,
            p_vfb: 0.0,
            p_elm: 0.0, p_cgsl: 0.0, p_cgdl: 0.0, p_ckappa: 0.0,
            p_cf: 0.0, p_clc: 0.0, p_cle: 0.0, p_vfbcv: 0.0,
            p_acde: 0.0, p_moin: 0.0, p_noff: 0.0, p_voffcv: 0.0,
            tnom_given: false,
            k1_given: false, k2_given: false,
            npeak_given: false, ngate_given: false,
            nsub_given: false, xt_given: false,
            vbx_given: false, vfb_given: false,
            vth0_given: false, gamma1_given: false, gamma2_given: false,
            dlc_given: false, dwc_given: false,
            cf_given: false, cgdo_given: false, cgso_given: false, cgbo_given: false,
        }
    }
}

impl Bsim3Model {
    /// Apply post-parse defaults that depend on other parameters.
    /// Port of the initial section of BSIM3setup (b3set.c).
    pub fn apply_defaults(&mut self) {
        // Type-dependent defaults
        if self.vth0 == 0.0 && !self.vth0_given {
            self.vth0 = if self.mos_type == 1 { 0.7 } else { -0.7 };
        }
        if self.u0 == 0.0 {
            self.u0 = if self.mos_type == 1 { 0.067 } else { 0.025 };
        }
        if self.uc == 0.0 {
            self.uc = if self.mob_mod == 3 { -0.0465 } else { -0.0465e-9 };
        }
        if self.uc1 == 0.0 {
            self.uc1 = if self.mob_mod == 3 { -0.056 } else { -0.056e-9 };
        }
        // dsub defaults to drout
        if self.dsub == 0.0 { self.dsub = self.drout; }
        // toxm defaults to tox
        if self.toxm == 0.0 { self.toxm = self.tox; }
        // cox
        self.cox = EPSOX / self.tox;
        // Binning defaults that depend on other params
        if self.llc == 0.0 { self.llc = self.ll; }
        if self.lwc == 0.0 { self.lwc = self.lw; }
        if self.lwlc == 0.0 { self.lwlc = self.lwl; }
        if self.wlc == 0.0 { self.wlc = self.wl; }
        if self.wwc == 0.0 { self.wwc = self.ww; }
        if self.wwlc == 0.0 { self.wwlc = self.wwl; }
        if !self.dwc_given { self.dwc = self.wint; }
        if !self.dlc_given { self.dlc = self.lint; }
        // cf
        if !self.cf_given {
            self.cf = 2.0 * EPSOX / PI * (1.0 + 0.4e-6 / self.tox).ln();
        }
        // cgdo/cgso
        if !self.cgdo_given {
            if self.dlc_given && self.dlc > 0.0 {
                self.cgdo = self.dlc * self.cox - self.cgdl;
            } else {
                self.cgdo = 0.6 * self.xj * self.cox;
            }
        }
        if !self.cgso_given {
            if self.dlc_given && self.dlc > 0.0 {
                self.cgso = self.dlc * self.cox - self.cgsl;
            } else {
                self.cgso = 0.6 * self.xj * self.cox;
            }
        }
        if !self.cgbo_given {
            self.cgbo = 2.0 * self.dwc * self.cox;
        }
        // Gate sidewall junction potential
        if self.gate_sidewall_jct_potential == 0.0 {
            self.gate_sidewall_jct_potential = self.sidewall_jct_potential;
        }
        if self.unit_length_gate_sidewall_jct_cap == 0.0 {
            self.unit_length_gate_sidewall_jct_cap = self.unit_length_sidewall_jct_cap;
        }
        if self.bulk_jct_gate_side_grading_coeff == 0.0 {
            self.bulk_jct_gate_side_grading_coeff = self.bulk_jct_side_grading_coeff;
        }
    }
}

/// BSIM3 device instance.
#[derive(Debug)]
pub struct Bsim3 {
    name: String,
    // External nodes
    d_node: usize, g_node: usize, s_node: usize, b_node: usize,
    // Internal nodes
    dp_node: usize, sp_node: usize,
    // Model
    model: Bsim3Model,
    w: f64, l: f64, m: f64,
    // Size-dependent parameters
    param: Bsim3SizeDepParam,
    // Instance variables (from bsim3def.h instance struct)
    inst_vth0: f64, inst_vfb: f64, inst_vfbzb: f64,
    inst_u0temp: f64, inst_tconst: f64,
    drain_conductance: f64, source_conductance: f64,
    inst_cgso: f64, inst_cgdo: f64,
    vjsm: f64, is_evjsm: f64, vjdm: f64, is_evjdm: f64,
    // OP point
    cd: f64, cbs: f64, cbd: f64, csub: f64,
    gm: f64, gds: f64, gmbs: f64,
    gbd: f64, gbs: f64,
    gbbs: f64, gbgs: f64, gbds: f64,
    von: f64, vdsat: f64,
    mode: i32,
    // Capacitance OP
    cggb: f64, cgdb: f64, cgsb: f64,
    cdgb: f64, cddb: f64, cdsb: f64,
    cbgb: f64, cbdb: f64, cbsb: f64,
    capbd: f64, capbs: f64,
    cqgb: f64, cqdb: f64, cqsb: f64, cqbb: f64,
    qgate: f64, qbulk: f64, qdrn: f64, qinv: f64,
    inst_cgdo_op: f64, inst_cgso_op: f64,
    gtau: f64, gtg: f64, gtd: f64, gts: f64, gtb: f64,
    taunet: f64,
    rds: f64,
    ueff: f64, thetavth: f64,
    vgsteff: f64, vdseff: f64, abulk: f64,
    above_vgst2vtm: f64,
    off: bool,
    // Area/perimeter for junction caps
    drain_area: f64, source_area: f64,
    drain_perimeter: f64, source_perimeter: f64,
    drain_squares: f64, source_squares: f64,
    // Intermediate derivatives from Ids (needed by charge model)
    vbseff_ids: f64, // Vbseff from Ids computation
    d_vbseff_d_vb: f64,
    d_vgs_eff_d_vg: f64,
    d_vth_d_vb: f64, d_vth_d_vd: f64,
    d_vgsteff_d_vg: f64, d_vgsteff_d_vd: f64, d_vgsteff_d_vb: f64,
    abulk0: f64, d_abulk0_d_vb: f64,
    sqrt_phis: f64, d_sqrt_phis_d_vb: f64,
    phis: f64, d_phis_d_vb: f64,
    // Subthreshold slope factor and Vgst (for VgsteffCV in charge model)
    n_ids: f64, dn_dvb: f64, dn_dvd: f64, vgst: f64,
    // Device initial conditions (from .IC node voltages or instance params)
    ic_vds: f64, ic_vgs: f64, ic_vbs: f64,
    ic_vds_given: bool, ic_vgs_given: bool, ic_vbs_given: bool,
    // Last computed voltages (for trace comparison)
    last_vgs: f64, last_vds: f64, last_vbs: f64,
    // State offset
    state_offset: usize,
    // Integration coefficients
    pub ag: [f64; 7],
    pub order: usize,
    pub delta: f64,
    pub delta_old1: f64,
}

impl Bsim3 {
    pub fn new(name: &str, d: usize, g: usize, s: usize, b: usize,
               model: Bsim3Model, w: f64, l: f64, m: f64) -> Self {
        Self {
            name: name.to_string(),
            d_node: d, g_node: g, s_node: s, b_node: b,
            dp_node: d, sp_node: s,
            model, w, l, m,
            param: Bsim3SizeDepParam::default(),
            inst_vth0: 0.0, inst_vfb: 0.0, inst_vfbzb: 0.0,
            inst_u0temp: 0.0, inst_tconst: 0.0,
            drain_conductance: 0.0, source_conductance: 0.0,
            inst_cgso: 0.0, inst_cgdo: 0.0,
            vjsm: 0.0, is_evjsm: 0.0, vjdm: 0.0, is_evjdm: 0.0,
            cd: 0.0, cbs: 0.0, cbd: 0.0, csub: 0.0,
            gm: 0.0, gds: 0.0, gmbs: 0.0,
            gbd: 0.0, gbs: 0.0,
            gbbs: 0.0, gbgs: 0.0, gbds: 0.0,
            von: 0.0, vdsat: 0.0,
            mode: 1,
            cggb: 0.0, cgdb: 0.0, cgsb: 0.0,
            cdgb: 0.0, cddb: 0.0, cdsb: 0.0,
            cbgb: 0.0, cbdb: 0.0, cbsb: 0.0,
            capbd: 0.0, capbs: 0.0,
            cqgb: 0.0, cqdb: 0.0, cqsb: 0.0, cqbb: 0.0,
            qgate: 0.0, qbulk: 0.0, qdrn: 0.0, qinv: 0.0,
            inst_cgdo_op: 0.0, inst_cgso_op: 0.0,
            gtau: 0.0, gtg: 0.0, gtd: 0.0, gts: 0.0, gtb: 0.0,
            taunet: 0.0,
            rds: 0.0,
            ueff: 0.0, thetavth: 0.0,
            vgsteff: 0.0, vdseff: 0.0, abulk: 0.0,
            above_vgst2vtm: 0.0,
            off: false,
            drain_area: 0.0, source_area: 0.0,
            drain_perimeter: 0.0, source_perimeter: 0.0,
            drain_squares: 1.0, source_squares: 1.0,
            vbseff_ids: 0.0, d_vbseff_d_vb: 0.0, d_vgs_eff_d_vg: 0.0,
            d_vth_d_vb: 0.0, d_vth_d_vd: 0.0,
            d_vgsteff_d_vg: 0.0, d_vgsteff_d_vd: 0.0, d_vgsteff_d_vb: 0.0,
            abulk0: 0.0, d_abulk0_d_vb: 0.0,
            sqrt_phis: 0.0, d_sqrt_phis_d_vb: 0.0,
            phis: 0.0, d_phis_d_vb: 0.0,
            n_ids: 1.0, dn_dvb: 0.0, dn_dvd: 0.0, vgst: 0.0,
            ic_vds: 0.0, ic_vgs: 0.0, ic_vbs: 0.0,
            ic_vds_given: false, ic_vgs_given: false, ic_vbs_given: false,
            last_vgs: 0.0, last_vds: 0.0, last_vbs: 0.0,
            state_offset: 0,
            ag: [0.0; 7],
            order: 1,
            delta: 0.0,
            delta_old1: 0.0,
        }
    }

    pub fn set_internal_nodes(&mut self, dp: usize, sp: usize) {
        self.dp_node = dp;
        self.sp_node = sp;
    }

    /// Return charge state offsets for LTE truncation (qb, qg, qd).
    pub fn qcap_offsets(&self) -> [usize; 3] {
        let base = self.state_offset;
        [base + ST_QB, base + ST_QG, base + ST_QD]
    }
}

// ========================================================================
// Temperature processing — port of b3temp.c
// ========================================================================
fn bsim3_temperature(dev: &mut Bsim3, temp: f64, tnom_global: f64) {
    let model = &mut dev.model;
    let tnom = if model.tnom_given { model.tnom } else { tnom_global };
    model.tnom = tnom;

    // Clamp junction potentials
    if model.bulk_jct_potential < 0.1 { model.bulk_jct_potential = 0.1; }
    if model.sidewall_jct_potential < 0.1 { model.sidewall_jct_potential = 0.1; }
    if model.gate_sidewall_jct_potential < 0.1 { model.gate_sidewall_jct_potential = 0.1; }

    let t_ratio = temp / tnom;
    model.vcrit = CONSTVT0 * (CONSTVT0 / (CONSTROOT2 * 1.0e-14)).ln();
    model.factor1 = (EPSSI / EPSOX * model.tox).sqrt();

    let vtm0 = KBOQ * tnom;
    let eg0 = 1.16 - 7.02e-4 * tnom * tnom / (tnom + 1108.0);
    let ni = 1.45e10 * (tnom / 300.15) * (tnom / 300.15).sqrt()
           * (21.5565981 - eg0 / (2.0 * vtm0)).exp();

    model.vtm = KBOQ * temp;
    let eg = 1.16 - 7.02e-4 * temp * temp / (temp + 1108.0);

    if temp != tnom {
        let t0 = eg0 / vtm0 - eg / model.vtm + model.jct_temp_exponent
               * (temp / tnom).ln();
        let t1 = (t0 / model.jct_emission_coeff).exp();
        model.jct_temp_sat_cur_density = model.jct_sat_cur_density * t1;
        model.jct_sidewall_temp_sat_cur_density = model.jct_sidewall_sat_cur_density * t1;
    } else {
        model.jct_temp_sat_cur_density = model.jct_sat_cur_density;
        model.jct_sidewall_temp_sat_cur_density = model.jct_sidewall_sat_cur_density;
    }
    if model.jct_temp_sat_cur_density < 0.0 { model.jct_temp_sat_cur_density = 0.0; }
    if model.jct_sidewall_temp_sat_cur_density < 0.0 { model.jct_sidewall_temp_sat_cur_density = 0.0; }

    // Junction cap temperature dependence
    let del_temp = temp - tnom;
    let t0 = model.tcj * del_temp;
    model.unit_area_temp_jct_cap = if t0 >= -1.0 {
        model.unit_area_jct_cap * (1.0 + t0)
    } else { 0.0 };

    let t0 = model.tcjsw * del_temp;
    model.unit_length_sidewall_temp_jct_cap = if t0 >= -1.0 {
        model.unit_length_sidewall_jct_cap * (1.0 + t0)
    } else { 0.0 };

    let t0 = model.tcjswg * del_temp;
    model.unit_length_gate_sidewall_temp_jct_cap = if t0 >= -1.0 {
        model.unit_length_gate_sidewall_jct_cap * (1.0 + t0)
    } else { 0.0 };

    model.phi_b = model.bulk_jct_potential - model.tpb * del_temp;
    if model.phi_b < 0.01 { model.phi_b = 0.01; }
    model.phi_bsw = model.sidewall_jct_potential - model.tpbsw * del_temp;
    if model.phi_bsw <= 0.01 { model.phi_bsw = 0.01; }
    model.phi_bswg = model.gate_sidewall_jct_potential - model.tpbswg * del_temp;
    if model.phi_bswg <= 0.01 { model.phi_bswg = 0.01; }

    // Size-dependent parameter binning
    let p = &mut dev.param;
    let ldrn = dev.l;
    let wdrn = dev.w;

    let t0_l = ldrn.powf(model.lln);
    let t1_l = wdrn.powf(model.lwn);
    let tmp1 = model.ll / t0_l + model.lw / t1_l + model.lwl / (t0_l * t1_l);
    p.dl = model.lint + tmp1;
    let tmp2 = model.llc / t0_l + model.lwc / t1_l + model.lwlc / (t0_l * t1_l);
    p.dlc = model.dlc + tmp2;

    let t2_l = ldrn.powf(model.wln);
    let t3_l = wdrn.powf(model.wwn);
    let tmp1 = model.wl / t2_l + model.ww / t3_l + model.wwl / (t2_l * t3_l);
    p.dw = model.wint + tmp1;
    let tmp2 = model.wlc / t2_l + model.wwc / t3_l + model.wwlc / (t2_l * t3_l);
    p.dwc = model.dwc + tmp2;

    p.leff = ldrn + model.xl - 2.0 * p.dl;
    p.weff = wdrn + model.xw - 2.0 * p.dw;
    p.leff_cv = ldrn + model.xl - 2.0 * p.dlc;
    p.weff_cv = wdrn + model.xw - 2.0 * p.dwc;

    let (inv_l, inv_w, inv_lw) = if model.bin_unit == 1 {
        (1.0e-6 / p.leff, 1.0e-6 / p.weff, 1.0e-12 / (p.leff * p.weff))
    } else {
        (1.0 / p.leff, 1.0 / p.weff, 1.0 / (p.leff * p.weff))
    };

    // Macro-like binning: param = base + l*inv_l + w*inv_w + p*inv_lw
    macro_rules! bin {
        ($base:ident, $lp:ident, $wp:ident, $pp:ident) => {
            model.$base + model.$lp * inv_l + model.$wp * inv_w + model.$pp * inv_lw
        };
    }

    p.cdsc = bin!(cdsc, l_cdsc, w_cdsc, p_cdsc);
    p.cdscb = bin!(cdscb, l_cdscb, w_cdscb, p_cdscb);
    p.cdscd = bin!(cdscd, l_cdscd, w_cdscd, p_cdscd);
    p.cit = bin!(cit, l_cit, w_cit, p_cit);
    p.nfactor = bin!(nfactor, l_nfactor, w_nfactor, p_nfactor);
    p.xj = bin!(xj, l_xj, w_xj, p_xj);
    p.vsat = bin!(vsat, l_vsat, w_vsat, p_vsat);
    p.at = bin!(at, l_at, w_at, p_at);
    p.a0 = bin!(a0, l_a0, w_a0, p_a0);
    p.ags = bin!(ags, l_ags, w_ags, p_ags);
    p.a1 = bin!(a1, l_a1, w_a1, p_a1);
    p.a2 = bin!(a2, l_a2, w_a2, p_a2);
    p.keta = bin!(keta, l_keta, w_keta, p_keta);
    p.nsub = bin!(nsub, l_nsub, w_nsub, p_nsub);
    p.npeak = bin!(npeak, l_npeak, w_npeak, p_npeak);
    p.ngate = bin!(ngate, l_ngate, w_ngate, p_ngate);
    p.gamma1 = bin!(gamma1, l_gamma1, w_gamma1, p_gamma1);
    p.gamma2 = bin!(gamma2, l_gamma2, w_gamma2, p_gamma2);
    p.vbx = bin!(vbx, l_vbx, w_vbx, p_vbx);
    p.vbm = bin!(vbm, l_vbm, w_vbm, p_vbm);
    p.xt = bin!(xt, l_xt, w_xt, p_xt);
    p.vfb = bin!(vfb, l_vfb, w_vfb, p_vfb);
    p.k1 = bin!(k1, l_k1, w_k1, p_k1);
    p.kt1 = bin!(kt1, l_kt1, w_kt1, p_kt1);
    p.kt1l = bin!(kt1l, l_kt1l, w_kt1l, p_kt1l);
    p.k2 = bin!(k2, l_k2, w_k2, p_k2);
    p.kt2 = bin!(kt2, l_kt2, w_kt2, p_kt2);
    p.k3 = bin!(k3, l_k3, w_k3, p_k3);
    p.k3b = bin!(k3b, l_k3b, w_k3b, p_k3b);
    p.w0 = bin!(w0, l_w0, w_w0, p_w0);
    p.nlx = bin!(nlx, l_nlx, w_nlx, p_nlx);
    p.dvt0 = bin!(dvt0, l_dvt0, w_dvt0, p_dvt0);
    p.dvt1 = bin!(dvt1, l_dvt1, w_dvt1, p_dvt1);
    p.dvt2 = bin!(dvt2, l_dvt2, w_dvt2, p_dvt2);
    p.dvt0w = bin!(dvt0w, l_dvt0w, w_dvt0w, p_dvt0w);
    p.dvt1w = bin!(dvt1w, l_dvt1w, w_dvt1w, p_dvt1w);
    p.dvt2w = bin!(dvt2w, l_dvt2w, w_dvt2w, p_dvt2w);
    p.drout = bin!(drout, l_drout, w_drout, p_drout);
    p.dsub = bin!(dsub, l_dsub, w_dsub, p_dsub);
    p.vth0 = bin!(vth0, l_vth0, w_vth0, p_vth0);
    p.ua = bin!(ua, l_ua, w_ua, p_ua);
    p.ua1 = bin!(ua1, l_ua1, w_ua1, p_ua1);
    p.ub = bin!(ub, l_ub, w_ub, p_ub);
    p.ub1 = bin!(ub1, l_ub1, w_ub1, p_ub1);
    p.uc = bin!(uc, l_uc, w_uc, p_uc);
    p.uc1 = bin!(uc1, l_uc1, w_uc1, p_uc1);
    p.u0 = bin!(u0, l_u0, w_u0, p_u0);
    p.ute = bin!(ute, l_ute, w_ute, p_ute);
    p.voff = bin!(voff, l_voff, w_voff, p_voff);
    p.delta = bin!(delta, l_delta, w_delta, p_delta);
    p.rdsw = bin!(rdsw, l_rdsw, w_rdsw, p_rdsw);
    p.prwg = bin!(prwg, l_prwg, w_prwg, p_prwg);
    p.prwb = bin!(prwb, l_prwb, w_prwb, p_prwb);
    p.prt = bin!(prt, l_prt, w_prt, p_prt);
    p.eta0 = bin!(eta0, l_eta0, w_eta0, p_eta0);
    p.etab = bin!(etab, l_etab, w_etab, p_etab);
    p.pclm = bin!(pclm, l_pclm, w_pclm, p_pclm);
    p.pdibl1 = bin!(pdibl1, l_pdibl1, w_pdibl1, p_pdibl1);
    p.pdibl2 = bin!(pdibl2, l_pdibl2, w_pdibl2, p_pdibl2);
    p.pdiblb = bin!(pdiblb, l_pdiblb, w_pdiblb, p_pdiblb);
    p.pscbe1 = bin!(pscbe1, l_pscbe1, w_pscbe1, p_pscbe1);
    p.pscbe2 = bin!(pscbe2, l_pscbe2, w_pscbe2, p_pscbe2);
    p.pvag = bin!(pvag, l_pvag, w_pvag, p_pvag);
    p.wr = bin!(wr, l_wr, w_wr, p_wr);
    p.dwg = bin!(dwg, l_dwg, w_dwg, p_dwg);
    p.dwb = bin!(dwb, l_dwb, w_dwb, p_dwb);
    p.b0 = bin!(b0, l_b0, w_b0, p_b0);
    p.b1 = bin!(b1, l_b1, w_b1, p_b1);
    p.alpha0 = bin!(alpha0, l_alpha0, w_alpha0, p_alpha0);
    p.alpha1 = bin!(alpha1, l_alpha1, w_alpha1, p_alpha1);
    p.beta0 = bin!(beta0, l_beta0, w_beta0, p_beta0);
    // CV
    p.elm = bin!(elm, l_elm, w_elm, p_elm);
    p.cgsl = bin!(cgsl, l_cgsl, w_cgsl, p_cgsl);
    p.cgdl = bin!(cgdl, l_cgdl, w_cgdl, p_cgdl);
    p.ckappa = bin!(ckappa, l_ckappa, w_ckappa, p_ckappa);
    p.cf = bin!(cf, l_cf, w_cf, p_cf);
    p.clc = bin!(clc, l_clc, w_clc, p_clc);
    p.cle = bin!(cle, l_cle, w_cle, p_cle);
    p.vfbcv = bin!(vfbcv, l_vfbcv, w_vfbcv, p_vfbcv);
    p.acde = bin!(acde, l_acde, w_acde, p_acde);
    p.moin = bin!(moin, l_moin, w_moin, p_moin);
    p.noff = bin!(noff, l_noff, w_noff, p_noff);
    p.voffcv = bin!(voffcv, l_voffcv, w_voffcv, p_voffcv);

    p.abulk_cv_factor = 1.0 + (p.clc / p.leff_cv).powf(p.cle);

    // Temperature-dependent adjustments
    let t0 = t_ratio - 1.0;
    p.ua = p.ua + p.ua1 * t0;
    p.ub = p.ub + p.ub1 * t0;
    p.uc = p.uc + p.uc1 * t0;
    if p.u0 > 1.0 { p.u0 = p.u0 / 1.0e4; }

    p.u0temp = p.u0 * t_ratio.powf(p.ute);
    p.vsattemp = p.vsat - p.at * t0;
    p.rds0 = (p.rdsw + p.prt * t0) / (p.weff * 1e6).powf(p.wr);

    p.cgdo = (model.cgdo + p.cf) * p.weff_cv;
    p.cgso = (model.cgso + p.cf) * p.weff_cv;
    p.cgbo = model.cgbo * p.leff_cv;

    let t0_cv = p.leff_cv * p.leff_cv;
    p.tconst = p.u0temp * p.elm / (model.cox * p.weff_cv * p.leff_cv * t0_cv);

    if !model.npeak_given && model.gamma1_given {
        let t0 = p.gamma1 * model.cox;
        p.npeak = 3.021e22 * t0 * t0;
    }

    p.phi = 2.0 * vtm0 * (p.npeak / ni).ln();
    p.sqrt_phi = p.phi.sqrt();
    p.phis3 = p.sqrt_phi * p.phi;

    p.xdep0 = (2.0 * EPSSI / (CHARGE_Q * p.npeak * 1.0e6)).sqrt() * p.sqrt_phi;
    p.sqrt_xdep0 = p.xdep0.sqrt();
    p.litl = (3.0 * p.xj * model.tox).sqrt();
    p.vbi = vtm0 * (1.0e20 * p.npeak / (ni * ni)).ln();
    p.cdep0 = (CHARGE_Q * EPSSI * p.npeak * 1.0e6 / 2.0 / p.phi).sqrt();

    p.ldeb = (EPSSI * vtm0 / (CHARGE_Q * p.npeak * 1.0e6)).sqrt() / 3.0;
    p.acde *= (p.npeak / 2.0e16).powf(-0.25);

    // k1, k2 computation
    if model.k1_given || model.k2_given {
        if !model.k1_given { p.k1 = 0.53; }
        if !model.k2_given { p.k2 = -0.0186; }
    } else {
        if !model.vbx_given {
            p.vbx = p.phi - 7.7348e-4 * p.npeak * p.xt * p.xt;
        }
        if p.vbx > 0.0 { p.vbx = -p.vbx; }
        if p.vbm > 0.0 { p.vbm = -p.vbm; }
        if !model.gamma1_given {
            p.gamma1 = 5.753e-12 * p.npeak.sqrt() / model.cox;
        }
        if !model.gamma2_given {
            p.gamma2 = 5.753e-12 * p.nsub.sqrt() / model.cox;
        }
        let t0 = p.gamma1 - p.gamma2;
        let t1 = (p.phi - p.vbx).sqrt() - p.sqrt_phi;
        let t2 = (p.phi * (p.phi - p.vbm)).sqrt() - p.phi;
        p.k2 = t0 * t1 / (2.0 * t2 + p.vbm);
        p.k1 = p.gamma2 - 2.0 * p.k2 * (p.phi - p.vbm).sqrt();
    }

    if p.k2 < 0.0 {
        let t0 = 0.5 * p.k1 / p.k2;
        p.vbsc = 0.9 * (p.phi - t0 * t0);
        if p.vbsc > -3.0 { p.vbsc = -3.0; }
        else if p.vbsc < -30.0 { p.vbsc = -30.0; }
    } else {
        p.vbsc = -30.0;
    }
    if p.vbsc > p.vbm { p.vbsc = p.vbm; }

    if !model.vfb_given {
        if model.vth0_given {
            p.vfb = model.mos_type as f64 * p.vth0 - p.phi - p.k1 * p.sqrt_phi;
        } else {
            p.vfb = -1.0;
        }
    }
    if !model.vth0_given {
        p.vth0 = model.mos_type as f64 * (p.vfb + p.phi + p.k1 * p.sqrt_phi);
    }

    p.k1ox = p.k1 * model.tox / model.toxm;
    p.k2ox = p.k2 * model.tox / model.toxm;

    let t1 = (EPSSI / EPSOX * model.tox * p.xdep0).sqrt();
    let t0 = (-0.5 * p.dsub * p.leff / t1).exp();
    p.theta0vb0 = t0 + 2.0 * t0 * t0;

    let t0 = (-0.5 * p.drout * p.leff / t1).exp();
    let t2 = t0 + 2.0 * t0 * t0;
    p.theta_rout = p.pdibl1 * t2 + p.pdibl2;

    let tmp = p.xdep0.sqrt();
    let tmp1 = p.vbi - p.phi;
    let tmp2 = model.factor1 * tmp;

    let t0 = -0.5 * p.dvt1w * p.weff * p.leff / tmp2;
    let (t1, t2) = if t0 > -EXP_THRESHOLD {
        let t1 = t0.exp();
        (t1, t1 * (1.0 + 2.0 * t1))
    } else {
        (MIN_EXP, MIN_EXP * (1.0 + 2.0 * MIN_EXP))
    };
    let t0_val = p.dvt0w * t2;
    let t2_val = t0_val * tmp1;

    let t0 = -0.5 * p.dvt1 * p.leff / tmp2;
    let t3 = if t0 > -EXP_THRESHOLD {
        let t1 = t0.exp();
        t1 * (1.0 + 2.0 * t1)
    } else {
        MIN_EXP * (1.0 + 2.0 * MIN_EXP)
    };
    let t3 = p.dvt0 * t3 * tmp1;

    let t4 = model.tox * p.phi / (p.weff + p.w0);

    let t0 = (1.0 + p.nlx / p.leff).sqrt();
    let t5 = p.k1ox * (t0 - 1.0) * p.sqrt_phi
           + (p.kt1 + p.kt1l / p.leff) * (t_ratio - 1.0);

    let tmp3 = model.mos_type as f64 * p.vth0 - t2_val - t3 + p.k3 * t4 + t5;
    p.vfbzb = tmp3 - p.phi - p.k1 * p.sqrt_phi;

    // Instance adjustments (delvto, mulu0)
    dev.inst_vth0 = p.vth0; // no delvto support for now
    dev.inst_vfb = p.vfb;
    dev.inst_vfbzb = p.vfbzb;
    dev.inst_u0temp = p.u0temp;
    dev.inst_tconst = dev.inst_u0temp * p.elm / (model.cox * p.weff_cv * p.leff_cv * t0_cv);

    // Source/drain resistance
    let drain_r = model.sheet_resistance * 1.0; // drainSquares default 1
    let source_r = model.sheet_resistance * 1.0;
    dev.drain_conductance = if drain_r > 0.0 { 1.0 / drain_r } else { 0.0 };
    dev.source_conductance = if source_r > 0.0 { 1.0 / source_r } else { 0.0 };

    dev.inst_cgso = p.cgso;
    dev.inst_cgdo = p.cgdo;

    // Junction saturation current + threshold
    let nvtm = model.vtm * model.jct_emission_coeff;
    // Source
    let source_sat_current = 1.0e-14_f64.max(0.0); // area=0, perim=0 → 1e-14
    if source_sat_current > 0.0 && model.ijth > 0.0 {
        dev.vjsm = nvtm * (model.ijth / source_sat_current + 1.0).ln();
        dev.is_evjsm = source_sat_current * (dev.vjsm / nvtm).exp();
    }
    // Drain
    let drain_sat_current = 1.0e-14_f64;
    if drain_sat_current > 0.0 && model.ijth > 0.0 {
        dev.vjdm = nvtm * (model.ijth / drain_sat_current + 1.0).ln();
        dev.is_evjdm = drain_sat_current * (dev.vjdm / nvtm).exp();
    }
}

// ========================================================================
// Device trait implementation
// ========================================================================
impl Device for Bsim3 {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn name(&self) -> &str { &self.name }

    fn setic(&mut self, rhs: &[f64]) {
        if !self.ic_vbs_given { self.ic_vbs = rhs[self.b_node] - rhs[self.s_node]; }
        if !self.ic_vds_given { self.ic_vds = rhs[self.d_node] - rhs[self.s_node]; }
        if !self.ic_vgs_given { self.ic_vgs = rhs[self.g_node] - rhs[self.s_node]; }
    }

    fn setup(&mut self, states: &mut StateVectors) -> usize {
        self.state_offset = states.allocate(BSIM3_NUM_STATES);
        BSIM3_NUM_STATES
    }

    fn setup_matrix(&mut self, mna: &mut MnaSystem) {
        let (d, g, s, b, dp, sp) = (self.d_node, self.g_node, self.s_node, self.b_node, self.dp_node, self.sp_node);
        // 22 TSTALLOC elements matching b3set.c:1090-1111
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
        bsim3_temperature(self, temp, tnom);
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
        bsim3_load(self, mna, states, mode, gmin, noncon)
    }

    // truncate uses default (infinity) — actual LTE truncation happens in transient.rs via qcap_offsets()

    fn conductances(&self) -> Vec<(&str, f64)> {
        // Match ngspice b3ld.c trace: [gm, gds, gmbs, vgs, vds, vbs, ids, vth, cggb, cgdb, cgsb, cdgb, cddb, cdsb, cbgb, cbdb, cbsb, capbd, capbs, cgdo, cgso]
        vec![
            ("gm", self.gm), ("gds", self.gds), ("gmbs", self.gmbs),
            ("vgs", self.last_vgs), ("vds", self.last_vds), ("vbs", self.last_vbs),
            ("ids", self.cd), ("von", self.von),
            ("cggb", self.cggb), ("cgdb", self.cgdb), ("cgsb", self.cgsb),
            ("cdgb", self.cdgb), ("cddb", self.cddb), ("cdsb", self.cdsb),
            ("cbgb", self.cbgb), ("cbdb", self.cbdb), ("cbsb", self.cbsb),
            ("capbd", self.capbd), ("capbs", self.capbs),
            ("cgdo", self.inst_cgdo_op), ("cgso", self.inst_cgso_op),
        ]
    }

    fn limited_voltages(&self) -> Vec<(&str, f64)> {
        vec![]
    }

    fn ac_load(&mut self, mna: &mut MnaSystem, _states: &StateVectors, omega: f64) -> Result<(), SimError> {
        bsim3_ac_load(self, mna, omega);
        Ok(())
    }
}

// ========================================================================
// Main load function — port of b3ld.c
// ========================================================================
fn bsim3_load(
    dev: &mut Bsim3,
    mna: &mut MnaSystem,
    states: &mut StateVectors,
    mode: Mode,
    gmin: f64,
    noncon: &mut bool,
) -> Result<(), SimError> {
    // Clone model/param to avoid borrow conflicts (bsim3_ids needs &mut dev)
    let model = dev.model.clone();
    let p = dev.param.clone();
    let base = dev.state_offset;
    let mos_type = model.mos_type as f64;

    let (d, g, s, b, dp, sp) = (dev.d_node, dev.g_node, dev.s_node, dev.b_node, dev.dp_node, dev.sp_node);

    let scaling_factor = 1.0e-9;
    let charge_computation_needed =
        mode.is(MODEDCTRANCURVE) || mode.is(MODEAC) || mode.is(MODETRAN) || mode.is(MODEINITSMSIG)
        || (mode.is(MODETRANOP) && mode.is(MODEUIC));

    let mut check = true;
    #[allow(unused_assignments)]
    let mut bypass = false;
    let mut vbs: f64;
    let mut vgs: f64;
    let mut vds: f64;
    let mut qdef: f64 = 0.0;
    let mut qgate: f64 = 0.0;
    let mut qbulk: f64 = 0.0;
    let mut qdrn: f64 = 0.0;
    let mut cdrain: f64;

    // Voltage initialization
    if mode.is(MODEINITSMSIG) {
        vbs = states.get(0, base + ST_VBS);
        vgs = states.get(0, base + ST_VGS);
        vds = states.get(0, base + ST_VDS);
        qdef = states.get(0, base + ST_QDEF);
    } else if mode.is(MODEINITTRAN) {
        vbs = states.get(1, base + ST_VBS);
        vgs = states.get(1, base + ST_VGS);
        vds = states.get(1, base + ST_VDS);
        qdef = states.get(1, base + ST_QDEF);
    } else if mode.is(MODEINITJCT) && !dev.off {
        vds = mos_type * 0.0;
        vgs = mos_type * 0.0;
        vbs = mos_type * 0.0;
        qdef = 0.0;
        if vds == 0.0 && vgs == 0.0 && vbs == 0.0
           && (mode.is(MODETRAN | MODEAC | MODEDCOP | MODEDCTRANCURVE)
               || !mode.is(MODEUIC))
        {
            vbs = 0.0;
            vgs = mos_type * dev.inst_vth0 + 0.1;
            vds = 0.1;
        }
    } else if mode.is(MODEINITJCT | MODEINITFIX) && dev.off {
        qdef = 0.0; vbs = 0.0; vgs = 0.0; vds = 0.0;
    } else {
        // Normal iteration or predictor
        if mode.is(MODEINITPRED) {
            let xfact = dev.delta / dev.delta_old1;
            states.set(0, base + ST_VBS, states.get(1, base + ST_VBS));
            vbs = (1.0 + xfact) * states.get(1, base + ST_VBS)
                - xfact * states.get(2, base + ST_VBS);
            states.set(0, base + ST_VGS, states.get(1, base + ST_VGS));
            vgs = (1.0 + xfact) * states.get(1, base + ST_VGS)
                - xfact * states.get(2, base + ST_VGS);
            states.set(0, base + ST_VDS, states.get(1, base + ST_VDS));
            vds = (1.0 + xfact) * states.get(1, base + ST_VDS)
                - xfact * states.get(2, base + ST_VDS);
            states.set(0, base + ST_VBD,
                states.get(0, base + ST_VBS) - states.get(0, base + ST_VDS));
            states.set(0, base + ST_QDEF, states.get(1, base + ST_QDEF));
            qdef = (1.0 + xfact) * states.get(1, base + ST_QDEF)
                 - xfact * states.get(2, base + ST_QDEF);
        } else {
            vbs = mos_type * (mna.rhs_old_val(b) - mna.rhs_old_val(sp));
            vgs = mos_type * (mna.rhs_old_val(g) - mna.rhs_old_val(sp));
            vds = mos_type * (mna.rhs_old_val(dp) - mna.rhs_old_val(sp));
            qdef = mos_type * mna.rhs_old_val(0); // qNode = 0 for non-NQS
        }

        let mut vbd = vbs - vds;
        let vgd = vgs - vds;
        let vgdo = states.get(0, base + ST_VGS) - states.get(0, base + ST_VDS);
        let delvbs = vbs - states.get(0, base + ST_VBS);
        let delvbd = vbd - states.get(0, base + ST_VBD);
        let delvgs = vgs - states.get(0, base + ST_VGS);
        let delvds = vds - states.get(0, base + ST_VDS);
        let delvgd = vgd - vgdo;

        // cdhat/cbhat for bypass (skipped for now)

        // Voltage limiting
        let von = dev.von;
        if states.get(0, base + ST_VDS) >= 0.0 {
            vgs = dev_fetlim(vgs, states.get(0, base + ST_VGS), von);
            vds = vgs - vgd;
            vds = dev_limvds(vds, states.get(0, base + ST_VDS));
            let vgd = vgs - vds;
        } else {
            let mut vgd = dev_fetlim(vgd, vgdo, von);
            vds = vgs - vgd;
            vds = -dev_limvds(-vds, -states.get(0, base + ST_VDS));
            vgs = vgd + vds;
        }

        if vds >= 0.0 {
            vbs = crate::device::limiting::pnjlim(vbs, states.get(0, base + ST_VBS),
                                                    CONSTVT0, model.vcrit, &mut check);
            vbd = vbs - vds;
        } else {
            vbd = crate::device::limiting::pnjlim(vbd, states.get(0, base + ST_VBD),
                                                    CONSTVT0, model.vcrit, &mut check);
            vbs = vbd + vds;
        }
    }

    // determine DC current and derivatives
    let vbd = vbs - vds;
    let vgd = vgs - vds;
    let vgb = vgs - vbs;

    // Source/drain junction diode DC model (gmin = CKTgmin, passed from solver)
    let nvtm = model.vtm * model.jct_emission_coeff;
    let source_sat_current = 1.0e-14_f64; // area=0, perim=0
    let drain_sat_current = 1.0e-14_f64;

    // Source junction
    if source_sat_current <= 0.0 {
        dev.gbs = gmin;
        dev.cbs = dev.gbs * vbs;
    } else {
        if model.ijth == 0.0 {
            let evbs = (vbs / nvtm).exp();
            dev.gbs = source_sat_current * evbs / nvtm + gmin;
            dev.cbs = source_sat_current * (evbs - 1.0) + gmin * vbs;
        } else {
            if vbs < dev.vjsm {
                let evbs = (vbs / nvtm).exp();
                dev.gbs = source_sat_current * evbs / nvtm + gmin;
                dev.cbs = source_sat_current * (evbs - 1.0) + gmin * vbs;
            } else {
                let t0 = dev.is_evjsm / nvtm;
                dev.gbs = t0 + gmin;
                dev.cbs = dev.is_evjsm - source_sat_current
                         + t0 * (vbs - dev.vjsm) + gmin * vbs;
            }
        }
    }

    // Drain junction
    if drain_sat_current <= 0.0 {
        dev.gbd = gmin;
        dev.cbd = dev.gbd * vbd;
    } else {
        if model.ijth == 0.0 {
            let evbd = (vbd / nvtm).exp();
            dev.gbd = drain_sat_current * evbd / nvtm + gmin;
            dev.cbd = drain_sat_current * (evbd - 1.0) + gmin * vbd;
        } else {
            if vbd < dev.vjdm {
                let evbd = (vbd / nvtm).exp();
                dev.gbd = drain_sat_current * evbd / nvtm + gmin;
                dev.cbd = drain_sat_current * (evbd - 1.0) + gmin * vbd;
            } else {
                let t0 = dev.is_evjdm / nvtm;
                dev.gbd = t0 + gmin;
                dev.cbd = dev.is_evjdm - drain_sat_current
                         + t0 * (vbd - dev.vjdm) + gmin * vbd;
            }
        }
    }

    // Mode determination
    let (big_vds, big_vgs, big_vbs);
    if vds >= 0.0 {
        dev.mode = 1;
        big_vds = vds; big_vgs = vgs; big_vbs = vbs;
    } else {
        dev.mode = -1;
        big_vds = -vds; big_vgs = vgd; big_vbs = vbd;
    }

    // ========================================================================
    // BSIM3 Ids equations — port of b3ld.c lines 500-1250
    // ========================================================================
    // Call the Ids computation
    let (ids, gm_val, gds_val, gmb_val, isub, gbg, gbd_sub, gbb) =
        bsim3_ids(dev, &model, &p, big_vds, big_vgs, big_vbs, charge_computation_needed);

    cdrain = ids;
    dev.gds = gds_val;
    dev.gm = gm_val;
    dev.gmbs = gmb_val;
    dev.gbbs = gbb;
    dev.gbgs = gbg;
    dev.gbds = gbd_sub;
    dev.csub = isub;
    dev.cd = cdrain;

    // ========================================================================
    // Charge computation — port of b3ld.c lines 1258-2434
    // ========================================================================
    if model.xpart < 0.0 || !charge_computation_needed {
        qgate = 0.0; qdrn = 0.0; qbulk = 0.0;
        dev.cggb = 0.0; dev.cgsb = 0.0; dev.cgdb = 0.0;
        dev.cdgb = 0.0; dev.cdsb = 0.0; dev.cddb = 0.0;
        dev.cbgb = 0.0; dev.cbsb = 0.0; dev.cbdb = 0.0;
        dev.cqdb = 0.0; dev.cqsb = 0.0; dev.cqgb = 0.0; dev.cqbb = 0.0;
        dev.gtau = 0.0;
    } else {
        // capMod=3 (CTM) charge model — port of b3ld.c lines 1957-2242
        // For capMod 0/1/2, we also need to handle them, but capMod=3 is the default
        let d_vgs_eff_d_vg = dev.d_vgs_eff_d_vg;
        let d_vth_d_vb = dev.d_vth_d_vb;
        let d_vth_d_vd = dev.d_vth_d_vd;
        let d_vbseff_d_vb = dev.d_vbseff_d_vb;
        let vth = dev.von; // von = Vth after computation
        let cox = model.cox;
        let vtm = model.vtm;

        // Separate VgsteffCV with noff and voffcv (b3ld.c:1601-1631)
        let noff = dev.n_ids * p.noff;
        let dnoff_dvd = p.noff * dev.dn_dvd;
        let dnoff_dvb = p.noff * dev.dn_dvb;
        let t0_cv = vtm * noff;
        let voffcv = p.voffcv;
        let vgst_nvt = (dev.vgst - voffcv) / t0_cv;

        let (vgsteff, d_vgsteff_d_vg, d_vgsteff_d_vd, d_vgsteff_d_vb);
        if vgst_nvt > EXP_THRESHOLD {
            vgsteff = dev.vgst - voffcv;
            d_vgsteff_d_vg = d_vgs_eff_d_vg;
            d_vgsteff_d_vd = -d_vth_d_vd;
            d_vgsteff_d_vb = -d_vth_d_vb;
        } else if vgst_nvt < -EXP_THRESHOLD {
            vgsteff = t0_cv * (1.0 + MIN_EXP).ln();
            d_vgsteff_d_vg = 0.0;
            let t_d = vgsteff / noff;
            d_vgsteff_d_vb = t_d * dnoff_dvb;
            d_vgsteff_d_vd = t_d * dnoff_dvd;
        } else {
            let exp_vgst = vgst_nvt.exp();
            vgsteff = t0_cv * (1.0 + exp_vgst).ln();
            let t_d = exp_vgst / (1.0 + exp_vgst);
            d_vgsteff_d_vd = -t_d * (d_vth_d_vd + (dev.vgst - voffcv)
                             / noff * dnoff_dvd) + vgsteff / noff * dnoff_dvd;
            d_vgsteff_d_vb = -t_d * (d_vth_d_vb + (dev.vgst - voffcv)
                             / noff * dnoff_dvb) + vgsteff / noff * dnoff_dvb;
            d_vgsteff_d_vg = t_d * d_vgs_eff_d_vg;
        }

        // VbseffCV for charge model (b3ld.c:1588-1595)
        // Uses Vbseff from the Ids computation, NOT a fresh computation
        let (vbseff_cv, d_vbseff_cv_d_vb) = if dev.vbseff_ids < 0.0 {
            (dev.vbseff_ids, 1.0)
        } else {
            (p.phi - dev.phis, -dev.d_phis_d_vb)
        };

        // Vgs_eff for charge (same as Ids, re-use)
        let vgs_eff = if p.ngate > 1.0e18 && p.ngate < 1.0e25 && big_vgs > (dev.inst_vfb + p.phi) {
            let t0_pg = dev.inst_vfb + p.phi;
            let t1 = 1.0e6 * CHARGE_Q * EPSSI * p.ngate / (cox * cox);
            let t4 = (1.0 + 2.0 * (big_vgs - t0_pg) / t1).sqrt();
            let t2 = t1 * (t4 - 1.0);
            let t3 = 0.5 * t2 * t2 / t1;
            let t7 = 1.12 - t3 - 0.05;
            let t6 = (t7 * t7 + 0.224).sqrt();
            let t5 = 1.12 - 0.5 * (t7 + t6);
            big_vgs - t5
        } else {
            big_vgs
        };

        let cox_wl = cox * p.weff_cv * p.leff_cv;

        // capMod=3 (CTM) — b3ld.c:1958-2242
        if model.cap_mod == 3 {
            // Vfbeff
            let v3 = dev.inst_vfbzb - vgs_eff + vbseff_cv - DELTA_3;
            let (t0_fb, t2_fb) = if dev.inst_vfbzb <= 0.0 {
                let t0 = (v3 * v3 - 4.0 * DELTA_3 * dev.inst_vfbzb).sqrt();
                (t0, -DELTA_3 / t0)
            } else {
                let t0 = (v3 * v3 + 4.0 * DELTA_3 * dev.inst_vfbzb).sqrt();
                (t0, DELTA_3 / t0)
            };
            let t1 = 0.5 * (1.0 + v3 / t0_fb);
            let vfbeff = dev.inst_vfbzb - 0.5 * (v3 + t0_fb);
            let d_vfbeff_d_vg = t1 * d_vgs_eff_d_vg;
            let d_vfbeff_d_vb = -t1 * d_vbseff_cv_d_vb;

            // Tox-related
            let tox = 1.0e8 * model.tox;
            let t0 = (vgs_eff - vbseff_cv - dev.inst_vfbzb) / tox;
            let d_t0_d_vg = d_vgs_eff_d_vg / tox;
            let d_t0_d_vb = -d_vbseff_cv_d_vb / tox;

            let tmp = t0 * p.acde;
            let (tcen, d_tcen_d_vg_1, d_tcen_d_vb_1) = if tmp > -EXP_THRESHOLD && tmp < EXP_THRESHOLD {
                let tc = p.ldeb * tmp.exp();
                let dtc = p.acde * tc;
                (tc, dtc * d_t0_d_vg, dtc * d_t0_d_vb)
            } else if tmp <= -EXP_THRESHOLD {
                (p.ldeb * MIN_EXP, 0.0, 0.0)
            } else {
                (p.ldeb * MAX_EXP, 0.0, 0.0)
            };

            let link = 1.0e-3 * model.tox;
            let v3a = p.ldeb - tcen - link;
            let v4 = (v3a * v3a + 4.0 * link * p.ldeb).sqrt();
            let tcen = p.ldeb - 0.5 * (v3a + v4);
            let t1 = 0.5 * (1.0 + v3a / v4);
            let d_tcen_d_vg = d_tcen_d_vg_1 * t1;
            let d_tcen_d_vb = d_tcen_d_vb_1 * t1;

            let ccen = EPSSI / tcen;
            let t2 = cox / (cox + ccen);
            let coxeff = t2 * ccen;
            let t3 = -ccen / tcen;
            let d_coxeff_d_vg_base = t2 * t2 * t3;
            let d_coxeff_d_vb = d_coxeff_d_vg_base * d_tcen_d_vb;
            let d_coxeff_d_vg = d_coxeff_d_vg_base * d_tcen_d_vg;
            let cox_wlcen = cox_wl * coxeff / cox;

            // Qac0
            let qac0 = cox_wlcen * (vfbeff - dev.inst_vfbzb);
            let qov_cox = qac0 / coxeff;
            let d_qac0_d_vg = cox_wlcen * d_vfbeff_d_vg + qov_cox * d_coxeff_d_vg;
            let d_qac0_d_vb = cox_wlcen * d_vfbeff_d_vb + qov_cox * d_coxeff_d_vb;

            // Qsub0
            let t0 = 0.5 * p.k1ox;
            let t3 = vgs_eff - vfbeff - vbseff_cv - vgsteff;
            let (t1, t2) = if p.k1ox == 0.0 {
                (0.0, 0.0)
            } else if t3 < 0.0 {
                (t0 + t3 / p.k1ox, cox_wlcen)
            } else {
                let t1 = (t0 * t0 + t3).sqrt();
                (t1, cox_wlcen * t0 / t1)
            };
            let qsub0 = cox_wlcen * p.k1ox * (t1 - t0);
            let qov_cox = qsub0 / coxeff;
            let d_qsub0_d_vg = t2 * (d_vgs_eff_d_vg - d_vfbeff_d_vg - d_vgsteff_d_vg)
                             + qov_cox * d_coxeff_d_vg;
            let d_qsub0_d_vd = -t2 * d_vgsteff_d_vd;
            let d_qsub0_d_vb = -t2 * (d_vfbeff_d_vb + d_vbseff_cv_d_vb + d_vgsteff_d_vb)
                             + qov_cox * d_coxeff_d_vb;

            // Gate-bias dependent delta Phis
            let vtm = model.vtm;
            let (denomi, t0_dp) = if p.k1ox <= 0.0 {
                (0.25 * p.moin * vtm, 0.5 * p.sqrt_phi)
            } else {
                (p.moin * vtm * p.k1ox * p.k1ox, p.k1ox * p.sqrt_phi)
            };
            let t1 = 2.0 * t0_dp + vgsteff;
            let delta_phi = vtm * (1.0 + t1 * vgsteff / denomi).ln();
            let d_delta_phi_d_vg = 2.0 * vtm * (t1 - t0_dp) / (denomi + t1 * vgsteff);

            // VgDP = Vgsteff - DeltaPhi
            let t0 = vgsteff - delta_phi - 0.001;
            let d_t0_d_vg = 1.0 - d_delta_phi_d_vg;
            let t1 = (t0 * t0 + vgsteff * 0.004).sqrt();
            let vg_dp = 0.5 * (t0 + t1);
            let d_vg_dp_d_vg = 0.5 * (d_t0_d_vg + (t0 * d_t0_d_vg + 0.002) / t1);

            // Second Coxeff computation for drain charge
            let t3 = 4.0 * (vth - dev.inst_vfbzb - p.phi);
            let tox2 = tox + tox; // Tox += Tox in C code
            let (t0_2, d_t0_2_d_vd, d_t0_2_d_vb) = if t3 >= 0.0 {
                ((vgsteff + t3) / tox2,
                 (d_vgsteff_d_vd + 4.0 * d_vth_d_vd) / tox2,
                 (d_vgsteff_d_vb + 4.0 * d_vth_d_vb) / tox2)
            } else {
                ((vgsteff + 1.0e-20) / tox2,
                 d_vgsteff_d_vd / tox2,
                 d_vgsteff_d_vb / tox2)
            };
            let tmp_exp = (0.7 * t0_2.ln()).exp(); // exp(0.7 * log(T0))
            let t1 = 1.0 + tmp_exp;
            let t2 = 0.7 * tmp_exp / (t0_2 * tox2);
            let tcen2 = 1.9e-9 / t1;
            let d_tcen2_d_vg_base = -1.9e-9 * t2 / t1 / t1;
            let d_tcen2_d_vd = tox2 * d_tcen2_d_vg_base * d_t0_2_d_vd;
            let d_tcen2_d_vb = tox2 * d_tcen2_d_vg_base * d_t0_2_d_vb;
            let d_tcen2_d_vg = d_tcen2_d_vg_base * d_vgsteff_d_vg;

            let ccen2 = EPSSI / tcen2;
            let t0 = cox / (cox + ccen2);
            let coxeff2 = t0 * ccen2;
            let t1 = -ccen2 / tcen2;
            let d_coxeff2_d_vg_base = t0 * t0 * t1;
            let d_coxeff2_d_vd = d_coxeff2_d_vg_base * d_tcen2_d_vd;
            let d_coxeff2_d_vb = d_coxeff2_d_vg_base * d_tcen2_d_vb;
            let d_coxeff2_d_vg = d_coxeff2_d_vg_base * d_tcen2_d_vg;
            let cox_wlcen2 = cox_wl * coxeff2 / cox;

            // AbulkCV
            let abulk_cv = dev.abulk0 * p.abulk_cv_factor;
            let d_abulk_cv_d_vb = p.abulk_cv_factor * dev.d_abulk0_d_vb;
            let vdsat_cv = vg_dp / abulk_cv;

            // VdseffCV
            let t0 = vdsat_cv - big_vds - DELTA_4;
            let d_t0_d_vg = d_vg_dp_d_vg / abulk_cv;
            let d_t0_d_vb = -vdsat_cv * d_abulk_cv_d_vb / abulk_cv;
            let t1 = (t0 * t0 + 4.0 * DELTA_4 * vdsat_cv).sqrt();
            let d_t1_d_vg_base = (t0 + DELTA_4 + DELTA_4) / t1;
            let d_t1_d_vd_base = -t0 / t1;
            let d_t1_d_vb = d_t1_d_vg_base * d_t0_d_vb;
            let d_t1_d_vg = d_t1_d_vg_base * d_t0_d_vg;
            let d_t1_d_vd = d_t1_d_vd_base;
            let (vdseff_cv, d_vdseff_cv_d_vg, d_vdseff_cv_d_vd, d_vdseff_cv_d_vb) = if t0 >= 0.0 {
                (vdsat_cv - 0.5 * (t0 + t1),
                 0.5 * (d_t0_d_vg - d_t1_d_vg),
                 0.5 * (1.0 - d_t1_d_vd),
                 0.5 * (d_t0_d_vb - d_t1_d_vb))
            } else {
                let t3 = (DELTA_4 + DELTA_4) / (t1 - t0);
                let t4 = 1.0 - t3;
                let t5 = vdsat_cv * t3 / (t1 - t0);
                (vdsat_cv * t4,
                 d_t0_d_vg * t4 + t5 * (d_t1_d_vg - d_t0_d_vg),
                 t5 * (d_t1_d_vd + 1.0),
                 d_t0_d_vb * (1.0 - t5) + t5 * d_t1_d_vb)
            };
            let (vdseff_cv, d_vdseff_cv_d_vg, d_vdseff_cv_d_vb) = if big_vds == 0.0 {
                (0.0, 0.0, 0.0)
            } else {
                (vdseff_cv, d_vdseff_cv_d_vg, d_vdseff_cv_d_vb)
            };

            // Main charge computation
            let t0 = abulk_cv * vdseff_cv;
            let t1 = vg_dp;
            let t2 = 12.0 * (t1 - 0.5 * t0 + 1.0e-20);
            let t3 = t0 / t2;
            let t4 = 1.0 - 12.0 * t3 * t3;
            let t5 = abulk_cv * (6.0 * t0 * (4.0 * t1 - t0) / (t2 * t2) - 0.5);
            let t6 = t5 * vdseff_cv / abulk_cv;

            let qinoi = cox_wlcen2 * (t1 - t0 * (0.5 - t3));
            qgate = qinoi;
            let qov_cox = qgate / coxeff2;
            let cgg1 = cox_wlcen2 * (t4 * d_vg_dp_d_vg + t5 * d_vdseff_cv_d_vg);
            let cgd1 = cox_wlcen2 * t5 * d_vdseff_cv_d_vd + cgg1 * d_vgsteff_d_vd
                     + qov_cox * d_coxeff2_d_vd;
            let cgb1 = cox_wlcen2 * (t5 * d_vdseff_cv_d_vb + t6 * d_abulk_cv_d_vb)
                     + cgg1 * d_vgsteff_d_vb + qov_cox * d_coxeff2_d_vb;
            let cgg1 = cgg1 * d_vgsteff_d_vg + qov_cox * d_coxeff2_d_vg;

            // qbulk
            let t7 = 1.0 - abulk_cv;
            let t8 = t2 * t2;
            let t9 = 12.0 * t7 * t0 * t0 / (t8 * abulk_cv);
            let t10 = t9 * d_vg_dp_d_vg;
            let t11 = -t7 * t5 / abulk_cv;
            let t12 = -(t9 * t1 / abulk_cv + vdseff_cv * (0.5 - t0 / t2));

            qbulk = cox_wlcen2 * t7 * (0.5 * vdseff_cv - t0 * vdseff_cv / t2);
            let qov_cox = qbulk / coxeff2;
            let cbg1 = cox_wlcen2 * (t10 + t11 * d_vdseff_cv_d_vg);
            let cbd1 = cox_wlcen2 * t11 * d_vdseff_cv_d_vd + cbg1 * d_vgsteff_d_vd
                     + qov_cox * d_coxeff2_d_vd;
            let cbb1 = cox_wlcen2 * (t11 * d_vdseff_cv_d_vb + t12 * d_abulk_cv_d_vb)
                     + cbg1 * d_vgsteff_d_vb + qov_cox * d_coxeff2_d_vb;
            let cbg1 = cbg1 * d_vgsteff_d_vg + qov_cox * d_coxeff2_d_vg;

            // qsrc partition (50/50 for xpart=0.5, default xpart=0)
            let (qsrc, csg, csd, csb) = if model.xpart > 0.5 {
                // 0/100 partition
                let t2x = t2 + t2; // T2 += T2 in C code
                let t3x = t2x * t2x;
                let t7 = -(0.25 - 12.0 * t0 * (4.0 * t1 - t0) / t3x);
                let t4 = -(0.5 + 24.0 * t0 * t0 / t3x) * d_vg_dp_d_vg;
                let t5 = t7 * abulk_cv;
                let t6 = t7 * vdseff_cv;
                let qsrc = -cox_wlcen2 * (t1 / 2.0 + t0 / 4.0 - 0.5 * t0 * t0 / t2);
                let qov_cox = qsrc / coxeff2;
                let csg = cox_wlcen2 * (t4 + t5 * d_vdseff_cv_d_vg);
                let csd = cox_wlcen2 * t5 * d_vdseff_cv_d_vd + csg * d_vgsteff_d_vd
                        + qov_cox * d_coxeff2_d_vd;
                let csb = cox_wlcen2 * (t5 * d_vdseff_cv_d_vb + t6 * d_abulk_cv_d_vb)
                        + csg * d_vgsteff_d_vb + qov_cox * d_coxeff2_d_vb;
                let csg = csg * d_vgsteff_d_vg + qov_cox * d_coxeff2_d_vg;
                (qsrc, csg, csd, csb)
            } else if model.xpart < 0.5 {
                // 40/60 partition
                let t2x = t2 / 12.0;
                let t3x = 0.5 * cox_wlcen2 / (t2x * t2x);
                let t4 = t1 * (2.0 * t0 * t0 / 3.0 + t1 * (t1 - 4.0 * t0 / 3.0))
                       - 2.0 * t0 * t0 * t0 / 15.0;
                let qsrc = -t3x * t4;
                let qov_cox = qsrc / coxeff2;
                let t8 = 4.0 / 3.0 * t1 * (t1 - t0) + 0.4 * t0 * t0;
                let t5 = -2.0 * qsrc / t2x - t3x * (t1 * (3.0 * t1 - 8.0 * t0 / 3.0)
                       + 2.0 * t0 * t0 / 3.0);
                let t6 = abulk_cv * (qsrc / t2x + t3x * t8);
                let t7 = t6 * vdseff_cv / abulk_cv;
                let csg = t5 * d_vg_dp_d_vg + t6 * d_vdseff_cv_d_vg;
                let csd = csg * d_vgsteff_d_vd + t6 * d_vdseff_cv_d_vd
                        + qov_cox * d_coxeff2_d_vd;
                let csb = csg * d_vgsteff_d_vb + t6 * d_vdseff_cv_d_vb
                        + t7 * d_abulk_cv_d_vb + qov_cox * d_coxeff2_d_vb;
                let csg = csg * d_vgsteff_d_vg + qov_cox * d_coxeff2_d_vg;
                (qsrc, csg, csd, csb)
            } else {
                // 50/50 partition
                (-0.5 * qgate, -0.5 * cgg1, -0.5 * cgd1, -0.5 * cgb1)
            };

            // Final charge adjustments
            qgate = qgate + qac0 + qsub0 - qbulk;
            qbulk = qbulk - (qac0 + qsub0);
            qdrn = -(qgate + qbulk + qsrc);

            let cbg = cbg1 - d_qac0_d_vg - d_qsub0_d_vg;
            let cbd = cbd1 - d_qsub0_d_vd;
            let cbb = cbb1 - d_qac0_d_vb - d_qsub0_d_vb;

            let cgg = cgg1 - cbg;
            let cgd = cgd1 - cbd;
            let cgb = cgb1 - cbb;

            let cgb = cgb * d_vbseff_d_vb;
            let cbb = cbb * d_vbseff_d_vb;
            let csb = csb * d_vbseff_d_vb;

            dev.cggb = cgg;
            dev.cgsb = -(cgg + cgd + cgb);
            dev.cgdb = cgd;
            dev.cdgb = -(cgg + cbg + csg);
            dev.cdsb = cgg + cgd + cgb + cbg + cbd + cbb + csg + csd + csb;
            dev.cddb = -(cgd + cbd + csd);
            dev.cbgb = cbg;
            dev.cbsb = -(cbg + cbd + cbb);
            dev.cbdb = cbd;
            dev.qinv = -qinoi;
        } else {
            // capMod=0 simplified — zero charges for now
            // TODO: port capMod 0, 1, 2
            qgate = 0.0; qdrn = 0.0; qbulk = 0.0;
            dev.cggb = 0.0; dev.cgsb = 0.0; dev.cgdb = 0.0;
            dev.cdgb = 0.0; dev.cdsb = 0.0; dev.cddb = 0.0;
            dev.cbgb = 0.0; dev.cbsb = 0.0; dev.cbdb = 0.0;
        }
        dev.cqdb = 0.0; dev.cqsb = 0.0; dev.cqgb = 0.0; dev.cqbb = 0.0;
        dev.gtau = 0.0;
    }

    // Junction capacitance (b3ld.c:2256-2434)
    // For area=0, perim=0 (no junction caps): capbd=capbs=0
    dev.capbd = 0.0;
    dev.capbs = 0.0;
    if charge_computation_needed {
        let czbd = model.unit_area_temp_jct_cap * dev.drain_area;
        let czbs = model.unit_area_temp_jct_cap * dev.source_area;
        let (czbdsw, czbdswg) = if dev.drain_perimeter < p.weff {
            (0.0, model.unit_length_gate_sidewall_temp_jct_cap * dev.drain_perimeter)
        } else {
            (model.unit_length_sidewall_temp_jct_cap * (dev.drain_perimeter - p.weff),
             model.unit_length_gate_sidewall_temp_jct_cap * p.weff)
        };
        let (czbssw, czbsswg) = if dev.source_perimeter < p.weff {
            (0.0, model.unit_length_gate_sidewall_temp_jct_cap * dev.source_perimeter)
        } else {
            (model.unit_length_sidewall_temp_jct_cap * (dev.source_perimeter - p.weff),
             model.unit_length_gate_sidewall_temp_jct_cap * p.weff)
        };

        let mj = model.bulk_jct_bot_grading_coeff;
        let mjsw = model.bulk_jct_side_grading_coeff;
        let mjswg = model.bulk_jct_gate_side_grading_coeff;

        // Source junction cap
        if vbs == 0.0 {
            states.set(0, base + ST_QBS, 0.0);
            dev.capbs = czbs + czbssw + czbsswg;
        } else if vbs < 0.0 {
            let mut qbs_val = 0.0;
            let mut capbs_val = 0.0;
            if czbs > 0.0 {
                let arg = 1.0 - vbs / model.phi_b;
                let sarg = if mj == 0.5 { 1.0 / arg.sqrt() } else { (-mj * arg.ln()).exp() };
                qbs_val += model.phi_b * czbs * (1.0 - arg * sarg) / (1.0 - mj);
                capbs_val += czbs * sarg;
            }
            if czbssw > 0.0 {
                let arg = 1.0 - vbs / model.phi_bsw;
                let sarg = if mjsw == 0.5 { 1.0 / arg.sqrt() } else { (-mjsw * arg.ln()).exp() };
                qbs_val += model.phi_bsw * czbssw * (1.0 - arg * sarg) / (1.0 - mjsw);
                capbs_val += czbssw * sarg;
            }
            if czbsswg > 0.0 {
                let arg = 1.0 - vbs / model.phi_bswg;
                let sarg = if mjswg == 0.5 { 1.0 / arg.sqrt() } else { (-mjswg * arg.ln()).exp() };
                qbs_val += model.phi_bswg * czbsswg * (1.0 - arg * sarg) / (1.0 - mjswg);
                capbs_val += czbsswg * sarg;
            }
            states.set(0, base + ST_QBS, qbs_val);
            dev.capbs = capbs_val;
        } else {
            let t0 = czbs + czbssw + czbsswg;
            let t1 = vbs * (czbs * mj / model.phi_b + czbssw * mjsw / model.phi_bsw
                   + czbsswg * mjswg / model.phi_bswg);
            states.set(0, base + ST_QBS, vbs * (t0 + 0.5 * t1));
            dev.capbs = t0 + t1;
        }

        // Drain junction cap
        if vbd == 0.0 {
            states.set(0, base + ST_QBD, 0.0);
            dev.capbd = czbd + czbdsw + czbdswg;
        } else if vbd < 0.0 {
            let mut qbd_val = 0.0;
            let mut capbd_val = 0.0;
            if czbd > 0.0 {
                let arg = 1.0 - vbd / model.phi_b;
                let sarg = if mj == 0.5 { 1.0 / arg.sqrt() } else { (-mj * arg.ln()).exp() };
                qbd_val += model.phi_b * czbd * (1.0 - arg * sarg) / (1.0 - mj);
                capbd_val += czbd * sarg;
            }
            if czbdsw > 0.0 {
                let arg = 1.0 - vbd / model.phi_bsw;
                let sarg = if mjsw == 0.5 { 1.0 / arg.sqrt() } else { (-mjsw * arg.ln()).exp() };
                qbd_val += model.phi_bsw * czbdsw * (1.0 - arg * sarg) / (1.0 - mjsw);
                capbd_val += czbdsw * sarg;
            }
            if czbdswg > 0.0 {
                let arg = 1.0 - vbd / model.phi_bswg;
                let sarg = if mjswg == 0.5 { 1.0 / arg.sqrt() } else { (-mjswg * arg.ln()).exp() };
                qbd_val += model.phi_bswg * czbdswg * (1.0 - arg * sarg) / (1.0 - mjswg);
                capbd_val += czbdswg * sarg;
            }
            states.set(0, base + ST_QBD, qbd_val);
            dev.capbd = capbd_val;
        } else {
            let t0 = czbd + czbdsw + czbdswg;
            let t1 = vbd * (czbd * mj / model.phi_b + czbdsw * mjsw / model.phi_bsw
                   + czbdswg * mjswg / model.phi_bswg);
            states.set(0, base + ST_QBD, vbd * (t0 + 0.5 * t1));
            dev.capbd = t0 + t1;
        }
    }

    dev.qgate = qgate;
    dev.qbulk = qbulk;
    dev.qdrn = qdrn;

    // Convergence check
    if !dev.off || !mode.is(MODEINITFIX) {
        if check {
            *noncon = true;
        }
    }

    // Store state
    states.set(0, base + ST_VBS, vbs);
    states.set(0, base + ST_VBD, vbd);
    states.set(0, base + ST_VGS, vgs);
    states.set(0, base + ST_VDS, vds);
    states.set(0, base + ST_QDEF, qdef);

    // Store for trace comparison
    dev.last_vgs = vgs;
    dev.last_vds = vds;
    dev.last_vbs = vbs;

    // ========================================================================
    // Overlap caps + gc** coefficients + integration — b3ld.c:2475-2870
    // ========================================================================
    let mut ceqqg = 0.0;
    let mut ceqqb = 0.0;
    let mut ceqqd = 0.0;
    let mut cqcheq = 0.0;
    let mut cqdef = 0.0;
    let (mut gcdgb, mut gcddb, mut gcdsb) = (0.0, 0.0, 0.0);
    let (mut gcsgb, mut gcsdb, mut gcssb) = (0.0, 0.0, 0.0);
    let (mut gcggb, mut gcgdb, mut gcgsb) = (0.0, 0.0, 0.0);
    let (mut gcbgb, mut gcbdb, mut gcbsb) = (0.0, 0.0, 0.0);
    let mut gqdef = 0.0;
    let (mut gcqgb, mut gcqdb, mut gcqsb, mut gcqbb) = (0.0, 0.0, 0.0, 0.0);
    let (mut ggtg, mut ggtd, mut ggtb, mut ggts) = (0.0, 0.0, 0.0, 0.0);
    let mut dxpart = if dev.mode > 0 { 0.4 } else { 0.6 };
    let mut sxpart = 1.0 - dxpart;
    let (mut ddxpart_dvd, mut ddxpart_dvg, mut ddxpart_dvb, mut ddxpart_dvs) = (0.0, 0.0, 0.0, 0.0);
    let (mut dsxpart_dvd, mut dsxpart_dvg, mut dsxpart_dvb, mut dsxpart_dvs) = (0.0, 0.0, 0.0, 0.0);
    dev.gtau = 0.0;

    // b3ld.c:2827-2895 — skip, zero, or integrate depending on mode
    let do_integration = charge_computation_needed
        && !mode.is(MODEDCTRANCURVE)  // DC sweep: no integration
        && !mode.is(MODEINITSMSIG);   // small-signal: skip

    if do_integration {
        // Overlap cap computation — port of b3ld.c:2500-2557
        let (cgdo, qgdo, cgso, qgso);
        if model.cap_mod == 0 {
            cgdo = p.cgdo; qgdo = p.cgdo * vgd;
            cgso = p.cgso; qgso = p.cgso * vgs;
        } else if model.cap_mod == 1 {
            if vgd < 0.0 {
                let t1 = (1.0 - 4.0 * vgd / p.ckappa).sqrt();
                cgdo = p.cgdo + p.weff_cv * p.cgdl / t1;
                qgdo = p.cgdo * vgd - p.weff_cv * 0.5 * p.cgdl * p.ckappa * (t1 - 1.0);
            } else {
                cgdo = p.cgdo + p.weff_cv * p.cgdl;
                qgdo = (p.weff_cv * p.cgdl + p.cgdo) * vgd;
            }
            if vgs < 0.0 {
                let t1 = (1.0 - 4.0 * vgs / p.ckappa).sqrt();
                cgso = p.cgso + p.weff_cv * p.cgsl / t1;
                qgso = p.cgso * vgs - p.weff_cv * 0.5 * p.cgsl * p.ckappa * (t1 - 1.0);
            } else {
                cgso = p.cgso + p.weff_cv * p.cgsl;
                qgso = (p.weff_cv * p.cgsl + p.cgso) * vgs;
            }
        } else {
            // capMod >= 2 (including capMod=3)
            let t0 = vgd + DELTA_1;
            let t1 = (t0 * t0 + 4.0 * DELTA_1).sqrt();
            let t2 = 0.5 * (t0 - t1);
            let t3 = p.weff_cv * p.cgdl;
            let t4 = (1.0 - 4.0 * t2 / p.ckappa).sqrt();
            cgdo = p.cgdo + t3 - t3 * (1.0 - 1.0 / t4) * (0.5 - 0.5 * t0 / t1);
            qgdo = (p.cgdo + t3) * vgd - t3 * (t2 + 0.5 * p.ckappa * (t4 - 1.0));

            let t0 = vgs + DELTA_1;
            let t1 = (t0 * t0 + 4.0 * DELTA_1).sqrt();
            let t2 = 0.5 * (t0 - t1);
            let t3 = p.weff_cv * p.cgsl;
            let t4 = (1.0 - 4.0 * t2 / p.ckappa).sqrt();
            cgso = p.cgso + t3 - t3 * (1.0 - 1.0 / t4) * (0.5 - 0.5 * t0 / t1);
            qgso = (p.cgso + t3) * vgs - t3 * (t2 + 0.5 * p.ckappa * (t4 - 1.0));
        }

        dev.inst_cgdo_op = cgdo;
        dev.inst_cgso_op = cgso;

        let ag0 = dev.ag[0];


        // gc** coefficients — QS mode (nqsMod=0), b3ld.c:2563-2728
        if dev.mode > 0 {
            gcggb = (dev.cggb + cgdo + cgso + p.cgbo) * ag0;
            gcgdb = (dev.cgdb - cgdo) * ag0;
            gcgsb = (dev.cgsb - cgso) * ag0;
            gcdgb = (dev.cdgb - cgdo) * ag0;
            gcddb = (dev.cddb + dev.capbd + cgdo) * ag0;
            gcdsb = dev.cdsb * ag0;
            gcsgb = -(dev.cggb + dev.cbgb + dev.cdgb + cgso) * ag0;
            gcsdb = -(dev.cgdb + dev.cbdb + dev.cddb) * ag0;
            gcssb = (dev.capbs + cgso - (dev.cgsb + dev.cbsb + dev.cdsb)) * ag0;
            gcbgb = (dev.cbgb - p.cgbo) * ag0;
            gcbdb = (dev.cbdb - dev.capbd) * ag0;
            gcbsb = (dev.cbsb - dev.capbs) * ag0;

            let qgd = qgdo;
            let qgs = qgso;
            let qgb = p.cgbo * vgb;
            qgate += qgd + qgs + qgb;
            qbulk -= qgb;
            qdrn -= qgd;
            let _qsrc = -(qgate + qbulk + qdrn);

            ggtg = 0.0; ggtd = 0.0; ggtb = 0.0; ggts = 0.0;
            sxpart = 0.6; dxpart = 0.4;
        } else {
            gcggb = (dev.cggb + cgdo + cgso + p.cgbo) * ag0;
            gcgdb = (dev.cgsb - cgdo) * ag0;
            gcgsb = (dev.cgdb - cgso) * ag0;
            gcdgb = -(dev.cggb + dev.cbgb + dev.cdgb + cgdo) * ag0;
            gcddb = (dev.capbd + cgdo - (dev.cgsb + dev.cbsb + dev.cdsb)) * ag0;
            gcdsb = -(dev.cgdb + dev.cbdb + dev.cddb) * ag0;
            gcsgb = (dev.cdgb - cgso) * ag0;
            gcsdb = dev.cdsb * ag0;
            gcssb = (dev.cddb + dev.capbs + cgso) * ag0;
            gcbgb = (dev.cbgb - p.cgbo) * ag0;
            gcbdb = (dev.cbsb - dev.capbd) * ag0;
            gcbsb = (dev.cbdb - dev.capbs) * ag0;

            let qgd = qgdo;
            let qgs = qgso;
            let qgb = p.cgbo * vgb;
            qgate += qgd + qgs + qgb;
            qbulk -= qgb;
            let qsrc = qdrn - qgs;
            qdrn = -(qgate + qbulk + qsrc);

            ggtg = 0.0; ggtd = 0.0; ggtb = 0.0; ggts = 0.0;
            sxpart = 0.4; dxpart = 0.6;
        }

        // Store charges in state vectors (b3ld.c:2799-2804)
        // ngspice adjusts qd and qb by junction charges:
        //   qd = qdrn - qbd
        //   qb = qbulk + qbd + qbs
        states.set(0, base + ST_QG, qgate);
        states.set(0, base + ST_QD,
            qdrn - states.get(0, base + ST_QBD));
        states.set(0, base + ST_QB,
            qbulk + states.get(0, base + ST_QBD) + states.get(0, base + ST_QBS));

        // MODEINITTRAN: copy state0 → state1 (b3ld.c:2837-2849)
        if mode.is(MODEINITTRAN) {
            states.set(1, base + ST_QB, states.get(0, base + ST_QB));
            states.set(1, base + ST_QG, states.get(0, base + ST_QG));
            states.set(1, base + ST_QD, states.get(0, base + ST_QD));
        }

        // NI_integrate for qb, qg, qd (b3ld.c:2837-2843)
        // ngspice calls NIintegrate(ckt, &geq, &ceq, 0.0, here->BSIM3q*)
        // cap=0.0 so geq=0 and ceq is unused; only the state update matters.
        {
            use crate::integration::ni_integrate;
            ni_integrate(&dev.ag, states, 0.0, base + ST_QB, dev.order);
            ni_integrate(&dev.ag, states, 0.0, base + ST_QG, dev.order);
            ni_integrate(&dev.ag, states, 0.0, base + ST_QD, dev.order);
        }

        // line860: Charge current equivalents (b3ld.c:2898-2936)
        ceqqg = states.get(0, base + ST_CQG) - gcggb * vgb + gcgdb * vbd + gcgsb * vbs;
        ceqqb = states.get(0, base + ST_CQB) - gcbgb * vgb + gcbdb * vbd + gcbsb * vbs;
        ceqqd = states.get(0, base + ST_CQD) - gcdgb * vgb + gcddb * vbd + gcdsb * vbs;

        // MODEINITTRAN: copy integrated currents state0→state1 (b3ld.c:2919-2925)
        if mode.is(MODEINITTRAN) {
            states.set(1, base + ST_CQB, states.get(0, base + ST_CQB));
            states.set(1, base + ST_CQG, states.get(0, base + ST_CQG));
            states.set(1, base + ST_CQD, states.get(0, base + ST_CQD));
        }
    }
    // else: line850 — gc**/ceqq* remain zero (initialized above)

    // Current sources
    let (fwd_sum, rev_sum, cdreq, mut ceqbd, mut ceqbs);
    let (gbbdp, gbbsp, gbdpg, gbdpdp, gbdpb, gbdpsp, gbspg, gbspdp, gbspb, gbspsp);

    if dev.mode >= 0 {
        let gm = dev.gm;
        let gmbs = dev.gmbs;
        fwd_sum = gm + gmbs;
        rev_sum = 0.0;
        cdreq = mos_type * (cdrain - dev.gds * vds - gm * vgs - gmbs * vbs);
        ceqbd = -mos_type * (dev.csub - dev.gbds * vds - dev.gbgs * vgs - dev.gbbs * vbs);
        ceqbs = 0.0;
        gbbdp = -dev.gbds;
        gbbsp = dev.gbds + dev.gbgs + dev.gbbs;
        gbdpg = dev.gbgs; gbdpdp = dev.gbds; gbdpb = dev.gbbs;
        gbdpsp = -(gbdpg + gbdpdp + gbdpb);
        gbspg = 0.0; gbspdp = 0.0; gbspb = 0.0; gbspsp = 0.0;
    } else {
        let gm = -dev.gm;
        let gmbs = -dev.gmbs;
        fwd_sum = 0.0;
        rev_sum = -(gm + gmbs);
        cdreq = -mos_type * (cdrain + dev.gds * vds + gm * vgd + gmbs * vbd);
        ceqbs = -mos_type * (dev.csub + dev.gbds * vds - dev.gbgs * vgd - dev.gbbs * vbd);
        ceqbd = 0.0;
        gbbsp = -dev.gbds;
        gbbdp = dev.gbds + dev.gbgs + dev.gbbs;
        gbdpg = 0.0; gbdpsp = 0.0; gbdpb = 0.0; gbdpdp = 0.0;
        gbspg = dev.gbgs; gbspsp = dev.gbds; gbspb = dev.gbbs;
        gbspdp = -(gbspg + gbspsp + gbspb);
    }

    if model.mos_type > 0 {
        ceqbs += dev.cbs - dev.gbs * vbs;
        ceqbd += dev.cbd - dev.gbd * vbd;
    } else {
        ceqbs -= dev.cbs - dev.gbs * vbs;
        ceqbd -= dev.cbd - dev.gbd * vbd;
        ceqqg = -ceqqg;
        ceqqb = -ceqqb;
        ceqqd = -ceqqd;
        cqdef = -cqdef;
        cqcheq = -cqcheq;
    }

    let m = dev.m;


    // Load RHS
    mna.stamp_rhs(g,  -m * ceqqg);
    mna.stamp_rhs(b,  -m * (ceqbs + ceqbd + ceqqb));
    mna.stamp_rhs(dp,  m * (ceqbd - cdreq - ceqqd));
    mna.stamp_rhs(sp,  m * (cdreq + ceqbs + ceqqg + ceqqb + ceqqd));

    // Load Y matrix
    let t1 = qdef * dev.gtau;
    mna.stamp(d, d, m * dev.drain_conductance);
    mna.stamp(g, g, m * (gcggb - ggtg));
    mna.stamp(s, s, m * dev.source_conductance);
    mna.stamp(b, b, m * (dev.gbd + dev.gbs - gcbgb - gcbdb - gcbsb - dev.gbbs));
    mna.stamp(dp, dp, m * (dev.drain_conductance + dev.gds + dev.gbd
                            + rev_sum + gcddb + dxpart * ggtd
                            + t1 * ddxpart_dvd + gbdpdp));
    mna.stamp(sp, sp, m * (dev.source_conductance + dev.gds + dev.gbs
                            + fwd_sum + gcssb + sxpart * ggts
                            + t1 * dsxpart_dvs + gbspsp));
    mna.stamp(d, dp, -m * dev.drain_conductance);
    mna.stamp(g, b, -m * (gcggb + gcgdb + gcgsb + ggtb));
    mna.stamp(g, dp, m * (gcgdb - ggtd));
    mna.stamp(g, sp, m * (gcgsb - ggts));
    mna.stamp(s, sp, -m * dev.source_conductance);
    mna.stamp(b, g, m * (gcbgb - dev.gbgs));
    mna.stamp(b, dp, m * (gcbdb - dev.gbd + gbbdp));
    mna.stamp(b, sp, m * (gcbsb - dev.gbs + gbbsp));
    mna.stamp(dp, d, -m * dev.drain_conductance);
    mna.stamp(dp, g, m * (dev.gm + gcdgb + dxpart * ggtg + t1 * ddxpart_dvg + gbdpg));
    mna.stamp(dp, b, -m * (dev.gbd - dev.gmbs + gcdgb + gcddb + gcdsb
                            - dxpart * ggtb - t1 * ddxpart_dvb - gbdpb));
    mna.stamp(dp, sp, -m * (dev.gds + fwd_sum - gcdsb
                             - dxpart * ggts - t1 * ddxpart_dvs - gbdpsp));
    mna.stamp(sp, g, m * (gcsgb - dev.gm + sxpart * ggtg + t1 * dsxpart_dvg + gbspg));
    mna.stamp(sp, s, -m * dev.source_conductance);
    mna.stamp(sp, b, -m * (dev.gbs + dev.gmbs + gcsgb + gcsdb + gcssb
                            - sxpart * ggtb - t1 * dsxpart_dvb - gbspb));
    mna.stamp(sp, dp, -m * (dev.gds + rev_sum - gcsdb
                             - sxpart * ggtd - t1 * dsxpart_dvd - gbspdp));

    Ok(())
}

// ========================================================================
// Ids computation — extracted from b3ld.c lines 500-1250
// Returns (Ids, Gm, Gds, Gmb, Isub, Gbg, Gbd, Gbb)
// ========================================================================
fn bsim3_ids(
    dev: &mut Bsim3,
    model: &Bsim3Model,
    p: &Bsim3SizeDepParam,
    vds: f64, vgs: f64, vbs: f64,
    charge_needed: bool,
) -> (f64, f64, f64, f64, f64, f64, f64, f64) {
    let vtm = model.vtm;
    let leff = p.leff;

    // Vbseff
    let t0 = vbs - p.vbsc - 0.001;
    let t1 = (t0 * t0 - 0.004 * p.vbsc).sqrt();
    let mut vbseff = p.vbsc + 0.5 * (t0 + t1);
    let mut d_vbseff_d_vb = 0.5 * (1.0 + t0 / t1);
    if vbseff < vbs { vbseff = vbs; }

    let (phis, d_phis_d_vb, sqrt_phis, d_sqrt_phis_d_vb);
    if vbseff > 0.0 {
        let t0 = p.phi / (p.phi + vbseff);
        phis = p.phi * t0;
        d_phis_d_vb = -t0 * t0;
        sqrt_phis = p.phis3 / (p.phi + 0.5 * vbseff);
        d_sqrt_phis_d_vb = -0.5 * sqrt_phis * sqrt_phis / p.phis3;
    } else {
        phis = p.phi - vbseff;
        d_phis_d_vb = -1.0;
        sqrt_phis = phis.sqrt();
        d_sqrt_phis_d_vb = -0.5 / sqrt_phis;
    }

    let xdep = p.xdep0 * sqrt_phis / p.sqrt_phi;
    let d_xdep_d_vb = (p.xdep0 / p.sqrt_phi) * d_sqrt_phis_d_vb;

    // Vth calculation
    let t3 = xdep.sqrt();
    let v0 = p.vbi - p.phi;

    let t0 = p.dvt2 * vbseff;
    let (t1_vth, t2_vth) = if t0 >= -0.5 {
        (1.0 + t0, p.dvt2)
    } else {
        let t4 = 1.0 / (3.0 + 8.0 * t0);
        ((1.0 + 3.0 * t0) * t4, p.dvt2 * t4 * t4)
    };
    let lt1 = model.factor1 * t3 * t1_vth;
    let dlt1_dvb = model.factor1 * (0.5 / t3 * t1_vth * d_xdep_d_vb + t3 * t2_vth);

    let t0 = p.dvt2w * vbseff;
    let (t1_w, t2_w) = if t0 >= -0.5 {
        (1.0 + t0, p.dvt2w)
    } else {
        let t4 = 1.0 / (3.0 + 8.0 * t0);
        ((1.0 + 3.0 * t0) * t4, p.dvt2w * t4 * t4)
    };
    let ltw = model.factor1 * t3 * t1_w;
    let dltw_dvb = model.factor1 * (0.5 / t3 * t1_w * d_xdep_d_vb + t3 * t2_w);

    let t0 = -0.5 * p.dvt1 * leff / lt1;
    let (theta0, d_theta0_d_vb) = if t0 > -EXP_THRESHOLD {
        let t1 = t0.exp();
        let theta0 = t1 * (1.0 + 2.0 * t1);
        let dt1_dvb = -t0 / lt1 * t1 * dlt1_dvb;
        (theta0, (1.0 + 4.0 * t1) * dt1_dvb)
    } else {
        let t1 = MIN_EXP;
        (t1 * (1.0 + 2.0 * t1), 0.0)
    };

    dev.thetavth = p.dvt0 * theta0;
    let delt_vth = dev.thetavth * v0;
    let d_delt_vth_d_vb = p.dvt0 * d_theta0_d_vb * v0;

    let t0 = -0.5 * p.dvt1w * p.weff * leff / ltw;
    let (t2_w2, d_t2_w2_d_vb) = if t0 > -EXP_THRESHOLD {
        let t1 = t0.exp();
        let t2 = t1 * (1.0 + 2.0 * t1);
        let dt1_dvb = -t0 / ltw * t1 * dltw_dvb;
        (t2, (1.0 + 4.0 * t1) * dt1_dvb)
    } else {
        let t1 = MIN_EXP;
        (t1 * (1.0 + 2.0 * t1), 0.0)
    };

    let t0 = p.dvt0w * t2_w2;
    let t2_val = t0 * v0;
    let dt2_val_dvb = p.dvt0w * d_t2_w2_d_vb * v0;

    let temp_ratio = model.vtm / KBOQ / model.tnom - 1.0; // (Temp/Tnom - 1)
    let t0 = (1.0 + p.nlx / leff).sqrt();
    let t1 = p.k1ox * (t0 - 1.0) * p.sqrt_phi
           + (p.kt1 + p.kt1l / leff + p.kt2 * vbseff) * temp_ratio;
    let tmp2 = model.tox * p.phi / (p.weff + p.w0);

    let mut t3 = p.eta0 + p.etab * vbseff;
    let t4 = if t3 < 1.0e-4 {
        let t9 = 1.0 / (3.0 - 2.0e4 * t3);
        t3 = (2.0e-4 - t3) * t9;
        t9 * t9
    } else {
        1.0
    };
    let d_dibl_sft_d_vd = t3 * p.theta0vb0;
    let dibl_sft = d_dibl_sft_d_vd * vds;

    let vth = model.mos_type as f64 * dev.inst_vth0 - p.k1 * p.sqrt_phi
            + p.k1ox * sqrt_phis - p.k2ox * vbseff - delt_vth - t2_val
            + (p.k3 + p.k3b * vbseff) * tmp2 + t1 - dibl_sft;
    dev.von = vth;

    let d_vth_d_vb = p.k1ox * d_sqrt_phis_d_vb - p.k2ox
                   - d_delt_vth_d_vb - dt2_val_dvb + p.k3b * tmp2
                   - p.etab * vds * p.theta0vb0 * t4
                   + p.kt2 * temp_ratio;
    let d_vth_d_vd = -d_dibl_sft_d_vd;

    // n calculation
    let tmp2_n = p.nfactor * EPSSI / xdep;
    let tmp3_n = p.cdsc + p.cdscb * vbseff + p.cdscd * vds;
    let tmp4_n = (tmp2_n + tmp3_n * theta0 + p.cit) / model.cox;
    let (n, dn_dvb, dn_dvd) = if tmp4_n >= -0.5 {
        (1.0 + tmp4_n,
         (-tmp2_n / xdep * d_xdep_d_vb + tmp3_n * d_theta0_d_vb + p.cdscb * theta0) / model.cox,
         p.cdscd * theta0 / model.cox)
    } else {
        let t0 = 1.0 / (3.0 + 8.0 * tmp4_n);
        let nn = (1.0 + 3.0 * tmp4_n) * t0;
        let t0sq = t0 * t0;
        (nn,
         (-tmp2_n / xdep * d_xdep_d_vb + tmp3_n * d_theta0_d_vb + p.cdscb * theta0) / model.cox * t0sq,
         p.cdscd * theta0 / model.cox * t0sq)
    };

    // Poly gate Si depletion
    let t0_pg = dev.inst_vfb + p.phi;
    let (vgs_eff, d_vgs_eff_d_vg) = if p.ngate > 1.0e18 && p.ngate < 1.0e25 && vgs > t0_pg {
        let t1 = 1.0e6 * CHARGE_Q * EPSSI * p.ngate / (model.cox * model.cox);
        let t4 = (1.0 + 2.0 * (vgs - t0_pg) / t1).sqrt();
        let t2 = t1 * (t4 - 1.0);
        let t3 = 0.5 * t2 * t2 / t1;
        let t7 = 1.12 - t3 - 0.05;
        let t6 = (t7 * t7 + 0.224).sqrt();
        let t5 = 1.12 - 0.5 * (t7 + t6);
        (vgs - t5, 1.0 - (0.5 - 0.5 / t4) * (1.0 + t7 / t6))
    } else {
        (vgs, 1.0)
    };
    let vgst = vgs_eff - vth;

    // Effective Vgst (Vgsteff)
    let t10 = 2.0 * n * vtm;
    let vgst_nvt = vgst / t10;
    let exp_arg = (2.0 * p.voff - vgst) / t10;

    let (vgsteff, mut d_vgsteff_d_vg, mut d_vgsteff_d_vd, mut d_vgsteff_d_vb);
    if vgst_nvt > EXP_THRESHOLD {
        vgsteff = vgst;
        d_vgsteff_d_vg = d_vgs_eff_d_vg;
        d_vgsteff_d_vd = -d_vth_d_vd;
        d_vgsteff_d_vb = -d_vth_d_vb;
    } else if exp_arg > EXP_THRESHOLD {
        let t0 = (vgst - p.voff) / (n * vtm);
        let exp_vgst = t0.exp();
        vgsteff = vtm * p.cdep0 / model.cox * exp_vgst;
        d_vgsteff_d_vg = vgsteff / (n * vtm);
        d_vgsteff_d_vd = -d_vgsteff_d_vg * (d_vth_d_vd + t0 * vtm * dn_dvd);
        d_vgsteff_d_vb = -d_vgsteff_d_vg * (d_vth_d_vb + t0 * vtm * dn_dvb);
        d_vgsteff_d_vg *= d_vgs_eff_d_vg;
    } else {
        let exp_vgst = vgst_nvt.exp();
        let t1 = t10 * (1.0 + exp_vgst).ln();
        let dt1_dvg = exp_vgst / (1.0 + exp_vgst);
        let dt1_dvb = -dt1_dvg * (d_vth_d_vb + vgst / n * dn_dvb) + t1 / n * dn_dvb;
        let dt1_dvd = -dt1_dvg * (d_vth_d_vd + vgst / n * dn_dvd) + t1 / n * dn_dvd;

        let dt2_dvg = -model.cox / (vtm * p.cdep0) * exp_arg.exp();
        let t2 = 1.0 - t10 * dt2_dvg;
        let dt2_dvd = -dt2_dvg * (d_vth_d_vd - 2.0 * vtm * exp_arg * dn_dvd)
                     + (t2 - 1.0) / n * dn_dvd;
        let dt2_dvb = -dt2_dvg * (d_vth_d_vb - 2.0 * vtm * exp_arg * dn_dvb)
                     + (t2 - 1.0) / n * dn_dvb;

        vgsteff = t1 / t2;
        let t3 = t2 * t2;
        d_vgsteff_d_vg = (t2 * dt1_dvg - t1 * dt2_dvg) / t3 * d_vgs_eff_d_vg;
        d_vgsteff_d_vd = (t2 * dt1_dvd - t1 * dt2_dvd) / t3;
        d_vgsteff_d_vb = (t2 * dt1_dvb - t1 * dt2_dvb) / t3;
    }
    dev.vgsteff = vgsteff;

    // Weff, Rds
    let t9 = sqrt_phis - p.sqrt_phi;
    let mut weff = p.weff - 2.0 * (p.dwg * vgsteff + p.dwb * t9);
    let mut d_weff_d_vg = -2.0 * p.dwg;
    let mut d_weff_d_vb = -2.0 * p.dwb * d_sqrt_phis_d_vb;
    if weff < 2.0e-8 {
        let t0 = 1.0 / (6.0e-8 - 2.0 * weff);
        weff = 2.0e-8 * (4.0e-8 - weff) * t0;
        let t0_sq = t0 * t0 * 4.0e-16;
        d_weff_d_vg *= t0_sq;
        d_weff_d_vb *= t0_sq;
    }

    let t0_rds = p.prwg * vgsteff + p.prwb * t9;
    let (rds, d_rds_d_vg, d_rds_d_vb) = if t0_rds >= -0.9 {
        (p.rds0 * (1.0 + t0_rds),
         p.rds0 * p.prwg,
         p.rds0 * p.prwb * d_sqrt_phis_d_vb)
    } else {
        let t1 = 1.0 / (17.0 + 20.0 * t0_rds);
        let rds = p.rds0 * (0.8 + t0_rds) * t1;
        let t1sq = t1 * t1;
        (rds, p.rds0 * p.prwg * t1sq, p.rds0 * p.prwb * d_sqrt_phis_d_vb * t1sq)
    };
    dev.rds = rds;

    // Abulk
    let t1_ab = 0.5 * p.k1ox / sqrt_phis;
    let dt1_ab_dvb = -t1_ab / sqrt_phis * d_sqrt_phis_d_vb;
    let t9_ab = (p.xj * xdep).sqrt();
    let tmp1_ab = leff + 2.0 * t9_ab;
    let t5_ab = leff / tmp1_ab;
    let tmp2_ab = p.a0 * t5_ab;
    let tmp3_ab = p.weff + p.b1;
    let tmp4_ab = p.b0 / tmp3_ab;
    let t2_ab = tmp2_ab + tmp4_ab;
    let dt2_ab_dvb = -t9_ab / tmp1_ab / xdep * d_xdep_d_vb;
    let t6_ab = t5_ab * t5_ab;
    let t7_ab = t5_ab * t6_ab;

    let mut abulk0 = 1.0 + t1_ab * t2_ab;
    let mut d_abulk0_d_vb = t1_ab * tmp2_ab * dt2_ab_dvb + t2_ab * dt1_ab_dvb;

    let t8_ab = p.ags * p.a0 * t7_ab;
    let mut d_abulk_d_vg = -t1_ab * t8_ab;
    let mut abulk_val = abulk0 + d_abulk_d_vg * vgsteff;
    let mut d_abulk_d_vb = d_abulk0_d_vb - t8_ab * vgsteff * (dt1_ab_dvb + 3.0 * t1_ab * dt2_ab_dvb);

    if abulk0 < 0.1 {
        let t9 = 1.0 / (3.0 - 20.0 * abulk0);
        abulk0 = (0.2 - abulk0) * t9;
        d_abulk0_d_vb *= t9 * t9;
    }
    if abulk_val < 0.1 {
        let t9 = 1.0 / (3.0 - 20.0 * abulk_val);
        abulk_val = (0.2 - abulk_val) * t9;
        let t10 = t9 * t9;
        d_abulk_d_vb *= t10;
        d_abulk_d_vg *= t10;
    }
    dev.abulk = abulk_val;

    let t2_keta = p.keta * vbseff;
    let (t0_keta, dt0_keta_dvb) = if t2_keta >= -0.9 {
        let t0 = 1.0 / (1.0 + t2_keta);
        (t0, -p.keta * t0 * t0)
    } else {
        let t1 = 1.0 / (0.8 + t2_keta);
        ((17.0 + 20.0 * t2_keta) * t1, -p.keta * t1 * t1)
    };
    d_abulk_d_vg *= t0_keta;
    d_abulk_d_vb = d_abulk_d_vb * t0_keta + abulk_val * dt0_keta_dvb;
    d_abulk0_d_vb = d_abulk0_d_vb * t0_keta + abulk0 * dt0_keta_dvb;
    abulk_val *= t0_keta;
    abulk0 *= t0_keta;
    let abulk = abulk_val;

    // Mobility
    let (t5_mob, d_denomi_d_vg, d_denomi_d_vd, d_denomi_d_vb);
    if model.mob_mod == 1 {
        let t0 = vgsteff + vth + vth;
        let t2 = p.ua + p.uc * vbseff;
        let t3 = t0 / model.tox;
        t5_mob = t3 * (t2 + p.ub * t3);
        d_denomi_d_vg = (t2 + 2.0 * p.ub * t3) / model.tox;
        d_denomi_d_vd = d_denomi_d_vg * 2.0 * d_vth_d_vd;
        d_denomi_d_vb = d_denomi_d_vg * 2.0 * d_vth_d_vb + p.uc * t3;
    } else if model.mob_mod == 2 {
        t5_mob = vgsteff / model.tox * (p.ua + p.uc * vbseff + p.ub * vgsteff / model.tox);
        d_denomi_d_vg = (p.ua + p.uc * vbseff + 2.0 * p.ub * vgsteff / model.tox) / model.tox;
        d_denomi_d_vd = 0.0;
        d_denomi_d_vb = vgsteff * p.uc / model.tox;
    } else {
        let t0 = vgsteff + vth + vth;
        let t2 = 1.0 + p.uc * vbseff;
        let t3 = t0 / model.tox;
        let t4 = t3 * (p.ua + p.ub * t3);
        t5_mob = t4 * t2;
        d_denomi_d_vg = (p.ua + 2.0 * p.ub * t3) * t2 / model.tox;
        d_denomi_d_vd = d_denomi_d_vg * 2.0 * d_vth_d_vd;
        d_denomi_d_vb = d_denomi_d_vg * 2.0 * d_vth_d_vb + p.uc * t4;
    }

    let (denomi, mut d_denomi_d_vg_f, mut d_denomi_d_vd_f, mut d_denomi_d_vb_f);
    if t5_mob >= -0.8 {
        denomi = 1.0 + t5_mob;
        d_denomi_d_vg_f = d_denomi_d_vg;
        d_denomi_d_vd_f = d_denomi_d_vd;
        d_denomi_d_vb_f = d_denomi_d_vb;
    } else {
        let t9 = 1.0 / (7.0 + 10.0 * t5_mob);
        denomi = (0.6 + t5_mob) * t9;
        let t9sq = t9 * t9;
        d_denomi_d_vg_f = d_denomi_d_vg * t9sq;
        d_denomi_d_vd_f = d_denomi_d_vd * t9sq;
        d_denomi_d_vb_f = d_denomi_d_vb * t9sq;
    }

    let ueff = dev.inst_u0temp / denomi;
    dev.ueff = ueff;
    let t9 = -ueff / denomi;
    let dueff_dvg = t9 * d_denomi_d_vg_f;
    let dueff_dvd = t9 * d_denomi_d_vd_f;
    let dueff_dvb = t9 * d_denomi_d_vb_f;

    // Vdsat
    let wvcox = weff * p.vsattemp * model.cox;
    let wvcoxrds = wvcox * rds;
    let esat = 2.0 * p.vsattemp / ueff;
    let esat_l = esat * leff;
    let t0 = -esat_l / ueff;
    let desat_l_dvg = t0 * dueff_dvg;
    let desat_l_dvd = t0 * dueff_dvd;
    let desat_l_dvb = t0 * dueff_dvb;

    // Lambda
    let a1 = p.a1;
    let (lambda, d_lambda_d_vg) = if a1 == 0.0 {
        (p.a2, 0.0)
    } else if a1 > 0.0 {
        let t0 = 1.0 - p.a2;
        let t1 = t0 - p.a1 * vgsteff - 0.0001;
        let t2 = (t1 * t1 + 0.0004 * t0).sqrt();
        (p.a2 + t0 - 0.5 * (t1 + t2), 0.5 * p.a1 * (1.0 + t1 / t2))
    } else {
        let t1 = p.a2 + p.a1 * vgsteff - 0.0001;
        let t2 = (t1 * t1 + 0.0004 * p.a2).sqrt();
        (0.5 * (t1 + t2), 0.5 * p.a1 * (1.0 + t1 / t2))
    };

    let vgst2vtm = vgsteff + 2.0 * vtm;
    dev.above_vgst2vtm = abulk / vgst2vtm;

    let (tmp2_rds, tmp3_rds) = if rds > 0.0 {
        (d_rds_d_vg / rds + d_weff_d_vg / weff, d_rds_d_vb / rds + d_weff_d_vb / weff)
    } else {
        (d_weff_d_vg / weff, d_weff_d_vb / weff)
    };

    let (mut d_vdsat_d_vg, mut d_vdsat_d_vb, mut d_vdsat_d_vd, vdsat_val);

    if rds == 0.0 && lambda == 1.0 {
        let t0 = 1.0 / (abulk * esat_l + vgst2vtm);
        let t1 = t0 * t0;
        let t2 = vgst2vtm * t0;
        let t3 = esat_l * vgst2vtm;
        vdsat_val = t3 * t0;

        let dt0_dvg = -(abulk * desat_l_dvg + esat_l * d_abulk_d_vg + 1.0) * t1;
        let dt0_dvd = -(abulk * desat_l_dvd) * t1;
        let dt0_dvb = -(abulk * desat_l_dvb + d_abulk_d_vb * esat_l) * t1;

        d_vdsat_d_vg = t3 * dt0_dvg + t2 * desat_l_dvg + esat_l * t0;
        d_vdsat_d_vd = t3 * dt0_dvd + t2 * desat_l_dvd;
        d_vdsat_d_vb = t3 * dt0_dvb + t2 * desat_l_dvb;
    } else {
        let tmp1 = d_lambda_d_vg / (lambda * lambda);
        let t9 = abulk * wvcoxrds;
        let t8 = abulk * t9;
        let t7 = vgst2vtm * t9;
        let t6 = vgst2vtm * wvcoxrds;
        let t0 = 2.0 * abulk * (t9 - 1.0 + 1.0 / lambda);
        let dt0_dvg = 2.0 * (t8 * tmp2_rds - abulk * tmp1
                     + (2.0 * t9 + 1.0 / lambda - 1.0) * d_abulk_d_vg);
        let dt0_dvb = 2.0 * (t8 * (2.0 / abulk * d_abulk_d_vb + tmp3_rds)
                     + (1.0 / lambda - 1.0) * d_abulk_d_vb);
        let dt0_dvd = 0.0;

        let t1 = vgst2vtm * (2.0 / lambda - 1.0) + abulk * esat_l + 3.0 * t7;
        let dt1_dvg = (2.0 / lambda - 1.0) - 2.0 * vgst2vtm * tmp1
                    + abulk * desat_l_dvg + esat_l * d_abulk_d_vg
                    + 3.0 * (t9 + t7 * tmp2_rds + t6 * d_abulk_d_vg);
        let dt1_dvb = abulk * desat_l_dvb + esat_l * d_abulk_d_vb
                    + 3.0 * (t6 * d_abulk_d_vb + t7 * tmp3_rds);
        let dt1_dvd = abulk * desat_l_dvd;

        let t2 = vgst2vtm * (esat_l + 2.0 * t6);
        let dt2_dvg = esat_l + vgst2vtm * desat_l_dvg + t6 * (4.0 + 2.0 * vgst2vtm * tmp2_rds);
        let dt2_dvb = vgst2vtm * (desat_l_dvb + 2.0 * t6 * tmp3_rds);
        let dt2_dvd = vgst2vtm * desat_l_dvd;

        let t3 = (t1 * t1 - 2.0 * t0 * t2).sqrt();
        vdsat_val = (t1 - t3) / t0;

        d_vdsat_d_vg = (dt1_dvg - (t1 * dt1_dvg - dt0_dvg * t2 - t0 * dt2_dvg) / t3
                       - vdsat_val * dt0_dvg) / t0;
        d_vdsat_d_vb = (dt1_dvb - (t1 * dt1_dvb - dt0_dvb * t2 - t0 * dt2_dvb) / t3
                       - vdsat_val * dt0_dvb) / t0;
        d_vdsat_d_vd = (dt1_dvd - (t1 * dt1_dvd - t0 * dt2_dvd) / t3) / t0;
    }
    dev.vdsat = vdsat_val;

    // tmp1 = dLambda_dVg / (Lambda^2) — needed for Vasat derivatives (b3ld.c:918, used at 1004)
    let tmp1 = d_lambda_d_vg / (lambda * lambda);

    // Vdseff
    let t1 = vdsat_val - vds - p.delta;
    let t2 = (t1 * t1 + 4.0 * p.delta * vdsat_val).sqrt();
    let t0 = t1 / t2;
    let t3 = 2.0 * p.delta / t2;

    let mut vdseff = vdsat_val - 0.5 * (t1 + t2);
    let mut d_vdseff_d_vg = d_vdsat_d_vg - 0.5 * (d_vdsat_d_vg + t0 * d_vdsat_d_vg + t3 * d_vdsat_d_vg);
    let mut d_vdseff_d_vd = d_vdsat_d_vd - 0.5 * ((d_vdsat_d_vd - 1.0) + t0 * (d_vdsat_d_vd - 1.0) + t3 * d_vdsat_d_vd);
    let mut d_vdseff_d_vb = d_vdsat_d_vb - 0.5 * (d_vdsat_d_vb + t0 * d_vdsat_d_vb + t3 * d_vdsat_d_vb);

    if vds == 0.0 {
        vdseff = 0.0;
        d_vdseff_d_vg = 0.0;
        d_vdseff_d_vb = 0.0;
    }

    // Vasat — b3ld.c:987-1010
    let tmp4_va = 1.0 - 0.5 * abulk * vdsat_val / vgst2vtm;
    let t9_va = wvcoxrds * vgsteff;
    let t8_va = t9_va / vgst2vtm;
    let t0_va = esat_l + vdsat_val + 2.0 * t9_va * tmp4_va;
    let t7_va = 2.0 * wvcoxrds * tmp4_va;

    let dt0_va_dvg = desat_l_dvg + d_vdsat_d_vg + t7_va * (1.0 + tmp2_rds * vgsteff)
                   - t8_va * (abulk * d_vdsat_d_vg - abulk * vdsat_val / vgst2vtm
                   + vdsat_val * d_abulk_d_vg);
    let dt0_va_dvb = desat_l_dvb + d_vdsat_d_vb + t7_va * tmp3_rds * vgsteff
                   - t8_va * (d_abulk_d_vb * vdsat_val + abulk * d_vdsat_d_vb);
    let dt0_va_dvd = desat_l_dvd + d_vdsat_d_vd - t8_va * abulk * d_vdsat_d_vd;

    let t9_va2 = wvcoxrds * abulk;
    let t1_va = 2.0 / lambda - 1.0 + t9_va2;
    let dt1_va_dvg = -2.0 * tmp1 + wvcoxrds * (abulk * tmp2_rds + d_abulk_d_vg);
    let dt1_va_dvb = d_abulk_d_vb * wvcoxrds + t9_va2 * tmp3_rds;

    let vasat = t0_va / t1_va;
    let dvasat_dvg = (dt0_va_dvg - vasat * dt1_va_dvg) / t1_va;
    let dvasat_dvb = (dt0_va_dvb - vasat * dt1_va_dvb) / t1_va;
    let dvasat_dvd = dt0_va_dvd / t1_va;

    if vdseff > vds { vdseff = vds; }
    let diff_vds = vds - vdseff;
    dev.vdseff = vdseff;

    // VACLM — b3ld.c:1017-1040
    let (vaclm, dvaclm_dvg, dvaclm_dvd, dvaclm_dvb);
    if p.pclm > 0.0 && diff_vds > 1.0e-10 {
        let t0 = 1.0 / (p.pclm * abulk * p.litl);
        let dt0_dvb = -t0 / abulk * d_abulk_d_vb;
        let dt0_dvg = -t0 / abulk * d_abulk_d_vg;

        let t2 = vgsteff / esat_l;
        let t1 = leff * (abulk + t2);
        let dt1_dvg = leff * ((1.0 - t2 * desat_l_dvg) / esat_l + d_abulk_d_vg);
        let dt1_dvb = leff * (d_abulk_d_vb - t2 * desat_l_dvb / esat_l);
        let dt1_dvd = -t2 * desat_l_dvd / esat;

        let t9 = t0 * t1;
        vaclm = t9 * diff_vds;
        dvaclm_dvg = t0 * dt1_dvg * diff_vds - t9 * d_vdseff_d_vg
                   + t1 * diff_vds * dt0_dvg;
        dvaclm_dvb = (dt0_dvb * t1 + t0 * dt1_dvb) * diff_vds
                   - t9 * d_vdseff_d_vb;
        dvaclm_dvd = t0 * dt1_dvd * diff_vds + t9 * (1.0 - d_vdseff_d_vd);
    } else {
        vaclm = MAX_EXP;
        dvaclm_dvg = 0.0; dvaclm_dvd = 0.0; dvaclm_dvb = 0.0;
    }

    // VADIBL — b3ld.c:1042-1086
    let (mut vadibl, mut dvadibl_dvg, mut dvadibl_dvd, mut dvadibl_dvb);
    if p.theta_rout > 0.0 {
        let t8 = abulk * vdsat_val;
        let t0 = vgst2vtm * t8;
        let dt0_dvg = vgst2vtm * abulk * d_vdsat_d_vg + t8
                    + vgst2vtm * vdsat_val * d_abulk_d_vg;
        let dt0_dvb = vgst2vtm * (d_abulk_d_vb * vdsat_val + abulk * d_vdsat_d_vb);
        let dt0_dvd = vgst2vtm * abulk * d_vdsat_d_vd;

        let t1 = vgst2vtm + t8;
        let dt1_dvg = 1.0 + abulk * d_vdsat_d_vg + vdsat_val * d_abulk_d_vg;
        let dt1_dvb = abulk * d_vdsat_d_vb + d_abulk_d_vb * vdsat_val;
        let dt1_dvd = abulk * d_vdsat_d_vd;

        let t9 = t1 * t1;
        let t2 = p.theta_rout;
        vadibl = (vgst2vtm - t0 / t1) / t2;
        dvadibl_dvg = (1.0 - dt0_dvg / t1 + t0 * dt1_dvg / t9) / t2;
        dvadibl_dvb = (-dt0_dvb / t1 + t0 * dt1_dvb / t9) / t2;
        dvadibl_dvd = (-dt0_dvd / t1 + t0 * dt1_dvd / t9) / t2;

        let t7 = p.pdiblb * vbseff;
        if t7 >= -0.9 {
            let t3 = 1.0 / (1.0 + t7);
            vadibl *= t3;
            dvadibl_dvg *= t3;
            dvadibl_dvb = (dvadibl_dvb - vadibl * p.pdiblb) * t3;
            dvadibl_dvd *= t3;
        } else {
            let t4 = 1.0 / (0.8 + t7);
            let t3 = (17.0 + 20.0 * t7) * t4;
            dvadibl_dvg *= t3;
            dvadibl_dvb = dvadibl_dvb * t3
                        - vadibl * p.pdiblb * t4 * t4;
            dvadibl_dvd *= t3;
            vadibl *= t3;
        }
    } else {
        vadibl = MAX_EXP;
        dvadibl_dvg = 0.0; dvadibl_dvd = 0.0; dvadibl_dvb = 0.0;
    }

    // VA — b3ld.c:1088-1122
    let t8_pvag = p.pvag / esat_l;
    let t9_pvag = t8_pvag * vgsteff;
    let (t0_pvag, dt0_pvag_dvg, dt0_pvag_dvb, dt0_pvag_dvd);
    if t9_pvag > -0.9 {
        t0_pvag = 1.0 + t9_pvag;
        dt0_pvag_dvg = t8_pvag * (1.0 - vgsteff * desat_l_dvg / esat_l);
        dt0_pvag_dvb = -t9_pvag * desat_l_dvb / esat_l;
        dt0_pvag_dvd = -t9_pvag * desat_l_dvd / esat_l;
    } else {
        let t1 = 1.0 / (17.0 + 20.0 * t9_pvag);
        t0_pvag = (0.8 + t9_pvag) * t1;
        let t1sq = t1 * t1;
        dt0_pvag_dvg = t8_pvag * (1.0 - vgsteff * desat_l_dvg / esat_l) * t1sq;
        let t9_2 = t9_pvag * t1sq / esat_l;
        dt0_pvag_dvb = -t9_2 * desat_l_dvb;
        dt0_pvag_dvd = -t9_2 * desat_l_dvd;
    }

    let tmp1_va = vaclm * vaclm;
    let tmp2_va = vadibl * vadibl;
    let tmp3_va = vaclm + vadibl;

    let t1_va2 = vaclm * vadibl / tmp3_va;
    let tmp3_va_sq = tmp3_va * tmp3_va;
    let dt1_va_dvg = (tmp1_va * dvadibl_dvg + tmp2_va * dvaclm_dvg) / tmp3_va_sq;
    let dt1_va_dvd = (tmp1_va * dvadibl_dvd + tmp2_va * dvaclm_dvd) / tmp3_va_sq;
    let dt1_va_dvb = (tmp1_va * dvadibl_dvb + tmp2_va * dvaclm_dvb) / tmp3_va_sq;

    let va = vasat + t0_pvag * t1_va2;
    let dva_dvg = dvasat_dvg + t1_va2 * dt0_pvag_dvg + t0_pvag * dt1_va_dvg;
    let dva_dvd = dvasat_dvd + t1_va2 * dt0_pvag_dvd + t0_pvag * dt1_va_dvd;
    let dva_dvb = dvasat_dvb + t1_va2 * dt0_pvag_dvb + t0_pvag * dt1_va_dvb;

    // VASCBE — b3ld.c:1124-1143
    let (vascbe, dvascbe_dvg, dvascbe_dvd, dvascbe_dvb);
    if p.pscbe2 > 0.0 {
        if diff_vds > p.pscbe1 * p.litl / EXP_THRESHOLD {
            let t0 = p.pscbe1 * p.litl / diff_vds;
            vascbe = leff * t0.exp() / p.pscbe2;
            let t1 = t0 * vascbe / diff_vds;
            dvascbe_dvg = t1 * d_vdseff_d_vg;
            dvascbe_dvd = -t1 * (1.0 - d_vdseff_d_vd);
            dvascbe_dvb = t1 * d_vdseff_d_vb;
        } else {
            vascbe = MAX_EXP * leff / p.pscbe2;
            dvascbe_dvg = 0.0; dvascbe_dvd = 0.0; dvascbe_dvb = 0.0;
        }
    } else {
        vascbe = MAX_EXP;
        dvascbe_dvg = 0.0; dvascbe_dvd = 0.0; dvascbe_dvb = 0.0;
    }

    // Ids calculation — b3ld.c:1145-1210
    let cox_w_ov_l = model.cox * weff / leff;
    let beta = ueff * cox_w_ov_l;
    let dbeta_dvg = cox_w_ov_l * dueff_dvg + beta * d_weff_d_vg / weff;
    let dbeta_dvd = cox_w_ov_l * dueff_dvd;
    let dbeta_dvb = cox_w_ov_l * dueff_dvb + beta * d_weff_d_vb / weff;

    let t0 = 1.0 - 0.5 * abulk * vdseff / vgst2vtm;
    let dt0_dvg = -0.5 * (abulk * d_vdseff_d_vg
                - abulk * vdseff / vgst2vtm + vdseff * d_abulk_d_vg) / vgst2vtm;
    let dt0_dvd = -0.5 * abulk * d_vdseff_d_vd / vgst2vtm;
    let dt0_dvb = -0.5 * (abulk * d_vdseff_d_vb + d_abulk_d_vb * vdseff)
                / vgst2vtm;

    let fgche1 = vgsteff * t0;
    let dfgche1_dvg = vgsteff * dt0_dvg + t0;
    let dfgche1_dvd = vgsteff * dt0_dvd;
    let dfgche1_dvb = vgsteff * dt0_dvb;

    let t9 = vdseff / esat_l;
    let fgche2 = 1.0 + t9;
    let dfgche2_dvg = (d_vdseff_d_vg - t9 * desat_l_dvg) / esat_l;
    let dfgche2_dvd = (d_vdseff_d_vd - t9 * desat_l_dvd) / esat_l;
    let dfgche2_dvb = (d_vdseff_d_vb - t9 * desat_l_dvb) / esat_l;

    let gche = beta * fgche1 / fgche2;
    let dgche_dvg = (beta * dfgche1_dvg + fgche1 * dbeta_dvg
                  - gche * dfgche2_dvg) / fgche2;
    let dgche_dvd = (beta * dfgche1_dvd + fgche1 * dbeta_dvd
                  - gche * dfgche2_dvd) / fgche2;
    let dgche_dvb = (beta * dfgche1_dvb + fgche1 * dbeta_dvb
                  - gche * dfgche2_dvb) / fgche2;

    let t0_ids = 1.0 + gche * rds;
    let t9_ids = vdseff / t0_ids;
    let idl = gche * t9_ids;

    let didl_dvg = (gche * d_vdseff_d_vg + t9_ids * dgche_dvg) / t0_ids
                 - idl * gche / t0_ids * d_rds_d_vg;
    let didl_dvd = (gche * d_vdseff_d_vd + t9_ids * dgche_dvd) / t0_ids;
    let didl_dvb = (gche * d_vdseff_d_vb + t9_ids * dgche_dvb
                 - idl * d_rds_d_vb * gche) / t0_ids;

    let t9_diff = diff_vds / va;
    let t0_va2 = 1.0 + t9_diff;
    let idsa = idl * t0_va2;
    let didsa_dvg = t0_va2 * didl_dvg - idl * (d_vdseff_d_vg + t9_diff * dva_dvg) / va;
    let didsa_dvd = t0_va2 * didl_dvd + idl * (1.0 - d_vdseff_d_vd
                  - t9_diff * dva_dvd) / va;
    let didsa_dvb = t0_va2 * didl_dvb - idl * (d_vdseff_d_vb + t9_diff * dva_dvb) / va;

    let t9_scbe = diff_vds / vascbe;
    let t0_scbe = 1.0 + t9_scbe;
    let ids = idsa * t0_scbe;

    let mut gm = t0_scbe * didsa_dvg - idsa * (d_vdseff_d_vg + t9_scbe * dvascbe_dvg) / vascbe;
    let mut gds = t0_scbe * didsa_dvd + idsa * (1.0 - d_vdseff_d_vd
                - t9_scbe * dvascbe_dvd) / vascbe;
    let mut gmb = t0_scbe * didsa_dvb - idsa * (d_vdseff_d_vb
                + t9_scbe * dvascbe_dvb) / vascbe;

    gds += gm * d_vgsteff_d_vd;
    gmb += gm * d_vgsteff_d_vb;
    gm *= d_vgsteff_d_vg;
    gmb *= d_vbseff_d_vb;

    // Substrate current
    let tmp = p.alpha0 + p.alpha1 * leff;
    let (isub, gbg, gbd_s, gbb);
    if tmp <= 0.0 || p.beta0 <= 0.0 {
        isub = 0.0; gbg = 0.0; gbd_s = 0.0; gbb = 0.0;
    } else {
        let t2 = tmp / leff;
        let (t1, dt1_dvg, dt1_dvd, dt1_dvb);
        if diff_vds > p.beta0 / EXP_THRESHOLD {
            let t0 = -p.beta0 / diff_vds;
            t1 = t2 * diff_vds * t0.exp();
            let t3 = t1 / diff_vds * (t0 - 1.0);
            dt1_dvg = t3 * d_vdseff_d_vg;
            dt1_dvd = t3 * (d_vdseff_d_vd - 1.0);
            dt1_dvb = t3 * d_vdseff_d_vb;
        } else {
            let t3 = t2 * MIN_EXP;
            t1 = t3 * diff_vds;
            dt1_dvg = -t3 * d_vdseff_d_vg;
            dt1_dvd = t3 * (1.0 - d_vdseff_d_vd);
            dt1_dvb = -t3 * d_vdseff_d_vb;
        }
        isub = t1 * idsa;
        let mut gbg_val = t1 * didsa_dvg + idsa * dt1_dvg;
        let mut gbd_val = t1 * didsa_dvd + idsa * dt1_dvd;
        let mut gbb_val = t1 * didsa_dvb + idsa * dt1_dvb;
        gbd_val += gbg_val * d_vgsteff_d_vd;
        gbb_val += gbg_val * d_vgsteff_d_vb;
        gbg_val *= d_vgsteff_d_vg;
        gbb_val *= d_vbseff_d_vb;
        gbg = gbg_val; gbd_s = gbd_val; gbb = gbb_val;
    }

    // Save intermediate derivatives needed by charge model
    dev.vbseff_ids = vbseff;
    dev.d_vbseff_d_vb = d_vbseff_d_vb;
    dev.d_vgs_eff_d_vg = d_vgs_eff_d_vg;
    dev.d_vth_d_vb = d_vth_d_vb;
    dev.d_vth_d_vd = d_vth_d_vd;
    dev.d_vgsteff_d_vg = d_vgsteff_d_vg;
    dev.d_vgsteff_d_vd = d_vgsteff_d_vd;
    dev.d_vgsteff_d_vb = d_vgsteff_d_vb;
    dev.n_ids = n;
    dev.dn_dvb = dn_dvb;
    dev.dn_dvd = dn_dvd;
    dev.vgst = vgst;
    dev.abulk0 = abulk0;
    dev.d_abulk0_d_vb = d_abulk0_d_vb;
    dev.sqrt_phis = sqrt_phis;
    dev.d_sqrt_phis_d_vb = d_sqrt_phis_d_vb;
    dev.phis = phis;
    dev.d_phis_d_vb = d_phis_d_vb;

    (ids, gm, gds, gmb, isub, gbg, gbd_s, gbb)
}

// ========================================================================
// AC load — port of b3acld.c (simplified for QS mode, non-NQS)
// ========================================================================
fn bsim3_ac_load(dev: &Bsim3, mna: &mut MnaSystem, omega: f64) {
    let (d, g, s, b, dp, sp) = (dev.d_node, dev.g_node, dev.s_node, dev.b_node, dev.dp_node, dev.sp_node);
    let m = dev.m;

    let csd = -(dev.cddb + dev.cgdb + dev.cbdb);
    let csg = -(dev.cdgb + dev.cggb + dev.cbgb);
    let css = -(dev.cdsb + dev.cgsb + dev.cbsb);

    let (gm, gmbs, gds_ac);
    let (cggb, cgdb, cgsb, cbgb, cbdb, cbsb, cdgb, cddb, cdsb);
    let (fwd_sum, rev_sum);
    let (gbbdp, gbbsp, gbdpg, gbdpdp, gbdpb, gbdpsp, gbspg, gbspdp, gbspb, gbspsp);

    // QS mode
    gm = dev.gm; gmbs = dev.gmbs; gds_ac = dev.gds;
    cggb = dev.cggb; cgdb = dev.cgdb; cgsb = dev.cgsb;
    cbgb = dev.cbgb; cbdb = dev.cbdb; cbsb = dev.cbsb;
    cdgb = dev.cdgb; cddb = dev.cddb; cdsb = dev.cdsb;

    if dev.mode >= 0 {
        fwd_sum = gm + gmbs;
        rev_sum = 0.0;
        gbbdp = -dev.gbds;
        gbbsp = dev.gbds + dev.gbgs + dev.gbbs;
        gbdpg = dev.gbgs; gbdpdp = dev.gbds; gbdpb = dev.gbbs;
        gbdpsp = -(gbdpg + gbdpdp + gbdpb);
        gbspg = 0.0; gbspdp = 0.0; gbspb = 0.0; gbspsp = 0.0;
    } else {
        fwd_sum = 0.0;
        rev_sum = -(gm + gmbs);
        gbbsp = -dev.gbds;
        gbbdp = dev.gbds + dev.gbgs + dev.gbbs;
        gbdpg = 0.0; gbdpsp = 0.0; gbdpb = 0.0; gbdpdp = 0.0;
        gbspg = dev.gbgs; gbspsp = dev.gbds; gbspb = dev.gbbs;
        gbspdp = -(gbspg + gbspsp + gbspb);
    }

    let gdpr = dev.drain_conductance;
    let gspr = dev.source_conductance;
    let gbd_ac = dev.gbd;
    let gbs_ac = dev.gbs;
    let capbd = dev.capbd;
    let capbs = dev.capbs;

    let gs_overlap_cap = dev.inst_cgso;
    let gd_overlap_cap = dev.inst_cgdo;
    let gb_overlap_cap = dev.param.cgbo;

    let xcdgb = (cdgb - gd_overlap_cap) * omega;
    let xcddb = (cddb + capbd + gd_overlap_cap) * omega;
    let xcdsb = cdsb * omega;
    let xcsgb = -(cggb + cbgb + cdgb + gs_overlap_cap) * omega;
    let xcsdb = -(cgdb + cbdb + cddb) * omega;
    let xcssb = (capbs + gs_overlap_cap - (cgsb + cbsb + cdsb)) * omega;
    let xcggb = (cggb + gd_overlap_cap + gs_overlap_cap + gb_overlap_cap) * omega;
    let xcgdb = (cgdb - gd_overlap_cap) * omega;
    let xcgsb = (cgsb - gs_overlap_cap) * omega;
    let xcbgb = (cbgb - gb_overlap_cap) * omega;
    let xcbdb = (cbdb - capbd) * omega;
    let xcbsb = (cbsb - capbs) * omega;

    // Stamp real parts
    mna.stamp(d, d, m * gdpr);
    mna.stamp(s, s, m * gspr);
    mna.stamp(b, b, m * (gbd_ac + gbs_ac - dev.gbbs));
    mna.stamp(dp, dp, m * (gdpr + gds_ac + gbd_ac + rev_sum + gbdpdp));
    mna.stamp(sp, sp, m * (gspr + gds_ac + gbs_ac + fwd_sum + gbspsp));
    mna.stamp(d, dp, m * (-gdpr));
    mna.stamp(s, sp, m * (-gspr));
    mna.stamp(b, g, m * (-dev.gbgs));
    mna.stamp(b, dp, m * (-gbd_ac + gbbdp));
    mna.stamp(b, sp, m * (-gbs_ac + gbbsp));
    mna.stamp(dp, d, m * (-gdpr));
    mna.stamp(dp, g, m * (gm + gbdpg));
    mna.stamp(dp, b, m * (-gbd_ac + gmbs + gbdpb));
    mna.stamp(dp, sp, m * (-gds_ac - fwd_sum + gbdpsp));
    mna.stamp(sp, g, m * (-gm + gbspg));
    mna.stamp(sp, s, m * (-gspr));
    mna.stamp(sp, b, m * (-gbs_ac - gmbs + gbspb));
    mna.stamp(sp, dp, m * (-gds_ac - rev_sum + gbspdp));

    // Stamp imaginary parts
    mna.stamp_imag(d, d, 0.0);
    mna.stamp_imag(g, g, m * xcggb);
    mna.stamp_imag(s, s, 0.0);
    mna.stamp_imag(b, b, m * (-(xcbgb + xcbdb + xcbsb)));
    mna.stamp_imag(dp, dp, m * xcddb);
    mna.stamp_imag(sp, sp, m * xcssb);
    mna.stamp_imag(g, b, m * (-(xcggb + xcgdb + xcgsb)));
    mna.stamp_imag(g, dp, m * xcgdb);
    mna.stamp_imag(g, sp, m * xcgsb);
    mna.stamp_imag(b, g, m * xcbgb);
    mna.stamp_imag(b, dp, m * xcbdb);
    mna.stamp_imag(b, sp, m * xcbsb);
    mna.stamp_imag(dp, g, m * xcdgb);
    mna.stamp_imag(dp, b, m * (-(xcdgb + xcddb + xcdsb)));
    mna.stamp_imag(dp, sp, m * xcdsb);
    mna.stamp_imag(sp, g, m * xcsgb);
    mna.stamp_imag(sp, dp, m * xcsdb);
    mna.stamp_imag(sp, b, m * (-(xcsgb + xcsdb + xcssb)));
}

// ========================================================================
// Voltage limiters — duplicated from mosfet1 (DEVfetlim, DEVlimvds)
// ========================================================================

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
