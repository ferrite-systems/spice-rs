# Transient Analysis

Transient analysis answers the most direct question you can ask a circuit simulator: *given these input signals, what are the voltages and currents as a function of time?*

Apply a step to an RC circuit and watch the exponential charge. Toggle a clock signal into a logic gate and see the propagation delay. Feed a pulse into a transmission line and watch it ring. Transient analysis simulates all of this by solving the circuit equations at every timestep from $t = 0$ to the end of the simulation.

Unlike DC analysis (one operating point, one solve) or AC analysis (linearized, one solve per frequency), transient analysis is the **full nonlinear problem** at every single timestep. Every Newton-Raphson iteration from Chapter 3 runs at every time point. Capacitor voltages and inductor currents evolve according to their differential equations, and the simulator must track these continuously. This makes transient analysis by far the most computationally expensive analysis in SPICE.

The fundamental challenge is this: the real circuit evolves in continuous time, but a computer can only work at discrete time points. The art of transient analysis is choosing those time points wisely — close enough together to be accurate, far enough apart to be efficient — and connecting them with numerical integration methods that faithfully approximate the continuous-time behavior.

---

## What makes it hard

Three things make transient analysis more difficult than DC or AC:

**1. Differential equations.** Capacitors and inductors have time-dependent behavior: $I = C\,dV/dt$ for a capacitor, $V = L\,dI/dt$ for an inductor. SPICE cannot solve differential equations directly. It must convert them into algebraic equations using numerical integration — replacing the derivatives with finite differences. The choice of integration method (trapezoidal, Gear, etc.) affects both accuracy and stability.

**2. Nonlinearity at every step.** The circuit has diodes, transistors, and other nonlinear devices. At each timestep, Newton-Raphson must iterate to find the solution. A transient simulation of 1000 timesteps might require 3-10 NR iterations per step — that's 3,000 to 10,000 matrix solves.

**3. Accuracy vs. efficiency.** A small timestep gives high accuracy but takes forever. A large timestep is fast but can miss fast transitions or accumulate error. The simulator must *adaptively* choose the timestep — making it smaller when voltages are changing rapidly and larger when things are settling. Getting this right is one of the most delicate parts of SPICE.

---

## The plan

This chapter covers each piece of the transient analysis engine:

1. **[Numerical integration](ch09-01-numerical-integration.md)** — the core idea: replacing $dV/dt$ with a difference equation that turns a capacitor into a conductance plus a current source (the "companion model")

2. **[Trapezoidal rule](ch09-02-trapezoidal.md)** — the default integration method: second-order accurate, A-stable, but prone to numerical ringing on stiff circuits

3. **[Gear methods](ch09-03-gear-methods.md)** — BDF methods: better for stiff circuits, variable order, the workhorse for semiconductor device simulation

4. **[Timestep control](ch09-04-timestep-control.md)** — how SPICE adaptively chooses $h$ using local truncation error, and the accept/reject loop that keeps accuracy in bounds

5. **[Breakpoints](ch09-05-breakpoints.md)** — forcing the simulator to land on the exact moments when sources have abrupt transitions

6. **[Transient circuits](ch09-06-transient-circuits.md)** — RC step response and MOSFET switching as concrete examples

By the end, you'll understand the full transient simulation loop in spice-rs — from the DC operating point at $t = 0$ through every accepted and rejected timestep to the final simulation time — and why the code in [`analysis/transient.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/analysis/transient.rs) is structured the way it is.

<!-- TODO: interactive transient overview — show a circuit with a PULSE source, step through the simulation one timestep at a time, show accepted/rejected steps and the adaptive timestep -->
