//! Pole-zero analysis — port of ngspice pzan.c / cktpzstr.c / cktpzld.c / cktpzset.c / nipzmeth.c.
//!
//! Algorithm:
//! 1. Compute DC operating point (CKTop)
//! 2. Set mode to MODEDCOP | MODEINITSMSIG and call CKTload once
//!    to compute small-signal parameters
//! 3. Build a separate MNA system for PZ (CKTpzSetup)
//! 4. For each of {poles, zeros}:
//!    a. Setup the driving/balancing columns
//!    b. Iteratively find roots of the determinant using the
//!       CKTpzFindZeros algorithm (zeroin variant)

use crate::circuit::Circuit;
use crate::config::SimConfig;
use crate::error::SimError;
use crate::mna::MnaSystem;
use crate::mode::*;
use crate::parser::{PzAnalysisType, PzInputType};

use std::f64::consts::LN_2;

const M_LN10: f64 = std::f64::consts::LN_10;

/// Result of a PZ analysis — lists of poles and zeros as (real, imag) pairs.
pub struct PzResult {
    pub poles: Vec<(f64, f64)>,
    pub zeros: Vec<(f64, f64)>,
}

// Strategy constants — port of cktpzstr.c
const SHIFT_LEFT: i32 = 2;
const SHIFT_RIGHT: i32 = 3;
const SKIP_LEFT: i32 = 4;
const SKIP_RIGHT: i32 = 5;
const INIT: i32 = 6;
const GUESS: i32 = 7;
const SPLIT_LEFT: i32 = 8;
const SPLIT_RIGHT: i32 = 9;
const MULLER: i32 = 10;
const SYM: i32 = 11;
const SYM2: i32 = 12;
const COMPLEX_INIT: i32 = 13;
const COMPLEX_GUESS: i32 = 14;
#[allow(dead_code)]
const QUIT: i32 = 15;

const NEAR_LEFT: i32 = 4;
const MID_LEFT: i32 = 5;
#[allow(dead_code)]
const FAR_LEFT: i32 = 6;
const NEAR_RIGHT: i32 = 7;
#[allow(dead_code)]
const FAR_RIGHT: i32 = 8;
const MID_RIGHT: i32 = 9;

const NITER_LIM: i32 = 200;

// Flags
const ISAROOT: u32 = 2;
const ISAREPEAT: u32 = 4;
const ISANABERRATION: u32 = 8;
const ISAMINIMA: u32 = 16;

#[derive(Clone, Debug)]
struct PzTrial {
    s: (f64, f64),          // complex frequency point
    f_raw: (f64, f64),      // raw determinant value
    f_def: (f64, f64),      // deflated determinant value
    mag_raw: i32,           // raw magnitude exponent
    mag_def: i32,           // deflated magnitude exponent
    multiplicity: i32,
    flags: u32,
    seq_num: i32,
    count: i32,
}

impl PzTrial {
    fn new(seq_num: i32) -> Self {
        Self {
            s: (0.0, 0.0),
            f_raw: (0.0, 0.0),
            f_def: (0.0, 0.0),
            mag_raw: 0,
            mag_def: 0,
            multiplicity: 0,
            flags: 0,
            count: 0,
            seq_num,
        }
    }
}

/// PZ analysis state — holds all the mutable state for the root-finding algorithm.
struct PzState {
    trials: Vec<PzTrial>,       // Linked list as Vec, ordered by s.real
    zero_trial: Option<usize>,  // Port of ZeroTrial — cursor into trials list
    nzeros: i32,
    nflat: i32,
    max_zeros: i32,
    niter: i32,
    ntrials: i32,
    seq_num: i32,
    guess_param: f64,
    high_guess: f64,
    low_guess: f64,
    last_move: i32,
    consec_moves: i32,
    trapped: i32,
    aberr_num: i32,
    nipzk: f64,
    nipzk_mag: i32,
    numswaps: i32,
}

impl PzState {
    fn new(max_zeros: i32) -> Self {
        Self {
            trials: Vec::new(),
            zero_trial: None,
            nzeros: 0,
            nflat: 0,
            max_zeros,
            niter: 0,
            ntrials: 0,
            seq_num: 1,
            guess_param: 0.0,
            high_guess: -1.0,
            low_guess: 1.0,
            last_move: 0,
            consec_moves: 0,
            trapped: 0,
            aberr_num: 0,
            nipzk: 0.0,
            nipzk_mag: 0,
            numswaps: 1,
        }
    }
}

fn sgn(x: f64) -> i32 {
    if x < 0.0 { -1 } else if x == 0.0 { 0 } else { 1 }
}

#[allow(dead_code)]
fn strat_name(strat: i32) -> &'static str {
    match strat {
        SHIFT_LEFT => "shift_left",
        SHIFT_RIGHT => "shift_right",
        SKIP_LEFT => "skip_left",
        SKIP_RIGHT => "skip_right",
        INIT => "init",
        GUESS => "guess",
        SPLIT_LEFT => "split_left",
        SPLIT_RIGHT => "split_right",
        MULLER => "muller",
        SYM => "sym",
        SYM2 => "sym2",
        COMPLEX_INIT => "complex_init",
        COMPLEX_GUESS => "complex_guess",
        QUIT => "quit",
        _ => "unknown",
    }
}

/// Port of zaddeq from cktpzstr.c:1012.
/// Adds x * 2^xmag + y * 2^ymag, returning (result, result_mag).
fn zaddeq(x: f64, xmag: i32, y: f64, ymag: i32) -> (f64, i32) {
    let (mut x, mut y) = (x, y);
    let amag;

    if xmag > ymag {
        amag = xmag;
        if xmag > 50 + ymag {
            y = 0.0;
        } else {
            for _ in 0..(xmag - ymag) {
                y /= 2.0;
            }
        }
    } else {
        amag = ymag;
        if ymag > 50 + xmag {
            x = 0.0;
        } else {
            for _ in 0..(ymag - xmag) {
                x /= 2.0;
            }
        }
    }

    let mut a = x + y;
    let mut result_mag = amag;

    if a == 0.0 {
        result_mag = 0;
    } else {
        while a.abs() > 1.0 {
            a /= 2.0;
            result_mag += 1;
        }
        while a.abs() < 0.5 {
            a *= 2.0;
            result_mag -= 1;
        }
    }

    (a, result_mag)
}

/// Port of R_NORM from macros.h.
fn r_norm(a: &mut f64, b: &mut i32) {
    if *a == 0.0 {
        *b = 0;
    } else {
        while a.abs() > 1.0 {
            *b += 1;
            *a /= 2.0;
        }
        while a.abs() < 0.5 {
            *b -= 1;
            *a *= 2.0;
        }
    }
}

/// Port of C_NORM from complex.h.
fn c_norm(re: &mut f64, im: &mut f64, mag: &mut i32) {
    if *re == 0.0 && *im == 0.0 {
        *mag = 0;
    } else {
        while re.abs() > 1.0 || im.abs() > 1.0 {
            *mag += 1;
            *re /= 2.0;
            *im /= 2.0;
        }
        while re.abs() <= 0.5 && im.abs() <= 0.5 {
            *mag -= 1;
            *re *= 2.0;
            *im *= 2.0;
        }
    }
}

/// Port of C_MAG2: a.real = a.real^2 + a.imag^2, a.imag = 0.
fn c_mag2(re: &mut f64, im: &mut f64) {
    *re = *re * *re + *im * *im;
    *im = 0.0;
}

/// Port of C_SQRT from complex.h.
fn c_sqrt(re: &mut f64, im: &mut f64) {
    if *im == 0.0 {
        if *re < 0.0 {
            *im = (-*re).sqrt();
            *re = 0.0;
        } else {
            *re = (*re).sqrt();
            *im = 0.0;
        }
    } else {
        let mag = (*re * *re + *im * *im).sqrt(); // hypot
        let a = (mag - *re) / 2.0;
        if a <= 0.0 {
            *re = mag.sqrt();
            *im /= 2.0 * *re;
        } else {
            let a_sqrt = a.sqrt();
            *re = *im / (2.0 * a_sqrt);
            *im = a_sqrt;
        }
    }
}

/// Complex multiply: (a_re, a_im) * (b_re, b_im)
fn c_mul(a_re: f64, a_im: f64, b_re: f64, b_im: f64) -> (f64, f64) {
    (a_re * b_re - a_im * b_im, a_re * b_im + a_im * b_re)
}

