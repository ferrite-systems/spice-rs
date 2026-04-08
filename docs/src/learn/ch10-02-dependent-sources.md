# Dependent Sources

A dependent source produces a voltage or current that is controlled by some other voltage or current in the circuit. Where independent sources model external stimuli (batteries, signal generators), dependent sources model *internal relationships* — an amplifier's gain, a transistor's transconductance, or any linear coupling between two parts of a circuit.

SPICE has four types of dependent sources, covering every combination of voltage and current for both input and output:

| Element | Name | Relationship | Controlling variable | Output |
|---------|------|-------------|---------------------|--------|
| **E** | VCVS | Voltage-controlled voltage source | Voltage | Voltage |
| **G** | VCCS | Voltage-controlled current source | Voltage | Current |
| **H** | CCVS | Current-controlled voltage source | Current | Voltage |
| **F** | CCCS | Current-controlled current source | Current | Current |

Each has a single parameter: the **gain** (dimensionless for E and F, transconductance in siemens for G, transresistance in ohms for H). The output is always the gain times the controlling variable. These are *linear* elements — no matter how large the controlling signal, the relationship is a straight line through the origin. This makes them easy to stamp into the MNA matrix and means they don't require Newton-Raphson iteration.

Dependent sources are fundamental building blocks for modeling active devices. The small-signal model of a MOSFET, for instance, includes a VCCS ($g_m \cdot V_{gs}$) as its core gain element. Op-amp macromodels use VCVS elements with gains of $10^5$ or more. Feedback networks, current mirrors, and gyrators all use dependent sources.

---

## VCVS — Voltage-Controlled Voltage Source (E element)

```text
E1 out 0 in+ in- 10
```

The output voltage equals the gain times the controlling voltage difference:

$$V_{\text{out+}} - V_{\text{out-}} = \mu \cdot (V_{\text{ctrl+}} - V_{\text{ctrl-}})$$

where $\mu$ is the voltage gain (dimensionless).

Because the output is a *voltage*, the VCVS needs a **branch equation** — just like an independent voltage source. The branch current $I_b$ flows through the source, and the MNA system enforces the voltage constraint. The stamps are:

```text
         pos  neg  ctrl+  ctrl-  branch
pos   [                            +1  ]
neg   [                            -1  ]
branch[ +1   -1    -μ     +μ          ]
```

The first two rows say "branch current flows in at pos, out at neg." The branch row says "$V_{\text{pos}} - V_{\text{neg}} - \mu(V_{\text{ctrl+}} - V_{\text{ctrl-}}) = 0$" — the voltage constraint. This is identical to an independent voltage source, except the RHS is replaced by the gain terms in the matrix.

In spice-rs, the implementation lives in [`src/device/vcvs.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/device/vcvs.rs). The `load()` function stamps six values:

```rust
mna.stamp(p, b, 1.0);          // pos gets branch current
mna.stamp(n, b, -1.0);         // neg loses branch current
mna.stamp(b, p, 1.0);          // branch eq: +V_pos
mna.stamp(b, n, -1.0);         // branch eq: -V_neg
mna.stamp(b, cp, -self.gain);  // branch eq: -μ * V_ctrl+
mna.stamp(b, cn, self.gain);   // branch eq: +μ * V_ctrl-
```

---

## VCCS — Voltage-Controlled Current Source (G element)

```text
G1 out+ out- ctrl+ ctrl- 0.01
```

The output current equals the transconductance times the controlling voltage difference:

$$I_{\text{out}} = g_m \cdot (V_{\text{ctrl+}} - V_{\text{ctrl-}})$$

where $g_m$ has units of siemens (A/V).

Because the output is a *current*, no branch equation is needed. The VCCS stamps directly into the conductance matrix — it's a pure four-terminal conductance stamp:

```text
         ctrl+  ctrl-
out+  [  +gm    -gm  ]
out-  [  -gm    +gm  ]
```

This is the simplest of the four dependent sources. Current enters at out+ and leaves at out-, with magnitude proportional to the controlling voltage. The stamp pattern is identical to a resistor's conductance stamp, except the rows and columns correspond to *different* node pairs.

The VCCS is the most physically intuitive dependent source — it's exactly how a transconductance amplifier works. The small-signal model of a MOSFET's drain current is $g_m V_{gs}$, which is a VCCS. In spice-rs, see [`src/device/vccs.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/device/vccs.rs):

