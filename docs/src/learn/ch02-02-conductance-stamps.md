# Conductance Stamps

The resistor stamp is the atom of circuit simulation. Every other stamp — capacitors, diodes, transistors — is a variation on this pattern. Understand it once, and the rest follows.

---

## Why the 2x2 pattern

A resistor $R$ between nodes $i$ and $j$ carries current $I = G(V_i - V_j)$ where $G = 1/R$. Write KCL at both nodes:

**Node $i$:** current leaves through the resistor, so $-G(V_i - V_j)$ is the resistor's contribution. Expanding: $-GV_i + GV_j$.

**Node $j$:** current enters through the resistor, so $+G(V_i - V_j)$. Expanding: $+GV_i - GV_j$.

These coefficients go into the matrix:

|  | $V_i$ | $V_j$ |
|---|---|---|
| **Row $i$** | $+G$ | $-G$ |
| **Row $j$** | $-G$ | $+G$ |

Positive on the diagonal, negative off-diagonal. Symmetric. This is the stamp — it's what KCL looks like in matrix form.

---

## Stamping two resistors

Take two resistors: $R_1$ between nodes 1 and 2 ($G_1 = 0.001$ S), and $R_2$ between nodes 2 and ground ($G_2 = 0.002$ S).

**$R_1$ stamps into a 2x2 matrix** (nodes 1 and 2):

|  | $V_1$ | $V_2$ |
|---|---|---|
| **Row 1** | $+0.001$ | $-0.001$ |
| **Row 2** | $-0.001$ | $+0.001$ |

**$R_2$ connects node 2 to ground.** Ground is node 0 — it has no row or column. So $R_2$ stamps only the diagonal entry at node 2:

|  | $V_1$ | $V_2$ |
|---|---|---|
| **Row 1** | $0$ | $0$ |
| **Row 2** | $0$ | $+0.002$ |

---

## Superposition: stamps add up

The final matrix is the sum of all stamps:

|  | $V_1$ | $V_2$ |
|---|---|---|
| **Row 1** | $0.001$ | $-0.001$ |
| **Row 2** | $-0.001$ | $0.003$ |

This is the key insight: each component stamps independently, and the stamps superpose by addition. The order doesn't matter. The components don't know about each other. The matrix assembles itself.

This is why spice-rs (and ngspice) can handle any circuit topology with the same code — loop over all devices, call `load()` on each one, and the matrix is complete.

---

## When one node is ground

When a resistor connects to ground (node 0), that node has no row or column. The 2x2 stamp degenerates: the row and column for ground are simply discarded. Only the diagonal entry at the other node survives.

This is not a special case in the code — ground is just absent from the matrix, so stamps that reference it naturally contribute only their non-ground entries.
