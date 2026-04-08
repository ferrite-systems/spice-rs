# Newton-Raphson Iteration

Newton-Raphson is the algorithm that turns a nonlinear problem into a sequence of linear ones. The idea: at each step, *linearize* every nonlinear device around the current guess, solve the resulting linear system, and use the solution as the next guess. Repeat until the guess stops changing.

## The algorithm

1. **Start with a guess** for all node voltages (typically 0V, or a smarter initial estimate)
2. **Linearize** every nonlinear device at the current guess — replace each device with a conductance and current source that match the device's behavior at that operating point
3. **Stamp** the linearized models into the MNA matrix
4. **Solve** the linear system to get new node voltages
5. **Check convergence** — if the new voltages are close enough to the old ones, stop
6. **Otherwise**, go back to step 2 with the new voltages as the guess

Each pass through this loop is called a **Newton-Raphson iteration** (or just an "NR iteration"). Typical circuits converge in 5-15 iterations.

---

## Linearization: the companion model

At each iteration, the diode from the previous section is replaced by its **companion model** — a linear circuit that behaves identically to the diode *at the current operating point*.

Given a guess $V_b^{(k)}$ at iteration $k$, evaluate the Shockley equation:

$$I_d^{(k)} = I_s\left(e^{V_b^{(k)} / nV_t} - 1\right)$$

and its derivative (the small-signal conductance):

$$g_d^{(k)} = \frac{I_s}{nV_t} \cdot e^{V_b^{(k)} / nV_t}$$

The linearized diode is a conductance $g_d^{(k)}$ in parallel with a current source $I_{eq}^{(k)}$:

$$I_{eq}^{(k)} = I_d^{(k)} - g_d^{(k)} \cdot V_b^{(k)}$$

This is the tangent-line approximation to the I-V curve. At $V_b = V_b^{(k)}$, the companion model produces exactly the same current as the real diode. Near that point, it's a good approximation. Far from it, it may be wildly wrong — but that's fine, because we'll re-linearize at the new solution and iterate.

The companion model stamps into the matrix exactly like a resistor (conductance $g_d^{(k)}$) plus a current source ($I_{eq}^{(k)}$):

$$G[b,b] \mathrel{+}= g_d^{(k)}$$
$$b[b] \mathrel{-}= I_{eq}^{(k)}$$

(The signs follow the standard MNA convention — current source contribution enters the RHS with appropriate sign from the Norton equivalent.)

<!-- TODO: interactive tangent-line animation — show the I-V curve with a tangent line that slides as V_b changes, converging toward the solution -->

---

## A worked example

Let's trace Newton-Raphson on our diode circuit: R1 = 1k between node `a` (5V) and node `b`, diode from `b` to ground. We use $I_s = 10^{-14}$ A, $n = 1$, $V_t = 0.02585$ V.

**Iteration 1:** Guess $V_b = 0$ V.

Evaluate: $I_d = I_s(e^0 - 1) = 0$, $g_d = I_s / V_t = 3.87 \times 10^{-13}$ S.

The diode is essentially an open circuit. The matrix solve gives $V_b \approx 5$ V.

That's way too high — at 5V forward bias, a real diode would be passing amps of current. But we've got a new guess.

**Iteration 2:** Guess $V_b = 5$ V.

In practice, spice-rs would apply **voltage limiting** here (covered in Chapter 4) to prevent the exponential $e^{5/0.026}$ from overflowing. Limiting would clamp this to something reasonable, like $V_b \approx 0.7$ V. But to see the pure algorithm, let's suppose we somehow evaluate it.

**After a few iterations** (with limiting doing its job), the guesses converge:

| Iteration | $V_b$ guess | $I_d$ (mA) | $g_d$ (S) |
|---|---|---|---|
| 1 | 0.000 V | 0.000 | 3.87e-13 |
| 2 | ~0.700 V | 4.30 | 0.166 |
| 3 | ~0.651 V | 4.35 | 0.168 |
| 4 | ~0.649 V | 4.35 | 0.168 |
| 5 | ~0.649 V | 4.35 | 0.168 | 

By iteration 4-5, the voltages have stopped changing to within tolerance. The operating point is $V_b \approx 0.649$ V, $I_d \approx 4.35$ mA.

---

## Quadratic convergence

Newton-Raphson has a remarkable property: once you're close to the answer, the number of correct digits roughly **doubles** with each iteration. If iteration $k$ is off by $10^{-3}$, iteration $k+1$ is off by about $10^{-6}$, and $k+2$ by about $10^{-12}$.

This is called **quadratic convergence**, and it's why NR is so effective — once the iterates enter the "convergence basin" around the true solution, they converge extremely fast. The challenge is getting close enough for this rapid convergence to kick in. Most of the iterations in a typical simulation are spent in the early phase, feeling out the right neighborhood.

---

## How spice-rs implements this

The NR loop lives in `solver.rs`, in the function `ni_iter` — a faithful port of ngspice's `NIiter`. Here is the core structure, with tracing and profiling stripped away:

```rust
loop {
    // 1. Clear matrix and RHS
    sim.mna.clear();

    // 2. Load all devices — each one linearizes at current guess
    //    and stamps its companion model into the matrix
    for device in &mut circuit.devices {
        device.load(&mut sim.mna, ...)?;
    }

    // 3. Add diagonal gmin for numerical stability
    sim.mna.add_diag_gmin(sim.diag_gmin);

    // 4. Factor and solve the linear system
    sim.mna.solve()?;

    // 5. Check convergence — compare new solution to old
    if sim.noncon == 0 && sim.iter_count > 1 {
        sim.noncon = ni_conv_test(sim, circuit, config);
    }

    // 6. If converged, return; otherwise swap and iterate
    if sim.noncon == 0 {
        return Ok(sim.iter_count);
    }
    sim.mna.swap_rhs();
}
```

Each device's `load()` function is where linearization happens. For the diode, it evaluates the Shockley equation at the current voltage (from `rhs_old`), computes $g_d$ and $I_{eq}$, and stamps them into the matrix. For a MOSFET, the process is the same — just with a more complex I-V relationship.

The convergence test in `ni_conv_test` compares every node voltage and branch current between the current solution (`rhs`) and the previous one (`rhs_old`). The tolerance is:

$$|V^{(k+1)} - V^{(k)}| < \text{reltol} \cdot \max(|V^{(k)}|, |V^{(k+1)}|) + \text{vntol}$$

where `reltol` = $10^{-3}$ and `vntol` = $10^{-6}$ V by default. The combination of relative and absolute tolerance handles both large signals and small ones gracefully.
