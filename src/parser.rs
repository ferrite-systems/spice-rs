//! Minimal SPICE netlist parser — enough for L1/L2 eval circuits.
//!
//! Handles: R, C, L, V, I, E, G, F, H, T + .OP, .TRAN, .END
//! Value suffixes: T, G, MEG, k, m, u, n, p, f

use std::collections::HashMap;

use crate::circuit::Circuit;
use crate::device::capacitor::Capacitor;
use crate::device::cccs::Cccs;
use crate::device::ccvs::Ccvs;
use crate::device::inductor::Inductor;
use crate::device::isource::CurrentSource;
use crate::device::mutual_inductor::MutualInductor;
use crate::device::resistor::Resistor;
use crate::device::vccs::Vccs;
use crate::device::vcvs::Vcvs;
use crate::device::vsource::VoltageSource;
use crate::waveform::Waveform;
use crate::constants::{CHARGE, KoverQ};
const EPSSIL: f64 = 11.7 * 8.854214871e-12;

/// Parsed analysis directive.
#[derive(Debug, Clone)]
pub enum Analysis {
    Op,
    Tran { step: f64, stop: f64, uic: bool },
    /// DC sweep: `.DC srcname start stop step [src2 start2 stop2 step2]`
    DcSweep {
        src1: String,
        start1: f64,
        stop1: f64,
        step1: f64,
        /// Optional nested (outer) sweep
        src2: Option<String>,
        start2: f64,
        stop2: f64,
        step2: f64,
    },
    /// AC sweep: `.AC DEC|OCT|LIN num_points fstart fstop`
    Ac {
        sweep_type: AcSweepType,
        num_points: usize,
        fstart: f64,
        fstop: f64,
    },
    /// Transfer function: `.TF V(out[,ref]) input_src` or `.TF I(outsrc) input_src`
    Tf {
        /// Output specification
        output: TfOutput,
        /// Input source name (voltage or current source)
        input_src: String,
    },
    /// Sensitivity analysis: `.SENS V(out[,ref])` or `.SENS I(src)`
    Sens {
        /// Output specification
        output: TfOutput,
    },
    /// Pole-zero analysis: `.PZ node1 node2 node3 node4 VOL|CUR PZ|POL|ZER`
    Pz {
        /// Input positive node name
        in_pos: String,
        /// Input negative node name
        in_neg: String,
        /// Output positive node name
        out_pos: String,
        /// Output negative node name
        out_neg: String,
        /// Input type: voltage or current
        input_type: PzInputType,
        /// What to compute: poles, zeros, or both
        pz_type: PzAnalysisType,
    },
}

/// PZ input type — VOL or CUR.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PzInputType {
    Vol,
    Cur,
}

/// PZ analysis type — POL, ZER, or PZ (both).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PzAnalysisType {
    Poles,
    Zeros,
    Both,
}

/// Transfer function / sensitivity output specification.
#[derive(Debug, Clone)]
pub enum TfOutput {
    /// Voltage output: V(node_pos) or V(node_pos, node_neg)
    Voltage { pos_name: String, neg_name: Option<String> },
    /// Current output: I(source_name)
    Current { src_name: String },
}

/// AC sweep type — matches ngspice DECADE/OCTAVE/LINEAR.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AcSweepType {
    Dec,
    Oct,
    Lin,
}

/// Parsed K (coupled inductor) spec: name, inductor1_name, inductor2_name, coupling.
#[derive(Debug, Clone)]
pub struct CoupledInductorSpec {
    pub name: String,
    pub ind1_name: String,
    pub ind2_name: String,
    pub coupling: f64,
}

/// Parse result: circuit + analysis directive.
pub struct ParseResult {
    pub circuit: Circuit,
    pub analysis: Analysis,
    pub title: String,
    pub temp: Option<f64>,
    pub tnom: Option<f64>,
    pub ic_nodes: Vec<(String, f64)>,      // .IC V(node)=value pairs
    pub nodeset_nodes: Vec<(String, f64)>, // .NODESET V(node)=value pairs
    /// K element specs — resolved after circuit.setup() when inductor state offsets are known.
    pub k_specs: Vec<CoupledInductorSpec>,
    /// .OPTIONS ABSTOL (CKTabstol)
    pub abstol: Option<f64>,
    /// .OPTIONS VNTOL (CKTvoltTol)
    pub vntol: Option<f64>,
    /// .OPTIONS RELTOL (CKTreltol)
    pub reltol: Option<f64>,
}

/// Intermediate parsed device spec — collected during parsing, then instantiated
/// in ngspice DEVices[] type order to match equation numbering.
/// AC stimulus parameters parsed from source lines: `AC mag [phase]`
#[derive(Debug, Clone, Default)]
pub struct AcParams {
    pub mag: f64,
    pub phase_deg: f64,
}

enum DeviceSpec {
    Resistor { name: String, n1: usize, n2: usize, value: f64, ac_value: Option<f64> },
    Capacitor { name: String, n1: usize, n2: usize, value: f64 },
    Inductor { name: String, n1: usize, n2: usize, value: f64, ic: Option<f64> },
    VoltageSource { name: String, n1: usize, n2: usize, waveform: Waveform, ac: Option<AcParams> },
    CurrentSource { name: String, n1: usize, n2: usize, waveform: Waveform, ac: Option<AcParams> },
    Vcvs { name: String, n1: usize, n2: usize, cp: usize, cn: usize, gain: f64 },
    Vccs { name: String, n1: usize, n2: usize, cp: usize, cn: usize, gm: f64 },
    Cccs { name: String, n1: usize, n2: usize, ctrl_name: String, gain: f64 },
    Ccvs { name: String, n1: usize, n2: usize, ctrl_name: String, tr: f64 },
    Diode { name: String, n_anode: usize, n_cathode: usize,
            model: crate::device::diode::DiodeModel, area: f64, model_name: String },
    Mosfet { name: String, nd: usize, ng: usize, ns: usize, nb: usize,
             model: crate::device::mosfet1::Mos1Model, w: f64, l: f64, m: f64, model_name: String },
    Mosfet2 { name: String, nd: usize, ng: usize, ns: usize, nb: usize,
              model: crate::device::mosfet2::Mos2Model, w: f64, l: f64, m: f64, model_name: String },
    Mosfet3 { name: String, nd: usize, ng: usize, ns: usize, nb: usize,
              model: crate::device::mosfet3::Mos3Model, w: f64, l: f64, m: f64, model_name: String },
    Bsim3 { name: String, nd: usize, ng: usize, ns: usize, nb: usize,
            model: crate::device::bsim3::Bsim3Model, w: f64, l: f64, m: f64, model_name: String },
    Bsim4 { name: String, nd: usize, ng: usize, ns: usize, nb: usize,
            model: crate::device::bsim4::Bsim4Model, w: f64, l: f64, m: f64, model_name: String },
    Bjt { name: String, nc: usize, nb: usize, ne: usize, ns: usize,
          model: crate::device::bjt::BjtModel, area: f64, off: bool, model_name: String },
    Jfet { name: String, nd: usize, ng: usize, ns: usize,
           model: crate::device::jfet::JfetModel, area: f64, model_name: String },
    TLine { name: String, p1: usize, n1: usize, p2: usize, n2: usize,
            z0: f64, td: f64, td_given: bool, nl: f64, freq: f64 },
}

impl DeviceSpec {
    /// ngspice DEVices[] type index — determines CKTsetup iteration order.
    /// Devices are set up in this order, which controls when branches and
    /// internal nodes are created (and thus their equation numbers).
    fn type_order(&self) -> u32 {
        match self {
            DeviceSpec::Bjt { .. }            => 14,  // BJT
            DeviceSpec::Capacitor { .. }      => 17,  // CAP
            DeviceSpec::Cccs { .. }           => 18,  // CCCS
            DeviceSpec::Ccvs { .. }           => 19,  // CCVS
            DeviceSpec::Diode { .. }          => 22,  // DIO
            DeviceSpec::Inductor { .. }       => 29,  // IND
            DeviceSpec::CurrentSource { .. }  => 31,  // ISRC
            DeviceSpec::Jfet { .. }           => 32,  // JFET
            DeviceSpec::Mosfet { .. }         => 33,  // MOS1
            DeviceSpec::Mosfet2 { .. }        => 34,  // MOS2
            DeviceSpec::Mosfet3 { .. }        => 35,  // MOS3
            DeviceSpec::Bsim3 { .. }          => 5,   // BSIM3
            DeviceSpec::Bsim4 { .. }          => 10,  // BSIM4
            DeviceSpec::Resistor { .. }       => 42,  // RES
            DeviceSpec::TLine { .. }          => 45,  // TRA
            DeviceSpec::Vccs { .. }           => 48,  // VCCS
            DeviceSpec::Vcvs { .. }           => 49,  // VCVS
            DeviceSpec::VoltageSource { .. }  => 50,  // VSRC
        }
    }

    /// Return model name for devices that have one, None otherwise.
    fn model_name(&self) -> Option<&str> {
        match self {
            DeviceSpec::Bjt { model_name, .. } |
            DeviceSpec::Diode { model_name, .. } |
            DeviceSpec::Mosfet { model_name, .. } |
            DeviceSpec::Mosfet2 { model_name, .. } |
            DeviceSpec::Mosfet3 { model_name, .. } |
            DeviceSpec::Bsim3 { model_name, .. } |
            DeviceSpec::Bsim4 { model_name, .. } |
            DeviceSpec::Jfet { model_name, .. } => Some(model_name),
            _ => None,
        }
    }
}

/// Get or create a branch equation for a device, matching ngspice's
/// "create if not exists" pattern (VSRCsetup checks `if(here->VSRCbranch == 0)`).
fn get_or_create_branch(
    circuit: &mut Circuit,
    branch_map: &mut HashMap<String, usize>,
    name: &str,
) -> usize {
    let key = name.to_uppercase();
    if let Some(&br) = branch_map.get(&key) {
        return br;
    }
    let br = circuit.branch(&format!("{name}#branch"));
    branch_map.insert(key, br);
    br
}

/// Find a controlling source's branch, creating it on demand.
/// Matches ngspice CKTfndBranch → VSRCfindBr: when CCCS/CCVS needs a branch
/// from a source that hasn't been set up yet, create the branch early.
fn find_control_branch(
    circuit: &mut Circuit,
    branch_map: &mut HashMap<String, usize>,
    source_names: &HashMap<String, String>,
    ctrl_name: &str,
    requesting_device: &str,
) -> Result<usize, String> {
    let key = ctrl_name.to_uppercase();
    if let Some(&br) = branch_map.get(&key) {
        return Ok(br);
    }
    if let Some(orig_name) = source_names.get(&key) {
        let br = circuit.branch(&format!("{orig_name}#branch"));
        branch_map.insert(key, br);
        return Ok(br);
    }
    Err(format!("{requesting_device}: unknown controlling source '{ctrl_name}'"))
}

/// Join SPICE continuation lines: lines starting with `+` are appended
/// to the previous line. Port of ngspice INPgetLine / inp_readall.
fn join_continuation_lines(netlist: &str) -> String {
    let mut joined = Vec::new();
    for line in netlist.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('+') {
            // Continuation: append to previous line (without the '+')
            if let Some(prev) = joined.last_mut() {
                let prev: &mut String = prev;
                prev.push(' ');
                prev.push_str(trimmed[1..].trim());
            }
        } else {
            joined.push(trimmed.to_string());
        }
    }
    joined.join("\n")
}

/// Subcircuit definition collected during preprocessing.
#[derive(Debug, Clone)]
struct SubcktDef {
    /// Interface pin names (formal parameters).
    pins: Vec<String>,
    /// Lines inside the subcircuit body (between .SUBCKT and .ENDS).
    body: Vec<String>,
}

/// Expand .SUBCKT definitions and X instantiations — port of ngspice subckt.c.
///
/// This is a textual preprocessing step that runs BEFORE the main parse.
/// Each X instance is replaced by the subcircuit body with:
/// - Interface pins mapped to the instance's connections
/// - Internal nodes prefixed with `scname.` (where scname is the X line name minus 'X')
/// - Device names prefixed with `scname.`
/// - Model names inside .SUBCKT prefixed with `scname:`
/// - "0" is treated as a global node and never prefixed
///
/// Nested subcircuits are handled by recursively expanding definitions.
fn expand_subcircuits(lines: &[String]) -> Result<Vec<String>, String> {
    // Pass 1: collect .SUBCKT definitions and separate them from the main deck.
    let mut subckt_defs: HashMap<String, SubcktDef> = HashMap::new();
    let mut main_deck: Vec<String> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim().to_string();
        let upper = line.to_uppercase();

        if upper.starts_with(".SUBCKT") {
            // Parse .SUBCKT name pin1 pin2 ...
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                return Err(format!("Invalid .SUBCKT line: {line}"));
            }
            let subckt_name = parts[1].to_uppercase();
            // Pins are everything after the name, stopping at PARAMS: or end
            let mut pins = Vec::new();
            for &p in &parts[2..] {
                if p.to_uppercase().starts_with("PARAMS:") {
                    break;
                }
                pins.push(p.to_uppercase());
            }

            // Collect body lines until .ENDS
            let mut body = Vec::new();
            i += 1;
            let mut nest = 0;
            while i < lines.len() {
                let bline = lines[i].trim().to_string();
                let bupper = bline.to_uppercase();
                if bupper.starts_with(".SUBCKT") {
                    nest += 1;
                    body.push(bline);
                } else if bupper.starts_with(".ENDS") {
                    if nest == 0 {
                        break;
                    }
                    nest -= 1;
                    body.push(bline);
                } else {
                    body.push(bline);
                }
                i += 1;
            }
            // i now points to the .ENDS line, skip it
            i += 1;

            subckt_defs.insert(subckt_name, SubcktDef { pins, body });
        } else {
            main_deck.push(line);
            i += 1;
        }
    }

    if subckt_defs.is_empty() {
        return Ok(main_deck);
    }

    // Recursively expand nested subcircuits within definitions.
    // Iterate until no more X instances are found within any definition.
    let max_passes = 100;
    for _ in 0..max_passes {
        let mut any_expanded = false;
        let names: Vec<String> = subckt_defs.keys().cloned().collect();
        for name in &names {
            let def = subckt_defs[name].clone();
            let expanded = expand_x_instances(&def.body, &subckt_defs)?;
            if expanded != def.body {
                any_expanded = true;
                subckt_defs.get_mut(name).unwrap().body = expanded;
            }
        }
        if !any_expanded {
            break;
        }
    }

    // Pass 2: expand X instances in the main deck.
    let result = expand_x_instances(&main_deck, &subckt_defs)?;
    Ok(result)
}

/// Expand all X instances in a list of lines using the given subcircuit definitions.
fn expand_x_instances(
    lines: &[String],
    subckt_defs: &HashMap<String, SubcktDef>,
) -> Result<Vec<String>, String> {
    let mut result = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('*') {
            result.push(line.clone());
            continue;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.is_empty() {
            result.push(line.clone());
            continue;
        }

        let first_char = parts[0].chars().next().unwrap_or(' ').to_ascii_uppercase();
        if first_char != 'X' {
            result.push(line.clone());
            continue;
        }

        // X instance line: Xname node1 node2 ... subckt_name
        if parts.len() < 3 {
            return Err(format!("Invalid X instance: {trimmed}"));
        }

        let inst_name = parts[0]; // e.g., "X1"
        // scname = instance name without leading 'X' (ngspice convention)
        let scname = &inst_name[1..];

        // The subcircuit name is the LAST token on the line
        let subckt_name = parts[parts.len() - 1].to_uppercase();

        let def = subckt_defs.get(&subckt_name).ok_or_else(|| {
            format!("Unknown subcircuit '{subckt_name}' in: {trimmed}")
        })?;

        // Actual nodes are everything between the instance name and the subcircuit name
        let actual_nodes: Vec<&str> = parts[1..parts.len() - 1].to_vec();

        if actual_nodes.len() != def.pins.len() {
            return Err(format!(
                "Subcircuit '{}' expects {} pins ({:?}), but instance '{}' provides {} ({:?})",
                subckt_name, def.pins.len(), def.pins,
                inst_name, actual_nodes.len(), actual_nodes
            ));
        }

        // Build the formal→actual pin mapping
        let mut pin_map: HashMap<String, String> = HashMap::new();
        for (formal, actual) in def.pins.iter().zip(actual_nodes.iter()) {
            pin_map.insert(formal.to_string(), actual.to_string());
        }

        // Expand the body, translating node and device names
        let expanded = expand_subckt_body(
            &def.body,
            scname,
            &pin_map,
            &subckt_name,
            subckt_defs,
        )?;
        result.extend(expanded);
    }

    Ok(result)
}

