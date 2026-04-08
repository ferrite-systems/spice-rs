use crate::device::Device;
use crate::device::limiting::pnjlim;
use crate::error::SimError;
use crate::integration::ni_integrate;
use crate::mna::MnaSystem;
use crate::mode::*;
use crate::state::StateVectors;

// State offsets (diodefs.h:216-228)
const ST_VOLTAGE: usize = 0;
const ST_CURRENT: usize = 1;
const ST_CONDUCT: usize = 2;
const ST_CAP_CHARGE: usize = 3;
const ST_CAP_CURRENT: usize = 4;
const NUM_STATES: usize = 5; // Skip thermal states for now

/// Diode model parameters — matches ngspice DIOmodel fields.
#[derive(Debug, Clone)]
pub struct DiodeModel {
    pub is: f64,    // Saturation current (default 1e-14)
    pub n: f64,     // Emission coefficient (default 1)
    pub rs: f64,    // Series resistance (default 0)
    pub cjo: f64,   // Zero-bias junction capacitance (default 0)
    pub vj: f64,    // Junction potential (default 1.0)
    pub m: f64,     // Grading coefficient (default 0.5)
    pub tt: f64,    // Transit time (default 0)
    pub bv: f64,    // Breakdown voltage (default 0 = not given)
    pub ibv: f64,   // Breakdown current (default 1e-3)
    pub fc: f64,    // Forward-bias depletion cap coefficient (default 0.5)
    pub eg: f64,    // Bandgap energy (default 1.11 eV for Si)
}

impl Default for DiodeModel {
    fn default() -> Self {
        Self {
            is: 1e-14,
            n: 1.0,
            rs: 0.0,
            cjo: 0.0,
            vj: 1.0,
            m: 0.5,
            tt: 0.0,
            bv: 0.0,
            ibv: 1e-3,
            fc: 0.5,
            eg: 1.11,
        }
    }
}

/// Diode device — port of ngspice dio/dioload.c.
///
/// Implements the Shockley diode equation with series resistance, junction
/// capacitance, and voltage limiting (pnjlim).
#[derive(Debug)]
pub struct Diode {
    name: String,
    pos_node: usize,
    neg_node: usize,
    pos_prime: usize, // Internal node after RS (= pos_node if RS=0)
    model: DiodeModel,
    area: f64,
    last_gd: f64,
    last_cd: f64,
    last_vd: f64,
    pre_vd: f64,
    s1_vd: f64,
    s2_vd: f64,
    state_offset: usize,

    // Temperature-adjusted parameters
    t_sat_cur: f64,    // Is adjusted for temperature + area
    t_vcrit: f64,      // Critical voltage for pnjlim
    t_jct_pot: f64,    // Junction potential
    t_jct_cap: f64,    // Junction capacitance
    t_dep_cap: f64,    // FC * VJ
    t_f1: f64,         // Precomputed cap coefficient
    t_f2: f64,
    t_f3: f64,
    t_conductance: f64, // 1/RS * area
    t_brkdwn_v: f64,   // Adjusted breakdown voltage
    t_grading: f64,    // Grading coefficient

    // Device initial condition (from .IC node voltages or instance params)
    init_cond: f64,
    init_cond_given: bool,

    // Cached temperature for load()
    temp: f64,

    // Integration params (set by transient engine)
    pub ag: [f64; 7],
    pub order: usize,
    // Timestep info for DEVpred extrapolation
    pub delta: f64,
    pub delta_old1: f64,
}

impl Diode {
    pub fn new(
        name: impl Into<String>,
        pos_node: usize,
        neg_node: usize,
        pos_prime: usize,
        model: DiodeModel,
        area: f64,
    ) -> Self {
        Self {
            name: name.into(),
            pos_node,
            neg_node,
            pos_prime,
            model,
            area,
            last_gd: 0.0, last_cd: 0.0, last_vd: 0.0, pre_vd: 0.0, s1_vd: 0.0, s2_vd: 0.0,
            init_cond: 0.0, init_cond_given: false,
            state_offset: 0,
            temp: 300.15,
            t_brkdwn_v: 0.0,
            t_sat_cur: 0.0,
            t_vcrit: 0.0,
            t_jct_pot: 0.0,
            t_jct_cap: 0.0,
            t_dep_cap: 0.0,
            t_f1: 0.0,
            t_f2: 0.0,
            t_f3: 0.0,
            t_conductance: 0.0,
            t_grading: 0.0,
            ag: [0.0; 7],
            order: 1,
            delta: 0.0,
            delta_old1: 1.0, // avoid division by zero
        }
    }

