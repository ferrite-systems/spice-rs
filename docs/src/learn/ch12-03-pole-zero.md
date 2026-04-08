# Pole-Zero Analysis

Pole-zero analysis (`.PZ`) finds the **poles** and **zeros** of a circuit's transfer function in the complex $s$-plane. This is the most mathematically involved analysis in SPICE, but the physical insight it provides is profound: the poles and zeros completely determine a circuit's frequency response and stability.

```text
SPICE syntax:

.PZ V(out) GND Vin GND CUR POL    * find poles
.PZ V(out) GND Vin GND CUR ZER    * find zeros
.PZ V(out) GND Vin GND CUR PZ     * find both
```

---

## What are poles and zeros?

Any linear circuit's transfer function can be written as a ratio of polynomials in the complex frequency variable $s = \sigma + j\omega$:

$$H(s) = K \cdot \frac{(s - z_1)(s - z_2)\cdots(s - z_m)}{(s - p_1)(s - p_2)\cdots(s - p_n)}$$

The **zeros** $z_1, z_2, \ldots$ are the values of $s$ where the transfer function is zero — the output vanishes completely. The **poles** $p_1, p_2, \ldots$ are the values of $s$ where the transfer function is infinite — the circuit's response blows up.

Each pole and zero is a complex number with a real part and an imaginary part. They come in conjugate pairs when the circuit has real-valued components (which it always does in practice): if $p = \sigma + j\omega$ is a pole, then $p^* = \sigma - j\omega$ is also a pole.

---

## The physical meaning of poles

A pole at $s = \sigma + j\omega$ corresponds to a **natural mode** of the circuit — a way the circuit can oscillate or decay on its own, without any input. The real part $\sigma$ determines the decay rate, and the imaginary part $\omega$ determines the oscillation frequency.

```text
  The s-plane:

  jω
   ↑
   │     ×          × = pole
   │     ·
   │     ·          Real part σ < 0:
  ─┼─────·──────→ σ   decaying (stable)
   │     ·
   │     ×          Real part σ > 0:
   │                   growing (unstable)
```

**Left-half-plane poles** ($\sigma < 0$): The natural mode decays over time. The circuit is stable. The more negative $\sigma$ is, the faster the decay — the "faster" the pole. A purely real pole at $s = -1/\tau$ produces an exponential decay with time constant $\tau$. A complex pair at $s = -\sigma \pm j\omega$ produces a decaying oscillation (ringing).

**Right-half-plane poles** ($\sigma > 0$): The natural mode *grows* over time. The circuit is **unstable** — it oscillates with increasing amplitude until something limits it (clipping, power supply rails, or physical destruction). Any right-half-plane pole means the design is broken.

**Imaginary-axis poles** ($\sigma = 0$): The natural mode neither grows nor decays. This is the boundary — a sustained oscillation. Practical oscillators are designed to have poles very close to the imaginary axis.

### Connecting poles to frequency response

Each pole creates a -20 dB/decade rolloff in the magnitude response at frequencies above the pole's natural frequency. Each zero creates a +20 dB/decade rise. The frequency response you see in a Bode plot is completely determined by the locations of the poles and zeros:

- A dominant pole at low frequency creates the amplifier's bandwidth limit.
- A pair of complex conjugate poles creates a resonance peak (from the RLC circuits of Chapter 11).
- A zero can cancel a pole, creating a flat region in the response.

---

## The physical meaning of zeros

A zero at $s = z$ is a frequency where the output is exactly zero — complete cancellation. Physically, this happens when two signal paths through the circuit produce equal and opposite contributions at the output.

For example, in a bridged-T notch filter, the signal reaches the output through both a direct path and a feedback path. At the notch frequency, these paths cancel perfectly — the transfer function has a zero there, and the output is zero regardless of the input amplitude.

---

## How SPICE finds them

Finding poles and zeros is harder than it might seem. The transfer function is defined implicitly by the circuit's MNA matrix — you don't have an explicit polynomial to factor. The poles are values of $s$ where the determinant of the MNA matrix (with frequency-dependent elements evaluated at $s$) is zero.

The algorithm in spice-rs, ported from ngspice's `cktpzstr.c` and `nipzmeth.c`, uses an iterative root-finding approach:

1. **Setup.** Solve the DC operating point and compute small-signal parameters. Build a separate MNA system for the PZ analysis.

2. **Initial search.** Start from a real initial guess on the negative real axis and use a logarithmic search strategy to bracket roots.

3. **Refinement.** Use a variant of Muller's method (a quadratic interpolation root-finder that works with complex numbers) to converge on each root.

4. **Deflation.** After finding a root, deflate it out of the determinant so the next iteration finds a different root.

5. **Repeat.** Continue until no more roots are found within the search region.

The implementation handles both real poles (on the negative real axis) and complex conjugate pairs (which require tracking both the real and imaginary parts simultaneously). It's the most algorithmically complex analysis in spice-rs — the source in [`src/analysis/pz.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/analysis/pz.rs) is a faithful port of ngspice's PZ machinery, including the search strategy state machine with its shift, skip, split, and Muller phases.

---

## Reading the output

The result is a list of poles and zeros as complex numbers:

```text
Poles:
  p1 = -1.59e+06              (real pole at 253 kHz)
  p2 = -4.78e+08              (real pole at 76 MHz)
  p3,4 = -2.3e+07 ± j3.1e+07 (complex pair at ~6 MHz)

Zeros:
  z1 = -3.14e+09              (real zero at 500 MHz)
```

A real pole at $s = -2\pi \times 253\,\text{kHz}$ means there's a -3 dB point at 253 kHz — this is the dominant pole that sets the amplifier's bandwidth. The complex pair at roughly 6 MHz indicates a resonance (perhaps from parasitic LC elements). The zero at 500 MHz causes the gain to flatten or increase at very high frequencies.

---

## When to use pole-zero analysis

Pole-zero analysis is most valuable for:

- **Stability analysis:** If any pole has a positive real part, the circuit is unstable. This is the definitive test.
- **Feedback loop design:** The locations of poles and zeros determine phase margin and gain margin. Moving a pole by changing a compensation capacitor is the classic technique for stabilizing an amplifier.
- **Understanding frequency response shape:** The Bode plot is just a graphical representation of the pole-zero locations. Knowing the poles and zeros gives you the complete picture in a compact form.
- **Resonance identification:** Complex conjugate poles identify resonant frequencies and their damping. The $Q$ factor of a resonance is related to how close the poles are to the imaginary axis.

For simple circuits with one or two poles, AC analysis and visual inspection of the Bode plot is usually sufficient. Pole-zero analysis becomes essential for complex multi-stage amplifiers, feedback loops, and any circuit where stability is a concern.

<!-- TODO: interactive s-plane — place poles and zeros by clicking, see the corresponding Bode plot (magnitude and phase) update in real time; drag a pole toward the right half plane and watch the step response become unstable -->
