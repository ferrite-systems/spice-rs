# The BJT

The bipolar junction transistor was the workhorse of electronics for three decades before CMOS took over digital circuits. It remains indispensable in analog design: operational amplifiers, voltage references, bandgap circuits, high-speed ECL logic, and RF front-ends all rely on BJTs.

Where the MOSFET is a voltage-controlled switch, the BJT is a *current-controlled current amplifier*. A small current into the base produces a much larger current between collector and emitter, with a gain ($\beta$) that is remarkably well-controlled by the fabrication process.

## Two flavors

```text
     NPN                    PNP
                            
  C  collector           C  collector
  |                      |
  +--| (arrow out)       +--| (arrow in)
  |                      |
  B  base                B  base
  |                      |
  +--| (arrow out)       +--| (arrow in)
  |                      |
  E  emitter             E  emitter
```

**NPN:** Current flows from collector to emitter, controlled by base current. The emitter arrow points *out* of the device. This is the more common type.

**PNP:** Current flows from emitter to collector, controlled by base current. The emitter arrow points *in*. Complementary to NPN, like PMOS is to NMOS.

The BJT has three terminals: base (B), collector (C), and emitter (E). Unlike the MOSFET, the control terminal (base) draws significant current -- this is the fundamental difference between field-effect and bipolar devices.

## The exponential law

The BJT's collector current follows an exponential relationship with base-emitter voltage:

$$I_C = I_S \cdot \exp\left(\frac{V_{BE}}{V_T}\right)$$

where $I_S$ is the saturation current (typically $10^{-15}$ to $10^{-12}$ A) and $V_T = kT/q \approx 26$ mV at room temperature.

This exponential relationship is extraordinarily precise -- it holds over many decades of current. It gives the BJT its transconductance:

$$g_m = \frac{I_C}{V_T}$$

At $I_C = 1$ mA, $g_m = 38.5$ mS. Compare this to a MOSFET, where achieving the same $g_m$ requires much more current or a very wide device. This intrinsic transconductance advantage is why BJTs dominate in precision analog circuits.

## The four operating regions

Like the MOSFET, the BJT has distinct operating regions:

| Region | BE Junction | BC Junction | Behavior |
|--------|-----------|-----------|----------|
| Forward active | Forward biased | Reverse biased | Normal amplification |
| Reverse active | Reverse biased | Forward biased | Poor amplification (rarely used) |
| Saturation | Forward biased | Forward biased | Both junctions on, $V_{CE} \approx 0.2$ V |
| Cutoff | Reverse biased | Reverse biased | Off |

Forward active is the amplification region -- where the collector current is $\beta$ times the base current. Saturation (not the same meaning as MOSFET saturation) is where the BJT is used as a switch in the "on" state.

## What this chapter covers

1. **[Gummel-Poon Model](ch06-01-gummel-poon.md)** -- The standard SPICE BJT model. Transport current, forward and reverse beta, base charge modulation. How it stamps into the MNA matrix.

2. **[Early Effect](ch06-02-early-effect.md)** -- Output conductance: why $I_C$ is not perfectly constant in forward active mode. The Early voltage and its impact on amplifier gain.

3. **[Parasitics](ch06-03-parasitics.md)** -- Series resistances, junction capacitances, and transit times. The real-world additions to the ideal model.

4. **[Amplifier Circuits](ch06-04-amplifier-circuits.md)** -- The common-emitter amplifier in a live circuit.

## In spice-rs

The BJT device model lives in `device/bjt.rs`. It implements the Gummel-Poon model, which is the standard SPICE BJT model used by virtually all simulators. The model includes:

- Transport current with forward and reverse components
- Base charge modulation (Early effect + high-injection)
- Junction capacitances (depletion + diffusion)
- Series resistances (RB, RE, RC)
- Transit times (TF, TR)

The BJT model is simpler than BSIM3/4 in parameter count but has its own subtleties, particularly in the treatment of base charge and the smooth transitions between operating regions.
