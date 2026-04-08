use crate::device::Device;
use crate::device::limiting::pnjlim;
use crate::error::SimError;
use crate::integration::ni_integrate;
use crate::mna::MnaSystem;
use crate::mode::*;
use crate::state::StateVectors;

// State offsets — matches jfetdefs.h:176-188
const ST_VGS: usize = 0;
const ST_VGD: usize = 1;
const ST_CG: usize = 2;
const ST_CD: usize = 3;
const ST_CGD: usize = 4;
const ST_GM: usize = 5;
const ST_GDS: usize = 6;
const ST_GGS: usize = 7;
const ST_GGD: usize = 8;
const ST_QGS: usize = 9;
const ST_CQGS: usize = 10;
const ST_QGD: usize = 11;
const ST_CQGD: usize = 12;
const NUM_STATES: usize = 13;

// Physical constants — same as in constants.rs
use crate::constants::{BOLTZ, CHARGE, KoverQ, REFTEMP};

/// JFET model parameters — matches ngspice JFETmodel fields (jfetdefs.h:194-263).
#[derive(Debug, Clone)]
pub struct JfetModel {
    pub jfet_type: i32, // NJF=1, PJF=-1
    pub vto: f64,       // threshold voltage (VTO, default -2)
    pub beta: f64,      // transconductance coefficient (BETA, default 1e-4)
    pub lambda: f64,    // channel-length modulation (LAMBDA, default 0)
    pub rd: f64,        // drain ohmic resistance (RD, default 0)
    pub rs: f64,        // source ohmic resistance (RS, default 0)
    pub cgs: f64,       // zero-bias G-S junction cap (CGS, default 0)
    pub cgd: f64,       // zero-bias G-D junction cap (CGD, default 0)
    pub pb: f64,        // gate junction potential (PB, default 1)
    pub is_: f64,       // gate junction saturation current (IS, default 1e-14)
    pub n: f64,         // emission coefficient (N, default 1)
    pub fc: f64,        // forward-bias depletion cap coefficient (FC, default 0.5)
    pub b: f64,         // doping profile parameter (B, default 1.0 — Sydney Uni mod)
    pub tnom: f64,      // nominal temperature (K)
    pub tnom_given: bool,
    pub tcv: f64,       // threshold voltage temp coefficient
    pub vtotc: f64,     // VTO temperature coefficient
    pub vtotc_given: bool,
    pub bex: f64,       // mobility temperature exponent
    pub betatce: f64,   // BETA exponential temperature coefficient
    pub betatce_given: bool,
    pub xti: f64,       // IS temperature exponent (XTI, default 3)
    pub xti_given: bool,
    pub eg: f64,        // bandgap energy (EG, default 1.11)
    // Derived quantities (computed in temp)
    pub drain_conduct: f64,
    pub source_conduct: f64,
    pub f2: f64,
    pub f3: f64,
    pub b_fac: f64,
}

impl Default for JfetModel {
    fn default() -> Self {
        Self {
            jfet_type: 1, // NJF
            vto: -2.0,
            beta: 1e-4,
            lambda: 0.0,
            rd: 0.0,
            rs: 0.0,
            cgs: 0.0,
            cgd: 0.0,
            pb: 1.0,
            is_: 1e-14,
            n: 1.0,
            fc: 0.5,
            b: 1.0,
            tnom: 300.15, // REFTEMP
            tnom_given: false,
            tcv: 0.0,
            vtotc: 0.0,
            vtotc_given: false,
            bex: 0.0,
            betatce: 0.0,
            betatce_given: false,
            xti: 3.0,
            xti_given: false,
            eg: 1.11,
            drain_conduct: 0.0,
            source_conduct: 0.0,
            f2: 0.0,
            f3: 0.0,
            b_fac: 0.0,
        }
    }
}

/// JFET device — port of ngspice jfet/jfetload.c.
///
/// Junction FET with Shichman-Hodges drain current model,
/// gate junction diodes, and Sydney University doping profile modification.
#[derive(Debug)]
pub struct Jfet {
    name: String,
    // External nodes
    drain_node: usize,
    gate_node: usize,
    source_node: usize,
    // Internal nodes
    drain_prime_node: usize,
    source_prime_node: usize,

    model: JfetModel,
    area: f64,
    state_offset: usize,

