use crate::analysis::dc::{dc_operating_point_tran, dc_operating_point_uic};
use crate::breakpoint::Breakpoints;
use crate::circuit::Circuit;
use crate::config::SimConfig;
use crate::error::SimError;
use crate::integration::{ckt_terr, ni_com_cof};
use crate::mode::*;
use crate::solver::ni_iter;

/// Transient analysis result — time-series of node voltages.
#[derive(Debug)]
pub struct TransientResult {
    /// Time points (accepted steps).
    pub times: Vec<f64>,
    /// Node voltages at each time point. `values[t][eq]` = voltage at equation `eq`, time index `t`.
    pub values: Vec<Vec<f64>>,
    /// Per-step diagnostics: (delta, order, nr_iters)
    pub step_info: Vec<StepInfo>,
    /// Number of accepted steps.
    pub accepted: usize,
    /// Number of rejected steps.
    pub rejected: usize,
    /// Per-NR-iteration snapshots (only when SPICERS_PROFILE is set).
    pub nr_snapshots: Vec<NrSnapshot>,
}

/// Per-NR-iteration snapshot for divergence profiling.
#[derive(Debug, Clone)]
pub struct NrSnapshot {
    pub step: usize,
    pub iter: usize,
    pub time: f64,
    pub mode: u32,
    pub rhs_pre: Vec<f64>,  // rhs values BEFORE solve (device stamp output)
    pub values: Vec<f64>,   // rhs values AFTER solve (solver output)
    /// Pre-factor matrix elements as (row, col, value) triples in internal coords.
    /// Only populated for the first NR iteration when profiling is enabled.
    pub matrix_elements: Vec<f64>,
    /// Per-device conductances: Vec of (device_name, Vec of values).
    pub device_conds: Vec<(String, Vec<f64>)>,
    /// Per-device stored currents (cd, cbs, cbd for MOSFETs): Vec of (device_name, Vec of values).
    pub device_currents: Vec<(String, Vec<f64>)>,
    /// Per-device limited voltages: Vec of (device_name, Vec of values).
    pub device_volts: Vec<(String, Vec<f64>)>,
    /// noncon flag after device load.
    pub noncon: i32,
}

/// Per-step diagnostic data.
#[derive(Debug, Clone)]
pub struct StepInfo {
    pub delta: f64,
    pub order: usize,
    pub order_used: usize,  // order at start of NR (after breakpoint handling)
    pub nr_iters: usize,
}

