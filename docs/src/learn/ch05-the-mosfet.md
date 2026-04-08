# The MOSFET

The metal-oxide-semiconductor field-effect transistor is the most manufactured object in human history. Every processor, every memory chip, every system-on-chip is built from billions of them. If you understand the MOSFET, you understand the atom of digital logic.

At its heart, a MOSFET is a voltage-controlled valve. Apply a voltage to the *gate*, and current flows between *drain* and *source*. Remove the voltage, and the valve shuts off. That is the entire story of digital switching -- and the beginning of a much richer analog story.

```text
        Gate
         |
    +----|----+
    |  oxide  |
    |         |
 Source    Drain
    |         |
    +---Bulk--+
```

A MOSFET has four terminals: gate, drain, source, and bulk (substrate). The gate is separated from the channel by a thin oxide layer -- this is the "insulator" in the "metal-oxide-semiconductor" sandwich. No DC current flows into the gate. The device is controlled entirely by the electric field that the gate voltage creates across the oxide.

## What this chapter covers

We build understanding in layers, the same way SPICE models evolved historically:

1. **[Three Regions](ch05-01-three-regions.md)** -- The MOSFET operates in cutoff, linear, or saturation depending on its terminal voltages. These three regions are the skeleton that every model hangs from.

2. **[Level 1: Shichman-Hodges](ch05-02-level1.md)** -- The simplest useful MOSFET model. Four parameters, three equations. This is where we see how a device model stamps into the MNA matrix.

3. **[The Body Effect](ch05-03-body-effect.md)** -- When the source is not at the substrate potential, the threshold voltage shifts. A two-parameter correction that matters in every stacked circuit.

4. **[Capacitances](ch05-04-capacitances.md)** -- The gate oxide stores charge. Junction depletion regions store charge. These capacitances determine how fast a MOSFET can switch -- they are everything in transient analysis.

5. **[Level 2 and Level 3](ch05-05-level2-3.md)** -- Velocity saturation, narrow-channel effects, improved mobility models. The bridge between textbook equations and silicon reality.

6. **[BSIM3v3](ch05-06-bsim3.md)** -- The industry-standard model for sub-micron MOSFETs. 150+ parameters capturing physics that Level 1 cannot see: DIBL, channel-length modulation, quantum effects.

7. **[BSIM4](ch05-07-bsim4.md)** -- BSIM3's successor, extending to deep sub-micron and FinFET. A brief overview.

8. **[CMOS Circuits](ch05-08-cmos-circuits.md)** -- Putting it all together: the CMOS inverter, logic gates, and how SPICE simulates them.

## The two flavors

MOSFETs come in two complementary types:

- **NMOS** (N-channel): turns on when $V_{GS} > V_{TO}$ (gate positive relative to source). Electrons carry current.
- **PMOS** (P-channel): turns on when $V_{GS} < V_{TO}$ (gate negative relative to source). Holes carry current.

Every equation in this chapter is written for NMOS. For PMOS, flip the sign of all voltages and currents. SPICE handles this internally -- in spice-rs, the `MosfetType` enum carries the polarity factor that gets multiplied through the equations.

## In spice-rs

The MOSFET device models live in these source files:

| Model | File | Lines | Parameters |
|-------|------|-------|------------|
| Level 1 | `device/mosfet1.rs` | ~400 | 4 core |
| Level 2 | `device/mosfet2.rs` | ~800 | 20+ |
| Level 3 | `device/mosfet3.rs` | ~700 | 20+ |
| BSIM3v3 | `device/bsim3.rs` | ~2700 | 150+ |
| BSIM4 | `device/bsim4.rs` | ~5000 | 200+ |

Each model implements the same trait -- it computes drain current, transconductances, and capacitance charges given the terminal voltages, then stamps those into the MNA matrix. The progression from Level 1 to BSIM4 is not a change in architecture; it is a refinement of the physics inside the same computational structure.

Let's begin with the three regions of operation.
