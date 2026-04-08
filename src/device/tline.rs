use crate::device::Device;
use crate::error::SimError;
use crate::mna::MnaSystem;
use crate::mode::*;
use crate::state::StateVectors;

/// Lossless transmission line — port of ngspice TRA device.
///
/// `T name port1+ port1- port2+ port2- Z0=val TD=val`
///
/// Has 4 external nodes, 2 internal nodes, and 2 branch equations.
/// DC model: two independent voltage sources (straight-through connection).
/// AC model: complex Y-parameter admittance stamps.
/// Transient model: delayed companion model with interpolated history.
///
/// Reference: vendor/ngspice/src/spicelib/devices/tra/
#[derive(Debug)]
pub struct TransmissionLine {
    name: String,

    // External nodes (from netlist)
    pub pos_node1: usize,
    pub neg_node1: usize,
    pub pos_node2: usize,
    pub neg_node2: usize,

    // Internal nodes (created in setup) — trasetup.c
    pub int_node1: usize,
    pub int_node2: usize,

    // Branch equations (created in setup) — trasetup.c
    pub br_eq1: usize,
    pub br_eq2: usize,

    // Parameters
    pub imped: f64,      // Z0 — characteristic impedance
    pub conduct: f64,    // 1/Z0 — computed in temperature()
    pub td: f64,         // propagation delay
    pub td_given: bool,  // whether TD was explicitly given
    pub nl: f64,         // normalized length (default 0.25)
    pub freq: f64,       // frequency for NL (default 1e9)

    // Runtime state set by transient engine (like VoltageSource.time)
    pub time: f64,       // current simulation time (CKTtime)
    pub gmin: f64,       // device gmin (CKTgmin, default 1e-12)

    // Transient state
    pub input1: f64,     // accumulated excitation for port 1
    pub input2: f64,     // accumulated excitation for port 2
    pub delays: Vec<f64>,  // delayed values: [t, input1, input2] triples
    pub size_delay: usize, // number of active delay entries (last valid index)

    // Initial conditions (not commonly used but present in ngspice)
    pub init_volt1: f64,
    pub init_cur1: f64,
    pub init_volt2: f64,
    pub init_cur2: f64,

    // Tolerances for breakpoint setting
    pub reltol: f64,
    pub abstol: f64,
}

impl TransmissionLine {
    pub fn new(
        name: &str,
        pos_node1: usize,
        neg_node1: usize,
        pos_node2: usize,
        neg_node2: usize,
        int_node1: usize,
        int_node2: usize,
        br_eq1: usize,
        br_eq2: usize,
        imped: f64,
        td: f64,
        td_given: bool,
        nl: f64,
        freq: f64,
    ) -> Self {
        Self {
            name: name.to_string(),
            pos_node1,
            neg_node1,
            pos_node2,
            neg_node2,
            int_node1,
            int_node2,
            br_eq1,
            br_eq2,
            imped,
            conduct: 0.0, // computed in temperature()
            td,
            td_given,
            nl,
            freq,
            time: 0.0,
            gmin: 1e-12,  // CKTgmin default
            input1: 0.0,
            input2: 0.0,
            delays: vec![0.0; 15], // trasetup.c: TMALLOC(double, 15)
            size_delay: 0,
            init_volt1: 0.0,
            init_cur1: 0.0,
            init_volt2: 0.0,
            init_cur2: 0.0,
            reltol: 1.0,  // trasetup.c default
            abstol: 1.0,  // trasetup.c default
        }
    }
}

impl Device for TransmissionLine {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn name(&self) -> &str { &self.name }

