# Modified Nodal Analysis

Every SPICE simulator works the same way: it turns a circuit into a matrix equation $Gx = b$, then solves for $x$. The method for building that matrix is called **Modified Nodal Analysis** (MNA).

This chapter shows exactly how MNA works — how each component becomes entries in a matrix, and how solving that matrix gives you every voltage and current in the circuit.

---

## The unknowns

The solution vector $x$ contains two kinds of unknowns:

1. **Node voltages** — one for every node in the circuit (except ground, which is defined as 0V)
2. **Branch currents** — one for every voltage source and every inductor

If a circuit has $n$ nodes (excluding ground) and $m$ voltage sources, the matrix is $(n+m) \times (n+m)$.

> The "Modified" in MNA refers to the addition of branch currents. Plain Nodal Analysis only solves for node voltages, which makes it unable to handle voltage sources directly. MNA adds extra equations for each voltage source, making the system slightly larger but much more general.

---

## Conductance stamps

Each component contributes entries to the matrix $G$ and the right-hand side $b$. These contributions are called **stamps** — the component "stamps" its values into the matrix.

### Resistor

A resistor $R$ between nodes $i$ and $j$ has conductance $G = 1/R$. It stamps a 2×2 pattern:

$$G[i,i] \mathrel{+}= G \qquad G[i,j] \mathrel{-}= G$$
$$G[j,i] \mathrel{-}= G \qquad G[j,j] \mathrel{+}= G$$

The pattern is symmetric: positive on the diagonal, negative on the off-diagonal. This is the most important stamp in SPICE — it shows up everywhere because even nonlinear devices are linearized into an equivalent conductance at each iteration.

Here is how spice-rs implements it:

```rust
// From device/resistor.rs — the load() function
let g = self.conductance;
mna.stamp(self.pos_node, self.pos_node,  g);
mna.stamp(self.neg_node, self.neg_node,  g);
mna.stamp(self.pos_node, self.neg_node, -g);
mna.stamp(self.neg_node, self.pos_node, -g);
```

Four calls to `stamp()`. Each one adds a value to one matrix entry. That's all a resistor does.

### Current source

An independent current source $I$ from node $j$ to node $i$ (current flows from $j$ to $i$) stamps only the right-hand side:

$$b[i] \mathrel{+}= I \qquad b[j] \mathrel{-}= I$$

No matrix entries — a current source doesn't depend on any node voltage, so it contributes no conductance.

### Voltage source

A voltage source $V_s$ between nodes $i$ and $j$ (positive terminal at $i$) adds a new unknown: the branch current $I_{Vs}$. If this is the $k$-th branch variable, it stamps:

$$G[i, n{+}k] \mathrel{+}= 1 \qquad G[j, n{+}k] \mathrel{-}= 1$$
$$G[n{+}k, i] \mathrel{+}= 1 \qquad G[n{+}k, j] \mathrel{-}= 1$$
$$b[n{+}k] \mathrel{+}= V_s$$

The first two rows express KCL: the branch current $I_{Vs}$ enters node $i$ and leaves node $j$. The last row enforces the voltage constraint: $V_i - V_j = V_s$.

This is why voltage sources add a row and column to the matrix — they introduce both a new unknown (the branch current) and a new equation (the voltage constraint).

---

## Building the full matrix

Let's assemble the matrix for the voltage divider from Chapter 1:

```
V1 in 0 DC 10
R1 in mid 1k
R2 mid 0 1k
.OP
```

**Nodes:** `in` = node 1, `mid` = node 2, `gnd` = node 0 (ground, eliminated)

**Branch variables:** V1 contributes branch current $I_{V1}$ in position 3

The system is 3×3:

| | $V_1$ (in) | $V_2$ (mid) | $I_{V1}$ | = | $b$ |
|---|---|---|---|---|---|
| **KCL node 1** | $G_1$ | $-G_1$ | $1$ | | $0$ |
| **KCL node 2** | $-G_1$ | $G_1 + G_2$ | $0$ | | $0$ |
| **V1 equation** | $1$ | $0$ | $0$ | | $10$ |

Where $G_1 = G_2 = 1/1000 = 0.001$ S.

Filling in the numbers:

$$\begin{bmatrix} 0.001 & -0.001 & 1 \\\\ -0.001 & 0.002 & 0 \\\\ 1 & 0 & 0 \end{bmatrix} \begin{bmatrix} V_{in} \\\\ V_{mid} \\\\ I_{V1} \end{bmatrix} = \begin{bmatrix} 0 \\\\ 0 \\\\ 10 \end{bmatrix}$$

Solving this system gives: $V_{in} = 10\text{V}$, $V_{mid} = 5\text{V}$, $I_{V1} = -0.01\text{A}$.

The negative current means 10 mA flows *into* V1's positive terminal — the source is supplying current, as expected.

---

## How spice-rs builds this

In spice-rs, the `MnaSystem` holds the matrix and right-hand side vector. The process follows ngspice exactly:

1. **Allocate the matrix** — `MnaSystem::new(size)` creates a sparse matrix with room for all nodes and branch variables
2. **Register elements** — Each device calls `setup_matrix()` to tell the MNA system which matrix positions it will use. The sparse solver pre-allocates entries at those positions.
3. **Clear and stamp** — At each iteration, `clear()` zeros the matrix, then each device's `load()` function stamps its current values
4. **Solve** — The sparse solver (Markowitz LU factorization) factors and solves the system in-place

The matrix is sparse — a 1000-node circuit might have a 1000×1000 matrix, but only ~5000 nonzero entries (most components touch just 2-4 nodes). The sparse solver exploits this structure to solve the system in $O(n)$ time rather than $O(n^3)$.

---

## The stamp pattern

Every component in SPICE reduces to the same operation: *stamp values into the matrix and RHS*. This is true for resistors, capacitors, diodes, MOSFETs, transmission lines — everything.

For linear components (resistors, linear capacitors at a given frequency), the stamps are constant. For nonlinear components (diodes, transistors), the stamps change at each Newton-Raphson iteration as the linearization point updates. But the mechanism is identical: call `mna.stamp()` with a row, column, and value.

This uniformity is the key insight of MNA — it reduces the entire problem of circuit simulation to: *fill a matrix, solve it, repeat*.

The next chapters show what happens when the stamps aren't constant — when Newton-Raphson iteration is needed to handle nonlinear devices.

---

## Try it

Here is a three-resistor network. Press **Run** to simulate, then try changing the resistor values.

```ferrite-circuit
circuit "Three Resistor Network" {
    node "in" label="VIN" rail=#true voltage="12"
    node "gnd" ground=#true
    group "chain" topology="generic" {
        component "R1" type="resistor" role="passive" {
            value "1k"
            port "1" net="in"
            port "2" net="a"
        }
        component "R2" type="resistor" role="passive" {
            value "2k"
            port "1" net="a"
            port "2" net="b"
        }
        component "R3" type="resistor" role="passive" {
            value "3k"
            port "1" net="b"
            port "2" net="gnd"
        }
    }
    node "a" label="Va"
    node "b" label="Vb"
}
```

The matrix for this circuit is 4×4 (3 nodes + 1 voltage source branch). Each resistor stamps its 2×2 conductance block, the voltage source adds its row and column. The sparse solver handles the rest.
