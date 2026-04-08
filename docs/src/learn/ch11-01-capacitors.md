# Capacitors

A capacitor stores energy in an electric field between two conductive plates. The fundamental relationship is:

$$Q = C \cdot V$$

where $Q$ is the charge stored (coulombs), $C$ is the capacitance (farads), and $V$ is the voltage across the capacitor. Differentiating both sides with respect to time gives the current-voltage relationship:

$$I = C \frac{dV}{dt}$$

This single equation says everything about how a capacitor behaves: current flows only when the voltage is *changing*. A constant voltage produces zero current. A rapidly changing voltage produces a large current. The faster the voltage changes, the more current flows.

---

## DC: open circuit

In DC steady state, all voltages are constant — $dV/dt = 0$ everywhere. The capacitor equation becomes $I = C \cdot 0 = 0$. No current flows. The capacitor behaves as an **open circuit**: it's simply absent from the DC problem.

This is why capacitors don't appear in the DC operating point calculation (except through their initial conditions). The DC solver doesn't stamp any conductance or current for capacitors. It only records the charge state:

```rust
// From device/capacitor.rs — DC mode
let vcap = mna.rhs_old_val(p) - mna.rhs_old_val(n);
states.set(0, qcap, self.capacitance * vcap);
```

---

## AC: complex impedance

In AC analysis, all signals are sinusoidal at frequency $\omega$. A sinusoidal voltage $V = V_0 e^{j\omega t}$ across the capacitor produces current:

$$I = C \frac{dV}{dt} = C \cdot j\omega V_0 e^{j\omega t} = j\omega C \cdot V$$

The ratio of voltage to current defines the impedance:

$$Z_C = \frac{V}{I} = \frac{1}{j\omega C}$$

This is a purely imaginary impedance. Its magnitude $|Z_C| = 1/(\omega C)$ decreases with frequency — at high frequencies, the capacitor has low impedance and passes signals easily. At low frequencies, the impedance is high and the capacitor blocks signals. At DC ($\omega = 0$), the impedance is infinite — consistent with the open-circuit behavior.

The current *leads* the voltage by 90 degrees. When the voltage is at its peak (not changing), the current is zero. When the voltage crosses zero (changing fastest), the current is at its peak.

```text
  V, I
       I leads V by 90°
  ┌─╲──────╱──────╲──────╱──→ t
  │  V    ╱ I      V    ╱ I
  │   ╲  ╱          ╲  ╱
  │    ╲╱            ╲╱
```

