# Level 1: The Shichman-Hodges Model

The Level 1 MOSFET model is the simplest model that a SPICE simulator actually uses. Published by Shichman and Hodges in 1968, it captures the essential physics -- threshold, square-law current, and channel-length modulation -- in four core parameters.

If you understand Level 1, you understand the skeleton that every more complex model builds upon.

## The four core parameters

| Parameter | Symbol | Typical NMOS | Meaning |
|-----------|--------|-------------|---------|
| VTO | $V_{TO}$ | 0.7 V | Threshold voltage |
| KP | $KP$ | 110 $\mu$A/V$^2$ | Transconductance parameter ($\mu_n C_{ox}$) |
| LAMBDA | $\lambda$ | 0.04 V$^{-1}$ | Channel-length modulation |
| W/L | $W/L$ | varies | Width-to-length ratio (geometry) |

The effective gain factor is:

$$\beta = KP \cdot \frac{W}{L}$$

## The equations

**Cutoff** ($V_{GS} \leq V_{TH}$):

$$I_{DS} = 0$$

**Linear** ($V_{GS} > V_{TH}$ and $V_{DS} < V_{GS} - V_{TH}$):

$$I_{DS} = \beta \left[ (V_{GS} - V_{TH}) V_{DS} - \frac{1}{2} V_{DS}^2 \right] (1 + \lambda V_{DS})$$

**Saturation** ($V_{GS} > V_{TH}$ and $V_{DS} \geq V_{GS} - V_{TH}$):

$$I_{DS} = \frac{\beta}{2} (V_{GS} - V_{TH})^2 (1 + \lambda V_{DS})$$

Notice the $(1 + \lambda V_{DS})$ factor. This is channel-length modulation -- the slight increase of drain current with $V_{DS}$ in saturation. Without it ($\lambda = 0$), the saturation current would be perfectly flat. With it, the I-V curves have a small upward slope, and the device has a finite output resistance:

$$r_o = \frac{1}{\lambda I_{DS}}$$

We write $V_{TH}$ rather than $V_{TO}$ because the threshold voltage is modified by the [body effect](ch05-03-body-effect.md). When $V_{BS} = 0$, $V_{TH} = V_{TO}$.

## How Level 1 stamps into the MNA matrix

SPICE does not plug the $I_{DS}$ equation directly into the matrix. Instead, it *linearizes* the device around its current operating point using a Newton-Raphson companion model. The MOSFET becomes a small-signal equivalent circuit:

```text
        Drain
          |
          |
     +----+----+
     |    |    |
    gds  gm   Ieq
     |   Vgs   |
     |    |    |
     +----+----+
          |
        Source
```

Three elements stamp into the matrix:

1. **$g_{ds}$** -- output conductance (drain-source conductance), stamps like a resistor between drain and source
2. **$g_m \cdot V_{gs}$** -- a voltage-controlled current source from drain to source, controlled by $V_{GS}$
3. **$I_{eq}$** -- an equivalent current source that accounts for the difference between the full nonlinear current and the linearized approximation

The partial derivatives are:

**Linear region:**

$$g_m = \frac{\partial I_{DS}}{\partial V_{GS}} = \beta \cdot V_{DS} \cdot (1 + \lambda V_{DS})$$

$$g_{ds} = \frac{\partial I_{DS}}{\partial V_{DS}} = \beta \left[ (V_{GS} - V_{TH}) - V_{DS} \right] (1 + \lambda V_{DS}) + \beta \left[ (V_{GS} - V_{TH}) V_{DS} - \frac{1}{2} V_{DS}^2 \right] \lambda$$

**Saturation region:**

$$g_m = \beta (V_{GS} - V_{TH})(1 + \lambda V_{DS})$$

$$g_{ds} = \frac{\beta}{2} (V_{GS} - V_{TH})^2 \cdot \lambda$$

<!-- TODO: interactive MNA stamping visualization -- show the 4x4 matrix for a single MOSFET circuit, highlight cells as you toggle gm/gds/Ieq -->

## The equivalent current

The equivalent current source ensures that the linearized model produces the correct total current at the current operating point. It is computed as:

$$I_{eq} = I_{DS} - g_m V_{GS} - g_{ds} V_{DS}$$

This is a standard Newton-Raphson trick: we linearize a nonlinear function $f(x)$ at point $x_0$ as $f(x_0) + f'(x_0)(x - x_0)$. The "$f(x_0)$" part becomes the current source; the "$f'(x_0)$" part becomes the conductance.

## In spice-rs

The Level 1 implementation lives in `device/mosfet1.rs`. The core computation follows this structure:

```text
fn load(&mut self, vgs: f64, vds: f64, vbs: f64) {
    // 1. Compute VTH (threshold with body effect)
    // 2. Determine region: cutoff / linear / saturation
    // 3. Compute IDS
    // 4. Compute gm, gds, gmbs
    // 5. Stamp into MNA matrix:
    //    - gm  between (drain,source) controlled by (gate,source)
    //    - gds between (drain,source)
    //    - gmbs between (drain,source) controlled by (bulk,source)
    //    - Ieq current source from drain to source
}
```

The `gmbs` term ($\partial I_{DS} / \partial V_{BS}$) captures the body effect's influence on drain current -- it appears as another voltage-controlled current source, controlled by $V_{BS}$. We cover this in the [body effect](ch05-03-body-effect.md) section.

## What Level 1 gets right and wrong

**Gets right:**
- Basic switching behavior (digital circuits at long channel lengths)
- DC operating point for hand calculations
- Qualitative I-V characteristics
- The structure of MNA stamping that all models share

**Gets wrong:**
- No velocity saturation (real devices saturate at lower $V_{DS}$ than the square law predicts)
- No subthreshold conduction (real devices leak below threshold)
- Poor modeling of short-channel effects (DIBL, hot carriers)
- Capacitances are too simplified (addressed in [Chapter 5.4](ch05-04-capacitances.md))

Level 1 is the pedagogical model: learn here, then graduate to [BSIM3](ch05-06-bsim3.md) for real design work. But every BSIM3 simulation still evaluates region, computes current, takes derivatives, and stamps -- the same four steps, just with more physics inside each one.
