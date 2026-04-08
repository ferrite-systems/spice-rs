//! High-level simulation runner — takes a netlist string, returns results.
//!
//! This is the main entry point for the ngspice-eval adapter.

use std::collections::HashMap;

use crate::analysis::ac::ac_analysis;
use crate::analysis::dc::{dc_operating_point, dc_sweep};
use crate::analysis::sens::sens_analysis_to_map;
use crate::analysis::tf::tf_analysis_to_map;
use crate::analysis::transient::transient;
use crate::config::SimConfig;
use crate::parser::{parse_netlist, resolve_coupled_inductors, Analysis, ParseResult};

/// Apply parsed .OPTIONS to SimConfig.
fn apply_options(config: &mut SimConfig, parsed: &ParseResult) {
    if let Some(temp_c) = parsed.temp {
        config.temp = temp_c + 273.15;
    }
    if let Some(tnom_c) = parsed.tnom {
        config.tnom = tnom_c + 273.15;
    }
    if let Some(abstol) = parsed.abstol {
        config.abs_tol = abstol;
    }
    if let Some(vntol) = parsed.vntol {
        config.volt_tol = vntol;
    }
    if let Some(reltol) = parsed.reltol {
        config.reltol = reltol;
    }
}

/// Run a simulation from a netlist string.
///
/// Returns a HashMap of node name → value, matching the format expected by
/// ngspice-eval's comparison infrastructure.
///
/// For DC OP: returns final node voltages and branch currents.
/// For TRAN: returns the LAST timepoint's node voltages and branch currents.
pub fn run_netlist(netlist: &str) -> Result<(HashMap<String, f64>, Analysis), String> {
    let mut parsed = parse_netlist(netlist)?;
    let mut config = SimConfig::default();

    apply_options(&mut config, &parsed);

    parsed.circuit.setup();
    resolve_coupled_inductors(&mut parsed.circuit, &parsed.k_specs)
        .map_err(|e| format!("K element resolution failed: {e}"))?;
    parsed.circuit.temperature(&config);

    // Apply .NODESET values to circuit nodes
    for (name, val) in &parsed.nodeset_nodes {
        if let Some(eq) = parsed.circuit.find_node(name) {
            parsed.circuit.nodes[eq].nodeset = Some(*val);
        }
    }

    match &parsed.analysis {
        Analysis::Op => {
            let sim = dc_operating_point(&mut parsed.circuit, &config)
                .map_err(|e| format!("DC OP failed: {e}"))?;

            // Use rhs_old: matches ngspice CKTdump which outputs CKTrhsOld (cktdump.c:43).
            // After NR convergence, NIiter returns before the final SWAP, so
            // rhs_old holds the second-to-last iteration (same as CKTrhsOld).
            let values = extract_node_values(&parsed.circuit, &sim.mna.rhs_old);
            Ok((values, parsed.analysis))
        }
        Analysis::Tran { step, stop, uic } => {
            let step = *step;
            let stop = *stop;
            let uic = *uic;

            let result = transient(&mut parsed.circuit, &config, step, stop, None, 50, uic, &parsed.ic_nodes)
                .map_err(|e| format!("Transient failed: {e}"))?;

            // Return last timepoint values
            if let Some(last) = result.values.last() {
                let values = extract_node_values(&parsed.circuit, last);
                Ok((values, parsed.analysis))
            } else {
                Err("Transient produced no output".to_string())
            }
        }
        Analysis::DcSweep { src1, start1, stop1, step1, src2, start2, stop2, step2 } => {
            let result = dc_sweep(
                &mut parsed.circuit,
                &config,
                src1,
                *start1,
                *stop1,
                *step1,
                src2.as_deref(),
                *start2,
                *stop2,
                *step2,
            ).map_err(|e| format!("DC sweep failed: {e}"))?;

            // Return last sweep point values (for single-value comparison)
            if let Some(last) = result.values.last() {
                let values = extract_node_values(&parsed.circuit, last);
                Ok((values, parsed.analysis))
            } else {
                Err("DC sweep produced no output".to_string())
            }
        }
        Analysis::Tf { output, input_src } => {
            let values = tf_analysis_to_map(
                &mut parsed.circuit,
                &config,
                output,
                input_src,
            ).map_err(|e| format!("TF analysis failed: {e}"))?;
            Ok((values, parsed.analysis))
        }
        Analysis::Sens { output } => {
            let values = sens_analysis_to_map(
                &mut parsed.circuit,
                &config,
                output,
            ).map_err(|e| format!("SENS analysis failed: {e}"))?;
            Ok((values, parsed.analysis))
        }
        Analysis::Ac { sweep_type, num_points, fstart, fstop } => {
            // For simple single-value comparison, return last frequency point's mag/phase
            let result = ac_analysis(
                &mut parsed.circuit,
                &config,
                *sweep_type,
                *num_points,
                *fstart,
                *fstop,
            ).map_err(|e| format!("AC analysis failed: {e}"))?;

            // Return last frequency point values (complex magnitude for each node)
            let mut values = HashMap::new();
            if let Some(last_re) = result.values_re.last() {
                if let Some(last_im) = result.values_im.last() {
                    for (i, node) in parsed.circuit.nodes.iter().enumerate() {
                        if i == 0 { continue; }
                        if i >= last_re.len() { break; }
                        // For AC, ngspice outputs complex values
                        // The eval harness extracts magnitude
                        let re = last_re[i];
                        let im = last_im[i];
                        let name = &node.name;
                        if node.node_type == crate::node::NodeType::Voltage {
                            values.insert(format!("v({})", name.to_lowercase()), re);
                            values.insert(format!("v({})-imag", name.to_lowercase()), im);
                        } else {
                            values.insert(name.to_lowercase(), re);
                            values.insert(format!("{}-imag", name.to_lowercase()), im);
                        }
                    }
                }
            }
            Ok((values, parsed.analysis))
        }
        Analysis::Pz { in_pos, in_neg, out_pos, out_neg, input_type, pz_type } => {
            use crate::analysis::pz::pz_analysis;
            let result = pz_analysis(
                &mut parsed.circuit,
                &config,
                in_pos, in_neg, out_pos, out_neg,
                *input_type, *pz_type,
            ).map_err(|e| format!("PZ analysis failed: {e}"))?;

            // Return poles and zeros as named complex values
            let mut values = HashMap::new();
            for (i, (re, im)) in result.poles.iter().enumerate() {
                values.insert(format!("pole({})", i + 1), *re);
                values.insert(format!("pole({})-imag", i + 1), *im);
            }
            for (i, (re, im)) in result.zeros.iter().enumerate() {
                values.insert(format!("zero({})", i + 1), *re);
                values.insert(format!("zero({})-imag", i + 1), *im);
            }
            Ok((values, parsed.analysis))
        }
    }
}

