# Convergence

Newton-Raphson doesn't always work. The quadratic convergence guarantee holds only when you start close enough to the solution. When the initial guess is far away — or when the circuit has multiple stable states — the iteration can fail in several ways.

---

## What convergence looks like

A healthy NR iteration produces a sequence of voltages that settle down quickly:

```text
Iteration 1:  Vb = 0.000 V    (initial guess)
Iteration 2:  Vb = 0.702 V    (big jump — getting in range)
Iteration 3:  Vb = 0.651 V    (refining)
Iteration 4:  Vb = 0.6492 V   (close)
Iteration 5:  Vb = 0.6491 V   (converged — change < tolerance)
```

Each iteration, the change gets smaller. Once the change is below the tolerance threshold, the loop declares convergence and stops.

---

## What can go wrong

### Oscillation

If the I-V curve has regions of very high curvature, Newton-Raphson can overshoot on one iteration and undershoot on the next, bouncing back and forth without settling:

```text
Iteration 1:  Vb = 0.000 V
Iteration 2:  Vb = 5.000 V    (overshot)
Iteration 3:  Vb = -2.300 V   (undershot)
Iteration 4:  Vb = 7.100 V    (overshot again)
...
Iteration 100: FAILED — iteration limit exceeded
```

This happens because the tangent-line approximation is only good locally. At $V_b = 0$, the diode's exponential curve is nearly flat, so the tangent line shoots off to a huge voltage. At that huge voltage, the exponential is astronomical, so the tangent line plunges back to a very negative voltage.

Voltage limiting (Chapter 4) directly addresses this problem by preventing the solver from taking steps that are too large for the exponential to handle.

### Slow convergence

Some circuits converge, but painfully slowly:

```text
Iteration 1:   Vb = 0.000 V
Iteration 2:   Vb = 0.400 V
Iteration 3:   Vb = 0.500 V
Iteration 4:   Vb = 0.560 V
...
Iteration 45:  Vb = 0.648 V
Iteration 46:  Vb = 0.649 V   (finally converged)
```

This typically means the circuit has near-singular behavior — nodes that are weakly connected to anything, or regions where multiple devices interact in a way that creates very flat (but not quite flat) solution landscapes.

### No solution found

In rare cases, the NR iteration hits the iteration limit (typically 100 or 150 iterations for DC operating point) and gives up. This doesn't necessarily mean the circuit has no DC operating point — it means the solver couldn't find one from its starting point using the basic approach.

---

## The convergence test

In spice-rs (ported from ngspice's `NIconvTest`), convergence is declared when *every* unknown in the system has stabilized. For each node voltage:

$$|V_i^{\text{new}} - V_i^{\text{old}}| < \text{reltol} \cdot \max(|V_i^{\text{new}}|, |V_i^{\text{old}}|) + \text{vntol}$$

For each branch current:

$$|I_k^{\text{new}} - I_k^{\text{old}}| < \text{reltol} \cdot \max(|I_k^{\text{new}}|, |I_k^{\text{old}}|) + \text{abstol}$$

where the default tolerances are:
- `reltol` = $10^{-3}$ (0.1% relative change)
- `vntol` = $10^{-6}$ V (1 microvolt absolute)
- `abstol` = $10^{-12}$ A (1 picoamp absolute)

The absolute tolerance matters for signals near zero — without it, a node voltage that bounced between $10^{-15}$ and $10^{-14}$ would never "converge" by the relative test alone, even though both values are essentially zero.

In addition to the node-level test, spice-rs (like ngspice) runs a **per-device convergence test**. Each nonlinear device checks that its terminal voltages haven't changed by more than its own tolerance. For diodes and MOSFETs, this catches cases where the overall node voltages look stable but the device's operating point is still shifting.

---

## The noncon flag

Inside the NR loop, spice-rs tracks a flag called `noncon` (short for "non-convergence"). Each device's `load()` function can set this flag if it applied voltage limiting — a signal that the device hasn't reached its natural operating point yet and more iterations are needed.

The convergence test only runs when `noncon` is already 0. If any device flagged non-convergence during the load step, the solver skips the test and goes directly to the next iteration. This prevents premature convergence when devices are still being actively limited.

```rust
// From solver.rs
if sim.noncon == 0 && sim.iter_count > 1 {
    sim.noncon = ni_conv_test(sim, circuit, config);
} else {
    sim.noncon = 1;
}
```

The `iter_count > 1` check ensures we always do at least two iterations. A single iteration can't converge — we need at least two values to compare.
