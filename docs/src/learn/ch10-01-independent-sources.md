# Independent Sources

An independent source produces a voltage or current that is entirely determined by its own parameters — it doesn't depend on anything else in the circuit. In a netlist, voltage sources start with `V` and current sources start with `I`:

```text
V1 in  0  DC 5             * 5V battery
I1 vcc 0  DC 1m            * 1 mA current source
V2 clk 0  PULSE(0 3.3 0 1n 1n 5n 10n)  * 3.3V clock
```

Independent sources serve three distinct roles depending on the analysis type:

- **DC analysis:** the source supplies a single constant value. This is the operating point around which everything else is evaluated.
- **AC analysis:** the source supplies a small-signal amplitude and phase. AC sources don't produce a time-domain waveform — they define the magnitude and phase of a phasor at each frequency point.
- **Transient analysis:** the source produces a time-varying waveform, evaluated at every timestep from $t = 0$ to the end of the simulation.

A single source element can have all three specifications simultaneously. The DC value sets the operating point, the AC specification is used during `.AC` analysis, and the transient waveform drives `.TRAN` simulation.

---

## DC sources

The simplest case. A DC source supplies a constant value throughout the simulation:

```text
V1 vdd 0  DC 3.3
I1 bias 0 DC 100u
```

During DC operating point analysis, this is the *only* value that matters — transient waveforms are ignored and the circuit is solved for the steady-state condition where all capacitors are open circuits and all inductors are short circuits.

In spice-rs, a DC-only source is represented by the `Waveform::Dc(value)` variant. Its `eval()` method returns the same value regardless of time.

---

## AC sources

An AC source specifies the amplitude and phase of a small-signal excitation for frequency-domain analysis:

```text
V1 in 0  AC 1 0          * 1V amplitude, 0° phase
V2 in 0  AC 0.5 90       * 0.5V amplitude, 90° phase
```

AC sources do not produce a time-domain signal. They define a complex phasor $A \angle \phi$ that is applied at each frequency point during `.AC` analysis. The amplitude is in the same units as the source (volts or amps), and the phase is in degrees.

Typically one source in the circuit has `AC 1 0` (unit amplitude, zero phase), and all other voltages and currents are measured relative to it. The ratio of output to input phasor gives the transfer function at each frequency.

---

## Transient waveforms

