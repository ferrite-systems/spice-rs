# CMOS Circuits

Everything in the previous sections -- regions of operation, Level 1 equations, body effect, capacitances -- comes together in CMOS circuits. Complementary MOS pairs an NMOS and a PMOS to build logic gates that consume almost no static power and switch cleanly between supply rails.

The CMOS inverter is the fundamental circuit. Every other digital gate is a variation on it.

## The CMOS inverter

An NMOS pull-down and a PMOS pull-up, sharing the same gate input:

```ferrite-circuit
circuit "CMOS Inverter" {
    node "vdd" label="VDD" rail=#true voltage="5"
    node "gnd" ground=#true
    group "inverter" topology="cmos-inverter" {
        component "M1" type="nmos" role="cmos-drive" {
            model "NMOS"
            port "drain" net="vout"
            port "gate" net="vin"
            port "source" net="gnd"
        }
        component "M2" type="pmos" role="cmos-pull" {
            model "PMOS"
            port "drain" net="vout"
            port "gate" net="vin"
            port "source" net="vdd"
        }
    }
    group "input" topology="signal-source" {
        component "Vin" type="voltage-source" role="signal-input" {
            value "DC 2.5"
            port "pos" net="vin"
            port "neg" net="gnd"
        }
    }
    node "vout" label="Vout"
}
```

The elegance of CMOS: when the input is low, M1 is off and M2 is on, pulling the output to VDD. When the input is high, M1 is on and M2 is off, pulling the output to ground. In both stable states, no DC current flows from VDD to ground. Power is consumed only during switching -- charging and discharging the load capacitance.

## The transfer characteristic

Sweeping $V_{in}$ from 0 to VDD produces the voltage transfer characteristic (VTC):

```text
  Vout
   ^
  VDD|_________
   |           \
   |            \
   |             \    <-- transition region
   |              \       (both devices in saturation)
   |               \
   |                \________
   0                          VDD
   +-----|-----|----|------|----> Vin
         A     B    C      D
```

**Region A** ($V_{in} < V_{TOn}$): M1 off, M2 on. $V_{out} = V_{DD}$.

**Region B** ($V_{in}$ rising): M1 enters saturation, M2 in linear. Output begins to fall.

**Region C** (both saturated): The steep transition region. Both transistors are in saturation, acting as current sources fighting each other. The slope here determines the *noise margin* of the gate.

**Region D** ($V_{in} > V_{DD} - |V_{TOp}|$): M1 in linear, M2 off. $V_{out} \approx 0$.

The switching threshold ($V_{in}$ where $V_{out} = V_{in}$) is controlled by the relative strengths of M1 and M2. For a symmetric inverter, the NMOS and PMOS are sized so that the switching threshold is at $V_{DD}/2$. Because PMOS mobility is typically 2-3x lower than NMOS mobility, the PMOS width is made 2-3x larger to compensate.

<!-- TODO: interactive VTC plotter -- adjust W/L ratios of NMOS and PMOS, see the transfer characteristic shift -->

## DC analysis in SPICE

To compute the VTC, SPICE performs a *DC sweep*: for each value of $V_{in}$, it solves the nonlinear DC operating point. At each point, Newton-Raphson iterates until the currents through M1 and M2 are consistent with KCL at the output node:

$$I_{DS,M1}(V_{in}, V_{out}) + I_{DS,M2}(V_{in}, V_{out}) = 0$$

Both device models evaluate their current and conductance, stamp into the MNA matrix, and the solver finds $V_{out}$. The device models from [Level 1](ch05-02-level1.md) through [BSIM4](ch05-07-bsim4.md) all plug into this same framework -- a more accurate model simply produces a more accurate VTC.

## Transient analysis: switching delay

When the input transitions from low to high, the NMOS turns on and discharges the output node:

```text
  Vin   ___________
       |
  _____|

  Vout
  _____
       \___________
       |<-- tpHL -->|
```

The fall time depends on:
- M1's saturation current ($\beta_n$, $V_{TOn}$)
- Load capacitance ($C_L$ = gate caps of driven transistors + junction caps + wire caps)
- Supply voltage

$$t_{pHL} \approx \frac{C_L \cdot V_{DD}/2}{I_{DS,sat}}$$

This is where the [capacitance models](ch05-04-capacitances.md) earn their keep. An accurate $C_{GS}$, $C_{GD}$, and $C_J$ from the MOSFET model translates directly to accurate delay prediction.

## The CMOS NAND gate

Complex logic is built by stacking transistors. A 2-input NAND gate uses two series NMOS and two parallel PMOS:

```text
            VDD
           /   \
         M3p   M4p     (parallel PMOS)
           \   /
            |
           Vout
            |
           M2n          (series NMOS)
            |
           M1n
            |
           GND
```

When both inputs are high, both NMOS conduct in series, pulling the output low. When either input is low, the corresponding PMOS pulls the output high. This is De Morgan's theorem implemented in silicon.

The series NMOS stack introduces the [body effect](ch05-03-body-effect.md): M2n's source is not at ground but at the intermediate node, so its threshold rises. To compensate, series NMOS devices are often made wider.

## Power dissipation

CMOS power has three components:

**Dynamic power:** Energy to charge and discharge capacitances:

$$P_{dynamic} = \alpha \cdot C_L \cdot V_{DD}^2 \cdot f$$

where $\alpha$ is the activity factor (fraction of clock cycles where the gate switches) and $f$ is the clock frequency.

**Short-circuit power:** During the input transition, both M1 and M2 are briefly on simultaneously (the transition region of the VTC). Current flows directly from VDD to ground. This is typically 10-15% of dynamic power.

**Leakage power:** Subthreshold current when devices are "off." At advanced nodes, this can rival dynamic power. Accurate subthreshold modeling (which Level 1 cannot do, but [BSIM3](ch05-06-bsim3.md)/[BSIM4](ch05-07-bsim4.md) can) is essential for predicting leakage.

SPICE captures all three power components naturally: dynamic power through capacitance charging/discharging during transient analysis, short-circuit power through the overlap of NMOS and PMOS conduction, and leakage through the subthreshold model. The accuracy of the power prediction depends directly on the accuracy of the device model.

<!-- TODO: interactive CMOS inverter simulation -- pulse input, show Vout waveform, annotate delay and power -->