    // Temperature-adjusted instance parameters
    t_sat_cur: f64,
    t_gate_pot: f64,
    t_cgs: f64,
    t_cgd: f64,
    t_cor_dep_cap: f64,
    t_vcrit: f64,
    t_f1: f64,
    t_threshold: f64,
    t_beta: f64,

    // Device initial conditions (from .IC node voltages or instance params)
    ic_vds: f64,
    ic_vgs: f64,
    ic_vds_given: bool,
    ic_vgs_given: bool,

    // Cached temperature
    temp: f64,

    // Integration params (set by transient engine)
    pub ag: [f64; 7],
    pub order: usize,
    pub delta: f64,
    pub delta_old1: f64,
}

impl Jfet {
    pub fn new(
        name: impl Into<String>,
        drain_node: usize,
        gate_node: usize,
        source_node: usize,
        drain_prime_node: usize,
        source_prime_node: usize,
        model: JfetModel,
        area: f64,
    ) -> Self {
        Self {
            name: name.into(),
            drain_node,
            gate_node,
            source_node,
            drain_prime_node,
            source_prime_node,
            model,
            area,
            state_offset: 0,
            t_sat_cur: 0.0,
            t_gate_pot: 0.0,
            t_cgs: 0.0,
            t_cgd: 0.0,
            t_cor_dep_cap: 0.0,
            t_vcrit: 0.0,
            t_f1: 0.0,
            t_threshold: 0.0,
            t_beta: 0.0,
            ic_vds: 0.0, ic_vgs: 0.0,
            ic_vds_given: false, ic_vgs_given: false,
            temp: 300.15,
            ag: [0.0; 7],
            order: 1,
            delta: 0.0,
            delta_old1: 1.0,
        }
    }

    /// State offsets for LTE truncation — jfettrun.c truncates QGS and QGD.
    pub fn qcap_offsets(&self) -> [usize; 2] {
        [
            self.state_offset + ST_QGS,
            self.state_offset + ST_QGD,
        ]
    }

    pub fn state_offset(&self) -> usize {
        self.state_offset
    }
}

/// FET voltage limiter — faithful port of DEVfetlim (devsup.c:93-151).
/// Same as mosfet1::dev_fetlim but duplicated here to avoid cross-device dependency.
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

impl Device for Jfet {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn name(&self) -> &str { &self.name }

    /// JFETgetic (jfetic.c): propagate .IC node voltages to device ICs.
    fn setic(&mut self, rhs: &[f64]) {
        if !self.ic_vds_given {
            self.ic_vds = rhs[self.drain_node] - rhs[self.source_node];
        }
        if !self.ic_vgs_given {
            self.ic_vgs = rhs[self.gate_node] - rhs[self.source_node];
        }
    }

    fn setup(&mut self, states: &mut StateVectors) -> usize {
        self.state_offset = states.allocate(NUM_STATES);
        NUM_STATES
    }

    fn setup_matrix(&mut self, mna: &mut MnaSystem) {
        // Pre-allocate matrix elements — must match jfetset.c:181-195 TSTALLOC order
        let d = self.drain_node;
        let g = self.gate_node;
        let s = self.source_node;
        let dp = self.drain_prime_node;
        let sp = self.source_prime_node;

        mna.make_element(d, dp);   // JFETdrainDrainPrimePtr
        mna.make_element(g, dp);   // JFETgateDrainPrimePtr
        mna.make_element(g, sp);   // JFETgateSourcePrimePtr
        mna.make_element(s, sp);   // JFETsourceSourcePrimePtr
        mna.make_element(dp, d);   // JFETdrainPrimeDrainPtr
        mna.make_element(dp, g);   // JFETdrainPrimeGatePtr
        mna.make_element(dp, sp);  // JFETdrainPrimeSourcePrimePtr
        mna.make_element(sp, g);   // JFETsourcePrimeGatePtr
        mna.make_element(sp, s);   // JFETsourcePrimeSourcePtr
        mna.make_element(sp, dp);  // JFETsourcePrimeDrainPrimePtr
        mna.make_element(d, d);    // JFETdrainDrainPtr
        mna.make_element(g, g);    // JFETgateGatePtr
        mna.make_element(s, s);    // JFETsourceSourcePtr
        mna.make_element(dp, dp);  // JFETdrainPrimeDrainPrimePtr
        mna.make_element(sp, sp);  // JFETsourcePrimeSourcePrimePtr
    }