During `.TRAN` analysis, independent sources produce time-varying signals. SPICE supports several standard waveform types, each designed for a common use case. In spice-rs, these are defined in [`src/waveform.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/waveform.rs) as variants of the `Waveform` enum.

### PULSE — periodic pulse

```text
PULSE(V1 V2 TD TR TF PW PER)
```

The workhorse waveform for digital circuits. Produces a periodic trapezoidal pulse:

| Parameter | Meaning | Default |
|-----------|---------|---------|
| V1 | Initial value (low level) | — |
| V2 | Pulsed value (high level) | — |
| TD | Delay before first pulse | 0 |
| TR | Rise time | TSTEP |
| TF | Fall time | TSTEP |
| PW | Pulse width (at V2) | TSTOP |
| PER | Period | TSTOP |

The shape within one period:

```text
  V
  V2 ┤        ┌────────┐
     │       ╱          ╲
     │      ╱            ╲
     │     ╱              ╲
  V1 ┤────╱                ╲────────────
     └──┬──┬──┬────────┬──┬──┬─────────→ t
        TD TR  (PW)     TF    (PER)
```

During the delay period ($t < \text{TD}$), the source outputs V1. It then ramps linearly to V2 over TR seconds, holds at V2 for PW seconds, ramps back to V1 over TF seconds, and stays at V1 until the next period begins. The entire pattern repeats with period PER.

The rise and fall times are important for transient accuracy. Setting TR and TF to zero would create an ideal step — but that's numerically problematic because voltages would need to change infinitely fast. SPICE defaults to `TSTEP` (the requested print interval) when TR or TF are zero, ensuring a finite slope.

The pulse waveform also generates **breakpoints** — it tells the transient engine to land exactly on the start and end of each edge. Without breakpoints, the adaptive timestep controller might step right over a fast transition and miss it entirely. In spice-rs, the `next_breakpoint()` method implements the same state machine as ngspice's `vsrcacct.c`.

```text
PULSE example: a 3.3V clock at 100 MHz with 0.5 ns edges

V1 clk 0 PULSE(0 3.3 0 0.5n 0.5n 4.5n 10n)
```

### SIN — damped sinusoid

```text
SIN(VO VA FREQ TD THETA PHASE)
```

Produces a sinusoidal waveform, optionally damped by an exponential envelope:

| Parameter | Meaning | Default |
|-----------|---------|---------|
| VO | DC offset | — |
| VA | Amplitude | — |
| FREQ | Frequency (Hz) | 1/TSTOP |
| TD | Delay before oscillation starts | 0 |
| THETA | Damping factor (1/s) | 0 |
| PHASE | Phase offset (degrees) | 0 |

The time-domain expression is:

$$v(t) = V_O + V_A \cdot \sin(2\pi f(t - T_D) + \phi) \cdot e^{-\theta(t - T_D)}$$

for $t > T_D$, and $v(t) = V_O + V_A \sin(\phi)$ for $t \leq T_D$.

```text
  V
  VO+VA ┤    ╱╲         Undamped (THETA=0)
        │   ╱  ╲
  VO    ┤──╱────╲──────╱╲──────────────
        │        ╲  ╱╱    ╲╲
  VO-VA ┤         ╲╱        ╲╱
        └──────────────────────────────→ t

  V
  VO+VA ┤    ╱╲         Damped (THETA>0)
        │   ╱  ╲
  VO    ┤──╱────╲──╱╲─────────────────
        │        ╲╱   ╲╱╱╲_____
  VO-VA ┤
        └──────────────────────────────→ t
```

When THETA is zero, the sinusoid oscillates forever at constant amplitude. When THETA is positive, the envelope decays exponentially — the oscillation rings down. This is useful for modeling switched sinusoidal excitation or natural decay.

Note that the SIN waveform does *not* generate breakpoints. Unlike PULSE and PWL, a sinusoid has no sharp edges, so the adaptive timestep controller handles it naturally. However, the timestep must be small enough to resolve the waveform — typically at least 10-20 points per period.

```text
SIN example: a 1 kHz tone, 1V amplitude, no damping

V1 in 0 SIN(0 1 1k)
```

### PWL — piecewise linear

```text
PWL(T1 V1 T2 V2 T3 V3 ...)
```

The most flexible waveform. Defines an arbitrary signal as a sequence of time-value pairs, with linear interpolation between them:

| Parameter | Meaning |
|-----------|---------|
| T1, V1 | First time-value pair |
| T2, V2 | Second time-value pair |
| ... | Additional pairs |

```text
  V
  V3 ┤            *───────*
     │          ╱           ╲
  V2 ┤        *               *
     │      ╱
  V1 ┤──*─*
     └──┬──┬──┬──┬───────┬──┬──→ t
       T1 T2 T3 T4      T5 T6
```

Between any two adjacent time points, the voltage changes linearly. Before the first time point, the source holds at V1. After the last time point, it holds at the final value. This means PWL can represent any waveform you can describe with straight line segments — step functions (with very short ramps), triangular waves, arbitrary test patterns, or digitized real-world signals.

Each corner point in the PWL sequence generates a breakpoint, ensuring the transient engine lands on the exact times where the slope changes. In spice-rs, `eval_pwl()` performs a linear search through the pairs and interpolates between the bracketing points.

```text
PWL example: a step that ramps from 0V to 5V between 1 us and 2 us

V1 in 0 PWL(0 0  1u 0  2u 5  10u 5)
```

### EXP — exponential

```text
EXP(V1 V2 TD1 TAU1 TD2 TAU2)
```

Produces a waveform with exponential rise and decay — useful for modeling RC charging/discharging behavior or step responses of first-order systems:

| Parameter | Meaning | Default |
|-----------|---------|---------|
| V1 | Initial value | — |
| V2 | Target value | — |
| TD1 | Rise delay | 0 |
| TAU1 | Rise time constant | TSTEP |
| TD2 | Fall delay | TD1 + TSTEP |
| TAU2 | Fall time constant | TSTEP |

The waveform has two phases:

**Rise phase** ($t > \text{TD1}$):

$$v(t) = V_1 + (V_2 - V_1)\left(1 - e^{-(t - \text{TD1})/\tau_1}\right)$$

**Decay phase** ($t > \text{TD2}$):

$$v(t) = V_1 + (V_2 - V_1)\left(1 - e^{-(t - \text{TD1})/\tau_1}\right) + (V_1 - V_2)\left(1 - e^{-(t - \text{TD2})/\tau_2}\right)$$

```text
  V
  V2 ┤     ·  ·  ·  ·  ·  ·  ·
     │    ╱──────╲
     │   ╱        ╲
     │  ╱          ╲───────
  V1 ┤──            (exponential decay)
     └─┬───────────┬───────────→ t
      TD1         TD2
```

The rise is an exponential approach to V2 with time constant TAU1. The decay is an exponential return toward V1 with time constant TAU2. After about 5 time constants, each transition is essentially complete.

---

## How waveforms enter the matrix

Independent sources interact with the MNA matrix in specific ways depending on whether they are voltage or current sources.

A **voltage source** adds a branch equation to the MNA system (as described in Chapter 2). During each Newton-Raphson iteration, its waveform is evaluated at the current time to determine the voltage value stamped into the RHS:

$$V_{\text{pos}} - V_{\text{neg}} = v(t)$$

A **current source** stamps directly into the RHS at its terminal nodes — no branch equation needed:

$$\text{RHS}[\text{pos}] \mathrel{-}= i(t), \quad \text{RHS}[\text{neg}] \mathrel{+}= i(t)$$

In both cases, the waveform evaluation is the same — only the stamping mechanism differs. In spice-rs, the `Waveform::eval(t, step, final_time)` method is called from the voltage source and current source `load()` functions at every iteration of every timestep.

<!-- TODO: interactive waveform builder — choose type, adjust parameters with sliders, see the waveform shape update in real time; toggle between PULSE, SIN, PWL, EXP -->