/// Expand a single subcircuit body, translating names according to ngspice's convention.
///
/// - Interface pins → mapped to actual nodes from the X line
/// - Internal nodes → prefixed with `scname.`
/// - Device names → prefixed (e.g., R1 → R.scname.R1 for simple devices)
/// - Model names defined inside .SUBCKT → prefixed with `scname:`
/// - Global nodes ("0") → never prefixed
fn expand_subckt_body(
    body: &[String],
    scname: &str,
    pin_map: &HashMap<String, String>,
    _subckt_name: &str,
    subckt_defs: &HashMap<String, SubcktDef>,
) -> Result<Vec<String>, String> {
    let scname_upper = scname.to_uppercase();
    let mut result = Vec::new();

    // Collect model names defined inside this subcircuit body.
    let mut local_models: std::collections::HashSet<String> = std::collections::HashSet::new();
    for line in body {
        let upper = line.to_uppercase();
        if upper.starts_with(".MODEL") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                local_models.insert(parts[1].to_uppercase());
            }
        }
    }

    for line in body {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('*') {
            continue; // skip comments in subcircuit body
        }

        let upper = trimmed.to_uppercase();

        // Handle .MODEL lines inside subcircuit: rename the model
        if upper.starts_with(".MODEL") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 3 {
                let old_model_name = parts[1];
                let new_model_name = format!("{}:{}", scname_upper, old_model_name.to_uppercase());
                // Rebuild: .MODEL new_name rest...
                let rest: String = parts[2..].join(" ");
                result.push(format!(".MODEL {} {}", new_model_name, rest));
            }
            continue;
        }

        // Skip other directives (but handle .IC, .NODESET inside subcircuits)
        if trimmed.starts_with('.') {
            // Translate node names in .IC and .NODESET
            if upper.starts_with(".IC ") || upper.starts_with(".NODESET") {
                let translated = translate_ic_nodeset(trimmed, &scname_upper, pin_map);
                result.push(translated);
            }
            // Other directives pass through (shouldn't normally appear in subcircuits)
            continue;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let dev_name = parts[0];
        let first_char = dev_name.chars().next().unwrap_or(' ').to_ascii_uppercase();

        match first_char {
            'R' | 'C' | 'L' => {
                // 2-terminal passive: name node1 node2 value [params]
                if parts.len() < 4 {
                    return Err(format!("Invalid device in subcircuit: {trimmed}"));
                }
                let new_name = translate_device_name(dev_name, &scname_upper);
                let n1 = translate_node(parts[1], &scname_upper, pin_map);
                let n2 = translate_node(parts[2], &scname_upper, pin_map);
                let rest: String = parts[3..].join(" ");
                result.push(format!("{} {} {} {}", new_name, n1, n2, rest));
            }
            'V' | 'I' => {
                // Source: name node1 node2 [spec]
                if parts.len() < 3 {
                    return Err(format!("Invalid source in subcircuit: {trimmed}"));
                }
                let new_name = translate_device_name(dev_name, &scname_upper);
                let n1 = translate_node(parts[1], &scname_upper, pin_map);
                let n2 = translate_node(parts[2], &scname_upper, pin_map);
                let rest: String = parts[3..].join(" ");
                if rest.is_empty() {
                    result.push(format!("{} {} {}", new_name, n1, n2));
                } else {
                    result.push(format!("{} {} {} {}", new_name, n1, n2, rest));
                }
            }
            'D' => {
                // Diode: name anode cathode model [area]
                if parts.len() < 4 {
                    return Err(format!("Invalid diode in subcircuit: {trimmed}"));
                }
                let new_name = translate_device_name(dev_name, &scname_upper);
                let n1 = translate_node(parts[1], &scname_upper, pin_map);
                let n2 = translate_node(parts[2], &scname_upper, pin_map);
                let model = translate_model_name(parts[3], &scname_upper, &local_models);
                let rest: String = parts[4..].join(" ");
                if rest.is_empty() {
                    result.push(format!("{} {} {} {}", new_name, n1, n2, model));
                } else {
                    result.push(format!("{} {} {} {} {}", new_name, n1, n2, model, rest));
                }
            }
            'Q' => {
                // BJT: name collector base emitter [substrate] model [area]
                if parts.len() < 5 {
                    return Err(format!("Invalid BJT in subcircuit: {trimmed}"));
                }
                let new_name = translate_device_name(dev_name, &scname_upper);
                let nc = translate_node(parts[1], &scname_upper, pin_map);
                let nb = translate_node(parts[2], &scname_upper, pin_map);
                let ne = translate_node(parts[3], &scname_upper, pin_map);
                // Determine if parts[4] is a node or model name
                let p4_upper = parts[4].to_uppercase();
                if local_models.contains(&p4_upper) || (!subckt_defs.contains_key(&p4_upper) && !p4_upper.chars().next().map_or(false, |c| c.is_ascii_digit())) {
                    // Could be model — check if it looks like a model name
                    // This heuristic: if it's in local_models, it's a model.
                    // Otherwise check remaining context.
                    // ngspice uses modnames list; we use local_models + known models
                    if parts.len() >= 6 {
                        // Could be: Q c b e sub model [area] or Q c b e model area
                        // Check if parts[5] is also a potential model name
                        let p5_upper = parts[5].to_uppercase();
                        if local_models.contains(&p5_upper) {
                            // Q c b e sub model [area]
                            let ns = translate_node(parts[4], &scname_upper, pin_map);
                            let model = translate_model_name(parts[5], &scname_upper, &local_models);
                            let rest: String = parts[6..].join(" ");
                            if rest.is_empty() {
                                result.push(format!("{} {} {} {} {} {}", new_name, nc, nb, ne, ns, model));
                            } else {
                                result.push(format!("{} {} {} {} {} {} {}", new_name, nc, nb, ne, ns, model, rest));
                            }
                        } else {
                            // Q c b e model area
                            let model = translate_model_name(parts[4], &scname_upper, &local_models);
                            let rest: String = parts[5..].join(" ");
                            result.push(format!("{} {} {} {} {} {}", new_name, nc, nb, ne, model, rest));
                        }
                    } else {
                        // Q c b e model
                        let model = translate_model_name(parts[4], &scname_upper, &local_models);
                        result.push(format!("{} {} {} {} {}", new_name, nc, nb, ne, model));
                    }
                } else {
                    // parts[4] is a substrate node
                    let ns = translate_node(parts[4], &scname_upper, pin_map);
                    if parts.len() >= 6 {
                        let model = translate_model_name(parts[5], &scname_upper, &local_models);
                        let rest: String = parts[6..].join(" ");
                        if rest.is_empty() {
                            result.push(format!("{} {} {} {} {} {}", new_name, nc, nb, ne, ns, model));
                        } else {
                            result.push(format!("{} {} {} {} {} {} {}", new_name, nc, nb, ne, ns, model, rest));
                        }
                    } else {
                        result.push(format!("{} {} {} {} {}", new_name, nc, nb, ne, ns));
                    }
                }
            }
            'M' => {
                // MOSFET: name drain gate source bulk model [W=val] [L=val]
                if parts.len() < 6 {
                    return Err(format!("Invalid MOSFET in subcircuit: {trimmed}"));
                }
                let new_name = translate_device_name(dev_name, &scname_upper);
                let nd = translate_node(parts[1], &scname_upper, pin_map);
                let ng = translate_node(parts[2], &scname_upper, pin_map);
                let ns = translate_node(parts[3], &scname_upper, pin_map);
                let nb = translate_node(parts[4], &scname_upper, pin_map);
                let model = translate_model_name(parts[5], &scname_upper, &local_models);
                let rest: String = parts[6..].join(" ");
                if rest.is_empty() {
                    result.push(format!("{} {} {} {} {} {}", new_name, nd, ng, ns, nb, model));
                } else {
                    result.push(format!("{} {} {} {} {} {} {}", new_name, nd, ng, ns, nb, model, rest));
                }
            }
            'J' => {
                // JFET: name drain gate source model [area]
                if parts.len() < 5 {
                    return Err(format!("Invalid JFET in subcircuit: {trimmed}"));
                }
                let new_name = translate_device_name(dev_name, &scname_upper);
                let nd = translate_node(parts[1], &scname_upper, pin_map);
                let ng = translate_node(parts[2], &scname_upper, pin_map);
                let ns = translate_node(parts[3], &scname_upper, pin_map);
                let model = translate_model_name(parts[4], &scname_upper, &local_models);
                let rest: String = parts[5..].join(" ");
                if rest.is_empty() {
                    result.push(format!("{} {} {} {} {}", new_name, nd, ng, ns, model));
                } else {
                    result.push(format!("{} {} {} {} {} {}", new_name, nd, ng, ns, model, rest));
                }
            }
            'E' | 'G' => {
                // VCVS/VCCS: name n+ n- nc+ nc- gain
                // Or POLY: name n+ n- POLY(dim) nc1+ nc1- [nc2+ nc2-...] coeffs...
                if parts.len() < 4 {
                    return Err(format!("Invalid controlled source in subcircuit: {trimmed}"));
                }
                let new_name = translate_device_name(dev_name, &scname_upper);
                let n1 = translate_node(parts[1], &scname_upper, pin_map);
                let n2 = translate_node(parts[2], &scname_upper, pin_map);

                // Check for POLY keyword
                let p3_upper = parts[3].to_uppercase();
                if p3_upper.starts_with("POLY") {
                    // POLY(dim) nc1+ nc1- [nc2+ nc2-...] coeffs...
                    // Extract dimension from POLY(N)
                    let dim_str = p3_upper.trim_start_matches("POLY");
                    let dim_str = dim_str.trim_start_matches('(').trim_end_matches(')');
                    let dim: usize = dim_str.parse().unwrap_or(1);
                    let num_ctrl_nodes = dim * 2; // pairs of control nodes

                    let mut out_parts = vec![new_name, n1, n2, parts[3].to_string()];
                    // Translate control node pairs
                    let ctrl_start = 4;
                    for j in 0..num_ctrl_nodes {
                        let idx = ctrl_start + j;
                        if idx < parts.len() {
                            out_parts.push(translate_node(parts[idx], &scname_upper, pin_map));
                        }
                    }
                    // Rest are coefficients — pass through unchanged
                    let coeff_start = ctrl_start + num_ctrl_nodes;
                    for j in coeff_start..parts.len() {
                        out_parts.push(parts[j].to_string());
                    }
                    result.push(out_parts.join(" "));
                } else {
                    // Standard: name n+ n- nc+ nc- gain
                    if parts.len() < 6 {
                        return Err(format!("Invalid controlled source in subcircuit: {trimmed}"));
                    }
                    let cp = translate_node(parts[3], &scname_upper, pin_map);
                    let cn = translate_node(parts[4], &scname_upper, pin_map);
                    let rest: String = parts[5..].join(" ");
                    result.push(format!("{} {} {} {} {} {}", new_name, n1, n2, cp, cn, rest));
                }
            }
            'F' | 'H' => {
                // CCCS/CCVS: name n+ n- ctrl_source gain
                if parts.len() < 5 {
                    return Err(format!("Invalid controlled source in subcircuit: {trimmed}"));
                }
                let new_name = translate_device_name(dev_name, &scname_upper);
                let n1 = translate_node(parts[1], &scname_upper, pin_map);
                let n2 = translate_node(parts[2], &scname_upper, pin_map);
                // Controlling source name gets instance-translated
                let ctrl = translate_device_name(parts[3], &scname_upper);
                let rest: String = parts[4..].join(" ");
                result.push(format!("{} {} {} {} {}", new_name, n1, n2, ctrl, rest));
            }
            'T' => {
                // Transmission line: name p1+ p1- p2+ p2- params...
                if parts.len() < 5 {
                    return Err(format!("Invalid transmission line in subcircuit: {trimmed}"));
                }
                let new_name = translate_device_name(dev_name, &scname_upper);
                let np1 = translate_node(parts[1], &scname_upper, pin_map);
                let nn1 = translate_node(parts[2], &scname_upper, pin_map);
                let np2 = translate_node(parts[3], &scname_upper, pin_map);
                let nn2 = translate_node(parts[4], &scname_upper, pin_map);
                let rest: String = parts[5..].join(" ");
                if rest.is_empty() {
                    result.push(format!("{} {} {} {} {}", new_name, np1, nn1, np2, nn2));
                } else {
                    result.push(format!("{} {} {} {} {} {}", new_name, np1, nn1, np2, nn2, rest));
                }
            }
            'K' => {
                // Coupled inductor: K1 L1 L2 coupling
                if parts.len() < 4 {
                    return Err(format!("Invalid coupled inductor in subcircuit: {trimmed}"));
                }
                let new_name = translate_device_name(dev_name, &scname_upper);
                let l1 = translate_device_name(parts[1], &scname_upper);
                let l2 = translate_device_name(parts[2], &scname_upper);
                let rest: String = parts[3..].join(" ");
                result.push(format!("{} {} {} {}", new_name, l1, l2, rest));
            }
            'X' => {
                // Nested X instance — translate nodes and pass through for later expansion
                // (The outer expand_x_instances loop will handle recursive expansion)
                if parts.len() < 3 {
                    return Err(format!("Invalid X instance in subcircuit: {trimmed}"));
                }
                let new_name = format!("{}.{}", scname_upper, &dev_name[1..].to_uppercase());
                let new_x_name = format!("X{}", new_name);
                // Last token is the subcircuit name (don't translate it)
                let subckt_ref = parts[parts.len() - 1];
                // Middle tokens are nodes
                let mut translated_parts = vec![new_x_name];
                for &node in &parts[1..parts.len() - 1] {
                    translated_parts.push(translate_node(node, &scname_upper, pin_map));
                }
                translated_parts.push(subckt_ref.to_string());
                result.push(translated_parts.join(" "));
            }
            _ => {
                // Unknown device type — pass through with basic translation
                result.push(line.clone());
            }
        }
    }

    Ok(result)
}

/// Translate a node name according to ngspice convention.
/// - If the node is in the pin_map (interface pin), return the mapped name.
/// - If the node is "0" (global ground), return "0".
/// - Otherwise, prefix with `scname.` for internal nodes.
fn translate_node(node: &str, scname: &str, pin_map: &HashMap<String, String>) -> String {
    let upper = node.to_uppercase();
    // Global ground node is never prefixed
    if upper == "0" {
        return "0".to_string();
    }
    // Check if this is an interface pin
    if let Some(actual) = pin_map.get(&upper) {
        return actual.clone();
    }
    // Internal node: prefix with scname.
    format!("{}.{}", scname, upper)
}

/// Translate a device/instance name according to ngspice convention.
///
/// ngspice translate_inst_name (subckt.c:1159-1171):
///   if first char is NOT 'x': first_char + "." + scname + "." + full_name
///     e.g., R1 in X1 (scname="1") → "R.1.R1"
///   if first char IS 'x': scname + "." + name_without_x
///     e.g., X2 in X1 → "X1.2" (but caller manages the "X" prefix)
///
/// The resulting name starts with the correct device-type letter, which is
/// what the main parser uses to determine device type.
fn translate_device_name(name: &str, scname: &str) -> String {
    let first_char = name.chars().next().unwrap_or(' ').to_ascii_uppercase();
    let name_upper = name.to_uppercase();
    if first_char == 'X' {
        // X device: "X" + scname + "." + rest_without_x
        format!("X{}.{}", scname, &name_upper[1..])
    } else {
        // Regular device: first_char + "." + scname + "." + full_name
        format!("{}.{}.{}", first_char, scname, name_upper)
    }
}

/// Translate a model name: if it's a locally-defined model, prefix with `scname:`.
fn translate_model_name(
    model: &str,
    scname: &str,
    local_models: &std::collections::HashSet<String>,
) -> String {
    let upper = model.to_uppercase();
    if local_models.contains(&upper) {
        format!("{}:{}", scname, upper)
    } else {
        model.to_string()
    }
}

