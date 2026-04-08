# Inductors

An inductor stores energy in a magnetic field created by current flowing through a coil. The fundamental relationship is the dual of the capacitor:

$$\Phi = L \cdot I$$

where $\Phi$ is the magnetic flux (weber), $L$ is the inductance (henry), and $I$ is the current through the inductor. Differentiating gives the voltage-current relationship:

$$V = L \frac{dI}{dt}$$

Voltage appears only when the current is *changing*. A constant current produces zero voltage drop. A rapidly changing current produces a large voltage. This is the mirror image of the capacitor, where current flows only when voltage changes.

---

## DC: short circuit

In DC steady state, all currents are constant — $dI/dt = 0$ everywhere. The inductor equation becomes $V = L \cdot 0 = 0$. No voltage is dropped. The inductor behaves as a **short circuit**: a wire with zero impedance.

This means inductors *do* affect the DC operating point — they provide a zero-resistance path for current. Unlike capacitors (which disappear from the DC problem), inductors are very much present. An inductor between two nodes forces those nodes to the same voltage.

In the MNA framework, the inductor is treated like a voltage source with $V = 0$ at DC. It has a **branch equation** that enforces zero voltage across the inductor and tracks the branch current. The stamps are the same topology stamps as a voltage source:

```rust
// Stamp branch current topology (always present)
mna.stamp(p, b, 1.0);
mna.stamp(n, b, -1.0);
mna.stamp(b, p, 1.0);
mna.stamp(b, n, -1.0);
```

The branch equation row says $V_{\text{pos}} - V_{\text{neg}} = 0$ at DC — a short circuit.

---

## AC: complex impedance

In AC analysis, a sinusoidal current $I = I_0 e^{j\omega t}$ through the inductor produces voltage:

$$V = L \frac{dI}{dt} = L \cdot j\omega I_0 e^{j\omega t} = j\omega L \cdot I$$

The impedance is:

$$Z_L = \frac{V}{I} = j\omega L$$

This is purely imaginary and *positive* — the dual of the capacitor's $1/(j\omega C)$. The magnitude $|Z_L| = \omega L$ *increases* with frequency. At low frequencies, the inductor has low impedance (consistent with being a short circuit at DC). At high frequencies, the impedance is large and the inductor blocks signals.

The voltage *leads* the current by 90 degrees — the opposite phase relationship from a capacitor.

```text
  V, I
       V leads I by 90°
  ┌─╲──────╱──────╲──────╱──→ t
  │  I    ╱ V      I    ╱ V
  │   ╲  ╱          ╲  ╱
  │    ╲╱            ╲╱
```

In spice-rs, the AC stamp puts $-\omega L$ into the imaginary part of the branch equation's diagonal. From [`src/device/inductor.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/device/inductor.rs):

```rust
// ac_load(): topology stamps (real) + impedance (imaginary)
mna.stamp(p, b, 1.0);
mna.stamp(n, b, -1.0);
mna.stamp(b, p, 1.0);
mna.stamp(b, n, -1.0);
mna.stamp_imag(b, b, -val);  // val = omega * L
```

The branch equation becomes $V_{\text{pos}} - V_{\text{neg}} - j\omega L \cdot I_b = 0$, which is exactly $V = j\omega L \cdot I$.

---

## Transient: the companion model

Like the capacitor, the inductor's differential equation must be converted into an algebraic companion model at each timestep. The approach is the same — numerical integration — but applied to flux rather than charge.

The state variable for an inductor is **flux**: $\Phi = L \cdot I$. The voltage across the inductor is the time derivative of the flux:

$$V = \frac{d\Phi}{dt}$$

The integration methods (trapezoidal, Gear) convert this into:

$$V_n = R_{\text{eq}} \cdot I_n + V_{\text{eq}}$$

where $R_{\text{eq}}$ is the companion resistance and $V_{\text{eq}}$ is the companion voltage source. This is the **dual** of the capacitor's companion model: where the capacitor becomes a conductance in parallel with a current source, the inductor becomes a resistance in series with a voltage source.

```text
  Continuous inductor:        Companion model (at timestep n):

       pos ─╖╖╖─ neg              pos ──R_eq──┤ V_eq ├── neg
              L                         
```

In the MNA framework (which uses branch equations for inductors), this translates to:

1. The branch equation enforces $V_{\text{pos}} - V_{\text{neg}} = R_{\text{eq}} \cdot I_b + V_{\text{eq}}$
2. $R_{\text{eq}}$ stamps into the branch equation's diagonal: `mna.stamp(b, b, -req)`
3. $V_{\text{eq}}$ stamps into the RHS: `mna.stamp_rhs(b, veq)`

The implementation in spice-rs splits the inductor's load into two phases to support mutual coupling:

**Phase 1 — `pre_load()`:** Compute the flux from the branch current: $\Phi = L \cdot I_b$. This runs before mutual inductors, so the flux starts with only the self-inductance contribution.

```rust
// pre_load(): first pass — compute flux from branch current
let i_branch = mna.rhs_old_val(self.branch_eq);
states.set(0, flux, self.inductance * i_branch);
```

**Phase 2 — `load()`:** After mutual inductors have added their cross-coupling flux contributions, integrate the total flux to get the companion model values:

```rust
// Integrate: flux -> voltage-equivalent -> companion model
let (req, veq) = ni_integrate(&self.ag, states, newmind, flux, self.order);

mna.stamp_rhs(b, veq);    // companion voltage source
mna.stamp(b, b, -req);    // companion resistance
```

---

## Initial conditions

An inductor can start with a specified current using the `IC` parameter:

```text
L1 pos neg 10u IC=100m
```

This sets $I_L(t=0) = 100$ mA. The initial condition determines the starting flux $\Phi_0 = L \cdot I_0$ and ensures the integration history is consistent. Without an explicit IC, the inductor's initial current comes from the DC operating point.

---

## Capacitor-inductor duality

Capacitors and inductors are mathematical duals. Every statement about one has a corresponding statement about the other with voltage and current swapped:

| Capacitor | Inductor |
|-----------|----------|
| $Q = CV$ | $\Phi = LI$ |
| $I = C\,dV/dt$ | $V = L\,dI/dt$ |
| DC: open circuit | DC: short circuit |
| AC: $Z = 1/(j\omega C)$ | AC: $Z = j\omega L$ |
| State variable: charge | State variable: flux |
| Companion: $G_{\text{eq}} \parallel I_{\text{eq}}$ | Companion: $R_{\text{eq}}$ in series with $V_{\text{eq}}$ |
| No branch equation | Has branch equation |

This duality extends to the code structure. Compare `capacitor.rs` and `inductor.rs` in spice-rs: they have the same `pre_load`/`load` two-phase structure, the same `ni_integrate()` call, and the same state variable layout (two slots: charge/flux and current/voltage-equivalent). The only structural difference is that the inductor uses a branch equation (like a voltage source) while the capacitor stamps directly into the admittance matrix (like a current source).

<!-- TODO: interactive LC comparison — show a capacitor and inductor side by side, both being driven by the same signal; see the phase relationships, companion models, and energy storage in each -->