/// Transient analysis — port of ngspice DCtran (dctran.c:139-1100).
///
/// Runs a time-domain simulation from t=0 to `final_time` with output every `step`.
pub fn transient(
    circuit: &mut Circuit,
    config: &SimConfig,
    step: f64,
    final_time: f64,
    max_step: Option<f64>,
    tran_max_iter: usize,
    uic: bool,
    ic_values: &[(String, f64)],
) -> Result<TransientResult, SimError> {
    // ngspice traninit.c:29-34: default maxstep logic
    // If tmax not specified by user (max_step == None):
    //   if step < (finalTime - initTime)/50, maxStep = step
    //   else maxStep = (finalTime - initTime)/50
    // We don't support tstart (initTime=0), so this is min(step, finalTime/50).
    let max_step = max_step.unwrap_or_else(|| {
        let tstop_50 = final_time / 50.0;
        if step < tstop_50 {
            step
        } else {
            tstop_50
        }
    });
    let max_order: usize = 2; // trapezoidal
    let xmu = 0.5; // standard trapezoidal
    let trtol = 7.0; // ngspice default CKTtrtol
    // ngspice traninit.c:36: CKTdelmin = 1e-11 * CKTmaxStep (timestep floor)
    let delmin = 1e-11 * max_step;
    // ngspice dctran.c:191: CKTminBreak = CKTmaxStep * 5e-5 (breakpoint tolerance)
    let min_break = max_step * 5e-5;

    // 1. DC operating point (dctran.c:212-269)
    // With UIC: ngspice runs CKTop with MODEUIC|MODETRANOP|MODEINITJCT.
    // Inductors act as short circuits (MODEDC), capacitors use .IC voltages.
    // This finds a consistent DC solution with energy storage at ICs.
    let mut sim = if uic {
        dc_operating_point_uic(circuit, config, ic_values)?
    } else {
        dc_operating_point_tran(circuit, config)?
    };

    let num_eq = circuit.num_equations();

    // After DC OP return from NIiter:
    //   rhs = last NR iteration solve result
    //   rhs_old = second-to-last iteration (from SWAP inside NIiter)
    // ngspice does NOT swap here — dctran.c goes directly to the transient loop.
    // Devices read from rhs_old (the second-to-last iteration value).
    // Do NOT swap — matching ngspice's behavior.

    let trace_step = config.trace.step > 0;
    let trace_step_limit = config.trace.step;
    let trace_bp = config.trace.bp;
    let profiling = config.trace.profile;

    if trace_step {
        eprint!("SR_DCOP nr={}", sim.iter_count);
        for i in 1..num_eq { eprint!(" v{}={:.15e}", i, sim.mna.rhs_old_val(i)); }
        eprintln!();
        eprint!("SR_DCOP_RHS");
        for i in 1..num_eq { eprint!(" v{}={:.15e}", i, sim.mna.rhs_val(i)); }
        eprintln!();
    }
    sim.profiling = profiling;

    // Collect initial DC OP values.
    // With UIC: ngspice skips the t=0 dump (dctran.c:534 has `time > 0` guard).
    // Without UIC: t=0 has the DC OP solution.
    let mut result = if uic {
        TransientResult {
            times: Vec::new(),
            values: Vec::new(),
            step_info: Vec::new(),
            accepted: 0,
            rejected: 0,
            nr_snapshots: Vec::new(),
        }
    } else {
        TransientResult {
            times: vec![0.0],
            values: vec![sim.mna.rhs_old[..num_eq].to_vec()],
            step_info: vec![StepInfo { delta: 0.0, order: 0, order_used: 0, nr_iters: 0 }],
            accepted: 0,
            rejected: 0,
            nr_snapshots: Vec::new(),
        }
    };

    // dctran.c:374,884,911,929: MODEUIC is preserved in mode throughout transient
    let uic_flag = if uic { MODEUIC } else { 0 };

    // 2. Initialize transient (dctran.c:139-353)
    let mut delta = f64::min(final_time / 100.0, step) / 10.0;
    let mut delta_old = [max_step; 7]; // dctran.c:318-320
    let mut order: usize = 1; // dctran.c:317
    let mut ag = [0.0f64; 7]; // dctran.c:349
    let mut time = 0.0;
    let mut save_delta = final_time / 50.0;
    let mut first_time = true;

    // Copy state0 → state1 (dctran.c:350-352)
    let nstates = circuit.states.len();
    if nstates > 0 {
        let s0: Vec<f64> = circuit.states.state0()[..nstates].to_vec();
        for i in 0..nstates {
            circuit.states.set(1, i, s0[i]);
        }
    }

    sim.mode = Mode::new(uic_flag | MODETRAN | MODEINITTRAN);

    // Breakpoint list (dctran.c:162-167)
    let mut breakpoints = Breakpoints::new(final_time, max_step);

    // UIC: set breakpoint at first output step to reduce ringing (dctran.c:633-634)
    if uic {
        breakpoints.set(step, 0.0);
    }

    // Set integration params on devices
    set_device_integration(circuit, &ag, order, time, step, final_time, delta, &delta_old);

    // 3. Main transient loop
    loop {
        // === nextTime: accept logic (dctran.c:389-544) ===
        result.accepted += 1;

        // Clear past breakpoints (dctran.c:422)
        if time > breakpoints.next() {
            breakpoints.clear_past(time, final_time);
        }

        // Register source breakpoints (vsrcacct.c — PULSE/PWL edge registration)
        register_source_breakpoints(circuit, time, step, final_time, min_break, &mut breakpoints);
        if trace_bp && result.accepted <= 15 {
            eprintln!("BP_STATE s={} t={:.6e} next_bp={:.6e} bps={:?}", result.accepted, time, breakpoints.next(), &breakpoints);
        }

        // Check termination (dctran.c:521)
        if time >= final_time - delmin * 0.1 {
            break;
        }

        // --- resume: timestep control (dctran.c:545-680) ---

        // Cap at max_step (dctran.c:564-565)
        delta = f64::min(delta, max_step);

        // Breakpoint-aware timestep control (dctran.c:575-624)
        // ngspice dctran.c:584: AlmostEqualUlps(time, bp, 100) || bp - time <= CKTdelmin
        let at_breakpoint = almost_equal_ulps(time, breakpoints.next(), 100)
            || breakpoints.next() - time <= delmin;

        if at_breakpoint {
            // At breakpoint: cut order and limit delta (dctran.c:581-613)
            order = 1;
            delta = f64::min(
                delta,
                0.1 * f64::min(save_delta, breakpoints.following() - breakpoints.next()),
            );

            if first_time {
                delta /= 10.0; // dctran.c:602
            }

            delta = f64::max(delta, delmin * 2.0); // dctran.c:610
        } else if time + delta >= breakpoints.next() {
            // About to overshoot breakpoint: cut to hit it exactly (dctran.c:616-624)
            if trace_bp {
                eprintln!("BP_CLIP t={:.6e} d={:.6e} → {:.6e} (bp={:.6e})",
                    time, delta, breakpoints.next() - time, breakpoints.next());
            }
            save_delta = delta;
            delta = breakpoints.next() - time;
        }

        // Don't overshoot final time
        if time + delta > final_time {
            delta = final_time - time;
        }

        // Rotate delta history (dctran.c:742-744)
        for i in (1..7).rev() {
            delta_old[i] = delta_old[i - 1];
        }
        delta_old[0] = delta;

        // Rotate state vectors (dctran.c:746-750)
        circuit.states.rotate(max_order);

        // === resume: try logic (dctran.c:753-1100) ===
        loop {
            let old_delta = delta;
            let order_used = order;  // capture order before promotion changes it

            // Advance time (dctran.c:770)
            time += delta;
            delta_old[0] = delta; // dctran.c:775

            // Compute integration coefficients (dctran.c:776)
            ag = ni_com_cof(delta, &delta_old, order, xmu);

            // Set integration params on devices
            set_device_integration(circuit, &ag, order, time, step, final_time, delta, &delta_old);

            // Set mode for NR
            sim.mode = Mode::new(uic_flag | MODETRAN | if first_time { MODEINITTRAN } else { MODEINITPRED });

            // NR solve (dctran.c:835)
            sim.iter_count = 0;
            sim.nr_snapshots.clear();
            match ni_iter(&mut sim, circuit, config, tran_max_iter) {
                Ok(_) => {
                    // Converged — but don't collect snapshots yet.
                    // Wait until LTE acceptance to avoid duplicates on LTE rejection.
                    if first_time {
                        // First timepoint: skip LTE, accept (dctran.c:869-914)
                        first_time = false;

                        // Copy state1 → state2, state3 (dctran.c:870-875)
                        // Initializes higher-order history for LTE/integration.
                        // Without this, state2/state3 hold DC OP values and
                        // corrupt second-order divided differences from step 2.
                        circuit.states.copy_level(1, 2);
                        if max_order >= 2 {
                            circuit.states.copy_level(1, 3);
                        }

                        // Record output
                        result.times.push(time);
                        // CKTdump outputs CKTrhsOld (cktdump.c:43)
                        result.values.push(sim.mna.rhs_old[..num_eq].to_vec());
                        result.step_info.push(StepInfo { delta: old_delta, order, order_used, nr_iters: sim.iter_count });

                        if trace_step {
                            eprint!("SR_ACCEPT s={} t={:.15e} d={:.15e} ord={} nr={}",
                                result.accepted, time, old_delta, order, sim.iter_count);
                            for i in 1..num_eq { eprint!(" v{}={:.15e}", i, sim.mna.rhs_old[i]); }
                            eprintln!();
                        }

                        // Collect NR snapshots only after acceptance
                        if profiling {
                            for snap in &mut sim.nr_snapshots {
                                snap.step = result.accepted + 1;
                                snap.time = time;
                            }
                            result.nr_snapshots.extend(sim.nr_snapshots.drain(..));
                        }

                        break; // → nextTime
                    }

                    // LTE check — port of CKTtrunc (ckttrunc.c:20-56)
                    // ngspice CKTtrunc: timetemp = HUGE; DEVtrunc shrinks it;
                    //   *timeStep = MIN(2 * *timeStep, timetemp)
                    // The MIN(2*delta, ...) is done INSIDE CKTtrunc on line 53.
                    let mut new_delta = ckt_trunc(circuit, config, order, delta, &delta_old, trtol);

                    if config.trace.lte && result.accepted <= 10 {
                        eprintln!("SR_LTE s={} t={:.15e} delta={:.15e} ord={} new_delta={:.15e} ratio={:.15e}",
                            result.accepted, time, delta, order, new_delta, new_delta / delta);
                    }
                    if new_delta > 0.9 * delta {
                        // ACCEPT (dctran.c:922)

                        // Order promotion (dctran.c:990-1003)
                        if order == 1 && max_order > 1 {
                            // newdelta = CKTdelta, CKTorder = 2, CKTtrunc(&newdelta)
                            // if newdelta <= 1.05 * CKTdelta → revert to order 1
                            // CKTdelta = newdelta
                            let newdelta2 = ckt_trunc(circuit, config, 2, delta, &delta_old, trtol);
                            if config.trace.lte && result.accepted <= 10 {
                                eprintln!("SR_ORDER_PROMO s={} delta={:.15e} newdelta2={:.15e} test={:.15e}",
                                    result.accepted, delta, newdelta2, 1.05 * delta);
                            }
                            order = 2;
                            set_device_order(circuit, 2);
                            if newdelta2 <= 1.05 * delta {
                                order = 1;
                            }
                            new_delta = newdelta2;
                            set_device_order(circuit, order);
                        }

                        delta = new_delta;

                        // Record output
                        result.times.push(time);
                        // CKTdump outputs CKTrhsOld (cktdump.c:43)
                        result.values.push(sim.mna.rhs_old[..num_eq].to_vec());
                        result.step_info.push(StepInfo { delta: old_delta, order, order_used, nr_iters: sim.iter_count });

                        if trace_step && result.accepted <= trace_step_limit {
                            eprint!("SR_ACCEPT s={} t={:.15e} d={:.15e} ord={} nr={}",
                                result.accepted, time, old_delta, order, sim.iter_count);
                            for i in 1..num_eq { eprint!(" v{}={:.15e}", i, sim.mna.rhs_old[i]); }
                            eprintln!();
                        }

                        // Collect NR snapshots only after acceptance
                        if profiling {
                            for snap in &mut sim.nr_snapshots {
                                snap.step = result.accepted + 1;
                                snap.time = time;
                            }
                            result.nr_snapshots.extend(sim.nr_snapshots.drain(..));
                        }

                        break; // → nextTime
                    } else {
                        // REJECT (dctran.c:999-1013)
                        time -= delta;
                        result.rejected += 1;
                        delta = new_delta;
                    }
                }
                Err(_) => {
                    // Non-convergence: reject (dctran.c:850-867)
                    time -= delta;
                    result.rejected += 1;
                    delta /= 8.0;
                    order = 1;

                    if first_time {
                        sim.mode = Mode::new(uic_flag | MODETRAN | MODEINITTRAN);
                    }
                }
            }

            // Timestep floor (dctran.c:1018-1032)
            if delta <= delmin {
                if old_delta > delmin {
                    delta = delmin;
                } else {
                    return Err(SimError::TimestepTooSmall(delta, delmin));
                }
            }
        }
    }

    Ok(result)
}

