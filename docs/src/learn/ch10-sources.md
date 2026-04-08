# Sources & Waveforms

Every circuit needs something to push it out of equilibrium. A battery, a signal generator, a sensor producing a time-varying voltage — without a source, all node voltages are zero and nothing happens. In SPICE, **sources** are the elements that inject energy into the circuit.

Sources come in two fundamental varieties, and understanding the distinction is essential to reading any netlist.

**Independent sources** (V and I elements) produce a voltage or current that depends only on time. A 5V battery, a 1 kHz sine wave, a pulsed clock — these are all independent sources. They are the external stimuli that drive the circuit. During DC analysis, they supply a constant value. During transient analysis, they produce a waveform — a function of time that can be a pulse, a sinusoid, a piecewise-linear signal, or several other shapes. The waveform is evaluated at every timestep to determine the source's current value.

**Dependent sources** (E, G, F, H elements) produce a voltage or current that depends on some other voltage or current *in the same circuit*. An operational amplifier's output voltage depends on the voltage difference at its inputs. A MOSFET's drain current depends on its gate voltage. Dependent sources model these kinds of controlled relationships. They are linear — the output is always a constant gain times the controlling variable — which makes them straightforward to stamp into the MNA matrix.

---

## The plan

This chapter covers each type:

1. **[Independent sources](ch10-01-independent-sources.md)** — DC, AC, and transient waveforms: PULSE, SIN, PWL, and EXP. How each waveform is evaluated and when to use each one.

2. **[Dependent sources](ch10-02-dependent-sources.md)** — The four controlled sources (VCVS, VCCS, CCVS, CCCS), how each stamps into the MNA matrix, and the physical relationships they model.

3. **[Transmission lines](ch10-03-transmission-lines.md)** — The lossless transmission line element, with its characteristic impedance and propagation delay.

By the end, you'll understand every source type in spice-rs — how they generate excitation, how they interact with the MNA system, and how the simulator evaluates their waveforms at each timestep.

<!-- TODO: interactive source gallery — pick a waveform type, adjust parameters, see the time-domain shape and its effect on a simple RC circuit -->
