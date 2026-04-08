# The Early Effect

In the ideal BJT, collector current depends only on $V_{BE}$. Change $V_{CE}$ all you want -- $I_C$ stays the same. But real BJTs show a slight increase of $I_C$ with $V_{CE}$. This is the *Early effect*, and it has a direct impact on amplifier gain.

## The physical picture

The collector-base junction has a depletion region. When $V_{CE}$ increases (making $V_{CB}$ more positive for NPN), this depletion region widens, eating into the neutral base:

```text
  V_CE small:                    V_CE large:
                                 
  E     Base      C              E    Base     C
  |  |==========|  |             |  |========|    |
  |  |==========|  |             |  |========|    |
  |  |==========|  |             |  |========|    |
  |  |   W_B    |  |             |  | W_B'   |    |
                                      ^       ^
                                      |  wider depletion
                                      shorter base
```

A shorter effective base width means:
- Fewer carriers recombine as they cross the base
- More carriers reach the collector
- $I_C$ increases slightly

This is base-width modulation -- the physical mechanism behind the Early effect.

## The Early voltage

If you extrapolate the $I_C$ vs $V_{CE}$ curves backward, they converge at a single point on the negative $V_{CE}$ axis. This intercept is the *forward Early voltage* $V_{AF}$ (parameter VAF in SPICE):

```text
  IC
   ^
   |                        ___/  VBE = 0.72V
   |                   ___/
   |              ___/      ___/  VBE = 0.70V
   |         ___/      ___/
   |    ___/      ___/      ___/  VBE = 0.68V
   |___/     ___/      ___/
   |    ___/      ___/
   |___/     ___/
   +___/ ___/
   |___/
   |
---+----|-----|------|------|-----> VCE
 -VAF   0    0.2         5V
   ^
   |
   Early voltage
   (all curves extrapolate here)
```

The modified collector current in forward active is:

$$I_C = I_S \exp\left(\frac{V_{BE}}{V_T}\right) \left(1 + \frac{V_{CE}}{V_{AF}}\right)$$

This is the BJT equivalent of the MOSFET's channel-length modulation factor $(1 + \lambda V_{DS})$.

| Parameter | Symbol | Typical NPN | Meaning |
|-----------|--------|-------------|---------|
| VAF | $V_{AF}$ | 100 V | Forward Early voltage |
| VAR | $V_{AR}$ | 20 V | Reverse Early voltage |

A larger $V_{AF}$ means flatter I-V curves and higher output resistance. Typical values range from 20 V (lateral PNP) to 200 V (high-performance NPN).

<!-- TODO: interactive Early voltage visualization -- draw I-V family, show extrapolation lines converging at -VAF -->

## Output resistance

The Early effect gives the BJT a finite output resistance:

$$r_o = \frac{V_{AF} + V_{CE}}{I_C} \approx \frac{V_{AF}}{I_C}$$

This is the inverse of the output conductance $g_o$ that stamps into the MNA matrix:

$$g_o = \frac{1}{r_o} = \frac{I_C}{V_{AF} + V_{CE}}$$

## Why it matters for amplifier gain

The voltage gain of a common-emitter amplifier is:

$$A_v = -g_m \cdot (R_C \| r_o)$$

where $R_C$ is the collector load resistance and $r_o$ is the output resistance from the Early effect. If $r_o$ is infinite (ideal BJT), the gain is simply $-g_m R_C$. But in practice, $r_o$ limits the maximum achievable gain.

For a BJT biased at $I_C = 1$ mA with $V_{AF} = 100$ V:

$$g_m = \frac{1 \text{ mA}}{26 \text{ mV}} = 38.5 \text{ mS}$$

$$r_o = \frac{100 \text{ V}}{1 \text{ mA}} = 100 \text{ k}\Omega$$

The intrinsic gain (gain into $r_o$ alone, without any external load) is:

$$A_v = g_m \cdot r_o = 38.5 \times 10^{-3} \times 100 \times 10^3 = 3850$$

This intrinsic gain of nearly 4000 is one reason BJTs excel in precision analog circuits. A MOSFET at the same current achieves an intrinsic gain of perhaps 20-50 (lower $g_m$ and lower $r_o$).

## In the Gummel-Poon model

The Early effect enters through the base charge factor $q_b$:

$$q_1 = 1 + \frac{V_{BE}}{V_{AF}} + \frac{V_{BC}}{V_{AR}}$$

In forward active operation ($V_{BC}$ is negative for NPN), the $V_{BC}/V_{AR}$ term makes $q_1 < 1$ for large $|V_{BC}|$, which increases $I_T = I_S/q_b \cdot \exp(V_{BE}/V_T)$. This is mathematically equivalent to the $(1 + V_{CE}/V_{AF})$ factor but is expressed in terms of junction voltages.

The Gummel-Poon formulation is more general because it handles both forward and reverse Early effects in a single unified framework, and it interacts correctly with the high-injection terms in $q_2$. The simple $(1 + V_{CE}/V_{AF})$ approximation is what you use for hand calculations; the full $q_b$ formulation is what the simulator computes.

## Reverse Early voltage

$V_{AR}$ (VAR) models the same effect for reverse operation -- when the emitter-base depletion region modulates the base width. It is typically much smaller than $V_{AF}$ (10-20 V vs 50-200 V) because the emitter is more heavily doped, making the depletion region extend further into the base for a given voltage change.

In forward active operation, $V_{AR}$ has a subtle effect: it slightly modulates $q_1$ through the $V_{BE}/V_{AF}$ term. In saturated operation (both junctions forward biased), both Early voltages contribute to the base charge.
