# Kirchhoff's Laws

Two conservation laws underpin every circuit simulation ever run. They were formulated by Gustav Kirchhoff in 1845, and they are all you need to derive every equation SPICE solves.

---

## Kirchhoff's Current Law (KCL)

*The sum of currents entering any node is zero.*

$$\sum_{k} I_k = 0$$

Convention matters: current flowing *into* a node is positive, current flowing *out* is negative. A node cannot accumulate charge — every electron that arrives must leave through some other branch.

Consider a node where three wires meet. If 3 mA flows in through one branch and 1 mA flows in through another, then exactly 4 mA must flow out through the third:

$$3\text{mA} + 1\text{mA} - 4\text{mA} = 0$$

KCL gives SPICE one equation per node. For a circuit with $n$ nodes (excluding ground), that's $n$ equations — exactly the number needed to determine $n$ unknown node voltages.

---

## Kirchhoff's Voltage Law (KVL)

*The sum of voltage drops around any closed loop is zero.*

$$\sum_{k} V_k = 0$$

Walk around a loop, adding up each voltage rise and drop. When you return to where you started, the total is zero — a charge that travels in a circle gains no net energy.

In a loop with a 10V source, a 6V drop across R1, and a 4V drop across R2:

$$10\text{V} - 6\text{V} - 4\text{V} = 0$$

KVL doesn't appear explicitly in the MNA matrix — it's enforced automatically. When you solve for node voltages, the voltage across any component is just $V_i - V_j$. Any loop sum reduces to a telescoping series that cancels to zero. KVL is baked in.

---

## A concrete example

Three nodes, two resistors, one voltage source:

```
V1 a 0 DC 12
R1 a b 2k
R2 b 0 4k
```

**KCL at node a** (current in from V1 = current out through R1):

$$I_{V1} = \frac{V_a - V_b}{R_1}$$

**KCL at node b** (current in from R1 = current out through R2):

$$\frac{V_a - V_b}{R_1} = \frac{V_b - 0}{R_2}$$

**Voltage constraint** (from V1):

$$V_a = 12\text{V}$$

Substituting: $\frac{12 - V_b}{2000} = \frac{V_b}{4000}$, giving $V_b = 8\text{V}$.

Two laws, one component equation (Ohm's law), and we have the full solution. That's the whole game — everything SPICE computes is an elaboration of this pattern.
