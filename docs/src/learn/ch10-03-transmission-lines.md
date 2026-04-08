# Transmission Lines

When a signal travels along a wire, cable, or PCB trace, it doesn't arrive instantaneously. At high frequencies — or over long distances — the propagation delay becomes significant, and the wire can no longer be modeled as a simple node connecting two points. It becomes a **transmission line**: a distributed element with its own impedance and a finite speed of signal propagation.

The lossless transmission line is defined by just two parameters:

**$Z_0$ — characteristic impedance.** The ratio of voltage to current for a wave propagating along the line. A 50-ohm coaxial cable, a 100-ohm differential PCB pair, a 75-ohm television cable — $Z_0$ is the intrinsic property that determines how the line interacts with whatever is connected at its ends. When the load impedance matches $Z_0$, the signal is absorbed perfectly. When there's a mismatch, part of the signal reflects back.

**$T_D$ — propagation delay.** The time it takes a signal to travel from one end of the line to the other. For a 15 cm PCB trace, $T_D$ might be about 0.5 ns (signals travel at roughly 60% the speed of light in FR4). For a 1-meter cable, about 5 ns.

```text
SPICE syntax:

T1 port1+ port1- port2+ port2- Z0=50 TD=1n
```

---

## The physics

A transmission line is a distributed LC network. Every infinitesimal segment has inductance per unit length ($L'$) and capacitance per unit length ($C'$):

```text
  port1+  ───L'──L'──L'──L'──L'──L'───  port2+
              │     │     │     │
             C'    C'    C'    C'
              │     │     │     │
  port1-  ─────────────────────────────  port2-
```

The characteristic impedance and propagation velocity follow from these distributed parameters:

$$Z_0 = \sqrt{\frac{L'}{C'}}$$

$$v_p = \frac{1}{\sqrt{L'C'}}$$

$$T_D = \frac{\ell}{v_p}$$

where $\ell$ is the physical length. A higher $L'/C'$ ratio means higher impedance; a higher $L'C'$ product means slower propagation.

---

## How SPICE models it

SPICE doesn't discretize the line into hundreds of LC segments (that would be expensive and inaccurate). Instead, it uses the **exact analytical solution** for the lossless case. The key insight: on a lossless transmission line, signals travel as waves without distortion. The voltage and current at port 2 at time $t$ depend on the voltage and current at port 1 at time $t - T_D$, and vice versa.

The model works by maintaining a **delay table** — a history of past excitation values at each port. At each timestep, the simulator looks back in time by $T_D$ seconds, interpolates the stored values, and uses them as sources driving the current timestep.

The equivalent circuit at each port is a conductance $G = 1/Z_0$ plus a current source whose value depends on the delayed excitation from the *other* port:

```text
  port 1:                          port 2:
  ┌───────────┐                    ┌───────────┐
  │           │                    │           │
  │  G=1/Z0  ↕ I_eq1(t-TD)       │  G=1/Z0  ↕ I_eq2(t-TD)
  │           │                    │           │
  └───────────┘                    └───────────┘
```

Each equivalent current source is computed from the voltage and current at the opposite port, delayed by $T_D$:

$$\text{input}_1(t) = V_2(t) + Z_0 \cdot I_2(t)$$
$$\text{input}_2(t) = V_1(t) + Z_0 \cdot I_1(t)$$

At time $t$, the companion model at port 1 uses $\text{input}_1(t - T_D)$, and port 2 uses $\text{input}_2(t - T_D)$.

---

## DC behavior

In DC steady state, signals have had infinite time to propagate. The transmission line becomes a **straight-through connection** — the voltage at port 2 equals the voltage at port 1 (assuming matching impedances or after all reflections have died out). In spice-rs, the DC load function stamps this as a direct coupling between the two ports with a small resistive term involving `gmin` for numerical conditioning.

---

## AC behavior

For AC analysis, the delay translates into a **phase shift** that depends on frequency:

$$\phi = -\omega \cdot T_D$$

At low frequencies ($\omega T_D \ll 1$), the phase shift is negligible and the line looks like a short wire. At higher frequencies, the phase rotation becomes significant. At $f = 1/(2T_D)$, the line introduces a half-wavelength delay — a 180-degree phase shift.

The AC stamps use complex-valued entries with $\cos(\omega T_D)$ and $\sin(\omega T_D)$ factors, coupling the two ports through the delayed phase relationship. In spice-rs, the `ac_load()` function in [`src/device/tline.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/device/tline.rs) stamps these complex Y-parameters.

---

## Transient behavior: reflections and ringing

The most interesting transmission line behavior appears in transient analysis. When a fast edge hits a line that is not terminated in its characteristic impedance, part of the signal reflects. The reflected wave travels back to the source, potentially reflects again, and so on. The result is **ringing** — a series of step-like transitions that gradually settle to the final value.

```text
  V at port 2 (open-ended line, step input at port 1):

  2V ┤· · · · · · · · · · · · · · · · ·
     │        ┌──────┐
     │        │      │        ┌─────
  1V ┤────────┘      │        │
     │               │        │
     │               └────────┘
   0 ┤
     └──┬──────┬──────┬──────┬──────→ t
        0     TD    3*TD   5*TD
```

An open-ended line doubles the voltage at the receiving end (the reflected wave adds constructively). A short-circuited termination inverts the reflection. A matched termination ($Z_L = Z_0$) absorbs the signal with no reflection.

In spice-rs, the transient `load()` function maintains the delay table, uses quadratic (Lagrange) interpolation to evaluate the delayed excitation at $t - T_D$, and stamps the resulting companion current sources into the RHS. The `accept_tran()` method records new excitation values into the delay history after each accepted timestep.

---

## When to use transmission lines

Transmission lines matter when the propagation delay is comparable to the signal's rise time. A common rule of thumb:

$$\text{Use a T-line model when } T_D > \frac{t_r}{6}$$

where $t_r$ is the signal's 10-90% rise time. For a 1 ns rise time, this means any trace longer than about 2.5 cm on a typical PCB. For slower signals (microsecond rise times), even meter-long cables can be modeled as simple wires.

The transmission line is a lossless model — it assumes no resistive loss along the line. For long cables where skin-effect loss matters, a lossy transmission line model (not currently in spice-rs) would be needed. But for most PCB-scale signal integrity analysis, the lossless model captures the essential physics: delay, impedance mismatch, and reflections.

<!-- TODO: interactive transmission line demo — step input into a line with adjustable Z0, TD, and load impedance; animate the forward and reflected waves; show the voltage waveform at both ports -->
