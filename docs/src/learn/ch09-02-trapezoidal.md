# The Trapezoidal Rule

The trapezoidal rule is the default integration method in SPICE and the primary method in spice-rs. It approximates the integral of a function by averaging the function's value at the beginning and end of each interval — geometrically, it's the area of a trapezoid rather than a rectangle.

## The formula

Given a differential equation $dy/dt = f(t, y)$, the trapezoidal rule computes:

$$y_{n+1} = y_n + \frac{h}{2}\big(f_n + f_{n+1}\big)$$

where $h = t_{n+1} - t_n$ is the timestep. The key feature: both $f_n$ (the derivative at the *old* time) and $f_{n+1}$ (the derivative at the *new* time) appear. Since $f_{n+1}$ depends on $y_{n+1}$ which we're trying to find, the method is **implicit** — we can't just evaluate a formula; we have to solve a system of equations. This is exactly what Newton-Raphson does at each timestep.

```text
    Trapezoidal rule — geometric interpretation

    f(t)
     │
     │    f_n ●─────────────● f_{n+1}
     │        │╱╱╱╱╱╱╱╱╱╱╱╱│
     │        │╱╱╱╱╱╱╱╱╱╱╱╱│   Area of the trapezoid
     │        │╱╱╱╱╱╱╱╱╱╱╱╱│   = (h/2)(f_n + f_{n+1})
     │        │╱╱╱╱╱╱╱╱╱╱╱╱│   = integral approximation
     └────────┴─────────────┴───── t
             t_n           t_{n+1}
              ◄─────────────►
                    h
```

## The companion model for a capacitor

For a capacitor with $I = C \cdot dV/dt$, applying the trapezoidal rule gives:

$$q_{n+1} = q_n + \frac{h}{2}(I_n + I_{n+1})$$

Since $q = CV$, the current at the new time point is:

$$I_{n+1} = \frac{2C}{h} V_{n+1} - \frac{2C}{h} V_n - I_n$$

This has the companion model form $I_{n+1} = G_{\text{eq}} \cdot V_{n+1} + I_{\text{eq}}$, with:

$$G_{\text{eq}} = \frac{2C}{h}$$

$$I_{\text{eq}} = -\frac{2C}{h} V_n - I_n$$

The equivalent conductance $G_{\text{eq}} = 2C/h$ makes physical sense: a large capacitance or a small timestep means a large equivalent conductance (the capacitor resists rapid voltage change). The current source $I_{\text{eq}}$ carries forward the memory of the previous timestep — the voltage and current that the capacitor had at $t_n$.

## Integration coefficients in spice-rs

In `ni_com_cof()` ([`integration.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/integration.rs)), the trapezoidal method corresponds to order 2 with $\mu = 0.5$:

```text
    Order 2 (trapezoidal):
        ag[0] = 1 / (delta * (1 - xmu))     = 1 / (h * 0.5) = 2/h
        ag[1] = xmu / (1 - xmu)             = 0.5 / 0.5     = 1
```

Then in `ni_integrate()`, the current is computed as:

```text
    I_{n+1} = -I_n * ag[1] + ag[0] * (q_{n+1} - q_n)
            = -I_n * 1    + (2/h)  * (q_{n+1} - q_n)
```

And the companion model output:

```text
    Geq = ag[0] * C = (2/h) * C = 2C/h
    Ieq = I_{n+1} - ag[0] * q_{n+1}
```

The `ag[0]` coefficient plays a dual role: it scales the charge difference to get the current, and it scales the capacitance to get the equivalent conductance.

## Why order matters: backward Euler as order 1

When spice-rs uses order 1 (backward Euler), the integration is simpler but less accurate:

$$y_{n+1} = y_n + h \cdot f_{n+1}$$

Only the derivative at the new time point is used — no averaging. The companion model becomes:

$$G_{\text{eq}} = \frac{C}{h}, \qquad I_{\text{eq}} = -\frac{C}{h} V_n$$

The coefficients from `ni_com_cof()`:

```text
    Order 1 (backward Euler):
        ag[0] = 1/h
        ag[1] = -1/h
```

Backward Euler is first-order accurate (error proportional to $h^2$), while trapezoidal is second-order accurate (error proportional to $h^3$). For the same accuracy, backward Euler needs roughly $\sqrt{N}$ times as many steps. But backward Euler is unconditionally stable in a stronger sense than trapezoidal — it never produces numerical oscillation.

## Advantages of trapezoidal

**Second-order accuracy.** The local truncation error is proportional to $h^3 \cdot d^3V/dt^3$. Doubling the timestep increases the error by a factor of 8, but doubling the number of steps reduces the total error by a factor of 4. This is a meaningful improvement over first-order methods and is the main reason trapezoidal is the default.

**A-stability.** The method is stable for all timestep sizes — it will never diverge due to the timestep being too large. (It might be inaccurate with a large timestep, but it won't blow up.) This is essential for circuits with widely separated time constants, where the fastest dynamics constrain the timestep needed for accuracy but the slowest dynamics determine the simulation duration.

**Time-reversibility.** The trapezoidal rule is a symmetric method — it treats the old and new time points equally. This gives it a special property: it doesn't artificially damp oscillations. A lossless LC circuit simulated with the trapezoidal rule will oscillate forever at constant amplitude, which is the physically correct behavior.

## The disadvantage: numerical ringing

The same time-reversibility that preserves oscillations in LC circuits can create *spurious* oscillations in circuits that shouldn't oscillate. This is called **trapezoidal ringing** or the **trap rule problem**.

It occurs in stiff circuits — circuits where some time constants are much faster than others. The classic example: a MOSFET switching rapidly, creating a step change in current through an inductor. The trapezoidal rule, being a second-order method with no numerical damping, can produce decaying oscillations around the true solution that look like high-frequency ringing.

```text
    Trapezoidal ringing

    V(t)
     │     True solution
     │    ╱─────────────────
     │   ╱
     │  ╱   Trapezoidal
     │ ╱  ╱╲  ╱╲
     │╱  ╱  ╲╱  ╲╱╲────────   oscillates around
     │  ╱              true    the correct value
     └──────────────────────── t
```

This is not a bug in the implementation — it's a mathematical property of the trapezoidal rule applied to stiff systems. The standard mitigation is to use the **Gear (BDF) methods** described in the next section, which trade some accuracy for numerical damping.

In spice-rs, the transient engine uses backward Euler (order 1) at breakpoints and at the start of the simulation, where the solution may have discontinuities. It then promotes to trapezoidal (order 2) once the solution is smooth, via the order promotion logic in `transient()`:

```text
    if order == 1 && max_order > 1:
        try LTE at order 2
        if newdelta2 <= 1.05 * delta:
            stay at order 1 (trapezoidal wouldn't help)
        else:
            promote to order 2 (trapezoidal is more efficient)
```

This hybrid approach gets the stability of backward Euler where it's needed and the accuracy of trapezoidal everywhere else.

<!-- TODO: interactive ringing demo — show a circuit with a fast edge, toggle between backward Euler and trapezoidal, see the ringing appear/disappear -->
