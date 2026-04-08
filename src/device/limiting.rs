/// PN junction voltage limiting — port of ngspice DEVpnjlim (devsup.c:49-84).
///
/// Prevents exponential blow-up during Newton-Raphson iteration by smoothly
/// damping voltage changes that exceed 2*Vt above the critical voltage.
///
/// Returns the limited voltage and sets `check` to true if limiting was applied.
pub fn pnjlim(vnew: f64, vold: f64, vt: f64, vcrit: f64, check: &mut bool) -> f64 {
    let mut vnew = vnew;

    if (vnew > vcrit) && ((vnew - vold).abs() > (vt + vt)) {
        // Large positive voltage above critical — apply log damping
        if vold > 0.0 {
            let arg = (vnew - vold) / vt;
            if arg > 0.0 {
                vnew = vold + vt * (2.0 + (arg - 2.0).ln());
            } else {
                vnew = vold - vt * (2.0 + (2.0 - arg).ln());
            }
        } else {
            vnew = vt * (vnew / vt).ln();
        }
        *check = true;
    } else if vnew < 0.0 {
        // Negative voltage — clamp to prevent excessive swings
        let arg = if vold > 0.0 {
            -vold - 1.0
        } else {
            2.0 * vold - 1.0
        };
        if vnew < arg {
            vnew = arg;
            *check = true;
        } else {
            *check = false;
        }
    } else {
        *check = false;
    }

    vnew
}
