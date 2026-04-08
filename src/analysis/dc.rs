use crate::circuit::Circuit;
use crate::config::SimConfig;
use crate::device::vsource::VoltageSource;
use crate::device::isource::CurrentSource;
use crate::error::SimError;
use crate::mode::*;
use crate::solver::{ni_iter, SimState};

/// DC operating point with UIC — port of ngspice NIiter with MODEUIC|MODETRANOP.
///
/// ngspice niiter.c:50-56: when (MODETRANOP && MODEUIC), NIiter just swaps
/// rhs/rhs_old, loads devices once, and returns OK. No NR iteration, no
/// factorization. The first factorization happens at the transient step.
pub fn dc_operating_point_uic(
    circuit: &mut Circuit,
    config: &SimConfig,
    ic_values: &[(String, f64)],
) -> Result<SimState, SimError> {
    let size = circuit.num_equations() - 1;
    let mut sim = SimState::new(size, config);

    for device in &mut circuit.devices {
        device.setup_matrix(&mut sim.mna);
    }

    // Apply .IC node voltages to rhs and rhs_old (cktic.c:49-71)
    for (name, val) in ic_values {
        if let Some(eq) = circuit.find_node(name) {
            sim.mna.rhs_old[eq] = *val;
            sim.mna.rhs[eq] = *val;
        }
    }

    // DEVsetic: propagate .IC node voltages to device ICs (cktic.c:74-81)
    // e.g. CAPgetic reads rhs[pos]-rhs[neg] into CAPinitCond
    for device in &mut circuit.devices {
        device.setic(&sim.mna.rhs);
    }

    // niiter.c:50-56: SWAP(rhs, rhs_old), CKTload, return OK.
    // No NR iteration — just a single device load to set initial state.
    // The matrix is NOT factored; first factorization happens at transient.
    sim.mode = Mode::new(MODEUIC | MODETRANOP | MODEINITJCT);
    sim.mna.swap_rhs();
    sim.mna.clear();
    // Pre-load pass (inductor flux computation)
    for device in &mut circuit.devices {
        device.pre_load(&mut sim.mna, &mut circuit.states, sim.mode);
    }
    for device in &mut circuit.devices {
        let mut noncon = false;
        device.load(&mut sim.mna, &mut circuit.states, sim.mode, sim.src_fact, sim.gmin, &mut noncon)?;
    }

    Ok(sim)
}

/// DC operating point — faithful port of ngspice CKTop (cktop.c:26-116).
///
/// Tries the following convergence strategies in order:
/// 1. Direct Newton-Raphson
/// 2. Dynamic gmin stepping
/// 3. Gillespie source stepping
///
/// Returns Ok(SimState) with converged solution on success.
pub fn dc_operating_point(
    circuit: &mut Circuit,
    config: &SimConfig,
) -> Result<SimState, SimError> {
    dc_operating_point_with_mode(circuit, config, MODEDCOP)
}

/// DC operating point for transient — uses MODETRANOP instead of MODEDCOP.
/// ngspice dctran.c:266-269: CKTop(ckt, MODETRANOP|MODEINITJCT, ...)
pub fn dc_operating_point_tran(
    circuit: &mut Circuit,
    config: &SimConfig,
) -> Result<SimState, SimError> {
    dc_operating_point_with_mode(circuit, config, MODETRANOP)
}

fn dc_operating_point_with_mode(
    circuit: &mut Circuit,
    config: &SimConfig,
    dc_mode: u32,
) -> Result<SimState, SimError> {
    let size = circuit.num_equations() - 1; // exclude ground
    let mut sim = SimState::new(size, config);

    // Pre-allocate matrix elements (ngspice DEVsetup TSTALLOC)
    for device in &mut circuit.devices {
        device.setup_matrix(&mut sim.mna);
    }

    // Apply .NODESET initial values (CKTic.c)
    sim.apply_nodesets(&circuit.nodes);

    // 1. Direct NIiter (cktop.c:46)
    sim.mode = Mode::new(dc_mode | MODEINITJCT);
    match ni_iter(&mut sim, circuit, config, config.dc_max_iter) {
        Ok(_) => return Ok(sim),
        Err(_) => {} // fall through to gmin stepping
    }

    // 2. Gmin stepping (cktop.c:57-77)
    if config.num_gmin_steps >= 1 {
        if config.num_gmin_steps == 1 {
            // Default path: dynamic_gmin first, then new_gmin (cktop.c:65-70)
            match dynamic_gmin(&mut sim, circuit, config, dc_mode) {
                Ok(()) => return Ok(sim),
                Err(_) => {
                    match new_gmin(&mut sim, circuit, config, dc_mode) {
                        Ok(()) => return Ok(sim),
                        Err(_) => {} // fall through to source stepping
                    }
                }
            }
        } else {
            // numGminSteps > 1: use spice3 gmin (not yet implemented, use dynamic_gmin)
            match dynamic_gmin(&mut sim, circuit, config, dc_mode) {
                Ok(()) => return Ok(sim),
                Err(_) => {}
            }
        }
    }

    // 3. Source stepping (cktop.c:87)
    if config.num_src_steps >= 1 {
        match gillespie_src(&mut sim, circuit, config, dc_mode) {
            Ok(()) => return Ok(sim),
            Err(_) => {}
        }
    }

    Err(SimError::NoConvergence)
}

