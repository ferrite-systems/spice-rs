# Filter Circuits

The best way to build intuition for AC analysis is to work through circuits where you can predict the answer before running the simulation. Filters are ideal for this — their frequency response follows directly from a few lines of algebra, and the Bode plots have clean, recognizable shapes.

## RC Low-Pass Filter

The simplest filter: one resistor, one capacitor. Low-frequency signals pass through; high-frequency signals are attenuated.

```ferrite-circuit
circuit "RC Low-Pass Filter" {
    node "vin" label="Vin" rail=#true voltage="1"
    node "gnd" ground=#true
    group "filter" topology="rc-low-pass" {
        component "R1" type="resistor" role="filter-element" {
            value "1k"
            port "1" net="vin"
            port "2" net="vout"
        }
        component "C1" type="capacitor" role="shunt" {
            value "100n"
            port "1" net="vout"
            port "2" net="gnd"
        }
    }
    node "vout" label="Vout"
}
```

### The physics

At DC ($f = 0$), the capacitor is an open circuit. No current flows, so there's no voltage drop across the resistor: $V_{\text{out}} = V_{\text{in}}$. Full signal, no loss.

At very high frequency, the capacitor is a short circuit. The output node is shorted to ground: $V_{\text{out}} \approx 0$. The signal is completely attenuated.

The transition between these extremes is governed by a single number: the **cutoff frequency**.

### Transfer function

The output is taken across the capacitor. Using the voltage divider with complex impedances:

$$H(f) = \frac{V_{\text{out}}}{V_{\text{in}}} = \frac{Z_C}{R + Z_C} = \frac{1/(j\omega C)}{R + 1/(j\omega C)} = \frac{1}{1 + j\omega RC}$$

The magnitude and phase:

$$|H(f)| = \frac{1}{\sqrt{1 + (f/f_c)^2}}$$

$$\angle H(f) = -\arctan\!\left(\frac{f}{f_c}\right)$$

where $f_c = \frac{1}{2\pi RC}$ is the cutoff frequency.

### Numbers

For $R = 1\text{ k}\Omega$ and $C = 100\text{ nF}$:

$$f_c = \frac{1}{2\pi \cdot 1000 \cdot 100 \times 10^{-9}} \approx 1{,}592\text{ Hz}$$

The Bode plot has three distinct regions:

```text
    Magnitude
     0 dB ──────────────╲
                         ╲
   -20 dB                 ╲  -20 dB/decade
                           ╲
   -40 dB                   ╲
         ────┬────┬────┬────┬────
           100  1k   10k 100k  (Hz)
                  ▲
                 fc = 1,592 Hz

    Phase
     0 deg ──────────╲
                      ╲
   -45 deg             ╳  (exactly -45 at fc)
                        ╲
   -90 deg               ────────
         ────┬────┬────┬────┬────
           100  1k   10k 100k  (Hz)
```

At the cutoff frequency:
- Magnitude = $-3.01\text{ dB}$ (by definition)
- Phase = $-45\degree$ (exactly halfway between $0\degree$ and $-90\degree$)

Below $f_c$: gain is approximately 0 dB, phase is near $0\degree$. The signal passes through unchanged.

Above $f_c$: gain drops at -20 dB/decade (one-tenth per decade), phase approaches $-90\degree$. The capacitor is increasingly dominating the impedance, shorting high-frequency signals to ground.

The -20 dB/decade rolloff is the signature of a *first-order* filter — one reactive element, one pole in the transfer function. Every additional RC stage adds another -20 dB/decade.

### What AC analysis computes

When spice-rs runs `.AC DEC 10 1 1MEG` on this circuit, it:

1. Finds the DC operating point (trivial here — no nonlinear devices, all voltages determined by the DC source)
2. Computes small-signal parameters (nothing to linearize — everything is already linear)
3. At each of ~50 frequency points, assembles and solves:

$$\begin{bmatrix} G + j\omega C_{11} & -(G + j\omega C_{12}) \\ -(G + j\omega C_{21}) & G + j\omega C_{22} \end{bmatrix} \begin{bmatrix} V_{\text{out}} \end{bmatrix} = \begin{bmatrix} \text{source} \end{bmatrix}$$

