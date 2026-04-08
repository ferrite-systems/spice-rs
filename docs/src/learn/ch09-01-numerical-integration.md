# Numerical Integration

At the heart of transient analysis is a problem: capacitors and inductors are described by differential equations, but SPICE's matrix solver only handles algebraic equations. Numerical integration bridges this gap.

## The differential equation

A capacitor's current is proportional to the *rate of change* of its voltage:

$$I_C = C \frac{dV}{dt}$$

An inductor's voltage is proportional to the rate of change of its current:

$$V_L = L \frac{dI}{dt}$$

These are ordinary differential equations (ODEs). In continuous time, they describe smooth, evolving quantities — the voltage across a capacitor changes continuously as charge accumulates. But our simulator works at discrete time points: $t_0$, $t_1$, $t_2$, and so on. We need to approximate the derivative $dV/dt$ using values we've already computed at previous time points.

## The core idea: companion models

The key insight is that a numerical integration formula converts a capacitor's differential equation into an algebraic relationship between the current voltage and the previous voltages. This algebraic relationship has the same form as a **conductance in parallel with a current source** — a linear two-terminal element that can be stamped directly into the MNA matrix.

This equivalent circuit is called the **companion model**. It changes at every timestep (because it depends on the previous solution and the current timestep $h$), but at any given timestep it is a simple linear element.

```text
    Continuous capacitor:       Companion model at timestep n+1:

         ┤├                       ┌────┐
    ──┤  C  ├──             ──┤  Geq  ├──┬──
         ┤├                       └────┘  │
                                         ╽
                                        Ieq
                                         │
                                        ─┴─

    I = C * dV/dt             Geq and Ieq depend on the
    (differential eqn)       integration method, timestep h,
                             and previous values
```

This is the same trick that Newton-Raphson uses for nonlinear devices: a diode becomes a conductance plus a current source at each iteration. The companion model does the same thing for energy-storage elements at each timestep. Both techniques convert something hard (a differential equation or a nonlinear equation) into something the MNA matrix can handle (a linear conductance plus a current source).

## How the companion model is computed

The general pattern for any integration method:

1. The **charge** $q(t)$ stored in the capacitor is tracked as a state variable. For a capacitor, $q = C \cdot V$. For an inductor, the equivalent quantity is flux: $\phi = L \cdot I$.

2. The integration formula approximates the **current** (derivative of charge) using the charge at the current and previous time points:

$$I_{n+1} \approx f(q_{n+1}, q_n, q_{n-1}, \ldots, h)$$

3. This formula is rearranged into the companion model form:

$$I_{n+1} = G_{\text{eq}} \cdot V_{n+1} + I_{\text{eq}}$$

where $G_{\text{eq}}$ depends on $C$ and $h$ (and possibly the integration coefficients), and $I_{\text{eq}}$ captures the contribution from previous time points.

$G_{\text{eq}}$ stamps into the MNA matrix exactly like a conductance. $I_{\text{eq}}$ stamps into the RHS vector exactly like a current source. The matrix solver doesn't know or care that this "conductance" actually represents a capacitor being integrated through time.

## The integration coefficients: `ag[]`

In spice-rs (and in ngspice), the details of the integration method are encoded in an array of coefficients called `ag[0..6]`. These coefficients are computed by `ni_com_cof()` in [`integration.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/integration.rs) at each timestep, based on:

- The current timestep $h$ (`delta`)
- The history of recent timesteps (`delta_old`)
- The integration order (1 for backward Euler, 2 for trapezoidal)
- The method parameter $\mu$ (0.5 for standard trapezoidal)

The companion model output is then computed by `ni_integrate()`:

```text
    ni_com_cof(delta, delta_old, order, xmu) → ag[0..6]
    ni_integrate(ag, states, cap, qcap, order) → (Geq, Ieq)
```

The `Geq` and `Ieq` are what each device's `load()` function stamps into the MNA matrix during transient analysis. The `ag` coefficients are passed to every device at each timestep.

## Charge as the fundamental quantity

A subtle but important design choice in SPICE: the integration is performed on **charge** (or flux), not on voltage (or current) directly.

Why? Because charge is a conserved quantity. Numerical errors in tracking charge lead to small voltage errors, but conservation of charge is maintained. If we integrated voltage directly, numerical errors could create or destroy charge — leading to energy that appears from nowhere or disappears, and potentially causing the simulation to drift or become unstable.

For a capacitor:
- State `qcap`: charge $q = C \cdot V$
- State `qcap + 1`: current $I = dq/dt$ (computed by integration)

For an inductor:
- State `flux`: flux linkage $\phi = L \cdot I$
- State `flux + 1`: voltage $V = d\phi/dt$ (computed by integration)

The `ni_integrate()` function reads the charge from the state vectors, applies the integration formula, and writes the current back. The device then computes $G_{\text{eq}} = \text{ag}[0] \cdot C$ and $I_{\text{eq}} = I - G_{\text{eq}} \cdot V$ for stamping.

## The connection to Newton-Raphson

At each timestep, the companion models make the circuit fully algebraic — capacitors are conductances, inductors are conductances, and everything is expressed as $G \cdot V = I$. But the circuit still has nonlinear devices (diodes, transistors) that need Newton-Raphson iteration to solve.

So the full transient loop at each timestep is:

1. Compute integration coefficients `ag[]` from the timestep and history
2. Start Newton-Raphson iteration:
   - Each device computes its companion model ($G_{\text{eq}}$, $I_{\text{eq}}$) using `ag[]`
   - Each nonlinear device linearizes at the current guess (same as DC OP)
   - Stamp everything into the matrix, solve, check convergence
3. If NR converges: this timestep is tentatively accepted (pending LTE check)
4. If NR fails: reject the timestep, halve $h$, try again

The integration method determines the companion model; Newton-Raphson solves the resulting nonlinear algebraic system. They work in concert at every timestep.

<!-- TODO: interactive companion model — show a capacitor charging, at each timestep display the companion model (Geq parallel with Ieq), watch how Geq changes with timestep size -->
