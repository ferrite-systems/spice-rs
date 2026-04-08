# Timestep Control

A fixed timestep is either too small (wasting computation on regions where nothing is changing) or too large (missing fast transitions and accumulating error). SPICE uses **adaptive timestep control** — the simulator dynamically adjusts $h$ based on how rapidly the solution is changing.

This is the mechanism that makes transient analysis practical. A simulation might use timesteps of picoseconds during a fast clock edge, then jump to microseconds during a quiet settling period, all within the same run. The quality of the timestep control directly determines both the accuracy and the speed of the simulation.

## The accept/reject loop

At every timestep, the transient engine follows a strict protocol:

```text
    ┌─────────────────────────────┐
    │  Choose timestep h          │
    │  Advance: t = t + h         │
    │  Compute ag[] coefficients  │
    │  Run Newton-Raphson         │
    └──────────┬──────────────────┘
               │
          ┌────▼────┐
          │ NR      │──── No ──→ REJECT: h = h/8
          │converge?│            restore t, retry
          └────┬────┘
               │ Yes
          ┌────▼────┐
          │ LTE     │──── No ──→ REJECT: h = h_new (from LTE)
          │ okay?   │            restore t, retry
          └────┬────┘
               │ Yes
          ┌────▼────┐
          │ ACCEPT  │ Record solution
          │ h = h_new│ (possibly larger)
          └─────────┘
```

Two things can cause a timestep to be rejected:

1. **Newton-Raphson failure.** The nonlinear solver didn't converge within the iteration limit. This usually means $h$ is too large — the linearization from the previous step is too far from the new solution. The remedy is aggressive: divide $h$ by 8 and retry.

2. **Local truncation error too large.** NR converged, but the numerical integration introduced too much error. The remedy is gentler: compute a new (smaller) $h$ that would satisfy the error tolerance, and retry.

If both checks pass, the step is accepted and the solution is recorded. The LTE estimator also suggests a *larger* timestep for the next step if the error is well below the tolerance.

## Local Truncation Error (LTE)

The LTE is the difference between the true solution and the numerical approximation, *at a single step*. It measures how much error the integration method introduces by taking a discrete step instead of following the continuous-time trajectory.

For the trapezoidal rule, the LTE is proportional to:

$$\text{LTE}_{\text{trap}} \propto h^3 \cdot \frac{d^3 V}{dt^3}$$

The key factors: the timestep cubed (so doubling $h$ increases the error eightfold) and the third derivative of the voltage (so the error is largest when the waveform is changing most rapidly — at edges and transitions).

SPICE doesn't compute the third derivative directly. Instead, it uses the **predictor-corrector difference**: the integration produces a corrector value, and a simpler formula produces a predictor value. The difference between them estimates the LTE.

## The LTE computation in spice-rs

The `ckt_terr()` function in [`integration.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/integration.rs) computes the maximum safe timestep for each charge state variable, following ngspice's `CKTterr()`:

1. **Compute divided differences** from the charge history. For order $k$, this builds a $(k+1)$-th order divided difference from $q_{n+1}, q_n, q_{n-1}, \ldots$ and the timestep history. The highest-order divided difference estimates the derivative that the integration method cannot represent.

2. **Compute the tolerance.** The error tolerance is the maximum of two quantities:
   - `volttol`: absolute tolerance plus relative tolerance scaled by the current magnitude
   - `chargetol`: relative tolerance scaled by the charge magnitude, divided by the timestep

   Taking the maximum ensures that both voltage accuracy and charge conservation accuracy are maintained.

3. **Compute the safe timestep.** The formula is:

$$h_{\text{new}} = \left(\frac{\text{trtol} \cdot \text{tol}}{\text{factor} \cdot |D^{k+1}|}\right)^{1/k}$$

where $D^{k+1}$ is the divided difference, `factor` is a method-dependent coefficient (0.5 for trapezoidal order 1, 1/12 for trapezoidal order 2), and `trtol` is the transient error tolerance multiplier (default: 7.0).

The exponent $1/k$ comes from the order of the method: for a second-order method, the error is proportional to $h^3$, so to scale the error by a factor $r$, you scale $h$ by $r^{1/2}$.

## The timestep floor

There's a hard lower bound on the timestep: `delmin = 1e-11 * max_step`. This is a machine-precision floor — below this, floating-point arithmetic can't meaningfully distinguish one time point from the next.

If the timestep hits `delmin` and the simulation still can't proceed (NR diverges or LTE can't be satisfied), the simulation fails with a "timestep too small" error. This usually indicates a modeling problem — a discontinuity that the circuit doesn't physically have, or a device model with a numerical issue at the operating point.

From `transient()` in [`analysis/transient.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/analysis/transient.rs):

```text
    delmin = 1e-11 * max_step

    if delta <= delmin:
        if old_delta > delmin:
            delta = delmin     (try once at the floor)
        else:
            error: timestep too small
```

## The acceptance threshold: 0.9

The LTE check doesn't require `h_new >= h`. It requires:

```text
    if new_delta > 0.9 * delta:
        ACCEPT
    else:
        REJECT (the timestep should have been smaller)
```

The 0.9 factor provides a margin — if the LTE says the timestep should be reduced by more than 10%, the step is rejected and recomputed. If the reduction is less than 10%, the step is accepted (the error is within tolerance, just not optimally efficient) and the next step will use the smaller timestep.

This avoids excessive rejection. Without the 0.9 factor, the engine would reject steps that are only slightly too large, wasting the NR computation already performed.

## The doubling cap

After a successful step, the LTE formula might suggest a very large next timestep — perhaps because the waveform just entered a flat region. But jumping too far is dangerous: there might be a fast transition just ahead that the LTE can't predict.

The `ckt_trunc()` function caps the growth:

```text
    new_delta = MIN(2 * delta, timetemp)
```

where `timetemp` is the LTE-computed safe timestep. The $2\times$ cap ensures the timestep never more than doubles in a single step, allowing a gradual ramp-up to larger steps.

## Putting it together

The full timestep control dance for a typical simulation:

```text
    Time ─────────────────────────────────────────────►
    h    ▕▏ ▕▏ ▕▏  ▕──▏  ▕────▏  ▕────────▏  ▕────────▏
              │        │           │
              │        │           └── Flat region: h grows
              │        └── Settling: h gradually increases
              └── Fast edge: h shrinks (breakpoint + LTE)

    Small h at transitions, large h in quiet regions.
    Rejected steps (not shown) would appear as gaps
    where the simulator backtracks and retries.
```

A well-tuned transient engine spends most of its computation on the interesting parts of the waveform — the edges, the ringing, the settling — and races through the flat parts with minimal effort. The LTE mechanism ensures that the accuracy is roughly uniform throughout, regardless of the timestep size.

<!-- TODO: interactive timestep visualization — run a transient sim, show accepted/rejected steps on a timeline, color-code by step size, hover for LTE details -->
