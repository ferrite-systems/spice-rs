# The Body Effect

In textbook circuits, the MOSFET source is connected to the substrate (bulk). In real circuits, it often is not. When $V_{BS} \neq 0$, the threshold voltage shifts -- and this shift can be large enough to change the operating region of the device.

This is the *body effect*, and it is governed by two parameters: GAMMA ($\gamma$) and PHI ($\phi$).

## The physical intuition

The gate voltage creates an electric field that pushes carriers away from the surface, forming the depletion region that precedes channel formation. But the substrate has its own potential. If the source is lifted above the substrate ($V_{SB} > 0$, which means $V_{BS} < 0$ for NMOS), the depletion region under the gate widens.

A wider depletion region means more charge that the gate must overcome before a channel forms. So the threshold voltage increases.

```text
  VBS = 0                        VBS < 0 (source above bulk)
                                  
  Gate   VGS                     Gate   VGS
  ========================       ========================
  oxide                          oxide
  ~~~~~~~~~~~~~~~~~~~~~~~~       ~~~~~~~~~~~~~~~~~~~~~~~~
  - - - - channel - - - -        - - - - channel - - - -
  . . . . . . . . . . . .       . . . . . . . . . . . .
  . .  depletion   . . .        . . . . . . . . . . . .
  . . . region . . . . .        . . .  depletion   . . .
  . . . . . . . . . . . .       . . . . region . . . . .
                                 . . . . . . . . . . . .
  Bulk (0V)                      . . . . . . . . . . . .
                                 Bulk (more negative)
```

Think of it this way: the substrate is a "back gate." Making $V_{BS}$ more negative is like pulling the threshold voltage higher. It is a second knob that modulates the channel, less efficient than the gate but always present.

## The equation

The modified threshold voltage is:

$$V_{TH} = V_{TO} + \gamma \left( \sqrt{\phi - V_{BS}} - \sqrt{\phi} \right)$$

where:

| Parameter | Symbol | Typical NMOS | Meaning |
|-----------|--------|-------------|---------|
| VTO | $V_{TO}$ | 0.7 V | Zero-bias threshold voltage |
| GAMMA | $\gamma$ | 0.4 V$^{1/2}$ | Body effect coefficient |
| PHI | $\phi$ | 0.6 V | Surface potential (twice the Fermi potential, $2\phi_F$) |

For NMOS, $V_{BS} \leq 0$ in normal operation (source at or above bulk potential), so $\phi - V_{BS} \geq \phi$, and the square root term is always $\geq \sqrt{\phi}$. Therefore the body effect always *increases* $V_{TH}$ above $V_{TO}$.

## How large is the shift?

Consider $\gamma = 0.4$ V$^{1/2}$, $\phi = 0.6$ V, $V_{BS} = -2$ V:

$$\Delta V_{TH} = 0.4 \left( \sqrt{0.6 + 2} - \sqrt{0.6} \right) = 0.4 \left( 1.612 - 0.775 \right) = 0.335 \text{ V}$$

A threshold shift of 335 mV -- nearly half the $V_{TO}$ value. This is not a minor correction.

<!-- TODO: interactive body effect calculator -- sliders for VBS, GAMMA, PHI, show VTH shift in real time -->

## Where it matters in circuits

The body effect appears everywhere that a MOSFET source is not tied to its bulk:

```ferrite-circuit
circuit "Stacked NMOS" {
    node "vdd" label="VDD" rail=#true voltage="5"
    node "gnd" ground=#true
    group "stack" topology="generic" {
        component "M2" type="nmos" role="active-device" {
            port "drain" net="vdd"
            port "gate" net="vdd"
            port "source" net="vx"
        }
        component "M1" type="nmos" role="active-device" {
            port "drain" net="vx"
            port "gate" net="vdd"
            port "source" net="gnd"
        }
    }
    node "vx" label="VX"
}
```

In a stacked NMOS (like a NAND gate), M2's source sits at node VX, which is above ground when M1 is on. If the NMOS bulk is tied to ground, then M2 has $V_{BS} < 0$, its threshold rises, and it passes less current than a naive calculation would predict.

This is why PMOS devices in a standard CMOS process have their bulk tied to VDD (the highest potential) and NMOS devices have their bulk tied to ground (the lowest potential) -- to minimize the body effect.

## The gmbs transconductance

The body effect introduces a third transconductance into the small-signal model. In addition to $g_m$ (gate) and $g_{ds}$ (output), there is:

$$g_{mbs} = \frac{\partial I_{DS}}{\partial V_{BS}} = g_m \cdot \frac{\gamma}{2\sqrt{\phi - V_{BS}}}$$

This is the rate at which drain current changes with bulk-source voltage. It stamps into the MNA matrix as a voltage-controlled current source between drain and source, controlled by $V_{BS}$:

```text
        Drain
          |
     +----+----+----+
     |    |    |    |
    gds  gm  gmbs  Ieq
     |   Vgs  Vbs   |
     |    |    |    |
     +----+----+----+
          |
        Source
```

In spice-rs, `gmbs` is computed alongside `gm` and `gds` in the device load function and stamped into the matrix in the same pass. The equivalent current source becomes:

$$I_{eq} = I_{DS} - g_m V_{GS} - g_{ds} V_{DS} - g_{mbs} V_{BS}$$

## Connection to process parameters

GAMMA and PHI are not arbitrary fitting parameters -- they come from the fabrication process:

$$\gamma = \frac{\sqrt{2 q \epsilon_{Si} N_A}}{C_{ox}}$$

$$\phi = 2 \phi_F = 2 \frac{kT}{q} \ln\left(\frac{N_A}{n_i}\right)$$

where $N_A$ is the substrate doping concentration, $C_{ox}$ is the oxide capacitance per unit area, and $n_i$ is the intrinsic carrier concentration. Higher doping means a larger GAMMA and a stronger body effect. Thinner oxide means larger $C_{ox}$, which reduces GAMMA -- one of many reasons the industry has pushed oxide thickness down.