/// Run a transient simulation and return the full waveform + node names.
pub fn run_netlist_tran_waveform(netlist: &str) -> Result<(Vec<String>, crate::analysis::transient::TransientResult), String> {
    run_netlist_tran_waveform_with(netlist, crate::config::TraceFlags::default())
}

pub fn run_netlist_tran_waveform_with(netlist: &str, trace: crate::config::TraceFlags) -> Result<(Vec<String>, crate::analysis::transient::TransientResult), String> {
    let mut parsed = parse_netlist(netlist)?;
    let mut config = SimConfig::default();
    config.trace = trace;
    apply_options(&mut config, &parsed);
    parsed.circuit.setup();
    resolve_coupled_inductors(&mut parsed.circuit, &parsed.k_specs)
        .map_err(|e| format!("K element resolution failed: {e}"))?;
    parsed.circuit.temperature(&config);

    // Apply .NODESET values to circuit nodes
    for (name, val) in &parsed.nodeset_nodes {
        if let Some(eq) = parsed.circuit.find_node(name) {
            parsed.circuit.nodes[eq].nodeset = Some(*val);
        }
    }

    match &parsed.analysis {
        Analysis::Tran { step, stop, uic } => {
            let result = transient(&mut parsed.circuit, &config, *step, *stop, None, 50, *uic, &parsed.ic_nodes)
                .map_err(|e| format!("Transient failed: {e}"))?;

            // Build node name list
            let names: Vec<String> = parsed.circuit.nodes.iter().enumerate().map(|(i, n)| {
                if i == 0 { "gnd".to_string() }
                else if n.node_type == crate::node::NodeType::Voltage { format!("v({})", n.name.to_lowercase()) }
                else { n.name.to_lowercase() }
            }).collect();

            Ok((names, result))
        }
        _ => Err("Not a .TRAN analysis".to_string()),
    }
}

