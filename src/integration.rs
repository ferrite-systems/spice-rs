use crate::config::SimConfig;
use crate::state::StateVectors;

/// Compute integration coefficients — port of ngspice NIcomCof (nicomcof.c:16-206).
///
/// Fills `ag[0..6]` from current timestep `delta`, timestep history `delta_old`,
/// integration order, and theta parameter `xmu` (0.5 for standard trapezoidal).
///
/// Only trapezoidal method (orders 1-2) implemented for Phase 1.
pub fn ni_com_cof(
    delta: f64,
    _delta_old: &[f64; 7],
    order: usize,
    xmu: f64,
) -> [f64; 7] {
    let mut ag = [0.0f64; 7];

    match order {
        // Backward Euler (nicomcof.c:37-40)
        1 => {
            ag[0] = 1.0 / delta;
            ag[1] = -1.0 / delta;
        }
        // Trapezoidal (nicomcof.c:42-45)
        2 => {
            ag[0] = 1.0 / delta / (1.0 - xmu);
            ag[1] = xmu / (1.0 - xmu);
        }
        _ => panic!("unsupported integration order {order}"),
    }

    ag
}

/// Numerical integration — port of ngspice NIintegrate (niinteg.c:17-79).
///
/// Converts charge (at `qcap`) to current (at `qcap+1`) using the companion model.
/// Returns `(geq, ceq)`:
/// - `geq` = equivalent conductance to stamp on diagonal
/// - `ceq` = equivalent current source to stamp in RHS
///
/// `cap` is the capacitance (or inductance) value.
pub fn ni_integrate(
    ag: &[f64; 7],
    states: &mut StateVectors,
    cap: f64,
    qcap: usize,
    order: usize,
) -> (f64, f64) {
    let ccap = qcap + 1; // current is stored at offset qcap+1

    match order {
        // Backward Euler (niinteg.c:27-29)
        1 => {
            let q0 = states.get(0, qcap);
            let q1 = states.get(1, qcap);
            let i = ag[0] * q0 + ag[1] * q1;
            states.set(0, ccap, i);
        }
        // Trapezoidal (niinteg.c:30-34)
        2 => {
            let ccap_old = states.get(1, ccap);
            let q0 = states.get(0, qcap);
            let q1 = states.get(1, qcap);
            let i = -ccap_old * ag[1] + ag[0] * (q0 - q1);
            states.set(0, ccap, i);
        }
        _ => panic!("unsupported integration order {order}"),
    }

    // Companion model output (niinteg.c:76-77)
    let ceq = states.get(0, ccap) - ag[0] * states.get(0, qcap);
    let geq = ag[0] * cap;

    (geq, ceq)
}

/// Truncation error estimation — port of ngspice CKTterr (cktterr.c:12-89).
///
/// Returns the maximum safe timestep based on LTE for the charge state at `qcap`.
pub fn ckt_terr(
    states: &StateVectors,
    qcap: usize,
    order: usize,
    delta: f64,
    delta_old: &[f64; 7],
    config: &SimConfig,
    trtol: f64,
    // trace: pass true to enable detailed tracing
) -> f64 {
    let ccap = qcap + 1;

    let trap_coeff: [f64; 2] = [0.5, 0.08333333333];

    let volttol = config.abs_tol
        + config.reltol * f64::max(states.get(0, ccap).abs(), states.get(1, ccap).abs());

    let chgtol = 1e-14;
    let chargetol = config.reltol
        * f64::max(
            f64::max(states.get(0, qcap).abs(), states.get(1, qcap).abs()),
            chgtol,
        )
        / delta;
    let tol = f64::max(volttol, chargetol);

    let mut diff = [0.0f64; 8];
    let mut deltmp = [0.0f64; 8];

    for i in (0..=order + 1).rev() {
        diff[i] = states.get(i, qcap);
    }
    for i in 0..=order {
        deltmp[i] = delta_old[i];
    }

    let mut j = order as isize;
    loop {
        for i in 0..=(j as usize) {
            diff[i] = (diff[i] - diff[i + 1]) / deltmp[i];
        }
        j -= 1;
        if j < 0 {
            break;
        }
        for i in 0..=(j as usize) {
            deltmp[i] = deltmp[i + 1] + delta_old[i];
        }
    }

    let factor = trap_coeff[order - 1];
    let denom = f64::max(config.abs_tol, factor * diff[0].abs());
    let mut del = trtol * tol / denom;

    if order == 2 {
        del = del.sqrt();
    } else if order > 2 {
        del = (del.ln() / order as f64).exp();
    }

    del
}
