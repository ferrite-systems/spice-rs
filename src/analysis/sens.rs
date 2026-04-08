//! DC Sensitivity analysis — port of ngspice cktsens.c (DC-only path).
//!
//! `.SENS V(out[,ref])` computes dV_out / d_param for every device parameter.
//!
//! Algorithm (adjoint method):
//!   1. Run DC operating point, get solution E and factored matrix Y
//!   2. For each device parameter p:
//!      a. delta_var = p * 1e-6 (or 1e-6 if p == 0)
//!      b. Load device into delta matrix at original param → get stamps
//!      c. Negate the matrix and RHS stamps
//!      d. Perturb p by delta_var, re-run temperature
//!      e. Load device again → delta_Y now has Y_new - Y_old, delta_I has I_new - I_old
//!      f. Compute RHS = delta_I - delta_Y * E
//!      g. Solve Y * delta_E = RHS using original factored matrix
//!      h. sensitivity = delta_E[output] / delta_var
//!      i. Restore parameter and temperature

use std::collections::HashMap;

use crate::circuit::Circuit;
use crate::config::SimConfig;
use crate::device::Device;
use crate::error::SimError;
use crate::parser::TfOutput;

const SENS_DELTA: f64 = 0.000001;
const SENS_ABS_DELTA: f64 = 0.000001;

/// A single sensitivity result.
pub struct SensResult {
    /// Parameter name (e.g., "q1:bf", "rs1:resistance")
    pub name: String,
    /// Sensitivity value: d(output) / d(param)
    pub value: f64,
}

/// Run DC sensitivity analysis.
///
/// Returns a list of (param_name, sensitivity_value) pairs in ngspice order.
pub fn sens_analysis(
    circuit: &mut Circuit,
    config: &SimConfig,
    output: &TfOutput,
) -> Result<Vec<SensResult>, SimError> {
    // Step 1: Run DC operating point
    let mut sim = crate::analysis::dc::dc_operating_point(circuit, config)?;

    let size = sim.mna.size();

    // Save the DC OP solution (E vector)
    let e_solution: Vec<f64> = sim.mna.rhs_old[0..=size].to_vec();

    // Resolve output node equations
    let (out_is_voltage, out_pos_eq, out_neg_eq, out_branch_eq) =
        resolve_sens_output(circuit, output)?;

    let mut results = Vec::new();

    // Step 2: Iterate over all devices and their perturbable parameters
    // We iterate in the order devices appear in the circuit (which matches
    // ngspice's DEVices[] order since our parser creates devices in that order).
    for dev_idx in 0..circuit.devices.len() {
        let params: Vec<(String, u32)> = circuit.devices[dev_idx].sensitivity_params();
        if params.is_empty() {
            continue;
        }

        let dev_name = circuit.devices[dev_idx].name().to_string();

        for (param_name, param_id) in &params {
            // Get current parameter value
            let original_value = match circuit.devices[dev_idx].get_param(*param_id) {
                Some(v) => v,
                None => continue,
            };

            // Compute perturbation (cktsens.c:570-573)
            let delta_var = if original_value != 0.0 {
                original_value * SENS_DELTA
            } else {
                SENS_ABS_DELTA
            };

            // --- Phase 1: Load device at original params into delta matrix ---
            // We use a dense matrix to accumulate stamps
            let mut delta_matrix = vec![vec![0.0f64; size + 1]; size + 1];
            let mut delta_rhs = vec![0.0f64; size + 1];

            // Load the device at original parameter value
            stamp_device_into_dense(
                circuit.devices[dev_idx].as_mut(),
                &mut circuit.states,
                sim.mode,
                sim.src_fact,
                sim.gmin,
                &mut delta_matrix,
                &mut delta_rhs,
                size,
            );

            // Negate (cktsens.c:586-591)
            for row in delta_matrix.iter_mut() {
                for val in row.iter_mut() {
                    *val *= -1.0;
                }
            }
            for val in delta_rhs.iter_mut() {
                *val *= -1.0;
            }

            // --- Phase 2: Perturb parameter and reload ---
            circuit.devices[dev_idx].set_param(*param_id, original_value + delta_var);

            // Re-run temperature on the device
            circuit.devices[dev_idx].temperature(config.temp, config.tnom);

            // Load again (adds to the negated values → delta = new - old)
            stamp_device_into_dense(
                circuit.devices[dev_idx].as_mut(),
                &mut circuit.states,
                sim.mode,
                sim.src_fact,
                sim.gmin,
                &mut delta_matrix,
                &mut delta_rhs,
                size,
            );

            // Restore parameter and temperature
            circuit.devices[dev_idx].set_param(*param_id, original_value);
            circuit.devices[dev_idx].temperature(config.temp, config.tnom);

            // --- Phase 3: Compute RHS = delta_I - delta_Y * E ---
            // Matrix-vector multiply: delta_Y * E
            let mut delta_y_e = vec![0.0f64; size + 1];
            for row in 1..=size {
                let mut sum = 0.0;
                for col in 1..=size {
                    sum += delta_matrix[row][col] * e_solution[col];
                }
                delta_y_e[row] = sum;
            }

            // RHS = delta_I - delta_Y * E
            let mut rhs = vec![0.0f64; size + 1];
            for j in 1..=size {
                rhs[j] = delta_rhs[j] - delta_y_e[j];
            }

            // --- Phase 4: Solve Y * delta_E = RHS ---
            // Copy RHS into mna.rhs and solve
            for j in 0..=size {
                sim.mna.rhs[j] = rhs[j];
            }
            sim.mna.solve_only()?;
            sim.mna.rhs[0] = 0.0;

            // --- Phase 5: Extract sensitivity ---
            let delta_e_output = if out_is_voltage {
                sim.mna.rhs[out_pos_eq] - sim.mna.rhs[out_neg_eq]
            } else {
                sim.mna.rhs[out_branch_eq]
            };

            let sensitivity = delta_e_output / delta_var;

            // Build the vector name matching ngspice format
            // Principal params (name starts with "!") use just the device name
            // Model params use "device:param" format
            // Non-principal instance params use "device_param" format
            let vec_name = if param_name.starts_with('!') {
                dev_name.to_lowercase()
            } else if param_name.starts_with('_') {
                // Instance param (non-principal): device_param
                format!("{}_{}", dev_name.to_lowercase(), &param_name[1..])
            } else {
                // Model param: device:param
                format!("{}:{}", dev_name.to_lowercase(), param_name)
            };
            results.push(SensResult {
                name: vec_name,
                value: sensitivity,
            });
        }
    }

    Ok(results)
}

