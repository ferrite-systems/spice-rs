# Shockley Equation

The current through an ideal diode is:

$$I_D = I_s\left(e^{V_D / nV_t} - 1\right)$$

This is the **Shockley diode equation**, and it governs every PN junction in SPICE — not just standalone diodes, but also the junctions inside MOSFETs and BJTs. Understanding it here will pay off in every device chapter that follows.

---

## The parameters

**$I_s$ — saturation current.** The tiny reverse-bias leakage current, typically around $10^{-14}$ A for a small silicon diode. This is the current that flows when the diode is reverse-biased — thermally generated carriers drifting across the depleted junction. It's small, but it sets the scale for the entire I-V curve. In the spice-rs `DiodeModel`, this is the `is` parameter (default: `1e-14`).

**$n$ — emission coefficient (ideality factor).** A dimensionless number between 1 and 2. An ideal diode has $n = 1$; real diodes have higher values because of recombination in the depletion region. Most SPICE models default to $n = 1$. Higher $n$ makes the exponential rise more gradual — the diode "turns on" at a slightly higher voltage. In spice-rs: the `n` parameter (default: `1.0`).

**$V_t$ — thermal voltage.** Defined as $V_t = kT/q$, where $k$ is Boltzmann's constant, $T$ is temperature in Kelvin, and $q$ is the electron charge. At room temperature (300.15 K):

$$V_t = \frac{1.38065 \times 10^{-23} \cdot 300.15}{1.60218 \times 10^{-19}} \approx 0.02585 \text{ V} \approx 26 \text{ mV}$$

The thermal voltage is not a model parameter — it's a physical constant that depends only on temperature. It appears in the exponent, so it controls how sharply the I-V curve transitions from "off" to "on." At room temperature, every 26 mV increase in $V_D$ multiplies the current by $e \approx 2.72$.

The product $nV_t$ appears so often that ngspice computes it once and calls it `vte`:

```rust
// From device/diode.rs
let vt = BOLTZMANN_OVER_Q * self.temp;   // kT/q
let vte = self.model.n * vt;             // n * kT/q
```

---

## The I-V curve

The Shockley equation produces a dramatically asymmetric curve:

```text
  I (mA)
  10 ┤                                          ╱
     │                                        ╱
   8 ┤                                      ╱
     │                                    ╱
   6 ┤                                  ╱
     │                                ╱
   4 ┤                              ╱
     │                           ╱╱
   2 ┤                        ╱╱
     │                    ╱╱╱
   0 ┤━━━━━━━━━━━━━━━╱╱╱╱─────────────
     │
  -Is┤ · · · · · · · · · · · · · · ·
     └───┬───┬───┬───┬───┬───┬───┬───→ V_D
       -0.4-0.2  0  0.2 0.4 0.6 0.8 (V)
```

Three regimes are visible:

**Reverse bias** ($V_D < 0$): Current is approximately $-I_s \approx -10^{-14}$ A. Essentially zero. The diode blocks.

**Below turn-on** ($0 < V_D < 0.5$ V): Current is positive but negligibly small. At $V_D = 0.3$ V, $I_D \approx I_s \cdot e^{0.3/0.026} \approx 10^{-14} \cdot 10^5 = 10^{-9}$ A — still just a nanoamp.

**Forward conduction** ($V_D > 0.6$ V): The exponential takes off. At $V_D = 0.65$ V, $I_D \approx 7$ mA. At $V_D = 0.7$ V, $I_D \approx 50$ mA. A 50 mV change produces a 7x change in current. This extreme sensitivity is both what makes diodes useful (as switches and rectifiers) and what makes them challenging to simulate (the solver must track a curve that rises by a factor of $e$ every 26 mV).

---

## Why the -1?

The equation has a $-1$ inside the parentheses: $I_s(e^{V/nV_t} - 1)$. This ensures that at $V_D = 0$, the current is exactly zero (as it must be — no voltage, no current). It also means that in reverse bias, $I_D \to -I_s$ as $V_D \to -\infty$. In practice, the $-1$ only matters when $V_D$ is very small or negative. At forward-bias operating points, $e^{V/nV_t}$ is billions or trillions, and subtracting 1 is negligible.

---

## The exponential challenge

The factor $V_D / nV_t$ in the exponent means that the argument to `exp()` grows at about 38.6 per volt (for $n = 1$). At $V_D = 1$ V, the exponent is 38.6. At $V_D = 2$ V, it's 77.2. At $V_D = 20$ V (which can easily happen during a bad NR iteration), the exponent is 772 — and $e^{772}$ is astronomically larger than what a 64-bit float can represent ($e^{709.8}$ is the maximum).

This is not a theoretical concern. During Newton-Raphson iteration, the solver proposes trial voltages that may be far from the final answer. A single bad step can push $V_D$ to a value where `exp()` returns infinity, which poisons the entire matrix solve.

SPICE handles this with **voltage limiting**, covered in the section after next. But the exponential sensitivity is worth internalizing now: it's the root cause of most diode convergence problems, and the same issue appears in every device with a PN junction.

<!-- TODO: interactive parameter explorer — sliders for Is, n, temperature; I-V curve updates live -->