    pub fn qcap(&self) -> usize {
        self.state_offset + ST_CAP_CHARGE
    }

    pub fn state_offset(&self) -> usize {
        self.state_offset
    }
}

impl Device for Diode {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn name(&self) -> &str { &self.name }

    /// DIOgetic (diogetic.c): propagate .IC node voltages to device IC.
    fn setic(&mut self, rhs: &[f64]) {
        if !self.init_cond_given {
            self.init_cond = rhs[self.pos_node] - rhs[self.neg_node];
        }
    }

    fn setup(&mut self, states: &mut StateVectors) -> usize {
        self.state_offset = states.allocate(NUM_STATES);
        NUM_STATES
    }

    fn setup_matrix(&mut self, mna: &mut crate::mna::MnaSystem) {
        let p = self.pos_node;
        let pp = self.pos_prime;
        let n = self.neg_node;
        // Pre-allocate all elements — MUST match ngspice TSTALLOC order exactly
        // (diosetup.c:371-387) to get identical TRANSLATE assignments.
        if self.model.rs > 0.0 {
            mna.make_element(p, pp);   // DIOposPosPrimePtr
            mna.make_element(n, pp);   // DIOnegPosPrimePtr
            mna.make_element(pp, p);   // DIOposPrimePosPtr
            mna.make_element(pp, n);   // DIOposPrimeNegPtr
            mna.make_element(p, p);    // DIOposPosPtr
            mna.make_element(n, n);    // DIOnegNegPtr
            mna.make_element(pp, pp);  // DIOposPrimePosPrimePtr
        } else {
            // When RS=0, pp=p, only need neg stamps
            mna.make_element(pp, pp);  // DIOposPrimePosPrimePtr (= pos,pos)
            mna.make_element(n, n);    // DIOnegNegPtr
            mna.make_element(n, pp);   // DIOnegPosPrimePtr (= neg,pos)
            mna.make_element(pp, n);   // DIOposPrimeNegPtr (= pos,neg)
        }
    }

