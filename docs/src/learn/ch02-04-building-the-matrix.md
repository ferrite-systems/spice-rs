# Building the Matrix

Let's assemble a complete MNA matrix from scratch. Every stamp, every entry, laid out so you can trace each number back to the component that put it there.

---

## The circuit

A voltage divider — the same one from Chapter 1, now with full detail:

```
V1 in 0 DC 10
R1 in mid 1k
R2 mid 0 1k
.OP
```

**Nodes:** `in` = 1, `mid` = 2, ground = 0 (excluded). **Branch variables:** $I_{V1}$ at position 3. Matrix size: $3 \times 3$.

---

## Stamp R1 (1 kOhm between nodes 1 and 2)

$G_1 = 1/1000 = 0.001$ S.

| | $V_1$ | $V_2$ | $I_{V1}$ | $b$ |
|---|---|---|---|---|
| Row 1 | **+0.001** | **-0.001** | | |
| Row 2 | **-0.001** | **+0.001** | | |
| Row 3 | | | | |

---

## Stamp R2 (1 kOhm between node 2 and ground)

$G_2 = 0.001$ S. Ground has no row/column, so only the (2,2) diagonal entry survives.

| | $V_1$ | $V_2$ | $I_{V1}$ | $b$ |
|---|---|---|---|---|
| Row 1 | 0.001 | -0.001 | | |
| Row 2 | -0.001 | 0.001 **+0.001** | | |
| Row 3 | | | | |

Node 2 diagonal is now $G_1 + G_2 = 0.002$.

---

## Stamp V1 (10V source, positive at node 1, negative at ground)

Branch current $I_{V1}$ at index 3. The stamp adds 1's coupling the current into KCL at node 1, and the constraint equation in row 3. The ground side contributes nothing (no row for node 0).

| | $V_1$ | $V_2$ | $I_{V1}$ | $b$ |
|---|---|---|---|---|
| Row 1 | 0.001 | -0.001 | **+1** | 0 |
| Row 2 | -0.001 | 0.002 | 0 | 0 |
| Row 3 | **+1** | 0 | 0 | **10** |

---

## The complete system

$$\begin{bmatrix} 0.001 & -0.001 & 1 \\\\ -0.001 & 0.002 & 0 \\\\ 1 & 0 & 0 \end{bmatrix} \begin{bmatrix} V_{in} \\\\ V_{mid} \\\\ I_{V1} \end{bmatrix} = \begin{bmatrix} 0 \\\\ 0 \\\\ 10 \end{bmatrix}$$

Three stamps, three devices, one matrix. Each entry traces back to exactly one component (or in the case of the (2,2) diagonal, two components that share a node).

---

## Reading the matrix

Each row tells a story:

- **Row 1** (KCL at `in`): $0.001 V_{in} - 0.001 V_{mid} + I_{V1} = 0$. The current through R1 plus the current from V1 sum to zero.
- **Row 2** (KCL at `mid`): $-0.001 V_{in} + 0.002 V_{mid} = 0$. Current into `mid` from R1 equals current out through R2.
- **Row 3** (V1 constraint): $V_{in} = 10$. The source forces the voltage.

Solving gives $V_{in} = 10\text{V}$, $V_{mid} = 5\text{V}$, $I_{V1} = -0.01\text{A}$. The voltage divider divides, and the source supplies 10 mA.