/// Extract parsed model parameters from all devices in a netlist.
/// Returns Vec of (device_name, Vec<(param_name, value)>).
pub fn run_netlist_params(netlist: &str) -> Result<Vec<(String, Vec<(String, f64)>)>, String> {
    let parsed = parse_netlist(netlist)?;
    let mut result = Vec::new();
    for device in &parsed.circuit.devices {
        let params = device.model_params();
        if !params.is_empty() {
            result.push((
                device.name().to_string(),
                params.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
            ));
        }
    }
    Ok(result)
}

/// Run a DC OP and return per-device conductances after convergence.
pub fn run_netlist_conductances(netlist: &str) -> Result<Vec<(String, Vec<(String, f64)>)>, String> {
    let mut parsed = parse_netlist(netlist)?;
    let mut config = SimConfig::default();
    apply_options(&mut config, &parsed);
    parsed.circuit.setup();
    resolve_coupled_inductors(&mut parsed.circuit, &parsed.k_specs)
        .map_err(|e| format!("K element resolution failed: {e}"))?;
    parsed.circuit.temperature(&config);

    // Apply .NODESET values to circuit nodes
    for (name, val) in &parsed.nodeset_nodes {
        if let Some(eq) = parsed.circuit.find_node(name) {
            parsed.circuit.nodes[eq].nodeset = Some(*val);
        }
    }

    let _ = crate::analysis::dc::dc_operating_point(&mut parsed.circuit, &config)
        .map_err(|e| format!("DC OP failed: {e}"))?;

    let mut result = Vec::new();
    for dev in &parsed.circuit.devices {
        let conds = dev.conductances();
        if !conds.is_empty() {
            result.push((
                dev.name().to_string(),
                conds.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
            ));
        }
    }
    Ok(result)
}

/// Return the equation map (eq_number, name, type) after parsing.
pub fn run_netlist_eqmap(netlist: &str) -> Result<Vec<(usize, String, &'static str)>, String> {
    let parsed = parse_netlist(netlist)?;
    Ok(parsed.circuit.equation_map())
}

/// Return the TRANSLATE ext_to_int map after circuit setup.
pub fn run_netlist_translate(netlist: &str) -> Result<Vec<usize>, String> {
    let mut parsed = parse_netlist(netlist)?;
    let mut config = SimConfig::default();
    apply_options(&mut config, &parsed);
    parsed.circuit.setup();
    resolve_coupled_inductors(&mut parsed.circuit, &parsed.k_specs)
        .map_err(|e| format!("K element resolution failed: {e}"))?;
    parsed.circuit.temperature(&config);

    // Apply .NODESET values to circuit nodes
    for (name, val) in &parsed.nodeset_nodes {
        if let Some(eq) = parsed.circuit.find_node(name) {
            parsed.circuit.nodes[eq].nodeset = Some(*val);
        }
    }
    let size = parsed.circuit.num_equations() - 1;
    let mut sim = crate::solver::SimState::new(size, &config);
    for device in &mut parsed.circuit.devices {
        device.setup_matrix(&mut sim.mna);
    }
    Ok(sim.mna.ext_to_int_map().to_vec())
}