    fn setup_matrix(&mut self, mna: &mut MnaSystem) {
        // Port of trasetup.c TSTALLOC — 22 matrix elements
        let p1 = self.pos_node1;
        let n1 = self.neg_node1;
        let p2 = self.pos_node2;
        let n2 = self.neg_node2;
        let i1 = self.int_node1;
        let i2 = self.int_node2;
        let b1 = self.br_eq1;
        let b2 = self.br_eq2;

        mna.make_element(b1, b2);
        mna.make_element(b1, i1);
        mna.make_element(b1, n1);
        mna.make_element(b1, n2);
        mna.make_element(b1, p2);
        mna.make_element(b2, b1);
        mna.make_element(b2, i2);
        mna.make_element(b2, n1);
        mna.make_element(b2, n2);
        mna.make_element(b2, p1);
        mna.make_element(i1, b1);
        mna.make_element(i1, i1);
        mna.make_element(i1, p1);
        mna.make_element(i2, b2);
        mna.make_element(i2, i2);
        mna.make_element(i2, p2);
        mna.make_element(n1, b1);
        mna.make_element(n2, b2);
        mna.make_element(p1, i1);
        mna.make_element(p1, p1);
        mna.make_element(p2, i2);
        mna.make_element(p2, p2);
    }

    fn temperature(&mut self, _temp: f64, _tnom: f64) {
        // Port of tratemp.c
        if !self.td_given {
            self.td = self.nl / self.freq;
        }
        self.conduct = 1.0 / self.imped;
    }

    fn load(
        &mut self,
        mna: &mut MnaSystem,
        _states: &mut StateVectors,
        mode: Mode,
        _src_fact: f64,
        _gmin: f64,
        _noncon: &mut bool,
    ) -> Result<(), SimError> {
        let p1 = self.pos_node1;
        let n1 = self.neg_node1;
        let p2 = self.pos_node2;
        let n2 = self.neg_node2;
        let i1 = self.int_node1;
        let i2 = self.int_node2;
        let b1 = self.br_eq1;
        let b2 = self.br_eq2;
        let g = self.conduct;

        // Common stamps (traload.c:36-51) — same for DC and transient
        mna.stamp(p1, p1,  g);
        mna.stamp(p1, i1, -g);
        mna.stamp(n1, b1, -1.0);
        mna.stamp(p2, p2,  g);
        mna.stamp(n2, b2, -1.0);
        mna.stamp(i1, p1, -g);
        mna.stamp(i1, i1,  g);
        mna.stamp(i1, b1,  1.0);
        mna.stamp(i2, i2,  g);
        mna.stamp(i2, b2,  1.0);
        mna.stamp(b1, n1, -1.0);
        mna.stamp(b1, i1,  1.0);
        mna.stamp(b2, n2, -1.0);
        mna.stamp(b2, i2,  1.0);
        mna.stamp(p2, i2, -g);
        mna.stamp(i2, p2, -g);

        if mode.is(MODEDC) {
            // DC mode (traload.c:53-59)
            // Straight-through connection with gmin coupling
            mna.stamp(b1, p2, -1.0);
            mna.stamp(b1, n2,  1.0);
            mna.stamp(b1, b2, -(1.0 - self.gmin) * self.imped);
            mna.stamp(b2, p1, -1.0);
            mna.stamp(b2, n1,  1.0);
            mna.stamp(b2, b1, -(1.0 - self.gmin) * self.imped);
        } else {
            // Transient mode (traload.c:60-146)
            if mode.is(MODEINITTRAN) {
                // Initialize delay table (traload.c:68-86)
                if mode.is(MODEUIC) {
                    self.input1 = self.init_volt2 + self.init_cur2 * self.imped;
                    self.input2 = self.init_volt1 + self.init_cur1 * self.imped;
                } else {
                    // Use DC operating point values
                    self.input1 =
                        (mna.rhs_old_val(p2) - mna.rhs_old_val(n2))
                        + (mna.rhs_old_val(b2) * self.imped);
                    self.input2 =
                        (mna.rhs_old_val(p1) - mna.rhs_old_val(n1))
                        + (mna.rhs_old_val(b1) * self.imped);
                }

                // Initialize delay table with 3 entries at times -2*td, -td, 0
                // (traload.c:79-86)
                self.delays[0] = -2.0 * self.td;
                self.delays[3] = -self.td;
                self.delays[6] = 0.0;
                self.delays[1] = self.input1;
                self.delays[4] = self.input1;
                self.delays[7] = self.input1;
                self.delays[2] = self.input2;
                self.delays[5] = self.input2;
                self.delays[8] = self.input2;
                self.size_delay = 2;
            } else if mode.is(MODEINITPRED) {
                // Interpolate delayed values (traload.c:88-141)
                let time = self.time;

                // Find the right triple of delay entries for interpolation
                // (traload.c:89-91)
                let mut i = 2usize;
                while i < self.size_delay
                    && self.delays[3 * i] <= (time - self.td)
                {
                    i += 1;
                }

                let t1 = self.delays[3 * (i - 2)];
                let t2 = self.delays[3 * (i - 1)];
                let t3 = self.delays[3 * i];

                if (t2 - t1) == 0.0 || (t3 - t2) == 0.0 {
                    // Skip — degenerate (traload.c:95 — continue in ngspice)
                    // Still stamp RHS with previous values
                    mna.stamp_rhs(b1, self.input1);
                    mna.stamp_rhs(b2, self.input2);
                    return Ok(());
                }

                // Quadratic (Lagrange) interpolation coefficients
                // (traload.c:96-125)
                let td_time = time - self.td;
                let mut f1 = (td_time - t2) * (td_time - t3);
                let mut f2 = (td_time - t1) * (td_time - t3);
                let mut f3 = (td_time - t1) * (td_time - t2);

                if (t2 - t1) == 0.0 {
                    f1 = 0.0;
                    f2 = 0.0;
                } else {
                    f1 /= t1 - t2;
                    f2 /= t2 - t1;
                }

                if (t3 - t2) == 0.0 {
                    f2 = 0.0;
                    f3 = 0.0;
                } else {
                    f2 /= t2 - t3;
                    f3 /= t2 - t3;
                }

                if (t3 - t1) == 0.0 {
                    f1 = 0.0;
                    f2 = 0.0;
                } else {
                    f1 /= t1 - t3;
                    f3 /= t1 - t3;
                }

                // Compute interpolated delayed inputs (traload.c:136-141)
                self.input1 = f1 * self.delays[3 * (i - 2) + 1]
                            + f2 * self.delays[3 * (i - 1) + 1]
                            + f3 * self.delays[3 * i + 1];
                self.input2 = f1 * self.delays[3 * (i - 2) + 2]
                            + f2 * self.delays[3 * (i - 1) + 2]
                            + f3 * self.delays[3 * i + 2];
            }

            // Stamp RHS with delayed excitation (traload.c:144-145)
            mna.stamp_rhs(b1, self.input1);
            mna.stamp_rhs(b2, self.input2);
        }

        Ok(())
    }

