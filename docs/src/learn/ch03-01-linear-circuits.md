# Linear Circuits

Consider a resistive voltage divider — three resistors and a voltage source:

```ferrite-circuit
circuit "Resistor Chain" {
    node "a" label="VIN" rail=#true voltage="12"
    node "gnd" ground=#true
    group "chain" topology="generic" {
        component "R1" type="resistor" role="passive" {
            value "2k"
            port "1" net="a"
            port "2" net="b"
        }
        component "R2" type="resistor" role="passive" {
            value "1k"
            port "1" net="b"
            port "2" net="c"
        }
        component "R3" type="resistor" role="passive" {
            value "3k"
            port "1" net="c"
            port "2" net="gnd"
        }
    }
    node "b" label="Vb"
    node "c" label="Vc"
}
```

Four nodes: `a`, `b`, `c`, `gnd`. The voltage at `a` is fixed at 12V by V1. The unknowns are $V_b$, $V_c$, and the branch current $I_{V1}$.

From Chapter 2, we know how to build the MNA system. Each resistor stamps a conductance pattern; the voltage source adds a constraint equation. Assembling everything:

$$\begin{bmatrix} G_1 & -G_1 & 0 & 1 \\\\ -G_1 & G_1 + G_2 & -G_2 & 0 \\\\ 0 & -G_2 & G_2 + G_3 & 0 \\\\ 1 & 0 & 0 & 0 \end{bmatrix} \begin{bmatrix} V_a \\\\ V_b \\\\ V_c \\\\ I_{V1} \end{bmatrix} = \begin{bmatrix} 0 \\\\ 0 \\\\ 0 \\\\ 12 \end{bmatrix}$$

where $G_1 = 1/2000$, $G_2 = 1/1000$, $G_3 = 1/3000$.

This is a standard linear system: $Gx = b$. The matrix $G$ contains only constants — resistor conductances and the voltage source stamps. The right-hand side $b$ is also constant. One call to the sparse solver and we have the answer:

$$V_a = 12\text{V}, \quad V_b = 6\text{V}, \quad V_c = 3.6\text{V}, \quad I_{V1} = -3\text{mA}$$

The key property is **linearity**: the conductance of every component is fixed, independent of the voltages across it. A 2k resistor is always a 2k resistor, whether it has 1V across it or 100V. This means the matrix $G$ can be assembled once and solved once. No iteration, no guessing.

---

## What makes it easy

Every entry in the matrix is known before we start solving:

- Resistor stamps depend only on the resistance value (a design parameter)
- Voltage source stamps are always $\pm 1$ (they enforce a voltage constraint)
- Current source stamps go only into $b$, the right-hand side

Nothing depends on the solution. The matrix is determined entirely by the circuit topology and component values.

In spice-rs, this means: stamp once, factor once, solve once. The `ni_iter` loop (the Newton-Raphson loop in `solver.rs`) still runs, but it converges in a single iteration — the first solution is exact, so the convergence check immediately passes.

```
Iteration 1: Stamp all devices → Solve → Check convergence → Done.
```

For circuits with only linear components, the DC operating point is trivial. The real challenge begins when we add a component whose stamps depend on the answer.
