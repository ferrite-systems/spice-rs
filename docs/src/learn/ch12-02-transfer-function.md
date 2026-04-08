# Transfer Function Analysis

Transfer function analysis (`.TF`) extracts the three most important small-signal properties of a circuit in a single analysis:

1. **Transfer function** — the ratio of output to input: voltage gain ($V_{\text{out}}/V_{\text{in}}$), transimpedance ($V_{\text{out}}/I_{\text{in}}$), or current gain ($I_{\text{out}}/I_{\text{in}}$).
2. **Input impedance** — the impedance seen looking into the input source.
3. **Output impedance** — the impedance seen looking back from the output.

These three numbers completely characterize a linear two-port network at DC. They tell you the gain of an amplifier, how much it loads the source driving it, and how stiff its output is. An analog designer needs all three to determine whether cascaded stages will work together.

```text
SPICE syntax:

.TF V(out) Vin           * voltage gain and impedances
.TF V(out,ref) Vin       * differential output
.TF I(Vout) Iin          * current gain
```

---

## The algorithm

Transfer function analysis is remarkably efficient. It requires one DC operating point solve (which factors the matrix) and then just two back-substitutions — no additional matrix factorizations. The implementation lives in [`src/analysis/tf.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/analysis/tf.rs).

### Step 1: DC operating point

Solve the full nonlinear circuit to find the operating point. Factor the MNA matrix $\mathbf{Y}$. This is the same DC solve from Chapter 3.

### Step 2: Apply unit excitation at the input

Zero the entire RHS vector. Then inject a unit excitation at the input source:

- **Voltage source input:** Set 1V at the source's branch equation: $\text{RHS}[\text{branch}] = 1$.
- **Current source input:** Inject 1A into the source's terminal nodes: $\text{RHS}[\text{pos}] = -1$, $\text{RHS}[\text{neg}] = +1$.

### Step 3: Solve

Since the matrix is already factored, this is just forward/back substitution. The result $\Delta\mathbf{E}$ gives the circuit's response to a unit input perturbation.

### Step 4: Read the transfer function

The transfer function is simply the output variable from the solution:

$$\text{TF} = \begin{cases} \Delta E[\text{out\_pos}] - \Delta E[\text{out\_neg}] & \text{(voltage output)} \\ \Delta E[\text{out\_branch}] & \text{(current output)} \end{cases}$$

Since the input was 1V (or 1A), the ratio is just the output value itself.

### Step 5: Read the input impedance

The input impedance comes from the same solution:

- **Current source input:** $Z_{\text{in}} = V_{\text{neg}} - V_{\text{pos}}$ (voltage across the source divided by the 1A current).
- **Voltage source input:** $Z_{\text{in}} = -1/I_{\text{branch}}$ (1V divided by the current the source must supply).

### Step 6: Compute output impedance

Zero the RHS again and inject a unit excitation at the *output*:

- **Voltage output:** Inject 1A at the output nodes.
- **Current output:** Inject 1V at the output source's branch.

Solve (back-substitution only), then read the impedance:

- **Voltage output:** $Z_{\text{out}} = V_{\text{neg}} - V_{\text{pos}}$.
- **Current output:** $Z_{\text{out}} = 1/I_{\text{branch}}$.

---

## What the numbers mean

Consider a common-emitter amplifier:

```text
.TF V(out) Vin

Transfer function:     -45.2    (voltage gain)
Input impedance:       2.8 kΩ
Output impedance:      4.7 kΩ
```

**Transfer function = -45.2** means the amplifier inverts the signal and provides 45.2x voltage gain. A 1 mV input change produces a 45.2 mV output change (in the opposite direction).

**Input impedance = 2.8 k$\Omega$** means the amplifier draws current from the source. If the source has significant output impedance (say 1 k$\Omega$), the voltage at the amplifier's input will be reduced by the voltage divider effect: $V_{\text{in,actual}} = V_{\text{source}} \cdot 2800/(2800+1000)$. A higher input impedance is better for not loading the source.

**Output impedance = 4.7 k$\Omega$** means the output voltage drops when the load draws current. If the load is 10 k$\Omega$, the effective gain is reduced by $10000/(10000+4700) \approx 0.68$. A lower output impedance is better for driving loads.

---

## Relationship to AC analysis

Transfer function analysis gives you the *DC* (zero-frequency) values of gain and impedance. AC analysis gives you these same quantities as a function of frequency. The `.TF` result corresponds to the $f = 0$ point on the Bode plot from `.AC`.

Why have `.TF` as a separate analysis? Because it's cheaper and more direct. AC analysis requires sweeping across hundreds of frequency points. `.TF` gives you the DC gain and both impedances from two back-substitutions — effectively three numbers for the cost of one frequency point. If all you need is the midband gain and the port impedances, `.TF` is the right tool.

---

## Under the hood

The elegance of `.TF` comes from the **superposition principle** applied to the linearized circuit. At the operating point, all nonlinear devices have been replaced by their linearized models (conductances and controlled sources). The linearized circuit obeys superposition, so:

- Setting the input to 1V and reading the output gives the gain directly.
- The current drawn by the 1V source gives the input admittance.
- Injecting 1A at the output and reading the voltage gives the output impedance.

No frequency-dependent elements are involved (we're at DC), so the matrix is real-valued and the solve is fast. The entire analysis touches the matrix solver exactly twice after the initial DC OP factorization.

<!-- TODO: interactive transfer function — show an amplifier circuit, run .TF, display the three results; let the user change component values and see how the gain and impedances shift -->
