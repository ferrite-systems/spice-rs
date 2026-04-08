# Voltage Sources

A voltage source doesn't fit the conductance stamp pattern. It forces a specific voltage across two nodes — it doesn't define a current as a function of voltage. This is why MNA needs the "Modified" in its name.

---

## The problem

A resistor says: *given a voltage, here is the current.* A voltage source says: *the voltage across me shall be $V_s$, whatever current that requires.* There is no conductance to stamp. The current through the source is a free variable determined by the rest of the circuit.

In plain Nodal Analysis (without the "M"), voltage sources are impossible to handle directly. MNA solves this by adding a new unknown and a new equation.

---

## The branch current variable

For a voltage source $V_s$ between nodes $i$ (positive) and $j$ (negative), MNA introduces a new unknown: the branch current $I_{Vs}$.

If there are $n$ nodes and this is the first voltage source, $I_{Vs}$ occupies position $n+1$ in the solution vector. The matrix grows from $n \times n$ to $(n+1) \times (n+1)$.

---

## The stamp

The voltage source contributes two things:

**KCL contribution** — the branch current enters node $i$ and leaves node $j$:

$$G[i, n{+}1] \mathrel{+}= 1 \qquad G[j, n{+}1] \mathrel{-}= 1$$

This says: when computing the current sum at node $i$, include $+I_{Vs}$. At node $j$, include $-I_{Vs}$.

**Voltage constraint** — the new equation in row $n+1$:

$$G[n{+}1, i] \mathrel{+}= 1 \qquad G[n{+}1, j] \mathrel{-}= 1 \qquad b[n{+}1] \mathrel{+}= V_s$$

This says: $V_i - V_j = V_s$. It's a constraint, not a KCL equation — but it lives in the same matrix, solved by the same solver.

---

## The full pattern

For a voltage source between node $i$ (+) and node $j$ (-), with branch index $k = n+1$:

|  | ... $V_i$ ... $V_j$ ... | $I_{Vs}$ | $b$ |
|---|---|---|---|
| **Row $i$ (KCL)** | | $+1$ | |
| **Row $j$ (KCL)** | | $-1$ | |
| **Row $k$ (constraint)** | $+1$ ... $-1$ | $0$ | $V_s$ |

The 1's and -1's are dimensionless — they couple the current variable into KCL and the voltage variables into the constraint. The matrix remains sparse: only 4 off-diagonal entries plus the RHS value.

---

## Why the current comes out negative

When you simulate a simple circuit with a voltage source supplying power, the branch current $I_{Vs}$ is typically negative. This is not a bug — it's a consequence of the sign convention. MNA defines $I_{Vs}$ as current flowing from the positive terminal through the source to the negative terminal (i.e., internally). In a source supplying current to the circuit, current flows out of the positive terminal externally, which is the opposite direction. Hence the negative sign.
