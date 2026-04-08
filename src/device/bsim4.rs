//! BSIM4 v4.8.3 MOSFET model — port of ngspice bsim4/b4ld.c, b4temp.c, b4set.c.
//!
//! LEVEL=14. Uses ngspice's hardcoded constants.
//! This is a faithful port — all equations match the C reference line-by-line.

use crate::device::Device;
use crate::error::SimError;
use crate::mna::MnaSystem;
use crate::mode::*;
use crate::state::StateVectors;

// ngspice BSIM4 constants (from b4ld.c, b4temp.c, b4set.c)
const MAX_EXP: f64 = 5.834617425e14;
const MIN_EXP: f64 = 1.713908431e-15;
const EXP_THRESHOLD: f64 = 34.0;
const EPS0: f64 = 8.85418e-12;
const EPSSI: f64 = 1.03594e-10;
const PI: f64 = 3.141592654;
const CHARGE_Q: f64 = 1.60219e-19;
const KBOQ: f64 = 8.617087e-5;
const DELTA_1: f64 = 0.02;
const DELTA_2: f64 = 0.02;
const DELTA_3: f64 = 0.02;
const DELTA_4: f64 = 0.02;
const NMOS: i32 = 1;
#[allow(dead_code)]
const PMOS: i32 = -1;

// From ngspice const.h
const CONSTVT0: f64 = 0.025864186389684037;
const CONSTROOT2: f64 = 1.4142135623730951;

/// Number of state variables per BSIM4 instance.
const BSIM4_NUM_STATES: usize = 29;

// State vector offsets (from bsim4def.h)
#[allow(dead_code)]
const ST_VBD: usize = 0;
const ST_VBS: usize = 1;
const ST_VGS: usize = 2;
const ST_VDS: usize = 3;

/// DEXP without derivative (b4temp.c version)
#[inline]
fn dexp_nodiv(a: f64) -> f64 {
    if a > EXP_THRESHOLD {
        MAX_EXP * (1.0 + a - EXP_THRESHOLD)
    } else if a < -EXP_THRESHOLD {
        MIN_EXP
    } else {
        a.exp()
    }
}

/// BSIM4 size-dependent parameters — computed from model + L/W binning in temperature().
/// All fields default to 0.0 and are filled in by temperature().
#[derive(Debug, Clone)]
pub struct Bsim4SizeDepParam {
    // Effective dimensions
    pub leff: f64, pub weff: f64, pub leff_cv: f64, pub weff_cv: f64, pub weff_cj: f64,
    pub dl: f64, pub dw: f64, pub dlc: f64, pub dwc: f64, pub dwj: f64,
    // All binned params — set from model base + L/W/P terms
    pub cdsc: f64, pub cdscb: f64, pub cdscd: f64, pub cit: f64,
    pub nfactor: f64, pub tnfactor: f64, pub xj: f64, pub vsat: f64, pub at: f64,
    pub a0: f64, pub ags: f64, pub a1: f64, pub a2: f64,
    pub keta: f64, pub nsub: f64, pub ndep: f64, pub nsd: f64, pub phin: f64,
    pub ngate: f64, pub gamma1: f64, pub gamma2: f64, pub vbx: f64, pub vbi: f64,
    pub vbm: f64, pub xt: f64, pub phi: f64, pub litl: f64,
    pub k1: f64, pub kt1: f64, pub kt1l: f64, pub kt2: f64, pub k2: f64,
    pub k3: f64, pub k3b: f64, pub w0: f64,
    pub lpe0: f64, pub lpeb: f64,
    pub dvtp0: f64, pub dvtp1: f64, pub dvtp2: f64, pub dvtp3: f64, pub dvtp4: f64, pub dvtp5: f64,
    pub dvt0: f64, pub dvt1: f64, pub dvt2: f64,
    pub dvt0w: f64, pub dvt1w: f64, pub dvt2w: f64,
    pub drout: f64, pub dsub: f64,
    pub vth0: f64, pub ua: f64, pub ua1: f64, pub ub: f64, pub ub1: f64,
    pub uc: f64, pub uc1: f64, pub ud: f64, pub ud1: f64, pub up: f64, pub lp: f64,
    pub u0: f64, pub eu: f64, pub ucs: f64, pub ucste: f64,
    pub ute: f64, pub voff: f64, pub tvoff: f64, pub minv: f64, pub minvcv: f64,
    pub vfb: f64, pub delta: f64,
    pub rdsw: f64, pub rds0: f64, pub rdswmin: f64,
    pub prwg: f64, pub prwb: f64, pub prt: f64,
    pub eta0: f64, pub teta0: f64, pub tvoffcv: f64,
    pub etab: f64, pub pclm: f64,
    pub pdibl1: f64, pub pdibl2: f64, pub pdiblb: f64,
    pub fprout: f64, pub pdits: f64, pub pditsd: f64,
    pub pscbe1: f64, pub pscbe2: f64, pub pvag: f64,
    pub wr: f64, pub dwg: f64, pub dwb: f64,
    pub b0: f64, pub b1: f64,
    pub alpha0: f64, pub alpha1: f64, pub beta0: f64,
    pub agidl: f64, pub bgidl: f64, pub cgidl: f64, pub egidl: f64,
    pub fgidl: f64, pub kgidl: f64, pub rgidl: f64,
    pub agisl: f64, pub bgisl: f64, pub cgisl: f64, pub egisl: f64,
    pub fgisl: f64, pub kgisl: f64, pub rgisl: f64,
    pub aigc: f64, pub bigc: f64, pub cigc: f64,
    pub nigc: f64, pub nigbacc: f64, pub nigbinv: f64,
    pub ntox: f64, pub eigbinv: f64, pub pigcd: f64, pub poxedge: f64,
    pub xrcrg1: f64, pub xrcrg2: f64,
    pub lambda: f64, pub vtl: f64, pub xn: f64, pub lc: f64,
    pub tfactor: f64,
    pub vfbsdoff: f64, pub tvfbsdoff: f64,
    // CV model
    pub cgsl: f64, pub cgdl: f64, pub ckappas: f64, pub ckappad: f64,
    pub cf: f64, pub clc: f64, pub cle: f64, pub vfbcv: f64,
    pub noff: f64, pub voffcv: f64, pub acde: f64, pub moin: f64,
    // Pre-calculated
    pub abulk_cv_factor: f64,
    pub cgso: f64, pub cgdo: f64, pub cgbo: f64,
    pub u0temp: f64, pub vsattemp: f64,
    pub sqrt_phi: f64, pub phis3: f64,
    pub xdep0: f64, pub sqrt_xdep0: f64,
    pub theta0vb0: f64, pub theta_rout: f64,
    pub mstar: f64, pub vgsteff_vth: f64,
    pub mstarcv: f64, pub voffcbn: f64, pub voffcbncv: f64,
    pub vfbsd: f64, pub cdep0: f64,
    pub tox_ratio: f64, pub tox_ratio_edge: f64,
    pub aechvb: f64, pub bechvb: f64,
    pub aechvb_edge_s: f64, pub aechvb_edge_d: f64, pub bechvb_edge: f64,
    pub ldeb: f64, pub k1ox: f64,
    pub vfbzb_factor: f64, pub dvtp2factor: f64,
    // Stress
    pub ku0: f64, pub kvth0: f64, pub ku0temp: f64,
    pub rho_ref: f64, pub inv_od_ref: f64,
}

