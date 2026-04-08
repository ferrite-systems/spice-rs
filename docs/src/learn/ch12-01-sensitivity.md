# Sensitivity Analysis

Every component in a circuit has tolerances. A 10 k$\Omega$ resistor might actually be 9.8 k$\Omega$ or 10.2 k$\Omega$. A transistor's $\beta$ might be 150 instead of the nominal 200. How much do these variations affect the output? Which components matter and which don't?

Sensitivity analysis (`.SENS`) answers this quantitatively. For a specified output (typically a node voltage), it computes:

$$\text{sensitivity}_i = \frac{\partial V_{\text{out}}}{\partial p_i}$$

for every device parameter $p_i$ in the circuit. The result is a table: one row per parameter, showing how many volts (or amps) the output changes per unit change in that parameter. A resistor with high sensitivity is a critical component that needs tight tolerance. A resistor with near-zero sensitivity can be cheap and imprecise.

---

## The adjoint method

The brute-force approach to sensitivity would be: for each parameter, perturb it slightly, re-solve the entire circuit, and measure the change in output. With $N$ parameters, that's $N$ full DC solves. For a large circuit with thousands of parameters, this is prohibitively expensive.

SPICE uses a much more efficient approach: the **adjoint method**. The idea is to factor the matrix *once* during the DC operating point, then reuse that factored matrix for every parameter perturbation. Each parameter's sensitivity requires only a single forward/back substitution — not a full matrix factorization.

The algorithm, implemented in [`src/analysis/sens.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/analysis/sens.rs):

### Step 1: Solve the DC operating point

Run the full nonlinear DC solver from Chapter 3. This gives the operating point solution $\mathbf{E}$ (node voltages and branch currents) and the factored MNA matrix $\mathbf{Y}$.

### Step 2: For each parameter, perturb and measure

For each device parameter $p_i$:

**a. Choose the perturbation.** The perturbation $\delta p$ is a small fraction of the parameter value:

$$\delta p = \begin{cases} p_i \times 10^{-6} & \text{if } p_i \neq 0 \\ 10^{-6} & \text{if } p_i = 0 \end{cases}$$

This is small enough that the linearization is accurate, but large enough to avoid floating-point noise.

**b. Compute the delta stamps.** Load the device at its original parameter value, capturing the matrix stamps and RHS contributions. Negate them. Then perturb the parameter by $\delta p$, re-run the device's temperature calculations, and load again. The result is the *difference*: $\Delta\mathbf{Y} = \mathbf{Y}_{\text{new}} - \mathbf{Y}_{\text{old}}$ and $\Delta\mathbf{I} = \mathbf{I}_{\text{new}} - \mathbf{I}_{\text{old}}$.

**c. Form the RHS.** The linearized change in the solution is governed by:

$$\mathbf{Y} \cdot \Delta\mathbf{E} = \Delta\mathbf{I} - \Delta\mathbf{Y} \cdot \mathbf{E}$$

The right-hand side combines the direct change in source currents ($\Delta\mathbf{I}$) with the indirect change from the modified conductance matrix multiplied by the original solution ($\Delta\mathbf{Y} \cdot \mathbf{E}$).

**d. Solve.** Since $\mathbf{Y}$ is already factored, this is just a forward/back substitution — fast.

**e. Extract sensitivity.** The sensitivity is:

$$\text{sensitivity}_i = \frac{\Delta E_{\text{output}}}{\delta p}$$

**f. Restore.** Set the parameter back to its original value and re-run temperature calculations, leaving the circuit unchanged for the next parameter.

---

## What gets perturbed

Not every device parameter is meaningful for sensitivity. A device's `sensitivity_params()` method returns the list of parameters that can be perturbed. Typical examples:

| Device | Parameters |
|--------|-----------|
| Resistor | resistance |
| Capacitor | capacitance |
| Inductor | inductance |
| Diode | $I_s$, $n$, junction capacitance parameters |
| MOSFET | $V_{th}$, $K_P$, $\lambda$, oxide capacitance, ... |
| BJT | $\beta_F$, $\beta_R$, $I_s$, Early voltage, ... |

The sensitivity analysis iterates over *all* devices in the circuit and *all* their perturbable parameters, producing a comprehensive table.

---

## Reading the results

The output is a table of sensitivities. For a voltage output $V(\text{out})$:

```text
Parameter         Sensitivity (V/unit)
r1                -4.5000e-04      (V per ohm)
r2                 2.1000e-04      (V per ohm)
q1:bf              8.3000e-06      (V per unit beta)
q1:is             -1.2000e+08      (V per amp of Is)
```

The large magnitude for `q1:is` doesn't mean $I_s$ is the most critical parameter — it just means $I_s$ is very small (around $10^{-15}$), so the sensitivity per *absolute* unit is huge. To compare parameters fairly, normalize by each parameter's nominal value:

$$\text{relative sensitivity}_i = \frac{p_i}{V_{\text{out}}} \cdot \frac{\partial V_{\text{out}}}{\partial p_i}$$

This gives the percent change in output per percent change in parameter — a dimensionless quantity that allows direct comparison across all device types.

---

## Practical use

Sensitivity analysis is invaluable for:

- **Tolerance analysis:** Identify which components need tight tolerances (high sensitivity) and which can be relaxed (low sensitivity). This directly affects BOM cost.
- **Design centering:** Understand which parameters shift the output in which direction, guiding the designer toward a robust operating point.
- **Debugging:** When a circuit doesn't meet spec, sensitivity tells you which parameter to adjust for the most effect.

The computation is fast — one DC operating point plus $N$ back-substitutions. For a circuit with 50 parameters, the sensitivity analysis takes roughly the time of one DC solve plus 50 lightweight linear solves. The matrix factorization (the expensive part) happens only once.

<!-- TODO: interactive sensitivity table — show a simple amplifier, compute sensitivities, highlight the high-sensitivity parameters in the schematic; let the user perturb a parameter and see the output change match the predicted sensitivity -->
