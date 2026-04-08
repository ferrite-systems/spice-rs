//! Transfer function analysis — port of ngspice tfanal.c.
//!
//! `.TF V(out[,ref]) input_src` computes:
//!   1. Transfer function (V_out / V_in or V_out / I_in)
//!   2. Input impedance (Z_in)
//!   3. Output impedance (Z_out)
//!
//! Algorithm:
//!   1. Run DC operating point (CKTop)
//!   2. Zero RHS, set 1V (voltage src) or 1A (current src) excitation at input
//!   3. Solve the linearized system (forward/back substitution only)
//!   4. Read transfer function = V(out_pos) - V(out_neg)
//!   5. Read input impedance from the solution
//!   6. Zero RHS, set 1A excitation at output, re-solve for output impedance

use std::collections::HashMap;

use crate::circuit::Circuit;
use crate::config::SimConfig;
use crate::device::isource::CurrentSource;
use crate::device::vsource::VoltageSource;
use crate::error::SimError;
use crate::parser::TfOutput;

/// Transfer function result — 3 values matching ngspice's tf plot.
pub struct TfResult {
    /// transfer_function: V(out)/V(in), V(out)/I(in), I(out)/V(in), or I(out)/I(in)
    pub transfer_function: f64,
    /// Input impedance (or admittance for current source input)
    pub input_impedance: f64,
    /// Output impedance (or admittance for current source output)
    pub output_impedance: f64,
    /// Descriptive name for the output impedance (matches ngspice naming)
    pub output_impedance_name: String,
    /// Input source name (for naming)
    pub input_src_name: String,
}

/// Run transfer function analysis — faithful port of TFanal (tfanal.c).
///
/// The matrix is already factored after DC OP. We only do forward/back
/// substitution (SMPsolve) with modified RHS vectors.
pub fn tf_analysis(
    circuit: &mut Circuit,
    config: &SimConfig,
    output: &TfOutput,
    input_src_name: &str,
) -> Result<TfResult, SimError> {
    // Step 1: Run DC operating point (CKTop)
    let mut sim = crate::analysis::dc::dc_operating_point(circuit, config)?;

    let size = sim.mna.size();

    // Step 2: Find the input source and determine its type
    let input_src_upper = input_src_name.to_uppercase();
    let (in_is_voltage, in_pos_node, in_neg_node, in_branch_eq) = find_source_info(circuit, &input_src_upper)?;

    // Step 3: Zero the RHS and set excitation (tfanal.c:73-84)
    for i in 0..=size {
        sim.mna.rhs[i] = 0.0;
    }

    if !in_is_voltage {
        // Current source input: inject -1A at pos, +1A at neg
        // tfanal.c:79-80: CKTrhs[GENnode(ptr)[0]] -= 1; CKTrhs[GENnode(ptr)[1]] += 1;
        if in_pos_node > 0 { sim.mna.rhs[in_pos_node] -= 1.0; }
        if in_neg_node > 0 { sim.mna.rhs[in_neg_node] += 1.0; }
    } else {
        // Voltage source input: inject 1V via branch equation
        // tfanal.c:82-83: insrc = CKTfndBranch(...); CKTrhs[insrc] += 1;
        sim.mna.rhs[in_branch_eq] += 1.0;
    }

    // Step 4: Solve (forward/back substitution only, matrix already factored)
    // tfanal.c:87: SMPsolve(ckt->CKTmatrix, ckt->CKTrhs, ckt->CKTrhsSpare);
    sim.mna.solve_only()?;
    sim.mna.rhs[0] = 0.0; // tfanal.c:88

    // Step 5: Compute transfer function (tfanal.c:112-118)
    let (out_is_voltage, out_pos_eq, out_neg_eq, out_branch_eq, out_src_name) =
        resolve_output(circuit, output)?;

    let transfer_function = if out_is_voltage {
        // tfanal.c:113-114: CKTrhs[TFoutPos->number] - CKTrhs[TFoutNeg->number]
        sim.mna.rhs[out_pos_eq] - sim.mna.rhs[out_neg_eq]
    } else {
        // tfanal.c:116-117: outsrc = CKTfndBranch(...); CKTrhs[outsrc]
        sim.mna.rhs[out_branch_eq]
    };

    // Step 6: Compute input impedance (tfanal.c:121-130)
    let input_impedance = if !in_is_voltage {
        // Current source: Z_in = V(n-) - V(n+) (tfanal.c:122-123)
        sim.mna.rhs[in_neg_node] - sim.mna.rhs[in_pos_node]
    } else {
        // Voltage source: Z_in = -1/I(branch) (tfanal.c:125-129)
        let i_branch = sim.mna.rhs[in_branch_eq];
        if i_branch.abs() < 1e-20 {
            1e20
        } else {
            -1.0 / i_branch
        }
    };

    // Step 7: Check if output impedance computation can be skipped
    // tfanal.c:132-139: if output source == input source, Z_out = Z_in
    let output_impedance_name = if out_is_voltage {
        format!("output_impedance_at_{}", output_name(output))
    } else {
        format!("{}#output_impedance", out_src_name.to_lowercase())
    };

    if !out_is_voltage
        && out_src_name.to_uppercase() == input_src_upper
    {
        return Ok(TfResult {
            transfer_function,
            input_impedance,
            output_impedance: input_impedance,
            output_impedance_name,
            input_src_name: input_src_name.to_lowercase(),
        });
    }

    // Step 8: Compute output impedance (tfanal.c:141-157)
    // Zero the RHS again
    for i in 0..=size {
        sim.mna.rhs[i] = 0.0;
    }

    if out_is_voltage {
        // Voltage output: inject -1A at pos, +1A at neg (tfanal.c:145-146)
        if out_pos_eq > 0 { sim.mna.rhs[out_pos_eq] -= 1.0; }
        if out_neg_eq > 0 { sim.mna.rhs[out_neg_eq] += 1.0; }
    } else {
        // Current output: inject 1V via output source branch (tfanal.c:148)
        sim.mna.rhs[out_branch_eq] += 1.0;
    }

    sim.mna.solve_only()?;
    sim.mna.rhs[0] = 0.0;

    let output_impedance = if out_is_voltage {
        // tfanal.c:153-154: CKTrhs[TFoutNeg->number] - CKTrhs[TFoutPos->number]
        sim.mna.rhs[out_neg_eq] - sim.mna.rhs[out_pos_eq]
    } else {
        // tfanal.c:156: 1/MAX(1e-20, CKTrhs[outsrc])
        let i_branch = sim.mna.rhs[out_branch_eq];
        1.0 / f64::max(1e-20, i_branch)
    };

    Ok(TfResult {
        transfer_function,
        input_impedance,
        output_impedance,
        output_impedance_name,
        input_src_name: input_src_name.to_lowercase(),
    })
}

