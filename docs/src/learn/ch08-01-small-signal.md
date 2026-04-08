# Small-Signal Linearization

AC analysis only works because of a powerful simplification: at the DC operating point, every nonlinear device can be replaced by a *linear* small-signal model.

Think about what the DC operating point gives you. A MOSFET has a specific $V_{GS}$, $V_{DS}$, and $I_D$. A diode has a specific $V_D$ and $I_D$. These voltages and currents define a single point on each device's nonlinear I-V curve. If we now add a *tiny* AC signal on top of these DC values — small enough that we don't move far from that point — the device's response is approximately linear. The I-V curve has a definite slope at that point, and for small perturbations, the curve looks like a straight line.

This is the same linearization idea from Newton-Raphson (Chapter 3), but with a different purpose. In NR, we linearize to find the operating point. In AC analysis, we linearize *at* the operating point to study the circuit's frequency response.

## The diode: one conductance, one capacitance

The simplest case. At the DC operating point, the diode has a forward voltage $V_D$ and current $I_D = I_S(e^{V_D/V_T} - 1)$. The small-signal conductance is the derivative of current with respect to voltage:

$$g_d = \frac{dI_D}{dV_D} = \frac{I_D + I_S}{V_T} \approx \frac{I_D}{V_T}$$

For a diode biased at $I_D = 1\text{ mA}$ with $V_T \approx 26\text{ mV}$: $g_d \approx 38\text{ mS}$, or equivalently a small-signal resistance of about $26\ \Omega$.

The diode also has a *diffusion capacitance* $C_d$ that models the charge stored in the junction when forward-biased. This capacitance is frequency-dependent in the real world, but in the SPICE small-signal model it is computed once at the operating point and treated as a constant.

```text
    Small-signal diode model
    ┌───────────────────────┐
    │                       │
  (+)─── gd ───┬─── Cd ───(-)
    │          │           │
    └──────────┴───────────┘

    gd = dI/dV at operating point
    Cd = diffusion + depletion capacitance
```

Together, $g_d$ and $C_d$ are all that AC analysis needs from the diode. The exponential equation is gone. The device is fully linear.

## The MOSFET: a richer model

A MOSFET in saturation has more small-signal parameters, because it has three terminals (ignoring bulk for a moment) and the relationships between them are more complex.

The core small-signal parameters:

- **$g_m$ (transconductance)** — how much the drain current changes when $V_{GS}$ changes:
  $$g_m = \frac{\partial I_D}{\partial V_{GS}}$$
  This is the gain mechanism. A small voltage wiggle at the gate produces a proportional current wiggle at the drain.

- **$g_{ds}$ (output conductance)** — how much the drain current changes when $V_{DS}$ changes:
  $$g_{ds} = \frac{\partial I_D}{\partial V_{DS}}$$
  In an ideal MOSFET in saturation, $g_{ds} = 0$ (current is independent of $V_{DS}$). In real MOSFETs, channel-length modulation gives a small but nonzero $g_{ds}$.

- **$g_{mbs}$ (body transconductance)** — how the bulk-source voltage modulates drain current via the body effect:
  $$g_{mbs} = \frac{\partial I_D}{\partial V_{BS}}$$

- **Capacitances** — $C_{gs}$, $C_{gd}$, $C_{gb}$, $C_{bs}$, $C_{bd}$. These model the charge stored in the gate oxide and depletion regions. They determine the high-frequency behavior: as frequency increases, current flows through the capacitances rather than being controlled by the transconductance, and the gain rolls off.

```text
    Small-signal MOSFET model (simplified)
                  Cgd
         G ──────┤├────── D
         │                │
         │   ┌──────┐     │
    Cgs ═╤═  │gm*Vgs│    gds
         │   └──┬───┘     │
         │      │         │
         S ─────┴──────── S
```

The equivalent circuit shows why the MOSFET amplifies: a voltage-controlled current source ($g_m \cdot v_{gs}$) at the output, with $g_{ds}$ as a parasitic conductance that limits the gain, and capacitances that limit the bandwidth.

## What linearization looks like in spice-rs

In the code, small-signal parameter computation happens in a special mode. After `dc_operating_point()` finds the operating point, the AC analysis function sets:

```
mode = MODEDCOP | MODEINITSMSIG
```

and calls `load()` on every device once. This tells each device: "you're at the operating point — compute and store your small-signal parameters." The device `load()` function, when it sees `MODEINITSMSIG`, computes all the $g_m$, $g_{ds}$, capacitances, etc. from the DC voltages and currents, and stores them in the device state.

These parameters are then used by `ac_load()` during the frequency sweep. The `ac_load()` function stamps the small-signal conductances into the real part of the complex matrix, and the capacitances into the imaginary part (scaled by $\omega$).

## The key assumption

All of this rests on one assumption: **the AC signal is infinitesimally small.** The linearization is only valid for perturbations small enough that the nonlinear terms (the curvature of the I-V characteristic) are negligible.

This is why AC analysis has no concept of signal amplitude. You specify an AC source magnitude in your netlist (e.g., `AC 1`), but it's a *scaling factor*, not a physical voltage. The circuit is perfectly linear in the AC world — double the input, double the output, always. If you need to know what happens when the signal is large enough to push devices out of their linear range, you need transient analysis.

In practice, AC analysis is remarkably useful precisely because most amplifier circuits are *designed* to operate in their linear range. The small-signal model accurately describes the behavior that the circuit designer intended.

<!-- TODO: interactive small-signal explorer — pick a MOSFET operating point on the Id-Vgs curve, see gm (tangent slope) and the small-signal equivalent circuit update -->