```rust
mna.stamp(p, cp, self.gm);     // I into out+ from ctrl+
mna.stamp(p, cn, -self.gm);    // I into out+ from ctrl-
mna.stamp(n, cp, -self.gm);    // I out of out- from ctrl+
mna.stamp(n, cn, self.gm);     // I out of out- from ctrl-
```

---

## CCVS — Current-Controlled Voltage Source (H element)

```text
H1 out+ out- Vsense 1000
```

The output voltage equals the transresistance times the controlling current:

$$V_{\text{out+}} - V_{\text{out-}} = r_m \cdot I_{\text{ctrl}}$$

where $r_m$ has units of ohms (V/A).

There's a subtlety with current-controlled sources: SPICE cannot directly observe current through an arbitrary branch. It can only access the branch current of a **voltage source**. So the controlling current must be the current through a named voltage source — often a zero-volt "sense" source inserted just for this purpose.

The CCVS needs its own branch equation (it's a voltage source) and references the controlling source's branch equation. The stamps:

```text
         pos  neg  ctrl_branch  branch
pos   [                          +1   ]
neg   [                          -1   ]
branch[ +1   -1    -rm                ]
```

The branch row says "$V_{\text{pos}} - V_{\text{neg}} - r_m \cdot I_{\text{ctrl}} = 0$." The transresistance $r_m$ couples the branch equation of the controlling voltage source into the output voltage constraint. In spice-rs, see [`src/device/ccvs.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/device/ccvs.rs):

```rust
mna.stamp(p, b, 1.0);
mna.stamp(n, b, -1.0);
mna.stamp(b, p, 1.0);
mna.stamp(b, n, -1.0);
mna.stamp(b, cb, -self.transresistance);
```

---

## CCCS — Current-Controlled Current Source (F element)

```text
F1 out+ out- Vsense 5
```

The output current equals the gain times the controlling current:

$$I_{\text{out}} = \beta \cdot I_{\text{ctrl}}$$

where $\beta$ is the current gain (dimensionless).

Like the CCVS, the controlling current must flow through a named voltage source. But since the output is a *current*, no branch equation is needed for the F element itself. The stamps are the simplest of the current-controlled sources:

```text
         ctrl_branch
out+  [    +β       ]
out-  [    -β       ]
```

The gain $\beta$ links the controlling branch current to the output current. This is the natural model for a current mirror or the $\beta$ of a bipolar transistor (in a simplified view). In spice-rs, see [`src/device/cccs.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/device/cccs.rs):

```rust
mna.stamp(p, cb, self.gain);
mna.stamp(n, cb, -self.gain);
```

---

## The pattern

Looking across all four dependent sources, a clear structure emerges:

|  | **Voltage output** (needs branch eq) | **Current output** (no branch eq) |
|--|--------------------------------------|-----------------------------------|
| **Voltage controlled** | VCVS (E): 6 stamps | VCCS (G): 4 stamps |
| **Current controlled** | CCVS (H): 5 stamps | CCCS (F): 2 stamps |

Voltage outputs always require a branch equation because the MNA framework enforces voltage constraints through auxiliary equations. Current outputs stamp directly into the conductance matrix at the output node rows. Voltage-controlled sources reference the controlling nodes directly. Current-controlled sources reference the branch equation of a sensing voltage source.

The VCCS (G element) is the most commonly used dependent source in practice, because transconductance is the natural gain mechanism of field-effect transistors. When you look at the small-signal model of a MOSFET in Chapter 5, the $g_m V_{gs}$ current source is a VCCS — and its four-entry conductance stamp is exactly what appears in the linearized MNA matrix during AC analysis.

<!-- TODO: interactive MNA explorer — pick a dependent source type, set the gain, see the matrix stamps highlighted in the MNA system; connect to a simple resistive network and see how the source affects the solution -->