/// Complex divide: (a_re, a_im) / (b_re, b_im)
fn c_div(a_re: f64, a_im: f64, b_re: f64, b_im: f64) -> (f64, f64) {
    let mag = b_re * b_re + b_im * b_im;
    ((a_re * b_re + a_im * b_im) / mag, (a_im * b_re - a_re * b_im) / mag)
}

/// Run PZ analysis.
///
/// Port of PZan from pzan.c.
pub fn pz_analysis(
    circuit: &mut Circuit,
    config: &SimConfig,
    in_pos_name: &str,
    in_neg_name: &str,
    out_pos_name: &str,
    out_neg_name: &str,
    input_type: PzInputType,
    pz_type: PzAnalysisType,
) -> Result<PzResult, SimError> {
    // Resolve node names to equation numbers
    let in_pos = resolve_node(circuit, in_pos_name)?;
    let in_neg = resolve_node(circuit, in_neg_name)?;
    let out_pos = resolve_node(circuit, out_pos_name)?;
    let out_neg = resolve_node(circuit, out_neg_name)?;

    // Validate
    if in_pos == in_neg {
        return Err(SimError::Other("PZ: input is shorted".to_string()));
    }
    if out_pos == out_neg {
        return Err(SimError::Other("PZ: output is shorted".to_string()));
    }

    // Step 1: Compute DC operating point
    let mut sim = crate::analysis::dc::dc_operating_point(circuit, config)?;

    // Step 2: Set MODEINITSMSIG and call load once to compute small-signal params.
    let smsig_mode = Mode::new(MODEDCOP | MODEINITSMSIG);
    sim.mna.clear();
    let mut noncon = false;
    for dev in circuit.devices.iter_mut() {
        dev.pre_load(&mut sim.mna, &mut circuit.states, smsig_mode);
    }
    for dev in circuit.devices.iter_mut() {
        dev.load(&mut sim.mna, &mut circuit.states, smsig_mode, 1.0, sim.gmin, &mut noncon)?;
    }

    let mut result = PzResult {
        poles: Vec::new(),
        zeros: Vec::new(),
    };

    let do_poles = matches!(pz_type, PzAnalysisType::Both | PzAnalysisType::Poles);
    let do_zeros = matches!(pz_type, PzAnalysisType::Both | PzAnalysisType::Zeros);

    // Step 3: Find poles
    if do_poles {
        let (solution_col, balance_col, drive_pos, drive_neg) =
            pz_setup_columns(in_pos, in_neg, out_pos, out_neg, input_type, false);

        let poles = pz_find_zeros(
            circuit, &sim.mna,
            solution_col, balance_col, drive_pos, drive_neg,
        )?;

        // Convert trial list to output format
        for trial in &poles {
            for _ in 0..trial.multiplicity {
                result.poles.push((trial.s.0, trial.s.1));
                if trial.s.1 != 0.0 {
                    // Conjugate pair
                    result.poles.push((trial.s.0, -trial.s.1));
                }
            }
        }
    }

    // Step 4: Find zeros
    if do_zeros {
        let (solution_col, balance_col, drive_pos, drive_neg) =
            pz_setup_columns(in_pos, in_neg, out_pos, out_neg, input_type, true);

        let zeros = pz_find_zeros(
            circuit, &sim.mna,
            solution_col, balance_col, drive_pos, drive_neg,
        )?;

        for trial in &zeros {
            for _ in 0..trial.multiplicity {
                result.zeros.push((trial.s.0, trial.s.1));
                if trial.s.1 != 0.0 {
                    result.zeros.push((trial.s.0, -trial.s.1));
                }
            }
        }
    }

    Ok(result)
}

fn resolve_node(circuit: &Circuit, name: &str) -> Result<usize, SimError> {
    if name == "0" || name.eq_ignore_ascii_case("gnd") {
        return Ok(0);
    }
    circuit.find_node(name)
        .ok_or_else(|| SimError::Other(format!("PZ: node '{}' not found", name)))
}

/// Port of CKTpzSetup from cktpzset.c.
/// Determines which columns to use for the driving function and solution extraction.
fn pz_setup_columns(
    in_pos: usize,
    in_neg: usize,
    out_pos: usize,
    out_neg: usize,
    input_type: PzInputType,
    do_zeros: bool,
) -> (usize, usize, usize, usize) {
    let (mut input_pos, mut input_neg, output_pos, output_neg);

    input_pos = in_pos;
    input_neg = in_neg;

    if do_zeros {
        // Vo/Ii in Y
        output_pos = out_pos;
        output_neg = out_neg;
    } else if input_type == PzInputType::Vol {
        // Vi/Ii in Y (poles of voltage transfer function)
        output_pos = in_pos;
        output_neg = in_neg;
    } else {
        // Denominator (poles of current transfer function)
        output_pos = 0;
        output_neg = 0;
        input_pos = 0;
        input_neg = 0;
    }

    let (solution_col, balance_col);
    if output_pos != 0 {
        solution_col = output_pos;
        balance_col = if output_neg != 0 { output_neg } else { 0 };
    } else {
        solution_col = output_neg;
        // SWAP(input_pos, input_neg)
        std::mem::swap(&mut input_pos, &mut input_neg);
        balance_col = 0;
    }

    (solution_col, balance_col, input_pos, input_neg)
}

/// Port of CKTpzLoad from cktpzld.c.
/// Loads device PZ stamps into the MNA system, then applies column operations
/// for the driving function.
fn pz_load(
    circuit: &mut Circuit,
    mna: &mut MnaSystem,
    s_re: f64, s_im: f64,
    solution_col: usize,
    balance_col: usize,
    drive_pos: usize,
    drive_neg: usize,
) -> Result<(), SimError> {
    // Clear RHS and matrix
    mna.clear_complex();

    // Load all device PZ stamps
    for dev in circuit.devices.iter_mut() {
        dev.pz_load(mna, s_re, s_im)?;
    }

    // SMPcAddCol: add solution_col into balance_col (if both nonzero)
    if balance_col != 0 && solution_col != 0 {
        sparse_rs::markowitz::add_col(&mut mna.matrix, balance_col, solution_col);
    }

    // SMPcZeroCol: zero out solution_col
    if solution_col != 0 {
        sparse_rs::markowitz::zero_col(&mut mna.matrix, solution_col);
    }

    // Driving function: inject current source at drive nodes
    if drive_pos != 0 {
        let idx = mna.find_or_create_element(drive_pos, solution_col);
        if idx != sparse_rs::markowitz::matrix::NONE {
            mna.matrix.el_mut(idx).real = 1.0;
        }
    }
    if drive_neg != 0 {
        let idx = mna.find_or_create_element(drive_neg, solution_col);
        if idx != sparse_rs::markowitz::matrix::NONE {
            mna.matrix.el_mut(idx).real = -1.0;
        }
    }

    Ok(())
}

/// Port of SMPcDProd from spsmp.c.
/// Computes the complex determinant product (mantissa, exponent in base 2).
/// Port of SMPcDProd from spsmp.c:362-438.
/// Computes the complex determinant product and returns ((re, im), exponent_base2).
fn smp_c_dprod(mna: &MnaSystem) -> ((f64, f64), i32) {
    let (re, im, p) = sparse_rs::markowitz::determinant_complex(&mna.matrix);

    // Convert base-10 exponent to base-2 (spsmp.c:381-384)
    // y = p * ln(10) / ln(2)
    let y_full = p as f64 * M_LN10 / LN_2;
    let x = y_full as i64 as f64;  // truncation toward zero, matches C `(int) y`
    let y = y_full - x;

    // Fold in fractional part (spsmp.c:394-396)
    // Use C library pow via FFI to match ngspice exactly. Rust's f64::powf
    // may use a different implementation that differs by 1 ULP.
    // Use ngspice's own C library math functions to avoid platform-specific
    // 1-ULP differences between Rust's f64::powf and the C pow linked by ngspice.
    // These wrappers are defined in spsmp.c and link to the exact same pow/logb/scalbn.
    unsafe extern "C" {
        fn ferrite_c_pow(base: f64, exponent: f64) -> f64;
    }
    let z = unsafe { ferrite_c_pow(2.0, y) };
    let out_re = re * z;
    let out_im = im * z;

    // Re-normalize using C library logb/scalbn to match ngspice exactly.
    // These wrappers are defined in spsmp.c.
    unsafe extern "C" {
        fn ferrite_c_logb(x: f64) -> f64;
        fn ferrite_c_scalbn(x: f64, n: i32) -> f64;
    }

    let (y_norm, z_norm);
    if out_re != 0.0 {
        y_norm = unsafe { ferrite_c_logb(out_re) };
        z_norm = if out_im != 0.0 { unsafe { ferrite_c_logb(out_im) } } else { 0.0 };
    } else if out_im != 0.0 {
        z_norm = unsafe { ferrite_c_logb(out_im) };
        y_norm = 0.0;
    } else {
        // Singular
        return ((0.0, 0.0), 0);
    }

    let y_max = if y_norm < z_norm { z_norm } else { y_norm };

    let exponent = (x + y_max) as i32;
    let mant_re = unsafe { ferrite_c_scalbn(out_re, -y_max as i32) };
    let mant_im = unsafe { ferrite_c_scalbn(out_im, -y_max as i32) };

    ((mant_re, mant_im), exponent)
}