In spice-rs, the AC stamp puts $\omega C$ into the imaginary part of the MNA matrix. From [`src/device/capacitor.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/device/capacitor.rs):

```rust
// ac_load(): stamp omega*C into imaginary matrix
let val = omega * self.capacitance;
mna.stamp_imag(p, p, val);
mna.stamp_imag(n, n, val);
mna.stamp_imag(p, n, -val);
mna.stamp_imag(n, p, -val);
```

This is the standard two-terminal admittance stamp pattern (same as a resistor's conductance stamp, but into the imaginary matrix).

---

## Transient: the companion model

Transient analysis is where capacitors get interesting — and computationally demanding. The simulator must solve $I = C\,dV/dt$ at every timestep, but it can only solve algebraic equations (linear systems). The bridge between differential and algebraic is **numerical integration**.

The idea: replace the continuous derivative $dV/dt$ with a discrete approximation based on the voltage values at the current and previous timesteps. The specific approximation depends on the integration method.

### Trapezoidal rule

The trapezoidal rule approximates the integral of current over one timestep as the average of the current at the start and end of the step:

$$Q_n - Q_{n-1} = \frac{h}{2}(I_n + I_{n-1})$$

Since $Q = CV$, this gives:

$$C(V_n - V_{n-1}) = \frac{h}{2}(I_n + I_{n-1})$$

Solving for $I_n$:

$$I_n = \frac{2C}{h} V_n - \frac{2C}{h} V_{n-1} - I_{n-1}$$

This has the form of a conductance times the current voltage, plus a known history term:

$$I_n = G_{\text{eq}} \cdot V_n + I_{\text{eq}}$$

where:

$$G_{\text{eq}} = \frac{2C}{h}, \qquad I_{\text{eq}} = -\frac{2C}{h} V_{n-1} - I_{n-1}$$

### Gear (BDF) methods

The Gear methods use a different approximation — they estimate the derivative using a weighted combination of the solution at several previous timesteps. For Gear order 2:

$$G_{\text{eq}} = \frac{C \cdot \alpha_0}{h}$$

where $\alpha_0$ depends on the step size ratio. The history term $I_{\text{eq}}$ incorporates charge values from the previous two timesteps. Higher-order Gear methods use more history points for better accuracy on stiff circuits.

### The companion circuit

Regardless of the integration method, the result is the same structure: at each timestep, the capacitor is replaced by an **equivalent conductance** $G_{\text{eq}}$ in parallel with an **equivalent current source** $I_{\text{eq}}$:

```text
  Continuous capacitor:       Companion model (at timestep n):

       pos ─┤├─ neg               pos ──┬──── neg
              C                         │
                                   G_eq ↕ I_eq
                                        │
```

The companion model stamps into the MNA matrix exactly like a resistor (the $G_{\text{eq}}$ part) plus a current source (the $I_{\text{eq}}$ part). This is what makes transient simulation possible: every reactive element becomes a simple resistor-plus-source at each timestep, and the same matrix solver handles everything.

In spice-rs, the `load()` function calls `ni_integrate()` to compute $(G_{\text{eq}}, I_{\text{eq}})$ from the integration coefficients and charge history, then stamps both:

```rust
// Integrate: charge -> current -> companion model
let (geq, ceq) = ni_integrate(&self.ag, states, self.capacitance, qcap, self.order);

// Stamp companion model
mna.stamp(p, p, geq);     // G_eq conductance stamp
mna.stamp(n, n, geq);
mna.stamp(p, n, -geq);
mna.stamp(n, p, -geq);
mna.stamp_rhs(p, -ceq);   // I_eq current source stamp
mna.stamp_rhs(n, ceq);
```

The `ag` array contains the integration coefficients set by the transient engine before each iteration. These coefficients encode the integration method (trapezoidal or Gear) and the current timestep size. The `ni_integrate()` function is a faithful port of ngspice's `NIintegrate` — it computes the current from the charge derivative, then forms the companion conductance and current source.

---

## Initial conditions

A capacitor can start a transient simulation with a specified voltage using the `IC` parameter:

```text
C1 pos neg 10n IC=2.5
```

This sets $V_C(t=0) = 2.5$ V. The initial condition determines the starting charge $Q_0 = C \cdot V_0$, which enters the integration history and affects the companion model at the first timestep.

Without an explicit IC, the capacitor's initial voltage comes from the DC operating point. If `.IC V(pos)=2.5` is used in the netlist, the `setic()` method picks up the node voltage:

```rust
fn setic(&mut self, rhs: &[f64]) {
    if self.ic.is_none() {
        let v = rhs[self.pos_node] - rhs[self.neg_node];
        if v != 0.0 {
            self.ic = Some(v);
        }
    }
}
```

---

## Timestep and accuracy

The companion model's accuracy depends on the timestep $h$. A smaller timestep means the linear approximation of $dV/dt$ is closer to the true derivative. The transient engine monitors the **local truncation error** (LTE) — the difference between the numerical approximation and the true solution — and adjusts the timestep to keep this error within bounds.

For capacitors, the LTE is estimated from the difference between the predicted and computed charge values. If the error is too large, the timestep is rejected and retried with a smaller $h$. If the error is comfortably small, the next timestep can be larger. This adaptive process, described in detail in Chapter 9, ensures that capacitor dynamics are captured accurately without wasting computation on periods where voltages are barely changing.

<!-- TODO: interactive companion model — show a capacitor in an RC circuit, step through timesteps manually, see the companion G_eq and I_eq values change at each step, compare the numerical solution to the exact exponential -->