/// Stamp a single device into a dense matrix.
/// This simulates what happens during a device load, but captures the stamps
/// in a dense matrix rather than the sparse MNA system.
fn stamp_device_into_dense(
    device: &mut dyn Device,
    states: &mut crate::state::StateVectors,
    mode: crate::mode::Mode,
    src_fact: f64,
    gmin: f64,
    matrix: &mut Vec<Vec<f64>>,
    rhs: &mut Vec<f64>,
    _size: usize,
) {
    // We need to capture what the device stamps into the matrix.
    // The approach: use a DenseStampCollector that implements the same interface.
    let mut collector = DenseStampCollector {
        matrix,
        rhs,
    };
    let mut noncon = false;
    device.load_into_dense(&mut collector, states, mode, src_fact, gmin, &mut noncon);
}

/// Dense stamp collector — captures stamps for sensitivity analysis.
pub struct DenseStampCollector<'a> {
    pub matrix: &'a mut Vec<Vec<f64>>,
    pub rhs: &'a mut Vec<f64>,
}

impl<'a> DenseStampCollector<'a> {
    pub fn stamp(&mut self, row: usize, col: usize, value: f64) {
        if row == 0 || col == 0 || value == 0.0 { return; }
        self.matrix[row][col] += value;
    }

    pub fn stamp_rhs(&mut self, row: usize, value: f64) {
        if row == 0 { return; }
        self.rhs[row] += value;
    }
}

/// Resolve the output specification for sensitivity analysis.
fn resolve_sens_output(
    circuit: &Circuit,
    output: &TfOutput,
) -> Result<(bool, usize, usize, usize), SimError> {
    match output {
        TfOutput::Voltage { pos_name, neg_name } => {
            let pos_eq = circuit.find_node(pos_name)
                .ok_or_else(|| SimError::DeviceNotFound(format!("node {pos_name}")))?;
            let neg_eq = neg_name.as_ref()
                .and_then(|n| circuit.find_node(n))
                .unwrap_or(0);
            Ok((true, pos_eq, neg_eq, 0))
        }
        TfOutput::Current { src_name } => {
            let src_upper = src_name.to_uppercase();
            for dev in &circuit.devices {
                if dev.name().to_uppercase() == src_upper {
                    if let Some(vs) = dev.as_any().downcast_ref::<crate::device::vsource::VoltageSource>() {
                        return Ok((false, 0, 0, vs.branch_eq));
                    }
                }
            }
            Err(SimError::DeviceNotFound(format!("source {src_name}")))
        }
    }
}

/// Run sensitivity analysis and return results as a HashMap matching
/// ngspice's sens plot vector names.
pub fn sens_analysis_to_map(
    circuit: &mut Circuit,
    config: &SimConfig,
    output: &TfOutput,
) -> Result<HashMap<String, f64>, SimError> {
    let results = sens_analysis(circuit, config, output)?;
    let mut map = HashMap::new();
    for r in results {
        map.insert(r.name, r.value);
    }
    Ok(map)
}
