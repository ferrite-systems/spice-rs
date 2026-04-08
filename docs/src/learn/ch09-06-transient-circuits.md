# Transient Circuits

With all the machinery in place — integration methods, timestep control, breakpoints — let's see what transient analysis actually produces for two fundamental circuits.

## RC Step Response

The simplest transient circuit: a resistor, a capacitor, and a voltage step. The input jumps from 0 to $V_{\text{final}}$ at $t = 0$, and the output voltage across the capacitor rises exponentially toward the final value.

```ferrite-circuit
circuit "RC Step Response" {
    node "vin" label="Vin" rail=#true voltage="5"
    node "gnd" ground=#true
    group "rc" topology="rc-low-pass" {
        component "R1" type="resistor" role="filter-element" {
            value "1k"
            port "1" net="vin"
            port "2" net="vout"
        }
        component "C1" type="capacitor" role="shunt" {
            value "1u"
            port "1" net="vout"
            port "2" net="gnd"
        }
    }
    node "vout" label="Vout"
}
```

### The analytical solution

The output voltage follows the classic first-order exponential:

$$V_{\text{out}}(t) = V_{\text{final}} \cdot \left(1 - e^{-t/\tau}\right)$$

where $\tau = RC$ is the **time constant**. For $R = 1\text{ k}\Omega$ and $C = 1\text{ uF}$:

$$\tau = 1000 \cdot 1 \times 10^{-6} = 1\text{ ms}$$

The time constant has a clean physical meaning: it's the time it takes the capacitor to charge to $1 - 1/e \approx 63.2\%$ of the final value. After $5\tau$ (5 ms), the capacitor is within 0.7% of the final value — effectively fully charged.

```text
    RC step response

    V(t)
    5V ─────────────────────────────── V_final
     │                 ╱───────────
     │              ╱╱
     │           ╱╱
    3.16V ─ ─ ╱─ ─ ─ ─ ─ ─ ─ ─ ─ ─  63.2% at t = tau
     │      ╱╱
     │    ╱╱
     │  ╱╱
    0V ●
     └──┬──────┬──────┬──────┬──────── t
        0     1ms    2ms    3ms    4ms
              tau   2*tau  3*tau  4*tau
              63%    86%    95%    98%
```

### What the simulator does

At $t = 0$, the DC operating point gives $V_{\text{out}} = 0$ (capacitor is uncharged). Then the transient engine begins stepping:

1. **First steps:** the voltage is changing rapidly. The LTE mechanism keeps the timestep small — perhaps tens of microseconds — to track the steep initial rise accurately.

2. **Middle of the curve:** the rate of change is slowing (the second derivative is decreasing). The LTE allows the timestep to grow — hundreds of microseconds.

3. **Approaching steady state:** the voltage is barely changing. The timestep grows to the maximum allowed — perhaps the full output step size. The simulator races through this flat region.

For this circuit, there are no breakpoints (the DC source is constant) and no nonlinear devices (no NR iteration needed). The transient simulation is pure integration. Every step converges in a single NR iteration (the system is linear), and the only question is how many steps the LTE mechanism requires.

### The companion model in action

At each timestep, the capacitor's companion model creates a conductance $G_{\text{eq}} = 2C/h$ (for the trapezoidal rule) in parallel with a current source. The resistor stamps its usual $G = 1/R$. The resulting 1x1 MNA system (one unknown node: $V_{\text{out}}$) is:

$$\left(\frac{1}{R} + \frac{2C}{h}\right) V_{\text{out}}^{(n+1)} = \frac{V_{\text{in}}}{R} + I_{\text{eq}}$$

where $I_{\text{eq}}$ encodes the capacitor's state from the previous timestep. This is a single equation solved in one NR step — trivial for the computer but conceptually the same process that runs on a 10,000-node circuit.

### Accuracy check

The exponential solution is exact — there's no approximation in the analytical formula. The trapezoidal rule, being second-order, will track the exponential with error proportional to $h^3$. For this smooth waveform (no discontinuities, all derivatives bounded), the LTE mechanism maintains accuracy automatically. The simulation result should match the analytical formula to within the integration tolerance.

<!-- TODO: interactive RC step response — show the exponential curve, overlay simulation timesteps as dots, slider for R and C, show tau moving -->

## MOSFET Switching

A more realistic transient scenario: a MOSFET driven by a pulse, switching a resistive load. This exercises every part of the transient engine — nonlinear devices, Newton-Raphson iteration, breakpoints, and adaptive timestep control.

### The circuit

A common-source NMOS switch: the gate is driven by a PULSE source (0 to 5V), the drain is connected through a load resistor to VDD, and the source is grounded. The output is the drain voltage, which swings from VDD (MOSFET off) to near ground (MOSFET on).

### What happens during the switching transient

**1. Turn-on ($V_{GS}$ rising from 0 to 5V):**

