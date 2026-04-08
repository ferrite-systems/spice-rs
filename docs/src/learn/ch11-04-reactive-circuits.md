# Reactive Circuits

With capacitors, inductors, and mutual inductors in hand, we can now look at circuits where reactive elements produce the most characteristic behavior in analog electronics: oscillation, ringing, and energy transfer.

---

## RLC ringing

Connect a resistor, inductor, and capacitor in series, apply a voltage step, and watch what happens. If the resistance is small enough, the circuit doesn't just charge up monotonically — it *oscillates*. The energy sloshes back and forth between the capacitor's electric field and the inductor's magnetic field, with the resistor slowly dissipating energy on each cycle until the oscillation dies out.

```ferrite-circuit
circuit "Series RLC" {
    node "in" label="Vin" rail=#true voltage="5"
    node "gnd" ground=#true
    group "rlc" topology="rlc-filter" {
        component "R1" type="resistor" role="passive" {
            value "10"
            port "1" net="in"
            port "2" net="mid"
        }
        component "L1" type="inductor" role="filter-element" {
            value "1m"
            port "1" net="mid"
            port "2" net="out"
        }
        component "C1" type="capacitor" role="filter-element" {
            value "100n"
            port "1" net="out"
            port "2" net="gnd"
        }
    }
    node "out" label="Vout"
}
```

The natural behavior is governed by two parameters:

**Natural frequency** $\omega_0$ — the frequency at which the circuit would oscillate with zero resistance:

$$\omega_0 = \frac{1}{\sqrt{LC}}$$

$$f_0 = \frac{1}{2\pi\sqrt{LC}}$$

This comes from the energy exchange between L and C. When all the energy is in the capacitor (maximum voltage, zero current), it begins to discharge through the inductor. When all the energy has transferred to the inductor (zero voltage, maximum current), the magnetic field collapses and charges the capacitor in the opposite polarity. The cycle repeats at frequency $f_0$.

**Damping ratio** $\zeta$ — the ratio of actual resistance to the critical resistance:

$$\zeta = \frac{R}{2}\sqrt{\frac{C}{L}}$$

The damping ratio determines the character of the transient response:

- **$\zeta < 1$ (underdamped):** The circuit oscillates with an exponentially decaying envelope. Each successive peak is smaller than the last. This is *ringing* — the most visually distinctive reactive behavior.

- **$\zeta = 1$ (critically damped):** The circuit returns to equilibrium as fast as possible without oscillating. This is the boundary between oscillation and monotonic decay.

- **$\zeta > 1$ (overdamped):** The circuit returns to equilibrium monotonically, like a sluggish RC circuit. No oscillation occurs.

```text
  Vout                      Underdamped (ζ = 0.1)
  2V ┤  ╱╲
     │ ╱  ╲         ╱╲
  1V ┤╱    ╲       ╱  ╲       ╱╲
     │      ╲     ╱    ╲     ╱  ╲────
     │       ╲   ╱      ╲   ╱
   0 ┤        ╲ ╱        ╲ ╱
     │         ╲╱          ╲╱
     └──────────────────────────────→ t

  Vout                      Critically damped (ζ = 1.0)
     │
  1V ┤   ╱──────────────────────────
     │  ╱
     │ ╱
   0 ┤╱
     └──────────────────────────────→ t

  Vout                      Overdamped (ζ = 3.0)
     │
  1V ┤        ╱─────────────────────
     │      ╱╱
     │    ╱╱
   0 ┤──╱╱
     └──────────────────────────────→ t
```

The actual oscillation frequency in the underdamped case is slightly lower than $\omega_0$ because of the damping:

$$\omega_d = \omega_0\sqrt{1 - \zeta^2}$$

### A concrete example

Consider $R = 10\,\Omega$, $L = 1\,\text{mH}$, $C = 100\,\text{nF}$:

$$f_0 = \frac{1}{2\pi\sqrt{10^{-3} \cdot 10^{-7}}} = \frac{1}{2\pi \cdot 10^{-5}} \approx 15.9\,\text{kHz}$$

$$\zeta = \frac{10}{2}\sqrt{\frac{10^{-7}}{10^{-3}}} = 5 \cdot 10^{-2} \cdot \sqrt{10^{-4}} = 5 \times 10^{-2} \times 10^{-2} = 0.05$$

With $\zeta = 0.05$, this circuit is highly underdamped. A step input produces vigorous ringing at about 15.9 kHz, with the amplitude decaying very slowly. You would see dozens of oscillation cycles before the ringing settles.

In a SPICE netlist:

```text
* Series RLC with step input
V1 in 0 PULSE(0 1 0 1n 1n 1m 2m)
R1 in mid 10
L1 mid out 1m
C1 out 0 100n
.TRAN 1u 200u
```

### Why RLC ringing matters for simulation

RLC circuits are a stress test for transient simulation. The adaptive timestep must be small enough to resolve the oscillation frequency — if the timestep is larger than about $1/(10 f_0)$, the simulator will miss peaks or, worse, produce numerically unstable results.

The integration method matters too. The trapezoidal rule is second-order accurate and preserves energy well, but it can introduce its own **numerical ringing** on stiff circuits (an artifact where the numerical solution oscillates around the true solution). Gear methods are more stable but slightly less accurate for oscillatory circuits. The choice between them is one of the practical trade-offs in transient analysis, discussed in Chapter 9.

