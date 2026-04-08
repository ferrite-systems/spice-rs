# DC Operating Point

The DC operating point is the answer to the most basic question a circuit simulator can ask: *if nothing is changing, what are all the voltages and currents?*

Turn off every time-varying source. Remove every signal. Let the circuit settle into its steady state. The voltages and currents you find there are the **DC operating point** — and nearly everything else SPICE does (transient analysis, AC analysis, noise analysis) starts from it.

For a circuit with only resistors and voltage sources, finding the operating point is straightforward: assemble the MNA matrix from Chapter 2, solve it, done. One linear system, one solution.

But real circuits have diodes and transistors. These devices are *nonlinear* — their relationship between voltage and current isn't a straight line. The matrix coefficients depend on the very voltages we're trying to find, which means we can't just solve the system in one step. We need to iterate.

This chapter introduces the algorithm that makes it work: **Newton-Raphson iteration**. It is the beating heart of every SPICE simulator.

---

## The plan

We'll build up the idea in stages:

1. **Linear circuits** — a single matrix solve, no iteration needed
2. **Nonlinear circuits** — why a diode makes the problem fundamentally harder
3. **Newton-Raphson** — the iterative algorithm that handles nonlinearity: guess, linearize, solve, repeat
4. **Convergence** — what it means to converge, and what can go wrong
5. **Gmin and source stepping** — fallback strategies when Newton-Raphson needs help getting started

By the end, you'll understand exactly what spice-rs is doing when you run `.OP` — and why it sometimes takes dozens of iterations to find an answer, or occasionally fails to find one at all.

<!-- TODO: interactive convergence animation — show NR iterations spiraling toward solution on I-V curve -->