/// Port of ngspice AlmostEqualUlps (cktdefs.h / devsup.c).
/// Returns true if a and b are within max_ulps of each other in IEEE 754 representation.
fn almost_equal_ulps(a: f64, b: f64, max_ulps: i64) -> bool {
    let ai = a.to_bits() as i64;
    let bi = b.to_bits() as i64;
    (ai - bi).abs() <= max_ulps
}

/// Register source waveform breakpoints — port of vsrcacct.c PULSE case.
///
/// For each PULSE/PWL/SIN source, computes the next waveform edge time and
/// registers it as a breakpoint so the transient engine hits it exactly.
fn register_source_breakpoints(
    circuit: &mut Circuit,
    time: f64,
    step: f64,
    final_time: f64,
    min_break: f64,
    breakpoints: &mut Breakpoints,
) {
    for device in &mut circuit.devices {
        if let Some(vs) = downcast_mut::<crate::device::vsource::VoltageSource>(device.as_mut()) {
            // Guard: only register new breakpoint when past the previous one (vsrcacct.c:94)
            if time >= vs.break_time {
                if let Some(bp) = vs.next_breakpoint(time, step, final_time, min_break) {
                    vs.break_time = bp;           // vsrcacct.c:135
                    breakpoints.set(bp, time);
                    vs.break_time -= min_break;   // vsrcacct.c:145
                }
            }
        }
        if let Some(is) = downcast_mut::<crate::device::isource::CurrentSource>(device.as_mut()) {
            if let Some(bp) = is.next_breakpoint(time, step, final_time, min_break) {
                breakpoints.set(bp, time);
            }
        }
    }
}