/// Translate node names in .IC and .NODESET lines inside subcircuits.
fn translate_ic_nodeset(
    line: &str,
    scname: &str,
    pin_map: &HashMap<String, String>,
) -> String {
    // Simple approach: find V(node)=value patterns and translate the node names
    let mut result = String::new();
    let mut i = 0;
    let bytes = line.as_bytes();

    while i < bytes.len() {
        // Look for V( pattern
        if i + 2 < bytes.len()
            && (bytes[i] == b'V' || bytes[i] == b'v')
            && bytes[i + 1] == b'('
        {
            result.push(bytes[i] as char);
            result.push('(');
            i += 2;
            // Extract node name until )
            let start = i;
            while i < bytes.len() && bytes[i] != b')' && bytes[i] != b'=' {
                i += 1;
            }
            let node = &line[start..i];
            let translated = translate_node(node, scname, pin_map);
            result.push_str(&translated);
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }

    result
}

/// Parse a SPICE netlist string into a Circuit + Analysis.
pub fn parse_netlist(netlist: &str) -> Result<ParseResult, String> {
    // Step 0: join `+` continuation lines (ngspice INPgetLine)
    let netlist = join_continuation_lines(netlist);

    // Step 0.5: expand .SUBCKT definitions and X instances (ngspice subckt.c)
    let lines: Vec<String> = netlist.lines().map(|l| l.to_string()).collect();
    let expanded_lines = expand_subcircuits(&lines)?;
    let netlist = expanded_lines.join("\n");
    let netlist = netlist.as_str();

    let mut circuit = Circuit::new();
    let mut analysis = None;
    let mut title = String::new();
    let mut first_line = true;
    let mut diode_models: HashMap<String, crate::device::diode::DiodeModel> = HashMap::new();
    let mut mos_models: HashMap<String, crate::device::mosfet1::Mos1Model> = HashMap::new();
    let mut mos2_models: HashMap<String, crate::device::mosfet2::Mos2Model> = HashMap::new();
    let mut mos3_models: HashMap<String, crate::device::mosfet3::Mos3Model> = HashMap::new();
    let mut bsim3_models: HashMap<String, crate::device::bsim3::Bsim3Model> = HashMap::new();
    let mut bsim4_models: HashMap<String, crate::device::bsim4::Bsim4Model> = HashMap::new();
    let mut bjt_models: HashMap<String, crate::device::bjt::BjtModel> = HashMap::new();
    let mut jfet_models: HashMap<String, crate::device::jfet::JfetModel> = HashMap::new();
    let mut temp_celsius: Option<f64> = None;
    let mut tnom_celsius: Option<f64> = None;
    let mut ic_nodes: Vec<(String, f64)> = Vec::new();
    let mut nodeset_nodes: Vec<(String, f64)> = Vec::new();
    let mut opt_abstol: Option<f64> = None;
    let mut opt_vntol: Option<f64> = None;
    let mut opt_reltol: Option<f64> = None;

    // Pre-scan pass 1: collect .MODEL directives (ngspice pass 2)
    for line in netlist.lines() {
        let line = line.trim();
        if line.to_uppercase().starts_with(".MODEL") {
            parse_model_line(line, &mut diode_models, &mut mos_models, &mut mos2_models, &mut mos3_models, &mut bsim3_models, &mut bsim4_models, &mut bjt_models, &mut jfet_models)?;
        }
    }

    // Pre-scan pass 2: create all voltage nodes (ngspice creates terminal nodes during INPpas2)
    // This ensures branch equations get higher equation numbers than voltage nodes,
    // matching ngspice's CKTmkVolt (during parsing) vs CKTmkCur (during CKTsetup) ordering.
    {
        let mut seen_title = false;
        for line in netlist.lines() {
            let line = line.trim();
            if !seen_title && !line.is_empty() { seen_title = true; continue; }
            if line.is_empty() || line.starts_with('*') || line.starts_with('.') { continue; }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let first_char = parts[0].chars().next().unwrap().to_ascii_uppercase();
                match first_char {
                    'R' | 'C' | 'L' | 'V' | 'I' | 'D' | 'F' | 'H' => {
                        circuit.node(parts[1]);
                        circuit.node(parts[2]);
                    }
                    'E' | 'G' => {
                        circuit.node(parts[1]);
                        circuit.node(parts[2]);
                        if parts.len() >= 4 && parts[3].to_uppercase().starts_with("POLY") {
                            // POLY(dim): control nodes start at parts[4]
                            let dim_str = parts[3].to_uppercase();
                            let dim_str = dim_str.trim_start_matches("POLY");
                            let dim_str = dim_str.trim_start_matches('(').trim_end_matches(')');
                            let dim: usize = dim_str.parse().unwrap_or(1);
                            for j in 0..dim * 2 {
                                if 4 + j < parts.len() {
                                    circuit.node(parts[4 + j]);
                                }
                            }
                        } else if parts.len() >= 5 {
                            circuit.node(parts[3]);
                            circuit.node(parts[4]);
                        }
                    }
                    'J' => {
                        // JFET: J name drain gate source model
                        if parts.len() >= 4 {
                            circuit.node(parts[1]);
                            circuit.node(parts[2]);
                            circuit.node(parts[3]);
                        }
                    }
                    'Q' => {
                        // BJT: Q name collector base emitter [substrate] model
                        if parts.len() >= 4 {
                            circuit.node(parts[1]);
                            circuit.node(parts[2]);
                            circuit.node(parts[3]);
                        }
                    }
                    'M' => {
                        // MOSFET: M name drain gate source bulk model
                        if parts.len() >= 5 {
                            circuit.node(parts[1]);
                            circuit.node(parts[2]);
                            circuit.node(parts[3]);
                            circuit.node(parts[4]);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Main parse: collect device specs (no branches/internal nodes created here)
    let mut specs: Vec<DeviceSpec> = Vec::new();
    // K lines: (name, inductor1_name, inductor2_name, coupling_coefficient)
    let mut k_specs: Vec<(String, String, String, f64)> = Vec::new();

    for line in netlist.lines() {
        let line = line.trim();

        if first_line && !line.is_empty() {
            title = line.to_string();
            first_line = false;
            continue;
        }
        first_line = false;

        if line.is_empty() || line.starts_with('*') { continue; }

        // Directives
        if line.starts_with('.') {
            let upper = line.to_uppercase();
            if upper == ".OP" || upper.starts_with(".OP ") {
                analysis = Some(Analysis::Op);
            } else if upper.starts_with(".TRAN") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    let step = parse_value(parts[1])?;
                    let stop = parse_value(parts[2])?;
                    let uic = parts.iter().any(|p| p.eq_ignore_ascii_case("UIC"));
                    analysis = Some(Analysis::Tran { step, stop, uic });
                } else {
                    return Err(format!("Invalid .TRAN: {line}"));
                }
            } else if upper.starts_with(".DC ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    let src1 = parts[1].to_string();
                    let start1 = parse_value(parts[2])?;
                    let stop1 = parse_value(parts[3])?;
                    let step1 = parse_value(parts[4])?;
                    let (src2, start2, stop2, step2) = if parts.len() >= 9 {
                        (
                            Some(parts[5].to_string()),
                            parse_value(parts[6])?,
                            parse_value(parts[7])?,
                            parse_value(parts[8])?,
                        )
                    } else {
                        (None, 0.0, 0.0, 0.0)
                    };
                    analysis = Some(Analysis::DcSweep {
                        src1, start1, stop1, step1,
                        src2, start2, stop2, step2,
                    });
                } else {
                    return Err(format!("Invalid .DC: {line}"));
                }
            } else if upper.starts_with(".AC ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    let sweep_type = match parts[1].to_uppercase().as_str() {
                        "DEC" => AcSweepType::Dec,
                        "OCT" => AcSweepType::Oct,
                        "LIN" => AcSweepType::Lin,
                        _ => return Err(format!("Invalid .AC sweep type '{}': {line}", parts[1])),
                    };
                    let num_points = parse_value(parts[2])? as usize;
                    let fstart = parse_value(parts[3])?;
                    let fstop = parse_value(parts[4])?;
                    analysis = Some(Analysis::Ac { sweep_type, num_points, fstart, fstop });
                } else {
                    return Err(format!("Invalid .AC: {line}"));
                }
            } else if upper.starts_with(".TF ") {
                // .TF V(node1[,node2]) input_src
                // .TF I(outsrc) input_src
                let output_and_src = parse_tf_output_and_src(line)?;
                if let Some((output, input_src)) = output_and_src {
                    analysis = Some(Analysis::Tf { output, input_src });
                }
            } else if upper.starts_with(".SENS ") {
                // .SENS V(node1[,node2]) or .SENS I(src)
                let output = parse_sens_output(line)?;
                if let Some(output) = output {
                    analysis = Some(Analysis::Sens { output });
                }
            } else if upper.starts_with(".PZ ") || upper.starts_with(".PZ\t") {
                // .PZ node1 node2 node3 node4 VOL|CUR PZ|POL|ZER
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 7 {
                    let in_pos = parts[1].to_string();
                    let in_neg = parts[2].to_string();
                    let out_pos = parts[3].to_string();
                    let out_neg = parts[4].to_string();
                    let input_type = match parts[5].to_uppercase().as_str() {
                        "VOL" => PzInputType::Vol,
                        "CUR" => PzInputType::Cur,
                        _ => return Err(format!("Invalid .PZ input type '{}': {line}", parts[5])),
                    };
                    let pz_type = match parts[6].to_uppercase().as_str() {
                        "PZ" => PzAnalysisType::Both,
                        "POL" => PzAnalysisType::Poles,
                        "ZER" => PzAnalysisType::Zeros,
                        _ => return Err(format!("Invalid .PZ analysis type '{}': {line}", parts[6])),
                    };
                    analysis = Some(Analysis::Pz {
                        in_pos, in_neg, out_pos, out_neg,
                        input_type, pz_type,
                    });
                } else {
                    return Err(format!("Invalid .PZ: {line}"));
                }
            } else if upper.starts_with(".IC ") {
                let rest = &line[3..];
                for token in rest.split_whitespace() {
                    let token_upper = token.to_uppercase();
                    if token_upper.starts_with("V(") {
                        if let Some(eq) = token.find('=') {
                            let node_spec = &token[2..eq];
                            let node_name = node_spec.trim_end_matches(')');
                            let val = parse_value(&token[eq+1..]).unwrap_or(0.0);
                            ic_nodes.push((node_name.to_string(), val));
                        }
                    }
                }
            } else if upper.starts_with(".NODESET") {
                // .NODESET V(node)=value — parse same format as .IC
                let rest = &line[8..];
                for token in rest.split_whitespace() {
                    let token_upper = token.to_uppercase();
                    if token_upper.starts_with("V(") {
                        if let Some(eq) = token.find('=') {
                            let node_spec = &token[2..eq];
                            let node_name = node_spec.trim_end_matches(')');
                            let val = parse_value(&token[eq+1..]).unwrap_or(0.0);
                            nodeset_nodes.push((node_name.to_string(), val));
                        }
                    }
                }
            } else if upper.starts_with(".TEMP") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    temp_celsius = Some(parse_value(parts[1]).unwrap_or(27.0));
                }
            } else if upper.starts_with(".OPT") {
                // Parse .OPTIONS key=value pairs
                for token in line.split_whitespace().skip(1) {
                    let token_upper = token.to_uppercase();
                    if let Some(eq) = token_upper.find('=') {
                        let key = &token_upper[..eq];
                        let val_str = &token[eq+1..];
                        if let Ok(val) = parse_value(val_str) {
                            match key {
                                "TNOM" => tnom_celsius = Some(val),
                                "ABSTOL" => opt_abstol = Some(val),
                                "VNTOL" => opt_vntol = Some(val),
                                "RELTOL" => opt_reltol = Some(val),
                                _ => {} // ignore other options
                            }
                        }
                    }
                }
            } else if upper.starts_with(".MODEL") {
                // Already parsed in pass 1
            } else if upper.starts_with(".END") {
                break;
            } else if upper.starts_with('.') {
                // Warn on unrecognized dot-directives so parser bugs don't hide silently
                let directive = upper.split_whitespace().next().unwrap_or(&upper);
                // Skip known directives that we intentionally ignore
                if !matches!(directive, ".TITLE" | ".GLOBAL" | ".PRINT" | ".PLOT" | ".PROBE"
                    | ".WIDTH" | ".SAVE" | ".CONTROL" | ".ENDC" | ".INCLUDE" | ".LIB"
                    | ".PARAM" | ".FUNC" | ".MEASURE" | ".MEAS" | ".NOISE" | ".DISTO"
                    | ".FOUR" | ".STEP") {
                    eprintln!("WARNING: unrecognized directive '{}' — line ignored", directive);
                }
            }
            continue;
        }

        // Component lines — collect specs, don't create branches/internal nodes
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() { continue; }

        let name = parts[0];
        let first_char = name.chars().next().unwrap().to_ascii_uppercase();

        match first_char {
            'R' => {
                if parts.len() < 4 { return Err(format!("Invalid resistor: {line}")); }
                let n1 = circuit.node(parts[1]);
                let n2 = circuit.node(parts[2]);
                let value = parse_value(parts[3])?;
                // Look for ac=<value> parameter (ngspice RES_ACRESIST)
                let ac_value = parts[4..].iter()
                    .find(|p| p.to_uppercase().starts_with("AC="))
                    .map(|p| parse_value(&p[3..]))
                    .transpose()?;
                specs.push(DeviceSpec::Resistor { name: name.to_string(), n1, n2, value, ac_value });
            }
            'C' => {
                if parts.len() < 4 { return Err(format!("Invalid capacitor: {line}")); }
                let n1 = circuit.node(parts[1]);
                let n2 = circuit.node(parts[2]);
                let value = parse_value(parts[3])?;
                specs.push(DeviceSpec::Capacitor { name: name.to_string(), n1, n2, value });
            }
            'L' => {
                if parts.len() < 4 { return Err(format!("Invalid inductor: {line}")); }
                let n1 = circuit.node(parts[1]);
                let n2 = circuit.node(parts[2]);
                let value = parse_value(parts[3])?;
                let ic = parts[4..].iter()
                    .find(|p| p.to_uppercase().starts_with("IC="))
                    .and_then(|p| parse_value(&p[3..]).ok());
                specs.push(DeviceSpec::Inductor { name: name.to_string(), n1, n2, value, ic });
            }
            'V' => {
                if parts.len() < 3 { return Err(format!("Invalid voltage source: {line}")); }
                let n1 = circuit.node(parts[1]);
                let n2 = circuit.node(parts[2]);
                let rest = &parts[3..];
                let (waveform, ac) = parse_source_spec(rest, line)?;
                specs.push(DeviceSpec::VoltageSource { name: name.to_string(), n1, n2, waveform, ac });
            }
            'I' => {
                if parts.len() < 3 { return Err(format!("Invalid current source: {line}")); }
                // ngspice struct order: ISRCnegNode (first netlist node),
                // ISRCposNode (second netlist node). Load stamps RHS[pos] += I,
                // RHS[neg] -= I. Our CurrentSource(pos, neg) expects pos first,
                // so swap: n1(netlist)→neg, n2(netlist)→pos.
                let n1 = circuit.node(parts[2]); // pos = second netlist node
                let n2 = circuit.node(parts[1]); // neg = first netlist node
                let rest = &parts[3..];
                let (waveform, ac) = parse_source_spec(rest, line)?;
                specs.push(DeviceSpec::CurrentSource { name: name.to_string(), n1, n2, waveform, ac });
            }
            'E' => {
                if parts.len() < 6 { return Err(format!("Invalid VCVS: {line}")); }
                let n1 = circuit.node(parts[1]);
                let n2 = circuit.node(parts[2]);
                let cp = circuit.node(parts[3]);
                let cn = circuit.node(parts[4]);
                let gain = parse_value(parts[5])?;
                specs.push(DeviceSpec::Vcvs { name: name.to_string(), n1, n2, cp, cn, gain });
            }
            'G' => {
                if parts.len() < 4 { return Err(format!("Invalid VCCS: {line}")); }
                let n1 = circuit.node(parts[1]);
                let n2 = circuit.node(parts[2]);

                // Check for POLY
                let p3_upper = parts[3].to_uppercase();
                if p3_upper.starts_with("POLY") {
                    // G name n+ n- POLY(dim) nc1+ nc1- [nc2+ nc2-...] c0 c1 [c2...]
                    let dim_str = p3_upper.trim_start_matches("POLY");
                    let dim_str = dim_str.trim_start_matches('(').trim_end_matches(')');
                    let dim: usize = dim_str.parse().map_err(|_| format!("Invalid POLY dimension: {line}"))?;

                    // Parse control node pairs
                    let ctrl_start = 4;
                    let mut ctrl_pairs: Vec<(usize, usize)> = Vec::new();
                    for j in 0..dim {
                        let cp_idx = ctrl_start + j * 2;
                        let cn_idx = ctrl_start + j * 2 + 1;
                        if cn_idx >= parts.len() {
                            return Err(format!("POLY({dim}) needs {dim} control pairs: {line}"));
                        }
                        let cp = circuit.node(parts[cp_idx]);
                        let cn = circuit.node(parts[cn_idx]);
                        ctrl_pairs.push((cp, cn));
                    }

                    // Parse coefficients
                    let coeff_start = ctrl_start + dim * 2;
                    let mut coeffs: Vec<f64> = Vec::new();
                    for j in coeff_start..parts.len() {
                        coeffs.push(parse_value(parts[j])?);
                    }

                    // Expand POLY into individual VCCS devices.
                    // For linear POLY: I = c0 + c1*V1 + c2*V2 + ...
                    // c0 → DC current source (if non-zero)
                    // c1..cdim → VCCS with corresponding control pair
                    let c0 = coeffs.first().copied().unwrap_or(0.0);
                    if c0 != 0.0 {
                        // Add a DC current source for the constant term
                        specs.push(DeviceSpec::CurrentSource {
                            name: format!("{name}#poly_dc"),
                            n1, n2,
                            waveform: Waveform::Dc(c0),
                            ac: None,
                        });
                    }

                    for (j, &(cp, cn)) in ctrl_pairs.iter().enumerate() {
                        let gm = coeffs.get(j + 1).copied().unwrap_or(0.0);
                        if gm != 0.0 {
                            specs.push(DeviceSpec::Vccs {
                                name: format!("{name}#poly_{j}"),
                                n1, n2, cp, cn, gm,
                            });
                        }
                    }
                } else {
                    // Standard VCCS: G name n+ n- nc+ nc- gm
                    if parts.len() < 6 { return Err(format!("Invalid VCCS: {line}")); }
                    let cp = circuit.node(parts[3]);
                    let cn = circuit.node(parts[4]);
                    let gm = parse_value(parts[5])?;
                    specs.push(DeviceSpec::Vccs { name: name.to_string(), n1, n2, cp, cn, gm });
                }
            }
            'F' => {
                if parts.len() < 5 { return Err(format!("Invalid CCCS: {line}")); }
                let n1 = circuit.node(parts[1]);
                let n2 = circuit.node(parts[2]);
                let ctrl_name = parts[3].to_string();
                let gain = parse_value(parts[4])?;
                specs.push(DeviceSpec::Cccs { name: name.to_string(), n1, n2, ctrl_name, gain });
            }
            'H' => {
                if parts.len() < 5 { return Err(format!("Invalid CCVS: {line}")); }
                let n1 = circuit.node(parts[1]);
                let n2 = circuit.node(parts[2]);
                let ctrl_name = parts[3].to_string();
                let tr = parse_value(parts[4])?;
                specs.push(DeviceSpec::Ccvs { name: name.to_string(), n1, n2, ctrl_name, tr });
            }
            'J' => {
                // J1 drain gate source model [area]
                if parts.len() < 5 { return Err(format!("Invalid JFET: {line}")); }
                let nd = circuit.node(parts[1]);
                let ng = circuit.node(parts[2]);
                let ns = circuit.node(parts[3]);
                let jfet_model_name = parts[4].to_uppercase();
                let area = if parts.len() > 5 { parse_value(parts[5]).unwrap_or(1.0) } else { 1.0 };
                let model = jfet_models.get(&jfet_model_name).cloned().unwrap_or_default();
                specs.push(DeviceSpec::Jfet { name: name.to_string(), nd, ng, ns, model, area, model_name: jfet_model_name });
            }
            'D' => {
                if parts.len() < 4 { return Err(format!("Invalid diode: {line}")); }
                let n_anode = circuit.node(parts[1]);
                let n_cathode = circuit.node(parts[2]);
                let diode_model_name = parts[3].to_uppercase();
                let area = if parts.len() > 4 { parse_value(parts[4]).unwrap_or(1.0) } else { 1.0 };
                let model = diode_models.get(&diode_model_name).cloned().unwrap_or_default();
                specs.push(DeviceSpec::Diode { name: name.to_string(), n_anode, n_cathode, model, area, model_name: diode_model_name });
            }
            'Q' => {
                // Q1 collector base emitter [substrate] model [area]
                if parts.len() < 5 { return Err(format!("Invalid BJT: {line}")); }
                let nc = circuit.node(parts[1]);
                let nb = circuit.node(parts[2]);
                let ne = circuit.node(parts[3]);
                // Check if part 4 is a model name or substrate node
                let (ns, bjt_model_name, area_idx) = if parts.len() >= 6 {
                    // Could be: Q1 c b e s model [area] OR Q1 c b e model area
                    let p4_upper = parts[4].to_uppercase();
                    if bjt_models.contains_key(&p4_upper) {
                        // Q1 c b e model [area]
                        (0, p4_upper, 5)
                    } else {
                        // Q1 c b e substrate model [area]
                        (circuit.node(parts[4]), parts[5].to_uppercase(), 6)
                    }
                } else {
                    // Q1 c b e model
                    (0, parts[4].to_uppercase(), 5)
                };
                let mut area = 1.0;
                let mut off = false;
                // Parse remaining tokens: could be area value and/or "off"
                for i in area_idx..parts.len() {
                    if parts[i].eq_ignore_ascii_case("off") {
                        off = true;
                    } else if let Ok(v) = parse_value(parts[i]) {
                        area = v;
                    }
                }
                let model = bjt_models.get(&bjt_model_name).cloned().unwrap_or_default();
                specs.push(DeviceSpec::Bjt { name: name.to_string(), nc, nb, ne, ns, model, area, off, model_name: bjt_model_name });
            }
            'M' => {
                // M1 drain gate source bulk model [W=val] [L=val] [M=val]
                if parts.len() < 6 { return Err(format!("Invalid MOSFET: {line}")); }
                let nd = circuit.node(parts[1]);
                let ng = circuit.node(parts[2]);
                let ns = circuit.node(parts[3]);
                let nb = circuit.node(parts[4]);
                let mos_model_name = parts[5].to_uppercase();
                // Parse W=, L=, M= from remaining tokens
                let mut w = 100e-6; // default 100um
                let mut l = 100e-6;
                let mut m_val = 1.0;
                for p in &parts[6..] {
                    let pu = p.to_uppercase();
                    if pu.starts_with("W=") { w = parse_value(&p[2..]).unwrap_or(w); }
                    else if pu.starts_with("L=") { l = parse_value(&p[2..]).unwrap_or(l); }
                    else if pu.starts_with("M=") { m_val = parse_value(&p[2..]).unwrap_or(m_val); }
                }
                // Route to Level 1, 2, 3, BSIM3, or BSIM4 based on which model map contains the name
                if let Some(model) = bsim4_models.get(&mos_model_name).cloned() {
                    specs.push(DeviceSpec::Bsim4 { name: name.to_string(), nd, ng, ns, nb, model, w, l, m: m_val, model_name: mos_model_name });
                } else if let Some(model) = bsim3_models.get(&mos_model_name).cloned() {
                    specs.push(DeviceSpec::Bsim3 { name: name.to_string(), nd, ng, ns, nb, model, w, l, m: m_val, model_name: mos_model_name });
                } else if let Some(model) = mos3_models.get(&mos_model_name).cloned() {
                    specs.push(DeviceSpec::Mosfet3 { name: name.to_string(), nd, ng, ns, nb, model, w, l, m: m_val, model_name: mos_model_name });
                } else if let Some(model) = mos2_models.get(&mos_model_name).cloned() {
                    specs.push(DeviceSpec::Mosfet2 { name: name.to_string(), nd, ng, ns, nb, model, w, l, m: m_val, model_name: mos_model_name });
                } else {
                    let model = mos_models.get(&mos_model_name).cloned().unwrap_or_default();
                    specs.push(DeviceSpec::Mosfet { name: name.to_string(), nd, ng, ns, nb, model, w, l, m: m_val, model_name: mos_model_name });
                }
            }
            'T' => {
                // Transmission line: T1 port1+ port1- port2+ port2- Z0=val TD=val [F=val NL=val]
                if parts.len() < 5 { return Err(format!("Invalid transmission line: {line}")); }
                let p1 = circuit.node(parts[1]);
                let n1 = circuit.node(parts[2]);
                let p2 = circuit.node(parts[3]);
                let n2 = circuit.node(parts[4]);
                let mut z0 = 0.0;
                let mut td = 0.0;
                let mut td_given = false;
                let mut nl = 0.25;  // trasetup.c default
                let mut freq = 1e9; // trasetup.c default
                for p in &parts[5..] {
                    let pu = p.to_uppercase();
                    if pu.starts_with("Z0=") || pu.starts_with("ZO=") {
                        z0 = parse_value(&p[3..])?;
                    } else if pu.starts_with("TD=") {
                        td = parse_value(&p[3..])?;
                        td_given = true;
                    } else if pu.starts_with("F=") {
                        freq = parse_value(&p[2..])?;
                    } else if pu.starts_with("NL=") {
                        nl = parse_value(&p[3..])?;
                    }
                }
                if z0 <= 0.0 {
                    return Err(format!("Transmission line Z0 must be given: {line}"));
                }
                specs.push(DeviceSpec::TLine { name: name.to_string(), p1, n1, p2, n2, z0, td, td_given, nl, freq });
            }
            'K' => {
                // Coupled inductor: K1 L1 L2 coupling_value
                if parts.len() < 4 { return Err(format!("Invalid coupled inductor: {line}")); }
                let l1_name = parts[1].to_string();
                let l2_name = parts[2].to_string();
                let coupling = parse_value(parts[3])?;
                k_specs.push((name.to_string(), l1_name, l2_name, coupling));
            }
            _ => {
                return Err(format!("Unsupported device type '{first_char}': {line}"));
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Device instantiation in ngspice DEVices[] type order.
    //
    // ngspice CKTsetup iterates DEVices[0..DEVmaxnum] and calls each
    // device type's setup function in order. Within each type, ngspice
    // has TWO levels of PREPEND:
    //
    // 1. Models: CKTmodCrt (cktmcrt.c:38-39) PREPENDs each new model
    //    to CKThead[type]. Models are created lazily when the first
    //    device referencing them is parsed (INP2Q → INPgetMod →
    //    create_model → CKTmodCrt). So the model whose first device
    //    appears LAST in the netlist ends up at the HEAD of the list.
    //
    // 2. Instances: CKTcrtElt (cktcrte.c:62-64) PREPENDs each instance
    //    to its model's instance list. So the LAST-parsed instance
    //    under a given model is processed FIRST.
    //
    // The setup loop iterates: for each model (head→tail), for each
    // instance (head→tail). This produces a specific ordering that
    // determines equation numbering via CKTmkVolt/CKTmkCur.
    //
    // For device types without models (R, C, L, V, I, E, F, G, H, T),
    // all instances share a single implicit model, so simple reversal
    // (instance PREPEND) is sufficient.
    // ═══════════════════════════════════════════════════════════════
    specs.sort_by_key(|s| s.type_order());
    // Apply ngspice two-level prepend ordering within each type group
    {
        let mut i = 0;
        while i < specs.len() {
            let order = specs[i].type_order();
            let mut j = i + 1;
            while j < specs.len() && specs[j].type_order() == order {
                j += 1;
            }
            // Check if any spec in this type group has a model_name
            let has_models = specs[i..j].iter().any(|s| s.model_name().is_some());
            if has_models {
                // Two-level prepend: group by model, then reverse within each model group.
                // Models ordered by reverse-first-device-reference (PREPEND on model list).
                //
                // Step 1: Discover model order by scanning specs[i..j] in parse order.
                // The first device referencing a model creates it via CKTmodCrt (PREPEND).
                // So model order is: last-first-referenced model at HEAD.
                let mut model_first_seen: Vec<String> = Vec::new();
                for s in &specs[i..j] {
                    if let Some(mn) = s.model_name() {
                        if !model_first_seen.contains(&mn.to_string()) {
                            model_first_seen.push(mn.to_string());
                        }
                    }
                }
                // model_first_seen is in order of first reference (parse order).
                // After PREPEND, the model iteration order is reversed.
                model_first_seen.reverse();

                // Step 2: For each model, collect its instances and reverse them (PREPEND).
                let mut reordered: Vec<DeviceSpec> = Vec::with_capacity(j - i);
                // Collect specs by index, grouping by model with correct ordering.
                let mut remaining: Vec<usize> = (i..j).collect();
                for mn in &model_first_seen {
                    // Collect indices of specs matching this model, in original order
                    let mut model_indices: Vec<usize> = Vec::new();
                    remaining.retain(|&idx| {
                        if specs[idx].model_name().map(|s| s == mn.as_str()).unwrap_or(false) {
                            model_indices.push(idx);
                            false
                        } else {
                            true
                        }
                    });
                    // Reverse for instance PREPEND
                    model_indices.reverse();
                    for &idx in &model_indices {
                        reordered.push(std::mem::replace(&mut specs[idx],
                            DeviceSpec::Resistor { name: String::new(), n1: 0, n2: 0, value: 0.0, ac_value: None }));
                    }
                }
                // Any remaining specs (shouldn't happen, but safety)
                for &idx in &remaining {
                    reordered.push(std::mem::replace(&mut specs[idx],
                        DeviceSpec::Resistor { name: String::new(), n1: 0, n2: 0, value: 0.0, ac_value: None }));
                }
                // Put reordered specs back
                for (k, spec) in reordered.into_iter().enumerate() {
                    specs[i + k] = spec;
                }
            } else {
                // No models — simple instance PREPEND (reverse within type)
                specs[i..j].reverse();
            }
            i = j;
        }
    }

    // Build source name lookup for CKTfndBranch (CCCS/CCVS need to find
    // VSRC/IND branches that may not have been created yet).
    let source_names: HashMap<String, String> = specs.iter()
        .filter_map(|s| match s {
            DeviceSpec::VoltageSource { name, .. } |
            DeviceSpec::Inductor { name, .. } => Some((name.to_uppercase(), name.clone())),
            _ => None,
        })
        .collect();

    let mut branch_map: HashMap<String, usize> = HashMap::new();
    // Track inductor names for K element validation (actual resolution
    // happens later in resolve_coupled_inductors after circuit.setup()).
    let mut ind_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for spec in specs {
        match spec {
            DeviceSpec::Resistor { name, n1, n2, value, ac_value } => {
                circuit.add_device(Box::new(Resistor::new(&name, n1, n2, value, ac_value)));
            }
            DeviceSpec::Capacitor { name, n1, n2, value } => {
                circuit.add_device(Box::new(Capacitor::new(&name, n1, n2, value)));
            }
            DeviceSpec::Cccs { name, n1, n2, ctrl_name, gain } => {
                let cb = find_control_branch(
                    &mut circuit, &mut branch_map, &source_names, &ctrl_name, &name)?;
                circuit.add_device(Box::new(Cccs::new(&name, n1, n2, cb, gain)));
            }
            DeviceSpec::Ccvs { name, n1, n2, ctrl_name, tr } => {
                let cb = find_control_branch(
                    &mut circuit, &mut branch_map, &source_names, &ctrl_name, &name)?;
                let br = get_or_create_branch(&mut circuit, &mut branch_map, &name);
                circuit.add_device(Box::new(Ccvs::new(&name, n1, n2, br, cb, tr)));
            }
            DeviceSpec::Diode { name, n_anode, n_cathode, model, area, .. } => {
                // ngspice diosetup: CKTmkVolt(ckt, &tmp, name, "internal")
                let pos_prime = if model.rs > 0.0 {
                    circuit.node(&format!("{name}#internal"))
                } else {
                    n_anode
                };
                circuit.add_device(Box::new(
                    crate::device::diode::Diode::new(&name, n_anode, n_cathode, pos_prime, model, area)
                ));
            }
            DeviceSpec::Inductor { name, n1, n2, value, ic } => {
                let br = get_or_create_branch(&mut circuit, &mut branch_map, &name);
                let mut ind = Inductor::new(&name, n1, n2, br, value);
                if let Some(ic_val) = ic {
                    ind = ind.with_ic(ic_val);
                }
                ind_names.insert(name.to_uppercase());
                circuit.add_device(Box::new(ind));
            }
            DeviceSpec::CurrentSource { name, n1, n2, waveform, ac } => {
                let mut src = CurrentSource::new(&name, n1, n2, waveform);
                if let Some(ac_p) = ac {
                    src.ac_mag = ac_p.mag;
                    src.ac_phase_deg = ac_p.phase_deg;
                }
                circuit.add_device(Box::new(src));
            }
            DeviceSpec::Vccs { name, n1, n2, cp, cn, gm } => {
                circuit.add_device(Box::new(Vccs::new(&name, n1, n2, cp, cn, gm)));
            }
            DeviceSpec::Vcvs { name, n1, n2, cp, cn, gain } => {
                let br = get_or_create_branch(&mut circuit, &mut branch_map, &name);
                circuit.add_device(Box::new(Vcvs::new(&name, n1, n2, cp, cn, br, gain)));
            }
            DeviceSpec::VoltageSource { name, n1, n2, waveform, ac } => {
                let br = get_or_create_branch(&mut circuit, &mut branch_map, &name);
                let mut src = VoltageSource::with_waveform(&name, n1, n2, br, waveform);
                if let Some(ac_p) = ac {
                    src.ac_mag = ac_p.mag;
                    src.ac_phase_deg = ac_p.phase_deg;
                }
                circuit.add_device(Box::new(src));
            }
            DeviceSpec::Bjt { name, nc, nb, ne, ns, model, area, off, .. } => {
                let has_rc = model.rc > 0.0;
                let has_rb = model.rb > 0.0;
                let has_re = model.re > 0.0;
                let cp = if has_rc { circuit.node(&format!("{name}#collector")) } else { nc };
                let bp = if has_rb { circuit.node(&format!("{name}#base")) } else { nb };
                let ep = if has_re { circuit.node(&format!("{name}#emitter")) } else { ne };
                let mut bjt = crate::device::bjt::Bjt::new(&name, nc, nb, ne, ns, model, area);
                bjt.set_internal_nodes(cp, bp, ep);
                bjt.off = off;
                circuit.add_device(Box::new(bjt));
            }
            DeviceSpec::Jfet { name, nd, ng, ns, model, area, .. } => {
                // Create internal drain/source nodes if parasitic R exists (jfetset.c:130-173)
                // ngspice order: source_prime first, then drain_prime
                let sp = if model.rs != 0.0 {
                    circuit.node(&format!("{name}#source"))
                } else {
                    ns
                };
                let dp = if model.rd != 0.0 {
                    circuit.node(&format!("{name}#drain"))
                } else {
                    nd
                };
                circuit.add_device(Box::new(
                    crate::device::jfet::Jfet::new(&name, nd, ng, ns, dp, sp, model, area)
                ));
            }
            DeviceSpec::Mosfet { name, nd, ng, ns, nb, model, w, l, m, .. } => {
                // Create internal drain/source nodes if parasitic R exists (mos1set.c:137-185)
                let has_rd = model.rd > 0.0 || (model.rsh > 0.0);
                let has_rs = model.rs > 0.0 || (model.rsh > 0.0);
                let dp = if has_rd { circuit.node(&format!("{name}#drain")) } else { nd };
                let sp = if has_rs { circuit.node(&format!("{name}#source")) } else { ns };
                let mut mos = crate::device::mosfet1::Mosfet1::new(&name, nd, ng, ns, nb, model, w, l, m);
                mos.set_internal_nodes(dp, sp);
                circuit.add_device(Box::new(mos));
            }
            DeviceSpec::Mosfet2 { name, nd, ng, ns, nb, model, w, l, m, .. } => {
                // Create internal drain/source nodes if parasitic R exists (mos2set.c)
                let has_rd = model.rd > 0.0 || (model.rsh > 0.0);
                let has_rs = model.rs > 0.0 || (model.rsh > 0.0);
                let dp = if has_rd { circuit.node(&format!("{name}#drain")) } else { nd };
                let sp = if has_rs { circuit.node(&format!("{name}#source")) } else { ns };
                let mut mos = crate::device::mosfet2::Mosfet2::new(&name, nd, ng, ns, nb, model, w, l, m);
                mos.set_internal_nodes(dp, sp);
                circuit.add_device(Box::new(mos));
            }
            DeviceSpec::Mosfet3 { name, nd, ng, ns, nb, model, w, l, m, .. } => {
                // Create internal drain/source nodes if parasitic R exists (mos3set.c)
                let has_rd = model.rd > 0.0 || (model.rsh > 0.0);
                let has_rs = model.rs > 0.0 || (model.rsh > 0.0);
                let dp = if has_rd { circuit.node(&format!("{name}#drain")) } else { nd };
                let sp = if has_rs { circuit.node(&format!("{name}#source")) } else { ns };
                let mut mos = crate::device::mosfet3::Mosfet3::new(&name, nd, ng, ns, nb, model, w, l, m);
                mos.set_internal_nodes(dp, sp);
                circuit.add_device(Box::new(mos));
            }
            DeviceSpec::Bsim3 { name, nd, ng, ns, nb, model, w, l, m, .. } => {
                // Create internal drain/source nodes if parasitic R (b3set.c)
                let drain_r = model.sheet_resistance; // drainSquares default=1
                let source_r = model.sheet_resistance;
                let dp = if drain_r > 0.0 { circuit.node(&format!("{name}#drain")) } else { nd };
                let sp = if source_r > 0.0 { circuit.node(&format!("{name}#source")) } else { ns };
                let mut mos = crate::device::bsim3::Bsim3::new(&name, nd, ng, ns, nb, model, w, l, m);
                mos.set_internal_nodes(dp, sp);
                circuit.add_device(Box::new(mos));
            }
            DeviceSpec::Bsim4 { name, nd, ng, ns, nb, model, w, l, m, .. } => {
                // Create internal drain/source nodes if parasitic R (b4set.c)
                let drain_r = model.sheet_resistance;
                let source_r = model.sheet_resistance;
                let dp = if drain_r > 0.0 { circuit.node(&format!("{name}#drain")) } else { nd };
                let sp = if source_r > 0.0 { circuit.node(&format!("{name}#source")) } else { ns };
                let mut mos = crate::device::bsim4::Bsim4::new(&name, nd, ng, ns, nb, model, w, l, m);
                mos.set_internal_nodes(dp, sp);
                circuit.add_device(Box::new(mos));
            }
            DeviceSpec::TLine { name, p1, n1, p2, n2, z0, td, td_given, nl, freq } => {
                // trasetup.c: create 2 branch equations and 2 internal nodes
                // Order: brEq1, brEq2, intNode1, intNode2
                let br1 = circuit.branch(&format!("{name}#i1"));
                let br2 = circuit.branch(&format!("{name}#i2"));
                let int1 = circuit.node(&format!("{name}#int1"));
                let int2 = circuit.node(&format!("{name}#int2"));
                circuit.add_device(Box::new(
                    crate::device::tline::TransmissionLine::new(
                        &name, p1, n1, p2, n2, int1, int2, br1, br2,
                        z0, td, td_given, nl, freq,
                    )
                ));
            }
        }
    }

    let analysis = analysis.ok_or_else(|| "No analysis directive found (.OP or .TRAN)".to_string())?;

    // Convert K specs into CoupledInductorSpec with inductor metadata.
    // At this point inductors are created but state offsets aren't allocated yet
    // (that happens in circuit.setup()). We store branch/inductance/IC from ind_info
    // and resolve flux offsets later in the runner.
    let coupled_specs: Vec<CoupledInductorSpec> = k_specs.into_iter().map(|(name, l1, l2, coupling)| {
        CoupledInductorSpec {
            name,
            ind1_name: l1.to_uppercase(),
            ind2_name: l2.to_uppercase(),
            coupling,
        }
    }).collect();

    // Validate that referenced inductors exist
    for spec in &coupled_specs {
        if !ind_names.contains(&spec.ind1_name) {
            return Err(format!("{}: coupling to non-existent inductor {}", spec.name, spec.ind1_name));
        }
        if !ind_names.contains(&spec.ind2_name) {
            return Err(format!("{}: coupling to non-existent inductor {}", spec.name, spec.ind2_name));
        }
    }

    Ok(ParseResult {
        circuit,
        analysis,
        title,
        temp: temp_celsius,
        tnom: tnom_celsius,
        ic_nodes,
        nodeset_nodes,
        k_specs: coupled_specs,
        abstol: opt_abstol,
        vntol: opt_vntol,
        reltol: opt_reltol,
    })
}

/// Resolve K (coupled inductor) specs into MutualInductor devices.
///
/// Must be called AFTER circuit.setup() so that inductor state offsets are valid.
/// Matches ngspice MUTsetup (mutsetup.c) — finds referenced inductors and
/// allocates matrix elements. After this, mutual inductors are in circuit.devices
/// with type_order 30 (after inductors at 29).
pub fn resolve_coupled_inductors(circuit: &mut Circuit, k_specs: &[CoupledInductorSpec]) -> Result<(), String> {
    use crate::device::Device;
    use crate::device::inductor::Inductor;

    if k_specs.is_empty() {
        return Ok(());
    }

    // Build inductor lookup: name → (branch, inductance, flux_offset, ic)
    struct IndMeta {
        branch: usize,
        inductance: f64,
        flux_offset: usize,
        ic: Option<f64>,
    }
    let mut ind_meta: HashMap<String, IndMeta> = HashMap::new();

    for device in &circuit.devices {
        if let Some(ind) = device.as_any().downcast_ref::<Inductor>() {
            ind_meta.insert(ind.name().to_uppercase(), IndMeta {
                branch: ind.branch_eq(),
                inductance: ind.inductance(),
                flux_offset: ind.flux_offset(),
                ic: ind.ic(),
            });
        }
    }

    // Create MutualInductor devices (ngspice type order 30, after IND at 29)
    for spec in k_specs {
        let ind1 = ind_meta.get(&spec.ind1_name)
            .ok_or_else(|| format!("{}: coupling to non-existent inductor {}", spec.name, spec.ind1_name))?;
        let ind2 = ind_meta.get(&spec.ind2_name)
            .ok_or_else(|| format!("{}: coupling to non-existent inductor {}", spec.name, spec.ind2_name))?;

        let mut_ind = MutualInductor::new(
            &spec.name,
            spec.coupling,
            ind1.inductance,
            ind2.inductance,
            ind1.branch,
            ind2.branch,
            ind1.flux_offset,
            ind2.flux_offset,
            ind1.ic,
            ind2.ic,
        );
        circuit.add_device(Box::new(mut_ind));
    }

    Ok(())
}

/// Parse source specification from tokens after node names.
///
/// Handles: `DC 5`, `PULSE(0 3.3 10n ...)`, `SIN(0 1 1MEG)`, `PWL(0 0 10n 5 ...)`,
/// `DC 0 AC 1`, bare `5`, etc.
fn parse_source_spec(tokens: &[&str], _full_line: &str) -> Result<(Waveform, Option<AcParams>), String> {
    if tokens.is_empty() {
        return Ok((Waveform::Dc(0.0), None));
    }

    // Join tokens back and look for waveform keywords in the full remaining text.
    // We need to handle cases like "PULSE(0 3.3 10n 5n)" where parens may be
    // attached to the keyword or separated.
    let rest = tokens.join(" ");
    let rest_upper = rest.to_uppercase();

    // Parse AC params from the token stream (can coexist with transient waveform).
    let ac = parse_ac_params(tokens);

    // Check for PULSE
    if let Some(pos) = rest_upper.find("PULSE") {
        let after = &rest[pos + 5..];
        let args = extract_paren_args(after)?;
        return Ok((parse_pulse(&args)?, ac));
    }

    // Check for SIN
    if let Some(pos) = rest_upper.find("SIN") {
        // Make sure it's not just "SENSE" or similar
        let next_char = rest_upper.as_bytes().get(pos + 3).copied().unwrap_or(b' ');
        if next_char == b'(' || next_char == b' ' {
            let after = &rest[pos + 3..];
            let args = extract_paren_args(after)?;
            return Ok((parse_sin(&args)?, ac));
        }
    }

    // Check for PWL
    if let Some(pos) = rest_upper.find("PWL") {
        let after = &rest[pos + 3..];
        let args = extract_paren_args(after)?;
        return Ok((parse_pwl(&args)?, ac));
    }

    // No waveform keyword — parse DC value
    let mut dc_val = 0.0;
    let mut i = 0;
    while i < tokens.len() {
        let t = tokens[i];
        if t.eq_ignore_ascii_case("DC") {
            i += 1;
            if i < tokens.len() {
                dc_val = parse_value(tokens[i]).unwrap_or(0.0);
            }
            i += 1;
        } else if t.eq_ignore_ascii_case("AC") {
            // Skip AC spec (already parsed above)
            i += 1;
            if i < tokens.len() { let _ = parse_value(tokens[i]); i += 1; } // skip mag
            if i < tokens.len() { if parse_value(tokens[i]).is_ok() { i += 1; } } // skip optional phase
        } else {
            // Bare value
            if let Ok(v) = parse_value(t) {
                dc_val = v;
            }
            i += 1;
        }
    }

    Ok((Waveform::Dc(dc_val), ac))
}

/// Parse AC mag [phase] from a source token stream.
/// Handles: `AC 1`, `AC 1 0`, `AC 1.5 45` etc.
fn parse_ac_params(tokens: &[&str]) -> Option<AcParams> {
    for (i, t) in tokens.iter().enumerate() {
        if t.eq_ignore_ascii_case("AC") {
            let mag = if i + 1 < tokens.len() {
                parse_value(tokens[i + 1]).unwrap_or(0.0)
            } else {
                0.0
            };
            let phase_deg = if i + 2 < tokens.len() {
                parse_value(tokens[i + 2]).unwrap_or(0.0)
            } else {
                0.0
            };
            if mag != 0.0 || phase_deg != 0.0 {
                return Some(AcParams { mag, phase_deg });
            }
        }
    }
    None
}

/// Extract the content between parentheses, splitting into numeric tokens.
/// When parentheses are present, all tokens inside must be numeric.
/// When no parentheses, stops at the first non-numeric token (e.g., DISTOF1).
fn extract_paren_args(s: &str) -> Result<Vec<f64>, String> {
    // Find content between ( and )
    let s = s.trim();
    if let Some(start) = s.find('(') {
        let end = s.rfind(')').unwrap_or(s.len());
        let inner = &s[start + 1..end];
        // Inside parens: all tokens must be numeric
        inner
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .map(|t| parse_value(t))
            .collect()
    } else {
        // No parens: collect numeric tokens, stop at first non-numeric
        let mut vals = Vec::new();
        for t in s.split(|c: char| c.is_whitespace() || c == ',').filter(|s| !s.is_empty()) {
            match parse_value(t) {
                Ok(v) => vals.push(v),
                Err(_) => break, // Stop at non-numeric token (e.g., DISTOF1)
            }
        }
        Ok(vals)
    }
}

/// Parse PULSE(V1 V2 TD TR TF PW PER) — port of vsrcload.c:96-164 parameter handling.
fn parse_pulse(args: &[f64]) -> Result<Waveform, String> {
    if args.len() < 2 {
        return Err("PULSE requires at least V1 and V2".to_string());
    }
    Ok(Waveform::Pulse {
        v1: args[0],
        v2: args[1],
        td: args.get(2).copied().unwrap_or(0.0),
        tr: args.get(3).copied().unwrap_or(0.0), // will be resolved to step in engine
        tf: args.get(4).copied().unwrap_or(0.0),
        pw: args.get(5).copied().unwrap_or(0.0),
        per: args.get(6).copied().unwrap_or(0.0),
    })
}

/// Parse SIN(VO VA FREQ TD THETA PHASE) — port of vsrcload.c:167-196 parameter handling.
fn parse_sin(args: &[f64]) -> Result<Waveform, String> {
    if args.len() < 2 {
        return Err("SIN requires at least VO and VA".to_string());
    }
    Ok(Waveform::Sine {
        vo: args[0],
        va: args[1],
        freq: args.get(2).copied().unwrap_or(0.0), // 0 = will be 1/finalTime
        td: args.get(3).copied().unwrap_or(0.0),
        theta: args.get(4).copied().unwrap_or(0.0),
        phase_deg: args.get(5).copied().unwrap_or(0.0),
    })
}

/// Parse PWL(T1 V1 T2 V2 ...) — port of vsrcload.c:318-362 parameter handling.
fn parse_pwl(args: &[f64]) -> Result<Waveform, String> {
    if args.len() < 2 || args.len() % 2 != 0 {
        return Err(format!("PWL requires even number of args (time-value pairs), got {}", args.len()));
    }
    let pairs: Vec<(f64, f64)> = args.chunks(2).map(|c| (c[0], c[1])).collect();
    Ok(Waveform::Pwl { pairs })
}

/// Parse a .MODEL directive line.
/// Syntax: .MODEL <name> <type> (<param>=<value> ...)
fn parse_model_line(
    line: &str,
    diode_models: &mut HashMap<String, crate::device::diode::DiodeModel>,
    mos_models: &mut HashMap<String, crate::device::mosfet1::Mos1Model>,
    mos2_models: &mut HashMap<String, crate::device::mosfet2::Mos2Model>,
    mos3_models: &mut HashMap<String, crate::device::mosfet3::Mos3Model>,
    bsim3_models: &mut HashMap<String, crate::device::bsim3::Bsim3Model>,
    bsim4_models: &mut HashMap<String, crate::device::bsim4::Bsim4Model>,
    bjt_models: &mut HashMap<String, crate::device::bjt::BjtModel>,
    jfet_models: &mut HashMap<String, crate::device::jfet::JfetModel>,
) -> Result<(), String> {
    // Extract content between parentheses
    let paren_start = line.find('(');
    let paren_end = line.rfind(')');

    // Parse the part before parentheses for name and type
    let pre_paren = if let Some(ps) = paren_start { &line[..ps] } else { line };
    let pre_parts: Vec<&str> = pre_paren.split_whitespace().collect();
    if pre_parts.len() < 3 { return Ok(()); }

    let model_name = pre_parts[1].to_uppercase();
    let model_type = pre_parts[2].to_uppercase();

    // Parse parameters string (between parens or after type)
    let params_str = if let (Some(ps), Some(pe)) = (paren_start, paren_end) {
        line[ps + 1..pe].to_string()
    } else {
        pre_parts[3..].join(" ")
    };

    match model_type.as_str() {
        "D" => {
            let mut model = crate::device::diode::DiodeModel::default();
            for token in params_str.split_whitespace() {
                if let Some(eq_pos) = token.find('=') {
                    let key = token[..eq_pos].to_uppercase();
                    let val = parse_value(&token[eq_pos + 1..]).unwrap_or(0.0);
                    match key.as_str() {
                        "IS" => model.is = val,
                        "N" => model.n = val,
                        "RS" => model.rs = val,
                        "CJO" | "CJ0" | "CJ" => model.cjo = val,
                        "VJ" => model.vj = val,
                        "M" | "MJ" => model.m = val,
                        "TT" => model.tt = val,
                        "BV" => model.bv = val,
                        "IBV" => model.ibv = val,
                        "FC" => model.fc = val,
                        "EG" => model.eg = val,
                        _ => {}
                    }
                }
            }
            diode_models.insert(model_name, model);
        }
        "NMOS" | "PMOS" => {
            // Pre-scan for LEVEL parameter to route to correct model struct
            let mut level = 1;
            for token in params_str.split_whitespace() {
                if let Some(eq_pos) = token.find('=') {
                    let key = token[..eq_pos].to_uppercase();
                    if key == "LEVEL" {
                        level = parse_value(&token[eq_pos + 1..]).unwrap_or(1.0) as i32;
                    }
                }
            }
            let mos_type = if model_type == "NMOS" { 1 } else { -1 };

            if level == 2 {
                // Level 2 (Grove-Frohman) model
                let mut model = crate::device::mosfet2::Mos2Model::default();
                model.mos_type = mos_type;
                for token in params_str.split_whitespace() {
                    if let Some(eq_pos) = token.find('=') {
                        let key = token[..eq_pos].to_uppercase();
                        let val = parse_value(&token[eq_pos + 1..]).unwrap_or(0.0);
                        match key.as_str() {
                            "VTO" | "VT0" => { model.vto = val; model.vto_given = true; }
                            "KP" => { model.kp = val; model.kp_given = true; }
                            "GAMMA" => { model.gamma = val; model.gamma_given = true; }
                            "PHI" => { model.phi = val; model.phi_given = true; }
                            "LAMBDA" => model.lambda = val,
                            "RD" => model.rd = val,
                            "RS" => model.rs = val,
                            "CBD" => { model.cbd = val; model.cbd_given = true; }
                            "CBS" => { model.cbs = val; model.cbs_given = true; }
                            "IS" => model.is_ = val,
                            "PB" => model.pb = val,
                            "CGSO" => model.cgso = val,
                            "CGDO" => model.cgdo = val,
                            "CGBO" => model.cgbo = val,
                            "CJ" => { model.cj = val; model.cj_given = true; }
                            "MJ" => model.mj = val,
                            "CJSW" => { model.cjsw = val; model.cjsw_given = true; }
                            "MJSW" => model.mjsw = val,
                            "TOX" => model.tox = val,
                            "LD" => model.ld = val,
                            "U0" | "UO" => { model.u0 = val; model.surface_mobility = val; model.u0_given = true; }
                            "FC" => model.fc = val,
                            "NSS" => model.nss = val,
                            "NSUB" => { model.nsub = val; model.nsub_given = true; }
                            "TPG" => model.tpg = val as i32,
                            "RSH" => model.rsh = val,
                            "JS" => model.js = val,
                            "TNOM" => { model.tnom = val + 273.15; model.tnom_given = true; }
                            "NFS" => { model.nfs = val; model.nfs_given = true; }
                            "DELTA" => model.delta = val,
                            "UEXP" => model.uexp = val,
                            "UCRIT" => model.ucrit = val,
                            "VMAX" => model.vmax = val,
                            "XJ" => model.xj = val,
                            "NEFF" => model.neff = val,
                            "LEVEL" => {} // already handled
                            _ => {}
                        }
                    }
                }
                // Compute oxide cap factor (mos2temp.c:65-66)
                if model.tox <= 0.0 { model.tox = 1e-7; } // default from mos2temp.c:62-63
                model.oxide_cap_factor = 3.9 * 8.854214871e-12 / model.tox;
                if !model.u0_given { model.surface_mobility = 600.0; }
                if !model.kp_given {
                    model.kp = model.surface_mobility * 1e-4 * model.oxide_cap_factor;
                }
                // Auto-compute from NSUB (mos2temp.c:73-114)
                if model.nsub_given && model.nsub * 1e6 > 1.45e16 {
                    let vtnom = KoverQ * model.tnom;
                    let egfet1 = 1.16 - 7.02e-4 * model.tnom * model.tnom / (model.tnom + 1108.0);
                    if !model.phi_given {
                        model.phi = 2.0 * vtnom * (model.nsub * 1e6 / 1.45e16).ln();
                        model.phi = f64::max(0.1, model.phi);
                    }
                    let fermis = model.mos_type as f64 * 0.5 * model.phi;
                    let mut wkfng = 3.2;
                    if model.tpg != 0 {
                        let fermig = model.mos_type as f64 * model.tpg as f64 * 0.5 * egfet1;
                        wkfng = 3.25 + 0.5 * egfet1 - fermig;
                    }
                    let wkfngs = wkfng - (3.25 + 0.5 * egfet1 + fermis);
                    if !model.gamma_given {
                        model.gamma = (2.0 * 11.7 * 8.854214871e-12 * CHARGE
                            * model.nsub * 1e6).sqrt() / model.oxide_cap_factor;
                    }
                    if !model.vto_given {
                        let nss = model.nss; // default 0 if not given
                        let vfb = wkfngs - nss * 1e4 * CHARGE / model.oxide_cap_factor;
                        model.vto = vfb + model.mos_type as f64
                            * (model.gamma * model.phi.sqrt() + model.phi);
                    } else {
                        // Even if vto_given, compute vfb for xd
                    }
                    // xd computation (mos2temp.c:107-108)
                    model.xd = ((EPSSIL + EPSSIL) / (CHARGE * model.nsub * 1e6)).sqrt();
                }
                // bulkCapFactor default (mos2temp.c:116-120)
                if model.cj <= 0.0 && model.nsub > 0.0 {
                    model.cj = (EPSSIL * CHARGE * model.nsub * 1e6
                        / (2.0 * model.pb)).sqrt();
                }
                mos2_models.insert(model_name, model);
            } else if level == 3 {
                // Level 3 (semi-empirical) model
                let mut model = crate::device::mosfet3::Mos3Model::default();
                model.mos_type = mos_type;
                for token in params_str.split_whitespace() {
                    if let Some(eq_pos) = token.find('=') {
                        let key = token[..eq_pos].to_uppercase();
                        let val = parse_value(&token[eq_pos + 1..]).unwrap_or(0.0);
                        match key.as_str() {
                            "VTO" | "VT0" => { model.vto = val; model.vto_given = true; }
                            "KP" => { model.kp = val; model.kp_given = true; }
                            "GAMMA" => { model.gamma = val; model.gamma_given = true; }
                            "PHI" => { model.phi = val; model.phi_given = true; }
                            "RD" => model.rd = val,
                            "RS" => model.rs = val,
                            "CBD" => { model.cbd = val; model.cbd_given = true; }
                            "CBS" => { model.cbs = val; model.cbs_given = true; }
                            "IS" => model.is_ = val,
                            "PB" => model.pb = val,
                            "CGSO" => model.cgso = val,
                            "CGDO" => model.cgdo = val,
                            "CGBO" => model.cgbo = val,
                            "CJ" => { model.cj = val; model.cj_given = true; }
                            "MJ" => model.mj = val,
                            "CJSW" => { model.cjsw = val; model.cjsw_given = true; }
                            "MJSW" => model.mjsw = val,
                            "TOX" => model.tox = val,
                            "LD" => model.ld = val,
                            "U0" | "UO" => { model.u0 = val; model.surface_mobility = val; model.u0_given = true; }
                            "FC" => model.fc = val,
                            "NSS" => model.nss = val,
                            "NSUB" => { model.nsub = val; model.nsub_given = true; }
                            "TPG" => model.tpg = val as i32,
                            "RSH" => model.rsh = val,
                            "JS" => model.js = val,
                            "TNOM" => { model.tnom = val + 273.15; model.tnom_given = true; }
                            "ETA" => model.eta = val,
                            "THETA" => model.theta = val,
                            "KAPPA" => model.kappa = val,
                            "DELTA" => model.delta = val,
                            "NFS" => { model.nfs = val; model.nfs_given = true; }
                            "VMAX" => model.vmax = val,
                            "XJ" => model.xj = val,
                            "ALPHA" => model.alpha = val,
                            "XL" => model.length_adjust = val,
                            "WD" => model.width_narrow = val,
                            "XW" => model.width_adjust = val,
                            "DELVTO" | "DELVT0" => model.delvt0 = val,
                            "LEVEL" => {} // already handled
                            _ => {}
                        }
                    }
                }
                // Oxide thickness default (mos3set.c:133-134)
                if model.tox <= 0.0 { model.tox = 1e-7; }
                // Oxide cap factor (mos3temp.c:62-63)
                model.oxide_cap_factor = 3.9 * 8.854214871e-12 / model.tox;
                // Surface mobility default (mos3temp.c:64)
                if !model.u0_given { model.surface_mobility = 600.0; }
                // Transconductance (mos3temp.c:65-68)
                if !model.kp_given {
                    model.kp = model.surface_mobility * model.oxide_cap_factor * 1e-4;
                }
                // Auto-compute from NSUB (mos3temp.c:69-111)
                if model.nsub_given {
                    let vtnom = KoverQ * model.tnom;
                    let egfet1 = 1.16 - 7.02e-4 * model.tnom * model.tnom / (model.tnom + 1108.0);
                    // ni_temp computation (mos3temp.c:51-54)
                    let nifact = (model.tnom / 300.0) * (model.tnom / 300.0).sqrt();
                    let nifact = nifact * (0.5 * egfet1 * ((1.0 / 300.0) - (1.0 / model.tnom)) / KoverQ).exp();
                    let ni_temp = 1.45e16 * nifact;
                    if model.nsub * 1e6 > ni_temp {
                        if !model.phi_given {
                            model.phi = 2.0 * vtnom * (model.nsub * 1e6 / ni_temp).ln();
                            model.phi = f64::max(0.1, model.phi);
                        }
                        let fermis = model.mos_type as f64 * 0.5 * model.phi;
                        let mut wkfng = 3.2;
                        if model.tpg != 0 {
                            let fermig = model.mos_type as f64 * model.tpg as f64 * 0.5 * egfet1;
                            wkfng = 3.25 + 0.5 * egfet1 - fermig;
                        }
                        let wkfngs = wkfng - (3.25 + 0.5 * egfet1 + fermis);
                        if !model.gamma_given {
                            model.gamma = (2.0 * EPSSIL * CHARGE * model.nsub * 1e6).sqrt()
                                / model.oxide_cap_factor;
                        }
                        if !model.vto_given {
                            let vfb = wkfngs - model.nss * 1e4 * CHARGE / model.oxide_cap_factor;
                            model.vto = vfb + model.mos_type as f64
                                * (model.gamma * model.phi.sqrt() + model.phi);
                        }
                        // alpha and xd (mos3temp.c:102-104)
                        model.alpha = (EPSSIL + EPSSIL) / (CHARGE * model.nsub * 1e6);
                        model.xd = model.alpha.sqrt();
                    }
                }
                // Narrow factor (mos3temp.c:113-114)
                model.narrow_factor = model.delta * 0.5 * std::f64::consts::PI * EPSSIL
                    / model.oxide_cap_factor;
                mos3_models.insert(model_name, model);
            } else if level == 8 || level == 49 {
                // BSIM3v3.3
                let mut model = crate::device::bsim3::Bsim3Model::default();
                model.mos_type = mos_type;
                for token in params_str.split_whitespace() {
                    if let Some(eq_pos) = token.find('=') {
                        let key = token[..eq_pos].to_uppercase();
                        let val = parse_value(&token[eq_pos + 1..]).unwrap_or(0.0);
                        match key.as_str() {
                            "TOX" => model.tox = val,
                            "TOXM" => model.toxm = val,
                            "CDSC" => model.cdsc = val,
                            "CDSCB" => model.cdscb = val,
                            "CDSCD" => model.cdscd = val,
                            "CIT" => model.cit = val,
                            "NFACTOR" => model.nfactor = val,
                            "XJ" => { model.xj = val; model.xt_given = true; }
                            "VSAT" => model.vsat = val,
                            "AT" => model.at = val,
                            "A0" => model.a0 = val,
                            "AGS" => model.ags = val,
                            "A1" => model.a1 = val,
                            "A2" => model.a2 = val,
                            "KETA" => model.keta = val,
                            "NSUB" => { model.nsub = val; model.nsub_given = true; }
                            "NPEAK" | "NCH" => { model.npeak = val; model.npeak_given = true; }
                            "NGATE" => { model.ngate = val; model.ngate_given = true; }
                            "GAMMA1" => { model.gamma1 = val; model.gamma1_given = true; }
                            "GAMMA2" => { model.gamma2 = val; model.gamma2_given = true; }
                            "VBX" => { model.vbx = val; model.vbx_given = true; }
                            "VBM" => model.vbm = val,
                            "XT" => { model.xt = val; model.xt_given = true; }
                            "K1" => { model.k1 = val; model.k1_given = true; }
                            "KT1" => model.kt1 = val,
                            "KT1L" => model.kt1l = val,
                            "KT2" => model.kt2 = val,
                            "K2" => { model.k2 = val; model.k2_given = true; }
                            "K3" => model.k3 = val,
                            "K3B" => model.k3b = val,
                            "W0" => model.w0 = val,
                            "NLX" => model.nlx = val,
                            "DVT0" => model.dvt0 = val,
                            "DVT1" => model.dvt1 = val,
                            "DVT2" => model.dvt2 = val,
                            "DVT0W" => model.dvt0w = val,
                            "DVT1W" => model.dvt1w = val,
                            "DVT2W" => model.dvt2w = val,
                            "DROUT" => model.drout = val,
                            "DSUB" => model.dsub = val,
                            "VTH0" | "VTHO" => { model.vth0 = val; model.vth0_given = true; }
                            "UA" => model.ua = val,
                            "UA1" => model.ua1 = val,
                            "UB" => model.ub = val,
                            "UB1" => model.ub1 = val,
                            "UC" => model.uc = val,
                            "UC1" => model.uc1 = val,
                            "U0" => model.u0 = val,
                            "UTE" => model.ute = val,
                            "VOFF" => model.voff = val,
                            "DELTA" => model.delta = val,
                            "RDSW" => model.rdsw = val,
                            "PRWG" => model.prwg = val,
                            "PRWB" => model.prwb = val,
                            "PRT" => model.prt = val,
                            "ETA0" => model.eta0 = val,
                            "ETAB" => model.etab = val,
                            "PCLM" => model.pclm = val,
                            "PDIBLC1" | "PDIBL1" => model.pdibl1 = val,
                            "PDIBLC2" | "PDIBL2" => model.pdibl2 = val,
                            "PDIBLCB" | "PDIBLB" => model.pdiblb = val,
                            "PSCBE1" => model.pscbe1 = val,
                            "PSCBE2" => model.pscbe2 = val,
                            "PVAG" => model.pvag = val,
                            "WR" => model.wr = val,
                            "DWG" => model.dwg = val,
                            "DWB" => model.dwb = val,
                            "B0" => model.b0 = val,
                            "B1" => model.b1 = val,
                            "ALPHA0" => model.alpha0 = val,
                            "ALPHA1" => model.alpha1 = val,
                            "BETA0" => model.beta0 = val,
                            "IJTH" | "IJTHN" | "IJTHDFWD" => model.ijth = val,
                            "VFB" => { model.vfb = val; model.vfb_given = true; }
                            "ELM" => model.elm = val,
                            "CGSL" => model.cgsl = val,
                            "CGDL" => model.cgdl = val,
                            "CKAPPA" => model.ckappa = val,
                            "CF" => { model.cf = val; model.cf_given = true; }
                            "CLC" => model.clc = val,
                            "CLE" => model.cle = val,
                            "VFBCV" => model.vfbcv = val,
                            "ACDE" => model.acde = val,
                            "MOIN" => model.moin = val,
                            "NOFF" => model.noff = val,
                            "VOFFCV" => model.voffcv = val,
                            "TCJ" => model.tcj = val,
                            "TPB" => model.tpb = val,
                            "TCJSW" => model.tcjsw = val,
                            "TPBSW" => model.tpbsw = val,
                            "TCJSWG" => model.tcjswg = val,
                            "TPBSWG" => model.tpbswg = val,
                            "DLC" => { model.dlc = val; model.dlc_given = true; }
                            "DWC" => { model.dwc = val; model.dwc_given = true; }
                            "LINT" => model.lint = val,
                            "WINT" => model.wint = val,
                            "XL" => model.xl = val,
                            "XW" => model.xw = val,
                            "RSH" => model.sheet_resistance = val,
                            "JS" => model.jct_sat_cur_density = val,
                            "JSW" => model.jct_sidewall_sat_cur_density = val,
                            "PB" => model.bulk_jct_potential = val,
                            "PBSW" => model.sidewall_jct_potential = val,
                            "PBSWG" => model.gate_sidewall_jct_potential = val,
                            "CJ" => model.unit_area_jct_cap = val,
                            "CJSW" => model.unit_length_sidewall_jct_cap = val,
                            "CJSWG" | "CJGATE" => model.unit_length_gate_sidewall_jct_cap = val,
                            "MJ" => model.bulk_jct_bot_grading_coeff = val,
                            "MJSW" => model.bulk_jct_side_grading_coeff = val,
                            "MJSWG" => model.bulk_jct_gate_side_grading_coeff = val,
                            "NJ" => model.jct_emission_coeff = val,
                            "XTI" => model.jct_temp_exponent = val,
                            "XPART" => model.xpart = val,
                            "CGDO" => { model.cgdo = val; model.cgdo_given = true; }
                            "CGSO" => { model.cgso = val; model.cgso_given = true; }
                            "CGBO" => { model.cgbo = val; model.cgbo_given = true; }
                            "MOBMOD" => model.mob_mod = val as i32,
                            "CAPMOD" => model.cap_mod = val as i32,
                            "NQSMOD" => model.nqs_mod = val as i32,
                            "BINUNIT" => model.bin_unit = val as i32,
                            "TNOM" => { model.tnom = val + 273.15; model.tnom_given = true; }
                            "TOXM" => model.toxm = val,
                            "LL" => model.ll = val,
                            "LLN" => model.lln = val,
                            "LW" => model.lw = val,
                            "LWN" => model.lwn = val,
                            "LWL" => model.lwl = val,
                            "WL" => model.wl = val,
                            "WLN" => model.wln = val,
                            "WW" => model.ww = val,
                            "WWN" => model.wwn = val,
                            "WWL" => model.wwl = val,
                            "LEVEL" | "VERSION" => {} // already handled
                            _ => {} // ignore unknown params
                        }
                    }
                }
                model.apply_defaults();
                bsim3_models.insert(model_name, model);
            } else if level == 14 {
                // BSIM4 v4.8.3
                let mut model = crate::device::bsim4::Bsim4Model::default();
                model.mos_type = mos_type;
                for token in params_str.split_whitespace() {
                    if let Some(eq_pos) = token.find('=') {
                        let key = token[..eq_pos].to_uppercase();
                        let val = parse_value(&token[eq_pos + 1..]).unwrap_or(0.0);
                        match key.as_str() {
                            "TOXE" => { model.toxe = val; model.toxe_given = true; }
                            "TOXP" => { model.toxp = val; model.toxp_given = true; }
                            "TOXM" => { model.toxm = val; model.toxm_given = true; }
                            "DTOX" => model.dtox = val,
                            "EPSROX" => model.epsrox = val,
                            "VTH0" | "VTHO" => { model.vth0 = val; model.vth0_given = true; }
                            "K1" => { model.k1 = val; model.k1_given = true; }
                            "K2" => { model.k2 = val; model.k2_given = true; }
                            "K3" => model.k3 = val,
                            "K3B" => model.k3b = val,
                            "W0" => model.w0 = val,
                            "DVT0" => model.dvt0 = val,
                            "DVT1" => model.dvt1 = val,
                            "DVT2" => model.dvt2 = val,
                            "DVT0W" => model.dvt0w = val,
                            "DVT1W" => model.dvt1w = val,
                            "DVT2W" => model.dvt2w = val,
                            "DSUB" => { model.dsub = val; model.dsub_given = true; }
                            "DROUT" => model.drout = val,
                            "U0" => model.u0 = val,
                            "UA" => { model.ua = val; model.ua_given = true; }
                            "UA1" => model.ua1 = val,
                            "UB" => model.ub = val,
                            "UB1" => model.ub1 = val,
                            "UC" => { model.uc = val; model.uc_given = true; }
                            "UC1" => { model.uc1 = val; model.uc1_given = true; }
                            "UD" => model.ud = val,
                            "UD1" => model.ud1 = val,
                            "UP" => model.up = val,
                            "LP" => model.lp = val,
                            "EU" => model.eu = val,
                            "UCS" => model.ucs = val,
                            "UTE" => model.ute = val,
                            "UCSTE" => model.ucste = val,
                            "VSAT" => model.vsat = val,
                            "AT" => model.at = val,
                            "A0" => model.a0 = val,
                            "AGS" => model.ags = val,
                            "A1" => model.a1 = val,
                            "A2" => model.a2 = val,
                            "KETA" => model.keta = val,
                            "NSUB" => { model.nsub = val; model.nsub_given = true; }
                            "NDEP" => { model.ndep = val; model.ndep_given = true; }
                            "NSD" => model.nsd = val,
                            "PHIN" => model.phin = val,
                            "NGATE" => model.ngate = val,
                            "NFACTOR" => model.nfactor = val,
                            "VOFF" => model.voff = val,
                            "VOFFL" => model.voffl = val,
                            "MINV" => model.minv = val,
                            "MINVCV" => model.minvcv = val,
                            "ETA0" => model.eta0 = val,
                            "ETAB" => model.etab = val,
                            "PCLM" => model.pclm = val,
                            "PDIBLC1" => model.pdibl1 = val,
                            "PDIBLC2" => model.pdibl2 = val,
                            "PDIBLCB" => model.pdiblb = val,
                            "PSCBE1" => model.pscbe1 = val,
                            "PSCBE2" => model.pscbe2 = val,
                            "PVAG" => model.pvag = val,
                            "DELTA" => model.delta = val,
                            "RDSW" => model.rdsw = val,
                            "RDSWMIN" => model.rdswmin = val,
                            "RDW" => model.rdw = val,
                            "RSW" => model.rsw = val,
                            "PRWG" => model.prwg = val,
                            "PRWB" => model.prwb = val,
                            "PRT" => model.prt = val,
                            "WR" => model.wr = val,
                            "DWG" => model.dwg = val,
                            "DWB" => model.dwb = val,
                            "B0" => model.b0 = val,
                            "B1" => model.b1 = val,
                            "ALPHA0" => model.alpha0 = val,
                            "ALPHA1" => model.alpha1 = val,
                            "BETA0" => model.beta0 = val,
                            "XJ" => model.xj = val,
                            "XT" => model.xt = val,
                            "VBM" => model.vbm = val,
                            "VFB" => { model.vfb = val; model.vfb_given = true; }
                            "GAMMA1" => { model.gamma1 = val; model.gamma1_given = true; }
                            "GAMMA2" => { model.gamma2 = val; model.gamma2_given = true; }
                            "VBX" => { model.vbx = val; model.vbx_given = true; }
                            "KT1" => model.kt1 = val,
                            "KT1L" => model.kt1l = val,
                            "KT2" => model.kt2 = val,
                            "LPE0" => model.lpe0 = val,
                            "LPEB" => model.lpeb = val,
                            "FPROUT" => model.fprout = val,
                            "PDITS" => model.pdits = val,
                            "PDITSD" => model.pditsd = val,
                            "PDITSL" => model.pditsl = val,
                            "LAMBDA" => model.lambda = val,
                            "VTL" => { model.vtl = val; model.vtl_given = true; }
                            "XN" => model.xn = val,
                            "LC" => model.lc = val,
                            "TOXREF" => model.toxref = val,
                            "MOBMOD" => model.mob_mod = val as i32,
                            "CAPMOD" => model.cap_mod = val as i32,
                            "DIOMOD" => model.dio_mod = val as i32,
                            "RDSMOD" => model.rds_mod = val as i32,
                            "RBODYMOD" => model.rbody_mod = val as i32,
                            "RGATEMOD" => model.rgate_mod = val as i32,
                            "PERMOD" => model.per_mod = val as i32,
                            "GEOMOD" => model.geo_mod = val as i32,
                            "TEMPMOD" => model.temp_mod = val as i32,
                            "MTRLMOD" => model.mtrl_mod = val as i32,
                            "IGCMOD" => model.igc_mod = val as i32,
                            "IGBMOD" => model.igb_mod = val as i32,
                            "GIDLMOD" => model.gidl_mod = val as i32,
                            "BINUNIT" => model.bin_unit = val as i32,
                            "TNOM" => { model.tnom = val + 273.15; model.tnom_given = true; }
                            "DLC" => { model.dlc = val; model.dlc_given = true; }
                            "DWC" => model.dwc = val,
                            "DWJ" => model.dwj = val,
                            "CF" => { model.cf = val; model.cf_given = true; }
                            "CGSL" => model.cgsl = val,
                            "CGDL" => model.cgdl = val,
                            "CKAPPAS" => model.ckappas = val,
                            "CKAPPAD" => model.ckappad = val,
                            "CLC" => model.clc = val,
                            "CLE" => model.cle = val,
                            "VFBCV" => model.vfbcv = val,
                            "NOFF" => model.noff = val,
                            "VOFFCV" => model.voffcv = val,
                            "VOFFCVL" => model.voffcvl = val,
                            "ACDE" => model.acde = val,
                            "MOIN" => model.moin = val,
                            "CGDO" => { model.cgdo = val; model.cgdo_given = true; }
                            "CGSO" => { model.cgso = val; model.cgso_given = true; }
                            "CGBO" => { model.cgbo = val; model.cgbo_given = true; }
                            "XL" => model.xl = val,
                            "XW" => model.xw = val,
                            "LINT" => model.lint = val,
                            "WINT" => model.wint = val,
                            "RSH" | "SHEETRESISTANCE" => model.sheet_resistance = val,
                            "AGIDL" => model.agidl = val,
                            "BGIDL" => model.bgidl = val,
                            "CGIDL" => model.cgidl = val,
                            "EGIDL" => model.egidl = val,
                            "NIGC" => model.nigc = val,
                            "NIGBACC" => model.nigbacc = val,
                            "NIGBINV" => model.nigbinv = val,
                            "NTOX" => model.ntox = val,
                            "EIGBINV" => model.eigbinv = val,
                            "PIGCD" => model.pigcd = val,
                            "POXEDGE" => model.poxedge = val,
                            "XRCRG1" => model.xrcrg1 = val,
                            "XRCRG2" => model.xrcrg2 = val,
                            "IJTHSFWD" => model.ijthsfwd = val,
                            "IJTHDFWD" => model.ijthdfwd = val,
                            "IJTHSREV" => model.ijthsrev = val,
                            "IJTHDREV" => model.ijthdrev = val,
                            "XJBVS" => model.xjbvs = val,
                            "XJBVD" => model.xjbvd = val,
                            "BVS" => model.bvs = val,
                            "BVD" => model.bvd = val,
                            "GBMIN" => model.gbmin = val,
                            "TCJ" => model.tcj = val,
                            "TPB" => model.tpb = val,
                            "TCJSW" => model.tcjsw = val,
                            "TPBSW" => model.tpbsw = val,
                            "TCJSWG" => model.tcjswg = val,
                            "TPBSWG" => model.tpbswg = val,
                            "TVOFF" => model.tvoff = val,
                            "TNFACTOR" => model.tnfactor = val,
                            "TETA0" => model.teta0 = val,
                            "TVOFFCV" => model.tvoffcv = val,
                            "DVTP0" => model.dvtp0 = val,
                            "DVTP1" => model.dvtp1 = val,
                            "DVTP2" => model.dvtp2 = val,
                            "DVTP3" => model.dvtp3 = val,
                            "DVTP4" => model.dvtp4 = val,
                            "DVTP5" => model.dvtp5 = val,
                            "DMCG" => model.dmcg = val,
                            "DMCI" => model.dmci = val,
                            "DMDG" => model.dmdg = val,
                            "DMCGT" => model.dmcgt = val,
                            "XGW" => model.xgw = val,
                            "XGL" => model.xgl = val,
                            "RSHG" => model.rshg = val,
                            "NGCON" => model.ngcon = val,
                            "JS" | "JSS" => model.sjct_sat_cur_density = val,
                            "JSD" => model.djct_sat_cur_density = val,
                            "PBS" => model.sbulk_jct_potential = val,
                            "PBD" => model.dbulk_jct_potential = val,
                            "PBSWS" => model.ssidewall_jct_potential = val,
                            "PBSWD" => model.dsidewall_jct_potential = val,
                            "CJS" => model.sunit_area_jct_cap = val,
                            "CJD" => model.dunit_area_jct_cap = val,
                            "MJS" => model.sbulk_jct_bot_grading_coeff = val,
                            "MJD" => model.dbulk_jct_bot_grading_coeff = val,
                            "NJS" => model.sjct_emission_coeff = val,
                            "NJD" => model.djct_emission_coeff = val,
                            "LEVEL" | "VERSION" => {} // already handled
                            _ => {} // ignore unknown params
                        }
                    }
                }
                model.apply_defaults();
                bsim4_models.insert(model_name, model);
            } else {
                // Level 1 (Shichman-Hodges) model
                let mut model = crate::device::mosfet1::Mos1Model::default();
                model.mos_type = mos_type;
                for token in params_str.split_whitespace() {
                    if let Some(eq_pos) = token.find('=') {
                        let key = token[..eq_pos].to_uppercase();
                        let val = parse_value(&token[eq_pos + 1..]).unwrap_or(0.0);
                        match key.as_str() {
                            "VTO" | "VT0" => { model.vto = val; model.vto_given = true; }
                            "KP" => { model.kp = val; model.kp_given = true; }
                            "GAMMA" => { model.gamma = val; model.gamma_given = true; }
                            "PHI" => { model.phi = val; model.phi_given = true; }
                            "LAMBDA" => model.lambda = val,
                            "RD" => model.rd = val,
                            "RS" => model.rs = val,
                            "CBD" => { model.cbd = val; model.cbd_given = true; }
                            "CBS" => { model.cbs = val; model.cbs_given = true; }
                            "IS" => model.is_ = val,
                            "PB" => model.pb = val,
                            "CGSO" => model.cgso = val,
                            "CGDO" => model.cgdo = val,
                            "CGBO" => model.cgbo = val,
                            "CJ" => { model.cj = val; model.cj_given = true; }
                            "MJ" => model.mj = val,
                            "CJSW" => { model.cjsw = val; model.cjsw_given = true; }
                            "MJSW" => model.mjsw = val,
                            "TOX" => model.tox = val,
                            "LD" => model.ld = val,
                            "U0" | "UO" => { model.u0 = val; model.u0_given = true; }
                            "FC" => model.fc = val,
                            "NSS" => model.nss = val,
                            "NSUB" => model.nsub = val,
                            "TPG" => model.tpg = val as i32,
                            "RSH" => model.rsh = val,
                            "JS" => model.js = val,
                            "TNOM" => { model.tnom = val + 273.15; model.tnom_given = true; }
                            "LEVEL" => {} // accept and ignore for level 1
                            _ => {}
                        }
                    }
                }
                // Compute oxide cap factor (mos1temp.c:64)
                if model.tox > 0.0 {
                    model.oxide_cap_factor = EPSOX / model.tox;
                    if !model.kp_given {
                        let u0 = if model.u0_given { model.u0 } else { 600.0 };
                        model.kp = u0 * model.oxide_cap_factor * 1e-4;
                    }
                }
                // Auto-compute from NSUB (mos1temp.c:73-111)
                if model.nsub > 0.0 && model.nsub * 1e6 > 1.45e16 {
                    let vtnom = 8.6171e-5 * model.tnom; // k*T/q at nominal
                    let egfet1 = 1.16 - 7.02e-4 * model.tnom * model.tnom / (model.tnom + 1108.0);
                    if !model.phi_given {
                        model.phi = 2.0 * vtnom * (model.nsub * 1e6 / 1.45e16).ln();
                        model.phi = f64::max(0.1, model.phi);
                    }
                    let fermis = model.mos_type as f64 * 0.5 * model.phi;
                    let mut wkfng = 3.2;
                    if model.tpg != 0 {
                        let fermig = model.mos_type as f64 * model.tpg as f64 * 0.5 * egfet1;
                        wkfng = 3.25 + 0.5 * egfet1 - fermig;
                    }
                    let wkfngs = wkfng - (3.25 + 0.5 * egfet1 + fermis);
                    if !model.gamma_given && model.oxide_cap_factor > 0.0 {
                        model.gamma = (2.0 * 11.7 * 8.854214871e-12 * 1.6021918e-19
                            * model.nsub * 1e6).sqrt() / model.oxide_cap_factor;
                    }
                    if !model.vto_given && model.oxide_cap_factor > 0.0 {
                        let vfb = wkfngs - model.nss * 1e4 * 1.6021918e-19 / model.oxide_cap_factor;
                        model.vto = vfb + model.mos_type as f64
                            * (model.gamma * model.phi.sqrt() + model.phi);
                    }
                }
                mos_models.insert(model_name, model);
            }
        }
        "NPN" | "PNP" => {
            let mut model = crate::device::bjt::BjtModel::default();
            model.bjt_type = if model_type == "NPN" { 1 } else { -1 };
            for token in params_str.split_whitespace() {
                if let Some(eq_pos) = token.find('=') {
                    let key = token[..eq_pos].to_uppercase();
                    let val = parse_value(&token[eq_pos + 1..]).unwrap_or(0.0);
                    match key.as_str() {
                        "IS" => model.is_ = val,
                        "BF" => model.bf = val,
                        "NF" => model.nf = val,
                        "BR" => model.br = val,
                        "NR" => model.nr = val,
                        "ISE" => model.ise = val,
                        "NE" => model.ne = val,
                        "ISC" => model.isc = val,
                        "NC" => model.nc = val,
                        "VAF" | "VA" => model.vaf = val,
                        "VAR" | "VB" => model.var = val,
                        "IKF" | "JBF" => model.ikf = val,
                        "IKR" | "JBR" => model.ikr = val,
                        "RB" => model.rb = val,
                        "RBM" => model.rbm = val,
                        "RE" => model.re = val,
                        "RC" => model.rc = val,
                        "CJE" => model.cje = val,
                        "VJE" | "PE" => model.vje = val,
                        "MJE" | "ME" => model.mje = val,
                        "CJC" => model.cjc = val,
                        "VJC" | "PC" => model.vjc = val,
                        "MJC" | "MC" => model.mjc = val,
                        "XCJC" => model.xcjc = val,
                        "CJS" | "CCS" => model.cjs = val,
                        "VJS" => model.vjs = val,
                        "MJS" => model.mjs = val,
                        "TF" => model.tf = val,
                        "TR" => model.tr = val,
                        "XTF" => model.xtf = val,
                        "VTF" => model.vtf = val,
                        "ITF" => model.itf = val,
                        "PTF" => model.ptf = val,
                        "EG" => model.eg = val,
                        "XTB" => model.xtb = val,
                        "FC" => model.fc = val,
                        "TNOM" => { model.tnom = val + 273.15; model.tnom_given = true; }
                        "LEVEL" => {} // accept and ignore
                        _ => {}
                    }
                }
            }
            bjt_models.insert(model_name, model);
        }
        "NJF" | "PJF" => {
            let mut model = crate::device::jfet::JfetModel::default();
            model.jfet_type = if model_type == "NJF" { 1 } else { -1 };
            for token in params_str.split_whitespace() {
                if let Some(eq_pos) = token.find('=') {
                    let key = token[..eq_pos].to_uppercase();
                    let val = parse_value(&token[eq_pos + 1..]).unwrap_or(0.0);
                    match key.as_str() {
                        "VTO" | "VT0" => model.vto = val,
                        "BETA" => model.beta = val,
                        "LAMBDA" => model.lambda = val,
                        "RD" => model.rd = val,
                        "RS" => model.rs = val,
                        "CGS" => model.cgs = val,
                        "CGD" => model.cgd = val,
                        "PB" => model.pb = val,
                        "IS" => model.is_ = val,
                        "N" => model.n = val,
                        "FC" => model.fc = val,
                        "B" => model.b = val,
                        "TNOM" => { model.tnom = val + 273.15; model.tnom_given = true; }
                        "TCV" => model.tcv = val,
                        "VTOTC" => { model.vtotc = val; model.vtotc_given = true; }
                        "BEX" => model.bex = val,
                        "BETATCE" => { model.betatce = val; model.betatce_given = true; }
                        "XTI" => { model.xti = val; model.xti_given = true; }
                        "EG" => model.eg = val,
                        "KF" | "AF" => {} // noise params, ignore
                        "NLEV" | "GDSNOI" => {} // noise params, ignore
                        "LEVEL" => {} // accept and ignore
                        _ => {}
                    }
                }
            }
            jfet_models.insert(model_name, model);
        }
        _ => {} // ignore unknown model types
    }
    Ok(())
}

const EPSOX: f64 = 3.9 * 8.854214871e-12;

/// Parse a SPICE numeric value with optional suffix.
/// Supports: T, G, MEG, k, m, u, n, p, f
/// Parse a numeric value — faithful port of ngspice INPevaluate (inpeval.c:13-203).
///
/// Accumulates digits into a mantissa, then multiplies by pow(10, exponent).
/// This does NOT produce correctly-rounded IEEE 754 results for all inputs
/// (e.g., "0.568" → 568*pow(10,-3) which is 1 ULP different from strtod).
/// We match ngspice's behavior exactly so that parsed parameters are bit-identical.
pub fn parse_value(s: &str) -> Result<f64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Empty value".to_string());
    }

    let bytes = s.as_bytes();
    let mut i = 0;

    // Sign
    let sign: f64 = if i < bytes.len() && bytes[i] == b'+' {
        i += 1;
        1.0
    } else if i < bytes.len() && bytes[i] == b'-' {
        i += 1;
        -1.0
    } else {
        1.0
    };

    if i >= bytes.len() || (!bytes[i].is_ascii_digit() && bytes[i] != b'.') {
        return Err(format!("Invalid number: '{s}'"));
    }

    // Integer part: mantis = 10*mantis + digit
    let mut mantis: f64 = 0.0;
    let mut expo1: i32 = 0;

    while i < bytes.len() && bytes[i].is_ascii_digit() {
        mantis = 10.0 * mantis + (bytes[i] - b'0') as f64;
        i += 1;
    }

    // Decimal point
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            mantis = 10.0 * mantis + (bytes[i] - b'0') as f64;
            expo1 -= 1;
            i += 1;
        }
    }

    // Exponent: E/e/D/d
    let mut expo2: i32 = 0;
    let mut expsgn: i32 = 1;
    if i < bytes.len() && matches!(bytes[i], b'E' | b'e' | b'D' | b'd') {
        i += 1;
        if i < bytes.len() && bytes[i] == b'+' {
            i += 1;
        } else if i < bytes.len() && bytes[i] == b'-' {
            i += 1;
            expsgn = -1;
        }
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            expo2 = 10 * expo2 + (bytes[i] - b'0') as i32;
            i += 1;
        }
    }

    // SPICE scale factor suffix
    if i < bytes.len() {
        let rest = &s[i..].to_uppercase();
        if rest.starts_with("MEG") {
            expo1 += 6;
        } else if rest.starts_with("MIL") {
            expo1 -= 6;
            mantis *= 25.4;
        } else {
            match rest.as_bytes()[0] {
                b'T' => expo1 += 12,
                b'G' => expo1 += 9,
                b'K' => expo1 += 3,
                b'M' => expo1 -= 3,
                b'U' => expo1 -= 6,
                b'N' => expo1 -= 9,
                b'P' => expo1 -= 12,
                b'F' => expo1 -= 15,
                b'A' => expo1 -= 18,
                _ => {} // unknown suffix ignored
            }
        }
    }

    let total_exp = expo1 + expsgn * expo2;
    if total_exp == 0 {
        Ok(sign * mantis)
    } else {
        // Use powf to match C's pow(10.0, (double)expo) — NOT powi which
        // uses repeated squaring and rounds differently for some exponents.
        Ok(sign * mantis * 10.0_f64.powf(total_exp as f64))
    }
}

