# Gear (BDF) Methods

The trapezoidal rule's lack of numerical damping makes it vulnerable to ringing on stiff circuits. The **Gear methods** — also called **Backward Differentiation Formulas (BDF)** — are the standard alternative. They trade some accuracy for guaranteed damping of spurious oscillations.

## The idea

Instead of approximating the integral (like the trapezoidal rule), BDF methods approximate the *derivative* directly using a backward difference formula. The order-$k$ BDF uses values at $k+1$ time points — the current point and the $k$ most recent past points — to estimate the derivative at the current time.

**Gear-1** (backward Euler) uses one past point:

$$\frac{y_{n+1} - y_n}{h} = f_{n+1}$$

**Gear-2** uses two past points:

$$\frac{1}{h}\left(\frac{3}{2}y_{n+1} - 2y_n + \frac{1}{2}y_{n-1}\right) = f_{n+1}$$

Higher orders use more past points for higher accuracy, up to Gear-6.

```text
    BDF methods use backward time points

    ──●────────●────────●────────●──── t
    n-2      n-1       n       n+1
              │         │        │
              └────┬────┘        │
              Gear-2 uses        │
              these 3 points ────┘
                   to estimate
                   the derivative
                   at t_{n+1}
```

All BDF methods are **implicit** — $f_{n+1}$ depends on $y_{n+1}$, so Newton-Raphson iteration is still required at each step. The companion model structure is the same: an equivalent conductance plus a current source.

## Why BDF methods damp ringing

The trapezoidal rule is symmetric in time — it weights the old and new derivatives equally. BDF methods are *asymmetric*: they weight the new value more heavily. This asymmetry introduces numerical damping that suppresses the high-frequency oscillations that plague the trapezoidal rule on stiff circuits.

The cost of this damping: BDF methods are only **A($\alpha$)-stable**, not A-stable like trapezoidal. For orders 1 and 2, this distinction doesn't matter in practice (Gear-1 and Gear-2 are stable for all circuits SPICE encounters). For orders 3-6, the stability region narrows, and very stiff circuits may require the timestep to be limited for stability rather than accuracy. This is why most SPICE implementations cap the maximum order at 2 or 3.

## Gear-2 in detail

Gear-2 is the most commonly used BDF method in SPICE, and it's the natural companion to the trapezoidal rule. For a capacitor:

$$I_{n+1} = \frac{C}{h}\left(\frac{3}{2}V_{n+1} - 2V_n + \frac{1}{2}V_{n-1}\right)$$

The companion model:

$$G_{\text{eq}} = \frac{3C}{2h}, \qquad I_{\text{eq}} = \frac{C}{h}\left(-2V_n + \frac{1}{2}V_{n-1}\right)$$

Compared to the trapezoidal companion model ($G_{\text{eq}} = 2C/h$), the Gear-2 conductance is slightly smaller ($3C/2h$ vs. $2C/h$). The current source now depends on *two* past voltages, reflecting the use of two history points.

## Accuracy comparison

Each integration method has an order of accuracy — the power of $h$ in the local truncation error:

| Method | Order | LTE proportional to | Notes |
|:--|:-:|:--|:--|
| Backward Euler (Gear-1) | 1 | $h^2 \cdot d^2V/dt^2$ | Maximum damping, minimum accuracy |
| Trapezoidal | 2 | $h^3 \cdot d^3V/dt^3$ | No damping, good accuracy |
| Gear-2 | 2 | $h^3 \cdot d^3V/dt^3$ | Some damping, same order as trap |
| Gear-3 | 3 | $h^4 \cdot d^4V/dt^4$ | More accurate, narrower stability |
| Gear-4 | 4 | $h^5 \cdot d^5V/dt^5$ | Rarely needed in practice |

Trapezoidal and Gear-2 have the same *order* of accuracy, but the trapezoidal rule has a smaller error constant. For smooth waveforms, trapezoidal gives more accuracy per step. For stiff waveforms (sharp transitions followed by slow settling), Gear-2 can take larger steps without ringing, often making it faster overall despite lower accuracy per step.

## The integration coefficients

In spice-rs, all integration methods are encoded through the `ag[]` coefficient array computed by `ni_com_cof()` in [`integration.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/integration.rs). The device code doesn't know or care which method is being used — it just stamps `ag[0] * C` as the conductance and uses `ni_integrate()` to compute the current.

For the trapezoidal method currently implemented in spice-rs:

```text
    Order 1 (backward Euler):
        ag[0] = 1/h          (conductance coefficient)
        ag[1] = -1/h         (history coefficient)

    Order 2 (trapezoidal, xmu=0.5):
        ag[0] = 2/h          (conductance coefficient)
        ag[1] = 1            (history coefficient)
```

The BDF/Gear method would use different coefficients for orders 2+. For Gear-2:

```text
    Gear-2:
        ag[0] = 3/(2h)       (conductance coefficient)
        ag[1], ag[2]          (history coefficients for q_n, q_{n-1})
```

The beauty of this design is that switching integration methods requires changing only `ni_com_cof()` and `ni_integrate()`. The rest of the simulation engine — the NR loop, the timestep control, the breakpoint handling — is method-agnostic.

## Variable-order strategy

ngspice (and spice-rs) uses a variable-order strategy during transient analysis:

1. **Start at order 1** (backward Euler). At the beginning of the simulation and at breakpoints, the solution may have discontinuities. Backward Euler's strong damping handles these gracefully.

2. **Promote to order 2** when the solution is smooth. After each accepted step at order 1, the engine computes what the LTE-based timestep would be at order 2. If order 2 would allow a significantly larger step (more than 5% larger), promote. Otherwise, stay at order 1 — the higher order isn't buying anything.

3. **Drop back to order 1** at breakpoints. When the simulator hits a waveform edge (PULSE rise, PWL corner), it resets to order 1 to handle the potential discontinuity.

This dynamic order selection is handled in the acceptance branch of the main transient loop in `transient()`:

```text
    if order == 1 and max_order > 1:
        newdelta2 = ckt_trunc(order=2)
        if newdelta2 > 1.05 * delta:
            promote to order 2 (more efficient)
        else:
            stay at order 1
```

The 1.05 factor provides hysteresis — the engine doesn't oscillate between orders when they give similar timesteps.

## When to use Gear methods

In practice, the choice between trapezoidal and Gear is made by the user via a SPICE option (`.OPTIONS METHOD=GEAR`). The default is trapezoidal, which works well for most circuits. Gear methods are preferred when:

- The circuit has **sharp switching transitions** followed by slow settling (digital circuits, power converters)
- You see **unexplained oscillation** in the simulation output that isn't physically real
- The circuit has **widely separated time constants** (e.g., a fast clock driving a slow thermal network)

spice-rs currently implements the trapezoidal method (orders 1-2). BDF/Gear support for higher orders is a future extension — the `ag[]` framework is already in place.

<!-- TODO: interactive method comparison — same circuit simulated with trapezoidal vs Gear-2, overlay the waveforms to show ringing vs damping -->
