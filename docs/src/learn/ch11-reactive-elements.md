# Reactive Elements

Capacitors and inductors are fundamentally different from resistors. A resistor's current depends on the voltage *right now*. A capacitor's current depends on how fast the voltage is *changing*. An inductor's voltage depends on how fast the current is *changing*. This time-dependence — the involvement of derivatives — is what makes reactive elements reactive, and what makes them the source of nearly all the interesting behavior in analog circuits: oscillation, filtering, energy storage, timing, and resonance.

The three analysis types see reactive elements in three completely different ways:

**DC analysis:** Capacitors are open circuits. Inductors are short circuits. In steady state, nothing is changing — $dV/dt = 0$, $dI/dt = 0$ — so the capacitor passes no current and the inductor drops no voltage. Reactive elements simply vanish from the DC problem.

**AC analysis:** Capacitors and inductors become complex impedances. A capacitor has impedance $Z = 1/(j\omega C)$ — it passes high-frequency signals and blocks low-frequency ones. An inductor has impedance $Z = j\omega L$ — it blocks high-frequency signals and passes low-frequency ones. These frequency-dependent impedances are what make filters, resonators, and frequency-selective networks possible.

**Transient analysis:** This is where the real complexity lives. The simulator must solve the differential equations $I = C\,dV/dt$ and $V = L\,dI/dt$ at every timestep. Since SPICE works with algebraic equations (matrix solves), not differential equations, it must convert these derivatives into algebraic approximations using **numerical integration**. The result is a **companion model**: at each timestep, the capacitor or inductor is replaced by an equivalent conductance plus a current source, whose values depend on the integration method and the solution history.

---

## The plan

This chapter covers reactive elements and their simulation:

1. **[Capacitors](ch11-01-capacitors.md)** — the three faces of a capacitor (DC, AC, transient), the companion model from numerical integration, and initial conditions

2. **[Inductors](ch11-02-inductors.md)** — the dual of the capacitor: short circuit at DC, inductive impedance in AC, flux-based companion model in transient

3. **[Mutual inductors](ch11-03-mutual-inductors.md)** — coupled inductors with coupling coefficient $k$, mutual inductance, and transformer modeling

4. **[Reactive circuits](ch11-04-reactive-circuits.md)** — RLC ringing and coupled inductors as concrete examples, with the math that predicts oscillation frequency and damping

By the end, you'll understand how spice-rs turns the continuous-time behavior of capacitors and inductors into the discrete-timestep companion models that enter the MNA matrix at every transient iteration.

<!-- TODO: interactive energy visualization — show a capacitor and inductor in a circuit, animate the energy sloshing between electric field (capacitor) and magnetic field (inductor) during oscillation -->