The PULSE source has a finite rise time — say 10 ns. The simulator registers a breakpoint at the rise start and another at the rise end.

As $V_{GS}$ increases past the threshold voltage $V_{th}$ (typically 0.5-1V for modern devices), the MOSFET enters its active region. The drain current begins to flow, and $V_{DS}$ drops. This is where the physics is most interesting:

- The gate capacitances ($C_{gs}$, $C_{gd}$) must be charged by the drive source. The gate voltage doesn't rise instantaneously — it's an RC charging curve where the "R" is the drive source impedance and the "C" is the gate capacitance.

- The **Miller effect** slows the transition further. $C_{gd}$ is connected between the gate (rising) and the drain (falling). The voltage across $C_{gd}$ changes by more than the gate voltage change alone, so the effective capacitance seen by the driver is amplified. During the transition, $V_{GS}$ may plateau at a "Miller plateau" voltage while the drain voltage swings.

- The MOSFET transitions through three operating regions in rapid succession: cutoff ($V_{GS} < V_{th}$), saturation ($V_{DS} > V_{GS} - V_{th}$), and linear/triode ($V_{DS} < V_{GS} - V_{th}$). Each region has different I-V equations, and the transitions create kinks in the waveform that the NR solver must handle.

```text
    MOSFET switching waveforms

    Vgs(t)                          Vds(t)
    5V ──────────────────           VDD ─────────╲
     │       ╱────────               │            ╲
     │      ╱                        │             ╲  Miller
     │     ╱  Miller plateau         │              ╲  effect
     │    ╱───╱                      │               ╲ slows
     │   ╱                           │                ╲transition
    0V ──╱                          ~0V                ╲───────
     └────────────── t               └────────────── t
         rise                            fall
```

**2. The steady state (MOSFET on):**

Once $V_{GS} = 5\text{V}$ and $V_{DS}$ has settled, the MOSFET is in the linear region, acting like a small resistance ($R_{DS(on)}$). The drain voltage is $I_D \cdot R_{DS(on)}$, typically a few millivolts to tens of millivolts.

The LTE mechanism sees that the voltages have stopped changing and ramps the timestep up to the maximum. Newton-Raphson converges in 2-3 iterations (the operating point barely changed from the previous step).

**3. Turn-off ($V_{GS}$ falling from 5V to 0):**

The reverse process. The gate capacitances discharge, the Miller effect creates another plateau, and the drain voltage rises back to VDD. Another set of breakpoints guides the simulator through the transition.

### What the simulator sees

For the turn-on edge, a typical simulation might look like:

```text
    Step    Time        h          NR iters   Order   Action
    1-10    0-90ns      ~10ns      2-3        1       Approaching rise
    11      100ns       clip       -          -       Hit breakpoint (rise start)
    12-15   100.01ns+   ~0.1ns     3-5        1       Through threshold
    16-25   100.5ns+    ~1ns       3-4        1→2     Miller plateau
    26-30   108ns+      ~2ns       2-3        2       Entering linear region
    31-40   110ns+      ~50ns      2          2       Settling
    41+     200ns+      ~1us       2          2       Steady state
```

The timestep varies by four orders of magnitude within a single switching event. The NR iteration count peaks during the transition, where the device is changing regions and the linearization at the previous step is least accurate. The integration order drops to 1 at the breakpoint and promotes back to 2 once the waveform is smooth.

### The companion model for gate capacitance

During switching, the MOSFET's gate capacitances are the dominant energy-storage elements. The simulator tracks the charge on each capacitance ($Q_{gs}$, $Q_{gd}$, $Q_{gb}$) as state variables. At each timestep:

1. The device model computes the charges from the terminal voltages (these are nonlinear functions of voltage — the capacitance depends on the operating region)
2. `ni_integrate()` converts the charge to current using the integration coefficients
3. The companion model ($G_{\text{eq}}$, $I_{\text{eq}}$) is stamped into the MNA matrix
4. The NR solver finds the consistent set of voltages and currents

This is the full transient machinery at work: nonlinear charge computation, numerical integration, companion models, Newton-Raphson, and adaptive timestep control, all cooperating to track a fast switching event with high fidelity.

## From these examples to real circuits

The RC circuit and MOSFET switch are building blocks. A real digital circuit might have thousands of MOSFETs switching in sequence, each with its own gate capacitance, Miller effect, and switching trajectory. A power converter might have MOSFETs switching at megahertz rates, driving inductors and capacitors with widely separated time constants.

The transient engine handles all of these with the same machinery: companion models for energy storage, Newton-Raphson for nonlinearity, LTE for timestep control, and breakpoints for synchronization with the stimulus. The circuit gets more complex, but the algorithm at each timestep is the same loop we've seen in this chapter.

<!-- TODO: interactive MOSFET switching — show gate and drain waveforms, highlight the Miller plateau, show timestep sizes as tick marks on the time axis -->
