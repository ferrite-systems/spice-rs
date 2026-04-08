# Circuits: Rectifiers

The diode's ability to conduct in only one direction makes it the core component of **rectifiers** — circuits that convert AC to DC. These circuits are a natural test bed for everything we've covered: the Shockley equation, NR iteration, voltage limiting, and junction capacitance.

---

## Half-wave rectifier

The simplest rectifier: a diode in series with the load.

```ferrite-circuit
circuit "Half-Wave Rectifier" {
    node "in" label="in"
    node "gnd" ground=#true
    group "source" topology="signal-source" {
        component "Vin" type="voltage-source" role="signal-input" {
            value "AC 5V 60Hz"
            port "pos" net="in"
            port "neg" net="gnd"
        }
    }
    group "rectifier" topology="rectifier" {
        component "D1" type="diode" role="rectifier-diode" {
            port "anode" net="in"
            port "cathode" net="out"
        }
    }
    group "load" topology="generic" {
        component "R1" type="resistor" role="passive" {
            value "1k"
            port "1" net="out"
            port "2" net="gnd"
        }
        component "C1" type="capacitor" role="filter-element" {
            value "100u"
            port "1" net="out"
            port "2" net="gnd"
        }
    }
    node "out" label="Vout"
}
```

The SPICE netlist:

```spice
Half-Wave Rectifier
Vin in 0 SIN(0 5 60)
D1 in out DMOD
R1 out 0 1k
C1 out 0 100u
.model DMOD D(IS=1e-14 N=1 RS=10 CJO=5p TT=5n)
.tran 0.1m 50m
.end
```

### What happens

During the positive half-cycle, the input voltage rises above the output voltage plus the diode's forward drop (~0.65V). The diode conducts, charging the capacitor and delivering current to the load.

During the negative half-cycle, the input drops below the output. The diode is reverse-biased and blocks. The capacitor slowly discharges through the load resistor, holding the output near its peak value.

The result is a DC output with **ripple** — small periodic dips as the capacitor discharges between conduction pulses.

### What the simulator computes

At each timestep, the simulator:

1. Evaluates the AC source at the current time to get $V_{in}(t)$
2. Runs Newton-Raphson to find the operating point, including:
   - The diode's forward/reverse state (the Shockley equation)
   - The capacitor's current ($C \cdot dV/dt$, via numerical integration)
   - The resistor's current ($V_{out}/R$)
3. Records $V_{out}(t)$, $I_{D1}(t)$, etc.

The first few cycles are the most challenging — the capacitor starts uncharged, so the diode must pass large currents to charge it up. This produces the largest NR step sizes and the most limiting activity. After a few cycles, the circuit settles into a periodic steady state where each timestep converges in just 3-5 iterations.

### The diode's forward drop

The diode "costs" about 0.65V: the peak output is about 4.35V instead of 5V. This is the forward voltage drop we computed in earlier sections. With a real diode model that includes series resistance (RS = 10 ohms), the drop is slightly higher under load because $V_{drop} = V_{diode} + I_D \cdot R_S$.

---

## Full-wave bridge rectifier

A bridge rectifier uses four diodes to capture both half-cycles:

```ferrite-circuit
circuit "Bridge Rectifier" {
    node "in_p" label="in+"
    node "in_n" label="in-"
    node "gnd" ground=#true
    group "source" topology="signal-source" {
        component "Vin" type="voltage-source" role="signal-input" {
            value "AC 12V 60Hz"
            port "pos" net="in_p"
            port "neg" net="in_n"
        }
    }
    group "bridge" topology="rectifier" {
        component "D1" type="diode" role="rectifier-diode" {
            port "anode" net="in_p"
            port "cathode" net="out_p"
        }
        component "D2" type="diode" role="rectifier-diode" {
            port "anode" net="out_n"
            port "cathode" net="in_p"
        }
        component "D3" type="diode" role="rectifier-diode" {
            port "anode" net="in_n"
            port "cathode" net="out_p"
        }
        component "D4" type="diode" role="rectifier-diode" {
            port "anode" net="out_n"
            port "cathode" net="in_n"
        }
    }
    group "load" topology="generic" {
        component "R1" type="resistor" role="passive" {
            value "1k"
            port "1" net="out_p"
            port "2" net="out_n"
        }
        component "C1" type="capacitor" role="filter-element" {
            value "470u"
            port "1" net="out_p"
            port "2" net="out_n"
        }
    }
    node "out_p" label="out+"
    node "out_n" label="out-"
}
```

The SPICE netlist captures the same topology:

```spice
Bridge Rectifier
Vin in_p in_n SIN(0 12 60)
D1 in_p out_p DMOD
D2 out_n in_p DMOD
D3 in_n out_p DMOD
D4 out_n in_n DMOD
R1 out_p out_n 1k
C1 out_p out_n 470u
.model DMOD D(IS=1e-14 N=1 RS=0.5)
.tran 0.1m 100m
.end
```

### What happens

On the positive half-cycle ($V_{in\_p} > V_{in\_n}$), diodes D1 and D4 conduct. Current flows: $\text{in\_p} \to D1 \to \text{out\_p} \to R1 \to \text{out\_n} \to D4 \to \text{in\_n}$.

On the negative half-cycle ($V_{in\_n} > V_{in\_p}$), diodes D3 and D2 conduct. Current flows the opposite path through the source but the same direction through the load.

The result: the load sees a full-wave rectified signal — twice the frequency, half the ripple compared to the half-wave rectifier. Two diode drops are in the path (about 1.3V total), so the peak output from a 12V peak source is about 10.7V.

### Simulation challenges

Bridge rectifiers are interesting for simulation because they have rapid diode switching events. Twice per cycle, two diodes turn off and two turn on. At each switching event, the diode voltages change rapidly and the NR iteration must track the transition from forward conduction to reverse blocking (or vice versa).

The series resistance RS matters here: even a small RS creates an internal node inside each diode (between the external terminal and the junction), which adds unknowns to the matrix but improves convergence by limiting how quickly the junction voltage can change.

---

## What to observe

When running these simulations, look for:

- **Ripple voltage** — the periodic dip in $V_{out}$ between conduction pulses. Larger capacitance means less ripple.
- **Diode current pulses** — the diode conducts only near the peaks of the input waveform, in brief, high-current pulses. The peak diode current is much higher than the DC load current.
- **Forward voltage drop** — the ~0.65V offset between input peak and output peak. It increases slightly with current due to series resistance.
- **Reverse recovery** — if the diode model includes transit time (TT), there's a brief moment when the diode continues conducting after the voltage reverses, as the stored minority-carrier charge is swept out. This produces a reverse current spike visible in the diode current waveform.

<!-- TODO: interactive rectifier simulation
```spice-sim
analysis: tran
outputs: v(out), i(D1)
editable: C1, R1
---
Vin in 0 SIN(0 5 60)
D1 in out DMOD
R1 out 0 1k
C1 out 0 100u
.model DMOD D(IS=1e-14 N=1 RS=10 TT=5n)
.tran 0.1m 50m
.end
```
-->