    fn temperature(&mut self, temp: f64, tnom: f64) {
        // Port of diotemp.c — using EXACT ngspice constants and formulas
        self.temp = temp;
        let vt = BOLTZMANN_OVER_Q * temp;
        let vte = self.model.n * vt;
        let vtnom = BOLTZMANN_OVER_Q * tnom;

        // Bandgap energy (diotemp.c:58-60)
        let egfet = 1.16 - 7.02e-4 * temp * temp / (temp + 1108.0);
        let egfet1 = 1.16 - 7.02e-4 * tnom * tnom / (tnom + 1108.0);

        // Junction potential (diotemp.c:72-79) — uses CONSTboltz (Joules) and CHARGE
        let fact2 = temp / REFTEMP;
        let arg = -egfet / (2.0 * CONST_BOLTZ * temp)
            + 1.1150877 / (CONST_BOLTZ * (REFTEMP + REFTEMP));
        let pbfact = -2.0 * vt * (1.5 * fact2.ln() + CHARGE * arg);

        let fact1 = tnom / REFTEMP;
        let arg1 = -egfet1 / (CONST_BOLTZ * 2.0 * tnom)
            + 1.1150877 / (2.0 * CONST_BOLTZ * REFTEMP);
        let pbfact1 = -2.0 * vtnom * (1.5 * fact1.ln() + CHARGE * arg1);

        // Junction potential & capacitance temp adjust (diotemp.c:81-95)
        let pbo = (self.model.vj - pbfact1) / fact1;
        let gmaold = (self.model.vj - pbo) / pbo;
        self.t_jct_cap = self.model.cjo * self.area
            / (1.0 + self.model.m * (400e-6 * (tnom - REFTEMP) - gmaold));
        self.t_jct_pot = pbfact + fact2 * pbo;
        let gmanew = (self.t_jct_pot - pbo) / pbo;
        self.t_jct_cap *= 1.0 + self.model.m * (400e-6 * (temp - REFTEMP) - gmanew);

        // Saturation current temp adjust (diotemp.c:113-120)
        // Uses model->DIOactivationEnergy (EG parameter, default 1.11), NOT computed egfet1
        // arg1 = ((T/Tnom) - 1) * EG / (N*Vt)
        // arg2 = (XTI/N) * ln(T/Tnom)  where XTI=3 (saturationCurrentExp default)
        let sat_arg1 = (temp / tnom - 1.0) * self.model.eg / vte;
        let sat_arg2 = 3.0 / self.model.n * (temp / tnom).ln();
        self.t_sat_cur = self.model.is * self.area * (sat_arg1 + sat_arg2).exp();

        // Critical voltage (diotemp.c:196-197)
        // Vcrit = N*Vt * ln(N*Vt / (sqrt(2) * Is_total))
        self.t_vcrit = vte * (vte / (std::f64::consts::SQRT_2 * self.t_sat_cur)).ln();


        // Breakdown voltage iterative calculation (diotemp.c:204-240)
        // Port of ngspice's exact iteration, NOT a generic Newton
        if self.model.bv > 0.0 {
            let cbv = self.model.ibv * self.area;
            let bv = self.model.bv; // tBreakdownVoltage (no temp adjust for tlevc=0 default)
            let nbv_vt = vte; // NBV defaults to N, so NBV*vt = N*vt = vte
            let tol = 1e-3 * cbv; // CKTreltol * cbv

            // Initial guess (diotemp.c:225-226)
            let mut xbv = bv - nbv_vt * (1.0 + cbv / self.t_sat_cur).ln();

            // Iteration (diotemp.c:227-233)
            for _ in 0..25 {
                xbv = bv - nbv_vt * (cbv / self.t_sat_cur + 1.0 - xbv / vt).ln();
                let xcbv = self.t_sat_cur * (((bv - xbv) / nbv_vt).exp() - 1.0 + xbv / vt);
                if (xcbv - cbv).abs() <= tol {
                    break;
                }
            }
            self.t_brkdwn_v = xbv;
        }

        self.t_grading = self.model.m;
        self.t_dep_cap = self.model.fc * self.t_jct_pot;

        // F1, F2, F3 precomputation (diotemp.c:185-257)
        // Use exp/log to match ngspice's exact transcendental path
        let xfc = (1.0 - self.model.fc).ln(); // diotemp.c:185
        self.t_f1 = self.t_jct_pot
            * (1.0 - ((1.0 - self.t_grading) * xfc).exp())
            / (1.0 - self.t_grading);
        self.t_f2 = ((1.0 + self.t_grading) * xfc).exp(); // diotemp.c:257
        self.t_f3 = 1.0 - self.model.fc * (1.0 + self.t_grading);

        // Series resistance (diotemp.c:247-255)
        if self.model.rs != 0.0 {
            self.t_conductance = self.area / self.model.rs;
        } else {
            self.t_conductance = 0.0;
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
        let vt = BOLTZMANN_OVER_Q * self.temp;
        let vte = self.model.n * vt;
        let gspr = self.t_conductance;
        let has_bv = self.model.bv > 0.0;
        let vtebrk = vte; // NBV defaults to N
        let t_brkdwn_v = self.t_brkdwn_v;

        let p = self.pos_node;
        let pp = self.pos_prime;
        let n = self.neg_node;
        let so = self.state_offset;

        // 1. Determine junction voltage (dioload.c:131-236)
        let mut vd;
        let mut check = false;

        // ngspice dioload.c:138-155 — check order matters:
        // 1. MODEINITTRAN → read from state1
        // 2. MODEINITJCT && MODETRANOP && MODEUIC → use initCond (default 0)
        // 3. MODEINITJCT → use t_vcrit
        if mode.is(MODEINITTRAN) {
            // Read from previous state (dioload.c:141-143)
            vd = states.get(1, so + ST_VOLTAGE);
        } else if mode.is(MODEINITJCT) && mode.is(MODETRANOP) && mode.is(MODEUIC) {
            // UIC: use initial condition (dioload.c:144-146)
            vd = self.init_cond;
        } else if mode.is(MODEINITJCT) {
            // Junction initialization: force Vd to Vcrit (dioload.c:150-151)
            vd = self.t_vcrit;
        } else {
            // Normal path (dioload.c:156-188): includes MODEINITPRED and rhs_old
            if mode.is(MODEINITPRED) {
                // Save state1/state2 for parity tracing
                self.s1_vd = states.get(1, so + ST_VOLTAGE);
                self.s2_vd = states.get(2, so + ST_VOLTAGE);
                // State copies (dioload.c:159-165)
                states.set(0, so + ST_VOLTAGE, states.get(1, so + ST_VOLTAGE));
                states.set(0, so + ST_CURRENT, states.get(1, so + ST_CURRENT));
                states.set(0, so + ST_CONDUCT, states.get(1, so + ST_CONDUCT));
                // DEVpred extrapolation (dioload.c:161, devsup.c:815-823)
                let xfact = self.delta / self.delta_old1;
                vd = (1.0 + xfact) * states.get(1, so + ST_VOLTAGE)
                    - xfact * states.get(2, so + ST_VOLTAGE);
            } else {
                vd = mna.rhs_old_val(pp) - mna.rhs_old_val(n);
            }
            self.pre_vd = vd;

            // Apply pnjlim (dioload.c:219-230) — applies to BOTH paths
            let vd_old = states.get(0, so + ST_VOLTAGE);
            if has_bv && vd < (-t_brkdwn_v + 10.0 * vtebrk).min(0.0) {
                // Breakdown region limiting (dioload.c:220-226)
                let vdtemp = -(vd + t_brkdwn_v);
                let vdtemp = pnjlim(vdtemp, -(vd_old + t_brkdwn_v), vtebrk, self.t_vcrit, &mut check);
                vd = -(vdtemp + t_brkdwn_v);
            } else {
                vd = pnjlim(vd, vd_old, vte, self.t_vcrit, &mut check);
            }
        }

        // Device-level gmin (dioload.c:388-389)
        let dev_gmin = gmin;

        // 2. Evaluate Shockley equation (dioload.c:305-350)
        let (cd, gd) = if vd >= -3.0 * vte {
            // Forward region (dioload.c:305-309)
            let evd = (vd / vte).exp();
            let cd = self.t_sat_cur * (evd - 1.0);
            let gd = self.t_sat_cur * evd / vte;
            (cd, gd)
        } else if !has_bv || vd >= -(t_brkdwn_v + 10.0 * vtebrk) {
            // Reverse region — cube-root approximation (dioload.c:329-339)
            let arg = 3.0 * vte / (vd * std::f64::consts::E);
            let arg = arg * arg * arg;
            let cd = -self.t_sat_cur * (1.0 + arg);
            let gd = self.t_sat_cur * 3.0 * arg / vd;
            (cd, gd)
        } else {
            // Breakdown region (dioload.c:341-349)
            let evrev = (-(t_brkdwn_v + vd) / vtebrk).exp();
            let cd = -self.t_sat_cur * evrev;
            let gd = self.t_sat_cur * evrev / vtebrk;
            (cd, gd)
        };

        // Add device gmin (dioload.c:388-389): gd += CKTgmin; cd += CKTgmin*vd
        let cd = cd + dev_gmin * vd;
        let gd = gd + dev_gmin;

        // 3. Junction capacitance for transient (dioload.c:406-502)
        let (mut cd, mut gd) = (cd, gd);

        if mode.is(MODETRAN) || mode.is(MODEAC) {
            // Depletion charge (dioload.c:411-434)
            let czero = self.t_jct_cap;
            let (deplcharge, deplcap) = if czero > 0.0 {
                if vd < self.t_dep_cap {
                    let arg = 1.0 - vd / self.t_jct_pot;
                    // ngspice: sarg = exp(-m * log(arg)) — use exp/log path
                    let sarg = (-self.t_grading * arg.ln()).exp();
                    let q = self.t_jct_pot * czero * (1.0 - arg * sarg) / (1.0 - self.t_grading);
                    (q, czero * sarg)
                } else {
                    let czof2 = czero / self.t_f2;
                    let q = czero * self.t_f1
                        + czof2
                            * (self.t_f3 * (vd - self.t_dep_cap)
                                + (self.t_grading / (2.0 * self.t_jct_pot))
                                    * (vd * vd - self.t_dep_cap * self.t_dep_cap));
                    let c = czof2 * (self.t_f3 + self.t_grading * vd / self.t_jct_pot);
                    (q, c)
                }
            } else {
                (0.0, 0.0)
            };

            // Diffusion charge (dioload.c:436-442)
            let diffcharge = self.model.tt * cd;
            let diffcap = self.model.tt * gd;
            let capd = diffcap + deplcap;

            // Store total charge
            states.set(0, so + ST_CAP_CHARGE, diffcharge + deplcharge);

            if mode.is(MODEINITTRAN) {
                // dioload.c:502-504: copy current charge to history
                let q0 = states.get(0, so + ST_CAP_CHARGE);
                states.set(1, so + ST_CAP_CHARGE, q0);
            }
            // Note: ngspice does NOT copy state1→state0 for charge at MODEINITPRED.
            // The charge was just computed from the DEVpred vd — overwriting it
            // with the previous step's charge would be wrong.

            // Integrate (dioload.c:481-501)
            let (geq, _ceq) = ni_integrate(&self.ag, states, capd, so + ST_CAP_CHARGE, self.order);
            gd += geq;
            cd += states.get(0, so + ST_CAP_CURRENT);
        }

        // 4. Convergence check (dioload.c:520-528)
        if check {
            *noncon = true;
        }

        // 5. Store state (dioload.c:529-531)
        states.set(0, so + ST_VOLTAGE, vd);
        states.set(0, so + ST_CURRENT, cd);
        states.set(0, so + ST_CONDUCT, gd);
        self.last_gd = gd;
        self.last_cd = cd;
        self.last_vd = vd;

        // 6. Stamp matrix (dioload.c:561-588)
        // Current equation: cdeq = cd - gd*vd (Norton equivalent)
        let cdeq = cd - gd * vd;

        // RHS (dioload.c:561-569)
        mna.stamp_rhs(n, cdeq);
        mna.stamp_rhs(pp, -cdeq);

        // Conductance matrix (dioload.c:572-588)
        if gspr > 0.0 {
            // Series resistance: stamps between pos and pos_prime
            mna.stamp(p, p, gspr);
            mna.stamp(pp, pp, gd + gspr);
            mna.stamp(p, pp, -gspr);
            mna.stamp(pp, p, -gspr);
        } else {
            // No series resistance: pos_prime = pos
            mna.stamp(pp, pp, gd);
        }
        mna.stamp(n, n, gd);
        mna.stamp(n, pp, -gd);
        mna.stamp(pp, n, -gd);

        Ok(())
    }

    fn conductances(&self) -> Vec<(&str, f64)> {
        vec![("gd", self.last_gd), ("cd", self.last_cd)]
    }

    fn limited_voltages(&self) -> Vec<(&str, f64)> {
        vec![
            ("vd", self.last_vd), ("pre_vd", self.pre_vd),
            ("s1_vd", self.s1_vd), ("s2_vd", self.s2_vd),
        ]
    }

    /// Port of DIOconvTest (dioconv.c) — per-device convergence check.
    /// NOTE: Diode uses CKTrhsOld (rhs_old_val) not CKTrhs.
    fn conv_test(&self, mna: &MnaSystem, states: &StateVectors, reltol: f64, abstol: f64) -> bool {
        let so = self.state_offset;

        let vd = mna.rhs_old_val(self.pos_prime) - mna.rhs_old_val(self.neg_node);
        let delvd = vd - states.get(0, so + ST_VOLTAGE);
        let cd = states.get(0, so + ST_CURRENT);
        let cdhat = cd + states.get(0, so + ST_CONDUCT) * delvd;

        let tol = reltol * f64::max(cdhat.abs(), cd.abs()) + abstol;
        if (cdhat - cd).abs() > tol {
            return false;
        }

        true
    }

    fn model_params(&self) -> Vec<(&str, f64)> {
        vec![
            ("n", self.model.n), ("rs", self.model.rs),
            ("cjo", self.model.cjo), ("vj", self.model.vj), ("mj", self.model.m),
            ("tt", self.model.tt), ("bv", self.model.bv), ("ibv", self.model.ibv),
        ]
    }
}

// Physical constants — must match ngspice exactly (const.h)
use crate::constants::{BOLTZ as CONST_BOLTZ, CHARGE, KoverQ as BOLTZMANN_OVER_Q, REFTEMP};
