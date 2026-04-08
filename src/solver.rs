use crate::circuit::Circuit;
use crate::config::SimConfig;
use crate::error::SimError;
use crate::mna::MnaSystem;
use crate::mode::*;
use crate::node::NodeType;

/// Mutable simulation state — everything that changes during NR iteration.
/// Matches the mutable portion of ngspice's CKTcircuit.
pub struct SimState {
    pub mna: MnaSystem,
    pub mode: Mode,
    pub ni_state: NiState,
    pub diag_gmin: f64,
    /// Per-device junction gmin (CKTgmin) — stepped during "true gmin" stepping.
    /// Different from diag_gmin which is added to matrix diagonals only.
    pub gmin: f64,
    pub src_fact: f64,
    pub noncon: i32,
    pub iter_count: usize,
    /// Per-NR-iteration snapshots for profiling (drained after each ni_iter call).
    pub nr_snapshots: Vec<crate::analysis::transient::NrSnapshot>,
    pub profiling: bool,
    /// True if .NODESET was applied — triggers ipass mechanism (niiter.c:402-407)
    pub had_nodeset: bool,
}

impl SimState {
    pub fn new(size: usize, config: &SimConfig) -> Self {
        Self {
            mna: MnaSystem::new(size),
            mode: Mode::new(0),
            ni_state: NiState::new(),
            diag_gmin: config.gshunt,
            gmin: config.gmin,
            nr_snapshots: Vec::new(),
            profiling: config.trace.profile,
            src_fact: 1.0,
            noncon: 0,
            iter_count: 0,
            had_nodeset: false,
        }
    }

    /// Zero all node voltages in rhs_old (initial guess = 0).
    pub fn zero_solution(&mut self) {
        for v in &mut self.mna.rhs_old {
            *v = 0.0;
        }
    }

    /// Apply .NODESET values to rhs and rhs_old, and allocate diagonal
    /// matrix elements. Port of CKTic (cktic.c:26-47).
    ///
    /// Called once before the NR loop starts.
    pub fn apply_nodesets(&mut self, nodes: &[crate::node::Node]) {
        // CKTic: zero entire rhs first (cktic.c:22-24)
        // (Our rhs is already zeroed from MnaSystem::new, but be explicit)
        for v in &mut self.mna.rhs { *v = 0.0; }

        for (i, node) in nodes.iter().enumerate() {
            if let Some(val) = node.nodeset {
                if i < self.mna.rhs.len() && i > 0 {
                    // CKTic: allocate diagonal element (cktic.c:39)
                    self.mna.ensure_diag(i);
                    self.had_nodeset = true;
                    // CKTic: set rhs and rhs_old to nodeset value (cktic.c:47)
                    self.mna.rhs_old[i] = val;
                    self.mna.rhs[i] = val;
                }
            }
        }
    }
}