/// Port of CKTpzFindZeros from cktpzstr.c.
/// Returns the list of found roots (only those with ISAROOT flag).
fn pz_find_zeros(
    circuit: &mut Circuit,
    dc_mna: &MnaSystem,
    solution_col: usize,
    balance_col: usize,
    drive_pos: usize,
    drive_neg: usize,
) -> Result<Vec<PzTrial>, SimError> {
    let matrix_size = dc_mna.size;
    let mut state = PzState::new(matrix_size as i32);

    // Create a fresh MNA for PZ (port of CKTpzSetup: NIdestroy + NIinit)
    // We reuse the same circuit topology but create a new matrix.
    let mut pz_mna = MnaSystem::new(matrix_size);

    // Copy the node mapping from the DC MNA
    pz_mna.int_to_ext = dc_mna.int_to_ext.clone();
    pz_mna.ext_to_int = dc_mna.ext_to_int.clone();
    pz_mna.next_int = dc_mna.next_int;

    // Sync TRANSLATE maps to the matrix BEFORE any operations
    // This is critical for zero_col/add_col which use matrix.ext_to_int_col
    if pz_mna.next_int > 1 {
        for int_idx in 1..pz_mna.next_int {
            let ext_idx = pz_mna.int_to_ext[int_idx];
            pz_mna.matrix.int_to_ext_row[int_idx] = ext_idx;
            pz_mna.matrix.int_to_ext_col[int_idx] = ext_idx;
            pz_mna.matrix.ext_to_int_row[ext_idx] = int_idx;
            pz_mna.matrix.ext_to_int_col[ext_idx] = int_idx;
        }
    }

    // Setup matrix elements for all devices
    for dev in circuit.devices.iter_mut() {
        dev.setup_matrix(&mut pz_mna);
    }

    // Also ensure drive point elements exist
    if drive_pos != 0 && solution_col != 0 {
        pz_mna.make_element(drive_pos, solution_col);
    }
    if drive_neg != 0 && solution_col != 0 {
        pz_mna.make_element(drive_neg, solution_col);
    }

    let mut neighborhood: [Option<usize>; 3] = [None, None, None]; // indices into state.trials

    pz_reset(&mut state, &mut neighborhood);

    let mut total_iterations = 0;
    loop {
        total_iterations += 1;
        if total_iterations > 1000 {
            // Safety: prevent infinite loops
            break;
        }

        // Strategy selection loop
        let mut strat;
        let mut inner_count = 0;
        loop {
            strat = pz_strat(&mut state, &neighborhood);
            if strat >= GUESS || state.trapped != 0 {
                break;
            }
            if !pz_step(&mut state, strat, &mut neighborhood) {
                strat = GUESS;
                break;
            }
            inner_count += 1;
            if inner_count > 100 { strat = GUESS; break; }
        }

        state.niter += 1;


        // Evaluate current strategy to get next trial point
        let mut new_trial = pz_eval(&mut state, strat, &neighborhood)?;

        eprintln!("SR_ITER[{}] strat={} s=({:.15e},{:.15e}) ntrials={} trapped={}",
            total_iterations, strat_name(strat),
            new_trial.s.0, new_trial.s.1, state.ntrials, state.trapped);

        // Run the trial — load matrix, factor, get determinant
        let run_result = pz_run_trial(
            circuit, &mut pz_mna, &mut state,
            &mut new_trial, &neighborhood,
            solution_col, balance_col, drive_pos, drive_neg,
        )?;

        let result_desc = match &run_result {
            RunResult::Root(_) => "ROOT",
            RunResult::Minima(_) => "MINIMA",
            RunResult::Aberration => "ABERRATION",
            RunResult::Normal(_) => "NORMAL",
        };
        eprintln!("  SR_RESULT[{}] => {} s=({:.20e},{:.20e})",
            total_iterations, result_desc, new_trial.s.0, new_trial.s.1);

        match run_result {
            RunResult::Root(trial_or_idx) => {
                match trial_or_idx {
                    TrialRef::New(trial) => {
                        // Insert and verify
                        let idx = insert_trial_adj(&mut state, trial, &mut neighborhood);
                        if pz_verify(&mut state, idx) {
                            state.niter = 0;
                            pz_reset(&mut state, &mut neighborhood);
                        } else {
                            pz_update_set(&mut state, &mut neighborhood, idx);
                        }
                    }
                    TrialRef::Existing(idx) => {
                        // Repeat at existing root
                        state.trials[idx].flags |= ISAREPEAT;
                        state.trials[idx].multiplicity += 1;
                        state.niter = 0;
                        pz_reset(&mut state, &mut neighborhood);
                    }
                }
            }
            RunResult::Minima(idx) => {
                state.trials[idx].flags |= ISAMINIMA;
                neighborhood[0] = None;
                neighborhood[1] = Some(idx);
                neighborhood[2] = None;
            }
            RunResult::Aberration => {
                pz_reset(&mut state, &mut neighborhood);
                state.aberr_num += 1;
            }
            RunResult::Normal(trial) => {
                let idx = insert_trial_adj(&mut state, trial, &mut neighborhood);
                pz_update_set(&mut state, &mut neighborhood, idx);
            }
        }

        // Termination conditions (port of cktpzstr.c:194-199)
        if state.high_guess - state.low_guess >= 1e40 {
            break;
        }
        if state.nzeros >= state.max_zeros {
            break;
        }
        if state.niter >= NITER_LIM {
            break;
        }
        if state.aberr_num >= 3 {
            break;
        }
        if state.high_guess - state.low_guess >= 1e35 {
            break;
        }
        if let (Some(n0), Some(n2)) = (neighborhood[0], neighborhood[2]) {
            if state.trapped == 0
                && state.trials[n2].s.0 - state.trials[n0].s.0 >= 1e22
            {
                break;
            }
        }
    }

    // Collect roots
    let roots: Vec<PzTrial> = state.trials.iter()
        .filter(|t| t.flags & ISAROOT != 0)
        .cloned()
        .collect();

    for (i, r) in roots.iter().enumerate() {
        eprintln!("SR_PZ root[{}]: s=({:.20e}, {:.20e}) seq={} mult={}",
            i, r.s.0, r.s.1, r.seq_num, r.multiplicity);
    }

    Ok(roots)
}

enum RunResult {
    Root(TrialRef),
    Minima(usize),
    Aberration,
    Normal(PzTrial),
}

enum TrialRef {
    New(PzTrial),
    Existing(usize),
}

/// Insert a trial into the sorted list, maintaining order by s.real.
/// Also adjusts neighborhood indices and zero_trial for the insertion.
fn insert_trial_adj(
    state: &mut PzState,
    trial: PzTrial,
    neighborhood: &mut [Option<usize>; 3],
) -> usize {
    let was_empty = state.trials.is_empty();
    let pos = state.trials.iter().position(|t| t.s.0 > trial.s.0)
        .unwrap_or(state.trials.len());
    state.trials.insert(pos, trial);
    // Adjust neighborhood indices for the insertion
    for slot in neighborhood.iter_mut() {
        if let Some(idx) = slot {
            if *idx >= pos {
                *idx += 1;
            }
        }
    }
    // Adjust zero_trial
    if let Some(zt) = &mut state.zero_trial {
        if *zt >= pos {
            *zt += 1;
        }
    }
    // Set zero_trial on first insertion (port of ZeroTrial = new_trial when Trials was empty)
    if was_empty {
        state.zero_trial = Some(pos);
    }
    pos
}

