# Three Regions of Operation

Every MOSFET model -- from the simplest textbook equation to BSIM4's five thousand lines of code -- begins with the same question: *which region is this device operating in?*

The answer depends on just two voltages: $V_{GS}$ (gate-to-source) and $V_{DS}$ (drain-to-source), measured relative to one parameter: $V_{TO}$ (threshold voltage).

## The physical picture

Imagine the MOSFET as a channel of water between two reservoirs (source and drain), controlled by a gate that can raise or lower the channel floor.

```text
  VGS < VTO              VGS > VTO, small VDS       VGS > VTO, large VDS
                                                      
  Gate: OFF              Gate: ON                    Gate: ON
  ~~~~~~~~~~~~           ~~~~~~~~~~~~                ~~~~~~~~~~~~
  |  oxide   |           |  oxide   |                |  oxide   |
  |          |           |==========|                |======    |
  S          D           S==========D                S======  . D
  |  no      |           | channel  |                |channel \ |
  | channel  |           | (uniform)|                |(pinched) |
                                                      
  CUTOFF                 LINEAR (TRIODE)             SATURATION
  IDS = 0                IDS ~ VDS                   IDS ~ const
```

**Cutoff** ($V_{GS} < V_{TO}$): The gate voltage is too low to form a conducting channel. No current flows. The device is off.

**Linear / Triode** ($V_{GS} \geq V_{TO}$ and $V_{DS} < V_{GS} - V_{TO}$): The channel exists from source to drain. Current increases roughly linearly with $V_{DS}$. The device behaves like a voltage-controlled resistor.

**Saturation** ($V_{GS} \geq V_{TO}$ and $V_{DS} \geq V_{GS} - V_{TO}$): The channel is "pinched off" near the drain. Current no longer increases with $V_{DS}$ -- it depends only on $V_{GS}$. The device behaves like a voltage-controlled current source.

<!-- TODO: interactive region-selector widget -- drag VGS and VDS sliders, see the region highlight on the I-V plane and the channel diagram animate -->

## The three equations

For an NMOS with threshold voltage $V_{TO}$ and transconductance parameter $K = \frac{1}{2} \mu_n C_{ox} \frac{W}{L}$:

**Cutoff** ($V_{GS} < V_{TO}$):

$$I_{DS} = 0$$

**Linear** ($V_{GS} \geq V_{TO}$, $V_{DS} < V_{GS} - V_{TO}$):

$$I_{DS} = K \left[ 2(V_{GS} - V_{TO}) V_{DS} - V_{DS}^2 \right]$$

**Saturation** ($V_{GS} \geq V_{TO}$, $V_{DS} \geq V_{GS} - V_{TO}$):

$$I_{DS} = K (V_{GS} - V_{TO})^2$$

These are sometimes called the "square-law" equations because of the quadratic dependence on gate overdrive $V_{GS} - V_{TO}$ in saturation. They are the skeleton of the Level 1 model.

## The I-V family

If you sweep $V_{DS}$ from 0 to some maximum for several fixed values of $V_{GS}$, you get the characteristic *I-V family of curves*:

```text
  IDS
   ^
   |                          _____________  VGS = 5V
   |                    ____/
   |               ___/      _____________  VGS = 4V
   |          ___/     ____/
   |     ___/    ____/       _____________  VGS = 3V
   |   /   ____/       ___/
   |  / __/        ___/
   | //        ___/          _____________  VGS = 2V
   |/      __/          ___/
   |   __/          __/
   +--/----------/--------------------------------> VDS
   0
        linear    |   saturation
                  |
            VDS = VGS - VTO
            (boundary)
```

Each curve rises steeply in the linear region, then flattens in saturation. The boundary between regions is the parabola $V_{DS} = V_{GS} - V_{TO}$.

In reality, the saturation curves are not perfectly flat -- they have a slight upward slope due to *channel-length modulation* (the $\lambda$ parameter). We address that in the [Level 1](ch05-02-level1.md) section.

<!-- TODO: interactive I-V plotter -- sweep VDS for multiple VGS values, highlight active region, show operating point -->

## A circuit to see it

Here is an NMOS with a drain resistor, forming the simplest common-source configuration. The operating point lies where the device's I-V curve intersects the load line set by RD and VDD.

```ferrite-circuit
circuit "NMOS Common-Source" {
    node "vdd" label="VDD" rail=#true voltage="5"
    node "gnd" ground=#true
    group "amplifier" topology="common-source" {
        component "M1" type="nmos" role="active-device" {
            model "NMOS" level="1" VTO="1.0" KP="110u"
            port "drain" net="vout"
            port "gate" net="vin"
            port "source" net="gnd"
        }
        component "RD" type="resistor" role="drain-load" {
            value "2k"
            port "1" net="vdd"
            port "2" net="vout"
        }
    }
    group "bias" topology="signal-source" {
        component "Vin" type="voltage-source" role="signal-input" {
            value "DC 2.5"
            port "pos" net="vin"
            port "neg" net="gnd"
        }
    }
    node "vout" label="Vout"
}
```

With $V_{GS} = 2.5\text{ V}$ and $V_{TO} = 1.0\text{ V}$, the device is on. Whether it sits in the linear or saturation region depends on $V_{DS}$, which is set by the current through RD. SPICE solves this nonlinear system using Newton-Raphson iteration -- the subject of [Chapter 2](ch02-dc-operating-point.md).

## Why regions matter for simulation

Newton-Raphson needs derivatives. The drain current equation and its partial derivatives ($g_m = \partial I_{DS}/\partial V_{GS}$, $g_{ds} = \partial I_{DS}/\partial V_{DS}$) have different forms in each region. At every iteration, the simulator must:

1. Check which region the device is in
2. Evaluate $I_{DS}$, $g_m$, and $g_{ds}$ using that region's equations
3. Stamp these into the MNA matrix
4. Solve and iterate

Getting the region boundaries right -- and making the transitions smooth -- is one of the most important details in MOSFET modeling. A discontinuity at a region boundary can cause Newton-Raphson to oscillate endlessly between two regions, never converging. The [Level 1 model](ch05-02-level1.md) shows exactly how spice-rs handles this.