/// Find source info: (is_voltage, pos_node, neg_node, branch_eq).
/// For current sources, branch_eq is 0 (unused).
/// For voltage sources, branch_eq is the MNA branch equation.
fn find_source_info(circuit: &Circuit, name_upper: &str) -> Result<(bool, usize, usize, usize), SimError> {
    for dev in &circuit.devices {
        if dev.name().to_uppercase() == *name_upper {
            if let Some(vs) = dev.as_any().downcast_ref::<VoltageSource>() {
                return Ok((true, vs.pos_node, vs.neg_node, vs.branch_eq));
            }
            if let Some(cs) = dev.as_any().downcast_ref::<CurrentSource>() {
                return Ok((false, cs.pos_node, cs.neg_node, 0));
            }
        }
    }
    Err(SimError::DeviceNotFound(name_upper.to_string()))
}

/// Resolve output specification to equation numbers.
/// Returns (is_voltage, pos_eq, neg_eq, branch_eq, src_name).
fn resolve_output(
    circuit: &Circuit,
    output: &TfOutput,
) -> Result<(bool, usize, usize, usize, String), SimError> {
    match output {
        TfOutput::Voltage { pos_name, neg_name } => {
            let pos_eq = circuit.find_node(pos_name)
                .ok_or_else(|| SimError::DeviceNotFound(format!("node {pos_name}")))?;
            let neg_eq = neg_name.as_ref()
                .and_then(|n| circuit.find_node(n))
                .unwrap_or(0); // ground if not specified
            Ok((true, pos_eq, neg_eq, 0, String::new()))
        }
        TfOutput::Current { src_name } => {
            let src_upper = src_name.to_uppercase();
            for dev in &circuit.devices {
                if dev.name().to_uppercase() == src_upper {
                    if let Some(vs) = dev.as_any().downcast_ref::<VoltageSource>() {
                        return Ok((false, 0, 0, vs.branch_eq, src_name.clone()));
                    }
                }
            }
            Err(SimError::DeviceNotFound(format!("source {src_name}")))
        }
    }
}

/// Generate the output variable name for display (matching ngspice naming).
fn output_name(output: &TfOutput) -> String {
    match output {
        TfOutput::Voltage { pos_name, neg_name: Some(neg) } => {
            format!("v({},{})", pos_name.to_lowercase(), neg.to_lowercase())
        }
        TfOutput::Voltage { pos_name, neg_name: None } => {
            format!("v({})", pos_name.to_lowercase())
        }
        TfOutput::Current { src_name } => {
            format!("i({})", src_name.to_lowercase())
        }
    }
}

/// Run transfer function analysis and return results as a HashMap matching
/// ngspice's plot vector names.
pub fn tf_analysis_to_map(
    circuit: &mut Circuit,
    config: &SimConfig,
    output: &TfOutput,
    input_src_name: &str,
) -> Result<HashMap<String, f64>, SimError> {
    let result = tf_analysis(circuit, config, output, input_src_name)?;
    let mut map = HashMap::new();
    map.insert("transfer_function".to_string(), result.transfer_function);
    map.insert(result.output_impedance_name.to_lowercase(), result.output_impedance);
    map.insert(
        format!("{}#input_impedance", result.input_src_name),
        result.input_impedance,
    );
    Ok(map)
}