/// Port of CKTtrunc (ckttrunc.c:20-56).
///
/// Walks all devices, collects per-device LTE timesteps (via ckt_terr/DEVtrunc),
/// and returns `MIN(2 * timeStep, timetemp)` matching ckttrunc.c:53.
/// The caller passes `timeStep = CKTdelta` (ngspice dctran.c:962, 991).
fn ckt_trunc(
    circuit: &Circuit,
    config: &SimConfig,
    order: usize,
    time_step: f64,
    delta_old: &[f64; 7],
    trtol: f64,
) -> f64 {
    let mut timetemp = f64::MAX;

    for device in &circuit.devices {
        if let Some(cap) = downcast_ref::<crate::device::capacitor::Capacitor>(device.as_ref()) {
            timetemp = f64::min(timetemp, ckt_terr(&circuit.states, cap.qcap(), order, time_step, delta_old, config, trtol));
        }
        if let Some(ind) = downcast_ref::<crate::device::inductor::Inductor>(device.as_ref()) {
            timetemp = f64::min(timetemp, ckt_terr(&circuit.states, ind.flux_offset(), order, time_step, delta_old, config, trtol));
        }
        if let Some(dio) = downcast_ref::<crate::device::diode::Diode>(device.as_ref()) {
            timetemp = f64::min(timetemp, ckt_terr(&circuit.states, dio.qcap(), order, time_step, delta_old, config, trtol));
        }
        if let Some(mos) = downcast_ref::<crate::device::mosfet1::Mosfet1>(device.as_ref()) {
            for qoff in mos.qcap_offsets() {
                timetemp = f64::min(timetemp, ckt_terr(&circuit.states, qoff, order, time_step, delta_old, config, trtol));
            }
        }
        if let Some(mos) = downcast_ref::<crate::device::mosfet2::Mosfet2>(device.as_ref()) {
            for qoff in mos.qcap_offsets() {
                timetemp = f64::min(timetemp, ckt_terr(&circuit.states, qoff, order, time_step, delta_old, config, trtol));
            }
        }
        if let Some(mos) = downcast_ref::<crate::device::mosfet3::Mosfet3>(device.as_ref()) {
            for qoff in mos.qcap_offsets() {
                timetemp = f64::min(timetemp, ckt_terr(&circuit.states, qoff, order, time_step, delta_old, config, trtol));
            }
        }
        if let Some(mos) = downcast_ref::<crate::device::bsim3::Bsim3>(device.as_ref()) {
            for qoff in mos.qcap_offsets() {
                timetemp = f64::min(timetemp, ckt_terr(&circuit.states, qoff, order, time_step, delta_old, config, trtol));
            }
        }
        if let Some(bjt) = downcast_ref::<crate::device::bjt::Bjt>(device.as_ref()) {
            for qoff in bjt.qcap_offsets() {
                timetemp = f64::min(timetemp, ckt_terr(&circuit.states, qoff, order, time_step, delta_old, config, trtol));
            }
        }
        if let Some(jfet) = downcast_ref::<crate::device::jfet::Jfet>(device.as_ref()) {
            for qoff in jfet.qcap_offsets() {
                timetemp = f64::min(timetemp, ckt_terr(&circuit.states, qoff, order, time_step, delta_old, config, trtol));
            }
        }
    }

    // *timeStep = MIN(2 * *timeStep, timetemp) — ckttrunc.c:53
    f64::min(2.0 * time_step, timetemp)
}

