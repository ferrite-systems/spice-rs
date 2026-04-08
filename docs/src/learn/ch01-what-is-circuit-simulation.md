# What Is Circuit Simulation?

A circuit simulator solves one question: *given a circuit, what are the voltages and currents?*

You draw a schematic — resistors, capacitors, transistors, voltage sources connected by wires. The simulator turns that drawing into a system of equations, solves them, and hands you back numbers: the voltage at every node, the current through every branch.

This is what SPICE does. It has been doing it since 1973, when Larry Nagel wrote the original at UC Berkeley. Every chip designed since then — every phone, laptop, car, satellite — was simulated in SPICE or a descendant of it before it was built.

spice-rs is a faithful port of [ngspice](https://ngspice.sourceforge.io/), the open-source SPICE, rewritten in Rust. This guide will teach you how it works, from the inside out.

---

## The three laws

Everything SPICE computes follows from two conservation laws and one constitutive relationship:

**Kirchhoff's Current Law (KCL):** The sum of currents entering any node is zero. Current is conserved — what flows in must flow out.

**Kirchhoff's Voltage Law (KVL):** The sum of voltage drops around any closed loop is zero. Energy is conserved — a charge that travels in a circle returns to the same potential.

**Ohm's Law** (and its nonlinear generalizations): Each component defines a relationship between the voltage across it and the current through it. For a resistor, $I = V/R$. For a diode, $I = I_s(e^{V/V_t} - 1)$. For a MOSFET, it's considerably more complicated — but the principle is the same.

KCL gives us one equation per node. The component equations tell us how to fill in the coefficients. Solve the system, and you have your answer.

---

## From schematic to numbers

Consider the simplest possible circuit: a voltage source and a resistor.

```ferrite-circuit
circuit "Simple Resistor" {
    node "in" label="VIN" rail=#true voltage="5"
    node "gnd" ground=#true
    group "load" topology="generic" {
        component "R1" type="resistor" role="passive" {
            value "1k"
            port "1" net="in"
            port "2" net="gnd"
        }
    }
}
```

The circuit has two nodes: `in` and `gnd` (ground, our reference at 0V). The voltage source forces $V_{in} = 5\text{V}$. KCL at node `in` says the current leaving through R1 must equal the current supplied by V1:

$$I_{R1} = \frac{V_{in}}{R_1} = \frac{5}{1000} = 5\text{mA}$$

That's the simulation result. One node voltage (5V), one branch current (5mA). For this circuit you don't need a computer — but the method scales to millions of nodes, and SPICE uses the same approach for all of them.

---

## A circuit worth simulating

Now add a second resistor to make a voltage divider:

```ferrite-circuit
circuit "Voltage Divider" {
    node "vin" label="VDD" rail=#true voltage="10"
    node "gnd" ground=#true
    group "divider" topology="voltage-divider" {
        component "R1" type="resistor" role="divider-upper" {
            value "1k"
            port "1" net="vin"
            port "2" net="vmid"
        }
        component "R2" type="resistor" role="divider-lower" {
            value "1k"
            port "1" net="vmid"
            port "2" net="gnd"
        }
    }
    node "vmid" label="Vmid"
}
```

Three nodes: `in`, `mid`, `gnd`. We know $V_{in} = 10\text{V}$ (forced by V1) and $V_{gnd} = 0$ (reference). The unknown is $V_{mid}$.

KCL at node `mid`: current in from R1 equals current out through R2.

$$\frac{V_{in} - V_{mid}}{R_1} = \frac{V_{mid} - V_{gnd}}{R_2}$$

$$\frac{10 - V_{mid}}{1000} = \frac{V_{mid}}{1000}$$

$$V_{mid} = 5\text{V}$$

Still simple enough to do by hand. But the approach — write KCL at every node, substitute the component equations, solve the resulting linear system — is exactly what SPICE does. The difference is that SPICE assembles these equations into a matrix and uses a sparse direct solver to handle thousands of nodes simultaneously.

The next chapter shows how that matrix is built.

Press **Run** on either circuit above to simulate it in your browser — the results will appear on the schematic. Try changing `R2` to `500` in the voltage divider and re-running.
