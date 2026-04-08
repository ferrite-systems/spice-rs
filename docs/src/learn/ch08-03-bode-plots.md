# Bode Plots

The result of AC analysis is a table: for each frequency, a complex voltage at every node. The Bode plot is the standard way to visualize this data — two plots, stacked vertically, that together tell the complete story of a circuit's frequency response.

## Magnitude: how much signal gets through

The magnitude plot shows the ratio of output to input voltage, expressed in decibels:

$$|H(f)|_{\text{dB}} = 20 \log_{10} |H(f)|$$

where $H(f) = V_{\text{out}}(f) / V_{\text{in}}(f)$ is the transfer function.

The decibel scale is logarithmic, which matches how we perceive signal strength and makes exponential rolloffs appear as straight lines. Some reference points:

| Gain (linear) | Gain (dB) | Meaning |
|:-:|:-:|:--|
| 1 | 0 dB | Unity — output equals input |
| 0.707 | -3 dB | Half-power point (the "cutoff frequency") |
| 0.1 | -20 dB | One-tenth the input |
| 0.01 | -40 dB | One-hundredth the input |
| 10 | +20 dB | Ten times the input |
| 100 | +40 dB | Amplifier with gain of 100 |

The **-3 dB point** deserves special attention. It's defined as the frequency where the output power drops to half the passband value. Since power is proportional to voltage squared, half power corresponds to $|H| = 1/\sqrt{2} \approx 0.707$, or $20 \log_{10}(0.707) \approx -3.01\text{ dB}$. This is the conventional boundary between "passband" and "stopband" in filter design.

## Phase: how much the signal is shifted

The phase plot shows the angle of the transfer function:

$$\angle H(f) = \arctan\!\left(\frac{\text{Im}(H(f))}{\text{Re}(H(f))}\right)$$

measured in degrees. A phase of $0\degree$ means the output is in sync with the input. A phase of $-90\degree$ means the output lags by a quarter cycle. A phase of $-180\degree$ means the output is inverted — and in a feedback system, this is where oscillation can occur.

```text
    Bode plot layout

    ┌─────────────────────────────────────┐
    │  Magnitude (dB)                     │
    │   0 ─────────╲                      │
    │ -20           ╲                     │
    │ -40            ╲  -20 dB/dec slope  │
    │ -60             ╲                   │
    ├─────────────────────────────────────┤
    │  Phase (degrees)                    │
    │   0 ────────╲                       │
    │ -45          ╲                      │
    │ -90           ─────────────         │
    │                                     │
    └─────────────────────────────────────┘
           10    100   1k   10k  100k  (Hz)

    Typical first-order low-pass response.
    Magnitude rolls off at -20 dB/decade.
    Phase transitions from 0 to -90 degrees.
```

Both axes use a logarithmic frequency scale — each major division represents a factor of 10 in frequency (a "decade"). This makes the plots compact: you can see behavior from 1 Hz to 1 GHz on a single page.

## Frequency sweep types

The `.AC` command specifies how frequency points are distributed across the range:

**DEC (decade)** — logarithmically spaced, with a fixed number of points per decade. This is the most common choice. With 10 points per decade from 1 Hz to 1 MHz, you get 50 frequency points total, evenly spaced on the log axis.

In spice-rs, each step multiplies the frequency by a constant factor:

$$f_{n+1} = f_n \cdot \exp\!\left(\frac{\ln 10}{N_{\text{pts}}}\right)$$

where $N_{\text{pts}}$ is points per decade.

**OCT (octave)** — logarithmically spaced, with a fixed number of points per octave (factor of 2). Common in audio applications where octaves are a natural unit. Each step multiplies by:

$$f_{n+1} = f_n \cdot \exp\!\left(\frac{\ln 2}{N_{\text{pts}}}\right)$$

**LIN (linear)** — uniformly spaced in frequency. Useful when you're zooming in on a narrow frequency range (like a resonance peak) and want even resolution across it. Each step adds a constant:

$$f_{n+1} = f_n + \frac{f_{\text{stop}} - f_{\text{start}}}{N_{\text{pts}} - 1}$$

Linear spacing is a poor choice for wide-range sweeps — you'd need thousands of points to cover 1 Hz to 1 GHz with any resolution at the low end.

## The sweep loop in spice-rs

The frequency sweep in `ac_analysis()` ([`analysis/ac.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/analysis/ac.rs)) follows the ngspice structure closely. The key detail is the frequency increment logic:

For DEC and OCT sweeps, the frequency is *multiplied* at each step:

```text
freq *= freq_delta;    // geometric progression
```

For LIN sweeps, the frequency is *added*:

```text
freq += freq_delta;    // arithmetic progression
```

The loop terminates when `freq > fstop + freq_tol`, where `freq_tol` is a small tolerance that accounts for floating-point accumulation:

- DEC/OCT: `freq_tol = freq_delta * fstop * reltol`
- LIN: `freq_tol = freq_delta * reltol`

This tolerance prevents the sweep from missing the last frequency point due to rounding.

## What Bode plots tell you

A Bode plot encodes the essential character of a circuit in two curves:

- **Bandwidth** — the frequency range where the magnitude is within 3 dB of its peak. A wider bandwidth means the circuit responds to faster signals.

- **Rolloff rate** — how steeply the gain drops outside the passband. A first-order filter (one capacitor) rolls off at -20 dB/decade. A second-order filter (two reactive elements) rolls off at -40 dB/decade. Each additional order adds another -20 dB/decade.

- **Resonance** — if the magnitude has a peak above the passband level, the circuit resonates at that frequency. The height of the peak indicates how underdamped the system is.

- **Phase margin** — in a feedback amplifier, the phase at the frequency where gain crosses 0 dB determines stability. If the phase is near $-180\degree$ at that frequency, the amplifier is on the edge of oscillation.

These quantities — bandwidth, rolloff, resonance, phase margin — are the vocabulary of analog circuit design, and they all come from AC analysis.

<!-- TODO: interactive Bode plot — sweep type selector (DEC/OCT/LIN), adjustable frequency range, show the frequency points as dots on the curve -->