    /// Port of TRAacLoad from traacld.c.
    /// Stamps complex Y-parameters for AC analysis.
    fn ac_load(
        &mut self,
        mna: &mut MnaSystem,
        _states: &StateVectors,
        omega: f64,
    ) -> Result<(), SimError> {
        let p1 = self.pos_node1;
        let n1 = self.neg_node1;
        let p2 = self.pos_node2;
        let n2 = self.neg_node2;
        let i1 = self.int_node1;
        let i2 = self.int_node2;
        let b1 = self.br_eq1;
        let b2 = self.br_eq2;
        let g = self.conduct;

        // cos/sin of delay phase (traacld.c:30-31)
        let real = (-omega * self.td).cos();
        let imag = (-omega * self.td).sin();

        // Real-only stamps (traacld.c:33-43)
        mna.stamp(p1, p1,  g);          // TRApos1Pos1Ptr
        mna.stamp(p1, i1, -g);          // TRApos1Int1Ptr
        mna.stamp(n1, b1, -1.0);        // TRAneg1Ibr1Ptr
        mna.stamp(p2, p2,  g);          // TRApos2Pos2Ptr
        mna.stamp(n2, b2, -1.0);        // TRAneg2Ibr2Ptr
        mna.stamp(i1, p1, -g);          // TRAint1Pos1Ptr
        mna.stamp(i1, i1,  g);          // TRAint1Int1Ptr
        mna.stamp(i1, b1,  1.0);        // TRAint1Ibr1Ptr
        mna.stamp(i2, i2,  g);          // TRAint2Int2Ptr
        mna.stamp(i2, b2,  1.0);        // TRAint2Ibr2Ptr
        mna.stamp(b1, n1, -1.0);        // TRAibr1Neg1Ptr

        // Complex stamps: real + imaginary (traacld.c:44-50)
        mna.stamp(b1, p2, -real);        // TRAibr1Pos2Ptr+0
        mna.stamp_imag(b1, p2, -imag);  // TRAibr1Pos2Ptr+1
        mna.stamp(b1, n2,  real);        // TRAibr1Neg2Ptr+0
        mna.stamp_imag(b1, n2,  imag);  // TRAibr1Neg2Ptr+1
        mna.stamp(b1, i1,  1.0);        // TRAibr1Int1Ptr (real only)
        mna.stamp(b1, b2, -real * self.imped);       // TRAibr1Ibr2Ptr+0
        mna.stamp_imag(b1, b2, -imag * self.imped); // TRAibr1Ibr2Ptr+1

        // Complex stamps for branch 2 (traacld.c:51-58)
        mna.stamp(b2, p1, -real);        // TRAibr2Pos1Ptr+0
        mna.stamp_imag(b2, p1, -imag);  // TRAibr2Pos1Ptr+1
        mna.stamp(b2, n1,  real);        // TRAibr2Neg1Ptr+0
        mna.stamp_imag(b2, n1,  imag);  // TRAibr2Neg1Ptr+1
        mna.stamp(b2, n2, -1.0);        // TRAibr2Neg2Ptr (real only)
        mna.stamp(b2, i2,  1.0);        // TRAibr2Int2Ptr (real only)
        mna.stamp(b2, b1, -real * self.imped);       // TRAibr2Ibr1Ptr+0
        mna.stamp_imag(b2, b1, -imag * self.imped); // TRAibr2Ibr1Ptr+1

        // Real-only stamps (traacld.c:59-60)
        mna.stamp(p2, i2, -g);          // TRApos2Int2Ptr
        mna.stamp(i2, p2, -g);          // TRAint2Pos2Ptr

        Ok(())
    }
}

