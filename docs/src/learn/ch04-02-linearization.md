# Linearization & Companion Models

At each Newton-Raphson iteration, every nonlinear device must be replaced by a linear equivalent that can be stamped into the MNA matrix. For the diode, this means turning the exponential I-V curve into a straight line — a **companion model** consisting of a conductance in parallel with a current source.

---

## The tangent-line approximation

Suppose we're at iteration $k$, and the current guess for the diode voltage is $V_D^{(k)}$. We evaluate the Shockley equation to get the current:

$$I_D^{(k)} = I_s\left(e^{V_D^{(k)} / nV_t} - 1\right)$$

and take its derivative with respect to $V_D$ to get the conductance (slope of the I-V curve at this point):

$$g_d^{(k)} = \frac{dI_D}{dV_D}\bigg|_{V_D^{(k)}} = \frac{I_s}{nV_t} \cdot e^{V_D^{(k)} / nV_t}$$

These two numbers — the current and its derivative — define a tangent line to the I-V curve at the operating point $(V_D^{(k)}, I_D^{(k)})$:

$$I \approx I_D^{(k)} + g_d^{(k)} \cdot (V - V_D^{(k)})$$

This tangent line is exact at $V = V_D^{(k)}$ and approximately correct nearby. It can be rearranged into a form that looks like a conductance in parallel with a current source:

$$I = g_d^{(k)} \cdot V + I_{eq}^{(k)}$$

where

$$I_{eq}^{(k)} = I_D^{(k)} - g_d^{(k)} \cdot V_D^{(k)}$$

```text
  I (mA)
  10 ┤                                 ╱   ╱
     │                               ╱   ╱ I-V curve
   8 ┤                             ╱  ╱╱
     │                           ╱╱╱╱
   6 ┤                       ╱╱╱╱
     │                   ·╱╱╱··
   4 ┤                 ╱╱·          ← tangent line at V_D^(k)
     │              ╱╱·               slope = g_d
   2 ┤           ╱·╱
     │        ╱· ╱
   0 ┤━━━╱━·━╱──────────
     │ ╱ ·╱
     │· ╱       I_eq = intercept of tangent line
     └───┬───┬───┬───┬───┬───┬───┬───→ V_D
        0  0.1 0.2 0.3 0.4 0.5 0.6  (V)
```

The tangent line crosses the true I-V curve at the operating point. To the left, it overestimates the current (the curve is concave up, so the tangent sits above it). To the right, it underestimates. This systematic error is what Newton-Raphson corrects by re-linearizing at each iteration.

<!-- TODO: interactive tangent-line visualization — drag point along I-V curve, see companion model (g_d, I_eq) update, show how tangent intersects the load line -->

---

## The companion circuit

The companion model is a Norton equivalent: a conductance $g_d$ in parallel with a current source $I_{eq}$.

```
        ── b ──
        |      |
      [g_d]  [I_eq]
        |      |
        ── gnd ──
```

This is what the diode looks like to the matrix solver at iteration $k$. It's a linear circuit — a resistor and a current source — so it stamps into the MNA matrix using the patterns from Chapter 2:

**Conductance stamp** ($g_d$ between node $b$ and ground):
$$G[b,b] \mathrel{+}= g_d^{(k)}$$

**Current source stamp** ($I_{eq}$ from ground into node $b$):
$$\text{RHS}[b] \mathrel{-}= I_{eq}^{(k)}$$

(The sign convention: $I_{eq} = I_D - g_d \cdot V_D$ is typically negative when the diode is forward-biased, so subtracting it from the RHS adds a positive contribution — current flowing into the node from the diode.)

In spice-rs, this is the stamping section of `device/diode.rs`:

```rust
// From device/diode.rs — load() function
let cdeq = cd - gd * vd;   // Norton equivalent current

// RHS stamps
mna.stamp_rhs(n, cdeq);    // positive at cathode (neg node)
mna.stamp_rhs(pp, -cdeq);  // negative at anode (pos_prime node)

// Conductance stamps
mna.stamp(pp, pp, gd);
mna.stamp(n, n, gd);
mna.stamp(n, pp, -gd);
mna.stamp(pp, n, -gd);
```

The four conductance stamps follow the standard 2x2 pattern: positive on the diagonal, negative on the off-diagonal. This is identical to a resistor stamp with conductance $g_d$ — because at this iteration, the linearized diode *is* a resistor (plus a current source).

---

## Why it works

The companion model is exact at the operating point: if you plug $V = V_D^{(k)}$ into the companion model, you get $I = I_D^{(k)}$ — exactly the current the real diode would produce. The MNA solver then finds the node voltage that satisfies KCL for the entire circuit, using this linearized diode. The resulting voltage $V_D^{(k+1)}$ is generally not equal to $V_D^{(k)}$ — meaning the linearization point was slightly wrong — but it's *closer* to the true solution.

At the next iteration, the diode is re-linearized at $V_D^{(k+1)}$, producing a better companion model. The process converges because the tangent-line approximation gets better as we approach the true operating point, and once we're close, quadratic convergence kicks in.

---

## A critical detail: every device does this

The diode companion model — conductance + current source — is not specific to diodes. Every nonlinear device in SPICE follows exactly the same pattern:

1. Evaluate the device equations at the current operating point
2. Compute the conductance (partial derivative of current with respect to voltage)
3. Compute the Norton equivalent current source
4. Stamp both into the matrix

For a MOSFET, the "current" is the drain current $I_{DS}$, and the "conductances" include $g_m$ (transconductance, $\partial I_{DS}/\partial V_{GS}$), $g_{ds}$ (output conductance, $\partial I_{DS}/\partial V_{DS}$), and $g_{mbs}$ (body transconductance). The MOSFET has more terminals and more partial derivatives, but the stamping mechanism is identical.

This is the power of MNA + Newton-Raphson: every device, no matter how complex, reduces to conductances and current sources at each iteration.
