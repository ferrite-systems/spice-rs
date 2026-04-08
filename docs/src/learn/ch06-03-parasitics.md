# Parasitics

The Gummel-Poon model describes the *intrinsic* BJT -- the ideal transistor at the heart of the device. But a real BJT has parasitic elements that affect its high-frequency behavior, switching speed, and accuracy under certain bias conditions.

These parasitics fall into three categories: series resistances, junction capacitances, and transit times.

## Series resistances

The base, emitter, and collector terminals of a real BJT are connected to the intrinsic device through resistive semiconductor material:

```text
         External         Intrinsic         External
         Collector        BJT               Base
            |                |                |
           [RC]              |               [RB]
            |                |                |
            +---collector----+----base--------+
                             |
                          emitter
                             |
                           [RE]
                             |
                         External
                         Emitter
```

| Parameter | Symbol | Typical NPN | Meaning |
|-----------|--------|-------------|---------|
| RB | $R_B$ | 10-100 $\Omega$ | Base resistance |
| RE | $R_E$ | 1-5 $\Omega$ | Emitter resistance |
| RC | $R_C$ | 10-50 $\Omega$ | Collector resistance |

**Base resistance (RB)** is the most important parasitic. It creates a voltage drop between the external base terminal and the intrinsic base-emitter junction, reducing the effective $V_{BE}$. At high currents, this drop is significant: $\Delta V = I_B \cdot R_B$. Since $I_B$ increases with collector current, $R_B$ causes the effective $\beta$ to decrease at high currents.

The SPICE model supports both a constant $R_B$ and a current-dependent base resistance that decreases under high injection (as the base conductivity increases with injected carriers). The parameters RBM (minimum base resistance) and IRB (current where $R_B$ falls halfway to RBM) control this behavior.

**Emitter resistance (RE)** provides local negative feedback, stabilizing the bias point. It also degrades the intrinsic $g_m$ to an effective value:

$$g_{m,eff} = \frac{g_m}{1 + g_m R_E}$$

**Collector resistance (RC)** adds a voltage drop at high currents, affecting the saturation voltage $V_{CE,sat}$.

## Junction capacitances

The BJT has two PN junctions, each with a depletion capacitance:

$$C_J(V) = \frac{C_{J0}}{(1 - V/V_J)^{M_J}}$$

| Parameter | Junction | Symbol | Meaning |
|-----------|----------|--------|---------|
| CJE | B-E | $C_{JE0}$ | Zero-bias B-E depletion capacitance |
| VJE | B-E | $V_{JE}$ | B-E built-in potential |
| MJE | B-E | $M_{JE}$ | B-E grading coefficient |
| CJC | B-C | $C_{JC0}$ | Zero-bias B-C depletion capacitance |
| VJC | B-C | $V_{JC}$ | B-C built-in potential |
| MJC | B-C | $M_{JC}$ | B-C grading coefficient |
| CJS | C-S | $C_{JS0}$ | Zero-bias collector-substrate capacitance |

The base-collector capacitance $C_{JC}$ is particularly important because it appears between input and output of the common-emitter amplifier, creating the *Miller effect*. The effective input capacitance is multiplied by the voltage gain:

$$C_{in,Miller} = C_{JC} \cdot (1 + |A_v|)$$

This Miller multiplication is the dominant bandwidth limitation in common-emitter amplifiers.

## Transit times

When carriers are injected across the base, they take a finite time to traverse it. This transit time creates an additional capacitance -- the *diffusion capacitance* -- proportional to the current:

$$C_{diff} = \tau_F \cdot g_m$$

where $\tau_F$ is the forward transit time (parameter TF) and $g_m = I_C / V_T$.

| Parameter | Symbol | Typical NPN | Meaning |
|-----------|--------|-------------|---------|
| TF | $\tau_F$ | 0.1-1 ns | Forward transit time |
| TR | $\tau_R$ | 10-100 ns | Reverse transit time |

The forward transit time determines the *transition frequency* $f_T$, the frequency at which the current gain drops to unity:

$$f_T = \frac{1}{2\pi \left(\tau_F + \frac{C_{JE} + C_{JC}}{g_m}\right)}$$

At low currents, $g_m$ is small, so the depletion capacitances dominate and $f_T$ rises with current. At high currents, $\tau_F$ dominates and $f_T$ plateaus (and eventually falls due to high-injection effects). Typical $f_T$ values range from 1 GHz (standard process) to 300+ GHz (SiGe HBTs).

<!-- TODO: interactive fT vs IC plot -- show the three regimes (low current, peak fT, high-injection rolloff) -->

## The complete small-signal model

Combining all parasitics with the intrinsic model:

```text
   B ---[RB]---+---[Cmu]---+---[RC]--- C
               |            |
              [Cpi]       [gm*Vbe]
               |            |
              [gpi]       [go]
               |            |
               +------------+
               |
             [RE]
               |
               E
```

where:
- $C_\pi = C_{JE} + \tau_F g_m$ (junction + diffusion capacitance)
- $C_\mu = C_{JC}$ (collector junction capacitance)
- $g_\pi = I_C / (\beta V_T)$ (input conductance)
- $g_m = I_C / V_T$ (transconductance)
- $g_o = I_C / V_{AF}$ (output conductance, Early effect)

This is the *hybrid-$\pi$* model, and it is exactly what the SPICE small-signal analysis produces. Each element stamps into the MNA matrix. The parasitics do not change the stamping architecture -- they add more elements to stamp.

## In spice-rs

All parasitic elements are computed in the same `load_bjt` function that handles the intrinsic model. The junction capacitances use the same depletion capacitance formula as MOSFET junctions. The diffusion capacitances are computed from the transit times and the current operating point. The series resistances modify the node mapping so that the internal (intrinsic) node voltages differ from the external terminal voltages.

The key implementation detail is that series resistances add internal nodes to the circuit. A BJT with $R_B > 0$, $R_E > 0$, and $R_C > 0$ adds three internal nodes to the MNA matrix, increasing the matrix size. This is why some simple models set these resistances to zero -- it keeps the matrix smaller and the simulation faster.
