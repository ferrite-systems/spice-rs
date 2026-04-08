# JFET Circuits

The JFET's depletion-mode behavior -- conducting at zero gate voltage -- gives it a unique role in circuit design. Where MOSFETs and BJTs need bias circuits to turn them on, a JFET can be used with minimal surrounding components.

## The self-biased amplifier

The most common JFET amplifier uses *self-biasing*: a source resistor sets the gate-source voltage without requiring a separate bias supply.

```ferrite-circuit
circuit "Self-Biased JFET Amplifier" {
    node "vdd" label="VDD" rail=#true voltage="15"
    node "gnd" ground=#true
    group "amplifier" topology="common-source" {
        component "J1" type="jfet" role="active-device" {
            port "drain" net="vout"
            port "gate" net="vin"
            port "source" net="vs"
        }
        component "RD" type="resistor" role="drain-load" {
            value "2k"
            port "1" net="vdd"
            port "2" net="vout"
        }
        component "RS" type="resistor" role="source-degeneration" {
            value "500"
            port "1" net="vs"
            port "2" net="gnd"
        }
    }
    node "vout" label="Vout"
}
```

With the gate tied to ground through a large resistor (for DC bias), the current through RS creates a positive voltage at the source. Since $V_G = 0$, this means $V_{GS} = -I_{DS} \cdot R_S$ -- a negative value that partially pinches off the channel. The circuit self-regulates: if $I_{DS}$ increases, $V_{GS}$ becomes more negative, reducing $I_{DS}$ back down.

The operating point sits where the device I-V curve intersects the *bias line*:

$$V_{GS} = -I_{DS} \cdot R_S$$

```text
  IDS
   ^
   |  \  bias line: IDS = -VGS/RS
   |   \
   | ___\____________  VGS = 0 (IDSS)
   |/    \
   |  ____\__________  VGS = -0.5V
   | /     \
   |/ ______\________  VGS = -1.0V
   |  /      * <-- operating point
   | / _______\______  VGS = -1.5V
   |/ /        \
   | /          \
   +-----|-------|---> VGS
       VTO      0
```

The small-signal voltage gain is:

$$A_v = -\frac{g_m R_D}{1 + g_m R_S}$$

If $R_S$ is bypassed with a capacitor (removing AC feedback), the gain becomes simply $-g_m R_D$.

## JFET as a voltage-controlled resistor

In the linear region ($V_{DS}$ small), the JFET behaves as a resistor whose value is controlled by $V_{GS}$:

$$r_{DS} \approx \frac{1}{2\beta(V_{GS} - V_{TO})}$$

This makes the JFET useful as an analog switch or variable attenuator in the signal path. At $V_{GS} = 0$, the resistance is at its minimum. As $V_{GS}$ approaches $V_{TO}$, the resistance rises toward infinity. Automatic gain control (AGC) circuits exploit this property.

## JFET amplifier topologies

The three FET amplifier topologies apply to JFETs just as they do to MOSFETs:

| Topology | Gain | Input Z | Use |
|----------|------|---------|-----|
| Common-source | $-g_m R_D$ | Very high | General amplification |
| Common-drain (source follower) | $\approx 1$ | Very high | Buffer, impedance matching |
| Common-gate | $g_m R_D$ | $1/g_m$ | High-frequency, cascode |

The JFET's advantage over the MOSFET in these configurations is lower noise at low frequencies (no oxide interface means no 1/f noise from interface traps) and higher input impedance than the BJT (gate leakage is picoamps vs microamps of base current). This is why JFET input stages appear in precision instrumentation amplifiers and low-noise preamplifiers.

## In SPICE simulation

SPICE treats JFET amplifier circuits the same as any other: the JFET model from `device/jfet.rs` stamps $g_m$, $g_{ds}$, and junction capacitances into the MNA matrix; the resistors stamp their conductances; Newton-Raphson solves the system. The self-biasing feedback is captured automatically by the nonlinear solve -- the simulator does not need to know that $R_S$ is providing bias feedback. It simply enforces KCL at every node and finds the consistent solution.

The simplicity of JFET circuits, combined with the simplicity of the JFET model, makes them an excellent starting point for learning how SPICE simulation works end-to-end: a small circuit, a compact model, and results you can verify by hand.