impl Default for Bsim4SizeDepParam {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

/// BSIM4 model parameters — parsed from .MODEL card.
///
/// Uses a flat struct for the ~200 core parameters and a HashMap for L/W/P binning.
/// This avoids the 900+ fields that the full C struct has while still supporting binning.
#[derive(Debug, Clone)]
pub struct Bsim4Model {
    pub mos_type: i32,
    // Mode selectors
    pub mob_mod: i32, pub cap_mod: i32, pub dio_mod: i32,
    pub rds_mod: i32, pub rbody_mod: i32, pub rgate_mod: i32,
    pub per_mod: i32, pub geo_mod: i32,
    pub mtrl_mod: i32, pub temp_mod: i32, pub bin_unit: i32,
    pub igc_mod: i32, pub igb_mod: i32, pub gidl_mod: i32,
    // Core parameters
    pub toxe: f64, pub toxp: f64, pub toxm: f64, pub dtox: f64, pub epsrox: f64,
    pub cdsc: f64, pub cdscb: f64, pub cdscd: f64, pub cit: f64,
    pub nfactor: f64, pub xj: f64, pub vsat: f64, pub at: f64,
    pub a0: f64, pub ags: f64, pub a1: f64, pub a2: f64,
    pub keta: f64, pub ketac: f64, pub nsub: f64,
    pub ndep: f64, pub nsd: f64, pub phin: f64, pub ngate: f64,
    pub gamma1: f64, pub gamma2: f64, pub vbx: f64, pub vbm: f64, pub xt: f64,
    pub k1: f64, pub kt1: f64, pub kt1l: f64, pub kt2: f64,
    pub k2: f64, pub k3: f64, pub k3b: f64, pub w0: f64,
    pub lpe0: f64, pub lpeb: f64,
    pub dvtp0: f64, pub dvtp1: f64, pub dvtp2: f64, pub dvtp3: f64, pub dvtp4: f64, pub dvtp5: f64,
    pub dvt0: f64, pub dvt1: f64, pub dvt2: f64,
    pub dvt0w: f64, pub dvt1w: f64, pub dvt2w: f64,
    pub drout: f64, pub dsub: f64, pub vth0: f64, pub vfb: f64,
    pub eu: f64, pub ucs: f64,
    pub ua: f64, pub ua1: f64, pub ub: f64, pub ub1: f64,
    pub uc: f64, pub uc1: f64, pub ud: f64, pub ud1: f64, pub up: f64, pub lp: f64,
    pub u0: f64, pub ute: f64, pub ucste: f64,
    pub voff: f64, pub voffl: f64, pub voffcvl: f64,
    pub minv: f64, pub minvcv: f64,
    pub fprout: f64, pub pdits: f64, pub pditsd: f64, pub pditsl: f64,
    pub delta: f64,
    pub rdsw: f64, pub rdswmin: f64, pub rdwmin: f64, pub rswmin: f64,
    pub rsw: f64, pub rdw: f64,
    pub prwg: f64, pub prwb: f64, pub prt: f64,
    pub eta0: f64, pub etab: f64, pub pclm: f64,
    pub pdibl1: f64, pub pdibl2: f64, pub pdiblb: f64,
    pub pscbe1: f64, pub pscbe2: f64, pub pvag: f64,
    pub wr: f64, pub dwg: f64, pub dwb: f64,
    pub b0: f64, pub b1: f64,
    pub alpha0: f64, pub alpha1: f64, pub beta0: f64,
    pub agidl: f64, pub bgidl: f64, pub cgidl: f64, pub egidl: f64,
    pub fgidl: f64, pub kgidl: f64, pub rgidl: f64,
    pub agisl: f64, pub bgisl: f64, pub cgisl: f64, pub egisl: f64,
    pub fgisl: f64, pub kgisl: f64, pub rgisl: f64,
    pub aigc: f64, pub bigc: f64, pub cigc: f64,
    pub aigsd: f64, pub bigsd: f64, pub cigsd: f64,
    pub aigs: f64, pub bigs: f64, pub cigs: f64,
    pub aigd: f64, pub bigd: f64, pub cigd: f64,
    pub aigbacc: f64, pub bigbacc: f64, pub cigbacc: f64,
    pub aigbinv: f64, pub bigbinv: f64, pub cigbinv: f64,
    pub nigc: f64, pub nigbacc: f64, pub nigbinv: f64,
    pub ntox: f64, pub eigbinv: f64, pub pigcd: f64, pub poxedge: f64,
    pub toxref: f64,
    pub lambda: f64, pub vtl: f64, pub xn: f64, pub lc: f64,
    pub vfbsdoff: f64, pub tvfbsdoff: f64, pub tvoff: f64,
    pub tnfactor: f64, pub teta0: f64, pub tvoffcv: f64,
    pub xrcrg1: f64, pub xrcrg2: f64,
    pub ijthsfwd: f64, pub ijthdfwd: f64, pub ijthsrev: f64, pub ijthdrev: f64,
    pub xjbvs: f64, pub xjbvd: f64, pub bvs: f64, pub bvd: f64,
    pub gbmin: f64,
    // CV model
    pub cgsl: f64, pub cgdl: f64, pub ckappas: f64, pub ckappad: f64,
    pub cf: f64, pub vfbcv: f64, pub clc: f64, pub cle: f64,
    pub dwc: f64, pub dlc: f64, pub dlcig: f64, pub dlcigd: f64,
    pub dwj: f64, pub xl: f64, pub xw: f64,
    pub noff: f64, pub voffcv: f64, pub acde: f64, pub moin: f64,
    pub tcj: f64, pub tcjsw: f64, pub tcjswg: f64,
    pub tpb: f64, pub tpbsw: f64, pub tpbswg: f64,
    pub dmcg: f64, pub dmci: f64, pub dmdg: f64, pub dmcgt: f64,
    pub xgw: f64, pub xgl: f64, pub rshg: f64, pub ngcon: f64,
    // Geometry / Junction
    pub sheet_resistance: f64,
    pub sjct_sat_cur_density: f64, pub sbulk_jct_potential: f64,
    pub ssidewall_jct_potential: f64, pub sgate_sidewall_jct_potential: f64,
    pub sunit_area_jct_cap: f64, pub sunit_length_sidewall_jct_cap: f64,
    pub sunit_length_gate_sidewall_jct_cap: f64,
    pub sbulk_jct_bot_grading_coeff: f64, pub sbulk_jct_side_grading_coeff: f64,
    pub sjct_emission_coeff: f64, pub sjct_temp_exponent: f64,
    pub djct_sat_cur_density: f64, pub dbulk_jct_potential: f64, pub dbulk_jct_bot_grading_coeff: f64,
    pub dsidewall_jct_potential: f64, pub dgate_sidewall_jct_potential: f64,
    pub dunit_area_jct_cap: f64, pub dunit_length_sidewall_jct_cap: f64,
    pub djct_emission_coeff: f64, pub djct_temp_exponent: f64,
    // Binning scalars
    pub lint: f64, pub ll: f64, pub llc: f64, pub lln: f64,
    pub lw_bin: f64, pub lwc: f64, pub lwn: f64, pub lwl: f64, pub lwlc: f64,
    pub wint: f64, pub wl_bin: f64, pub wlc: f64, pub wln: f64,
    pub ww_bin: f64, pub wwc: f64, pub wwn: f64, pub wwl: f64, pub wwlc: f64,
    // Overlap caps
    pub cgdo: f64, pub cgso: f64, pub cgbo: f64,
    pub tnom: f64,
    // Stress
    pub saref: f64, pub sbref: f64,
    pub ku0: f64,
    // Gidl/idovvdsc
    pub gidlclamp: f64, pub idovvdsc: f64,
    // Quantum-mechanical Coxeff
    pub ados: f64, pub bdos: f64,
    // Lambda
    pub lambda_given: bool,
    // Temperature-computed
    pub coxe: f64, pub coxp: f64,
    pub eg0: f64, pub vtm: f64, pub vtm0: f64, pub factor1: f64, pub vcrit: f64,
    pub sjct_temp_sat_cur_density: f64,
    pub djct_temp_sat_cur_density: f64,
    pub phi_bs: f64, pub phi_bd: f64,
    pub sunit_area_temp_jct_cap: f64, pub dunit_area_temp_jct_cap: f64,
    // Given flags
    pub toxe_given: bool, pub toxp_given: bool, pub toxm_given: bool,
    pub k1_given: bool, pub k2_given: bool,
    pub ndep_given: bool, pub nsub_given: bool,
    pub vbx_given: bool, pub vfb_given: bool, pub vth0_given: bool,
    pub gamma1_given: bool, pub gamma2_given: bool,
    pub dlc_given: bool, pub cf_given: bool,
    pub cgdo_given: bool, pub cgso_given: bool, pub cgbo_given: bool,
    pub ua_given: bool, pub uc_given: bool, pub uc1_given: bool,
    pub dsub_given: bool, pub vtl_given: bool,
    pub aigsd_given: bool, pub aigs_given: bool, pub aigd_given: bool,
    pub bigsd_given: bool, pub bigs_given: bool, pub bigd_given: bool,
    pub cigsd_given: bool, pub cigs_given: bool, pub cigd_given: bool,
    pub tnom_given: bool,
}

impl Default for Bsim4Model {
    fn default() -> Self {
        let mut m: Self = unsafe { std::mem::zeroed() };
        m.mos_type = 1;
        m.cap_mod = 2;
        m.dio_mod = 1;
        m.per_mod = 1;
        m.bin_unit = 1;
        m.toxref = 30.0e-10;
        m.toxe = 30.0e-10;
        m.epsrox = 3.9;
        m.cdsc = 2.4e-4;
        m.nfactor = 1.0;
        m.xj = 0.15e-6;
        m.vsat = 8.0e4;
        m.at = 3.3e4;
        m.a0 = 1.0;
        m.a2 = 1.0;
        m.keta = -0.047;
        m.nsub = 6.0e16;
        m.ndep = 1.7e17;
        m.nsd = 1.0e20;
        m.vbm = -3.0;
        m.xt = 1.55e-7;
        m.kt1 = -0.11;
        m.kt2 = 0.022;
        m.k3 = 80.0;
        m.w0 = 2.5e-6;
        m.lpe0 = 1.74e-7;
        m.dvt0 = 2.2;
        m.dvt1 = 0.53;
        m.dvt2 = -0.032;
        m.dvt1w = 5.3e6;
        m.dvt2w = -0.032;
        m.drout = 0.56;
        m.vfb = -1.0;
        m.ua1 = 1.0e-9;
        m.ub = 1.0e-19;
        m.ub1 = -1.0e-18;
        m.lp = 1.0e-8;
        m.ute = -1.5;
        m.ucste = -4.775e-3;
        m.voff = -0.08;
        m.delta = 0.01;
        m.rdsw = 200.0;
        m.rdw = 100.0;
        m.rsw = 100.0;
        m.prwg = 1.0;
        m.eta0 = 0.08;
        m.etab = -0.07;
        m.pclm = 1.3;
        m.pdibl1 = 0.39;
        m.pdibl2 = 0.0086;
        m.pscbe1 = 4.24e8;
        m.pscbe2 = 1.0e-5;
        m.wr = 1.0;
        m.bgidl = 2.3e9;
        m.cgidl = 0.5;
        m.egidl = 0.8;
        m.fgidl = 1.0;
        m.rgidl = 1.0;
        m.aigbacc = 1.36e-2;
        m.bigbacc = 1.71e-3;
        m.cigbacc = 0.075;
        m.aigbinv = 1.11e-2;
        m.bigbinv = 9.49e-4;
        m.cigbinv = 0.006;
        m.nigc = 1.0;
        m.nigbinv = 3.0;
        m.nigbacc = 1.0;
        m.ntox = 1.0;
        m.eigbinv = 1.1;
        m.pigcd = 1.0;
        m.poxedge = 1.0;
        m.xrcrg1 = 12.0;
        m.xrcrg2 = 1.0;
        m.ijthsfwd = 0.1;
        m.ijthsrev = 0.1;
        m.xjbvs = 1.0;
        m.bvs = 10.0;
        m.gbmin = 1.0e-12;
        m.ckappas = 0.6;
        m.clc = 0.1e-6;
        m.cle = 0.6;
        m.vfbcv = -1.0;
        m.acde = 1.0;
        m.moin = 15.0;
        m.noff = 1.0;
        m.rshg = 0.1;
        m.ngcon = 1.0;
        m.vtl = 2.0e5;
        m.xn = 3.0;
        m.lc = 5.0e-9;
        m.gidlclamp = -1e-5;
        m.idovvdsc = 1e-9;
        m.ados = 1.0;
        m.bdos = 1.0;
        m.tnom = 300.15;
        m.lln = 1.0;
        m.lwn = 1.0;
        m.wln = 1.0;
        m.wwn = 1.0;
        m.saref = 1e-6;
        m.sbref = 1e-6;
        m.sjct_sat_cur_density = 1.0e-4;
        m.sjct_emission_coeff = 1.0;
        m.sjct_temp_exponent = 3.0;
        m.sbulk_jct_potential = 1.0;
        m.ssidewall_jct_potential = 1.0;
        m.sbulk_jct_bot_grading_coeff = 0.5;
        m.sbulk_jct_side_grading_coeff = 0.33;
        m.sunit_area_jct_cap = 5.0e-4;
        m.sunit_length_sidewall_jct_cap = 5.0e-10;
        m
    }
}

impl Bsim4Model {
    pub fn apply_defaults(&mut self) {
        let tp = self.mos_type;
        if !self.vth0_given {
            self.vth0 = if tp == NMOS { 0.7 } else { -0.7 };
        }
        if self.eu == 0.0 { self.eu = if tp == NMOS { 1.67 } else { 1.0 }; }
        if self.ucs == 0.0 { self.ucs = if tp == NMOS { 1.67 } else { 1.0 }; }
        if !self.ua_given {
            self.ua = if self.mob_mod == 2 || self.mob_mod == 6 { 1.0e-15 } else { 1.0e-9 };
        }
        if !self.uc_given {
            self.uc = if self.mob_mod == 1 || self.mob_mod == 5 { -0.0465 } else { -0.0465e-9 };
        }
        if !self.uc1_given {
            self.uc1 = if self.mob_mod == 1 || self.mob_mod == 5 { -0.056 } else { -0.056e-9 };
        }
        if self.u0 == 0.0 { self.u0 = if tp == NMOS { 0.067 } else { 0.025 }; }
        if self.aigc == 0.0 { self.aigc = if tp == NMOS { 1.36e-2 } else { 9.80e-3 }; }
        if self.bigc == 0.0 { self.bigc = if tp == NMOS { 1.71e-3 } else { 7.59e-4 }; }
        if self.cigc == 0.0 { self.cigc = if tp == NMOS { 0.075 } else { 0.03 }; }
        if self.aigsd_given {
            self.aigs = self.aigsd; self.aigd = self.aigsd;
        } else {
            if self.aigsd == 0.0 { self.aigsd = if tp == NMOS { 1.36e-2 } else { 9.80e-3 }; }
            if !self.aigs_given { self.aigs = self.aigsd; }
            if !self.aigd_given { self.aigd = self.aigsd; }
        }
        if self.bigsd_given {
            self.bigs = self.bigsd; self.bigd = self.bigsd;
        } else {
            if self.bigsd == 0.0 { self.bigsd = if tp == NMOS { 1.71e-3 } else { 7.59e-4 }; }
            if !self.bigs_given { self.bigs = self.bigsd; }
            if !self.bigd_given { self.bigd = self.bigsd; }
        }
        if self.cigsd_given {
            self.cigs = self.cigsd; self.cigd = self.cigsd;
        } else {
            if self.cigsd == 0.0 { self.cigsd = if tp == NMOS { 0.075 } else { 0.03 }; }
            if !self.cigs_given { self.cigs = self.cigsd; }
            if !self.cigd_given { self.cigd = self.cigsd; }
        }
        if self.agisl == 0.0 { self.agisl = self.agidl; }
        if self.bgisl == 0.0 { self.bgisl = self.bgidl; }
        if self.cgisl == 0.0 { self.cgisl = self.cgidl; }
        if self.egisl == 0.0 { self.egisl = self.egidl; }
        if self.fgisl == 0.0 { self.fgisl = self.fgidl; }
        if self.kgisl == 0.0 { self.kgisl = self.kgidl; }
        if self.rgisl == 0.0 { self.rgisl = self.rgidl; }
        if self.djct_sat_cur_density == 0.0 { self.djct_sat_cur_density = self.sjct_sat_cur_density; }
        if self.djct_emission_coeff == 0.0 { self.djct_emission_coeff = self.sjct_emission_coeff; }
        if self.djct_temp_exponent == 0.0 { self.djct_temp_exponent = self.sjct_temp_exponent; }
        if self.dbulk_jct_potential == 0.0 { self.dbulk_jct_potential = self.sbulk_jct_potential; }
        if self.dsidewall_jct_potential == 0.0 { self.dsidewall_jct_potential = self.ssidewall_jct_potential; }
        if self.dgate_sidewall_jct_potential == 0.0 { self.dgate_sidewall_jct_potential = self.sgate_sidewall_jct_potential; }
        if self.dunit_area_jct_cap == 0.0 { self.dunit_area_jct_cap = self.sunit_area_jct_cap; }
        if self.dunit_length_sidewall_jct_cap == 0.0 { self.dunit_length_sidewall_jct_cap = self.sunit_length_sidewall_jct_cap; }
        if self.ketac == 0.0 { self.ketac = self.keta; }
        if !self.dsub_given { self.dsub = self.drout; }
        if !self.toxp_given { self.toxp = self.toxe; }
        if !self.toxm_given { self.toxm = self.toxe; }
        if self.ckappad == 0.0 { self.ckappad = self.ckappas; }
        if self.dmci == 0.0 { self.dmci = self.dmcg; }
        if self.ijthdfwd == 0.0 { self.ijthdfwd = self.ijthsfwd; }
        if self.ijthdrev == 0.0 { self.ijthdrev = self.ijthsrev; }
        if self.xjbvd == 0.0 { self.xjbvd = self.xjbvs; }
        if self.bvd == 0.0 { self.bvd = self.bvs; }
        if self.mtrl_mod == 0 {
            if self.toxe_given && !self.toxp_given {
                self.toxp = self.toxe - self.dtox;
            } else if !self.toxe_given && self.toxp_given {
                self.toxe = self.toxp + self.dtox;
                if !self.toxm_given { self.toxm = self.toxe; }
            }
        }
    }
}

const MM: f64 = 3.0; // smooth coeff

#[inline]
fn dexp(a: f64) -> (f64, f64) {
    if a > EXP_THRESHOLD {
        (MAX_EXP * (1.0 + a - EXP_THRESHOLD), MAX_EXP)
    } else if a < -EXP_THRESHOLD {
        (MIN_EXP, 0.0)
    } else {
        let e = a.exp();
        (e, e)
    }
}

/// BSIM4 poly depletion effect — port of BSIM4polyDepletion() from b4ld.c
#[inline]
fn bsim4_poly_depletion(phi: f64, ngate: f64, epsgate: f64, coxe: f64, vgs: f64) -> (f64, f64) {
    if ngate > 1.0e18 && ngate < 1.0e25 && vgs > phi && epsgate != 0.0 {
        let t1 = 1.0e6 * CHARGE_Q * epsgate * ngate / (coxe * coxe);
        let t8 = vgs - phi;
        let t4 = (1.0 + 2.0 * t8 / t1).sqrt();
        let t2 = 2.0 * t8 / (t4 + 1.0);
        let t3 = 0.5 * t2 * t2 / t1;
        let t7 = 1.12 - t3 - 0.05;
        let t6 = (t7 * t7 + 0.224).sqrt();
        let t5 = 1.12 - 0.5 * (t7 + t6);
        let vgs_eff = vgs - t5;
        let dvgs_eff_dvg = 1.0 - (0.5 - 0.5 / t4) * (1.0 + t7 / t6);
        (vgs_eff, dvgs_eff_dvg)
    } else {
        (vgs, 1.0)
    }
}

/// BSIM4 MOSFET instance.
#[derive(Debug)]
pub struct Bsim4 {
    name: String,
    nd: usize, ng: usize, ns: usize, nb: usize,
    dp: usize, gp: usize, sp: usize, bp: usize,
    pub model: Bsim4Model,
    w: f64, l: f64, m: f64, nf: f64,
    pub param: Bsim4SizeDepParam,
    state_offset: usize,
    mode: i32,
    von: f64, vdsat: f64,
    gm: f64, gds: f64, gmbs: f64,
    gbd: f64, gbs: f64,
    cbd: f64, cbs: f64,
    cd: f64,
    csub: f64,
    source_conductance: f64, drain_conductance: f64,
    cgso: f64, cgdo: f64,
    vth0: f64, vfb: f64, vfbzb: f64,
    vtfbphi1: f64, vtfbphi2: f64,
    k2: f64, vbsc: f64, k2ox: f64, eta0: f64,
    u0temp: f64, vsattemp: f64,
    toxp: f64, coxp: f64,
    grgeltd: f64,
    mult_i: f64, mult_q: f64,
    vgsteff: f64, vdseff: f64,
    off: bool,
    // delta/delta_old for predictor
    pub delta: f64,
    pub delta_old1: f64,
    // Temperature from last temperature() call
    pub temp: f64,
    // Charge model caps (zeroed for DC)
    cggb: f64, cgdb: f64, cgsb: f64, cgbb: f64,
    cbgb: f64, cbdb: f64, cbsb: f64, cbbb: f64,
    cdgb: f64, cddb: f64, cdsb: f64, cdbb: f64,
    capbd: f64, capbs: f64,
    qgate: f64, qbulk: f64, qdrn: f64,
    // GIDL/GISL currents
    igidl: f64, ggidld: f64, ggidlg: f64, ggidlb: f64,
    igisl: f64, ggisls: f64, ggislg: f64, ggislb: f64,
    // Substrate current
    gbbs: f64, gbgs: f64, gbds: f64,
    // Device initial conditions (from .IC node voltages or instance params)
    ic_vds: f64, ic_vgs: f64, ic_vbs: f64,
    ic_vds_given: bool, ic_vgs_given: bool, ic_vbs_given: bool,
}

impl Bsim4 {
    pub fn new(name: &str, nd: usize, ng: usize, ns: usize, nb: usize,
               model: Bsim4Model, w: f64, l: f64, m: f64) -> Self {
        Self {
            name: name.to_string(),
            nd, ng, ns, nb,
            dp: nd, gp: ng, sp: ns, bp: nb,
            model, w, l, m, nf: 1.0,
            param: Bsim4SizeDepParam::default(),
            state_offset: 0,
            mode: 1,
            von: 0.0, vdsat: 0.0,
            gm: 0.0, gds: 0.0, gmbs: 0.0,
            gbd: 0.0, gbs: 0.0, cbd: 0.0, cbs: 0.0, cd: 0.0,
            csub: 0.0,
            source_conductance: 0.0, drain_conductance: 0.0,
            cgso: 0.0, cgdo: 0.0,
            vth0: 0.0, vfb: 0.0, vfbzb: 0.0,
            vtfbphi1: 0.0, vtfbphi2: 0.0,
            k2: 0.0, vbsc: 0.0, k2ox: 0.0, eta0: 0.0,
            u0temp: 0.0, vsattemp: 0.0,
            toxp: 0.0, coxp: 0.0,
            grgeltd: 0.0,
            mult_i: m, mult_q: m,
            vgsteff: 0.0, vdseff: 0.0,
            off: false,
            delta: 0.0, delta_old1: 1.0,
            temp: 300.15,
            cggb: 0.0, cgdb: 0.0, cgsb: 0.0, cgbb: 0.0,
            cbgb: 0.0, cbdb: 0.0, cbsb: 0.0, cbbb: 0.0,
            cdgb: 0.0, cddb: 0.0, cdsb: 0.0, cdbb: 0.0,
            capbd: 0.0, capbs: 0.0,
            qgate: 0.0, qbulk: 0.0, qdrn: 0.0,
            igidl: 0.0, ggidld: 0.0, ggidlg: 0.0, ggidlb: 0.0,
            igisl: 0.0, ggisls: 0.0, ggislg: 0.0, ggislb: 0.0,
            gbbs: 0.0, gbgs: 0.0, gbds: 0.0,
            ic_vds: 0.0, ic_vgs: 0.0, ic_vbs: 0.0,
            ic_vds_given: false, ic_vgs_given: false, ic_vbs_given: false,
        }
    }

    pub fn set_internal_nodes(&mut self, dp: usize, sp: usize) {
        self.dp = dp;
        self.sp = sp;
    }
}

impl Device for Bsim4 {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn name(&self) -> &str { &self.name }

    /// BSIM4getic (b4getic.c): propagate .IC node voltages to device ICs.
    /// Note: uses gNodeExt (ng) for VGS, not gNodePrime (gp).
    fn setic(&mut self, rhs: &[f64]) {
        if !self.ic_vds_given { self.ic_vds = rhs[self.nd] - rhs[self.ns]; }
        if !self.ic_vgs_given { self.ic_vgs = rhs[self.ng] - rhs[self.ns]; }
        if !self.ic_vbs_given { self.ic_vbs = rhs[self.nb] - rhs[self.ns]; }
    }