    fn temperature(&mut self, temp: f64, tnom: f64) {
        // Port of jfettemp.c — uses EXACT ngspice constants and formulas
        self.temp = temp;

        // Use tnom from model if given, else circuit tnom
        let model_tnom = if self.model.tnom_given { self.model.tnom } else { tnom };

        let vtnom = KoverQ * model_tnom;

        // fact1, egfet1 at model tnom (jfettemp.c:44-47)
        let fact1 = model_tnom / REFTEMP;
        let kt1 = BOLTZ * model_tnom;
        let egfet1 = 1.16 - (7.02e-4 * model_tnom * model_tnom) / (model_tnom + 1108.0);
        let arg1 = -egfet1 / (kt1 + kt1) + 1.1150877 / (BOLTZ * (REFTEMP + REFTEMP));
        let pbfact1 = -2.0 * vtnom * (1.5 * fact1.ln() + CHARGE * arg1);

        // pbo, gmaold, cjfact (jfettemp.c:49-52)
        let pbo = (self.model.pb - pbfact1) / fact1;
        let gmaold = (self.model.pb - pbo) / pbo;
        let cjfact = 1.0 / (1.0 + 0.5 * (4e-4 * (model_tnom - REFTEMP) - gmaold));

        // Drain/source conductance (jfettemp.c:54-63)
        // Note: these are also computed in setup (jfetset.c:106-115) but overwritten in temp
        let drain_conduct = if self.model.rd != 0.0 { 1.0 / self.model.rd } else { 0.0 };
        let source_conduct = if self.model.rs != 0.0 { 1.0 / self.model.rs } else { 0.0 };

        // Depletion cap coefficient clamp (jfettemp.c:64-69)
        let mut fc = self.model.fc;
        if fc > 0.95 {
            fc = 0.95;
        }

        // f2, f3 precomputation (jfettemp.c:71-73)
        let xfc = (1.0 - fc).ln();
        let f2 = ((1.0 + 0.5) * xfc).exp();
        let f3 = 1.0 - fc * (1.0 + 0.5);

        // Sydney University bFac (jfettemp.c:75-76)
        let b_fac = (1.0 - self.model.b) / (self.model.pb - self.model.vto);

        // Store model-level derived params
        self.model.drain_conduct = drain_conduct;
        self.model.source_conduct = source_conduct;
        self.model.f2 = f2;
        self.model.f3 = f3;
        self.model.b_fac = b_fac;

        // Instance-level temperature calculations (jfettemp.c:80-125)
        let vt = temp * KoverQ;
        let vtn = vt * self.model.n;
        let fact2 = temp / REFTEMP;
        let ratio1 = temp / model_tnom - 1.0;

        // Saturation current (jfettemp.c:93-97)
        if self.model.xti_given {
            self.t_sat_cur = self.model.is_
                * (ratio1 * self.model.eg / vtn).exp()
                * (ratio1 + 1.0).powf(self.model.xti);
        } else {
            self.t_sat_cur = self.model.is_ * (ratio1 * self.model.eg / vtn).exp();
        }

        // CGS, CGD temp adjust (jfettemp.c:98-99)
        self.t_cgs = self.model.cgs * cjfact;
        self.t_cgd = self.model.cgd * cjfact;

        // Gate potential temp adjust (jfettemp.c:100-108)
        let kt = BOLTZ * temp;
        let egfet = 1.16 - (7.02e-4 * temp * temp) / (temp + 1108.0);
        let arg = -egfet / (kt + kt) + 1.1150877 / (BOLTZ * (REFTEMP + REFTEMP));
        let pbfact = -2.0 * vt * (1.5 * fact2.ln() + CHARGE * arg);
        self.t_gate_pot = fact2 * pbo + pbfact;

        let gmanew = (self.t_gate_pot - pbo) / pbo;
        let cjfact1 = 1.0 + 0.5 * (4e-4 * (temp - REFTEMP) - gmanew);
        self.t_cgs *= cjfact1;
        self.t_cgd *= cjfact1;

        // corDepCap and f1 (jfettemp.c:111-113)
        self.t_cor_dep_cap = fc * self.t_gate_pot;
        self.t_f1 = self.t_gate_pot * (1.0 - ((1.0 - 0.5) * xfc).exp()) / (1.0 - 0.5);

        // vcrit (jfettemp.c:114)
        self.t_vcrit = vt * (vt / (std::f64::consts::SQRT_2 * self.t_sat_cur)).ln();

        // Threshold temp adjust (jfettemp.c:116-120)
        if self.model.vtotc_given {
            self.t_threshold = self.model.vto + self.model.vtotc * (temp - model_tnom);
        } else {
            self.t_threshold = self.model.vto - self.model.tcv * (temp - model_tnom);
        }

        // Beta temp adjust (jfettemp.c:121-125)
        if self.model.betatce_given {
            self.t_beta = self.model.beta * 1.01_f64.powf(self.model.betatce * (temp - model_tnom));
        } else {
            self.t_beta = self.model.beta * (temp / model_tnom).powf(self.model.bex);
        }
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
        let so = self.state_offset;
        let tp = self.model.jfet_type as f64; // +1 for NJF, -1 for PJF

        // DC model parameters (jfetload.c:92-96)
        let beta = self.t_beta * self.area;
        let gdpr = self.model.drain_conduct * self.area;
        let gspr = self.model.source_conduct * self.area;
        let csat = self.t_sat_cur * self.area;

        let g = self.gate_node;
        let dp = self.drain_prime_node;
        let sp = self.source_prime_node;

        // Initialization (jfetload.c:99-221)
        // icheck=1 at start; reset to 0 in else branch before pnjlim
        let mut icheck = true;
        let mut vgs;
        let mut vgd;
        let mut cghat = 0.0;
        let mut cdhat = 0.0;
        let mut do_limiting = false;

        // Match the exact if/else-if chain from jfetload.c:100-161
        if mode.is(MODEINITSMSIG) {
            // jfetload.c:100-102 — read from state0 (set during DC OP)
            vgs = states.get(0, so + ST_VGS);
            vgd = states.get(0, so + ST_VGD);
        } else if mode.is(MODEINITTRAN) {
            // jfetload.c:103-105
            vgs = states.get(1, so + ST_VGS);
            vgd = states.get(1, so + ST_VGD);
        } else if mode.is(MODEINITJCT) && mode.is(MODETRANOP) && mode.is(MODEUIC) {
            // jfetload.c:106-111 — UIC with initial conditions
            let vds = tp * self.ic_vds;
            vgs = tp * self.ic_vgs;
            vgd = vgs - vds;
        } else if mode.is(MODEINITJCT) {
            // jfetload.c:112-115 — JCT init, device not off
            // (here->JFEToff is always 0 in our parser)
            vgs = -1.0;
            vgd = -1.0;
        } else {
            // else branch (jfetload.c:120-221)
            // This handles: MODEINITPRED, MODEINITFLOAT, MODEINITFIX (with off=0)
            //
            // Note: The C code has an intermediate condition:
            //   } else if (MODEINITJCT || (MODEINITFIX && off)) { vgs=vgd=0; }
            // Since off=0 and we've already handled MODEINITJCT above,
            // MODEINITFIX falls here (reads rhs_old and applies limiting).

            if mode.is(MODEINITPRED) {
                // Predictor (jfetload.c:123-146)
                let xfact = self.delta / self.delta_old1;
                states.set(0, so + ST_VGS, states.get(1, so + ST_VGS));
                vgs = (1.0 + xfact) * states.get(1, so + ST_VGS)
                    - xfact * states.get(2, so + ST_VGS);
                states.set(0, so + ST_VGD, states.get(1, so + ST_VGD));
                vgd = (1.0 + xfact) * states.get(1, so + ST_VGD)
                    - xfact * states.get(2, so + ST_VGD);
                states.set(0, so + ST_CG, states.get(1, so + ST_CG));
                states.set(0, so + ST_CD, states.get(1, so + ST_CD));
                states.set(0, so + ST_CGD, states.get(1, so + ST_CGD));
                states.set(0, so + ST_GM, states.get(1, so + ST_GM));
                states.set(0, so + ST_GDS, states.get(1, so + ST_GDS));
                states.set(0, so + ST_GGS, states.get(1, so + ST_GGS));
                states.set(0, so + ST_GGD, states.get(1, so + ST_GGD));
            } else {
                // Compute new nonlinear branch voltages (jfetload.c:151-158)
                vgs = tp * (mna.rhs_old_val(g) - mna.rhs_old_val(sp));
                vgd = tp * (mna.rhs_old_val(g) - mna.rhs_old_val(dp));
            }

            // delvgs, delvgd, delvds (jfetload.c:162-164)
            let delvgs = vgs - states.get(0, so + ST_VGS);
            let delvgd = vgd - states.get(0, so + ST_VGD);
            let delvds = delvgs - delvgd;

            // cdhat, cghat (jfetload.c:165-171)
            cghat = states.get(0, so + ST_CG)
                + states.get(0, so + ST_GGD) * delvgd
                + states.get(0, so + ST_GGS) * delvgs;
            cdhat = states.get(0, so + ST_CD)
                + states.get(0, so + ST_GM) * delvgs
                + states.get(0, so + ST_GDS) * delvds
                - states.get(0, so + ST_GGD) * delvgd;

            // Bypass check skipped (spice-rs doesn't implement bypass)

            do_limiting = true;
        }

        // Voltage limiting (jfetload.c:209-221)
        // Only applied in the else branch, not for init paths
        if do_limiting {
            icheck = false;
            let mut ichk1 = false;
            vgs = pnjlim(vgs, states.get(0, so + ST_VGS),
                self.temp * KoverQ, self.t_vcrit, &mut icheck);
            vgd = pnjlim(vgd, states.get(0, so + ST_VGD),
                self.temp * KoverQ, self.t_vcrit, &mut ichk1);
            if ichk1 {
                icheck = true;
            }
            vgs = dev_fetlim(vgs, states.get(0, so + ST_VGS), self.t_threshold);
            vgd = dev_fetlim(vgd, states.get(0, so + ST_VGD), self.t_threshold);
        }

        self.compute_and_stamp(mna, states, mode, vgs, vgd, icheck, cghat, cdhat,
            beta, gdpr, gspr, csat, gmin, noncon)
    }