/// Insert a trial without adjusting neighborhood (used in pz_run_trial internals).
/// Does NOT update zero_trial since these are temporary insert+remove pairs.
fn insert_trial_raw(trials: &mut Vec<PzTrial>, trial: PzTrial) -> usize {
    let pos = trials.iter().position(|t| t.s.0 > trial.s.0)
        .unwrap_or(trials.len());
    trials.insert(pos, trial);
    pos
}

/// Port of CKTpzRunTrial from cktpzstr.c.
fn pz_run_trial(
    circuit: &mut Circuit,
    pz_mna: &mut MnaSystem,
    state: &mut PzState,
    new_trial: &mut PzTrial,
    neighborhood: &[Option<usize>; 3],
    solution_col: usize,
    balance_col: usize,
    drive_pos: usize,
    drive_neg: usize,
) -> Result<RunResult, SimError> {
    if new_trial.s.1 < 0.0 {
        new_trial.s.1 *= -1.0;
    }

    // Calculate deflation factor from previous zeros
    let mut def_re = 1.0_f64;
    let mut def_im = 0.0_f64;
    let mut def_mag: i32 = 0;

    let mut pretest = false;
    let mut pretest_idx: Option<usize> = None;
    let mut shifted = false;

    loop {
        def_mag = 0;
        def_re = 1.0;
        def_im = 0.0;
        let was_shifted = shifted;
        shifted = false;

        let mut match_idx: Option<usize> = None;

        // Use index-based iteration to allow borrowing state.trials for pz_alter
        let num_trials = state.trials.len();
        for i in 0..num_trials {
            let mut diff_re = state.trials[i].s.0 - new_trial.s.0;
            let mut diff_im = state.trials[i].s.1 - new_trial.s.1;

            let (abstol, reltol) = if state.trials[i].flags & ISAROOT != 0 {
                (1e-5, 1e-6)
            } else {
                (1e-20, 1e-12)
            };

            if diff_im == 0.0 &&
                diff_re.abs() / (state.trials[i].s.0.abs() + abstol / reltol) < reltol
            {
                // Port of cktpzstr.c:595-606
                // Try alter with neighborhood center (set[1])
                let do_pretest = if was_shifted || state.trials[i].count >= 3 {
                    true
                } else if let Some(nearto_idx) = neighborhood[1] {
                    // alter returns false = bail out → take pretest
                    !pz_alter(new_trial, &state.trials, nearto_idx,
                              state.trapped, abstol, reltol)
                } else {
                    true // no neighborhood center → pretest
                };

                if do_pretest {
                    state.trials[i].count = 0;
                    pretest = true;
                    pretest_idx = Some(i);
                    break;
                } else {
                    state.trials[i].count += 1; // try to shift
                    shifted = true;
                    break;
                }
            } else {
                if state.trapped == 0 {
                    state.trials[i].count = 0;
                }
                if state.trials[i].flags & ISAROOT != 0 {
                    let mut diff_mag: i32 = 0;
                    c_norm(&mut diff_re, &mut diff_im, &mut diff_mag);
                    if diff_im != 0.0 {
                        c_mag2(&mut diff_re, &mut diff_im);
                        diff_mag *= 2;
                    }
                    c_norm(&mut diff_re, &mut diff_im, &mut diff_mag);

                    let multiplicity = state.trials[i].multiplicity;
                    for _ in 0..multiplicity {
                        let (new_re, new_im) = c_mul(def_re, def_im, diff_re, diff_im);
                        def_re = new_re;
                        def_im = new_im;
                        def_mag += diff_mag;
                        c_norm(&mut def_re, &mut def_im, &mut def_mag);
                    }
                } else if match_idx.is_none() {
                    match_idx = Some(i);
                }
            }
        }

        if !shifted { break; }
    }

    if pretest {
        let p_idx = pretest_idx.unwrap();
        let p = &state.trials[p_idx];

        if !(p.flags & ISAROOT != 0) && state.trapped == 3
            && state.nipzk != 0.0 && state.nipzk_mag > -10
        {
            // Minima found
            return Ok(RunResult::Minima(p_idx));
        } else if p.flags & ISAROOT != 0 {
            // Repeat at existing root
            return Ok(RunResult::Root(TrialRef::Existing(p_idx)));
        } else {
            // Regular zero, as precise as we can get
            new_trial.f_raw = (0.0, 0.0);
            new_trial.f_def = (0.0, 0.0);
            new_trial.mag_raw = 0;
            new_trial.mag_def = 0;
            new_trial.flags = ISAROOT;
            let idx = insert_trial_raw(&mut state.trials, new_trial.clone());
            state.ntrials += 1;
            return Ok(RunResult::Root(TrialRef::New(state.trials.remove(idx))));
        }
    }

    // Run the trial — load and factor the matrix
    pz_load(circuit, pz_mna,
        new_trial.s.0, new_trial.s.1,
        solution_col, balance_col, drive_pos, drive_neg)?;

    // Factor and get determinant.
    // Port of cktpzstr.c:724: ngspice unconditionally sets NIPZSHOULDREORDER
    // before every trial, so every trial gets a fresh reorder.
    //
    // Uses order_and_factor_complex_in_place which does Markowitz ordering
    // with complex magnitudes (ELEMENT_MAG) and ComplexRowColElimination,
    // matching ngspice's spOrderAndFactor with Complex=YES.
    let mut is_singular = false;

    // Port of SMPcReorder thresholds: PivTol=1e-30, PivRel=0.0
    // ngspice passes RelThreshold=0.0 to spOrderAndFactor, but the guard
    // (RelThreshold <= 0.0) replaces it with Matrix->RelThreshold = 1e-3.
    // AbsThreshold = 1e-30 is accepted directly.
    // Our matrix default rel_threshold=1e-3 already matches, so we only set abs.
    pz_mna.matrix.set_abs_threshold(1.0e-30);
    pz_mna.matrix.needs_ordering = true;
    match sparse_rs::markowitz::order_and_factor_complex_in_place(&mut pz_mna.matrix) {
        Ok(()) => {}
        Err(_e) => {
            is_singular = true;
        }
    }

    if !is_singular {
        // Get determinant
        let ((mant_re, mant_im), exp) = smp_c_dprod(pz_mna);
        new_trial.f_raw = (mant_re, mant_im);
        new_trial.mag_raw = exp;

        if mant_re == 0.0 && mant_im == 0.0 {
            is_singular = true;
        }
    }

    if is_singular {
        new_trial.f_raw = (0.0, 0.0);
        new_trial.f_def = (0.0, 0.0);
        new_trial.mag_raw = 0;
        new_trial.mag_def = 0;
        new_trial.flags = ISAROOT;
    } else {
        // Apply numswaps sign
        new_trial.f_raw.0 *= state.numswaps as f64;
        new_trial.f_raw.1 *= state.numswaps as f64;

        eprintln!("  SR_DEFLATE s=({:.20e}) f_raw=({:.20e},{:.20e}) mag_raw={} def=({:.20e},{:.20e}) def_mag={}",
            new_trial.s.0, new_trial.f_raw.0, new_trial.f_raw.1, new_trial.mag_raw,
            def_re, def_im, def_mag);

        // Deflate
        new_trial.f_def = new_trial.f_raw;
        new_trial.mag_def = new_trial.mag_raw;

        let (div_re, div_im) = c_div(new_trial.f_def.0, new_trial.f_def.1, def_re, def_im);
        new_trial.f_def = (div_re, div_im);
        new_trial.mag_def -= def_mag;
        let mut re = new_trial.f_def.0;
        let mut im = new_trial.f_def.1;
        let mut mag = new_trial.mag_def;
        c_norm(&mut re, &mut im, &mut mag);
        new_trial.f_def = (re, im);
        new_trial.mag_def = mag;

        eprintln!("  SR_DEFLATE result: f_def=({:.20e},{:.20e}) mag_def={}",
            new_trial.f_def.0, new_trial.f_def.1, new_trial.mag_def);
    }

    // Insert the trial into the ordered list
    let idx = insert_trial_raw(&mut state.trials, new_trial.clone());
    state.ntrials += 1;

    if new_trial.flags & ISAROOT != 0 {
        return Ok(RunResult::Root(TrialRef::New(state.trials.remove(idx))));
    }

    // Check for flat region
    // (simplified — just track nflat)
    state.nflat = 1;

    Ok(RunResult::Normal(state.trials.remove(idx)))
}

