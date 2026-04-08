# Amplifier Circuits

The BJT's high transconductance and well-controlled current gain make it the natural choice for analog amplifier design. The common-emitter configuration is the most widely used BJT amplifier topology -- it provides both voltage gain and current gain.

## The common-emitter amplifier

```ferrite-circuit
circuit "Common-Emitter Amplifier" {
    node "vcc" label="VCC" rail=#true voltage="12"
    node "gnd" ground=#true
    group "amplifier" topology="common-emitter" {
        component "Q1" type="npn" role="active-device" {
            model "NPN"
            port "collector" net="vout"
            port "base" net="vin"
            port "emitter" net="gnd"
        }
        component "RC" type="resistor" role="drain-load" {
            value "1k"
            port "1" net="vcc"
            port "2" net="vout"
        }
    }
    group "bias" topology="signal-source" {
        component "Vin" type="voltage-source" role="signal-input" {
            value "DC 0.7"
            port "pos" net="vin"
            port "neg" net="gnd"
        }
    }
    node "vout" label="Vout"
}
```

This is the simplest form: a single NPN transistor with a collector resistor. The input voltage $V_{in}$ drives the base; the output is taken at the collector.

## DC operating point

SPICE finds the DC operating point by solving Kirchhoff's current law at every node. For this circuit, the key equation at the collector node is:

$$\frac{V_{CC} - V_{out}}{R_C} = I_C$$

where $I_C = I_S \exp(V_{BE}/V_T)$ from the Gummel-Poon model. With $V_{BE} = V_{in} = 0.7$ V:

$$I_C = 10^{-15} \exp\left(\frac{0.7}{0.026}\right) \approx 5 \text{ mA}$$

$$V_{out} = V_{CC} - I_C \cdot R_C = 12 - 5 \times 10^{-3} \times 1000 = 7 \text{ V}$$

The device is in forward active mode ($V_{CE} = 7$ V $> 0.2$ V), which is the desired operating region for amplification.

## Small-signal gain

Once SPICE has the DC operating point, the small-signal analysis linearizes around it. The voltage gain is:

$$A_v = -g_m \cdot (R_C \| r_o)$$

At $I_C = 5$ mA:

$$g_m = \frac{5 \text{ mA}}{26 \text{ mV}} = 192 \text{ mS}$$

$$A_v \approx -g_m \cdot R_C = -192 \times 10^{-3} \times 1000 = -192$$

The negative sign means the output is inverted relative to the input. A small increase in $V_{BE}$ causes a large increase in $I_C$, which causes a large voltage drop across $R_C$, pulling $V_{out}$ down.

<!-- TODO: interactive CE amplifier -- adjust VBE and RC, see the operating point on the I-V curves and the small-signal gain update -->

## The three BJT amplifier topologies

The common-emitter is one of three fundamental BJT amplifier configurations:

```ferrite-circuit
circuit "Common-Emitter" {
    node "vcc" label="VCC" rail=#true voltage="12"
    node "gnd" ground=#true
    group "amplifier" topology="common-emitter" {
        component "Q1" type="npn" role="active-device" {
            port "collector" net="vout"
            port "base" net="vin"
            port "emitter" net="gnd"
        }
        component "RC" type="resistor" role="drain-load" {
            value "1k"
            port "1" net="vcc"
            port "2" net="vout"
        }
    }
    group "input" topology="signal-source" {
        component "Vin" type="voltage-source" role="signal-input" {
            value "AC"
            port "pos" net="vin"
            port "neg" net="gnd"
        }
    }
    node "vout" label="Vout"
}
```

```ferrite-circuit
circuit "Common-Base" {
    node "vcc" label="VCC" rail=#true voltage="12"
    node "gnd" ground=#true
    group "amplifier" topology="generic" {
        component "Q1" type="npn" role="active-device" {
            port "collector" net="vout"
            port "base" net="vb"
            port "emitter" net="vin"
        }
        component "RC" type="resistor" role="drain-load" {
            value "1k"
            port "1" net="vcc"
            port "2" net="vout"
        }
        component "RE" type="resistor" role="source-degeneration" {
            value "1k"
            port "1" net="vin"
            port "2" net="gnd"
        }
    }
    node "vb" label="VB"
    node "vout" label="Vout"
}
```