/// Newton-Raphson iteration loop — faithful port of ngspice NIiter (niiter.c:42-440).
///
/// Returns Ok(iterations) on convergence, Err on failure.
pub fn ni_iter(
    sim: &mut SimState,
    circuit: &mut Circuit,
    config: &SimConfig,
    max_iter: usize,
) -> Result<usize, SimError> {
    let max_iter = max_iter.max(100); // niiter.c:53-54
    let mut ipass = 0_i32; // niiter.c:28 — extra iteration after FIX→FLT with .NODESET

    let trace_stamp = config.trace.stamp && sim.mna.size() <= 9;
    let trace_nr_dump = config.trace.nr_dump;
    let trace_nr = config.trace.nr && sim.mna.size() <= 5;

    sim.iter_count = 0;


    loop {
        // 1. CKTload: clear matrix+RHS, load all devices (niiter.c:93)
        sim.mna.clear();
        sim.noncon = 0;

        // Pre-load pass: inductors compute flux (indload.c first pass).
        // Must run before main load so mutual inductors can add flux contributions.
        for device in &mut circuit.devices {
            device.pre_load(&mut sim.mna, &mut circuit.states, sim.mode);
        }

        for device in &mut circuit.devices {
            let mut dev_noncon = false;
            device.load(
                &mut sim.mna,
                &mut circuit.states,
                sim.mode,
                sim.src_fact,
                sim.gmin,
                &mut dev_noncon,
            )?;
            if dev_noncon {
                sim.noncon = 1;
            }
        }

        // .NODESET / .IC injection — port of cktload.c:120-173.
        // After device loads, during MODEDC && (MODEINITJCT | MODEINITFIX),
        // override the matrix row for each NODESET node to force V(node) = nodeset.
        if sim.mode.is(MODEDC) && (sim.mode.is(MODEINITJCT) || sim.mode.is(MODEINITFIX)) {
            for (eq, node) in circuit.nodes.iter().enumerate() {
                if let Some(ns_val) = node.nodeset {
                    if eq == 0 { continue; }
                    // ZeroNoncurRow: zero all voltage-type elements in this row
                    let has_currents = sim.mna.zero_noncur_row(eq, &circuit.nodes);
                    if has_currents {
                        // Current columns exist: use 1e10 scaling (cktload.c:128-130)
                        sim.mna.rhs[eq] = 1.0e10 * ns_val * sim.src_fact;
                        let diag = sim.mna.find_elt(eq, eq);
                        sim.mna.set_elt(diag, 1.0e10);
                    } else {
                        // No current columns: simple identity (cktload.c:132-134)
                        sim.mna.rhs[eq] = ns_val * sim.src_fact;
                        let diag = sim.mna.find_elt(eq, eq);
                        sim.mna.set_elt(diag, 1.0);
                    }
                }
            }
        }

        // Capture per-device conductances, stored currents, and limited voltages for profiling
        let (dev_conds, dev_currents, dev_volts) = if sim.profiling {
            let conds = circuit.devices.iter().filter_map(|d| {
                let c = d.conductances();
                if c.is_empty() { None }
                else { Some((d.name().to_string(), c.iter().map(|(_, v)| *v).collect())) }
            }).collect();
            let currents = circuit.devices.iter().filter_map(|d| {
                let c = d.stored_currents();
                if c.is_empty() { None }
                else { Some((d.name().to_string(), c.iter().map(|(_, v)| *v).collect())) }
            }).collect();
            let volts = circuit.devices.iter().filter_map(|d| {
                let v = d.limited_voltages();
                if v.is_empty() { None }
                else { Some((d.name().to_string(), v.iter().map(|(_, val)| *val).collect())) }
            }).collect();
            (conds, currents, volts)
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };

        if trace_stamp && sim.iter_count < 2 {
            let n = sim.mna.size();
            eprint!("SR_DIAG_PRE iter={}", sim.iter_count + 1);
            for i in 1..=n { eprint!(" d{}={:.15e}", i, sim.mna.diag_val(i)); }
            eprintln!();
        }

        // Add diagonal gmin (spsmp.c LoadGmin)
        sim.mna.add_diag_gmin(sim.diag_gmin);

        sim.iter_count += 1;

        // NISHOULDREORDER (niiter.c:116-133):
        // Set at MODEINITJCT (→MODEINITFIX transition) and MODEINITTRAN first iter.
        // Triggers full Markowitz reordering instead of refactoring.
        if sim.mode.is(MODEINITJCT)
            || (sim.mode.is(crate::mode::MODEINITTRAN) && sim.iter_count == 1)
        {
            sim.ni_state.set(NI_SHOULD_REORDER);
        }
        if sim.ni_state.is(NI_SHOULD_REORDER) {
            sim.mna.force_reorder();
            sim.ni_state.clear(NI_SHOULD_REORDER);
        }

        // Capture RHS and matrix before solve (for profiling)
        let rhs_pre_snapshot = if sim.profiling {
            let n = sim.mna.size();
            sim.mna.rhs[1..=n].to_vec()
        } else {
            Vec::new()
        };
        let matrix_snapshot = if sim.profiling && sim.iter_count == 1 {
            sim.mna.dump_matrix_elements()
        } else {
            Vec::new()
        };

        if trace_nr_dump {
            let n = sim.mna.size();
            eprint!("SR_RHS_PRE iter={}", sim.iter_count);
            for i in 1..=n { eprint!(" r{}={:.15e}", i, sim.mna.rhs[i]); }
            eprintln!();
        }

        // 2-4. Factor and solve (niiter.c:158-301)
        sim.mna.solve()?;

        // Zero ground (niiter.c:316-318)
        sim.mna.zero_ground();

        // Collect NR snapshot for profiling
        if sim.profiling {
            let n = sim.mna.size();
            sim.nr_snapshots.push(crate::analysis::transient::NrSnapshot {
                step: 0,
                iter: sim.iter_count,
                time: 0.0,
                mode: sim.mode.bits(),
                rhs_pre: rhs_pre_snapshot.clone(),
                values: sim.mna.rhs[1..=n].to_vec(),
                matrix_elements: matrix_snapshot.clone(),
                device_conds: dev_conds.clone(),
                device_currents: dev_currents.clone(),
                device_volts: dev_volts.clone(),
                noncon: sim.noncon,
            });
        }

        if trace_nr_dump {
            let n = sim.mna.size();
            eprint!("SR_NR iter={} mode={:#06x} noncon={}", sim.iter_count, sim.mode.bits(), sim.noncon);
            for i in 1..=n { eprint!(" v{}={:.15e}", i, sim.mna.rhs_val(i)); }
            eprintln!();
        }

        if trace_nr {
            let n = sim.mna.size();
            eprint!("SR_NR iter={} mode={:#06x}", sim.iter_count, sim.mode.bits());
            for i in 1..=n { eprint!(" v{}={:.15e}", i, sim.mna.rhs_val(i)); }
            eprintln!();
        }


        // 5. Check iteration limit (niiter.c:320-333)
        if sim.iter_count > max_iter {
            return Err(SimError::IterationLimit(sim.iter_count));
        }

        // 6. Convergence test (niiter.c:335-338)
        // Only test if noncon==0 AND not first iteration
        if sim.noncon == 0 && sim.iter_count > 1 {
            sim.noncon = ni_conv_test(sim, circuit, config);
        } else {
            sim.noncon = 1;
        }


        // 7. Mode transitions (niiter.c:397-432)
        if sim.mode.is(MODEINITFLOAT) {
            // .NODESET ipass mechanism (niiter.c:402-407)
            if sim.mode.is(MODEDC) && sim.had_nodeset {
                if ipass != 0 {
                    sim.noncon = ipass;
                }
                ipass = 0;
            }
            if sim.noncon == 0 {
                return Ok(sim.iter_count); // CONVERGED
            }
        } else if sim.mode.is(MODEINITJCT) {
            sim.mode.set_init(MODEINITFIX);
            sim.ni_state.set(NI_SHOULD_REORDER);
        } else if sim.mode.is(MODEINITFIX) {
            if sim.noncon == 0 {
                sim.mode.set_init(MODEINITFLOAT);
            }
            ipass = 1; // niiter.c:419
        } else if sim.mode.is(MODEINITTRAN) || sim.mode.is(MODEINITPRED) {
            sim.mode.set_init(MODEINITFLOAT);
        }

        // 8. Swap rhs/rhs_old (niiter.c:435)
        sim.mna.swap_rhs();
    }
}