fn find_non_root_neighbor(trials: &[PzTrial], idx: usize) -> Option<usize> {
    // Search forward for a non-root trial
    for i in (idx + 1)..trials.len() {
        if trials[i].flags & ISAROOT == 0 && trials[i].flags & ISAMINIMA == 0 {
            return Some(i);
        }
    }
    // Search backward
    for i in (0..idx).rev() {
        if trials[i].flags & ISAROOT == 0 && trials[i].flags & ISAMINIMA == 0 {
            return Some(i);
        }
    }
    None
}

/// Port of alter() from cktpzstr.c:1149.
/// Shifts `new_trial` away from the neighborhood center `nearto_idx` to avoid
/// coinciding with an existing trial. Returns true if shift succeeded, false
/// if bailed out (too close to neighbor).
fn pz_alter(
    new_trial: &mut PzTrial,
    trials: &[PzTrial],
    nearto_idx: usize,
    trapped: i32,
    abstol: f64,
    reltol: f64,
) -> bool {
    let nearto = &trials[nearto_idx];
    let has_prev = nearto_idx > 0;
    let has_next = nearto_idx + 1 < trials.len();

    let p1;
    let p2;

    if trapped != 2 {
        let mut p1_tmp = nearto.s.0;
        if nearto.flags & ISAROOT != 0 {
            p1_tmp -= 1e-6 * nearto.s.0 + 1e-5;
        }
        if has_prev {
            p1_tmp += trials[nearto_idx - 1].s.0;
        } else {
            p1_tmp -= 10.0 * (p1_tmp.abs() + 1.0);
        }
        p1 = p1_tmp / 2.0;
    } else {
        p1 = nearto.s.0;
    }

    if trapped != 1 {
        let mut p2_tmp = nearto.s.0;
        if nearto.flags & ISAROOT != 0 {
            p2_tmp += 1e-6 * nearto.s.0 + 1e-5;
        }
        if has_next {
            p2_tmp += trials[nearto_idx + 1].s.0;
        } else {
            p2_tmp += 10.0 * (p2_tmp.abs() + 1.0);
        }
        p2 = p2_tmp / 2.0;
    } else {
        p2 = nearto.s.0;
    }

    // Bail-out check: if shifted point would be too close to the neighbor
    if (has_prev
        && (p1 - trials[nearto_idx - 1].s.0).abs()
            / trials[nearto_idx - 1].s.0.abs()
            + abstol / reltol
            < reltol)
        || (has_next
            && (p2 - trials[nearto_idx + 1].s.0).abs()
                / trials[nearto_idx + 1].s.0.abs()
                + abstol / reltol
                < reltol)
    {
        return false;
    }

    // Pick the direction that moves further from nearto
    if trapped != 2 && nearto.s.0 - p1 > p2 - nearto.s.0 {
        new_trial.s.0 = p1;
    } else {
        new_trial.s.0 = p2;
    }

    true
}

/// Port of CKTpzVerify from cktpzstr.c.
fn pz_verify(state: &mut PzState, new_idx: usize) -> bool {
    state.nzeros += 1;
    if state.trials[new_idx].s.1 != 0.0 {
        state.nzeros += 1;
    }
    state.nflat = 0;

    if state.trials[new_idx].multiplicity == 0 {
        state.trials[new_idx].flags |= ISAROOT;
        state.trials[new_idx].multiplicity = 1;
    }

    // Deflate other trials and remove nearby ones
    let root_s = state.trials[new_idx].s;
    let mut to_remove = Vec::new();

    for i in 0..state.trials.len() {
        if i == new_idx { continue; }
        if state.trials[i].flags & ISAROOT != 0 { continue; }

        let mut diff_re = root_s.0 - state.trials[i].s.0;
        let mut diff_im = root_s.1 - state.trials[i].s.1;

        if root_s.1 != 0.0 {
            c_mag2(&mut diff_re, &mut diff_im);
        }

        let tdiff = diff_re;

        if diff_re != 0.0 {
            let mut diff_mag: i32 = 0;
            c_norm(&mut diff_re, &mut diff_im, &mut diff_mag);
            diff_mag *= -1;
            let (div_re, div_im) = c_div(
                state.trials[i].f_def.0, state.trials[i].f_def.1,
                diff_re, diff_im,
            );
            let mut new_re = div_re;
            let mut new_im = div_im;
            c_norm(&mut new_re, &mut new_im, &mut diff_mag);
            state.trials[i].f_def = (new_re, new_im);
            state.trials[i].mag_def += diff_mag;
        }

        if state.trials[i].s.1 != 0.0
            || tdiff.abs() / (root_s.0.abs() + 200.0) < 0.005
        {
            to_remove.push(i);
        }
    }

    // Remove in reverse order, adjusting zero_trial for each removal
    to_remove.sort_unstable();
    for &i in to_remove.iter().rev() {
        // Update zero_trial: port of CKTpzVerify's ZeroTrial update
        if let Some(zt) = state.zero_trial {
            if zt == i {
                // This trial IS ZeroTrial — update to next or prev
                if i + 1 < state.trials.len() {
                    state.zero_trial = Some(i); // after removal, i points to what was i+1
                } else if i > 0 {
                    state.zero_trial = Some(i - 1);
                } else {
                    state.zero_trial = None;
                }
            } else if zt > i {
                state.zero_trial = Some(zt - 1);
            }
        }
        state.ntrials -= 1;
        state.trials.remove(i);
    }

    true
}

/// Port of CKTpzStrat from cktpzstr.c.
fn pz_strat(state: &mut PzState, neighborhood: &[Option<usize>; 3]) -> i32 {
    let n0 = neighborhood[0].map(|i| &state.trials[i]);
    let n1 = neighborhood[1].map(|i| &state.trials[i]);
    let n2 = neighborhood[2].map(|i| &state.trials[i]);

    let mut new_trap = 0;

    let mut suggestion;

    if n1.is_some() && (n1.unwrap().flags & ISAMINIMA != 0) {
        suggestion = COMPLEX_INIT;
    } else if n0.is_some() && n0.unwrap().s.1 != 0.0 {
        if n1.is_none() || n2.is_none() {
            suggestion = COMPLEX_GUESS;
        } else {
            suggestion = MULLER;
        }
    } else if n0.is_none() || n1.is_none() || n2.is_none() {
        suggestion = INIT;
    } else {
        let n0 = n0.unwrap();
        let n1 = n1.unwrap();
        let n2 = n2.unwrap();

        if sgn(n0.f_def.0) != sgn(n1.f_def.0) {
            new_trap = 1;
            suggestion = SYM2;
        } else if sgn(n1.f_def.0) != sgn(n2.f_def.0) {
            new_trap = 2;
            suggestion = SYM2;
        } else {
            let (a, a_mag) = zaddeq(n1.f_def.0, n1.mag_def, -n0.f_def.0, n0.mag_def);
            let (b, b_mag) = zaddeq(n2.f_def.0, n2.mag_def, -n1.f_def.0, n1.mag_def);

            if state.trapped == 0 {
                let k1 = n1.s.0 - n0.s.0;
                let k2 = n2.s.0 - n1.s.0;

                if a_mag + 10 < n0.mag_def
                    && a_mag + 10 < n1.mag_def
                    && b_mag + 10 < n1.mag_def
                    && b_mag + 10 < n2.mag_def
                {
                    suggestion = if k1 > k2 { SKIP_RIGHT } else { SKIP_LEFT };
                } else if sgn(a) != -sgn(b) {
                    if a == 0.0 {
                        suggestion = SKIP_LEFT;
                    } else if b == 0.0 {
                        suggestion = SKIP_RIGHT;
                    } else if sgn(a) == sgn(n1.f_def.0) {
                        suggestion = SHIFT_LEFT;
                    } else {
                        suggestion = SHIFT_RIGHT;
                    }
                } else if sgn(a) == -sgn(n1.f_def.0) {
                    new_trap = 3;
                    suggestion = SYM;
                } else if k1 > k2 {
                    suggestion = SKIP_RIGHT;
                } else {
                    suggestion = SKIP_LEFT;
                }
            } else {
                new_trap = 3;
                if sgn(a) != sgn(b) {
                    suggestion = SYM;
                } else if a_mag > b_mag || (a_mag == b_mag && a.abs() > b.abs()) {
                    suggestion = SPLIT_LEFT;
                } else {
                    suggestion = SPLIT_RIGHT;
                }
            }
        }
    }

    // Consec_Moves check — ALL paths reach here (port of cktpzstr.c:500-509)
    if state.consec_moves >= 3 && state.trapped == new_trap {
        new_trap = state.trapped;
        suggestion = if state.last_move == MID_LEFT || state.last_move == NEAR_RIGHT {
            SPLIT_LEFT
        } else if state.last_move == MID_RIGHT || state.last_move == NEAR_LEFT {
            SPLIT_RIGHT
        } else {
            suggestion
        };
        state.consec_moves = 0;
    }

    state.trapped = new_trap;
    suggestion
}

