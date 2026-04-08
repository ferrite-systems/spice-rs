//! AC small-signal analysis — port of ngspice acan.c / niaciter.c.
//!
//! Algorithm:
//! 1. Compute DC operating point (CKTop)
//! 2. Set mode to MODEDCOP | MODEINITSMSIG and call CKTload once
//!    to compute small-signal parameters (capacitances, conductances)
//! 3. For each frequency point:
//!    a. Set CKTomega = 2*pi*freq
//!    b. Call CKTacLoad (clear matrix, load complex G+jwC stamps)
//!    c. Factor and solve complex system (NIacIter)
//!    d. Store complex node voltages

use crate::circuit::Circuit;
use crate::config::SimConfig;
use crate::error::SimError;
use crate::mode::*;
use crate::parser::AcSweepType;
use crate::solver::SimState;

/// Result of an AC analysis — complex voltages at each frequency point.
pub struct AcResult {
    /// Frequency values (the x-axis).
    pub frequencies: Vec<f64>,
    /// Real part of node voltages at each frequency point.
    /// values_re[freq_idx][node_eq] = real voltage.
    pub values_re: Vec<Vec<f64>>,
    /// Imaginary part of node voltages at each frequency point.
    pub values_im: Vec<Vec<f64>>,
}

/// Run AC small-signal analysis.
///
/// Port of ACan() from acan.c.
pub fn ac_analysis(
    circuit: &mut Circuit,
    config: &SimConfig,
    sweep_type: AcSweepType,
    num_points: usize,
    fstart: f64,
    fstop: f64,
) -> Result<AcResult, SimError> {
    let num_points = num_points.max(1);

    // Step 1: Compute frequency delta — port of acan.c:82-124
    let freq_delta = match sweep_type {
        AcSweepType::Dec => {
            if fstart <= 0.0 {
                return Err(SimError::Other("AC startfreq <= 0".to_string()));
            }
            if fstop / 10.0 < fstart {
                // start-stop less than a decade apart
                if fstop == fstart {
                    1.0
                } else {
                    (10.0_f64.ln() / num_points as f64).exp()
                }
            } else {
                let num_steps = ((fstop / fstart).log10().abs() * num_points as f64).floor();
                ((fstop / fstart).ln() / num_steps).exp()
            }
        }
        AcSweepType::Oct => {
            if fstart <= 0.0 {
                return Err(SimError::Other("AC startfreq <= 0".to_string()));
            }
            (2.0_f64.ln() / num_points as f64).exp()
        }
        AcSweepType::Lin => {
            if num_points > 1 {
                (fstop - fstart) / (num_points as f64 - 1.0)
            } else {
                0.0
            }
        }
    };

    // Step 2: Compute DC operating point — port of acan.c:140-151
    let mut sim = crate::analysis::dc::dc_operating_point(circuit, config)?;

    // Step 3: Set MODEINITSMSIG and call load once to compute small-signal params.
    // Port of acan.c:189-190:
    //   ckt->CKTmode = MODEDCOP | MODEINITSMSIG
    //   error = CKTload(ckt)
    // This makes devices compute and store their AC parameters (capacitances, gm, etc.)
    let smsig_mode = Mode::new(MODEDCOP | MODEINITSMSIG);
    sim.mna.clear();
    let mut noncon = false;
    for dev in circuit.devices.iter_mut() {
        dev.pre_load(&mut sim.mna, &mut circuit.states, smsig_mode);
    }
    for dev in circuit.devices.iter_mut() {
        dev.load(&mut sim.mna, &mut circuit.states, smsig_mode, 1.0, sim.gmin, &mut noncon)?;
    }

    // Step 4: Frequency tolerance — port of acan.c:236-247
    let freq_tol = match sweep_type {
        AcSweepType::Dec | AcSweepType::Oct => {
            freq_delta * fstop * config.reltol
        }
        AcSweepType::Lin => {
            freq_delta * config.reltol
        }
    };

    // Step 5: Sweep through frequencies — port of acan.c:283-441
    let n = circuit.num_equations() - 1;
    let mut result = AcResult {
        frequencies: Vec::new(),
        values_re: Vec::new(),
        values_im: Vec::new(),
    };

    // The DC OP solve established the pivot ordering. For AC, we reuse that
    // ordering but do a complex refactor at each frequency.
    // Set needs_order=false so solve_complex does complex refactor, not reorder.
    // (The ordering was already done during DC OP.)

    let mut freq = fstart;

    while freq <= fstop + freq_tol {
        let omega = 2.0 * std::f64::consts::PI * freq;

        // CKTacLoad: clear matrix, load all device AC stamps
        sim.mna.clear_complex();

        for dev in circuit.devices.iter_mut() {
            dev.ac_load(&mut sim.mna, &circuit.states, omega)?;
        }

        // NIacIter: factor complex matrix and solve
        sim.mna.solve_complex()?;

        // Swap rhs ↔ rhs_old and irhs ↔ irhs_old (port of niaciter.c:170-172)
        sim.mna.rhs[0] = 0.0;
        sim.mna.irhs[0] = 0.0;
        sim.mna.rhs_old[0] = 0.0;
        sim.mna.irhs_old[0] = 0.0;
        sim.mna.swap_irhs();
        sim.mna.swap_rhs();

        // Store results (from rhs_old which now has the solution)
        result.frequencies.push(freq);
        result.values_re.push(sim.mna.rhs_old[..=n].to_vec());
        result.values_im.push(sim.mna.irhs_old[..=n].to_vec());

        // Increment frequency — port of acan.c:401-438
        match sweep_type {
            AcSweepType::Dec | AcSweepType::Oct => {
                freq *= freq_delta;
                if freq_delta == 1.0 {
                    break;
                }
            }
            AcSweepType::Lin => {
                freq += freq_delta;
                if freq_delta == 0.0 {
                    break;
                }
            }
        }
    }

    Ok(result)
}