/// Parse `.TF V(node1[,node2]) input_src` or `.TF I(outsrc) input_src`.
/// Returns (TfOutput, input_source_name) or None if unparseable.
fn parse_tf_output_and_src(line: &str) -> Result<Option<(TfOutput, String)>, String> {
    // Skip ".TF" prefix
    let rest = line[3..].trim();
    let upper = rest.to_uppercase();

    if upper.starts_with("V(") || upper.starts_with("V (") {
        // Find the opening paren
        let paren_start = rest.find('(').unwrap();
        let paren_end = rest.find(')').ok_or_else(|| format!("Missing ) in .TF: {line}"))?;
        let inner = rest[paren_start+1..paren_end].trim();

        // Parse node names (possibly comma-separated)
        let (pos_name, neg_name) = if inner.contains(',') {
            let parts: Vec<&str> = inner.split(',').collect();
            (parts[0].trim().to_string(), Some(parts[1].trim().to_string()))
        } else {
            (inner.to_string(), None)
        };

        // Input source is the last token
        let after_paren = rest[paren_end+1..].trim();
        let input_src = after_paren.split_whitespace().next()
            .ok_or_else(|| format!("Missing input source in .TF: {line}"))?
            .to_string();

        Ok(Some((TfOutput::Voltage { pos_name, neg_name }, input_src)))
    } else if upper.starts_with("I(") || upper.starts_with("I (") {
        let paren_start = rest.find('(').unwrap();
        let paren_end = rest.find(')').ok_or_else(|| format!("Missing ) in .TF: {line}"))?;
        let src_name = rest[paren_start+1..paren_end].trim().to_string();

        let after_paren = rest[paren_end+1..].trim();
        let input_src = after_paren.split_whitespace().next()
            .ok_or_else(|| format!("Missing input source in .TF: {line}"))?
            .to_string();

        Ok(Some((TfOutput::Current { src_name }, input_src)))
    } else {
        // Unrecognized .TF format — skip (don't error, other .TF directives may exist)
        Ok(None)
    }
}

