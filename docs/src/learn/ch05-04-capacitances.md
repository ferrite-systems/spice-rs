# Capacitances

Up to this point, we have treated the MOSFET as a purely resistive device: apply voltages, get current. But MOSFETs store charge -- in the gate oxide, in the depletion regions, in the junctions. These stored charges create capacitances that determine how fast the device can switch.

For DC analysis, capacitances do not matter. For transient and AC analysis, they are everything.

## Two families of capacitance

MOSFET capacitances fall into two groups with different physical origins:

```text
          Gate
           |
      +----|----+
      | CGS CGD |     <-- Gate capacitances (Meyer model)
      | CGB     |         charge in the oxide / channel
      |         |
   Source    Drain
      |         |
      +--Bulk---+
         |   |
        CBS CBD         <-- Junction capacitances
                            charge in depletion regions
```

**Gate capacitances** ($C_{GS}$, $C_{GD}$, $C_{GB}$) arise from charge stored in the gate oxide and channel. They depend on the operating region -- in a way that is surprisingly discontinuous.

**Junction capacitances** ($C_{BS}$, $C_{BD}$) arise from the reverse-biased PN junctions between the source/drain diffusions and the substrate. They behave like standard diode junction capacitances.

## The Meyer model for gate capacitances

The Meyer model (1971) partitions the total gate capacitance into three components that vary with operating region. The total gate oxide capacitance is:

$$C_{ox,total} = C_{ox} \cdot W \cdot L$$

where $C_{ox}$ is the oxide capacitance per unit area. This total capacitance is distributed among $C_{GS}$, $C_{GD}$, and $C_{GB}$ depending on the region:

**Cutoff** ($V_{GS} < V_{TH}$):

$$C_{GS} = 0, \quad C_{GD} = 0, \quad C_{GB} = C_{ox,total}$$

All the gate capacitance is between gate and bulk -- there is no channel to couple to.

**Linear** ($V_{DS} < V_{GS} - V_{TH}$):

$$C_{GS} = \frac{C_{ox,total}}{2} \left[ 1 - \left(\frac{V_{GS} - V_{DS} - V_{TH}}{2(V_{GS} - V_{TH}) - V_{DS}}\right)^2 \right]$$

$$C_{GD} = \frac{C_{ox,total}}{2} \left[ 1 - \left(\frac{V_{GS} - V_{TH}}{2(V_{GS} - V_{TH}) - V_{DS}}\right)^2 \right]$$

$$C_{GB} = 0$$

The channel exists and shields the gate from the bulk, so $C_{GB}$ drops to zero. The total oxide capacitance is shared between $C_{GS}$ and $C_{GD}$.

**Saturation** ($V_{DS} \geq V_{GS} - V_{TH}$):

$$C_{GS} = \frac{2}{3} C_{ox,total}, \quad C_{GD} = 0, \quad C_{GB} = 0$$

The channel is pinched off at the drain end, so $C_{GD}$ drops to zero. Two-thirds of the oxide capacitance goes to $C_{GS}$.

<!-- TODO: interactive Meyer capacitance plot -- sweep VGS and VDS, show CGS/CGD/CGB as colored areas that sum to Cox -->

## The discontinuity problem

Look at the transition from linear to saturation: $C_{GD}$ drops abruptly from a nonzero value to zero at $V_{DS} = V_{GS} - V_{TH}$. This discontinuity in capacitance means a discontinuity in charge, which creates a nonphysical spike in current during transient analysis.

```text
  Capacitance
       ^
       |
  Cox  |____
  2/3  |    \  CGS
       |     \_________
       |     
       |     ___
       |    /   \  CGD
       |___/     \______ = 0
       |
       +-----|-----------|-----> VDS
           linear     saturation
                 ^
                 |
           discontinuity!
```

The Meyer model computes capacitances as derivatives of charge with respect to voltage, but it does not guarantee that the underlying charges are continuous across region boundaries. More advanced models (Ward-Dutton, used in BSIM3/4) work with charges directly and take derivatives numerically, avoiding this problem.

For Level 1 through 3, spice-rs uses the Meyer model because it matches the ngspice reference implementation. The charge conservation issue is something to be aware of but not a showstopper for most simulations.

## Junction capacitances

The source-bulk and drain-bulk junctions are reverse-biased PN junctions. Their capacitance follows the standard depletion capacitance formula:

$$C_J(V) = \frac{C_{J0}}{\left(1 - \frac{V}{PB}\right)^{MJ}}$$

where:

| Parameter | Symbol | Meaning |
|-----------|--------|---------|
| CJ | $C_{J0}$ | Zero-bias junction capacitance per unit area |
| PB | $PB$ | Built-in potential (typically 0.8 V) |
| MJ | $MJ$ | Grading coefficient (0.5 for abrupt junction, 0.33 for graded) |

The total junction capacitance for the source is:

$$C_{BS} = C_J \cdot AS + C_{JSW} \cdot PS$$

where $AS$ is the source area, $PS$ is the source perimeter, and $C_{JSW}$ is the sidewall capacitance per unit length. The drain junction $C_{BD}$ has the same form with $AD$ and $PD$.

These parameters come from the technology -- the foundry provides $C_{J0}$, $PB$, $MJ$, and the sidewall equivalents. The designer provides $AS$, $AD$, $PS$, $PD$ in the device instance.

## Why capacitances determine speed

The propagation delay of a CMOS inverter is dominated by the time it takes to charge and discharge the load capacitance through the MOSFET:

$$t_p \approx \frac{C_L \cdot V_{DD}}{I_{DS}}$$

The load capacitance $C_L$ is the sum of the gate capacitances of the driven transistors plus the junction capacitances of the driving transistors plus any wire capacitance. Every picofarad in the model directly translates to picoseconds of delay.

This is why transient simulation cannot ignore capacitances, and why accurate capacitance modeling ($C_{GS}$, $C_{GD}$ especially) is critical for timing analysis.

## In spice-rs

The capacitance computation in `device/mosfet1.rs` follows the Meyer model for gate capacitances and the junction model for $C_{BS}$/$C_{BD}$. During transient analysis, these capacitances produce charge terms ($Q = CV$) that are differentiated by the integration method (trapezoidal or Gear) to produce currents:

$$I_C = \frac{dQ}{dt} \approx \frac{Q(t_n) - Q(t_{n-1})}{\Delta t}$$

These capacitive currents stamp into the MNA matrix alongside the resistive terms. The device load function computes everything in one pass: DC currents, conductances, charges, and capacitive contributions.
