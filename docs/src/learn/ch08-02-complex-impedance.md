# Complex Impedance and the AC Matrix

In DC analysis, the MNA matrix is real-valued: conductances and voltage source constraints, all ordinary numbers. In AC analysis, the matrix becomes *complex*. This is where frequency enters the picture.

## Why complex numbers?

A capacitor's behavior depends on frequency. Apply a DC voltage to a capacitor and no steady-state current flows — infinite impedance. Apply a 1 MHz sine wave and significant current flows — low impedance. The relationship between voltage and current isn't just a scaling factor (like a resistor); it also involves a *phase shift*. The current through a capacitor leads the voltage by 90 degrees.

Complex numbers are the natural language for encoding both magnitude and phase in a single quantity. The impedance of a capacitor is:

$$Z_C = \frac{1}{j\omega C}$$

where $j = \sqrt{-1}$ and $\omega = 2\pi f$. The $j$ in the denominator captures the 90-degree phase shift. The $\omega C$ captures the frequency dependence: higher frequency means lower impedance.

Similarly, an inductor's impedance is:

$$Z_L = j\omega L$$

Current through an inductor lags the voltage by 90 degrees — the opposite phase relationship from a capacitor.

A resistor remains simply $Z_R = R$: purely real, no phase shift, no frequency dependence.

## The AC MNA matrix: $G + j\omega C$

The real MNA matrix from DC analysis stamped conductances into positions determined by each device's node connections. The AC matrix works the same way, but now there are two components at each matrix position:

- **Real part ($G$):** conductances from resistors and small-signal parameters ($g_m$, $g_{ds}$, $g_d$, etc.)
- **Imaginary part ($j\omega C$):** admittances from capacitors and small-signal capacitances ($C_{gs}$, $C_{gd}$, etc.)

At each frequency point, the system to solve is:

$$(G + j\omega C)\mathbf{x} = \mathbf{b}$$

where $\mathbf{x}$ is the vector of complex node voltages and branch currents, and $\mathbf{b}$ is the excitation vector (from AC sources).

## How devices stamp the complex matrix

**Resistor** ($R$ between nodes $i$ and $j$):

Stamps $g = 1/R$ into the real part of the matrix — exactly like DC analysis. No imaginary contribution.

**Capacitor** ($C$ between nodes $i$ and $j$):

Stamps $j\omega C$ into the imaginary part:

```text
          Real part        Imaginary part
       [i]     [j]        [i]     [j]
  [i]   0       0     [i]  +wC    -wC
  [j]   0       0     [j]  -wC    +wC
```

At low frequency ($\omega \to 0$), the capacitor contribution vanishes — it's an open circuit. At high frequency ($\omega \to \infty$), the capacitor dominates — it's a short circuit.

**Inductor** ($L$ with branch current variable $k$):

The inductor is handled through its branch equation. In MNA, an inductor adds a current variable (like a voltage source) and stamps $j\omega L$ into the imaginary part of the branch equation:

```text
          Real part        Imaginary part
       [i]  [j]  [k]     [i]  [j]  [k]
  [i]   0    0   +1       0    0    0
  [j]   0    0   -1       0    0    0
  [k]  +1   -1    0       0    0   -wL
```

At low frequency, the $j\omega L$ term vanishes and the inductor looks like a wire (zero voltage drop). At high frequency, it looks like an open circuit.

**MOSFET small-signal model:**

The transconductance $g_m$ stamps into the real part (it's a conductance — no frequency dependence). The gate capacitances $C_{gs}$ and $C_{gd}$ stamp into the imaginary part, scaled by $\omega$. This is why MOSFETs have finite bandwidth: at high enough frequency, the capacitive admittances overwhelm the transconductance.

## The frequency sweep

In spice-rs, the frequency sweep is a straightforward loop. For each frequency point:

1. Compute $\omega = 2\pi f$
2. Clear the complex matrix
3. Call `ac_load()` on every device — each stamps its conductances (real part) and $\omega$-scaled capacitances (imaginary part)
4. Factor and solve the complex system with `solve_complex()`
5. Store the complex node voltages

The solver uses the same sparse LU decomposition as DC analysis, but operating on complex numbers. The pivot ordering from the DC operating point solve is reused — only the numerical factorization is redone at each frequency.

```text
    Frequency sweep in ac_analysis()

    ┌──────────────────────────────┐
    │  DC operating point          │  (expensive, done once)
    │  Small-signal linearization  │
    └──────────────┬───────────────┘
                   │
    ┌──────────────▼───────────────┐
    │  for each frequency f:       │
    │    w = 2*pi*f                │
    │    clear complex matrix      │
    │    ac_load all devices (w)   │  (cheap, done N times)
    │    solve (G + jwC)x = b     │
    │    store complex voltages    │
    └──────────────┬───────────────┘
                   │
    │  increment f (DEC/OCT/LIN)   │
    └──────────────────────────────┘
```

## Reading the result

The solution at each frequency point is a complex voltage at every node. For node $k$:

- **Magnitude:** $|V_k| = \sqrt{\text{Re}(V_k)^2 + \text{Im}(V_k)^2}$
- **Phase:** $\angle V_k = \arctan\!\left(\frac{\text{Im}(V_k)}{\text{Re}(V_k)}\right)$

The magnitude tells you how much signal gets through at that frequency. The phase tells you how much the signal is shifted in time. Together, plotted across frequency, they form the Bode plot — the subject of the next section.

<!-- TODO: interactive complex plane widget — show a phasor at a single frequency, rotate it as frequency sweeps, watch magnitude shrink for a low-pass filter -->