/// Parse `.SENS V(node1[,node2])` or `.SENS I(src)`.
fn parse_sens_output(line: &str) -> Result<Option<TfOutput>, String> {
    let rest = line[5..].trim();
    let upper = rest.to_uppercase();

    if upper.starts_with("V(") || upper.starts_with("V (") {
        let paren_start = rest.find('(').unwrap();
        let paren_end = rest.find(')').ok_or_else(|| format!("Missing ) in .SENS: {line}"))?;
        let inner = rest[paren_start+1..paren_end].trim();

        let (pos_name, neg_name) = if inner.contains(',') {
            let parts: Vec<&str> = inner.split(',').collect();
            (parts[0].trim().to_string(), Some(parts[1].trim().to_string()))
        } else {
            (inner.to_string(), None)
        };

        Ok(Some(TfOutput::Voltage { pos_name, neg_name }))
    } else if upper.starts_with("I(") || upper.starts_with("I (") {
        let paren_start = rest.find('(').unwrap();
        let paren_end = rest.find(')').ok_or_else(|| format!("Missing ) in .SENS: {line}"))?;
        let src_name = rest[paren_start+1..paren_end].trim().to_string();

        Ok(Some(TfOutput::Current { src_name }))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_value() {
        assert!((parse_value("1k").unwrap() - 1e3).abs() < 1e-10);
        assert!((parse_value("1K").unwrap() - 1e3).abs() < 1e-10);
        assert!((parse_value("2.2k").unwrap() - 2200.0).abs() < 1e-10);
        assert!((parse_value("1u").unwrap() - 1e-6).abs() < 1e-20);
        assert!((parse_value("10m").unwrap() - 0.01).abs() < 1e-10);
        assert!((parse_value("100n").unwrap() - 1e-7).abs() < 1e-20);
        assert!((parse_value("1MEG").unwrap() - 1e6).abs() < 1e-3);
        assert!((parse_value("3.3").unwrap() - 3.3).abs() < 1e-10);
        assert!((parse_value("1e-6").unwrap() - 1e-6).abs() < 1e-20);
        assert_eq!(parse_value("-3.5").unwrap(), -3.5);
        assert_eq!(parse_value("+100").unwrap(), 100.0);
        assert_eq!(parse_value("2.52e-9").unwrap(), 2.52e-9);
        assert_eq!(parse_value("1e-14").unwrap(), 1e-14);
    }

    /// Verify parse_value matches ngspice INPevaluate bit-for-bit.
    ///
    /// INPevaluate computes: mantis * pow(10, expo), which can differ from
    /// strtod by 1 ULP. This table covers all model parameter values used
    /// across our eval circuits, including the ones where INPevaluate differs
    /// from strtod. Expected bits were computed by simulating INPevaluate in
    /// Python (same FP ops: 10*mantis+digit, then mantis * pow(10, expo)).
    #[test]
    fn test_parse_value_matches_inpevaluate() {
        let cases: &[(&str, u64)] = &[
            // Values where INPevaluate == strtod
            ("100",              0x4059000000000000),
            ("0.5",              0x3fe0000000000000),
            ("0.01",             0x3f847ae147ae147b),
            ("1e-3",             0x3f50624dd2f1a9fc),
            ("1e-6",             0x3eb0c6f7a0b5ed8d),
            ("1e-12",            0x3d719799812dea11),
            ("1e-14",            0x3d06849b86a12b9b),
            ("2.52e-9",          0x3e25a589e1f37f03),
            ("1.752",            0x3ffc083126e978d5),
            ("10e-9",            0x3e45798ee2308c3a),
            ("4e-12",            0x3d919799812dea11),
            ("5e-12",            0x3d95fd7fe1796495),
            ("1.11",             0x3ff1c28f5c28f5c3),
            ("20e-9",            0x3e55798ee2308c3a),
            // Values where INPevaluate differs from strtod by 1 ULP
            ("0.568",            0x3fe22d0e56041894), // strtod: ...1893
            ("0.6",              0x3fe3333333333334), // strtod: ...3333
            ("100e-6",           0x3f1a36e2eb1c432c), // strtod: ...432d
            ("300.15",           0x4072c26666666667), // strtod: ...6666
            ("1.38064852e-23",   0x3b30b0e674035e1b), // strtod: ...5e1a
            ("1.6021766208e-19", 0x3c07a4da25c77014), // strtod: ...7013
            ("8.854214871e-12",  0x3da3787ad765df6d),
            ("0.08333333333",    0x3fb555555551ab15),
        ];
        for &(input, expected_bits) in cases {
            let v = parse_value(input).unwrap();
            assert_eq!(v.to_bits(), expected_bits,
                "parse_value(\"{input}\") = {:016x}, expected {:016x} (INPevaluate)",
                v.to_bits(), expected_bits);
        }
    }

    #[test]
    fn test_parse_single_resistor() {
        let netlist = "\
Single Resistor
V1 in 0 DC 5
R1 in 0 1k
.OP
.END
";
        let result = parse_netlist(netlist).unwrap();
        assert_eq!(result.title, "Single Resistor");
        assert!(matches!(result.analysis, Analysis::Op));
        assert_eq!(result.circuit.num_equations(), 3); // gnd + in + v1#branch
    }

    #[test]
    fn test_parse_rc_tran() {
        let netlist = "\
RC Step
V1 in 0 DC 5
R1 in out 1k
C1 out 0 1u
.TRAN 10u 5m
.END
";
        let result = parse_netlist(netlist).unwrap();
        assert!(matches!(result.analysis, Analysis::Tran { .. }));
        if let Analysis::Tran { step, stop, .. } = result.analysis {
            assert!((step - 1e-5).abs() < 1e-15);
            assert!((stop - 5e-3).abs() < 1e-15);
        }
    }

    #[test]
    fn test_subckt_expansion_simple() {
        let netlist = "\
* .SUBCKT with Parameters
.SUBCKT DIVIDER in out gnd
R1 in out 2k
R2 out gnd 2k
.ENDS DIVIDER

V1 vin 0 DC 10
X1 vin vout 0 DIVIDER
.OP
.END
";
        let lines: Vec<String> = join_continuation_lines(netlist).lines().map(|l| l.to_string()).collect();
        let expanded = expand_subcircuits(&lines).unwrap();
        eprintln!("Expanded lines:");
        for line in &expanded {
            eprintln!("  {}", line);
        }
        // After expansion, there should be no X lines and no .SUBCKT/.ENDS
        assert!(!expanded.iter().any(|l| l.trim().to_uppercase().starts_with("X")),
            "X instances should be expanded");
        assert!(!expanded.iter().any(|l| l.trim().to_uppercase().starts_with(".SUBCKT")),
            ".SUBCKT should be removed");
        // Should have R1 and R2 with translated names
        let result = parse_netlist(netlist).unwrap();
        assert!(matches!(result.analysis, Analysis::Op));
    }

    #[test]
    fn test_subckt_expansion_nested() {
        let netlist = "\
* Nested Subcircuit Test
.SUBCKT inner a b
R1 a b 1k
.ENDS inner

.SUBCKT outer vin vout
X1 vin mid inner
X2 mid vout inner
.ENDS outer

V1 in 0 DC 10
X1 in out outer
RL out 0 1k
.OP
.END
";
        let result = parse_netlist(netlist).unwrap();
        assert!(matches!(result.analysis, Analysis::Op));
        // Should have: V1, RL, plus two R1 instances from the nested expansions
        // Total devices: 1 voltage source + 3 resistors = 4
        assert_eq!(result.circuit.devices.len(), 4,
            "Expected 4 devices (1V + 3R), got {}",
            result.circuit.devices.len());
    }

    #[test]
    fn test_subckt_expansion_vcvs() {
        let netlist = "\
* Op-amp macromodel
.SUBCKT OPAMP inp inn out
E1 out 0 inp inn 100000
.ENDS OPAMP

V1 vinp 0 DC 2.6
V2 vinn 0 DC 2.4
R1 vinp sump 10k
R2 sump out 100k
R3 vinn sumn 10k
R4 sumn ref 100k
V3 ref 0 DC 2.5
X1 sump sumn out OPAMP
.OP
.END
";
        let result = parse_netlist(netlist).unwrap();
        assert!(matches!(result.analysis, Analysis::Op));
    }
}
