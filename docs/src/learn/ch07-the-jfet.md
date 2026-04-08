# The JFET

The junction field-effect transistor is the oldest type of FET, predating the MOSFET by several years. It is a *depletion-mode* device: it conducts with zero gate voltage and is turned off by applying a reverse bias to the gate. This is the opposite of the MOSFET, which is normally off and must be turned on.

JFETs occupy a small but important niche. They appear in the input stages of precision op-amps (extremely high input impedance with low noise), in voltage-controlled resistors, and in discrete RF amplifiers. They are simpler than MOSFETs to model -- the SPICE JFET model has fewer than 10 core parameters.

## The physical structure

```text
  N-channel JFET:
  
  Source           Drain
    |     N-type     |
    |   channel      |
    +================+
    |################|  <-- P-type gate (depletion region)
    |################|
    +----Gate--------+

  VGS = 0: channel fully open, maximum current
  VGS < 0: depletion region widens, channel narrows
  VGS = VTO: channel pinched off, no current
```

The gate is a PN junction formed directly against the channel. Reverse-biasing the gate ($V_{GS} < 0$ for N-channel) widens the depletion region, squeezing the channel and reducing current. At $V_{GS} = V_{TO}$ (the pinch-off voltage, which is *negative* for N-channel), the channel is completely pinched off.

No oxide layer. No insulator. The gate is a reverse-biased diode -- which means the gate draws a tiny leakage current (picoamps at room temperature), but far less than a BJT's base current.

## The two types

| Type | Channel | VTO | Turn-off voltage |
|------|---------|-----|-----------------|
| N-channel | N-type | Negative (e.g., $-2$ V) | $V_{GS} < V_{TO}$ |
| P-channel | P-type | Positive (e.g., $+2$ V) | $V_{GS} > V_{TO}$ |

N-channel JFETs are far more common than P-channel, just as NMOS is more common than PMOS.

## The square-law model

The JFET current equation in saturation has a familiar form:

$$I_{DS} = \beta (V_{GS} - V_{TO})^2 (1 + \lambda V_{DS})$$

This is structurally identical to the MOSFET Level 1 saturation equation. The JFET model is effectively a three-parameter device:

| Parameter | Symbol | Meaning |
|-----------|--------|---------|
| VTO | $V_{TO}$ | Pinch-off voltage (negative for N-channel) |
| BETA | $\beta$ | Transconductance coefficient |
| LAMBDA | $\lambda$ | Channel-length modulation |

## What this chapter covers

1. **[The Pinch-Off Model](ch07-01-pinch-off.md)** -- The complete JFET equations: linear region, saturation, and the parameters. How it compares to MOSFET Level 1.

2. **[JFET Circuits](ch07-02-jfet-circuits.md)** -- Brief notes on JFET amplifier configurations.

## In spice-rs

The JFET model lives in `device/jfet.rs`. It is one of the simplest device models in the simulator -- conceptually a stripped-down MOSFET model with junction gate capacitances instead of oxide capacitances. The load function computes drain current, transconductances ($g_m$, $g_{ds}$), gate junction charges, and stamps into the MNA matrix, following the same pattern as every other device.