where $G = 1/R = 1\text{ mS}$ and the capacitor stamps $j\omega C$ at the output node. The result at each frequency is a complex $V_{\text{out}}$, from which magnitude and phase are extracted.

For a purely passive circuit like this, the AC analysis result will match the analytical formula exactly — the only error is floating-point precision.

<!-- TODO: interactive RC filter — slider for R and C values, live Bode plot update, show fc moving -->

## RLC Series Resonance

Add an inductor to the RC circuit and the physics gets more interesting: the circuit can *resonate*.

Consider an RLC series circuit driven by an AC source, with the output measured across the resistor. The inductor and capacitor have opposite phase relationships — the inductor's impedance increases with frequency while the capacitor's decreases. At one specific frequency, their impedances are equal in magnitude and cancel each other exactly. At that frequency, the total impedance is purely resistive and minimized, so maximum current flows.

### The resonant frequency

The impedances cancel when $|Z_L| = |Z_C|$:

$$\omega_0 L = \frac{1}{\omega_0 C}$$

Solving:

$$f_0 = \frac{1}{2\pi\sqrt{LC}}$$

At resonance, the current in the series loop is limited only by $R$. Above and below resonance, the reactive impedances don't cancel and the current is reduced.

### The quality factor

How sharp the resonance peak is depends on the **quality factor** $Q$:

$$Q = \frac{1}{R}\sqrt{\frac{L}{C}} = \frac{f_0}{\Delta f_{-3\text{dB}}}$$

A high-$Q$ circuit has a narrow, tall resonance peak — it's very selective in frequency. A low-$Q$ circuit has a broad, gentle peak. The $-3\text{ dB}$ bandwidth of the resonance is $\Delta f = f_0 / Q$.

```text
    RLC resonance — magnitude plot

    High Q (low R):          Low Q (high R):
         ╱╲                      ╱──╲
        ╱  ╲                   ╱      ╲
       ╱    ╲                ╱          ╲
    ──╱      ╲──          ──╱              ╲──
      ───┬───              ─────┬─────
         f0                     f0

    Sharper peak = more selective = higher Q
```

### Phase at resonance

The phase of the impedance sweeps from $+90\degree$ (inductive, below resonance) through $0\degree$ (purely resistive, at resonance) to $-90\degree$ (capacitive, above resonance). This $180\degree$ phase swing happens within a narrow frequency range for high-$Q$ circuits.

### Example values

For $L = 10\text{ mH}$, $C = 100\text{ nF}$, $R = 100\ \Omega$:

$$f_0 = \frac{1}{2\pi\sqrt{10 \times 10^{-3} \cdot 100 \times 10^{-9}}} \approx 5{,}033\text{ Hz}$$

$$Q = \frac{1}{100}\sqrt{\frac{10 \times 10^{-3}}{100 \times 10^{-9}}} \approx 3.16$$

$$\Delta f_{-3\text{dB}} \approx \frac{5{,}033}{3.16} \approx 1{,}593\text{ Hz}$$

A moderate $Q$ — the resonance peak is visible but not dramatic. Increase $L$ or decrease $R$ and the peak sharpens.

## From filters to amplifiers

These passive filter examples demonstrate the mechanics of AC analysis without the complexity of nonlinear devices. But the real power of AC analysis is in circuits with transistors, where the small-signal linearization from Section 8.1 comes into play.

An amplifier's frequency response is shaped by the same physics: parasitic capacitances in the transistors create poles (like the RC cutoff frequency), feedback networks create zeros, and the interplay between them determines the bandwidth, gain, and stability. The Bode plot of an op-amp, with its dominant pole and unity-gain crossover, is just a more elaborate version of what we've seen here.

The math is the same. The matrix is larger. But the principle — solve $(G + j\omega C)\mathbf{x} = \mathbf{b}$ at each frequency — doesn't change.

<!-- TODO: interactive RLC resonance — sliders for R, L, C, show resonance peak moving and Q changing on live Bode plot -->
