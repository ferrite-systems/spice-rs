# Node Voltages

The solution vector in MNA starts with one unknown per circuit node — except ground. Understanding why ground is special, and how nodes get numbered, clarifies the entire matrix structure.

---

## Ground is the reference, not a variable

Voltage is always measured *between* two points. To get absolute numbers, SPICE defines one node as the reference and fixes it at 0V. This is ground — node 0 in SPICE netlists.

Ground does not appear in the solution vector. It has no row and no column in the matrix. Every other node voltage is implicitly measured with respect to it. When we write $V_a = 5\text{V}$, we mean the potential difference between node `a` and ground is 5V.

This is why every SPICE netlist must have a node 0. Without a reference, the system of equations is underdetermined — you can add any constant to all voltages and still satisfy KCL. Fixing ground removes that degree of freedom and makes the solution unique.

---

## Node numbering

In a netlist, nodes have names: `in`, `out`, `vdd`, `0`. Internally, the simulator assigns each non-ground node a sequential index starting at 1. This index is the row (and column) position in the MNA matrix.

```
V1 in 0 DC 5      -- "in" → node 1
R1 in out 1k       -- "out" → node 2
R2 out 0 1k
```

The solution vector is $x = [V_1, V_2, ...]^T$ where $V_1$ is the voltage at node 1 (`in`) and $V_2$ is the voltage at node 2 (`out`).

---

## Internal vs external nodes

Some devices create nodes that don't appear in the netlist. A MOSFET model, for example, adds internal nodes for the parasitic drain and source resistances. These nodes are real — they get indices, rows, and columns — but the user never names them.

In spice-rs (following ngspice), external nodes come first in the numbering, internal nodes are appended after. A circuit with 5 external nodes and 3 internal nodes has 8 node voltage unknowns, occupying rows 1 through 8 of the solution vector.

The user sees only the external voltages in the output. The internal ones exist solely to make the device model accurate.

---

## The solution vector

Putting it together, the full solution vector $x$ for a circuit with $n$ nodes and $m$ voltage sources is:

$$x = \begin{bmatrix} V_1 \\ V_2 \\ \vdots \\ V_n \\ I_{V1} \\ \vdots \\ I_{Vm} \end{bmatrix}$$

The first $n$ entries are node voltages — external then internal. The remaining $m$ entries are branch currents for voltage sources and inductors. The matrix is $(n+m) \times (n+m)$, and a single solve gives every voltage and current in the circuit.
