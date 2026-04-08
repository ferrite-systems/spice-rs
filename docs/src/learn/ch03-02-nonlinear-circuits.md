# Nonlinear Circuits

Now add a diode to the circuit:

```ferrite-circuit
circuit "Diode Resistor" {
    node "a" label="VIN" rail=#true voltage="5"
    node "gnd" ground=#true
    group "bias" topology="generic" {
        component "R1" type="resistor" role="passive" {
            value "1k"
            port "1" net="a"
            port "2" net="b"
        }
        component "D1" type="diode" role="passive" {
            port "anode" net="b"
            port "cathode" net="gnd"
        }
    }
    node "b" label="Vb"
}
```

The diode D1 connects node `b` to ground. Its current is given by the Shockley equation (Chapter 4 covers this in detail):

$$I_D = I_s\left(e^{V_b / nV_t} - 1\right)$$

where $I_s \approx 10^{-14}$ A is the saturation current, $n \approx 1$ is the emission coefficient, and $V_t \approx 26$ mV is the thermal voltage.

The MNA equation at node `b` is KCL — current in through R1 equals current out through D1:

$$\frac{V_a - V_b}{R_1} = I_s\left(e^{V_b / nV_t} - 1\right)$$

This is no longer a linear equation. The right-hand side contains $e^{V_b/nV_t}$ — an exponential function of the unknown $V_b$. There is no way to write this as a matrix entry times $V_b$.

---

## The circular dependency

For the resistive divider, we could fill in every matrix entry before solving. Not anymore. The diode's contribution to the matrix — its equivalent conductance — is:

$$g_d = \frac{dI_D}{dV_b} = \frac{I_s}{nV_t} \cdot e^{V_b / nV_t}$$

This is the slope of the I-V curve at the operating point. It tells us how much the current changes per volt of change in $V_b$. But to compute $g_d$, we need to know $V_b$ — which is the thing we're solving for.

We're stuck in a loop:
- To build the matrix, we need $g_d$
- To find $g_d$, we need $V_b$
- To find $V_b$, we need to solve the matrix

This is the fundamental challenge of nonlinear circuit simulation. Every nonlinear device — diodes, MOSFETs, BJTs — creates the same circular dependency. The matrix coefficients depend on the solution, so you can't assemble the matrix without already knowing the answer.

---

## You can't solve this in one step

For the linear circuit, the matrix was constant and we got the exact answer in one solve. For this circuit, any matrix we assemble is only *approximately* correct — it uses an assumed value of $V_b$ to compute $g_d$, but the solved $V_b$ won't exactly match that assumption.

Try plugging in a guess of $V_b = 0$:
- $g_d = I_s / (nV_t) \approx 3.8 \times 10^{-13}$ S — essentially zero
- The diode looks like an open circuit
- Solving gives $V_b \approx 5$ V

But at $V_b = 5$ V, the diode should be conducting heavily — it's certainly not an open circuit. Our guess was wrong, so the matrix we built was wrong, so the answer is wrong.

Try $V_b = 5$ V:
- $g_d = I_s / (nV_t) \cdot e^{5/0.026} \approx$ a very large number
- The diode looks like a short circuit
- Solving gives $V_b \approx 0$ V

Now we've swung too far the other way. The true answer is somewhere in between — around $V_b \approx 0.65$ V, where the diode is conducting but not yet a short circuit.

The solution: don't try to get it right in one step. **Iterate.**