/// Port of CKTpzStep from cktpzstr.c.
fn pz_step(state: &mut PzState, strat: i32, neighborhood: &mut [Option<usize>; 3]) -> bool {
    match strat {
        INIT => {
            if neighborhood[1].is_none() {
                neighborhood[1] = pz_seek(state, None, 0);
            } else if neighborhood[2].is_none() {
                neighborhood[2] = pz_seek(state, neighborhood[1], 1);
            } else if neighborhood[0].is_none() {
                neighborhood[0] = pz_seek(state, neighborhood[1], -1);
            }
        }
        SKIP_LEFT => {
            neighborhood[0] = pz_seek(state, neighborhood[0], -1);
        }
        SKIP_RIGHT => {
            neighborhood[2] = pz_seek(state, neighborhood[2], 1);
        }
        SHIFT_LEFT => {
            neighborhood[2] = neighborhood[1];
            neighborhood[1] = neighborhood[0];
            neighborhood[0] = pz_seek(state, neighborhood[0], -1);
        }
        SHIFT_RIGHT => {
            neighborhood[0] = neighborhood[1];
            neighborhood[1] = neighborhood[2];
            neighborhood[2] = pz_seek(state, neighborhood[2], 1);
        }
        _ => {}
    }

    neighborhood[0].is_some() && neighborhood[1].is_some() && neighborhood[2].is_some()
}

/// Port of pzseek from cktpzstr.c.
fn pz_seek(state: &mut PzState, start: Option<usize>, dir: i32) -> Option<usize> {
    state.guess_param = dir as f64;

    if state.trials.is_empty() {
        return None;
    }

    let start_idx = match start {
        Some(idx) => {
            if dir == 0 && state.trials.get(idx).map_or(false, |t|
                t.flags & ISAROOT == 0 && t.flags & ISAMINIMA == 0
            ) {
                return Some(idx);
            }
            idx
        }
        None => {
            if dir >= 0 {
                // Find first non-root, non-minima
                for (i, t) in state.trials.iter().enumerate() {
                    if t.flags & ISAROOT == 0 && t.flags & ISAMINIMA == 0 {
                        return Some(i);
                    }
                }
                return None;
            } else {
                // Find last non-root, non-minima
                for i in (0..state.trials.len()).rev() {
                    if state.trials[i].flags & ISAROOT == 0 && state.trials[i].flags & ISAMINIMA == 0 {
                        return Some(i);
                    }
                }
                return None;
            }
        }
    };

    if dir >= 0 {
        for i in (start_idx + 1)..state.trials.len() {
            if state.trials[i].flags & ISAROOT == 0 && state.trials[i].flags & ISAMINIMA == 0 {
                return Some(i);
            }
        }
    } else {
        for i in (0..start_idx).rev() {
            if state.trials[i].flags & ISAROOT == 0 && state.trials[i].flags & ISAMINIMA == 0 {
                return Some(i);
            }
        }
    }

    None
}

/// Port of CKTpzReset from cktpzstr.c.
fn pz_reset(state: &mut PzState, neighborhood: &mut [Option<usize>; 3]) {
    state.trapped = 0;
    state.consec_moves = 0;

    // Port of CKTpzReset: pzseek(ZeroTrial, 0)
    neighborhood[1] = pz_seek(state, state.zero_trial, 0);
    if let Some(n1) = neighborhood[1] {
        neighborhood[0] = pz_seek(state, Some(n1), -1);
        neighborhood[2] = pz_seek(state, Some(n1), 1);
    } else {
        neighborhood[0] = None;
        neighborhood[2] = None;
    }
}

/// Port of CKTpzUpdateSet from cktpzstr.c.
fn pz_update_set(state: &mut PzState, neighborhood: &mut [Option<usize>; 3], new_idx: usize) {
    let new_trial = &state.trials[new_idx];
    let mut this_move = 0;

    if new_trial.s.1 != 0.0 {
        neighborhood[2] = neighborhood[1];
        neighborhood[1] = neighborhood[0];
        neighborhood[0] = Some(new_idx);
    } else if neighborhood[1].is_none() {
        neighborhood[1] = Some(new_idx);
    } else {
        let n1 = &state.trials[neighborhood[1].unwrap()];
        if neighborhood[2].is_none() && new_trial.s.0 > n1.s.0 {
            neighborhood[2] = Some(new_idx);
        } else if neighborhood[0].is_none() {
            neighborhood[0] = Some(new_idx);
        } else if new_trial.flags & ISAMINIMA != 0 {
            neighborhood[1] = Some(new_idx);
        } else if let (Some(n0_idx), Some(n1_idx), Some(n2_idx)) =
            (neighborhood[0], neighborhood[1], neighborhood[2])
        {
            let n0 = &state.trials[n0_idx];
            let n1 = &state.trials[n1_idx];
            let n2 = &state.trials[n2_idx];

            if new_trial.s.0 < n0.s.0 {
                neighborhood[2] = neighborhood[1];
                neighborhood[1] = neighborhood[0];
                neighborhood[0] = Some(new_idx);
                this_move = FAR_LEFT;
            } else if new_trial.s.0 < n1.s.0 {
                if state.trapped == 0 || new_trial.mag_def < n1.mag_def
                    || (new_trial.mag_def == n1.mag_def
                        && new_trial.f_def.0.abs() < n1.f_def.0.abs())
                {
                    neighborhood[2] = neighborhood[1];
                    neighborhood[1] = Some(new_idx);
                    this_move = MID_LEFT;
                } else {
                    neighborhood[0] = Some(new_idx);
                    this_move = NEAR_LEFT;
                }
            } else if new_trial.s.0 < n2.s.0 {
                if state.trapped == 0 || new_trial.mag_def < n1.mag_def
                    || (new_trial.mag_def == n1.mag_def
                        && new_trial.f_def.0.abs() < n1.f_def.0.abs())
                {
                    neighborhood[0] = neighborhood[1];
                    neighborhood[1] = Some(new_idx);
                    this_move = MID_RIGHT;
                } else {
                    neighborhood[2] = Some(new_idx);
                    this_move = NEAR_RIGHT;
                }
            } else {
                neighborhood[0] = neighborhood[1];
                neighborhood[1] = neighborhood[2];
                neighborhood[2] = Some(new_idx);
                this_move = FAR_RIGHT;
            }
        }
    }

    if state.trapped != 0 && this_move == state.last_move {
        state.consec_moves += 1;
    } else {
        state.consec_moves = 0;
    }
    state.last_move = this_move;
}

