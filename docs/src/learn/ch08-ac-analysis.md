# AC Analysis

AC analysis answers the question: *how does this circuit respond to signals at different frequencies?*

Every amplifier has a bandwidth. Every filter has a rolloff. Every feedback loop has a phase margin. AC analysis is how you see these things — it sweeps across frequency, from hertz to gigahertz, and tells you the gain and phase shift at every point along the way.

The key insight is that AC analysis doesn't simulate a time-domain signal. It never applies a sine wave and watches the output. Instead, it takes a shortcut: *linearize everything at the DC operating point, then solve the resulting linear system at each frequency.* Because the linearized circuit is a purely linear problem, there's no Newton-Raphson iteration, no convergence concerns, no timestep control. Just one matrix solve per frequency point — and each solve is complex-valued, because impedances at nonzero frequency have both magnitude and phase.

This is what makes AC analysis fast. A transient simulation of a 10 MHz sine wave through an amplifier might need thousands of timesteps. AC analysis gets the same frequency response information by solving a few hundred linear systems.

The cost of this elegance is a strong assumption: **the signal is small enough that the linearization is valid.** AC analysis cannot tell you about clipping, distortion, or any behavior that depends on the signal amplitude. It describes the circuit's response to infinitesimally small perturbations around the DC operating point. For large signals, you need transient analysis (Chapter 9).

---

## How it works

AC analysis proceeds in three stages:

1. **Find the DC operating point.** Run the full nonlinear `.OP` solver from Chapter 3. This is the most expensive part of an AC analysis, and it only happens once.

2. **Linearize.** At the operating point, replace every nonlinear device with its small-signal equivalent: conductances and capacitances that describe the device's local behavior. A MOSFET becomes $g_m$, $g_{ds}$, and a handful of capacitances. A diode becomes $g_d$ and $C_d$. The entire circuit is now linear.

3. **Sweep frequency.** For each frequency $f$, set $\omega = 2\pi f$ and solve the complex MNA system $(G + j\omega C)\mathbf{x} = \mathbf{b}$. The result is a complex voltage at every node — magnitude tells you the gain, angle tells you the phase.

In spice-rs, these three stages map directly to `ac_analysis()` in [`analysis/ac.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/analysis/ac.rs): DC operating point (step 2), small-signal parameter computation with `MODEINITSMSIG` (step 3), and the frequency sweep loop (step 5).

---

## The plan

This chapter builds up AC analysis piece by piece:

1. **[Small-signal linearization](ch08-01-small-signal.md)** — how nonlinear devices become linear small-signal models at the operating point, and what those models look like for diodes and MOSFETs

2. **[Complex impedance and the AC matrix](ch08-02-complex-impedance.md)** — how capacitors and inductors introduce frequency dependence, and why the MNA matrix becomes complex

3. **[Bode plots](ch08-03-bode-plots.md)** — reading frequency response: magnitude in decibels, phase in degrees, and the three sweep types (DEC, OCT, LIN)

4. **[Filter circuits](ch08-04-filter-circuits.md)** — RC low-pass and RLC resonance as concrete examples, with the math that predicts their behavior

By the end, you'll understand what spice-rs computes during `.AC` — and why a single DC operating point plus a frequency sweep is enough to characterize a circuit's small-signal behavior across the entire spectrum.

<!-- TODO: interactive frequency response widget — show a simple RC circuit, sweep a frequency slider, watch the phasor rotate and shrink -->