/// Dynamic gmin stepping — port of cktop.c:161-274.
fn dynamic_gmin(
    sim: &mut SimState,
    circuit: &mut Circuit,
    config: &SimConfig,
    dc_mode: u32,
) -> Result<(), SimError> {
    sim.mode = Mode::new(dc_mode | MODEINITJCT);

    // Zero solution + state (cktop.c:183-186)
    sim.zero_solution();
    circuit.states.zero_state0();

    // Stepping parameters (cktop.c:188-191)
    let mut factor = config.gmin_factor;
    let mut old_gmin = 1e-2;
    sim.diag_gmin = old_gmin / factor;
    let gtarget = f64::max(config.gmin, config.gshunt);

    // Save/restore buffers
    let num_nodes = circuit.num_equations();
    let mut old_rhs = vec![0.0; num_nodes];
    let mut old_state0 = vec![0.0; circuit.states.len()];

    loop {
        sim.noncon = 1;

        match ni_iter(sim, circuit, config, config.dc_trcv_max_iter) {
            Ok(_) => {
                // ni_iter resets iter_count to 0, so after the call it IS the count
                let iters = sim.iter_count;

                sim.mode = Mode::new(dc_mode | MODEINITFLOAT);

                if sim.diag_gmin <= gtarget {
                    break; // SUCCESS — reached target gmin
                }

                // Save solution (cktop.c:210-214)
                old_rhs[..num_nodes].copy_from_slice(&sim.mna.rhs_old[..num_nodes]);
                old_state0[..circuit.states.len()]
                    .copy_from_slice(&circuit.states.state0()[..circuit.states.len()]);

                // Adaptive factor (cktop.c:216-223)
                if iters <= config.dc_trcv_max_iter / 4 {
                    factor *= factor.sqrt();
                    if factor > config.gmin_factor {
                        factor = config.gmin_factor;
                    }
                }
                if iters > 3 * config.dc_trcv_max_iter / 4 {
                    factor = f64::max(factor.sqrt(), 1.00005);
                }

                old_gmin = sim.diag_gmin;

                // Reduce gmin (cktop.c:227-231)
                if sim.diag_gmin < factor * gtarget {
                    factor = sim.diag_gmin / gtarget;
                    sim.diag_gmin = gtarget;
                } else {
                    sim.diag_gmin /= factor;
                }
            }
            Err(_) => {
                if factor < 1.00005 {
                    break; // FAILED — factor too small, fall through to final solve
                }
                // Backtrack (cktop.c:241-248)
                factor = factor.sqrt().sqrt(); // 4th root
                sim.diag_gmin = old_gmin / factor;
                sim.mna.rhs_old[..num_nodes].copy_from_slice(&old_rhs[..num_nodes]);
                let nstates = circuit.states.len();
                circuit.states.state0_mut()[..nstates]
                    .copy_from_slice(&old_state0[..nstates]);
            }
        }
    }

    // Final solve at target gmin (cktop.c:252-261)
    // ngspice ALWAYS does this regardless of how the loop exited
    sim.diag_gmin = config.gshunt;
    ni_iter(sim, circuit, config, config.dc_max_iter)?;

    Ok(())
}