    fn model_params(&self) -> Vec<(&str, f64)> {
        vec![
            ("vto", self.model.vto),
            ("beta", self.model.beta),
            ("lambda", self.model.lambda),
            ("rd", self.model.rd),
            ("rs", self.model.rs),
            ("cgs", self.model.cgs),
            ("cgd", self.model.cgd),
            ("pb", self.model.pb),
            ("is", self.model.is_),
        ]
    }

    /// Port of JFETacLoad from jfetacld.c.
    fn ac_load(
        &mut self,
        mna: &mut MnaSystem,
        states: &crate::state::StateVectors,
        omega: f64,
    ) -> Result<(), SimError> {
        let so = self.state_offset;
        let m = 1.0; // JFETm parallel multiplier

        let gdpr = self.model.drain_conduct * self.area;
        let gspr = self.model.source_conduct * self.area;
        let gm = states.get(0, so + ST_GM);
        let gds = states.get(0, so + ST_GDS);
        let ggs = states.get(0, so + ST_GGS);
        // During SMSIG, QGS/QGD state slots hold capgs/capgd (not charges)
        let xgs = states.get(0, so + ST_QGS) * omega;
        let ggd = states.get(0, so + ST_GGD);
        let xgd = states.get(0, so + ST_QGD) * omega;

        let d = self.drain_node;
        let g = self.gate_node;
        let s = self.source_node;
        let dp = self.drain_prime_node;
        let sp = self.source_prime_node;

        // Real stamps (jfetacld.c:47-68)
        mna.stamp(d, d, m * gdpr);
        mna.stamp(g, g, m * (ggd + ggs));
        mna.stamp(s, s, m * gspr);
        mna.stamp(dp, dp, m * (gdpr + gds + ggd));
        mna.stamp(sp, sp, m * (gspr + gds + gm + ggs));
        mna.stamp(d, dp, m * (-gdpr));
        mna.stamp(g, dp, m * (-ggd));
        mna.stamp(g, sp, m * (-ggs));
        mna.stamp(s, sp, m * (-gspr));
        mna.stamp(dp, d, m * (-gdpr));
        mna.stamp(dp, g, m * (-ggd + gm));
        mna.stamp(dp, sp, m * (-gds - gm));
        mna.stamp(sp, g, m * (-ggs - gm));
        mna.stamp(sp, s, m * (-gspr));
        mna.stamp(sp, dp, m * (-gds));

        // Imaginary stamps (jfetacld.c:49,52,54)
        mna.stamp_imag(g, g, m * (xgd + xgs));
        mna.stamp_imag(dp, dp, m * xgd);
        mna.stamp_imag(sp, sp, m * xgs);
        mna.stamp_imag(g, dp, m * (-xgd));
        mna.stamp_imag(g, sp, m * (-xgs));
        mna.stamp_imag(dp, g, m * (-xgd));
        mna.stamp_imag(sp, g, m * (-xgs));

        Ok(())
    }
}