/// Convergence test — faithful port of ngspice NIconvTest (niconv.c:20-90).
///
/// Compares current solution (rhs) against previous (rhs_old).
/// Returns 0 if converged, 1 if not.
///
/// NOTE on NEWCONV: ngspice defines NEWCONV (macros.h:19), so NIconvTest (niconv.c:82-86)
/// calls CKTconvTest (cktop.c:126-146) after the base node test passes. CKTconvTest
/// iterates device types and calls DEVconvTest (e.g., MOS1convTest). Device convTests
/// increment CKTnoncon when they detect non-convergence, but always return OK (0).
/// CKTconvTest checks CKTnoncon after each device type and early-exits if nonzero,
/// but still returns OK (0). NIconvTest then returns this OK (0) to niiter.c:343,
/// which does `CKTnoncon = NIconvTest(ckt)`, overwriting any device noncon increments
/// with 0. Thus the NEWCONV device convTests are effectively dead code — they run but
/// their noncon increments are immediately erased. We replicate this by calling
/// device conv_tests but ignoring their return values. Verified 2026-04-04: our
/// last_cd/last_cbs/last_cbd match ngspice's MOS1cd/MOS1cbs/MOS1cbd bit-identically.
fn ni_conv_test(
    sim: &SimState,
    circuit: &Circuit,
    config: &SimConfig,
) -> i32 {
    let size = sim.mna.size();

    for i in 1..=size {
        let new_val = sim.mna.rhs_val(i);
        let old_val = sim.mna.rhs_old_val(i);

        // NaN check (niconv.c:53)
        if new_val.is_nan() {
            return 1;
        }

        // Tolerance depends on node type (niconv.c:55-77)
        let tol = if i < circuit.nodes.len() && circuit.nodes[i].node_type == NodeType::Voltage {
            config.reltol * f64::max(old_val.abs(), new_val.abs()) + config.volt_tol
        } else {
            config.reltol * f64::max(old_val.abs(), new_val.abs()) + config.abs_tol
        };

        if (new_val - old_val).abs() > tol {
            return 1;
        }
    }

    // NEWCONV path (niconv.c:82-86, cktop.c:126-146):
    // ngspice calls CKTconvTest which calls DEVconvTest for each device. However,
    // CKTconvTest always returns OK (0), so the device noncon increments are
    // overwritten by niiter.c's assignment. We call the tests for completeness
    // (they may set device-internal state) but ignore their return values.
    for device in &circuit.devices {
        let _ = device.conv_test(&sim.mna, &circuit.states, config.reltol, config.abs_tol);
    }

    0 // converged
}