/// Run a DC OP and return the solver's pivot permutation.
/// The pivot permutation is fixed after the first factorization (order_and_factor)
/// and determines all subsequent solve accuracy. Returns (rows, cols) where
/// rows[i] and cols[i] are the external equation numbers for elimination step i.
pub fn run_netlist_pivot(netlist: &str) -> Result<(Vec<usize>, Vec<usize>), String> {
    let mut parsed = parse_netlist(netlist)?;
    let mut config = SimConfig::default();
    apply_options(&mut config, &parsed);
    parsed.circuit.setup();
    resolve_coupled_inductors(&mut parsed.circuit, &parsed.k_specs)
        .map_err(|e| format!("K element resolution failed: {e}"))?;
    parsed.circuit.temperature(&config);

    // Apply .NODESET values to circuit nodes
    for (name, val) in &parsed.nodeset_nodes {
        if let Some(eq) = parsed.circuit.find_node(name) {
            parsed.circuit.nodes[eq].nodeset = Some(*val);
        }
    }

    let sim = crate::analysis::dc::dc_operating_point(&mut parsed.circuit, &config)
        .map_err(|e| format!("DC OP failed: {e}"))?;

    Ok(sim.mna.pivot_permutation())
}

/// Run a DC OP simulation with profiling enabled to capture NR snapshots.
/// Uses the full dc_operating_point flow (direct NR → gmin → source stepping)
/// to match ngspice's CKTop convergence path.
pub fn run_netlist_dc_op_profiled(netlist: &str) -> Result<(HashMap<String, f64>, Vec<crate::analysis::transient::NrSnapshot>), String> {
    let mut parsed = parse_netlist(netlist)?;
    let mut config = SimConfig::default();
    apply_options(&mut config, &parsed);
    parsed.circuit.setup();
    resolve_coupled_inductors(&mut parsed.circuit, &parsed.k_specs)
        .map_err(|e| format!("K element resolution failed: {e}"))?;
    parsed.circuit.temperature(&config);

    // Apply .NODESET values to circuit nodes
    for (name, val) in &parsed.nodeset_nodes {
        if let Some(eq) = parsed.circuit.find_node(name) {
            parsed.circuit.nodes[eq].nodeset = Some(*val);
        }
    }

    config.trace.profile = true;

    let sim = crate::analysis::dc::dc_operating_point(&mut parsed.circuit, &config)
        .map_err(|e| format!("DC OP failed: {e}"))?;

    let values = extract_node_values(&parsed.circuit, &sim.mna.rhs_old);
    let snapshots = sim.nr_snapshots.clone();
    Ok((values, snapshots))
}

/// DC sweep result for the eval harness: sweep values + per-signal waveforms.
pub struct DcSweepWaveform {
    /// Sweep variable values (the x-axis)
    pub sweep_values: Vec<f64>,
    /// Signal name → vector of values at each sweep point
    pub signals: HashMap<String, Vec<f64>>,
    /// Signal names in order
    pub names: Vec<String>,
}

/// Run a DC sweep and return full waveform data.
pub fn run_netlist_dc_sweep(netlist: &str) -> Result<DcSweepWaveform, String> {
    let mut parsed = parse_netlist(netlist)?;
    let mut config = SimConfig::default();
    apply_options(&mut config, &parsed);
    parsed.circuit.setup();
    resolve_coupled_inductors(&mut parsed.circuit, &parsed.k_specs)
        .map_err(|e| format!("K element resolution failed: {e}"))?;
    parsed.circuit.temperature(&config);

    for (name, val) in &parsed.nodeset_nodes {
        if let Some(eq) = parsed.circuit.find_node(name) {
            parsed.circuit.nodes[eq].nodeset = Some(*val);
        }
    }

    match &parsed.analysis {
        Analysis::DcSweep { src1, start1, stop1, step1, src2, start2, stop2, step2 } => {
            let result = dc_sweep(
                &mut parsed.circuit,
                &config,
                src1,
                *start1,
                *stop1,
                *step1,
                src2.as_deref(),
                *start2,
                *stop2,
                *step2,
            ).map_err(|e| format!("DC sweep failed: {e}"))?;

            // Build signal names
            let names: Vec<String> = parsed.circuit.nodes.iter().enumerate().map(|(i, n)| {
                if i == 0 { "gnd".to_string() }
                else if n.node_type == crate::node::NodeType::Voltage {
                    format!("v({})", n.name.to_lowercase())
                } else {
                    n.name.to_lowercase()
                }
            }).collect();

            // Build per-signal waveforms
            let mut signals: HashMap<String, Vec<f64>> = HashMap::new();
            for name in &names {
                signals.insert(name.clone(), Vec::with_capacity(result.values.len()));
            }

            for soln in &result.values {
                for (i, name) in names.iter().enumerate() {
                    if i < soln.len() {
                        signals.get_mut(name).unwrap().push(soln[i]);
                    }
                }
            }

            Ok(DcSweepWaveform {
                sweep_values: result.sweep_values,
                signals,
                names,
            })
        }
        _ => Err("Not a .DC analysis".to_string()),
    }
}

