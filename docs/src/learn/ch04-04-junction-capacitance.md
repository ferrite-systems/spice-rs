# Junction Capacitance

For DC operating point analysis, the diode is fully described by its I-V curve — the Shockley equation and its linearization. But for transient analysis (circuits with changing signals) and AC analysis (small-signal frequency response), the diode also stores charge. This charge storage shows up as **junction capacitance**.

A real diode has two distinct capacitance mechanisms, each dominant in a different regime.

---

## Depletion capacitance

When a diode is reverse-biased, the depletion region — the zone of immobile charges around the junction — widens. This region behaves like a parallel-plate capacitor: two conductive regions (the P and N sides) separated by an insulating layer (the depleted zone). As the reverse voltage increases, the plates move farther apart and the capacitance decreases.

The SPICE model for depletion capacitance is:

$$C_j = \frac{CJ_0}{\left(1 - V_D / VJ\right)^M}$$

where:

**$CJ_0$** — zero-bias junction capacitance (the capacitance at $V_D = 0$). A physical parameter measured from the device. Typical values: 1-100 pF. In spice-rs: `cjo` (default: 0, meaning no capacitance unless specified).

**$VJ$** — junction potential (also called the built-in potential). Typically around 0.7-0.9 V for silicon. This is the voltage at which the depletion region would theoretically collapse to zero width. In spice-rs: `vj` (default: 1.0 V).

**$M$** — grading coefficient. Determines how rapidly the capacitance changes with voltage. $M = 0.5$ for an abrupt junction (step doping profile), $M = 0.33$ for a linearly graded junction. In spice-rs: `m` (default: 0.5).

As $V_D$ approaches $VJ$ from below, the denominator goes to zero and $C_j$ would go to infinity — a nonphysical singularity. SPICE avoids this by switching to a linear extrapolation above the threshold $FC \cdot VJ$ (where $FC$ is the forward-bias depletion cap coefficient, default 0.5). Above that threshold, the capacitance formula becomes a straight line that smoothly continues from the last valid point, avoiding the singularity.

The charge associated with the depletion capacitance is the integral of $C_j$ with respect to voltage:

$$Q_{dep} = \frac{VJ \cdot CJ_0}{1 - M}\left[1 - \left(1 - \frac{V_D}{VJ}\right)^{1-M}\right]$$

SPICE stores charge (not capacitance) in its state vectors. During transient analysis, the integration method converts charge changes into equivalent currents: $I_{cap} = dQ/dt$.

---

## Diffusion capacitance

When a diode is forward-biased, minority carriers are injected across the junction. These carriers take a finite time to recombine — the **transit time**, $TT$. While they exist, they represent stored charge that is proportional to the forward current:

$$Q_{diff} = TT \cdot I_D$$

The corresponding diffusion capacitance is:

$$C_d = TT \cdot g_d$$

where $g_d$ is the small-signal conductance $dI_D/dV_D$. This makes physical sense: as the current increases, more charge is stored in transit, and the capacitance grows proportionally.

Diffusion capacitance dominates in forward bias. At a forward current of 10 mA with $TT = 10$ ns and $g_d \approx 0.4$ S, the diffusion capacitance is about 4 nF — orders of magnitude larger than the typical depletion capacitance of a few picofarads.

---

## Total junction capacitance

The total charge stored in the diode is:

$$Q_{total} = Q_{dep} + Q_{diff}$$

And the total small-signal capacitance is:

$$C_{total} = C_j + C_d$$

In reverse bias, $C_d \approx 0$ (no forward current, no stored minority carriers), so depletion capacitance dominates. In forward bias, $C_d$ grows exponentially with voltage (through $g_d$) and quickly overwhelms $C_j$.

---

## How spice-rs implements this

In `device/diode.rs`, the capacitance calculation runs only during transient or AC analysis (not for DC operating point, where capacitors are open circuits):

```rust
// From device/diode.rs — inside load(), transient/AC path
if mode.is(MODETRAN) || mode.is(MODEAC) {
    // Depletion charge
    let (deplcharge, deplcap) = if czero > 0.0 {
        if vd < self.t_dep_cap {
            // Below FC*VJ: use the standard formula
            let arg = 1.0 - vd / self.t_jct_pot;
            let sarg = (-self.t_grading * arg.ln()).exp();
            let q = self.t_jct_pot * czero * (1.0 - arg * sarg)
                    / (1.0 - self.t_grading);
            (q, czero * sarg)
        } else {
            // Above FC*VJ: linear extrapolation to avoid singularity
            // ...
        }
    };

    // Diffusion charge
    let diffcharge = self.model.tt * cd;
    let diffcap = self.model.tt * gd;

    // Store total charge in state vector
    states.set(0, so + ST_CAP_CHARGE, diffcharge + deplcharge);

    // Integrate: convert dQ/dt into equivalent current and conductance
    let (geq, _ceq) = ni_integrate(&self.ag, states, capd,
                                    so + ST_CAP_CHARGE, self.order);
    gd += geq;
    cd += states.get(0, so + ST_CAP_CURRENT);
}
```

The `ni_integrate` call is where the numerical integration method (trapezoidal or Gear) converts the stored charge into an equivalent conductance and current source that get added to the diode's companion model. The integration methods are covered in detail in Chapter 9 (Transient Analysis).

The key point for now: the capacitance adds *more* conductance and current to the companion model. In transient simulation, the diode's stamp includes not just the resistive (Shockley) contribution but also the capacitive contribution. The matrix is larger in effect — each device's stamp is richer — but the NR iteration structure is exactly the same.

---

## When capacitance matters

For DC operating point, junction capacitance is irrelevant — nothing is changing, so $dQ/dt = 0$ and the capacitors contribute no current.

For **AC analysis**, the capacitances define the frequency response. A diode used as a varactor (variable capacitor) depends entirely on $C_j(V)$. The transit time $TT$ sets the upper frequency limit for diode switching.

For **transient analysis**, the capacitances determine how fast the diode can switch states. A diode turning off must first remove its stored diffusion charge — this creates the characteristic **reverse recovery** delay, where the diode continues to conduct in reverse for a brief period after the applied voltage reverses.