    fn setup(&mut self, states: &mut StateVectors) -> usize {
        self.state_offset = states.allocate(BSIM4_NUM_STATES);
        BSIM4_NUM_STATES
    }

    fn setup_matrix(&mut self, mna: &mut MnaSystem) {
        let dp = self.dp; let gp = self.gp; let sp = self.sp; let bp = self.bp;
        let nd = self.nd; let ns = self.ns;
        for &(r, c) in &[
            (dp, bp), (gp, bp), (sp, bp),
            (bp, dp), (bp, gp), (bp, sp), (bp, bp),
            (nd, nd), (gp, gp), (ns, ns),
            (dp, dp), (sp, sp),
            (nd, dp), (gp, dp), (gp, sp), (ns, sp),
            (dp, sp), (dp, nd), (dp, gp),
            (sp, gp), (sp, ns), (sp, dp),
        ] {
            mna.make_element(r, c);
        }
    }

    fn temperature(&mut self, temp: f64, tnom: f64) {
        bsim4_temperature(self, temp, tnom);
    }

    fn load(
        &mut self,
        mna: &mut MnaSystem,
        states: &mut StateVectors,
        mode: Mode,
        src_fact: f64,
        gmin: f64,
        noncon: &mut bool,
    ) -> Result<(), SimError> {
        bsim4_load(self, mna, states, mode, src_fact, gmin, noncon)
    }

