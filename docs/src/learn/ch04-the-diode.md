# The Diode

The diode is the simplest nonlinear device in SPICE, and therefore the best place to understand how all nonlinear devices work.

A diode does one thing: it lets current flow easily in one direction and blocks it in the other. A silicon diode forward-biased at 0.65V might pass milliamps of current. Reverse the polarity and it passes essentially nothing — a few picoamps of leakage. This asymmetry, described by an exponential equation, is what makes the diode nonlinear and what makes it interesting for simulation.

This chapter covers the diode model from the ground up:

1. **The Shockley equation** — the I-V relationship, and the physics behind each parameter
2. **Linearization** — how the diode becomes a conductance + current source at each NR iteration
3. **Voltage limiting** — how SPICE prevents the exponential from blowing up during iteration
4. **Junction capacitance** — the charge-storage effects that matter for transient and AC analysis
5. **Circuits** — half-wave and bridge rectifiers, simulated end to end

Every technique introduced here — linearization, companion models, voltage limiting — applies directly to MOSFETs, BJTs, and every other semiconductor device in SPICE. The diode just makes them easier to see.

<!-- TODO: interactive I-V curve — drag a point along the curve, see the companion model (tangent line + current source) update in real time -->