/// "True" (new) gmin stepping — port of cktop.c:349-466.
/// Steps the actual per-device CKTgmin (not just diagonal CKTdiagGmin).
/// This is the fallback when dynamic_gmin fails.
fn new_gmin(
    sim: &mut SimState,
    circuit: &mut Circuit,
    config: &SimConfig,
    dc_mode: u32,
) -> Result<(), SimError> {
    sim.mode = Mode::new(dc_mode | MODEINITJCT);

    // Zero solution + state (cktop.c:370-374)
    sim.zero_solution();
    circuit.states.zero_state0();

    // Save original gmin and set stepping parameters (cktop.c:376-380)
    let start_gmin = sim.gmin;
    let mut factor = 2.0_f64; // Use smaller factor for true gmin (vs 10 for diagonal)
    let mut old_gmin = 1e-2;
    sim.gmin = old_gmin / factor;
    // Also set diag_gmin to match device gmin for extra stability
    sim.diag_gmin = sim.gmin;
    let gtarget = f64::max(start_gmin, config.gshunt);

    // Save/restore buffers
    let num_nodes = circuit.num_equations();
    let mut old_rhs = vec![0.0; num_nodes];
    let mut old_state0 = vec![0.0; circuit.states.len()];

    loop {
        sim.noncon = 1;

        match ni_iter(sim, circuit, config, config.dc_trcv_max_iter) {
            Ok(_) => {
                let iters = sim.iter_count;

                sim.mode = Mode::new(dc_mode | MODEINITFLOAT);

                if sim.gmin <= gtarget {
                    break; // SUCCESS
                }

                // Save solution
                old_rhs[..num_nodes].copy_from_slice(&sim.mna.rhs_old[..num_nodes]);
                old_state0[..circuit.states.len()]
                    .copy_from_slice(&circuit.states.state0()[..circuit.states.len()]);

                // Adaptive factor (cktop.c:405-412) — note: uses MAX(sqrt(factor), 3) not 1.00005
                if iters <= config.dc_trcv_max_iter / 4 {
                    factor *= factor.sqrt();
                    if factor > config.gmin_factor {
                        factor = config.gmin_factor;
                    }
                }
                if iters > 3 * config.dc_trcv_max_iter / 4 {
                    factor = f64::max(factor.sqrt(), 3.0);
                }

                old_gmin = sim.gmin;

                // Reduce gmin (cktop.c:416-422)
                if sim.gmin < factor * gtarget {
                    factor = sim.gmin / gtarget;
                    sim.gmin = gtarget;
                } else {
                    sim.gmin /= factor;
                }
                // Keep diag_gmin in sync with device gmin
                sim.diag_gmin = sim.gmin;
            }
            Err(_) => {
                if factor < 1.00005 {
                    break; // FAILED
                }
                // Backtrack (cktop.c:432-439)
                factor = factor.sqrt().sqrt();
                sim.gmin = old_gmin / factor;
                sim.diag_gmin = sim.gmin;
                sim.mna.rhs_old[..num_nodes].copy_from_slice(&old_rhs[..num_nodes]);
                let nstates = circuit.states.len();
                circuit.states.state0_mut()[..nstates]
                    .copy_from_slice(&old_state0[..nstates]);
            }
        }
    }

    // Restore gmin (cktop.c:443)
    sim.gmin = f64::max(start_gmin, config.gshunt);
    sim.diag_gmin = config.gshunt;

    // Final solve (cktop.c:452)
    ni_iter(sim, circuit, config, config.dc_max_iter)?;

    Ok(())
}