impl TransmissionLine {
    /// Record a new delay entry — called from transient accept.
    /// Port of TRAaccept (traacct.c).
    pub fn accept_tran(&mut self, time: f64, rhs_old: &[f64]) {
        // Compute current excitation at both ports
        let v1 = if self.pos_node1 < rhs_old.len() { rhs_old[self.pos_node1] } else { 0.0 }
               - if self.neg_node1 < rhs_old.len() { rhs_old[self.neg_node1] } else { 0.0 };
        let i1 = if self.br_eq1 < rhs_old.len() { rhs_old[self.br_eq1] } else { 0.0 };
        let v2 = if self.pos_node2 < rhs_old.len() { rhs_old[self.pos_node2] } else { 0.0 }
               - if self.neg_node2 < rhs_old.len() { rhs_old[self.neg_node2] } else { 0.0 };
        let i2 = if self.br_eq2 < rhs_old.len() { rhs_old[self.br_eq2] } else { 0.0 };

        let new_input1 = v2 + i2 * self.imped;
        let new_input2 = v1 + i1 * self.imped;

        // Add entry to delay table
        let idx = self.size_delay + 1;
        let needed = (idx + 1) * 3;
        if needed > self.delays.len() {
            self.delays.resize(needed + 15, 0.0);
        }

        self.delays[3 * idx] = time;
        self.delays[3 * idx + 1] = new_input1;
        self.delays[3 * idx + 2] = new_input2;
        self.size_delay = idx;
    }
}
