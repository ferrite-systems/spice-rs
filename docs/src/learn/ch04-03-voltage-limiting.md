# Voltage Limiting

Newton-Raphson proposes new voltages by solving a linearized system. The linearization is only accurate near the current operating point, so the proposed voltage might be far from the truth — and for a diode, "far from the truth" can mean catastrophe.

If the solver proposes $V_D = 2$ V when the true answer is around 0.65 V, the Shockley equation requires computing $e^{2/0.026} = e^{77}$, which is about $3 \times 10^{33}$. Multiply by $I_s = 10^{-14}$ and you get a current of $3 \times 10^{19}$ amps — physically absurd, but numerically computable. The resulting conductance $g_d$ is similarly enormous, and the companion model will violently overcorrect on the next iteration.

If the solver proposes $V_D = 20$ V, the exponent is 772 and `exp()` returns infinity. The simulation crashes.

**Voltage limiting** prevents this by clamping the proposed voltage change before the exponential is evaluated. The device accepts the direction of the proposed change but limits its magnitude.

---

## The pnjlim algorithm

The limiting function in SPICE is called `pnjlim` (PN junction limit). It's a small, carefully designed function that has been essentially unchanged since SPICE2 in the 1970s. In spice-rs, it lives in `device/limiting.rs`.

The function takes four inputs:
- `vnew` — the voltage proposed by the solver
- `vold` — the voltage from the previous iteration
- `vt` — the thermal voltage ($nV_t$)
- `vcrit` — a critical voltage threshold

And returns a limited voltage that is safe to feed into `exp()`.

### The critical voltage

The threshold $V_{crit}$ is where limiting kicks in:

$$V_{crit} = nV_t \cdot \ln\!\left(\frac{nV_t}{\sqrt{2} \cdot I_s}\right)$$

For typical values ($n = 1$, $V_t = 0.02585$ V, $I_s = 10^{-14}$ A):

$$V_{crit} = 0.02585 \cdot \ln\!\left(\frac{0.02585}{\sqrt{2} \cdot 10^{-14}}\right) \approx 0.02585 \cdot 29.0 \approx 0.75 \text{ V}$$

This is the voltage at which the diode current starts becoming numerically dangerous. Below $V_{crit}$, the exponential is large but manageable. Above it, unconstrained changes could easily produce overflow.

In spice-rs, $V_{crit}$ is precomputed during temperature setup:

```rust
// From device/diode.rs — temperature()
self.t_vcrit = vte * (vte / (SQRT_2 * self.t_sat_cur)).ln();
```

### The limiting rules

The `pnjlim` function applies three different rules depending on the situation:

**Case 1: Large positive voltage, big change.** If $V_{new} > V_{crit}$ and $|V_{new} - V_{old}| > 2V_t$, the voltage change is compressed using a logarithm. Instead of jumping directly to $V_{new}$, the diode moves by:

$$V_{limited} = V_{old} + V_t \cdot \left(2 + \ln\!\left(\frac{V_{new} - V_{old}}{V_t} - 2\right)\right)$$

This keeps the step in the same direction as the solver wants, but compresses large steps logarithmically. A proposed jump of 5V becomes a step of maybe 0.1V. The solver's intent is preserved (increase the voltage) but the magnitude is tamed.

**Case 2: Negative voltage, big swing.** If $V_{new} < 0$ and the change is large, the voltage is clamped to prevent wild swings into deep reverse bias.

**Case 3: Normal region.** If the change is small or the voltage is below $V_{crit}$, no limiting is applied. The solver's proposal is accepted as-is.

Here is the implementation from spice-rs:

```rust
// From device/limiting.rs — pnjlim
pub fn pnjlim(vnew: f64, vold: f64, vt: f64, vcrit: f64,
              check: &mut bool) -> f64 {
    let mut vnew = vnew;

    if (vnew > vcrit) && ((vnew - vold).abs() > (vt + vt)) {
        // Large positive voltage above critical — log damping
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
        let arg = if vold > 0.0 { -vold - 1.0 } else { 2.0 * vold - 1.0 };
        if vnew < arg {
            vnew = arg;
            *check = true;
        }
    }

    vnew
}
```

---

## The check flag

When limiting is applied, `pnjlim` sets `check = true`. This flag propagates up to the NR loop as the `noncon` (non-convergence) flag. Its meaning: "I had to override the solver's voltage, so the solution isn't self-consistent yet — don't declare convergence."

This is important. If limiting clamps $V_D$ from 5V down to 0.8V, the matrix was solved for 5V but the device is being evaluated at 0.8V. The system is internally inconsistent. Another iteration is needed to reconcile the device state with the node voltages.

In `device/diode.rs`, limiting is applied after reading the solver's proposed voltage and before evaluating the Shockley equation:

```rust
// Apply pnjlim — from device/diode.rs load()
let vd_old = states.get(0, so + ST_VOLTAGE);
vd = pnjlim(vd, vd_old, vte, self.t_vcrit, &mut check);

// ... later, after evaluating the diode equations:
if check {
    *noncon = true;  // signal to NR loop: don't check convergence yet
}
```

---

## Why not just use smaller steps?

You might wonder: why not simply limit the NR step size globally, rather than using a device-specific function? Two reasons:

1. **Only exponential devices need it.** Resistors, capacitors, and voltage sources don't have overflow problems. Limiting them would slow convergence for no benefit.

2. **The limit depends on the device.** A diode with $I_s = 10^{-14}$ has $V_{crit} \approx 0.75$ V. A diode with $I_s = 10^{-6}$ (a Schottky) has $V_{crit} \approx 0.30$ V. Each device knows its own safe range.

This device-level limiting is one of the essential techniques that makes SPICE robust. Without it, simulating any circuit with forward-biased diodes or transistors would be a gamble — sometimes the initial guess lands close enough, sometimes `exp()` returns infinity on the second iteration.

<!-- TODO: interactive pnjlim visualization — show proposed voltage vs. limited voltage, with the I-V curve and vcrit marked -->
