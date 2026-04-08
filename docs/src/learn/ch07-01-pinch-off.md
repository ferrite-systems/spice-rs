# The Pinch-Off Model

The SPICE JFET model is compact: three core DC parameters, two junction capacitance parameters, and a handful of parasitic resistances. It follows the same three-region structure as the MOSFET but with a critical difference -- the threshold voltage $V_{TO}$ is negative for N-channel devices, meaning the device is *on* at $V_{GS} = 0$.

## The three regions

For an N-channel JFET with $V_{TO} < 0$ (e.g., $V_{TO} = -2$ V):

**Cutoff** ($V_{GS} \leq V_{TO}$):

$$I_{DS} = 0$$

The gate voltage is negative enough to completely pinch off the channel.

**Linear** ($V_{GS} > V_{TO}$ and $V_{DS} < V_{GS} - V_{TO}$):

$$I_{DS} = \beta \left[ 2(V_{GS} - V_{TO}) V_{DS} - V_{DS}^2 \right] (1 + \lambda V_{DS})$$

The channel is open from source to drain. Current increases with $V_{DS}$.

**Saturation** ($V_{GS} > V_{TO}$ and $V_{DS} \geq V_{GS} - V_{TO}$):

$$I_{DS} = \beta (V_{GS} - V_{TO})^2 (1 + \lambda V_{DS})$$

The channel is pinched off at the drain end. Current depends primarily on $V_{GS}$.

## The parameters

| SPICE Parameter | Symbol | Typical N-channel | Meaning |
|----------------|--------|-------------------|---------|
| VTO | $V_{TO}$ | $-2$ V | Pinch-off voltage |
| BETA | $\beta$ | $10^{-4}$ A/V$^2$ | Transconductance coefficient |
| LAMBDA | $\lambda$ | $10^{-2}$ V$^{-1}$ | Channel-length modulation |
| IS | $I_S$ | $10^{-14}$ A | Gate junction saturation current |
| RD | $R_D$ | 0 $\Omega$ | Drain ohmic resistance |
| RS | $R_S$ | 0 $\Omega$ | Source ohmic resistance |

Note that BETA here is the *total* transconductance coefficient, not $KP \cdot W/L$ as in the MOSFET. The JFET model does not separate process parameters from geometry -- BETA is a single lumped parameter.

## Comparing JFET and MOSFET Level 1

The equations are structurally identical, but the operating philosophy is inverted:

```text
  MOSFET (enhancement, NMOS):        JFET (depletion, N-channel):
  
  VTO = +0.7V (positive)             VTO = -2.0V (negative)
  Off at VGS = 0                     On at VGS = 0
  Turn on: raise VGS above VTO       Turn off: lower VGS below VTO
  
  IDS
   ^                                  IDS
   |        ___  VGS=5V               ^
   |      /                            |  ___  VGS=0V (max current)
   |    / /___  VGS=4V                 | /
   |  / /                              |/ /___  VGS=-0.5V
   | / /  ___  VGS=3V                  | /
   |/ /  /                             |/ /___  VGS=-1.0V
   | /  /                              | /
   |/  /                               |/ /___  VGS=-1.5V
   |  /                                | /
   | /  (no curves for VGS < 0.7)      |/      VGS=-2V: cutoff
   +-----> VDS                         +-----> VDS
```

The mapping between parameters:

| MOSFET Level 1 | JFET | Relationship |
|---------------|------|-------------|
| $\frac{KP}{2} \cdot \frac{W}{L}$ | $\beta$ | Same role: current gain factor |
| $V_{TO}$ (positive) | $V_{TO}$ (negative) | Same role: threshold, opposite sign |
| $\lambda$ | $\lambda$ | Identical: channel-length modulation |

## Transconductances

The partial derivatives for the MNA stamps follow the same pattern as MOSFET Level 1:

**Linear region:**

$$g_m = \frac{\partial I_{DS}}{\partial V_{GS}} = 2\beta V_{DS} (1 + \lambda V_{DS})$$

$$g_{ds} = \frac{\partial I_{DS}}{\partial V_{DS}} = 2\beta (V_{GS} - V_{TO} - V_{DS})(1 + \lambda V_{DS}) + \beta [2(V_{GS}-V_{TO})V_{DS} - V_{DS}^2] \lambda$$

**Saturation region:**

$$g_m = 2\beta (V_{GS} - V_{TO})(1 + \lambda V_{DS})$$

$$g_{ds} = \beta (V_{GS} - V_{TO})^2 \lambda$$

These stamp into the MNA matrix exactly as described in the [MOSFET Level 1](ch05-02-level1.md) chapter: $g_m$ as a voltage-controlled current source, $g_{ds}$ as an output conductance, plus an equivalent current source.

## Gate junction capacitances

Unlike the MOSFET (where the gate is insulated), the JFET gate forms a PN junction with the channel. This junction has a depletion capacitance:

$$C_{GS} = \frac{CGS}{(1 - V_{GS}/PB)^{1/2}}$$

$$C_{GD} = \frac{CGD}{(1 - V_{GD}/PB)^{1/2}}$$

where CGS and CGD are the zero-bias capacitances and PB is the built-in potential. These are standard reverse-biased junction capacitances -- the same formula used for BJT and MOSFET junction capacitances.

There is no gate oxide capacitance (no oxide) and no body effect (no separate substrate terminal in the standard model). This simplicity is why the JFET model has so few parameters.

<!-- TODO: interactive JFET I-V plotter -- sweep VDS for several VGS values from 0 down to VTO -->

## In spice-rs

The JFET implementation in `device/jfet.rs` is the most compact device model in the simulator. The load function:

1. Determines the region (cutoff / linear / saturation)
2. Computes $I_{DS}$, $g_m$, $g_{ds}$
3. Computes gate junction charges $Q_{GS}$, $Q_{GD}$
4. Stamps into the MNA matrix

The simplicity of the JFET model makes it an excellent starting point for understanding how *any* device model interfaces with the simulator. The same structure -- region selection, current computation, derivative computation, matrix stamping -- appears in every device, from the JFET's 100 lines to BSIM4's 5000.
