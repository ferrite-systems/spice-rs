/// Transient waveform types — port of ngspice vsrcload.c/isrcload.c waveform evaluation.
#[derive(Debug, Clone)]
pub enum Waveform {
    /// DC only (no transient waveform).
    Dc(f64),
    /// PULSE(V1 V2 TD TR TF PW PER)
    Pulse {
        v1: f64,
        v2: f64,
        td: f64,
        tr: f64,
        tf: f64,
        pw: f64,
        per: f64,
    },
    /// SIN(VO VA FREQ TD THETA PHASE)
    Sine {
        vo: f64,
        va: f64,
        freq: f64,
        td: f64,
        theta: f64,
        phase_deg: f64,
    },
    /// PWL(T1 V1 T2 V2 ...)
    Pwl {
        pairs: Vec<(f64, f64)>,
    },
}

impl Waveform {
    /// Evaluate the waveform at time `t`.
    /// For DC OP, call with `t = 0.0`.
    pub fn eval(&self, t: f64, step: f64, final_time: f64) -> f64 {
        match self {
            Waveform::Dc(v) => *v,
            Waveform::Pulse { v1, v2, td, tr, tf, pw, per } => {
                eval_pulse(*v1, *v2, *td, *tr, *tf, *pw, *per, t, step, final_time)
            }
            Waveform::Sine { vo, va, freq, td, theta, phase_deg } => {
                eval_sine(*vo, *va, *freq, *td, *theta, *phase_deg, t, final_time)
            }
            Waveform::Pwl { pairs } => eval_pwl(pairs, t),
        }
    }

    /// Compute the next waveform breakpoint from current time.
    /// Port of vsrcacct.c PULSE case (lines 48-148) breakpoint state machine.
    ///
    /// `min_break` is ngspice's CKTminBreak — used to advance past edges we're
    /// already within tolerance of (vsrcacct.c:110: `atime = time + CKTminBreak`).
    ///
    /// Returns the absolute time of the next waveform edge, or None for DC/SIN.
    pub fn next_breakpoint(&self, time: f64, step: f64, final_time: f64, min_break: f64) -> Option<f64> {
        match self {
            Waveform::Dc(_) => None,
            Waveform::Sine { .. } => None, // SIN has no sharp edges
            Waveform::Pulse { v1: _, v2: _, td, tr, tf, pw, per } => {
                // Apply defaults (same as eval)
                let tr = if *tr != 0.0 { *tr } else { step };
                let tf = if *tf != 0.0 { *tf } else { step };
                let pw = if *pw != 0.0 { *pw } else { final_time };
                let per = if *per != 0.0 { *per } else { final_time };

                let mut t = time - td;
                if per > 0.0 && t >= per {
                    t -= per * (t / per).floor();
                }

                // ngspice vsrcacct.c:110: atime = time + CKTminBreak
                // The offset advances past edges we're already at, preventing
                // repeated registration of the same breakpoint.
                let atime = t + min_break;

                // State machine: find next edge (vsrcacct.c:112-139)
                // Edge DECISION uses atime, but wait is computed from t.
                let wait = if atime < 0.0 {
                    -t // wait for pulse start
                } else if atime < tr {
                    tr - t // wait for end of rise
                } else if atime < tr + pw {
                    tr + pw - t // wait for fall start
                } else if atime < tr + pw + tf {
                    tr + pw + tf - t // wait for end of fall
                } else {
                    per - t // wait for next period
                };

                let bp = time + wait;
                if bp > time && bp <= final_time {
                    Some(bp)
                } else {
                    None
                }
            }
            Waveform::Pwl { pairs } => {
                // Register next PWL corner point (vsrcacct.c:203-217)
                // Use atime = time + min_break to skip corners we're already at
                let atime = time + min_break;
                for &(corner, _) in pairs {
                    if corner > atime {
                        // ngspice vsrcacct.c:207-209:
                        //   VSRCbreak_time = CKTtime + VSRCcoeffs[i] - time;
                        // C evaluates left-to-right: (CKTtime + corner) - time.
                        // With no delay, time == CKTtime, so this is (time + corner) - time.
                        // This gives a DIFFERENT FP result than time + (corner - time)
                        // because the intermediate (time + corner) preserves more precision
                        // when time and corner are of similar magnitude.
                        let bp = (time + corner) - time;
                        return Some(bp);
                    }
                }
                None
            }
        }
    }