/// Helper: try to downcast a Device trait object.
fn downcast_ref<T: 'static>(device: &dyn crate::device::Device) -> Option<&T> {
    device.as_any().downcast_ref::<T>()
}

/// Set integration parameters (ag, order, time) on all devices that need them.
fn set_device_integration(
    circuit: &mut Circuit,
    ag: &[f64; 7],
    order: usize,
    time: f64,
    step: f64,
    final_time: f64,
    delta: f64,
    delta_old: &[f64; 7],
) {
    for device in &mut circuit.devices {
        if let Some(cap) = downcast_mut::<crate::device::capacitor::Capacitor>(device.as_mut()) {
            cap.ag = *ag;
            cap.order = order;
        }
        if let Some(ind) = downcast_mut::<crate::device::inductor::Inductor>(device.as_mut()) {
            ind.ag = *ag;
            ind.order = order;
        }
        if let Some(mut_ind) = downcast_mut::<crate::device::mutual_inductor::MutualInductor>(device.as_mut()) {
            mut_ind.ag = *ag;
        }
        if let Some(vs) = downcast_mut::<crate::device::vsource::VoltageSource>(device.as_mut()) {
            vs.time = time;
            vs.step = step;
            vs.final_time = final_time;
        }
        if let Some(is) = downcast_mut::<crate::device::isource::CurrentSource>(device.as_mut()) {
            is.time = time;
            is.step = step;
            is.final_time = final_time;
        }
        if let Some(dio) = downcast_mut::<crate::device::diode::Diode>(device.as_mut()) {
            dio.ag = *ag;
            dio.order = order;
            dio.delta = delta;
            dio.delta_old1 = delta_old[1];
        }
        if let Some(mos) = downcast_mut::<crate::device::mosfet1::Mosfet1>(device.as_mut()) {
            mos.ag = *ag;
            mos.order = order;
            mos.delta = delta;
            mos.delta_old1 = delta_old[1];
        }
        if let Some(mos) = downcast_mut::<crate::device::mosfet2::Mosfet2>(device.as_mut()) {
            mos.ag = *ag;
            mos.order = order;
            mos.delta = delta;
            mos.delta_old1 = delta_old[1];
        }
        if let Some(mos) = downcast_mut::<crate::device::mosfet3::Mosfet3>(device.as_mut()) {
            mos.ag = *ag;
            mos.order = order;
            mos.delta = delta;
            mos.delta_old1 = delta_old[1];
        }
        if let Some(mos) = downcast_mut::<crate::device::bsim3::Bsim3>(device.as_mut()) {
            mos.ag = *ag;
            mos.order = order;
            mos.delta = delta;
            mos.delta_old1 = delta_old[1];
        }
        if let Some(bjt) = downcast_mut::<crate::device::bjt::Bjt>(device.as_mut()) {
            bjt.ag = *ag;
            bjt.order = order;
            bjt.delta = delta;
            bjt.delta_old1 = delta_old[1];
        }
        if let Some(jfet) = downcast_mut::<crate::device::jfet::Jfet>(device.as_mut()) {
            jfet.ag = *ag;
            jfet.order = order;
            jfet.delta = delta;
            jfet.delta_old1 = delta_old[1];
        }
    }
}

