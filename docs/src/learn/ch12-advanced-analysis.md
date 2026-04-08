# Advanced Analysis

The three core analyses — DC operating point, AC frequency sweep, and transient simulation — answer the most common questions about a circuit. But sometimes you need to ask deeper questions.

*How sensitive is the output voltage to a 1% change in a resistor?* Sensitivity analysis answers this, systematically computing the derivative of any output with respect to every device parameter in the circuit. It tells you which components matter most and which are irrelevant.

*What is the voltage gain, input impedance, and output impedance of this amplifier?* Transfer function analysis extracts all three from a single linearized solve, giving you the small-signal characterization that an analog designer needs.

*Is this feedback loop stable? Where are the poles and zeros?* Pole-zero analysis finds the roots of the transfer function in the complex $s$-plane, revealing the circuit's natural frequencies, its stability margins, and the fundamental shape of its frequency response.

These three analyses share a common foundation: they all start from the DC operating point and work with the linearized circuit. They're computationally inexpensive compared to transient analysis — each involves a handful of matrix solves rather than thousands — but they provide insight that would be difficult or impossible to extract from time-domain waveforms alone.

---

## The plan

1. **[Sensitivity analysis](ch12-01-sensitivity.md)** — `.SENS`: how much does the output change when each parameter is perturbed? The adjoint method, perturbation, and the sensitivity table.

2. **[Transfer function analysis](ch12-02-transfer-function.md)** — `.TF`: voltage gain (or transimpedance), input impedance, and output impedance from two linear solves.

3. **[Pole-zero analysis](ch12-03-pole-zero.md)** — `.PZ`: finding the poles and zeros of the transfer function in the complex plane. What they mean physically and why they matter for stability.

<!-- TODO: interactive analysis selector — show a simple amplifier circuit, run all three analyses, display the results side by side: sensitivity table, TF values, pole-zero plot -->
