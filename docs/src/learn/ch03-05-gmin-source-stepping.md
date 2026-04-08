# Gmin & Source Stepping

When direct Newton-Raphson fails to converge, SPICE doesn't give up. It has two fallback strategies that modify the circuit to make it easier to solve, then gradually remove the modifications until the original circuit is recovered. These are **gmin stepping** and **source stepping**.

In spice-rs (ported from ngspice's `CKTop`), the sequence is:

1. Try direct Newton-Raphson
2. If that fails, try **dynamic gmin stepping**
3. If that fails, try **true gmin stepping**
4. If that fails, try **Gillespie source stepping**
5. If everything fails, report "no convergence"

Most circuits converge at step 1. Steps 2-4 are safety nets that catch difficult circuits with multiple operating points, high-gain feedback loops, or latch-up conditions.

---

## Gmin stepping

### The problem

Some circuits have nodes that are connected only through nonlinear devices. At the initial guess (all voltages zero), those devices might have nearly zero conductance — meaning the node is effectively floating. A floating node makes the matrix singular or near-singular, and the solver fails.

A common example: a differential pair where both transistor gates start at 0V. Both transistors are off, neither contributes any conductance, and the output node has no path to anything.

### The solution

Add a small conductance from *every node to ground*. This is called **gmin** — a minimum conductance that ensures no node is ever truly floating.

```ferrite-circuit
circuit "Gmin Example" {
    node "a" label="VCC" rail=#true voltage="5"
    node "gnd" ground=#true
    group "circuit" topology="generic" {
        component "R1" type="resistor" role="passive" {
            value "1k"
            port "1" net="a"
            port "2" net="b"
        }
        component "gmin" type="resistor" role="shunt" {
            value "1T"
            port "1" net="b"
            port "2" net="gnd"
        }
        component "D1" type="diode" role="passive" {
            port "anode" net="b"
            port "cathode" net="gnd"
        }
    }
    node "b" label="b"
}
```

Each node gets an invisible conductance to ground (gmin ~ 1e-12 S).

With gmin present, every node has at least a tiny conductance to ground, so the matrix is always well-conditioned and the solver can find an answer — just not quite the right one, because we've added parasitic paths that don't exist in the real circuit.

### The stepping process

The trick is to start with a *large* gmin (around $10^{-2}$ S, a very noticeable conductance), solve the circuit, then gradually reduce gmin while using each solution as the starting point for the next.

1. Set $g_{\min} = 10^{-2}$ S. Solve. (Easy — heavy damping.)
2. Reduce $g_{\min}$ by a factor (typically 10). Solve, using the previous solution as the initial guess. (The previous solution is close to the new one.)
3. Repeat until $g_{\min}$ reaches its target value ($10^{-12}$ S by default).
4. Do one final solve with gmin at its target. This is the real answer.

Each step only changes the circuit slightly, so Newton-Raphson converges quickly from the previous solution. By the time gmin is back to its negligible target value, the solver has been guided smoothly to the correct operating point.

### Dynamic vs. true gmin

ngspice (and spice-rs) actually has two variants:

**Dynamic gmin (diagonal gmin):** Adds gmin to the *matrix diagonal* only. This is computationally cheaper and works for most circuits. The factor between steps adapts based on how many NR iterations the previous step required — if convergence was easy, take bigger steps; if it was hard, take smaller ones.

**True gmin (new gmin):** Adds gmin as a *per-device parameter* that appears inside each semiconductor's equations. This is more physically meaningful — it's as if each PN junction has a parallel leakage resistance — and can handle cases where diagonal gmin fails.

In the spice-rs code (`analysis/dc.rs`), the `dynamic_gmin` function manages the stepping loop with adaptive factor control and backtracking:

```rust
// From analysis/dc.rs — dynamic gmin stepping
let mut factor = config.gmin_factor;     // typically 10
let mut old_gmin = 1e-2;                 // start large
sim.diag_gmin = old_gmin / factor;

loop {
    match ni_iter(sim, circuit, config, max_iter) {
        Ok(_) => {
            if sim.diag_gmin <= gtarget {
                break; // reached target — success
            }
            // Adapt step size based on iteration count
            // ...
            sim.diag_gmin /= factor;   // reduce gmin
        }
        Err(_) => {
            // Backtrack: restore previous solution, reduce step
            factor = factor.sqrt().sqrt();
            sim.diag_gmin = old_gmin / factor;
            // ...
        }
    }
}
```

If a step fails to converge, the solver backtracks to the last successful solution and tries a smaller step. This robustness is essential — without it, a single failed step would abort the entire analysis.

---

## Source stepping

Source stepping takes a different approach: instead of modifying the circuit's connectivity, it modifies the *excitation*. All independent voltage and current sources are scaled by a factor that ramps from 0 to 1.

### The idea

With all sources at zero, every node is at 0V and every device is in a well-defined (if boring) state. Newton-Raphson converges trivially. Then:

1. Set `src_fact` = 0. Solve. (All sources off — trivial.)
2. Increase `src_fact` to 0.001 (sources at 0.1%). Solve.
3. Increase `src_fact` toward 1.0, adapting the step size.
4. At `src_fact` = 1.0, sources are at full strength. This is the real answer.

Each step, the sources get a little stronger and the operating point shifts a little. NR converges quickly at each step because the change is small.

### When source stepping helps

Source stepping is most effective for circuits where the difficulty comes from the *magnitude* of the excitation rather than the topology. A circuit with a 1000V supply and sensitive feedback might have trouble starting from a cold guess at 1000V, but can easily track the solution from 0V upward in small increments.

In spice-rs, source stepping is implemented in `gillespie_src` (named after the Gillespie algorithm variant used in ngspice). During the ramp, every voltage source's DC value and every current source's DC value is multiplied by `src_fact`. The factor is passed through to each device's `load()` function.

---

## The big picture

These convergence aids are what make SPICE practical for real circuits. Without them, even moderately complex circuits (a few dozen transistors) would routinely fail to converge. With them, SPICE can find the operating point of circuits with millions of devices.

The hierarchy of fallbacks reflects a design philosophy: try the simplest (and fastest) approach first, then progressively deploy heavier tools. Direct NR is fast but fragile. Gmin stepping is slower but more robust. Source stepping is slowest but handles the widest range of circuits.

When you see a SPICE simulator report "gmin stepping succeeded" or "source stepping required," now you know what it did and why.

<!-- TODO: interactive gmin stepping visualization — show a circuit with progressively smaller gmin conductances, watching the solution migrate from the heavily-damped state to the true operating point -->