fn set_device_order(circuit: &mut Circuit, order: usize) {
    for device in &mut circuit.devices {
        if let Some(cap) = downcast_mut::<crate::device::capacitor::Capacitor>(device.as_mut()) {
            cap.order = order;
        }
        if let Some(ind) = downcast_mut::<crate::device::inductor::Inductor>(device.as_mut()) {
            ind.order = order;
        }
        if let Some(dio) = downcast_mut::<crate::device::diode::Diode>(device.as_mut()) {
            dio.order = order;
        }
        if let Some(mos) = downcast_mut::<crate::device::mosfet1::Mosfet1>(device.as_mut()) {
            mos.order = order;
        }
        if let Some(mos) = downcast_mut::<crate::device::mosfet2::Mosfet2>(device.as_mut()) {
            mos.order = order;
        }
        if let Some(mos) = downcast_mut::<crate::device::mosfet3::Mosfet3>(device.as_mut()) {
            mos.order = order;
        }
        if let Some(mos) = downcast_mut::<crate::device::bsim3::Bsim3>(device.as_mut()) {
            mos.order = order;
        }
        if let Some(bjt) = downcast_mut::<crate::device::bjt::Bjt>(device.as_mut()) {
            bjt.order = order;
        }
        if let Some(jfet) = downcast_mut::<crate::device::jfet::Jfet>(device.as_mut()) {
            jfet.order = order;
        }
    }
}

fn downcast_mut<T: 'static>(device: &mut dyn crate::device::Device) -> Option<&mut T> {
    device.as_any_mut().downcast_mut::<T>()
}