/// AC waveform result for the eval harness.
pub struct AcWaveform {
    /// Frequency values (the x-axis)
    pub frequencies: Vec<f64>,
    /// Signal name → (real_values, imag_values) at each frequency point
    pub signals_re: HashMap<String, Vec<f64>>,
    pub signals_im: HashMap<String, Vec<f64>>,
    /// Signal names in order
    pub names: Vec<String>,
}

/// Run AC analysis and return full waveform data.
pub fn run_netlist_ac(netlist: &str) -> Result<AcWaveform, String> {
    let mut parsed = parse_netlist(netlist)?;
    let mut config = SimConfig::default();
    apply_options(&mut config, &parsed);
    parsed.circuit.setup();
    resolve_coupled_inductors(&mut parsed.circuit, &parsed.k_specs)
        .map_err(|e| format!("K element resolution failed: {e}"))?;
    parsed.circuit.temperature(&config);

    for (name, val) in &parsed.nodeset_nodes {
        if let Some(eq) = parsed.circuit.find_node(name) {
            parsed.circuit.nodes[eq].nodeset = Some(*val);
        }
    }

    match &parsed.analysis {
        Analysis::Ac { sweep_type, num_points, fstart, fstop } => {
            let result = ac_analysis(
                &mut parsed.circuit,
                &config,
                *sweep_type,
                *num_points,
                *fstart,
                *fstop,
            ).map_err(|e| format!("AC analysis failed: {e}"))?;

            // Build signal names
            let names: Vec<String> = parsed.circuit.nodes.iter().enumerate().map(|(i, n)| {
                if i == 0 { "gnd".to_string() }
                else if n.node_type == crate::node::NodeType::Voltage {
                    format!("v({})", n.name.to_lowercase())
                } else {
                    n.name.to_lowercase()
                }
            }).collect();

            // Build per-signal waveforms
            let mut signals_re: HashMap<String, Vec<f64>> = HashMap::new();
            let mut signals_im: HashMap<String, Vec<f64>> = HashMap::new();
            for name in &names {
                signals_re.insert(name.clone(), Vec::with_capacity(result.frequencies.len()));
                signals_im.insert(name.clone(), Vec::with_capacity(result.frequencies.len()));
            }

            for (fi, _freq) in result.frequencies.iter().enumerate() {
                let soln_re = &result.values_re[fi];
                let soln_im = &result.values_im[fi];
                for (i, name) in names.iter().enumerate() {
                    if i < soln_re.len() {
                        signals_re.get_mut(name).unwrap().push(soln_re[i]);
                        signals_im.get_mut(name).unwrap().push(soln_im[i]);
                    }
                }
            }

            Ok(AcWaveform {
                frequencies: result.frequencies,
                signals_re,
                signals_im,
                names,
            })
        }
        _ => Err("Not a .AC analysis".to_string()),
    }
}

/// Extract a HashMap<node_name, value> from a solution vector.
fn extract_node_values(
    circuit: &crate::circuit::Circuit,
    solution: &[f64],
) -> HashMap<String, f64> {
    let mut map = HashMap::new();

    for (i, node) in circuit.nodes.iter().enumerate() {
        if i == 0 {
            continue; // skip ground
        }
        if i >= solution.len() {
            break;
        }

        let value = solution[i];
        let name = &node.name;

        // Match ngspice naming conventions:
        // - Voltage nodes: v(name)
        // - Branch currents: name#branch → name#branch
        if node.node_type == crate::node::NodeType::Voltage {
            map.insert(format!("v({})", name.to_lowercase()), value);
        } else {
            // Branch current — ngspice uses "v1#branch" format
            map.insert(name.to_lowercase(), value);
        }
    }

    map
}
