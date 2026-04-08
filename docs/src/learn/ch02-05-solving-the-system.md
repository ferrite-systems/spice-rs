# Solving the System

The matrix is built. Now what? SPICE needs to solve $Gx = b$ — and it needs to do it fast, because nonlinear circuits require solving this system dozens of times per operating point.

---

## LU factorization

The workhorse is LU factorization: decompose $G$ into a lower-triangular matrix $L$ and an upper-triangular matrix $U$ such that $G = LU$. Then solving $Gx = b$ becomes two easy steps:

1. **Forward substitution:** solve $Ly = b$ for $y$ (top to bottom, each equation has one new unknown)
2. **Back substitution:** solve $Ux = y$ for $x$ (bottom to top, same idea)

For a dense $n \times n$ matrix, LU factorization is $O(n^3)$. But circuit matrices are not dense — and that changes everything.

---

## Sparsity

A 1000-node circuit has a $1000 \times 1000$ conductance matrix — one million entries. But each component touches only 2 to 4 nodes, so the vast majority of entries are zero. A typical circuit matrix is 99% zeros.

The sparse solver stores only the nonzero entries and operates only on those. Instead of $O(n^3)$, the factorization runs in time roughly proportional to the number of nonzero entries — effectively $O(n)$ for typical circuits. This is what makes SPICE practical for large designs.

spice-rs uses a Markowitz-ordered LU factorization, faithfully ported from the KLU algorithm in SuiteSparse. Chapter 19 covers the sparse solver in full detail — the pivot ordering strategies, fill-in minimization, and the data structures that make it efficient.

---

## From solution to results

After the solve, the solution vector $x$ contains everything:

**Node voltages** occupy the first $n$ positions. These are the primary output — the voltage at every node in the circuit, measured with respect to ground.

**Branch currents** for voltage sources and inductors occupy positions $n+1$ through $n+m$. These fall out of the solve for free — no extra computation needed.

**Other currents** (through resistors, into transistor terminals) are computed after the fact from the node voltages. For a resistor: $I = G(V_i - V_j)$. For a MOSFET: evaluate the device equations at the solved terminal voltages.

---

## What happens next

For a linear `.OP` analysis, the solve is done — one factorization, one forward/back substitution, and the answer is ready. But for nonlinear circuits, this is just one iteration of Newton-Raphson:

1. Linearize all devices at the current operating point
2. Stamp the linearized values into the matrix
3. Solve
4. Update the operating point
5. Check convergence — if not converged, go to step 1

The matrix is re-stamped and re-factored at every iteration. Factorization dominates the runtime, which is why the sparse solver matters so much — it's the innermost loop of the entire simulation.

For transient analysis (`.TRAN`), the simulator steps through time, solving an operating point at each timestep. Thousands of timesteps, each with multiple Newton-Raphson iterations, each requiring a matrix solve. A fast sparse solver is the difference between seconds and hours.