/// Gillespie source stepping — port of cktop.c:480-658.
fn gillespie_src(
    sim: &mut SimState,
    circuit: &mut Circuit,
    config: &SimConfig,
    dc_mode: u32,
) -> Result<(), SimError> {
    sim.mode = Mode::new(dc_mode | MODEINITJCT);

    // Zero solution + state (cktop.c:497-501)
    sim.zero_solution();
    circuit.states.zero_state0();
    sim.src_fact = 0.0;

    // Phase 1: solve with zero sources (cktop.c:506)
    if ni_iter(sim, circuit, config, config.dc_trcv_max_iter).is_err() {
        // Bootstrap with gmin steps (cktop.c:514-548)
        let gmin_base = if config.gshunt <= 0.0 {
            config.gmin
        } else {
            config.gshunt
        };
        sim.diag_gmin = gmin_base;
        for _ in 0..10 {
            sim.diag_gmin *= 10.0;
        }

        let mut bootstrap_ok = false;
        for step in 0..=10 {
            sim.noncon = 1;
            if ni_iter(sim, circuit, config, config.dc_trcv_max_iter).is_ok() {
                sim.diag_gmin /= 10.0;
                sim.mode = Mode::new(dc_mode | MODEINITFLOAT);
                bootstrap_ok = true;
            } else {
                sim.diag_gmin = config.gshunt;
                break;
            }
        }
        sim.diag_gmin = config.gshunt;
        if !bootstrap_ok {
            return Err(SimError::NoConvergence);
        }
    }

    // Phase 2: ramp sources 0→100% (cktop.c:553-641)
    let num_nodes = circuit.num_equations();
    let mut old_rhs = vec![0.0; num_nodes];
    let mut old_state0 = vec![0.0; circuit.states.len()];

    // Save zero-source solution
    old_rhs[..num_nodes].copy_from_slice(&sim.mna.rhs_old[..num_nodes]);
    old_state0[..circuit.states.len()]
        .copy_from_slice(&circuit.states.state0()[..circuit.states.len()]);

    let mut raise = 0.001;
    let mut conv_fact = 0.0;
    sim.src_fact = raise;

    loop {
        let converged = ni_iter(sim, circuit, config, config.dc_trcv_max_iter);
        let iters = sim.iter_count;

        // ngspice sets mode unconditionally after NIiter (cktop.c:588)
        sim.mode = Mode::new(dc_mode | MODEINITFLOAT);

        match converged {
            Ok(_) => {
                conv_fact = sim.src_fact;

                // Save solution
                old_rhs[..num_nodes].copy_from_slice(&sim.mna.rhs_old[..num_nodes]);
                old_state0[..circuit.states.len()]
                    .copy_from_slice(&circuit.states.state0()[..circuit.states.len()]);

                sim.src_fact = conv_fact + raise;

                // Adaptive (cktop.c:603-607)
                if iters <= config.dc_trcv_max_iter / 4 {
                    raise *= 1.5;
                }
                if iters > 3 * config.dc_trcv_max_iter / 4 {
                    raise *= 0.5;
                }
            }
            Err(_) => {
                if sim.src_fact - conv_fact < 1e-8 {
                    break;
                }
                raise /= 10.0;
                if raise > 0.01 {
                    raise = 0.01;
                }
                sim.src_fact = conv_fact;

                // Restore (cktop.c:631-635)
                sim.mna.rhs_old[..num_nodes].copy_from_slice(&old_rhs[..num_nodes]);
                let nstates = circuit.states.len();
                circuit.states.state0_mut()[..nstates]
                    .copy_from_slice(&old_state0[..nstates]);
            }
        }

        if sim.src_fact > 1.0 {
            sim.src_fact = 1.0;
        }

        if raise < 1e-7 || conv_fact >= 1.0 {
            break;
        }
    }

    sim.src_fact = 1.0;

    if conv_fact < 1.0 {
        return Err(SimError::NoConvergence);
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// DC Sweep — faithful port of ngspice DCtrCurv (dctrcurv.c)
// ═══════════════════════════════════════════════════════════════════

/// Result of a DC sweep: sweep values and per-point solution vectors.
pub struct DcSweepResult {
    /// The sweep variable values (length = number of sweep points)
    pub sweep_values: Vec<f64>,
    /// Per-sweep-point solution vectors (rhs_old after convergence)
    pub values: Vec<Vec<f64>>,
}

/// DC sweep analysis — port of ngspice DCtrCurv (dctrcurv.c:34-535).
///
/// For each sweep value: set the source DC value, run DC OP, collect results.
/// Supports single source sweeps (V or I).
/// For nested sweeps, the first source is inner (fast) and second is outer (slow).
pub fn dc_sweep(
    circuit: &mut Circuit,
    config: &SimConfig,
    src1_name: &str,
    start1: f64,
    stop1: f64,
    step1: f64,
    src2_name: Option<&str>,
    start2: f64,
    stop2: f64,
    step2: f64,
) -> Result<DcSweepResult, SimError> {
    let size = circuit.num_equations() - 1;
    let mut sim = SimState::new(size, config);

    // Pre-allocate matrix elements (ngspice DEVsetup TSTALLOC)
    for device in &mut circuit.devices {
        device.setup_matrix(&mut sim.mna);
    }

    // Apply .NODESET initial values
    sim.apply_nodesets(&circuit.nodes);

    // Find sweep source indices
    let src1_idx = find_source(circuit, src1_name)
        .ok_or_else(|| SimError::DeviceNotFound(src1_name.to_string()))?;
    let src2_idx = src2_name.and_then(|name| find_source(circuit, name));

    // Save original DC values (dctrcurv.c:108-123, 515-530)
    let save1 = get_source_dc(circuit, src1_idx);
    let save2 = src2_idx.map(|idx| get_source_dc(circuit, idx));

    // Set initial sweep values (dctrcurv.c:101-123)
    set_source_dc(circuit, src1_idx, start1);
    if let Some(idx) = src2_idx {
        set_source_dc(circuit, idx, start2);
    }

    // dctrcurv.c:78-86: initialize mode, timing, and delta
    sim.mode = Mode::new(MODEDCTRANCURVE | MODEINITJCT);
    // CKTdelta = step[0], CKTdeltaOld[0..7] = step[0] (dctrcurv.c:79-86)
    set_device_deltas(circuit, step1);

    let mut result = DcSweepResult {
        sweep_values: Vec::new(),
        values: Vec::new(),
    };

    let mut first_time = true;

    // Outer loop (second sweep variable, or just one pass if no nested sweep)
    let mut val2 = start2;
    loop {
        // Inner loop (first sweep variable)
        let mut val1 = start1;
        loop {
            // Check termination for inner sweep (dctrcurv.c:218-229)
            if sgn(step1) * (val1 - stop1) > f64::EPSILON * 1e3 {
                break;
            }

            // Rotate state vectors (dctrcurv.c:290-293)
            // CKTmaxOrder = 2 (same as transient)
            circuit.states.rotate(2);

            // Do operation: try NIiter first, fall back to CKTop (dctrcurv.c:302-320)
            // non-hs path: NIiter(dcTrcvMaxIter), if fails → CKTop(dcMaxIter)
            let dc_mode = MODEDCTRANCURVE;
            let ni_result = ni_iter(&mut sim, circuit, config, config.dc_trcv_max_iter);
            let converged = match ni_result {
                Ok(_) => Ok(()),
                Err(_) => {
                    solve_dc_point(&mut sim, circuit, config, dc_mode)
                },
            };

            if converged.is_err() {
                // Restore original values and return error
                set_source_dc(circuit, src1_idx, save1);
                if let (Some(idx), Some(sv)) = (src2_idx, save2) {
                    set_source_dc(circuit, idx, sv);
                }
                return Err(SimError::NoConvergence);
            }

            // After convergence: set mode to MODEINITPRED (dctrcurv.c:375)
            sim.mode = Mode::new(MODEDCTRANCURVE | MODEINITPRED);

            // First time: copy state0 to state1 (dctrcurv.c:458-462)
            if first_time {
                first_time = false;
                circuit.states.copy_state0_to_state1();
            }

            // Collect output (dctrcurv.c:445: CKTdump)
            result.sweep_values.push(val1);
            result.values.push(sim.mna.rhs_old.clone());

            // Advance inner sweep (dctrcurv.c:469-471)
            val1 += step1;
            set_source_dc(circuit, src1_idx, val1);
        }

        // Check if we have a nested sweep
        if src2_idx.is_none() {
            break;
        }

        // Advance outer sweep (dctrcurv.c:469-481 for outer variable)
        val2 += step2;
        if sgn(step2) * (val2 - stop2) > f64::EPSILON * 1e3 {
            break;
        }
        set_source_dc(circuit, src2_idx.unwrap(), val2);

        // Reset inner sweep to start (dctrcurv.c:272-274)
        set_source_dc(circuit, src1_idx, start1);

        // Re-enter INITJCT for new outer value (dctrcurv.c:226)
        first_time = true;
        sim.mode = Mode::new(MODEDCTRANCURVE | MODEINITJCT);
    }

    // Restore original DC values (dctrcurv.c:514-530)
    set_source_dc(circuit, src1_idx, save1);
    if let (Some(idx), Some(sv)) = (src2_idx, save2) {
        set_source_dc(circuit, idx, sv);
    }

    Ok(result)
}

/// Solve a single DC operating point within the sweep.
/// Port of the non-hs path in dctrcurv.c:311-320: NIiter then CKTop.
/// But for the first point, always uses CKTop (which includes gmin stepping).
fn solve_dc_point(
    sim: &mut SimState,
    circuit: &mut Circuit,
    config: &SimConfig,
    dc_mode: u32,
) -> Result<(), SimError> {
    sim.mode = Mode::new(dc_mode | MODEINITJCT);
    match ni_iter(sim, circuit, config, config.dc_max_iter) {
        Ok(_) => return Ok(()),
        Err(_) => {}
    }

    // gmin stepping (same as CKTop: dynamic_gmin then new_gmin for numGminSteps==1)
    if config.num_gmin_steps >= 1 {
        if config.num_gmin_steps == 1 {
            match dynamic_gmin(sim, circuit, config, dc_mode) {
                Ok(()) => return Ok(()),
                Err(_) => {
                    match new_gmin(sim, circuit, config, dc_mode) {
                        Ok(()) => return Ok(()),
                        Err(_) => {}
                    }
                }
            }
        } else {
            match dynamic_gmin(sim, circuit, config, dc_mode) {
                Ok(()) => return Ok(()),
                Err(_) => {}
            }
        }
    }

    // Source stepping
    if config.num_src_steps >= 1 {
        match gillespie_src(sim, circuit, config, dc_mode) {
            Ok(()) => return Ok(()),
            Err(_) => {}
        }
    }

    Err(SimError::NoConvergence)
}

/// Set delta/delta_old1 on all devices that use predictors.
/// Port of dctrcurv.c:79-86: CKTdelta = step, CKTdeltaOld[0..7] = step.
/// This prevents 0/0 = NaN in the predictor xfact calculation.
fn set_device_deltas(circuit: &mut Circuit, delta: f64) {
    use crate::device::diode::Diode;
    use crate::device::mosfet1::Mosfet1;
    use crate::device::mosfet2::Mosfet2;
    use crate::device::mosfet3::Mosfet3;
    use crate::device::bsim3::Bsim3;
    use crate::device::bjt::Bjt;
    use crate::device::jfet::Jfet;

    for device in &mut circuit.devices {
        if let Some(dio) = device.as_any_mut().downcast_mut::<Diode>() {
            dio.delta = delta;
            dio.delta_old1 = delta;
        }
        if let Some(mos) = device.as_any_mut().downcast_mut::<Mosfet1>() {
            mos.delta = delta;
            mos.delta_old1 = delta;
        }
        if let Some(mos) = device.as_any_mut().downcast_mut::<Mosfet2>() {
            mos.delta = delta;
            mos.delta_old1 = delta;
        }
        if let Some(mos) = device.as_any_mut().downcast_mut::<Mosfet3>() {
            mos.delta = delta;
            mos.delta_old1 = delta;
        }
        if let Some(mos) = device.as_any_mut().downcast_mut::<Bsim3>() {
            mos.delta = delta;
            mos.delta_old1 = delta;
        }
        if let Some(bjt) = device.as_any_mut().downcast_mut::<Bjt>() {
            bjt.delta = delta;
            bjt.delta_old1 = delta;
        }
        if let Some(jfet) = device.as_any_mut().downcast_mut::<Jfet>() {
            jfet.delta = delta;
            jfet.delta_old1 = delta;
        }
    }
}

/// Sign function matching ngspice SGN macro.
fn sgn(x: f64) -> f64 {
    if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
}

/// Find a source device by name (case-insensitive), returning its index.
fn find_source(circuit: &Circuit, name: &str) -> Option<usize> {
    let name_upper = name.to_uppercase();
    circuit.devices.iter().position(|d| {
        d.name().to_uppercase() == name_upper
    })
}

/// Get the DC value of a source device.
fn get_source_dc(circuit: &Circuit, idx: usize) -> f64 {
    let dev = &circuit.devices[idx];
    if let Some(v) = dev.as_any().downcast_ref::<VoltageSource>() {
        v.dc_value()
    } else if let Some(i) = dev.as_any().downcast_ref::<CurrentSource>() {
        i.dc_value()
    } else {
        0.0
    }
}

/// Set the DC value of a source device.
fn set_source_dc(circuit: &mut Circuit, idx: usize, value: f64) {
    let dev = &mut circuit.devices[idx];
    if let Some(v) = dev.as_any_mut().downcast_mut::<VoltageSource>() {
        v.set_dc_value(value);
    } else if let Some(i) = dev.as_any_mut().downcast_mut::<CurrentSource>() {
        i.set_dc_value(value);
    }
}