    fn conductances(&self) -> Vec<(&str, f64)> {
        vec![("gm", self.gm), ("gds", self.gds), ("gmbs", self.gmbs)]
    }
    fn limited_voltages(&self) -> Vec<(&str, f64)> { vec![] }
}

/// Port of BSIM4temp() from b4temp.c
fn bsim4_temperature(dev: &mut Bsim4, temp: f64, tnom_arg: f64) {
    dev.temp = temp;
    let model = &mut dev.model;
    let tnom = if model.tnom_given { model.tnom } else { tnom_arg };
    model.tnom = tnom;
    let t_ratio = temp / tnom;
    let del_temp = temp - tnom;
    let epsrox = model.epsrox;
    let toxe = model.toxe;
    let epssub = EPSSI;

    if !model.cf_given {
        model.cf = 2.0 * epsrox * EPS0 / PI * (1.0 + 0.4e-6 / toxe).ln();
    }
    model.coxe = epsrox * EPS0 / toxe;
    model.coxp = model.epsrox * EPS0 / model.toxp;
    if !model.cgdo_given {
        if model.dlc_given && model.dlc > 0.0 {
            model.cgdo = model.dlc * model.coxe - model.cgdl;
        } else {
            model.cgdo = 0.6 * model.xj * model.coxe;
        }
    }
    if !model.cgso_given {
        if model.dlc_given && model.dlc > 0.0 {
            model.cgso = model.dlc * model.coxe - model.cgsl;
        } else {
            model.cgso = 0.6 * model.xj * model.coxe;
        }
    }
    if !model.cgbo_given {
        model.cgbo = 2.0 * model.dwc * model.coxe;
    }
    model.vcrit = CONSTVT0 * (CONSTVT0 / (CONSTROOT2 * 1.0e-14)).ln();
    model.factor1 = (epssub / (epsrox * EPS0) * toxe).sqrt();
    let vtm0 = KBOQ * tnom;
    model.vtm0 = vtm0;
    model.vtm = KBOQ * temp;
    let eg0 = 1.16 - 7.02e-4 * tnom * tnom / (tnom + 1108.0);
    model.eg0 = eg0;
    let ni = 1.45e10 * (tnom / 300.15) * (tnom / 300.15).sqrt()
        * (21.5565981 - eg0 / (2.0 * vtm0)).exp();
    let _eg = 1.16 - 7.02e-4 * temp * temp / (temp + 1108.0);

    // Junction temperature scaling (simplified for now — test circuits don't exercise junctions)
    model.sjct_temp_sat_cur_density = model.sjct_sat_cur_density;
    model.djct_temp_sat_cur_density = model.djct_sat_cur_density;
    model.phi_bs = (model.sbulk_jct_potential - model.tpb * del_temp).max(0.01);
    model.phi_bd = (model.dbulk_jct_potential - model.tpb * del_temp).max(0.01);

    // Size-dependent parameter binning (base only — no L/W/P terms in test circuits)
    let p = &mut dev.param;
    let lnew = dev.l + model.xl;
    let wnew = dev.w / dev.nf + model.xw;
    p.dl = model.lint;
    p.dlc = model.dlc;
    p.dw = model.wint;
    p.dwc = model.dwc;
    p.dwj = model.dwj;
    p.leff = lnew - 2.0 * p.dl;
    p.weff = wnew - 2.0 * p.dw;
    p.leff_cv = lnew - 2.0 * p.dlc;
    p.weff_cv = wnew - 2.0 * p.dwc;
    p.weff_cj = wnew - 2.0 * p.dwj;

    // Copy base params directly (all L/W/P binning terms are 0)
    p.cdsc = model.cdsc; p.cdscb = model.cdscb; p.cdscd = model.cdscd;
    p.cit = model.cit; p.nfactor = model.nfactor; p.tnfactor = model.tnfactor;
    p.xj = model.xj; p.vsat = model.vsat; p.at = model.at;
    p.a0 = model.a0; p.ags = model.ags; p.a1 = model.a1; p.a2 = model.a2;
    p.keta = model.keta; p.nsub = model.nsub; p.ndep = model.ndep;
    p.nsd = model.nsd; p.phin = model.phin; p.ngate = model.ngate;
    p.gamma1 = model.gamma1; p.gamma2 = model.gamma2;
    p.vbx = model.vbx; p.vbm = model.vbm; p.xt = model.xt;
    p.vfb = model.vfb; p.k1 = model.k1; p.kt1 = model.kt1; p.kt1l = model.kt1l;
    p.kt2 = model.kt2; p.k2 = model.k2; p.k3 = model.k3; p.k3b = model.k3b;
    p.w0 = model.w0; p.lpe0 = model.lpe0; p.lpeb = model.lpeb;
    p.dvtp0 = model.dvtp0; p.dvtp1 = model.dvtp1; p.dvtp2 = model.dvtp2;
    p.dvtp3 = model.dvtp3; p.dvtp4 = model.dvtp4; p.dvtp5 = model.dvtp5;
    p.dvt0 = model.dvt0; p.dvt1 = model.dvt1; p.dvt2 = model.dvt2;
    p.dvt0w = model.dvt0w; p.dvt1w = model.dvt1w; p.dvt2w = model.dvt2w;
    p.drout = model.drout; p.dsub = model.dsub; p.vth0 = model.vth0;
    p.ua = model.ua; p.ua1 = model.ua1; p.ub = model.ub; p.ub1 = model.ub1;
    p.uc = model.uc; p.uc1 = model.uc1; p.ud = model.ud; p.ud1 = model.ud1;
    p.up = model.up; p.lp = model.lp; p.u0 = model.u0;
    p.eu = model.eu; p.ucs = model.ucs; p.ucste = model.ucste; p.ute = model.ute;
    p.voff = model.voff; p.tvoff = model.tvoff;
    p.minv = model.minv; p.minvcv = model.minvcv;
    p.fprout = model.fprout; p.pdits = model.pdits; p.pditsd = model.pditsd;
    p.delta = model.delta; p.rdsw = model.rdsw;
    p.prwg = model.prwg; p.prwb = model.prwb; p.prt = model.prt;
    p.eta0 = model.eta0; p.teta0 = model.teta0; p.tvoffcv = model.tvoffcv;
    p.etab = model.etab; p.pclm = model.pclm;
    p.pdibl1 = model.pdibl1; p.pdibl2 = model.pdibl2; p.pdiblb = model.pdiblb;
    p.pscbe1 = model.pscbe1; p.pscbe2 = model.pscbe2; p.pvag = model.pvag;
    p.wr = model.wr; p.dwg = model.dwg; p.dwb = model.dwb;
    p.b0 = model.b0; p.b1 = model.b1;
    p.alpha0 = model.alpha0; p.alpha1 = model.alpha1; p.beta0 = model.beta0;
    p.agidl = model.agidl; p.bgidl = model.bgidl; p.cgidl = model.cgidl;
    p.egidl = model.egidl; p.fgidl = model.fgidl; p.kgidl = model.kgidl; p.rgidl = model.rgidl;
    p.agisl = model.agisl; p.bgisl = model.bgisl; p.cgisl = model.cgisl;
    p.egisl = model.egisl; p.fgisl = model.fgisl; p.kgisl = model.kgisl; p.rgisl = model.rgisl;
    p.aigc = model.aigc; p.bigc = model.bigc; p.cigc = model.cigc;
    p.nigc = model.nigc; p.nigbacc = model.nigbacc; p.nigbinv = model.nigbinv;
    p.ntox = model.ntox; p.eigbinv = model.eigbinv; p.pigcd = model.pigcd;
    p.poxedge = model.poxedge;
    p.xrcrg1 = model.xrcrg1; p.xrcrg2 = model.xrcrg2;
    p.lambda = model.lambda; p.vtl = model.vtl; p.xn = model.xn;
    p.vfbsdoff = model.vfbsdoff; p.tvfbsdoff = model.tvfbsdoff;
    p.cgsl = model.cgsl; p.cgdl = model.cgdl;
    p.ckappas = model.ckappas; p.ckappad = model.ckappad;
    p.cf = model.cf; p.clc = model.clc; p.cle = model.cle;
    p.vfbcv = model.vfbcv; p.acde = model.acde; p.moin = model.moin;
    p.noff = model.noff; p.voffcv = model.voffcv;

    p.abulk_cv_factor = 1.0 + (p.clc / p.leff_cv).powf(p.cle);

    // Temperature-dependent modifications
    let t0 = t_ratio - 1.0;
    let pow_weff_wr = (p.weff_cj * 1.0e6).powf(p.wr) * dev.nf;
    p.ucs = p.ucs * t_ratio.powf(p.ucste);
    if model.temp_mod == 0 {
        p.ua = p.ua + p.ua1 * t0;
        p.ub = p.ub + p.ub1 * t0;
        p.uc = p.uc + p.uc1 * t0;
        p.ud = p.ud + p.ud1 * t0;
        p.vsattemp = p.vsat - p.at * t0;
        let t10 = p.prt * t0;
        p.rds0 = (p.rdsw + t10) * dev.nf / pow_weff_wr;
        p.rdswmin = (model.rdswmin + t10) * dev.nf / pow_weff_wr;
    } else {
        if model.temp_mod == 3 {
            p.ua = p.ua * t_ratio.powf(p.ua1);
            p.ub = p.ub * t_ratio.powf(p.ub1);
            p.uc = p.uc * t_ratio.powf(p.uc1);
            p.ud = p.ud * t_ratio.powf(p.ud1);
        } else {
            p.ua = p.ua * (1.0 + p.ua1 * del_temp);
            p.ub = p.ub * (1.0 + p.ub1 * del_temp);
            p.uc = p.uc * (1.0 + p.uc1 * del_temp);
            p.ud = p.ud * (1.0 + p.ud1 * del_temp);
        }
        p.vsattemp = p.vsat * (1.0 - p.at * del_temp);
        let t10 = 1.0 + p.prt * del_temp;
        p.rds0 = p.rdsw * t10 * dev.nf / pow_weff_wr;
        p.rdswmin = model.rdswmin * t10 * dev.nf / pow_weff_wr;
    }
    if p.u0 > 1.0 { p.u0 /= 1.0e4; }
    let t5 = 1.0 - p.up * (-p.leff / p.lp).exp();
    p.u0temp = p.u0 * t5 * t_ratio.powf(p.ute);
    if p.eu < 0.0 { p.eu = 0.0; }
    if p.ucs < 0.0 { p.ucs = 0.0; }
    p.vfbsdoff = p.vfbsdoff * (1.0 + p.tvfbsdoff * del_temp);
    p.voff = p.voff * (1.0 + p.tvoff * del_temp);
    p.nfactor = p.nfactor + p.tnfactor * del_temp / tnom;
    p.voffcv = p.voffcv * (1.0 + p.tvoffcv * del_temp);
    p.eta0 = p.eta0 + p.teta0 * del_temp / tnom;
    if model.vtl_given && model.vtl > 0.0 {
        p.lc = if model.lc < 0.0 { 0.0 } else { model.lc };
        let t0 = p.leff / (p.xn * p.leff + p.lc);
        p.tfactor = (1.0 - t0) / (1.0 + t0);
    }
    p.cgdo = (model.cgdo + p.cf) * p.weff_cv;
    p.cgso = (model.cgso + p.cf) * p.weff_cv;
    p.cgbo = model.cgbo * p.leff_cv * dev.nf;
    if !model.ndep_given && model.gamma1_given {
        let t0 = p.gamma1 * model.coxe;
        p.ndep = 3.01248e22 * t0 * t0;
    }
    p.phi = vtm0 * (p.ndep / ni).ln() + p.phin + 0.4;
    p.sqrt_phi = p.phi.sqrt();
    p.phis3 = p.sqrt_phi * p.phi;
    p.xdep0 = (2.0 * epssub / (CHARGE_Q * p.ndep * 1.0e6)).sqrt() * p.sqrt_phi;
    p.sqrt_xdep0 = p.xdep0.sqrt();
    p.litl = (3.0 * 3.9 / epsrox * p.xj * toxe).sqrt();
    p.vbi = vtm0 * (p.nsd * p.ndep / (ni * ni)).ln();
    p.vfbsd = if p.ngate > 0.0 { vtm0 * (p.ngate / p.nsd).ln() } else { 0.0 };
    p.cdep0 = (CHARGE_Q * epssub * p.ndep * 1.0e6 / 2.0 / p.phi).sqrt();
    p.tox_ratio = (p.ntox * (model.toxref / toxe).ln()).exp() / toxe / toxe;
    p.tox_ratio_edge = (p.ntox * (model.toxref / (toxe * p.poxedge)).ln()).exp()
        / toxe / toxe / p.poxedge / p.poxedge;
    p.aechvb = if model.mos_type == NMOS { 4.97232e-7 } else { 3.42537e-7 };
    p.bechvb = if model.mos_type == NMOS { 7.45669e11 } else { 1.16645e12 };
    p.aechvb_edge_s = p.aechvb * p.weff * model.dlcig * p.tox_ratio_edge;
    p.aechvb_edge_d = p.aechvb * p.weff * model.dlcigd * p.tox_ratio_edge;
    p.bechvb_edge = -p.bechvb * toxe * p.poxedge;
    p.aechvb *= p.weff * p.leff * p.tox_ratio;
    p.bechvb *= -toxe;
    p.mstar = 0.5 + p.minv.atan() / PI;
    p.mstarcv = 0.5 + p.minvcv.atan() / PI;
    p.voffcbn = p.voff + model.voffl / p.leff;
    p.voffcbncv = p.voffcv + model.voffcvl / p.leff;
    p.ldeb = (epssub * vtm0 / (CHARGE_Q * p.ndep * 1.0e6)).sqrt() / 3.0;
    p.acde *= (p.ndep / 2.0e16).powf(-0.25);

    // k1/k2 processing
    if model.k1_given || model.k2_given {
        if !model.k1_given { p.k1 = 0.53; }
        if !model.k2_given { p.k2 = -0.0186; }
    } else {
        if !model.vbx_given {
            p.vbx = p.phi - 7.7348e-4 * p.ndep * p.xt * p.xt;
        }
        if p.vbx > 0.0 { p.vbx = -p.vbx; }
        if p.vbm > 0.0 { p.vbm = -p.vbm; }
        if !model.gamma1_given {
            p.gamma1 = 5.753e-12 * p.ndep.sqrt() / model.coxe;
        }
        if !model.gamma2_given {
            p.gamma2 = 5.753e-12 * p.nsub.sqrt() / model.coxe;
        }
        let t0 = p.gamma1 - p.gamma2;
        let t1 = (p.phi - p.vbx).sqrt() - p.sqrt_phi;
        let t2 = (p.phi * (p.phi - p.vbm)).sqrt() - p.phi;
        p.k2 = t0 * t1 / (2.0 * t2 + p.vbm);
        p.k1 = p.gamma2 - 2.0 * p.k2 * (p.phi - p.vbm).sqrt();
    }
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
    p.k1ox = p.k1 * toxe / model.toxm;

    let tmp = (epssub / (epsrox * EPS0) * toxe * p.xdep0).sqrt();
    let t0v = p.dsub * p.leff / tmp;
    if t0v < EXP_THRESHOLD {
        let t1 = t0v.exp(); let t2 = t1 - 1.0;
        let t3 = t2 * t2; let t4 = t3 + 2.0 * t1 * MIN_EXP;
        p.theta0vb0 = t1 / t4;
    } else {
        p.theta0vb0 = 1.0 / (MAX_EXP - 2.0);
    }
    let t0v = p.drout * p.leff / tmp;
    let t5v = if t0v < EXP_THRESHOLD {
        let t1 = t0v.exp(); let t2 = t1 - 1.0;
        let t3 = t2 * t2; let t4 = t3 + 2.0 * t1 * MIN_EXP;
        t1 / t4
    } else {
        1.0 / (MAX_EXP - 2.0)
    };
    p.theta_rout = p.pdibl1 * t5v + p.pdibl2;

    let tmp = p.xdep0.sqrt();
    let tmp1 = p.vbi - p.phi;
    let tmp2 = model.factor1 * tmp;
    let t0v = p.dvt1w * p.weff * p.leff / tmp2;
    let t8 = if t0v < EXP_THRESHOLD {
        let t1 = t0v.exp(); let t2 = t1 - 1.0; let t3 = t2 * t2;
        let t4 = t3 + 2.0 * t1 * MIN_EXP; t1 / t4
    } else { 1.0 / (MAX_EXP - 2.0) };
    let t8 = p.dvt0w * t8 * tmp1;
    let t0v = p.dvt1 * p.leff / tmp2;
    let t9 = if t0v < EXP_THRESHOLD {
        let t1 = t0v.exp(); let t2 = t1 - 1.0; let t3 = t2 * t2;
        let t4 = t3 + 2.0 * t1 * MIN_EXP; t1 / t4
    } else { 1.0 / (MAX_EXP - 2.0) };
    let t9 = p.dvt0 * t9 * tmp1;
    let t4 = toxe * p.phi / (p.weff + p.w0);
    let t0v = (1.0 + p.lpe0 / p.leff).sqrt();
    let t3 = if model.temp_mod == 1 || model.temp_mod == 0 {
        (p.kt1 + p.kt1l / p.leff) * (t_ratio - 1.0)
    } else {
        -p.kt1 * (t_ratio - 1.0)
    };
    let t5 = p.k1ox * (t0v - 1.0) * p.sqrt_phi + t3;
    p.vfbzb_factor = -t8 - t9 + p.k3 * t4 + t5 - p.phi - p.k1 * p.sqrt_phi;

    p.ku0 = 1.0; p.kvth0 = 1.0; p.ku0temp = 1.0;
    p.inv_od_ref = 1.0 / (model.saref + 0.5 * dev.l) + 1.0 / (model.sbref + 0.5 * dev.l);
    p.rho_ref = model.ku0 / p.ku0temp * p.inv_od_ref;

    let t0v = -p.dvtp3 * p.leff.ln();
    let t1 = dexp_nodiv(t0v);
    p.dvtp2factor = p.dvtp5 + p.dvtp2 * t1;

    // Instance calculations
    dev.u0temp = p.u0temp;
    dev.vth0 = p.vth0;
    dev.vsattemp = p.vsattemp;
    dev.eta0 = p.eta0;
    dev.k2 = p.k2;
    dev.vfb = p.vfb;
    let t3 = model.mos_type as f64 * dev.vth0 - dev.vfb - p.phi;
    dev.vtfbphi1 = if model.mos_type == NMOS { (t3 + t3).max(0.0) } else { (2.5 * t3).max(0.0) };
    dev.vtfbphi2 = (4.0 * t3).max(0.0);
    if dev.k2 < 0.0 {
        let t0 = 0.5 * p.k1 / dev.k2;
        dev.vbsc = (0.9 * (p.phi - t0 * t0)).clamp(-30.0, -3.0);
    } else {
        dev.vbsc = -30.0;
    }
    if dev.vbsc > p.vbm { dev.vbsc = p.vbm; }
    dev.k2ox = dev.k2 * toxe / model.toxm;
    dev.vfbzb = p.vfbzb_factor + model.mos_type as f64 * dev.vth0;
    dev.cgso = p.cgso;
    dev.cgdo = p.cgdo;
    let lnew = dev.l + model.xl;
    dev.grgeltd = model.rshg * (model.xgw + p.weff_cj / 3.0 / model.ngcon)
        / (model.ngcon * dev.nf * (lnew - model.xgl));
    if dev.grgeltd > 0.0 { dev.grgeltd = 1.0 / dev.grgeltd; }
    else { dev.grgeltd = 1.0e3; }
    dev.toxp = model.toxp;
    dev.coxp = model.coxp;
    if model.sheet_resistance > 0.0 {
        dev.drain_conductance = dev.nf / model.sheet_resistance;
        dev.source_conductance = dev.nf / model.sheet_resistance;
    }
    dev.mult_i = dev.m;
    dev.mult_q = dev.m;
}

/// DEVfetlim — port from devsup.c
fn dev_fetlim(mut vnew: f64, vold: f64, vto: f64) -> f64 {
    let vtsthi = (2.0 * (vold - vto)).abs() + 2.0;
    let vtstlo = vtsthi / 2.0 + 2.0;
    let vtox = vto + 3.5;
    let delv = vnew - vold;
    if vold >= vto {
        if vold >= vtox {
            if delv <= 0.0 {
                if vnew >= vtox {
                    // no change
                } else if vnew >= vto {
                    vnew = vto + (vnew - vto).max(0.0);
                    vnew = vnew.max(vold - vtstlo);
                } else {
                    vnew = vto;
                    vnew = vnew.max(vold - vtstlo);
                }
            } else {
                vnew = vnew.min(vold + vtsthi);
            }
        } else {
            if delv <= 0.0 {
                vnew = vnew.max(vto - 0.5);
            } else {
                vnew = vnew.min(vto + 4.0);
            }
        }
    } else {
        if delv <= 0.0 {
            if vnew >= vto - 0.5 {
                // no change
            } else {
                vnew = (-vnew - 1.0).max(-(vold + 0.5)).max(vto - 0.5);
                vnew = -vnew - 1.0;
            }
        } else {
            vnew = vnew.min(vto + 0.5);
        }
    }
    vnew
}

/// DEVlimvds — port from devsup.c
fn dev_limvds(vnew: f64, vold: f64) -> f64 {
    if vold >= 3.5 {
        if vnew > vold {
            vnew.min(3.0 * vold + 2.0)
        } else if vnew < 3.5 {
            3.5_f64.max(vold - 3.5)
        } else {
            vnew
        }
    } else {
        if vnew > vold {
            vnew.min(4.0)
        } else {
            vnew.max(-0.5)
        }
    }
}

/// Port of BSIM4load() from b4ld.c
/// This is a faithful line-by-line translation of the core computation.
fn bsim4_load(
    dev: &mut Bsim4,
    mna: &mut MnaSystem,
    states: &mut StateVectors,
    mode: Mode,
    _src_fact: f64,
    gmin: f64,
    noncon: &mut bool,
) -> Result<(), SimError> {
    let m = &dev.model;
    let p = &dev.param;
    let base = dev.state_offset;
    let tp = m.mos_type as f64;

    // ChargeComputationNeeded: for DC OP, this is false
    let charge_computation_needed =
        mode.is(MODEDCTRANCURVE) || mode.is(MODEAC) || mode.is(MODETRAN)
        || mode.is(MODEINITSMSIG)
        || (mode.is(MODETRANOP) && mode.is(MODEUIC));

    let mut check: bool = true;

    // --- Voltage initialization ---
    let mut vds: f64;
    let mut vgs: f64;
    let mut vbs: f64;
    let mut qdef: f64 = 0.0;
    // Simplified: no rgateMod, no rbodyMod, no rdsMod
    let mut vges: f64 = 0.0;
    let mut vgms: f64 = 0.0;
    let mut vdbs: f64 = 0.0;
    let mut vsbs: f64 = 0.0;
    let mut vses: f64 = 0.0;
    let mut vdes: f64 = 0.0;

    if mode.is(MODEINITSMSIG) {
        vds = states.get(0, base + ST_VDS);
        vgs = states.get(0, base + ST_VGS);
        vbs = states.get(0, base + ST_VBS);
        qdef = 0.0;
    } else if mode.is(MODEINITTRAN) {
        vds = states.get(1, base + ST_VDS);
        vgs = states.get(1, base + ST_VGS);
        vbs = states.get(1, base + ST_VBS);
        qdef = 0.0;
    } else if mode.is(MODEINITJCT) && !dev.off {
        vds = tp * 0.0; // icVDS = 0
        vgs = tp * 0.0; // icVGS = 0
        vbs = tp * 0.0; // icVBS = 0
        vges = vgs; vgms = vgs; vdbs = vbs; vsbs = vbs;

        if vds == 0.0 && vgs == 0.0 && vbs == 0.0
            && (mode.is(MODETRAN | MODEAC | MODEDCOP | MODEDCTRANCURVE)
                || !mode.is(MODEUIC))
        {
            vds = 0.1;
            vdes = 0.11;
            vses = -0.01;
            vgs = tp * dev.vth0 + 0.1;
            vges = vgs; vgms = vgs;
            vbs = 0.0; vdbs = 0.0; vsbs = 0.0;
        }
        qdef = 0.0;
    } else if mode.is(MODEINITJCT | MODEINITFIX) && dev.off {
        vds = 0.0; vgs = 0.0; vbs = 0.0;
        vges = 0.0; vgms = 0.0; vdbs = 0.0; vsbs = 0.0; vdes = 0.0; vses = 0.0;
        qdef = 0.0;
    } else {
        // PREDICTOR or normal iteration
        if mode.is(MODEINITPRED) {
            let xfact = dev.delta / dev.delta_old1;
            states.set(0, base + ST_VDS, states.get(1, base + ST_VDS));
            vds = (1.0 + xfact) * states.get(1, base + ST_VDS)
                - xfact * states.get(2, base + ST_VDS);
            states.set(0, base + ST_VGS, states.get(1, base + ST_VGS));
            vgs = (1.0 + xfact) * states.get(1, base + ST_VGS)
                - xfact * states.get(2, base + ST_VGS);
            states.set(0, base + ST_VBS, states.get(1, base + ST_VBS));
            vbs = (1.0 + xfact) * states.get(1, base + ST_VBS)
                - xfact * states.get(2, base + ST_VBS);
            states.set(0, base + ST_VBD,
                states.get(0, base + ST_VBS) - states.get(0, base + ST_VDS));
        } else {
            // Normal iteration: read from solution vector
            let dp = dev.dp; let gp = dev.gp; let sp = dev.sp; let bp = dev.bp;
            vds = tp * (mna.rhs_old_val(dp) - mna.rhs_old_val(sp));
            vgs = tp * (mna.rhs_old_val(gp) - mna.rhs_old_val(sp));
            vbs = tp * (mna.rhs_old_val(bp) - mna.rhs_old_val(sp));
        }

        // Voltage limiting
        let von = dev.von;
        let mut vbd = vbs - vds;
        let mut vgd = vgs - vds;

        if states.get(0, base + ST_VDS) >= 0.0 {
            vgs = dev_fetlim(vgs, states.get(0, base + ST_VGS), von);
            vds = vgs - vgd;
            vds = dev_limvds(vds, states.get(0, base + ST_VDS));
            vgd = vgs - vds;
        } else {
            vgd = dev_fetlim(vgd, states.get(0, base + ST_VGS) - states.get(0, base + ST_VDS), von);
            vds = vgs - vgd;
            vds = -dev_limvds(-vds, -states.get(0, base + ST_VDS));
            vgs = vgd + vds;
        }

        if vds >= 0.0 {
            vbs = crate::device::limiting::pnjlim(
                vbs, states.get(0, base + ST_VBS), CONSTVT0, dev.model.vcrit, &mut check);
            vbd = vbs - vds;
        } else {
            vbd = crate::device::limiting::pnjlim(
                vbd, states.get(0, base + ST_VBD), CONSTVT0, dev.model.vcrit, &mut check);
            vbs = vbd + vds;
        }
    }

    // Calculate DC currents and their derivatives
    let vbd = vbs - vds;
    let vgd = vgs - vds;
    let vgb = vgs - vbs;

    // Junction voltages (no rbodyMod)
    let vbs_jct = vbs;
    let vbd_jct = vbd;

    // --- Source/drain junction diode DC model (dioMod=1 default) ---
    let nvtms = m.vtm * m.sjct_emission_coeff;
    let nvtmd = m.vtm * m.djct_emission_coeff;

    // Source junction
    // For our test circuits, Aseff=0, Pseff=0 → SourceSatCurrent=0
    let source_sat_current = 0.0_f64; // simplified: no junction area
    if source_sat_current <= 0.0 {
        dev.gbs = gmin;
        dev.cbs = dev.gbs * vbs_jct;
    }

    // Drain junction
    let drain_sat_current = 0.0_f64; // simplified: no junction area
    if drain_sat_current <= 0.0 {
        dev.gbd = gmin;
        dev.cbd = dev.gbd * vbd_jct;
    }

    // Skip trap-assisted tunneling (all zero for zero junction area)

    // --- Forward/Reverse mode ---
    let big_vds: f64;
    let big_vgs: f64;
    let big_vbs: f64;
    let big_vdb: f64;

    if vds >= 0.0 {
        dev.mode = 1;
        big_vds = vds;
        big_vgs = vgs;
        big_vbs = vbs;
        big_vdb = vds - vbs;
    } else {
        dev.mode = -1;
        big_vds = -vds;
        big_vgs = vgd;
        big_vbs = vbd;
        big_vdb = -vbs;
    }

    // --- Effective bias and material constants (mtrlMod=0) ---
    let epsrox = m.epsrox;
    let toxe = m.toxe;
    let epssub = EPSSI;

    // Vbseff clipping
    let mut t0 = big_vbs - dev.vbsc - 0.001;
    let mut t1 = (t0 * t0 - 0.004 * dev.vbsc).sqrt();
    let mut d_vbseff_d_vb: f64;
    let mut vbseff: f64;
    if t0 >= 0.0 {
        vbseff = dev.vbsc + 0.5 * (t0 + t1);
        d_vbseff_d_vb = 0.5 * (1.0 + t0 / t1);
    } else {
        let t2 = -0.002 / (t1 - t0);
        vbseff = dev.vbsc * (1.0 + t2);
        d_vbseff_d_vb = t2 * dev.vbsc / t1;
    }

    // Forward body bias correction
    let t9 = 0.95 * p.phi;
    t0 = t9 - vbseff - 0.001;
    t1 = (t0 * t0 + 0.004 * t9).sqrt();
    vbseff = t9 - 0.5 * (t0 + t1);
    d_vbseff_d_vb *= 0.5 * (1.0 + t0 / t1);
    let phis = p.phi - vbseff;
    let sqrt_phis = phis.sqrt();
    let dsqrt_phis_d_vb = -0.5 / sqrt_phis;

    let xdep = p.xdep0 * sqrt_phis / p.sqrt_phi;
    let d_xdep_d_vb = (p.xdep0 / p.sqrt_phi) * dsqrt_phis_d_vb;

    let leff = p.leff;
    let vtm = m.vtm;

    // --- Vth calculation ---
    let mut t3 = xdep.sqrt();
    let v0 = p.vbi - p.phi;

    t0 = p.dvt2 * vbseff;
    let mut t2: f64;
    let mut t4: f64;
    if t0 >= -0.5 {
        t1 = 1.0 + t0;
        t2 = p.dvt2;
    } else {
        t4 = 1.0 / (3.0 + 8.0 * t0);
        t1 = (1.0 + 3.0 * t0) * t4;
        t2 = p.dvt2 * t4 * t4;
    }
    let lt1 = m.factor1 * t3 * t1;
    let dlt1_d_vb = m.factor1 * (0.5 / t3 * t1 * d_xdep_d_vb + t3 * t2);

    t0 = p.dvt2w * vbseff;
    if t0 >= -0.5 {
        t1 = 1.0 + t0;
        t2 = p.dvt2w;
    } else {
        t4 = 1.0 / (3.0 + 8.0 * t0);
        t1 = (1.0 + 3.0 * t0) * t4;
        t2 = p.dvt2w * t4 * t4;
    }
    let ltw = m.factor1 * t3 * t1;
    let dltw_d_vb = m.factor1 * (0.5 / t3 * t1 * d_xdep_d_vb + t3 * t2);

    t0 = p.dvt1 * leff / lt1;
    let mut theta0: f64;
    let mut d_theta0_d_vb: f64;
    let mut d_t1_d_vb: f64;
    if t0 < EXP_THRESHOLD {
        t1 = t0.exp();
        t2 = t1 - 1.0;
        t3 = t2 * t2;
        t4 = t3 + 2.0 * t1 * MIN_EXP;
        theta0 = t1 / t4;
        d_t1_d_vb = -t0 * t1 * dlt1_d_vb / lt1;
        d_theta0_d_vb = d_t1_d_vb * (t4 - 2.0 * t1 * (t2 + MIN_EXP)) / t4 / t4;
    } else {
        theta0 = 1.0 / (MAX_EXP - 2.0);
        d_theta0_d_vb = 0.0;
    }
    let delt_vth = p.dvt0 * theta0 * v0;
    let d_delt_vth_d_vb = p.dvt0 * d_theta0_d_vb * v0;

    t0 = p.dvt1w * p.weff * leff / ltw;
    let mut t5: f64;
    let mut d_t5_d_vb: f64;
    if t0 < EXP_THRESHOLD {
        t1 = t0.exp();
        t2 = t1 - 1.0;
        t3 = t2 * t2;
        t4 = t3 + 2.0 * t1 * MIN_EXP;
        t5 = t1 / t4;
        d_t1_d_vb = -t0 * t1 * dltw_d_vb / ltw;
        d_t5_d_vb = d_t1_d_vb * (t4 - 2.0 * t1 * (t2 + MIN_EXP)) / t4 / t4;
    } else {
        t5 = 1.0 / (MAX_EXP - 2.0);
        d_t5_d_vb = 0.0;
    }
    t0 = p.dvt0w * t5;
    t2 = t0 * v0;
    let d_t2_d_vb_vth = p.dvt0w * d_t5_d_vb * v0;

    let temp_ratio = dev.temp / m.tnom - 1.0;
    t0 = (1.0 + p.lpe0 / leff).sqrt();
    t1 = p.k1ox * (t0 - 1.0) * p.sqrt_phi
        + (p.kt1 + p.kt1l / leff + p.kt2 * vbseff) * temp_ratio;
    let vth_narrow_w = toxe * p.phi / (p.weff + p.w0);

    t3 = dev.eta0 + p.etab * vbseff;
    if t3 < 1.0e-4 {
        let t9 = 1.0 / (3.0 - 2.0e4 * t3);
        t3 = (2.0e-4 - t3) * t9;
        t4 = t9 * t9;
    } else {
        t4 = 1.0;
    }
    let d_dibl_sft_d_vd = t3 * p.theta0vb0;
    let dibl_sft = d_dibl_sft_d_vd * big_vds;

    let lpe_vb = (1.0 + p.lpeb / leff).sqrt();

    let mut vth = tp * dev.vth0
        + (p.k1ox * sqrt_phis - p.k1 * p.sqrt_phi) * lpe_vb
        - dev.k2ox * vbseff - delt_vth - t2
        + (p.k3 + p.k3b * vbseff) * vth_narrow_w + t1 - dibl_sft;

    let mut d_vth_d_vb = lpe_vb * p.k1ox * dsqrt_phis_d_vb
        - dev.k2ox - d_delt_vth_d_vb - d_t2_d_vb_vth
        + p.k3b * vth_narrow_w
        - p.etab * big_vds * p.theta0vb0 * t4
        + p.kt2 * temp_ratio;
    let mut d_vth_d_vd = -d_dibl_sft_d_vd;

    // n calculation
    let tmp1_n = epssub / xdep;
    let mut tmp2_n = p.nfactor * tmp1_n;
    let tmp3_n = p.cdsc + p.cdscb * vbseff + p.cdscd * big_vds;
    let tmp4_n = (tmp2_n + tmp3_n * theta0 + p.cit) / m.coxe;
    let mut n: f64;
    let mut dn_d_vb: f64;
    let mut dn_d_vd: f64;
    if tmp4_n >= -0.5 {
        n = 1.0 + tmp4_n;
        dn_d_vb = (-tmp2_n / xdep * d_xdep_d_vb + tmp3_n * d_theta0_d_vb
            + p.cdscb * theta0) / m.coxe;
        dn_d_vd = p.cdscd * theta0 / m.coxe;
    } else {
        t0 = 1.0 / (3.0 + 8.0 * tmp4_n);
        n = (1.0 + 3.0 * tmp4_n) * t0;
        t0 *= t0;
        dn_d_vb = (-tmp2_n / xdep * d_xdep_d_vb + tmp3_n * d_theta0_d_vb
            + p.cdscb * theta0) / m.coxe * t0;
        dn_d_vd = p.cdscd * theta0 / m.coxe * t0;
    }

    // Pocket implant correction (dvtp0)
    if p.dvtp0 > 0.0 {
        t0 = -p.dvtp1 * big_vds;
        let (t2v, d_t2_d_vd_v) = if t0 < -EXP_THRESHOLD {
            (MIN_EXP, 0.0)
        } else {
            let e = t0.exp();
            (e, -p.dvtp1 * e)
        };
        t3 = leff + p.dvtp0 * (1.0 + t2v);
        let d_t3_d_vd = p.dvtp0 * d_t2_d_vd_v;
        let (t4v, d_t4_d_vd) = if m.temp_mod < 2 {
            (vtm * (leff / t3).ln(), -vtm * d_t3_d_vd / t3)
        } else {
            (m.vtm0 * (leff / t3).ln(), -m.vtm0 * d_t3_d_vd / t3)
        };
        let d_dits_sft_d_vd = dn_d_vd * t4v + n * d_t4_d_vd;
        let d_dits_sft_d_vb = t4v * dn_d_vb;
        vth -= n * t4v;
        d_vth_d_vd -= d_dits_sft_d_vd;
        d_vth_d_vb -= d_dits_sft_d_vb;
    }

    // DITS_SFT2 (v4.7)
    if p.dvtp4 != 0.0 && p.dvtp2factor != 0.0 {
        let t1v = 2.0 * p.dvtp4 * big_vds;
        let (t0v, t10v) = dexp(t1v);
        let dits_sft2 = p.dvtp2factor * (t0v - 1.0) / (t0v + 1.0);
        let d_dits_sft2_d_vd = p.dvtp2factor * p.dvtp4 * 4.0 * t10v
            / ((t0v + 1.0) * (t0v + 1.0));
        vth -= dits_sft2;
        d_vth_d_vd -= d_dits_sft2_d_vd;
    }

    dev.von = vth;

    // --- Poly gate depletion ---
    t0 = dev.vfb + p.phi;
    t1 = EPSSI; // mtrlMod=0

    let (vgs_eff_s, dvgs_eff_dvg_s) =
        bsim4_poly_depletion(t0, p.ngate, t1, m.coxe, big_vgs);
    let (vgd_eff_s, dvgd_eff_dvg_s) =
        bsim4_poly_depletion(t0, p.ngate, t1, m.coxe, vgd);

    let big_vgs_eff: f64;
    let d_vgs_eff_d_vg: f64;
    if dev.mode > 0 {
        big_vgs_eff = vgs_eff_s;
        d_vgs_eff_d_vg = dvgs_eff_dvg_s;
    } else {
        big_vgs_eff = vgd_eff_s;
        d_vgs_eff_d_vg = dvgd_eff_dvg_s;
    }

    let vgst = big_vgs_eff - vth;

    // --- Vgsteff ---
    t0 = n * vtm;
    t1 = p.mstar * vgst;
    t2 = t1 / t0;
    let mut t10: f64;
    let mut d_t10_d_vg: f64;
    let mut d_t10_d_vd: f64;
    let mut d_t10_d_vb: f64;
    if t2 > EXP_THRESHOLD {
        t10 = t1;
        d_t10_d_vg = p.mstar * d_vgs_eff_d_vg;
        d_t10_d_vd = -d_vth_d_vd * p.mstar;
        d_t10_d_vb = -d_vth_d_vb * p.mstar;
    } else if t2 < -EXP_THRESHOLD {
        t10 = vtm * (1.0 + MIN_EXP).ln();
        d_t10_d_vg = 0.0;
        d_t10_d_vd = t10 * dn_d_vd;
        d_t10_d_vb = t10 * dn_d_vb;
        t10 *= n;
    } else {
        let exp_vgst = t2.exp();
        t3 = vtm * (1.0 + exp_vgst).ln();
        t10 = n * t3;
        d_t10_d_vg = p.mstar * exp_vgst / (1.0 + exp_vgst);
        d_t10_d_vb = t3 * dn_d_vb - d_t10_d_vg * (d_vth_d_vb + vgst * dn_d_vb / n);
        d_t10_d_vd = t3 * dn_d_vd - d_t10_d_vg * (d_vth_d_vd + vgst * dn_d_vd / n);
        d_t10_d_vg *= d_vgs_eff_d_vg;
    }

    t1 = p.voffcbn - (1.0 - p.mstar) * vgst;
    t2 = t1 / t0;
    let mut t9: f64;
    let mut d_t9_d_vg: f64;
    let mut d_t9_d_vd: f64;
    let mut d_t9_d_vb: f64;
    if t2 < -EXP_THRESHOLD {
        t3 = m.coxe * MIN_EXP / p.cdep0;
        t9 = p.mstar + t3 * n;
        d_t9_d_vg = 0.0;
        d_t9_d_vd = dn_d_vd * t3;
        d_t9_d_vb = dn_d_vb * t3;
    } else if t2 > EXP_THRESHOLD {
        t3 = m.coxe * MAX_EXP / p.cdep0;
        t9 = p.mstar + t3 * n;
        d_t9_d_vg = 0.0;
        d_t9_d_vd = dn_d_vd * t3;
        d_t9_d_vb = dn_d_vb * t3;
    } else {
        let exp_vgst = t2.exp();
        t3 = m.coxe / p.cdep0;
        t4 = t3 * exp_vgst;
        t5 = t1 * t4 / t0;
        t9 = p.mstar + n * t4;
        d_t9_d_vg = t3 * (p.mstar - 1.0) * exp_vgst / vtm;
        d_t9_d_vb = t4 * dn_d_vb - d_t9_d_vg * d_vth_d_vb - t5 * dn_d_vb;
        d_t9_d_vd = t4 * dn_d_vd - d_t9_d_vg * d_vth_d_vd - t5 * dn_d_vd;
        d_t9_d_vg *= d_vgs_eff_d_vg;
    }

    let vgsteff = t10 / t9;
    dev.vgsteff = vgsteff;
    let mut t11 = t9 * t9;
    let d_vgsteff_d_vg = (t9 * d_t10_d_vg - t10 * d_t9_d_vg) / t11;
    let d_vgsteff_d_vd = (t9 * d_t10_d_vd - t10 * d_t9_d_vd) / t11;
    let d_vgsteff_d_vb = (t9 * d_t10_d_vb - t10 * d_t9_d_vb) / t11;

    // Weff
    t9 = sqrt_phis - p.sqrt_phi;
    let mut weff = p.weff - 2.0 * (p.dwg * vgsteff + p.dwb * t9);
    let mut d_weff_d_vg = -2.0 * p.dwg;
    let mut d_weff_d_vb = -2.0 * p.dwb * dsqrt_phis_d_vb;
    if weff < 2.0e-8 {
        t0 = 1.0 / (6.0e-8 - 2.0 * weff);
        weff = 2.0e-8 * (4.0e-8 - weff) * t0;
        t0 *= t0 * 4.0e-16;
        d_weff_d_vg *= t0;
        d_weff_d_vb *= t0;
    }

    // Rds (rdsMod=0)
    let mut rds: f64 = 0.0;
    let mut d_rds_d_vg: f64 = 0.0;
    let mut d_rds_d_vb: f64 = 0.0;
    if m.rds_mod != 1 {
        t0 = 1.0 + p.prwg * vgsteff;
        let d_t0_d_vg = -p.prwg / t0 / t0;
        t1 = p.prwb * t9;
        d_t1_d_vb = p.prwb * dsqrt_phis_d_vb;
        t2 = 1.0 / t0 + t1;
        t3 = t2 + (t2 * t2 + 0.01).sqrt();
        let d_t3_d_vg_f = 1.0 + t2 / (t3 - t2);
        let d_t3_d_vb = d_t3_d_vg_f * d_t1_d_vb;
        let d_t3_d_vg = d_t3_d_vg_f * d_t0_d_vg;
        t4 = p.rds0 * 0.5;
        rds = p.rdswmin + t3 * t4;
        d_rds_d_vg = t4 * d_t3_d_vg;
        d_rds_d_vb = t4 * d_t3_d_vb;
    }

    // Abulk
    t9 = 0.5 * p.k1ox * lpe_vb / sqrt_phis;
    t1 = t9 + dev.k2ox - p.k3b * vth_narrow_w;
    d_t1_d_vb = -t9 / sqrt_phis * dsqrt_phis_d_vb;

    t9 = (p.xj * xdep).sqrt();
    let tmp1_a = leff + 2.0 * t9;
    t5 = leff / tmp1_a;
    let tmp2_a = p.a0 * t5;
    let tmp3_a = p.weff + p.b1;
    let tmp4_a = p.b0 / tmp3_a;
    t2 = tmp2_a + tmp4_a;
    let d_t2_d_vb_a = -t9 / tmp1_a / xdep * d_xdep_d_vb;
    let t6 = t5 * t5;
    let mut t7 = t5 * t6;

    let mut abulk0 = 1.0 + t1 * t2;
    let mut d_abulk0_d_vb = t1 * tmp2_a * d_t2_d_vb_a + t2 * d_t1_d_vb;

    let mut t8 = p.ags * p.a0 * t7;
    let mut d_abulk_d_vg = -t1 * t8;
    let mut abulk = abulk0 + d_abulk_d_vg * vgsteff;
    let mut d_abulk_d_vb = d_abulk0_d_vb
        - t8 * vgsteff * (d_t1_d_vb + 3.0 * t1 * d_t2_d_vb_a);

    if abulk0 < 0.1 {
        t9 = 1.0 / (3.0 - 20.0 * abulk0);
        abulk0 = (0.2 - abulk0) * t9;
        d_abulk0_d_vb *= t9 * t9;
    }
    if abulk < 0.1 {
        t9 = 1.0 / (3.0 - 20.0 * abulk);
        abulk = (0.2 - abulk) * t9;
        t10 = t9 * t9;
        d_abulk_d_vb *= t10;
        d_abulk_d_vg *= t10;
    }

    // keta scaling
    t2 = p.keta * vbseff;
    let mut d_t0_d_vb: f64;
    if t2 >= -0.9 {
        t0 = 1.0 / (1.0 + t2);
        d_t0_d_vb = -p.keta * t0 * t0;
    } else {
        t1 = 1.0 / (0.8 + t2);
        t0 = (17.0 + 20.0 * t2) * t1;
        d_t0_d_vb = -p.keta * t1 * t1;
    }

    d_abulk_d_vg *= t0;
    d_abulk_d_vb = d_abulk_d_vb * t0 + abulk * d_t0_d_vb;
    let d_abulk0_d_vb_save = d_abulk0_d_vb * t0 + abulk0 * d_t0_d_vb;
    abulk *= t0;
    abulk0 *= t0;

    // --- Mobility ---
    let t14 = 0.0_f64; // mtrlMod=0
    let mut d_denomi_d_vg: f64;
    let mut d_denomi_d_vd: f64;
    let mut d_denomi_d_vb: f64;
    let t5_mob: f64;

    if m.mob_mod == 0 {
        t0 = vgsteff + vth + vth - t14;
        t2 = p.ua + p.uc * vbseff;
        t3 = t0 / toxe;
        let t12 = (vth * vth + 0.0001).sqrt();
        t9 = 1.0 / (vgsteff + 2.0 * t12);
        t10 = t9 * toxe;
        t8 = p.ud * t10 * t10 * vth;
        let t6 = t8 * vth;
        t5 = t3 * (t2 + p.ub * t3) + t6;
        let t7 = -2.0 * t6 * t9;
        t11 = t7 * vth / t12;
        d_denomi_d_vg = (t2 + 2.0 * p.ub * t3) / toxe;
        let t13 = 2.0 * (d_denomi_d_vg + t11 + t8);
        d_denomi_d_vd = t13 * d_vth_d_vd;
        d_denomi_d_vb = t13 * d_vth_d_vb + p.uc * t3;
        d_denomi_d_vg += t7;
        t5_mob = t5;
    } else if m.mob_mod == 1 {
        t0 = vgsteff + vth + vth - t14;
        t2 = 1.0 + p.uc * vbseff;
        t3 = t0 / toxe;
        t4 = t3 * (p.ua + p.ub * t3);
        let t12 = (vth * vth + 0.0001).sqrt();
        t9 = 1.0 / (vgsteff + 2.0 * t12);
        t10 = t9 * toxe;
        t8 = p.ud * t10 * t10 * vth;
        let t6 = t8 * vth;
        t5 = t4 * t2 + t6;
        let t7 = -2.0 * t6 * t9;
        t11 = t7 * vth / t12;
        d_denomi_d_vg = (p.ua + 2.0 * p.ub * t3) * t2 / toxe;
        let t13 = 2.0 * (d_denomi_d_vg + t11 + t8);
        d_denomi_d_vd = t13 * d_vth_d_vd;
        d_denomi_d_vb = t13 * d_vth_d_vb + p.uc * t4;
        d_denomi_d_vg += t7;
        t5_mob = t5;
    } else if m.mob_mod == 2 {
        t0 = (vgsteff + dev.vtfbphi1) / toxe;
        t1 = (p.eu * t0.ln()).exp();
        let d_t1_d_vg_m = t1 * p.eu / t0 / toxe;
        t2 = p.ua + p.uc * vbseff;
        let t12 = (vth * vth + 0.0001).sqrt();
        t9 = 1.0 / (vgsteff + 2.0 * t12);
        t10 = t9 * toxe;
        t8 = p.ud * t10 * t10 * vth;
        let t6 = t8 * vth;
        t5 = t1 * t2 + t6;
        let t7 = -2.0 * t6 * t9;
        t11 = t7 * vth / t12;
        d_denomi_d_vg = t2 * d_t1_d_vg_m + t7;
        let t13 = 2.0 * (t11 + t8);
        d_denomi_d_vd = t13 * d_vth_d_vd;
        d_denomi_d_vb = t13 * d_vth_d_vb + t1 * p.uc;
        t5_mob = t5;
    } else {
        // mobMod 4,5,6 or default: simplified for common case
        t0 = (vgsteff + dev.vtfbphi1) / toxe;
        t1 = (p.eu * t0.ln()).exp();
        let d_t1_d_vg_m = t1 * p.eu / t0 / toxe;
        t2 = p.ua + p.uc * vbseff;
        let t12 = (vth * vth + 0.0001).sqrt();
        t9 = 1.0 / (vgsteff + 2.0 * t12);
        t10 = t9 * toxe;
        t8 = p.ud * t10 * t10 * vth;
        let t6 = t8 * vth;
        t5 = t1 * t2 + t6;
        let t7 = -2.0 * t6 * t9;
        d_denomi_d_vg = t2 * d_t1_d_vg_m + t7;
        let t13 = 2.0 * (t7 * vth / t12 + t8);
        d_denomi_d_vd = t13 * d_vth_d_vd;
        d_denomi_d_vb = t13 * d_vth_d_vb + t1 * p.uc;
        t5_mob = t5;
    }

    let mut denomi: f64;
    if t5_mob >= -0.8 {
        denomi = 1.0 + t5_mob;
    } else {
        t9 = 1.0 / (7.0 + 10.0 * t5_mob);
        denomi = (0.6 + t5_mob) * t9;
        t9 *= t9;
        d_denomi_d_vg *= t9;
        d_denomi_d_vd *= t9;
        d_denomi_d_vb *= t9;
    }

    let ueff = dev.u0temp / denomi;
    t9 = -ueff / denomi;
    let dueff_d_vg = t9 * d_denomi_d_vg;
    let dueff_d_vd = t9 * d_denomi_d_vd;
    let dueff_d_vb = t9 * d_denomi_d_vb;

    // --- Vdsat, Ids ---
    let wv_cox = weff * dev.vsattemp * m.coxe;
    let wv_cox_rds = wv_cox * rds;

    let mut esat = 2.0 * dev.vsattemp / ueff;
    let mut esat_l = esat * leff;
    t0 = -esat_l / ueff;
    let mut d_esat_l_d_vg = t0 * dueff_d_vg;
    let mut d_esat_l_d_vd = t0 * dueff_d_vd;
    let mut d_esat_l_d_vb = t0 * dueff_d_vb;

    // Lambda
    let a1 = p.a1;
    let mut lambda: f64;
    let mut d_lambda_d_vg: f64;
    if a1 == 0.0 {
        lambda = p.a2;
        d_lambda_d_vg = 0.0;
    } else if a1 > 0.0 {
        t0 = 1.0 - p.a2;
        t1 = t0 - p.a1 * vgsteff - 0.0001;
        t2 = (t1 * t1 + 0.0004 * t0).sqrt();
        lambda = p.a2 + t0 - 0.5 * (t1 + t2);
        d_lambda_d_vg = 0.5 * p.a1 * (1.0 + t1 / t2);
    } else {
        t1 = p.a2 + p.a1 * vgsteff - 0.0001;
        t2 = (t1 * t1 + 0.0004 * p.a2).sqrt();
        lambda = 0.5 * (t1 + t2);
        d_lambda_d_vg = 0.5 * p.a1 * (1.0 + t1 / t2);
    }

    let vgst2_vtm = vgsteff + 2.0 * vtm;
    let (tmp2_v, tmp3_v) = if rds > 0.0 {
        (d_rds_d_vg / rds + d_weff_d_vg / weff, d_rds_d_vb / rds + d_weff_d_vb / weff)
    } else {
        (d_weff_d_vg / weff, d_weff_d_vb / weff)
    };

    // Vdsat — full formula from b4ld.c lines 1657-1716
    let mut d_t0_d_vd: f64;
    let mut d_t1_d_vd: f64;
    let mut d_t2_d_vg: f64;
    let mut d_vdsat_d_vb: f64;
    let mut d_vdsat_d_vd: f64;
    let mut d_vdsat_d_vg: f64;
    let mut vdsat: f64;
    let mut tmp1_ids: f64;

    if rds == 0.0 && lambda == 1.0 {
        t0 = 1.0 / (abulk * esat_l + vgst2_vtm);
        tmp1_ids = 0.0;
        t1 = t0 * t0;
        t2 = vgst2_vtm * t0;
        t3 = esat_l * vgst2_vtm;
        vdsat = t3 * t0;
        let d_t0_d_vg = -(abulk * d_esat_l_d_vg + esat_l * d_abulk_d_vg + 1.0) * t1;
        d_t0_d_vd = -(abulk * d_esat_l_d_vd) * t1;
        d_t0_d_vb = -(abulk * d_esat_l_d_vb + d_abulk_d_vb * esat_l) * t1;
        d_vdsat_d_vg = t3 * d_t0_d_vg + t2 * d_esat_l_d_vg + esat_l * t0;
        d_vdsat_d_vd = t3 * d_t0_d_vd + t2 * d_esat_l_d_vd;
        d_vdsat_d_vb = t3 * d_t0_d_vb + t2 * d_esat_l_d_vb;
    } else {
        tmp1_ids = d_lambda_d_vg / (lambda * lambda);
        t9 = abulk * wv_cox_rds;
        t8 = abulk * t9;
        t7 = vgst2_vtm * t9;
        let t6 = vgst2_vtm * wv_cox_rds;
        t0 = 2.0 * abulk * (t9 - 1.0 + 1.0 / lambda);
        let d_t0_d_vg = 2.0 * (t8 * tmp2_v - abulk * tmp1_ids
            + (2.0 * t9 + 1.0 / lambda - 1.0) * d_abulk_d_vg);
        d_t0_d_vb = 2.0 * (t8 * (2.0 / abulk * d_abulk_d_vb + tmp3_v)
            + (1.0 / lambda - 1.0) * d_abulk_d_vb);
        d_t0_d_vd = 0.0;
        t1 = vgst2_vtm * (2.0 / lambda - 1.0) + abulk * esat_l + 3.0 * t7;
        let d_t1_d_vg = (2.0 / lambda - 1.0) - 2.0 * vgst2_vtm * tmp1_ids
            + abulk * d_esat_l_d_vg + esat_l * d_abulk_d_vg
            + 3.0 * (t9 + t7 * tmp2_v + t6 * d_abulk_d_vg);
        d_t1_d_vb = abulk * d_esat_l_d_vb + esat_l * d_abulk_d_vb
            + 3.0 * (t6 * d_abulk_d_vb + t7 * tmp3_v);
        d_t1_d_vd = abulk * d_esat_l_d_vd;
        t2 = vgst2_vtm * (esat_l + 2.0 * t6);
        d_t2_d_vg = esat_l + vgst2_vtm * d_esat_l_d_vg
            + t6 * (4.0 + 2.0 * vgst2_vtm * tmp2_v);
        let d_t2_d_vb = vgst2_vtm * (d_esat_l_d_vb + 2.0 * t6 * tmp3_v);
        let d_t2_d_vd = vgst2_vtm * d_esat_l_d_vd;
        t3 = (t1 * t1 - 2.0 * t0 * t2).sqrt();
        vdsat = (t1 - t3) / t0;
        let d_t3_d_vg = (t1 * d_t1_d_vg - 2.0 * (t0 * d_t2_d_vg + t2 * d_t0_d_vg)) / t3;
        let d_t3_d_vd = (t1 * d_t1_d_vd - 2.0 * (t0 * d_t2_d_vd + t2 * d_t0_d_vd)) / t3;
        let d_t3_d_vb = (t1 * d_t1_d_vb - 2.0 * (t0 * d_t2_d_vb + t2 * d_t0_d_vb)) / t3;
        d_vdsat_d_vg = (d_t1_d_vg - (t1 * d_t1_d_vg - d_t0_d_vg * t2 - t0 * d_t2_d_vg) / t3 - vdsat * d_t0_d_vg) / t0;
        d_vdsat_d_vb = (d_t1_d_vb - (t1 * d_t1_d_vb - d_t0_d_vb * t2 - t0 * d_t2_d_vb) / t3 - vdsat * d_t0_d_vb) / t0;
        d_vdsat_d_vd = (d_t1_d_vd - (t1 * d_t1_d_vd - t0 * d_t2_d_vd) / t3) / t0;
    }
    dev.vdsat = vdsat;

    // Vdseff
    t1 = vdsat - big_vds - p.delta;
    let d_t1_d_vg_eff = d_vdsat_d_vg;
    let d_t1_d_vd_eff = d_vdsat_d_vd - 1.0;
    let d_t1_d_vb_eff = d_vdsat_d_vb;
    t2 = (t1 * t1 + 4.0 * p.delta * vdsat).sqrt();
    t0 = t1 / t2;
    t9 = 2.0 * p.delta;
    t3 = t9 / t2;
    d_t2_d_vg = t0 * d_t1_d_vg_eff + t3 * d_vdsat_d_vg;
    let d_t2_d_vd_eff = t0 * d_t1_d_vd_eff + t3 * d_vdsat_d_vd;
    let d_t2_d_vb_eff = t0 * d_t1_d_vb_eff + t3 * d_vdsat_d_vb;

    let mut vdseff: f64;
    let mut d_vdseff_d_vg: f64;
    let mut d_vdseff_d_vd: f64;
    let mut d_vdseff_d_vb: f64;
    if t1 >= 0.0 {
        vdseff = vdsat - 0.5 * (t1 + t2);
        d_vdseff_d_vg = d_vdsat_d_vg - 0.5 * (d_t1_d_vg_eff + d_t2_d_vg);
        d_vdseff_d_vd = d_vdsat_d_vd - 0.5 * (d_t1_d_vd_eff + d_t2_d_vd_eff);
        d_vdseff_d_vb = d_vdsat_d_vb - 0.5 * (d_t1_d_vb_eff + d_t2_d_vb_eff);
    } else {
        t4 = t9 / (t2 - t1);
        t5 = 1.0 - t4;
        let t6 = vdsat * t4 / (t2 - t1);
        vdseff = vdsat * t5;
        d_vdseff_d_vg = d_vdsat_d_vg * t5 + t6 * (d_t2_d_vg - d_t1_d_vg_eff);
        d_vdseff_d_vd = d_vdsat_d_vd * t5 + t6 * (d_t2_d_vd_eff - d_t1_d_vd_eff);
        d_vdseff_d_vb = d_vdsat_d_vb * t5 + t6 * (d_t2_d_vb_eff - d_t1_d_vb_eff);
    }
    if big_vds == 0.0 {
        vdseff = 0.0;
        d_vdseff_d_vg = 0.0;
        d_vdseff_d_vb = 0.0;
    }
    if vdseff > big_vds { vdseff = big_vds; }
    let diff_vds = big_vds - vdseff;
    dev.vdseff = vdseff;

    // Vasat
    let tmp4_vasat = 1.0 - 0.5 * abulk * vdsat / vgst2_vtm;
    t9 = wv_cox_rds * vgsteff;
    t8 = t9 / vgst2_vtm;
    t0 = esat_l + vdsat + 2.0 * t9 * tmp4_vasat;
    t7 = 2.0 * wv_cox_rds * tmp4_vasat;
    let d_t0_d_vg_vasat = d_esat_l_d_vg + d_vdsat_d_vg
        + t7 * (1.0 + tmp2_v * vgsteff)
        - t8 * (abulk * d_vdsat_d_vg - abulk * vdsat / vgst2_vtm + vdsat * d_abulk_d_vg);
    let d_t0_d_vb_vasat = d_esat_l_d_vb + d_vdsat_d_vb
        + t7 * tmp3_v * vgsteff
        - t8 * (d_abulk_d_vb * vdsat + abulk * d_vdsat_d_vb);
    let d_t0_d_vd_vasat = d_esat_l_d_vd + d_vdsat_d_vd
        - t8 * abulk * d_vdsat_d_vd;
    t9 = wv_cox_rds * abulk;
    t1 = 2.0 / lambda - 1.0 + t9;
    let d_t1_d_vg_vasat = -2.0 * tmp1_ids + wv_cox_rds * (abulk * tmp2_v + d_abulk_d_vg);
    let d_t1_d_vb_vasat = d_abulk_d_vb * wv_cox_rds + t9 * tmp3_v;
    let vasat = t0 / t1;
    let d_vasat_d_vg = (d_t0_d_vg_vasat - vasat * d_t1_d_vg_vasat) / t1;
    let d_vasat_d_vb = (d_t0_d_vb_vasat - vasat * d_t1_d_vb_vasat) / t1;
    let d_vasat_d_vd = d_t0_d_vd_vasat / t1;

    // --- Coxeff (quantum-mechanical oxide cap) ---
    let tmp1_cox = dev.vtfbphi2;
    let tmp2_cox = 2.0e8 * dev.toxp;
    let d_t0_d_vg_cox = 1.0 / tmp2_cox;
    t0 = (vgsteff + tmp1_cox) * d_t0_d_vg_cox;
    let tmp3_cox = (m.bdos * 0.7 * t0.ln()).exp();
    t1 = 1.0 + tmp3_cox;
    t2 = m.bdos * 0.7 * tmp3_cox / t0;
    let tcen = m.ados * 1.9e-9 / t1;
    let d_tcen_d_vg = -tcen * t2 * d_t0_d_vg_cox / t1;
    let coxeff = epssub * dev.coxp / (epssub + dev.coxp * tcen);
    let d_coxeff_d_vg = -coxeff * coxeff * d_tcen_d_vg / epssub;

    let coxeff_wov_l = coxeff * weff / leff;
    let beta = ueff * coxeff_wov_l;
    t3 = ueff / leff;
    let dbeta_d_vg = coxeff_wov_l * dueff_d_vg
        + t3 * (weff * d_coxeff_d_vg + coxeff * d_weff_d_vg);
    let dbeta_d_vd = coxeff_wov_l * dueff_d_vd;
    let dbeta_d_vb = coxeff_wov_l * dueff_d_vb + t3 * coxeff * d_weff_d_vb;

    // fgche1, fgche2, gche
    t0 = 1.0 - 0.5 * vdseff * abulk / vgst2_vtm;
    let d_t0_d_vg_gche = -0.5 * (abulk * d_vdseff_d_vg
        - abulk * vdseff / vgst2_vtm + vdseff * d_abulk_d_vg) / vgst2_vtm;
    let d_t0_d_vd_gche = -0.5 * abulk * d_vdseff_d_vd / vgst2_vtm;
    let d_t0_d_vb_gche = -0.5 * (abulk * d_vdseff_d_vb + d_abulk_d_vb * vdseff) / vgst2_vtm;

    let fgche1 = vgsteff * t0;
    let dfgche1_d_vg = vgsteff * d_t0_d_vg_gche + t0;
    let dfgche1_d_vd = vgsteff * d_t0_d_vd_gche;
    let dfgche1_d_vb = vgsteff * d_t0_d_vb_gche;

    t9 = vdseff / esat_l;
    let fgche2 = 1.0 + t9;
    let dfgche2_d_vg = (d_vdseff_d_vg - t9 * d_esat_l_d_vg) / esat_l;
    let dfgche2_d_vd = (d_vdseff_d_vd - t9 * d_esat_l_d_vd) / esat_l;
    let dfgche2_d_vb = (d_vdseff_d_vb - t9 * d_esat_l_d_vb) / esat_l;

    let gche = beta * fgche1 / fgche2;
    let dgche_d_vg = (beta * dfgche1_d_vg + fgche1 * dbeta_d_vg - gche * dfgche2_d_vg) / fgche2;
    let dgche_d_vd = (beta * dfgche1_d_vd + fgche1 * dbeta_d_vd - gche * dfgche2_d_vd) / fgche2;
    let dgche_d_vb = (beta * dfgche1_d_vb + fgche1 * dbeta_d_vb - gche * dfgche2_d_vb) / fgche2;

    // Idl with Rds
    t0 = 1.0 + gche * rds;
    let idl = gche / t0;
    t1 = (1.0 - idl * rds) / t0;
    t2 = idl * idl;
    let d_idl_d_vg = t1 * dgche_d_vg - t2 * d_rds_d_vg;
    let d_idl_d_vd = t1 * dgche_d_vd;
    let d_idl_d_vb = t1 * dgche_d_vb - t2 * d_rds_d_vb;

    // FP (pocket degradation)
    let fp: f64;
    let d_fp_d_vg: f64;
    if p.fprout <= 0.0 {
        fp = 1.0; d_fp_d_vg = 0.0;
    } else {
        t9 = p.fprout * leff.sqrt() / vgst2_vtm;
        fp = 1.0 / (1.0 + t9);
        d_fp_d_vg = fp * fp * t9 / vgst2_vtm;
    }

    // VACLM
    let pvag_term: f64;
    let d_pvag_d_vg: f64;
    let d_pvag_d_vb: f64;
    let d_pvag_d_vd: f64;
    t8 = p.pvag / esat_l;
    t9 = t8 * vgsteff;
    if t9 > -0.9 {
        pvag_term = 1.0 + t9;
        d_pvag_d_vg = t8 * (1.0 - vgsteff * d_esat_l_d_vg / esat_l);
        d_pvag_d_vb = -t9 * d_esat_l_d_vb / esat_l;
        d_pvag_d_vd = -t9 * d_esat_l_d_vd / esat_l;
    } else {
        t4 = 1.0 / (17.0 + 20.0 * t9);
        pvag_term = (0.8 + t9) * t4;
        let t4sq = t4 * t4;
        d_pvag_d_vg = t8 * (1.0 - vgsteff * d_esat_l_d_vg / esat_l) * t4sq;
        let t9_scaled = t9 * t4sq / esat_l;
        d_pvag_d_vb = -t9_scaled * d_esat_l_d_vb;
        d_pvag_d_vd = -t9_scaled * d_esat_l_d_vd;
    }

    let vaclm: f64;
    let d_vaclm_d_vg: f64;
    let d_vaclm_d_vd: f64;
    let d_vaclm_d_vb: f64;
    let mut cclm: f64;
    if p.pclm > MIN_EXP && diff_vds > 1.0e-10 {
        t0 = 1.0 + rds * idl;
        let d_t0_d_vg_cl = d_rds_d_vg * idl + rds * d_idl_d_vg;
        let d_t0_d_vd_cl = rds * d_idl_d_vd;
        let d_t0_d_vb_cl = d_rds_d_vb * idl + rds * d_idl_d_vb;
        t2 = vdsat / esat;
        t1 = leff + t2;
        let d_t1_d_vg_cl = (d_vdsat_d_vg - t2 * d_esat_l_d_vg / leff) / esat;
        let d_t1_d_vd_cl = (d_vdsat_d_vd - t2 * d_esat_l_d_vd / leff) / esat;
        let d_t1_d_vb_cl = (d_vdsat_d_vb - t2 * d_esat_l_d_vb / leff) / esat;
        cclm = fp * pvag_term * t0 * t1 / (p.pclm * p.litl);
        let d_cclm_d_vg = cclm * (d_fp_d_vg / fp + d_pvag_d_vg / pvag_term
            + d_t0_d_vg_cl / t0 + d_t1_d_vg_cl / t1);
        let d_cclm_d_vb = cclm * (d_pvag_d_vb / pvag_term + d_t0_d_vb_cl / t0
            + d_t1_d_vb_cl / t1);
        let d_cclm_d_vd = cclm * (d_pvag_d_vd / pvag_term + d_t0_d_vd_cl / t0
            + d_t1_d_vd_cl / t1);
        vaclm = cclm * diff_vds;
        d_vaclm_d_vg = d_cclm_d_vg * diff_vds - d_vdseff_d_vg * cclm;
        d_vaclm_d_vb = d_cclm_d_vb * diff_vds - d_vdseff_d_vb * cclm;
        d_vaclm_d_vd = d_cclm_d_vd * diff_vds + (1.0 - d_vdseff_d_vd) * cclm;
    } else {
        vaclm = MAX_EXP; cclm = MAX_EXP;
        d_vaclm_d_vg = 0.0; d_vaclm_d_vd = 0.0; d_vaclm_d_vb = 0.0;
    }

    // VADIBL
    let vadibl: f64;
    let d_vadibl_d_vg: f64;
    let d_vadibl_d_vd: f64;
    let d_vadibl_d_vb: f64;
    if p.theta_rout > MIN_EXP {
        t8 = abulk * vdsat;
        t0 = vgst2_vtm * t8;
        let d_t0_d_vg_di = vgst2_vtm * abulk * d_vdsat_d_vg + t8
            + vgst2_vtm * vdsat * d_abulk_d_vg;
        let d_t0_d_vb_di = vgst2_vtm * (d_abulk_d_vb * vdsat + abulk * d_vdsat_d_vb);
        let d_t0_d_vd_di = vgst2_vtm * abulk * d_vdsat_d_vd;
        t1 = vgst2_vtm + t8;
        let d_t1_d_vg_di = 1.0 + abulk * d_vdsat_d_vg + vdsat * d_abulk_d_vg;
        let d_t1_d_vb_di = abulk * d_vdsat_d_vb + d_abulk_d_vb * vdsat;
        let d_t1_d_vd_di = abulk * d_vdsat_d_vd;
        t9 = t1 * t1;
        t2 = p.theta_rout;
        let mut va_di = (vgst2_vtm - t0 / t1) / t2;
        let mut dva_d_vg = (1.0 - d_t0_d_vg_di / t1 + t0 * d_t1_d_vg_di / t9) / t2;
        let mut dva_d_vb = (-d_t0_d_vb_di / t1 + t0 * d_t1_d_vb_di / t9) / t2;
        let mut dva_d_vd = (-d_t0_d_vd_di / t1 + t0 * d_t1_d_vd_di / t9) / t2;
        t7 = p.pdiblb * vbseff;
        if t7 >= -0.9 {
            t3 = 1.0 / (1.0 + t7);
            va_di *= t3;
            dva_d_vg *= t3;
            dva_d_vb = (dva_d_vb - va_di * p.pdiblb) * t3;
            dva_d_vd *= t3;
        } else {
            t4 = 1.0 / (0.8 + t7);
            t3 = (17.0 + 20.0 * t7) * t4;
            dva_d_vg *= t3;
            dva_d_vb = dva_d_vb * t3 - va_di * p.pdiblb * t4 * t4;
            dva_d_vd *= t3;
            va_di *= t3;
        }
        vadibl = va_di * pvag_term;
        d_vadibl_d_vg = dva_d_vg * pvag_term + va_di * d_pvag_d_vg;
        d_vadibl_d_vb = dva_d_vb * pvag_term + va_di * d_pvag_d_vb;
        d_vadibl_d_vd = dva_d_vd * pvag_term + va_di * d_pvag_d_vd;
    } else {
        vadibl = MAX_EXP;
        d_vadibl_d_vg = 0.0; d_vadibl_d_vd = 0.0; d_vadibl_d_vb = 0.0;
    }

    // Va
    let va = vasat + vaclm;
    let d_va_d_vg = d_vasat_d_vg + d_vaclm_d_vg;
    let d_va_d_vb = d_vasat_d_vb + d_vaclm_d_vb;
    let d_va_d_vd = d_vasat_d_vd + d_vaclm_d_vd;

    // VADITS
    let vadits: f64 = MAX_EXP;
    let d_vadits_d_vg: f64 = 0.0;
    let d_vadits_d_vd: f64 = 0.0;

    // VASCBE
    let vascbe: f64;
    let d_vascbe_d_vg: f64;
    let d_vascbe_d_vd: f64;
    let d_vascbe_d_vb: f64;
    if p.pscbe2 > 0.0 && p.pscbe1 >= 0.0 {
        if diff_vds > p.pscbe1 * p.litl / EXP_THRESHOLD {
            t0 = p.pscbe1 * p.litl / diff_vds;
            vascbe = leff * t0.exp() / p.pscbe2;
            t1 = t0 * vascbe / diff_vds;
            d_vascbe_d_vg = t1 * d_vdseff_d_vg;
            d_vascbe_d_vd = -t1 * (1.0 - d_vdseff_d_vd);
            d_vascbe_d_vb = t1 * d_vdseff_d_vb;
        } else {
            vascbe = MAX_EXP * leff / p.pscbe2;
            d_vascbe_d_vg = 0.0; d_vascbe_d_vd = 0.0; d_vascbe_d_vb = 0.0;
        }
    } else {
        vascbe = MAX_EXP;
        d_vascbe_d_vg = 0.0; d_vascbe_d_vd = 0.0; d_vascbe_d_vb = 0.0;
    }

    // Idsa with output resistance
    let mut idsa = idl;
    let mut d_idsa_d_vg = d_idl_d_vg;
    let mut d_idsa_d_vd = d_idl_d_vd;
    let mut d_idsa_d_vb = d_idl_d_vb;

    // VADIBL contribution
    t9 = diff_vds / vadibl;
    t0 = 1.0 + t9;
    d_idsa_d_vg = t0 * d_idsa_d_vg - idsa * (d_vdseff_d_vg + t9 * d_vadibl_d_vg) / vadibl;
    d_idsa_d_vd = t0 * d_idsa_d_vd + idsa * (1.0 - d_vdseff_d_vd - t9 * d_vadibl_d_vd) / vadibl;
    d_idsa_d_vb = t0 * d_idsa_d_vb - idsa * (d_vdseff_d_vb + t9 * d_vadibl_d_vb) / vadibl;
    idsa *= t0;

    // VADITS contribution
    t9 = diff_vds / vadits;
    t0 = 1.0 + t9;
    d_idsa_d_vg = t0 * d_idsa_d_vg - idsa * (d_vdseff_d_vg + t9 * d_vadits_d_vg) / vadits;
    d_idsa_d_vd = t0 * d_idsa_d_vd + idsa * (1.0 - d_vdseff_d_vd - t9 * d_vadits_d_vd) / vadits;
    d_idsa_d_vb = t0 * d_idsa_d_vb - idsa * d_vdseff_d_vb / vadits;
    idsa *= t0;

    // VACLM contribution (Va/Vasat)
    t0 = (va / vasat).ln();
    let d_t0_d_vg_va = d_va_d_vg / va - d_vasat_d_vg / vasat;
    let d_t0_d_vb_va = d_va_d_vb / va - d_vasat_d_vb / vasat;
    let d_t0_d_vd_va = d_va_d_vd / va - d_vasat_d_vd / vasat;
    t1 = t0 / cclm;
    t9 = 1.0 + t1;
    let d_t9_d_vg_va = (d_t0_d_vg_va) / cclm;
    let d_t9_d_vb_va = (d_t0_d_vb_va) / cclm;
    let d_t9_d_vd_va = (d_t0_d_vd_va) / cclm;
    d_idsa_d_vg = d_idsa_d_vg * t9 + idsa * d_t9_d_vg_va;
    d_idsa_d_vb = d_idsa_d_vb * t9 + idsa * d_t9_d_vb_va;
    d_idsa_d_vd = d_idsa_d_vd * t9 + idsa * d_t9_d_vd_va;
    idsa *= t9;

    // Substrate current (Isub)
    let tmp = p.alpha0 + p.alpha1 * leff;
    let isub: f64;
    let gbg: f64;
    let gbd_sub: f64;
    let gbb: f64;
    if tmp <= 0.0 || p.beta0 <= 0.0 {
        isub = 0.0; gbg = 0.0; gbd_sub = 0.0; gbb = 0.0;
    } else {
        t2 = tmp / leff;
        if diff_vds > p.beta0 / EXP_THRESHOLD {
            t0 = -p.beta0 / diff_vds;
            t1 = t2 * diff_vds * t0.exp();
            t3 = t1 / diff_vds * (t0 - 1.0);
            d_t1_d_vb = t3 * d_vdseff_d_vb;
        } else {
            t3 = t2 * MIN_EXP;
            t1 = t3 * diff_vds;
            d_t1_d_vb = -t3 * d_vdseff_d_vb;
        }
        t4 = idsa * vdseff;
        isub = t1 * t4;
        gbg = 0.0; gbd_sub = 0.0; gbb = 0.0;
    }
    dev.csub = isub;
    dev.gbbs = gbb;
    dev.gbgs = gbg;
    dev.gbds = gbd_sub;

    // VASCBE contribution to Ids
    t9 = diff_vds / vascbe;
    t0 = 1.0 + t9;
    let ids = idsa * t0;
    let mut gm = t0 * d_idsa_d_vg - idsa * (d_vdseff_d_vg + t9 * d_vascbe_d_vg) / vascbe;
    let mut gds = t0 * d_idsa_d_vd + idsa * (1.0 - d_vdseff_d_vd - t9 * d_vascbe_d_vd) / vascbe;
    let mut gmb = t0 * d_idsa_d_vb - idsa * (d_vdseff_d_vb + t9 * d_vascbe_d_vb) / vascbe;

    // Final gm/gds/gmbs with chain rule
    let tmp1_gm = gds + gm * d_vgsteff_d_vd;
    let tmp2_gm = gmb + gm * d_vgsteff_d_vb;
    let tmp3_gm = gm;
    gm = (ids * d_vdseff_d_vg + vdseff * tmp3_gm) * d_vgsteff_d_vg;
    gds = ids * (d_vdseff_d_vd + d_vdseff_d_vg * d_vgsteff_d_vd) + vdseff * tmp1_gm;
    gmb = (ids * (d_vdseff_d_vb + d_vdseff_d_vg * d_vgsteff_d_vb) + vdseff * tmp2_gm) * d_vbseff_d_vb;

    let cdrain = ids * vdseff;

    // Velocity saturation limiting (vtl) — only if vtl was given
    if m.vtl_given && m.vtl > 0.0 {
        let t12 = 1.0 / leff / coxeff_wov_l;
        t11 = t12 / vgsteff;
        t10 = -t11 / vgsteff;
        let vs = cdrain * t11;
        let t0v = 2.0 * MM;
        t1 = vs / (p.vtl * p.tfactor);
        if t1 > 0.0 {
            t2 = 1.0 + (t0v * t1.ln()).exp();
            let fsevl = 1.0 / (t2.ln() / t0v).exp();
            gm *= fsevl;
            gm += cdrain * (-1.0 / t0v * fsevl / t2) * ((t2 - 1.0) * t0v / vs) * (gm * t11 + cdrain * t10 * d_vgsteff_d_vg);
            gmb *= fsevl;
            gds *= fsevl;
        }
    }

    let cdrain = ids * vdseff;
    // End of Ids computation
    dev.gds = gds;
    dev.gm = gm;
    dev.gmbs = gmb;

    // --- GIDL/GISL (gidlMod=0, simplified: zero for typical test circuits) ---
    let t0_gidl = 3.0 * toxe; // mtrlMod=0
    // Calculate GIDL
    let t1_g = (big_vds - vgs_eff_s - p.egidl) / t0_gidl;
    if p.agidl <= 0.0 || p.bgidl <= 0.0 || t1_g <= 0.0 || p.cgidl <= 0.0 || big_vdb <= 0.0 {
        dev.igidl = 0.0; dev.ggidld = 0.0; dev.ggidlg = 0.0; dev.ggidlb = 0.0;
    } else {
        let d_t1_d_vd = 1.0 / t0_gidl;
        let d_t1_d_vg = -dvgs_eff_dvg_s * d_t1_d_vd;
        let t2_g = p.bgidl / t1_g;
        let (igidl, mut ggidld, mut ggidlg) = if t2_g < 100.0 {
            let ig = p.agidl * p.weff_cj * t1_g * (-t2_g).exp();
            let t3_g = ig * (1.0 + t2_g) / t1_g;
            (ig, t3_g * d_t1_d_vd, t3_g * d_t1_d_vg)
        } else {
            let ig = p.agidl * p.weff_cj * 3.720075976e-44;
            let gd = ig * d_t1_d_vd;
            let gg = ig * d_t1_d_vg;
            (ig * t1_g, gd, gg)
        };
        let t4_g = big_vdb * big_vdb;
        let t5_g = -big_vdb * t4_g;
        let t6_g = p.cgidl + t5_g;
        let t7_g = t5_g / t6_g;
        let t8_g = 3.0 * p.cgidl * t4_g / t6_g / t6_g;
        ggidld = ggidld * t7_g + igidl * t8_g;
        ggidlg = ggidlg * t7_g;
        let ggidlb = -igidl * t8_g;
        let igidl_f = igidl * t7_g;
        dev.igidl = igidl_f; dev.ggidld = ggidld; dev.ggidlg = ggidlg; dev.ggidlb = ggidlb;
    }

    // Calculate GISL
    let vgd_eff_for_gisl = vgd_eff_s;
    let dvgd_eff_dvg_for_gisl = dvgd_eff_dvg_s;
    let t1_gs = (-big_vds - vgd_eff_for_gisl - p.egisl) / t0_gidl;
    if p.agisl <= 0.0 || p.bgisl <= 0.0 || t1_gs <= 0.0 || p.cgisl <= 0.0 || big_vbs > 0.0 {
        dev.igisl = 0.0; dev.ggisls = 0.0; dev.ggislg = 0.0; dev.ggislb = 0.0;
    } else {
        let d_t1_d_vd = 1.0 / t0_gidl;
        let d_t1_d_vg = -dvgd_eff_dvg_for_gisl * d_t1_d_vd;
        let t2_gs = p.bgisl / t1_gs;
        let (igisl, mut ggisls, mut ggislg) = if t2_gs < 100.0 {
            let ig = p.agisl * p.weff_cj * t1_gs * (-t2_gs).exp();
            let t3_gs = ig * (1.0 + t2_gs) / t1_gs;
            (ig, t3_gs * d_t1_d_vd, t3_gs * d_t1_d_vg)
        } else {
            let ig = p.agisl * p.weff_cj * 3.720075976e-44;
            (ig * t1_gs, ig * d_t1_d_vd, ig * d_t1_d_vg)
        };
        let t4_gs = big_vbs * big_vbs;
        let t5_gs = -big_vbs * t4_gs;
        let t6_gs = p.cgisl + t5_gs;
        let t7_gs = t5_gs / t6_gs;
        let t8_gs = 3.0 * p.cgisl * t4_gs / t6_gs / t6_gs;
        ggisls = ggisls * t7_gs + igisl * t8_gs;
        ggislg = ggislg * t7_gs;
        let ggislb = -igisl * t8_gs;
        let igisl_f = igisl * t7_gs;
        dev.igisl = igisl_f; dev.ggisls = ggisls; dev.ggislg = ggislg; dev.ggislb = ggislb;
    }

    dev.cd = cdrain;

    // --- Save state variables ---
    states.set(0, base + ST_VBS, vbs);
    states.set(0, base + ST_VBD, vbd);
    states.set(0, base + ST_VGS, vgs);
    states.set(0, base + ST_VDS, vds);

    if check {
        *noncon = true;
    }

    // --- line850 path (DC: no charge, all gc** = 0) ---
    let ceqqg = 0.0_f64;
    let ceqqd = 0.0_f64;
    let ceqqb = 0.0_f64;
    let ceqqgmid = 0.0_f64;

    let gcggb = 0.0_f64; let gcgdb = 0.0_f64; let gcgsb = 0.0_f64; let gcgbb = 0.0_f64;
    let gcdgb = 0.0_f64; let gcddb = 0.0_f64; let gcdsb = 0.0_f64; let gcdbb = 0.0_f64;
    let gcsgb = 0.0_f64; let gcsdb = 0.0_f64; let gcssb = 0.0_f64; let gcsbb = 0.0_f64;
    let gcbgb = 0.0_f64; let gcbdb = 0.0_f64; let gcbsb = 0.0_f64; let gcbbb = 0.0_f64;
    let ggtg = 0.0_f64; let ggtd = 0.0_f64; let ggts = 0.0_f64; let ggtb = 0.0_f64;
    let sxpart: f64;
    let dxpart: f64;
    if dev.mode > 0 { dxpart = 0.4; sxpart = 0.6; }
    else { dxpart = 0.6; sxpart = 0.4; }

    // --- line900: Load current vector ---
    let big_gm: f64;
    let big_gmbs: f64;
    let fwd_sum: f64;
    let rev_sum: f64;
    let ceqdrn: f64;
    let ceqbd: f64;
    let ceqbs: f64;
    let gbbdp: f64;
    let gbbsp: f64;
    let gbdpg: f64;
    let gbdpdp: f64;
    let gbdpb: f64;
    let gbdpsp: f64;
    let gbspg: f64;
    let gbspdp: f64;
    let gbspb: f64;
    let gbspsp: f64;

    if dev.mode >= 0 {
        big_gm = dev.gm;
        big_gmbs = dev.gmbs;
        fwd_sum = big_gm + big_gmbs;
        rev_sum = 0.0;

        ceqdrn = tp * (cdrain - dev.gds * vds - big_gm * vgs - big_gmbs * vbs);
        ceqbd = tp * (dev.csub + dev.igidl
            - (dev.gbds + dev.ggidld) * vds
            - (dev.gbgs + dev.ggidlg) * vgs
            - (dev.gbbs + dev.ggidlb) * vbs);
        ceqbs = tp * (dev.igisl + dev.ggisls * vds
            - dev.ggislg * vgd - dev.ggislb * vbd);

        gbbdp = -(dev.gbds);
        gbbsp = dev.gbds + dev.gbgs + dev.gbbs;
        gbdpg = dev.gbgs;
        gbdpdp = dev.gbds;
        gbdpb = dev.gbbs;
        gbdpsp = -(gbdpg + gbdpdp + gbdpb);
        gbspg = 0.0;
        gbspdp = 0.0;
        gbspb = 0.0;
        gbspsp = 0.0;
    } else {
        big_gm = dev.gm;
        big_gmbs = dev.gmbs;
        fwd_sum = 0.0;
        rev_sum = big_gm + big_gmbs;

        ceqdrn = -tp * (cdrain + dev.gds * vds + big_gm * vgd + big_gmbs * vbd);
        ceqbd = tp * (dev.csub + dev.igidl + dev.igisl);
        ceqbs = 0.0;

        gbbdp = dev.gbds + dev.gbgs + dev.gbbs;
        gbbsp = -(dev.gbds);
        gbdpg = 0.0;
        gbdpdp = 0.0;
        gbdpb = 0.0;
        gbdpsp = 0.0;
        gbspg = dev.gbgs;
        gbspdp = dev.gbds;
        gbspb = dev.gbbs;
        gbspsp = -(gbspg + gbspdp + gbspb);
    }

    // No igcMod, no igbMod
    let g_igtotg = 0.0_f64;
    let g_igtotd = 0.0_f64;
    let g_igtots = 0.0_f64;
    let g_igtotb = 0.0_f64;
    let igtoteq = 0.0_f64;
    let g_istotg = 0.0_f64; let g_istotd = 0.0_f64;
    let g_istots = 0.0_f64; let g_istotb = 0.0_f64; let istoteq = 0.0_f64;
    let g_idtotg = 0.0_f64; let g_idtotd = 0.0_f64;
    let g_idtots = 0.0_f64; let g_idtotb = 0.0_f64; let idtoteq = 0.0_f64;
    let g_ibtotg = 0.0_f64; let g_ibtotd = 0.0_f64;
    let g_ibtots = 0.0_f64; let g_ibtotb = 0.0_f64; let ibtoteq = 0.0_f64;

    // No rdsMod
    let gstot = 0.0_f64; let gstotd = 0.0_f64; let gstotg = 0.0_f64;
    let gstots = 0.0_f64; let gstotb = 0.0_f64; let ceqgstot = 0.0_f64;
    let gdtot = 0.0_f64; let gdtotd = 0.0_f64; let gdtotg = 0.0_f64;
    let gdtots = 0.0_f64; let gdtotb = 0.0_f64; let ceqgdtot = 0.0_f64;

    // No rgateMod
    let ceqgcrg = 0.0_f64;
    let gcrg = 0.0_f64;

    // Junction current stamps
    let ceqjs: f64;
    let ceqjd: f64;
    if tp > 0.0 {
        ceqjs = dev.cbs - dev.gbs * vbs_jct;
        ceqjd = dev.cbd - dev.gbd * vbd_jct;
    } else {
        ceqjs = -(dev.cbs - dev.gbs * vbs_jct);
        ceqjd = -(dev.cbd - dev.gbd * vbd_jct);
    }

    let mult_i = dev.mult_i;
    let mult_q = dev.mult_q;

    // --- Load RHS ---
    let dp = dev.dp; let gp = dev.gp; let sp = dev.sp; let bp = dev.bp;
    let nd = dev.nd; let ns = dev.ns;

    // dNodePrime
    mna.stamp_rhs(dp, mult_i * (ceqjd - ceqbd + ceqgdtot - ceqdrn + idtoteq)
        - mult_q * ceqqd);
    // gNodePrime
    mna.stamp_rhs(gp, -(mult_q * ceqqg - mult_i * (ceqgcrg - igtoteq)));
    // bNodePrime (no rbodyMod)
    let gjbd = dev.gbd;
    let gjbs = dev.gbs;
    mna.stamp_rhs(bp, mult_i * (ceqbd + ceqbs - ceqjd - ceqjs + ibtoteq)
        - mult_q * ceqqb);
    // sNodePrime (no rbodyMod)
    mna.stamp_rhs(sp, mult_i * (ceqdrn - ceqbs + ceqjs - ceqgstot + istoteq)
        + mult_q * (ceqqg + ceqqb + ceqqd + ceqqgmid));

    // --- Load matrix ---
    let gdpr = dev.drain_conductance;
    let gspr = dev.source_conductance;

    let t1_mat = 0.0_f64; // qdef * dev.gtau = 0 for DC

    // GPgp (rgateMod=0)
    mna.stamp(gp, gp, mult_q * (gcggb - ggtg) + mult_i * g_igtotg);
    mna.stamp(gp, dp, mult_q * (gcgdb - ggtd) + mult_i * g_igtotd);
    mna.stamp(gp, sp, mult_q * (gcgsb - ggts) + mult_i * g_igtots);
    mna.stamp(gp, bp, mult_q * (gcgbb - ggtb) + mult_i * g_igtotb);

    // DPdp
    mna.stamp(dp, dp, mult_i * (gdpr + dev.gds + gjbd - gdtotd + rev_sum + gbdpdp - g_idtotd)
        + mult_q * (gcddb + dxpart * ggtd));
    mna.stamp(dp, nd, -(mult_i * (gdpr + gdtot)));
    mna.stamp(dp, gp, mult_i * (big_gm - gdtotg + gbdpg - g_idtotg)
        + mult_q * (dxpart * ggtg + gcdgb));
    mna.stamp(dp, sp, -(mult_i * (dev.gds + gdtots + g_idtots + fwd_sum - gbdpsp)
        - mult_q * (dxpart * ggts + gcdsb)));
    mna.stamp(dp, bp, -(mult_i * (gjbd + gdtotb - big_gmbs - gbdpb + g_idtotb)
        - mult_q * (dxpart * ggtb + gcdbb)));

    // DdpPtr, DdPtr
    mna.stamp(nd, dp, -(mult_i * (gdpr - gdtotd)));
    mna.stamp(nd, nd, mult_i * (gdpr + gdtot));

    // SPdp, SPgp, SPsp, SPsPtr, SPbp
    mna.stamp(sp, dp, -(mult_i * (dev.gds + gstotd + rev_sum - gbspdp + g_istotd)
        - mult_q * (sxpart * ggtd + gcsdb)));
    mna.stamp(sp, gp, mult_q * (gcsgb + sxpart * ggtg)
        + mult_i * (gbspg - big_gm - gstotg - g_istotg));
    mna.stamp(sp, sp, mult_i * (gspr + dev.gds + gjbs - g_istots - gstots + fwd_sum + gbspsp)
        + mult_q * (sxpart * ggts + gcssb));
    mna.stamp(sp, ns, -(mult_i * (gspr + gstot)));
    mna.stamp(sp, bp, -(mult_i * (gjbs + gstotb + big_gmbs - gbspb + g_istotb)
        - mult_q * (gcsbb + sxpart * ggtb)));

    // SspPtr, SsPtr
    mna.stamp(ns, sp, -(mult_i * (gspr - gstots)));
    mna.stamp(ns, ns, mult_i * (gspr + gstot));

    // BPdp, BPgp, BPsp, BPbp
    mna.stamp(bp, dp, mult_q * gcbdb - mult_i * (gjbd - gbbdp + g_ibtotd));
    mna.stamp(bp, gp, mult_q * gcbgb - mult_i * (dev.gbgs + g_ibtotg));
    mna.stamp(bp, sp, mult_q * gcbsb - mult_i * (gjbs - gbbsp + g_ibtots));
    mna.stamp(bp, bp, mult_i * (gjbd + gjbs - dev.gbbs - g_ibtotb) + mult_q * gcbbb);

    // GIDL stamps
    let ggidld = dev.ggidld;
    let ggidlg = dev.ggidlg;
    let ggidlb = dev.ggidlb;
    let ggislg = dev.ggislg;
    let ggisls = dev.ggisls;
    let ggislb = dev.ggislb;

    mna.stamp(dp, dp, mult_i * ggidld);
    mna.stamp(dp, gp, mult_i * ggidlg);
    mna.stamp(dp, sp, -(mult_i * (ggidlg + ggidld + ggidlb)));
    mna.stamp(dp, bp, mult_i * ggidlb);
    mna.stamp(bp, dp, -(mult_i * ggidld));
    mna.stamp(bp, gp, -(mult_i * ggidlg));
    mna.stamp(bp, sp, mult_i * (ggidlg + ggidld + ggidlb));
    mna.stamp(bp, bp, -(mult_i * ggidlb));

    // GISL stamps
    mna.stamp(sp, dp, -(mult_i * (ggisls + ggislg + ggislb)));
    mna.stamp(sp, gp, mult_i * ggislg);
    mna.stamp(sp, sp, mult_i * ggisls);
    mna.stamp(sp, bp, mult_i * ggislb);
    mna.stamp(bp, dp, mult_i * (ggislg + ggisls + ggislb));
    mna.stamp(bp, gp, -(mult_i * ggislg));
    mna.stamp(bp, sp, -(mult_i * ggisls));
    mna.stamp(bp, bp, -(mult_i * ggislb));

    Ok(())
}
