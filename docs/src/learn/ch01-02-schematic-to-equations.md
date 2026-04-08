# From Schematic to Equations

A schematic is a picture. A simulator needs equations. The translation between the two is mechanical, and understanding it is the key to understanding everything SPICE does.

---

## Step 1: Identify the nodes

Every wire is a node. Every place where wires connect is a node. Label them. Pick one as ground (node 0, the reference at 0V). The remaining nodes are your unknowns.

Consider three resistors in a chain:

```
V1 in 0 DC 10
R1 in a 2k
R2 a b 3k
R3 b 0 1k
```

Four nodes: `in`, `a`, `b`, and `0` (ground). The voltage source fixes $V_{in} = 10\text{V}$, so the unknowns are $V_a$ and $V_b$ — two equations needed.

---

## Step 2: Write KCL at each node

At every non-ground node, the currents must sum to zero. Express each branch current using the component relationship.

For resistors, Ohm's law gives $I = (V_i - V_j) / R$, or equivalently $I = G \cdot (V_i - V_j)$ where $G = 1/R$ is the conductance.

**KCL at node a** — current in from R1 equals current out through R2:

$$\frac{V_{in} - V_a}{R_1} = \frac{V_a - V_b}{R_2}$$

**KCL at node b** — current in from R2 equals current out through R3:

$$\frac{V_a - V_b}{R_2} = \frac{V_b}{R_3}$$

---

## Step 3: Substitute and solve

With $V_{in} = 10\text{V}$, $R_1 = 2\text{k}\Omega$, $R_2 = 3\text{k}\Omega$, $R_3 = 1\text{k}\Omega$:

$$\frac{10 - V_a}{2000} = \frac{V_a - V_b}{3000}$$

$$\frac{V_a - V_b}{3000} = \frac{V_b}{1000}$$

From the second equation: $V_a - V_b = 3V_b$, so $V_a = 4V_b$.

Substituting into the first: $\frac{10 - 4V_b}{2000} = \frac{4V_b - V_b}{3000} = \frac{3V_b}{3000} = \frac{V_b}{1000}$.

Cross-multiplying: $1000(10 - 4V_b) = 2000 V_b$, giving $10000 = 6000 V_b$, so $V_b = 5/3 \approx 1.667\text{V}$ and $V_a = 20/3 \approx 6.667\text{V}$.

Two nodes, two equations, two unknowns — solved.

---

## What MNA automates

The process above is entirely mechanical: for each resistor, stamp a conductance pattern into a matrix. For each voltage source, add a constraint row. The system $Gx = b$ assembles itself, and a linear solver finishes the job.

You never write the KCL equations by hand. You never rearrange terms. You just stamp each component and solve. Chapter 2 shows exactly how those stamps work.