/// Port of PZeval from cktpzstr.c.
fn pz_eval(
    state: &mut PzState,
    strat: i32,
    neighborhood: &[Option<usize>; 3],
) -> Result<PzTrial, SimError> {
    let mut new_trial = PzTrial::new(state.seq_num);
    state.seq_num += 1;

    match strat {
        GUESS => {
            if state.high_guess < state.low_guess {
                state.guess_param = 0.0;
            } else if state.guess_param > 0.0 {
                state.guess_param = if state.high_guess > 0.0 {
                    state.high_guess * 10.0
                } else {
                    1.0
                };
            } else {
                state.guess_param = if state.low_guess < 0.0 {
                    state.low_guess * 10.0
                } else {
                    -1.0
                };
            }
            if state.high_guess < state.guess_param {
                state.high_guess = state.guess_param;
            }
            if state.low_guess > state.guess_param {
                state.low_guess = state.guess_param;
            }
            new_trial.s.0 = state.guess_param;
            if let Some(n1_idx) = neighborhood[1] {
                new_trial.s.1 = state.trials[n1_idx].s.1;
            } else {
                new_trial.s.1 = 0.0;
            }
        }
        SYM | SYM2 => {
            // Port of NIpzSym2 from nipzmeth.c
            // Clone trials to avoid borrow conflict with &mut state
            let n0 = state.trials[neighborhood[0].unwrap()].clone();
            let n1 = state.trials[neighborhood[1].unwrap()].clone();
            let n2 = state.trials[neighborhood[2].unwrap()].clone();

            new_trial.s = nipz_sym2(&n0, &n1, &n2, state)?;
            new_trial.s.1 = 0.0;

            eprintln!("  SR_SYM2 inputs: n0=({:.20e},{:.20e}) fdef=({:.20e}) mag={}",
                n0.s.0, n0.s.1, n0.f_def.0, n0.mag_def);
            eprintln!("  SR_SYM2 inputs: n1=({:.20e},{:.20e}) fdef=({:.20e}) mag={}",
                n1.s.0, n1.s.1, n1.f_def.0, n1.mag_def);
            eprintln!("  SR_SYM2 inputs: n2=({:.20e},{:.20e}) fdef=({:.20e}) mag={}",
                n2.s.0, n2.s.1, n2.f_def.0, n2.mag_def);
            eprintln!("  SR_SYM2 raw result: ({:.20e},{:.20e}) trapped={}",
                new_trial.s.0, new_trial.s.1, state.trapped);

            // Fix up bad strategy results (port of cktpzstr.c:279-330)
            if state.trapped == 1 {
                if new_trial.s.0 < n0.s.0 || new_trial.s.0 > n1.s.0 {
                    new_trial.s.0 = (n0.s.0 + n1.s.0) / 2.0;
                }
            } else if state.trapped == 2 {
                if new_trial.s.0 < n1.s.0 || new_trial.s.0 > n2.s.0 {
                    new_trial.s.0 = (n1.s.0 + n2.s.0) / 2.0;
                }
            } else if state.trapped == 3 {
                if new_trial.s.0 <= n0.s.0
                    || (new_trial.s.0 == n1.s.0 && new_trial.s.1 == n1.s.1)
                    || new_trial.s.0 >= n2.s.0
                {
                    new_trial.s.0 = (n0.s.0 + n2.s.0) / 2.0;
                    if new_trial.s.0 == n1.s.0 {
                        if state.last_move == MID_LEFT || state.last_move == NEAR_RIGHT {
                            new_trial.s.0 = (n0.s.0 + n1.s.0) / 2.0;
                        } else {
                            new_trial.s.0 = (n1.s.0 + n2.s.0) / 2.0;
                        }
                    }
                }
            }
        }
        COMPLEX_INIT => {
            let n1 = &state.trials[neighborhood[1].unwrap()];
            new_trial.s.0 = n1.s.0;

            if state.nipzk != 0.0 && state.nipzk_mag > -10 {
                let mut k = state.nipzk;
                let mut k_mag = state.nipzk_mag;
                while k_mag > 0 { k *= 2.0; k_mag -= 1; }
                while k_mag < 0 { k /= 2.0; k_mag += 1; }
                new_trial.s.1 = k;
            } else {
                new_trial.s.1 = 10000.0;
            }

            // Reset NIpzK so the same value doesn't get used again (port of cktpzstr.c:367-368)
            state.nipzk = 0.0;
            state.nipzk_mag = 0;
        }
        COMPLEX_GUESS => {
            if neighborhood[2].is_none() {
                let n0 = &state.trials[neighborhood[0].unwrap()];
                new_trial.s.0 = n0.s.0;
                new_trial.s.1 = 1.0e8;
            } else {
                let n0 = &state.trials[neighborhood[0].unwrap()];
                new_trial.s.0 = n0.s.0;
                new_trial.s.1 = 1.0e12;
            }
        }
        MULLER => {
            let n0 = &state.trials[neighborhood[0].unwrap()];
            let n1 = &state.trials[neighborhood[1].unwrap()];
            let n2 = &state.trials[neighborhood[2].unwrap()];
            new_trial.s = nipz_muller(n0, n1, n2)?;
        }
        SPLIT_LEFT => {
            let n0 = &state.trials[neighborhood[0].unwrap()];
            let n1 = &state.trials[neighborhood[1].unwrap()];
            new_trial.s.0 = (n0.s.0 + 2.0 * n1.s.0) / 3.0;
        }
        SPLIT_RIGHT => {
            let n1 = &state.trials[neighborhood[1].unwrap()];
            let n2 = &state.trials[neighborhood[2].unwrap()];
            new_trial.s.0 = (n2.s.0 + 2.0 * n1.s.0) / 3.0;
        }
        _ => {
            return Err(SimError::Other("PZ: unknown strategy".to_string()));
        }
    }

    Ok(new_trial)
}

/// Port of NIpzSym2 from nipzmeth.c.
fn nipz_sym2(
    n0: &PzTrial,
    n1: &PzTrial,
    n2: &PzTrial,
    state: &mut PzState,
) -> Result<(f64, f64), SimError> {
    let dx0 = n1.s.0 - n0.s.0;
    let dx1 = n2.s.0 - n1.s.0;
    let x0 = (n0.s.0 + n1.s.0) / 2.0;
    let d2x = (n2.s.0 - n0.s.0) / 2.0;

    let (mut a, mut a_mag) = zaddeq(n1.f_def.0, n1.mag_def, -n0.f_def.0, n0.mag_def);
    let mut tmag: i32 = 0;
    let mut dx0_n = dx0;
    r_norm(&mut dx0_n, &mut tmag);
    a /= dx0_n;
    a_mag -= tmag;
    r_norm(&mut a, &mut a_mag);

    let (mut b, mut b_mag) = zaddeq(n2.f_def.0, n2.mag_def, -n1.f_def.0, n1.mag_def);
    tmag = 0;
    let mut dx1_n = dx1;
    r_norm(&mut dx1_n, &mut tmag);
    b /= dx1_n;
    b_mag -= tmag;
    r_norm(&mut b, &mut b_mag);

    let (mut c, mut c_mag) = zaddeq(b, b_mag, -a, a_mag);
    tmag = 0;
    let mut d2x_n = d2x;
    r_norm(&mut d2x_n, &mut tmag);
    c /= d2x_n;
    c_mag -= tmag;
    r_norm(&mut c, &mut c_mag);

    // NOTE: ngspice line 261 has a C bug: `(b = 0.0 || c_mag < b_mag - 40)`.
    // This is an assignment (`b = ...`), not comparison (`b == ...`).
    // In C, `b = (0.0 || (c_mag < b_mag - 40))` sets b to 0 or 1 (boolean result),
    // and the entire expression evaluates to that boolean.
    // Short-circuit: b is only clobbered if the && LHS is true (and c != 0.0).
    let b_test = c_mag < b_mag - 40;
    let first_half = a == 0.0 || c_mag < a_mag - 40;

    // In C: if (c == 0) -> whole thing is true, b NOT clobbered (short-circuit on ||)
    //        if (c != 0 && first_half is false) -> whole thing is false, b NOT clobbered (short-circuit on &&)
    //        if (c != 0 && first_half is true) -> b IS clobbered, result depends on b_test
    let enters_linear = if c == 0.0 {
        true  // b is NOT clobbered
    } else if !first_half {
        false  // b is NOT clobbered
    } else {
        // b IS clobbered here
        b = if b_test { 1.0 } else { 0.0 };
        b_test
    };

    if enters_linear {
        if a == 0.0 {
            a = b;
            a_mag = b_mag;
        }
        if a != 0.0 {
            // Port of: new->s.real = -set[1]->f_def.real / a;
            //          a_mag -= set[1]->mag_def;
            //          while (a_mag > 0) { new->s.real /= 2.0; a_mag--; }
            //          while (a_mag < 0) { new->s.real *= 2.0; a_mag++; }
            //          new->s.real += set[1]->s.real;
            let mut result = -n1.f_def.0 / a;
            let mut exp = a_mag - n1.mag_def;
            while exp > 0 { result /= 2.0; exp -= 1; }
            while exp < 0 { result *= 2.0; exp += 1; }
            return Ok((result + n1.s.0, 0.0));
        } else {
            return Ok((n1.s.0, 0.0));
        }
    }

    // Quadratic case
    a /= c;
    r_norm(&mut a, &mut a_mag);
    a_mag -= c_mag;

    let diff = n1.s.0 - x0;
    let mut diff_v = diff;
    tmag = 0;
    r_norm(&mut diff_v, &mut tmag);

    let (a_new, a_mag_new) = zaddeq(a, a_mag, diff_v, tmag);
    a = a_new;
    a_mag = a_mag_new;

    b = 2.0 * n1.f_def.0 / c;
    b_mag = n1.mag_def - c_mag;
    r_norm(&mut b, &mut b_mag);

    let mut disc = a * a;
    let mut disc_mag = 2 * a_mag;

    let (disc_new, disc_mag_new) = zaddeq(disc, disc_mag, -b, b_mag);
    disc = disc_new;
    disc_mag = disc_mag_new;

    let mut new_mag = 0;
    if disc < 0.0 {
        disc *= -1.0;
        new_mag = 1;
    }

    if disc_mag % 2 == 0 {
        disc = disc.sqrt();
    } else {
        disc = (2.0 * disc).sqrt();
        disc_mag -= 1;
    }
    disc_mag /= 2;

    if new_mag != 0 {
        // Complex root — save discriminant for COMPLEX_INIT (port of nipzmeth.c:323-340)
        if state.nipzk == 0.0 {
            state.nipzk = disc;
            state.nipzk_mag = disc_mag;
        }
        disc = 0.0;
        disc_mag = 0;
    }

    let (mut c_val, mut c_mag_val);
    if a * disc >= 0.0 {
        let r = zaddeq(a, a_mag, disc, disc_mag);
        c_val = r.0;
        c_mag_val = r.1;
    } else {
        let r = zaddeq(a, a_mag, -disc, disc_mag);
        c_val = r.0;
        c_mag_val = r.1;
    }

    // Second root = b / c
    let mut b_val;
    let mut b_mag_val;
    if c_val != 0.0 {
        b_val = b / c_val;
        b_mag_val = b_mag - c_mag_val;
    } else {
        b_val = 0.0;
        b_mag_val = 0;
    }

    let (b_s, b_s_mag) = zaddeq(n1.s.0, 0, -b_val, b_mag_val);
    b_val = b_s;
    b_mag_val = b_s_mag;

    let (c_s, c_s_mag) = zaddeq(n1.s.0, 0, -c_val, c_mag_val);
    c_val = c_s;
    c_mag_val = c_s_mag;

    // Denormalize
    while b_mag_val > 0 { b_val *= 2.0; b_mag_val -= 1; }
    while b_mag_val < 0 { b_val /= 2.0; b_mag_val += 1; }
    while c_mag_val > 0 { c_val *= 2.0; c_mag_val -= 1; }
    while c_mag_val < 0 { c_val /= 2.0; c_mag_val += 1; }

    // Choose the best root
    let n0_s = n0.s.0;
    let n2_s = n2.s.0;

    let result = if b_val < n0_s || b_val > n2_s {
        if c_val < n0_s || c_val > n2_s {
            if state.trapped == 1 {
                (n0_s + n1.s.0) / 2.0
            } else if state.trapped == 2 {
                (n1.s.0 + n2_s) / 2.0
            } else if state.trapped == 3 {
                if (n1.s.0 - c_val).abs() < (n1.s.0 - b_val).abs() {
                    (n1.s.0 + c_val) / 2.0
                } else {
                    (n1.s.0 + b_val) / 2.0
                }
            } else {
                n1.s.0 // fallback
            }
        } else {
            c_val
        }
    } else if c_val < n0_s || c_val > n2_s {
        b_val
    } else {
        // Both in range — take the one closer to root (based on slope direction)
        if a > 0.0 { b_val } else { c_val }
    };

    Ok((result, 0.0))
}