impl Jfet {
    /// Compute DC currents, derivatives, charges, and stamp the matrix/RHS.
    /// Port of jfetload.c:222-532.
    fn compute_and_stamp(
        &mut self,
        mna: &mut MnaSystem,
        states: &mut StateVectors,
        mode: Mode,
        vgs: f64,
        vgd: f64,
        icheck: bool,
        cghat: f64,
        cdhat: f64,
        beta: f64,
        gdpr: f64,
        gspr: f64,
        csat: f64,
        gmin: f64,
        noncon: &mut bool,
    ) -> Result<(), SimError> {
        let so = self.state_offset;
        let tp = self.model.jfet_type as f64;

        let g = self.gate_node;
        let dp = self.drain_prime_node;
        let sp = self.source_prime_node;
        let d = self.drain_node;
        let s = self.source_node;

        // Determine DC current and derivatives (jfetload.c:225-326)
        let vds = vgs - vgd;

        let vt_temp = self.temp * KoverQ * self.model.n;

        // Gate-source junction diode (jfetload.c:228-237)
        let (cg_gs, mut ggs) = if vgs < -3.0 * vt_temp {
            let arg_val = 3.0 * vt_temp / (vgs * std::f64::consts::E);
            let arg3 = arg_val * arg_val * arg_val;
            (-csat * (1.0 + arg3) + gmin * vgs,
             csat * 3.0 * arg3 / vgs + gmin)
        } else {
            let evgs = (vgs / vt_temp).exp();
            (csat * (evgs - 1.0) + gmin * vgs,
             csat * evgs / vt_temp + gmin)
        };

        // Gate-drain junction diode (jfetload.c:239-248)
        let (cgd_junc, mut ggd) = if vgd < -3.0 * vt_temp {
            let arg_val = 3.0 * vt_temp / (vgd * std::f64::consts::E);
            let arg3 = arg_val * arg_val * arg_val;
            (-csat * (1.0 + arg3) + gmin * vgd,
             csat * 3.0 * arg3 / vgd + gmin)
        } else {
            let evgd = (vgd / vt_temp).exp();
            (csat * (evgd - 1.0) + gmin * vgd,
             csat * evgd / vt_temp + gmin)
        };

        // Total gate current (jfetload.c:250)
        let mut cg = cg_gs + cgd_junc;

        // Sydney University JFET model — drain current (jfetload.c:253-326)
        let vto = self.t_threshold;
        let cdrain;
        let gm;
        let gds;

        if vds >= 0.0 {
            let vgst = vgs - vto;
            // Normal mode (jfetload.c:258-289)
            if vgst <= 0.0 {
                // Cutoff
                cdrain = 0.0;
                gm = 0.0;
                gds = 0.0;
            } else {
                let betap = beta * (1.0 + self.model.lambda * vds);
                let b_fac_model = self.model.b_fac;
                if vgst >= vds {
                    // Linear region (jfetload.c:273-278)
                    let apart = 2.0 * self.model.b + 3.0 * b_fac_model * (vgst - vds);
                    let cpart = vds * (vds * (b_fac_model * vds - self.model.b) + vgst * apart);
                    cdrain = betap * cpart;
                    gm = betap * vds * (apart + 3.0 * b_fac_model * vgst);
                    gds = betap * (vgst - vds) * apart
                        + beta * self.model.lambda * cpart;
                } else {
                    // Saturation region (jfetload.c:280-288)
                    let b_fac = vgst * b_fac_model;
                    gm = betap * vgst * (2.0 * self.model.b + 3.0 * b_fac);
                    let cpart = vgst * vgst * (self.model.b + b_fac);
                    cdrain = betap * cpart;
                    gds = self.model.lambda * beta * cpart;
                }
            }
        } else {
            let vgdt = vgd - vto;
            // Inverse mode (jfetload.c:291-326)
            if vgdt <= 0.0 {
                // Cutoff
                cdrain = 0.0;
                gm = 0.0;
                gds = 0.0;
            } else {
                let betap = beta * (1.0 - self.model.lambda * vds);
                let b_fac_model = self.model.b_fac;
                if vgdt + vds >= 0.0 {
                    // Linear region (jfetload.c:309-314)
                    let apart = 2.0 * self.model.b + 3.0 * b_fac_model * (vgdt + vds);
                    let cpart = vds * (-vds * (-b_fac_model * vds - self.model.b) + vgdt * apart);
                    cdrain = betap * cpart;
                    gm = betap * vds * (apart + 3.0 * b_fac_model * vgdt);
                    gds = betap * (vgdt + vds) * apart
                        - beta * self.model.lambda * cpart - gm;
                } else {
                    // Saturation region (jfetload.c:316-324)
                    let b_fac = vgdt * b_fac_model;
                    gm = -betap * vgdt * (2.0 * self.model.b + 3.0 * b_fac);
                    let cpart = vgdt * vgdt * (self.model.b + b_fac);
                    cdrain = -betap * cpart;
                    gds = self.model.lambda * beta * cpart - gm;
                }
            }
        }

        // Equivalent drain current source (jfetload.c:402)
        let mut cd = cdrain - cgd_junc;
        let mut cgd = cgd_junc;

        // Charge storage (jfetload.c:403-471)
        // ngspice guard: (MODEDCTRANCURVE | MODETRAN | MODEAC | MODEINITSMSIG)
        //                || (MODETRANOP && MODEUIC)
        if mode.is(MODEDCTRANCURVE | MODETRAN | MODEAC | MODEINITSMSIG)
            || (mode.is(MODETRANOP) && mode.is(MODEUIC))
        {
            let czgs = self.t_cgs * self.area;
            let czgd = self.t_cgd * self.area;
            let twop = self.t_gate_pot + self.t_gate_pot;
            let fcpb2 = self.t_cor_dep_cap * self.t_cor_dep_cap;
            let czgsf2 = czgs / self.model.f2;
            let czgdf2 = czgd / self.model.f2;

            // QGS charge and capgs (jfetload.c:414-424)
            let capgs = if vgs < self.t_cor_dep_cap {
                let sarg = (1.0 - vgs / self.t_gate_pot).sqrt();
                states.set(0, so + ST_QGS, twop * czgs * (1.0 - sarg));
                czgs / sarg
            } else {
                states.set(0, so + ST_QGS,
                    czgs * self.t_f1
                        + czgsf2 * (self.model.f3 * (vgs - self.t_cor_dep_cap)
                            + (vgs * vgs - fcpb2) / (twop + twop)));
                czgsf2 * (self.model.f3 + vgs / twop)
            };

            // QGD charge and capgd (jfetload.c:425-435)
            let capgd = if vgd < self.t_cor_dep_cap {
                let sarg = (1.0 - vgd / self.t_gate_pot).sqrt();
                states.set(0, so + ST_QGD, twop * czgd * (1.0 - sarg));
                czgd / sarg
            } else {
                states.set(0, so + ST_QGD,
                    czgd * self.t_f1
                        + czgdf2 * (self.model.f3 * (vgd - self.t_cor_dep_cap)
                            + (vgd * vgd - fcpb2) / (twop + twop)));
                czgdf2 * (self.model.f3 + vgd / twop)
            };

            // Small-signal parameters for AC (jfetload.c:439-444)
            // In ngspice, MODEINITSMSIG stores capgs/capgd instead of charge, then `continue`s
            // (skipping stamping). The ac_load reads these cap values from the QGS/QGD state slots.
            if !mode.is(MODETRANOP) || !mode.is(MODEUIC) {
                if mode.is(MODEINITSMSIG) {
                    // Store capgs/capgd into QGS/QGD state slots (reused for AC)
                    states.set(0, so + ST_QGS, capgs);
                    states.set(0, so + ST_QGD, capgd);
                    // Store small-signal conductances into state (needed by ac_load)
                    states.set(0, so + ST_GM, gm);
                    states.set(0, so + ST_GDS, gds);
                    states.set(0, so + ST_GGS, ggs);
                    states.set(0, so + ST_GGD, ggd);
                    return Ok(());  // Skip stamping — equivalent to ngspice `continue`
                }
            }

            // Transient analysis (jfetload.c:446-471)
            if !mode.is(MODETRANOP) || !mode.is(MODEUIC) {
                if mode.is(MODEINITTRAN) {
                    states.set(1, so + ST_QGS, states.get(0, so + ST_QGS));
                    states.set(1, so + ST_QGD, states.get(0, so + ST_QGD));
                }

                // Integrate QGS (jfetload.c:455-458)
                let (geq_gs, _ceq_gs) = ni_integrate(&self.ag, states, capgs, so + ST_QGS, self.order);
                ggs = ggs + geq_gs;
                cg = cg + states.get(0, so + ST_CQGS);

                // Integrate QGD (jfetload.c:459-462)
                let (geq_gd, _ceq_gd) = ni_integrate(&self.ag, states, capgd, so + ST_QGD, self.order);
                ggd = ggd + geq_gd;
                cg = cg + states.get(0, so + ST_CQGD);
                cd = cd - states.get(0, so + ST_CQGD);
                cgd = cgd + states.get(0, so + ST_CQGD);

                if mode.is(MODEINITTRAN) {
                    states.set(1, so + ST_CQGS, states.get(0, so + ST_CQGS));
                    states.set(1, so + ST_CQGD, states.get(0, so + ST_CQGD));
                }
            }
        }

        // Convergence check (jfetload.c:476-486)
        if !mode.is(MODEINITFIX) || !mode.is(MODEUIC) {
            if icheck
                || (cghat - cg).abs()
                    >= 1e-3 * f64::max(cghat.abs(), cg.abs()) + 1e-12
                || (cdhat - cd).abs()
                    > 1e-3 * f64::max(cdhat.abs(), cd.abs()) + 1e-12
            {
                *noncon = true;
            }
        }

        // Store state (jfetload.c:487-495)
        states.set(0, so + ST_VGS, vgs);
        states.set(0, so + ST_VGD, vgd);
        states.set(0, so + ST_CG, cg);
        states.set(0, so + ST_CD, cd);
        states.set(0, so + ST_CGD, cgd);
        states.set(0, so + ST_GM, gm);
        states.set(0, so + ST_GDS, gds);
        states.set(0, so + ST_GGS, ggs);
        states.set(0, so + ST_GGD, ggd);

        // Load current vector (jfetload.c:501-510)
        // m = here->JFETm — parallel multiplier (always 1 for us)
        let m = 1.0;
        let ceqgd = tp * (cgd - ggd * vgd);
        let ceqgs = tp * ((cg - cgd) - ggs * vgs);
        let cdreq = tp * ((cd + cgd) - gds * vds - gm * vgs);

        mna.stamp_rhs(g, m * (-ceqgs - ceqgd));
        mna.stamp_rhs(dp, m * (-cdreq + ceqgd));
        mna.stamp_rhs(sp, m * (cdreq + ceqgs));

        // Load Y matrix (jfetload.c:514-528)
        mna.stamp(d, dp, m * (-gdpr));
        mna.stamp(g, dp, m * (-ggd));
        mna.stamp(g, sp, m * (-ggs));
        mna.stamp(s, sp, m * (-gspr));
        mna.stamp(dp, d, m * (-gdpr));
        mna.stamp(dp, g, m * (gm - ggd));
        mna.stamp(dp, sp, m * (-gds - gm));
        mna.stamp(sp, g, m * (-ggs - gm));
        mna.stamp(sp, s, m * (-gspr));
        mna.stamp(sp, dp, m * (-gds));
        mna.stamp(d, d, m * gdpr);
        mna.stamp(g, g, m * (ggd + ggs));
        mna.stamp(s, s, m * gspr);
        mna.stamp(dp, dp, m * (gdpr + gds + ggd));
        mna.stamp(sp, sp, m * (gspr + gds + gm + ggs));

        Ok(())
    }

}