    /// Get the DC value (value at t=0 for non-DC waveforms).
    pub fn dc_value(&self) -> f64 {
        match self {
            Waveform::Dc(v) => *v,
            Waveform::Pulse { v1, .. } => *v1,
            Waveform::Sine { vo, va, phase_deg, .. } => {
                *vo + *va * (*phase_deg * std::f64::consts::PI / 180.0).sin()
            }
            Waveform::Pwl { pairs } => {
                if pairs.is_empty() { 0.0 } else { pairs[0].1 }
            }
        }
    }

    /// Set the DC value — used by DC sweep to change the source value.
    /// For Dc waveform, replaces the value directly.
    /// For other waveforms, wraps into Dc (matching ngspice VSRCdcValue override).
    pub fn set_dc_value(&mut self, value: f64) {
        match self {
            Waveform::Dc(v) => *v = value,
            // For non-DC waveforms, ngspice sets VSRCdcValue separately from the
            // waveform function. Since we embed DC in the waveform, replace entirely.
            _ => *self = Waveform::Dc(value),
        }
    }
}

/// PULSE evaluation — port of vsrcload.c:96-164.
/// ngspice defaults: TR=0→step, TF=0→step, PW=0→finalTime, PER=0→finalTime
fn eval_pulse(
    v1: f64, v2: f64, td: f64, tr: f64, tf: f64, pw: f64, per: f64,
    t: f64, step: f64, final_time: f64,
) -> f64 {
    // Apply ngspice defaults for zero parameters (vsrcload.c:105-120)
    let tr = if tr != 0.0 { tr } else { step };
    let tf = if tf != 0.0 { tf } else { step };
    let pw = if pw != 0.0 { pw } else { final_time };
    let per = if per != 0.0 { per } else { final_time };

    let mut time = t - td;

    if time > per {
        let basetime = per * (time / per).floor();
        time -= basetime;
    }

    if time <= 0.0 || time >= tr + pw + tf {
        v1
    } else if time >= tr && time <= tr + pw {
        v2
    } else if time > 0.0 && time < tr {
        v1 + (v2 - v1) * time / tr
    } else {
        // falling edge
        v2 + (v1 - v2) * (time - (tr + pw)) / tf
    }
}

/// SINE evaluation — port of vsrcload.c:167-196.
fn eval_sine(
    vo: f64, va: f64, freq: f64, td: f64, theta: f64, phase_deg: f64,
    t: f64, final_time: f64,
) -> f64 {
    // ngspice default: freq=0 → 1/finalTime (vsrcload.c:185-187)
    let freq = if freq != 0.0 { freq } else { 1.0 / final_time };
    let phase = phase_deg * std::f64::consts::PI / 180.0;
    let time = t - td;

    if time <= 0.0 {
        vo + va * phase.sin()
    } else {
        vo + va * (freq * time * 2.0 * std::f64::consts::PI + phase).sin()
            * (-time * theta).exp()
    }
}

/// PWL evaluation — port of vsrcload.c:318-362.
fn eval_pwl(pairs: &[(f64, f64)], t: f64) -> f64 {
    if pairs.is_empty() {
        return 0.0;
    }
    if t <= pairs[0].0 {
        return pairs[0].1;
    }
    if t >= pairs[pairs.len() - 1].0 {
        return pairs[pairs.len() - 1].1;
    }
    // Linear interpolation between points
    for i in 1..pairs.len() {
        if t <= pairs[i].0 {
            let t0 = pairs[i - 1].0;
            let v0 = pairs[i - 1].1;
            let t1 = pairs[i].0;
            let v1 = pairs[i].1;
            let frac = (t - t0) / (t1 - t0);
            return v0 + frac * (v1 - v0);
        }
    }
    pairs[pairs.len() - 1].1
}