/// Port of NIpzMuller from nipzmeth.c.
fn nipz_muller(
    n0: &PzTrial,
    n1: &PzTrial,
    n2: &PzTrial,
) -> Result<(f64, f64), SimError> {
    // Scale factors
    let mut min = -999999_i32;
    let mut j = 0_i32;
    let mut total = 0_i32;
    let sets = [n2, n1, n0]; // set[0]=n2, set[1]=n1, set[2]=n0 in ngspice ordering

    for i in 0..3 {
        if sets[i].f_def.0 != 0.0 || sets[i].f_def.1 != 0.0 {
            if min < sets[i].mag_def - 50 {
                min = sets[i].mag_def - 50;
            }
            total += sets[i].mag_def;
            j += 1;
        }
    }

    let magx = if j != 0 { total / j } else { total };
    let magx = if magx < min { min } else { magx };

    let mut scale = [1.0_f64; 3];
    for i in 0..3 {
        let mut mag_diff = sets[i].mag_def - magx;
        scale[i] = 1.0;
        while mag_diff > 0 { scale[i] *= 2.0; mag_diff -= 1; }
        if mag_diff < -90 {
            scale[i] = 0.0;
        } else {
            while mag_diff < 0 { scale[i] /= 2.0; mag_diff += 1; }
        }
    }

    // h0 = set[0]->s - set[1]->s
    let h0 = (sets[0].s.0 - sets[1].s.0, sets[0].s.1 - sets[1].s.1);
    // h1 = set[1]->s - set[2]->s
    let h1 = (sets[1].s.0 - sets[2].s.0, sets[1].s.1 - sets[2].s.1);
    // lambda = h0 / h1
    let lambda = c_div(h0.0, h0.1, h1.0, h1.1);
    // delta = lambda + 1
    let delta = (lambda.0 + 1.0, lambda.1);

    // A = lambda * (f[i-2] * lambda - f[i-1] * delta + f[i])
    let sf2 = (scale[2] * sets[2].f_def.0, scale[2] * sets[2].f_def.1);
    let sf1 = (scale[1] * sets[1].f_def.0, scale[1] * sets[1].f_def.1);
    let sf0 = (scale[0] * sets[0].f_def.0, scale[0] * sets[0].f_def.1);

    let a_t1 = c_mul(sf2.0, sf2.1, lambda.0, lambda.1);
    let a_t2 = c_mul(sf1.0, sf1.1, delta.0, delta.1);
    let a_inner = (a_t1.0 - a_t2.0 + sf0.0, a_t1.1 - a_t2.1 + sf0.1);
    let mut a_val = c_mul(a_inner.0, a_inner.1, lambda.0, lambda.1);

    // B = f[i-2]*lambda^2 - f[i-1]*delta^2 + f[i]*(lambda+delta)
    let lam2 = c_mul(lambda.0, lambda.1, lambda.0, lambda.1);
    let del2 = c_mul(delta.0, delta.1, delta.0, delta.1);
    let b_t1 = c_mul(lam2.0, lam2.1, sf2.0, sf2.1);
    let b_t2 = c_mul(del2.0, del2.1, sf1.0, sf1.1);
    let lam_del = (lambda.0 + delta.0, lambda.1 + delta.1);
    let b_t3 = c_mul(lam_del.0, lam_del.1, sf0.0, sf0.1);
    let mut b_val = (b_t1.0 - b_t2.0 + b_t3.0, b_t1.1 - b_t2.1 + b_t3.1);

    // C = delta * f[i]
    let mut c_val = c_mul(delta.0, delta.1, sf0.0, sf0.1);

    // Normalize
    while a_val.0.abs() > 1.0 || a_val.1.abs() > 1.0
        || b_val.0.abs() > 1.0 || b_val.1.abs() > 1.0
        || c_val.0.abs() > 1.0 || c_val.1.abs() > 1.0
    {
        a_val.0 /= 2.0; a_val.1 /= 2.0;
        b_val.0 /= 2.0; b_val.1 /= 2.0;
        c_val.0 /= 2.0; c_val.1 /= 2.0;
    }

    // D = B^2 - 4*A*C
    let b2 = c_mul(b_val.0, b_val.1, b_val.0, b_val.1);
    let ac4 = c_mul(4.0 * a_val.0, 4.0 * a_val.1, c_val.0, c_val.1);
    let mut d_re = b2.0 - ac4.0;
    let mut d_im = b2.1 - ac4.1;

    c_sqrt(&mut d_re, &mut d_im);

    // Maximize denominator: dot product test
    let q = b_val.0 * d_re + b_val.1 * d_im;
    if q > 0.0 {
        b_val.0 += d_re;
        b_val.1 += d_im;
    } else {
        b_val.0 -= d_re;
        b_val.1 -= d_im;
    }

    // lambda_new = -2*C / B
    let denom = (-0.5 * b_val.0, -0.5 * b_val.1);
    let lambda_new = c_div(c_val.0, c_val.1, denom.0, denom.1);

    // newtry->s = h0 * lambda_new + set[0]->s
    let step = c_mul(h0.0, h0.1, lambda_new.0, lambda_new.1);
    Ok((step.0 + sets[0].s.0, step.1 + sets[0].s.1))
}