```ferrite-circuit
circuit "Common-Collector" {
    node "vcc" label="VCC" rail=#true voltage="12"
    node "gnd" ground=#true
    group "amplifier" topology="common-collector" {
        component "Q1" type="npn" role="active-device" {
            port "collector" net="vcc"
            port "base" net="vin"
            port "emitter" net="vout"
        }
        component "RE" type="resistor" role="source-degeneration" {
            value "1k"
            port "1" net="vout"
            port "2" net="gnd"
        }
    }
    group "input" topology="signal-source" {
        component "Vin" type="voltage-source" role="signal-input" {
            value "AC"
            port "pos" net="vin"
            port "neg" net="gnd"
        }
    }
    node "vout" label="Vout"
}
```

| Topology | Voltage gain | Input impedance | Output impedance | Use |
|----------|-------------|-----------------|------------------|-----|
| Common-emitter | $-g_m R_C$ (high) | $r_\pi$ (medium) | $R_C$ (medium) | General amplification |
| Common-base | $g_m R_C$ (high) | $1/g_m$ (low) | $R_C$ (medium) | High-frequency, cascode |
| Common-collector | $\approx 1$ (unity) | $\beta r_o$ (very high) | $1/g_m$ (low) | Impedance buffer |

Each topology has the same BJT, the same Gummel-Poon model, the same MNA stamps. What changes is the circuit around it -- which terminal is the input, which is the output, which is the AC ground. SPICE does not know or care about the topology; it simply solves the matrix. The topology is a *human* abstraction for understanding the result.

## Biasing in practice

The simple circuit above has a serious problem: the DC operating point is extremely sensitive to $V_{BE}$. A 10 mV change in $V_{BE}$ (from temperature variation or device mismatch) changes $I_C$ by about 50%. Practical amplifiers use feedback biasing to stabilize the operating point:

```ferrite-circuit
circuit "Biased CE Amplifier" {
    node "vcc" label="VCC" rail=#true voltage="12"
    node "gnd" ground=#true
    group "bias" topology="voltage-divider" {
        component "R1" type="resistor" role="divider-upper" {
            value "10k"
            port "1" net="vcc"
            port "2" net="base"
        }
        component "R2" type="resistor" role="divider-lower" {
            value "2.2k"
            port "1" net="base"
            port "2" net="gnd"
        }
    }
    group "amplifier" topology="common-emitter" {
        component "Q1" type="npn" role="active-device" {
            port "collector" net="vout"
            port "base" net="base"
            port "emitter" net="emitter"
        }
        component "RC" type="resistor" role="drain-load" {
            value "1k"
            port "1" net="vcc"
            port "2" net="vout"
        }
        component "RE" type="resistor" role="source-degeneration" {
            value "220"
            port "1" net="emitter"
            port "2" net="gnd"
        }
    }
    node "vout" label="Vout"
}
```

The voltage divider (R1, R2) sets the base voltage. The emitter resistor RE provides negative feedback: if $I_C$ increases, the voltage across RE increases, reducing $V_{BE}$, which reduces $I_C$ back down. The price is reduced gain:

$$A_v = -g_m \cdot \frac{R_C}{1 + g_m R_E} \approx -\frac{R_C}{R_E}$$

For AC amplification, RE is often bypassed with a capacitor to restore the full gain at signal frequencies while keeping the DC stabilization.

## What SPICE sees

When SPICE simulates this circuit, it does not think in terms of "common-emitter" or "biasing." It sees:
- A set of nodes connected by components
- Each BJT stamps $g_m$, $g_\pi$, $g_o$, $g_\mu$, capacitances, and current sources into the MNA matrix
- Each resistor stamps a conductance
- Newton-Raphson solves the resulting system

The beauty of SPICE is that the same engine handles any topology, any bias scheme, any combination of devices. The device models from the [Gummel-Poon chapter](ch06-01-gummel-poon.md) provide the physics; the MNA framework provides the mathematics; the simulator provides the iteration. Understanding the amplifier topology helps you *interpret* the result, but SPICE does not need that understanding to compute it.