---

## Coupled inductors: transformer circuits

Mutual inductors enable transformer simulation. The simplest transformer circuit has a primary winding driven by a source and a secondary winding connected to a load:

```ferrite-circuit
circuit "Transformer" {
    node "in" label="Vin"
    node "gnd" ground=#true
    group "primary" topology="generic" {
        component "Vin" type="voltage-source" role="signal-input" {
            value "AC 1"
            port "pos" net="in"
            port "neg" net="gnd"
        }
        component "L1" type="inductor" role="passive" {
            value "100u"
            port "1" net="in"
            port "2" net="gnd"
        }
    }
    group "secondary" topology="generic" {
        component "L2" type="inductor" role="passive" {
            value "1u"
            port "1" net="out"
            port "2" net="sec_gnd"
        }
        component "RL" type="resistor" role="passive" {
            value "50"
            port "1" net="out"
            port "2" net="sec_gnd"
        }
    }
    node "out" label="Vout"
    node "sec_gnd" ground=#true
}
```

With coupling coefficient $k$ close to 1 (K1 L1 L2 0.99), the voltage across the secondary is:

$$V_{\text{out}} \approx V_{\text{in}} \cdot \sqrt{\frac{L_2}{L_1}}$$

If $L_1 = 100\,\mu\text{H}$ and $L_2 = 1\,\mu\text{H}$, the turns ratio is $\sqrt{1/100} = 0.1$, producing a 10:1 step-down. The secondary voltage is approximately $V_{\text{in}}/10$.

The leakage inductance ($1 - k$ fraction of each winding's inductance) causes the transformer's frequency response to roll off at high frequencies. With $k = 0.99$, 1% of each winding's flux doesn't couple to the other — this appears as a small series inductance that creates an RL low-pass filter.

### Energy transfer

In a lossless transformer ($k = 1$, zero winding resistance), all power delivered to the primary appears at the secondary:

$$V_1 I_1 = V_2 I_2$$

The voltage steps down by the turns ratio, but the current steps *up* by the same factor. A 10:1 step-down transformer that receives 10V at 100 mA delivers 1V at 1A. Power is conserved.

In transient simulation, the energy transfer is mediated by the mutual flux. At each timestep, the mutual inductor's `pre_load()` adds cross-flux terms, and the companion models of both inductors automatically reflect the coupling. No special transformer element is needed — the K element plus two inductors captures the complete physics.

---

## Parallel LC resonance

A parallel LC circuit (inductor and capacitor in parallel) creates a **resonant tank** — a circuit with very high impedance at the resonant frequency and low impedance at all other frequencies:

```ferrite-circuit
circuit "Parallel LC Tank" {
    node "a" label="VIN" rail=#true voltage="1"
    node "gnd" ground=#true
    group "tank" topology="generic" {
        component "L1" type="inductor" role="passive" {
            value "100u"
            port "1" net="a"
            port "2" net="gnd"
        }
        component "C1" type="capacitor" role="passive" {
            value "100n"
            port "1" net="a"
            port "2" net="gnd"
        }
    }
}
```

At resonance ($\omega = \omega_0$), the inductive and capacitive currents are equal and opposite — they circulate within the tank, and the impedance seen from outside is theoretically infinite (limited only by parasitic resistance). This is the basis of oscillators, RF filters, and tuned circuits.

Adding a small resistance $R$ in parallel limits the peak impedance to $R$ and creates a bandpass response with quality factor:

$$Q = R\sqrt{\frac{C}{L}}$$

A high-Q tank has a sharp resonance peak — it selects a narrow band of frequencies. A low-Q tank has a broad peak. The bandwidth of the resonance is $\Delta f = f_0 / Q$.

---

## Simulation considerations

Reactive circuits require care in transient simulation:

1. **Timestep control:** The simulator must resolve the fastest oscillation in the circuit. For an RLC ring at 15.9 kHz, the timestep should be no larger than about 6 $\mu$s (10 points per period). The LTE-based adaptive timestep handles this automatically, but setting `TSTEP` (the maximum print interval) to something reasonable helps the initial timestep estimate.

2. **Initial conditions:** Capacitor and inductor initial conditions determine the initial energy in the circuit. A capacitor charged to $V_0$ stores energy $\frac{1}{2}CV_0^2$. An inductor carrying $I_0$ stores energy $\frac{1}{2}LI_0^2$. These initial energies drive the transient response.

3. **Integration method:** For oscillatory circuits, the trapezoidal method preserves energy better than Gear methods (it's "A-stable" and has no numerical damping). But for stiff circuits with widely separated time constants, Gear methods are more robust. The choice affects the accuracy of the oscillation envelope over long simulations.

4. **Breakpoints:** Reactive circuits driven by PULSE or PWL sources benefit from breakpoints that force the simulator to land on source transitions. Without breakpoints, the adaptive timestep might step over a fast edge and produce an inaccurate initial condition for the subsequent ringing.

<!-- TODO: interactive RLC simulator — adjustable R, L, C; step input; see the waveform evolve in real time; overlay the analytical envelope; toggle between trapezoidal and Gear to see the difference in numerical damping -->
