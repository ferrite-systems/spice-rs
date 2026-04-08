# The Gummel-Poon Model

The Gummel-Poon model (1970) is to the BJT what BSIM3 is to the MOSFET: the standard model that every SPICE simulator implements. It extends the ideal exponential transistor with base-width modulation, high-injection effects, and a unified treatment of forward and reverse operation.

## The transport current

At the heart of the Gummel-Poon model is the *transport current* -- the current that flows from collector to emitter through the base:

$$I_T = I_S \left( \exp\left(\frac{V_{BE}}{N_F \cdot V_T}\right) - \exp\left(\frac{V_{BC}}{N_R \cdot V_T}\right) \right)$$

where:

| Parameter | Symbol | Typical NPN | Meaning |
|-----------|--------|-------------|---------|
| IS | $I_S$ | $10^{-15}$ A | Saturation current |
| NF | $N_F$ | 1.0 | Forward emission coefficient |
| NR | $N_R$ | 1.0 | Reverse emission coefficient |
| VT | $V_T$ | 26 mV | Thermal voltage ($kT/q$) |

This is the Ebers-Moll transport equation. In forward active operation ($V_{BE} > 0$, $V_{BC} < 0$), the second exponential is negligible and $I_T \approx I_S \exp(V_{BE}/N_F V_T)$. In reverse active, the first exponential is negligible.

The emission coefficients $N_F$ and $N_R$ are ideality factors. For an ideal junction, $N = 1$. Real devices have $N$ slightly above 1, reflecting recombination in the depletion region.

## Forward and reverse beta

The base current is the sum of the forward and reverse components:

$$I_B = \frac{I_S}{B_F} \left( \exp\left(\frac{V_{BE}}{N_F \cdot V_T}\right) - 1 \right) + \frac{I_S}{B_R} \left( \exp\left(\frac{V_{BC}}{N_R \cdot V_T}\right) - 1 \right)$$

where:

| Parameter | Symbol | Typical NPN | Meaning |
|-----------|--------|-------------|---------|
| BF | $B_F$ | 100 | Ideal maximum forward beta |
| BR | $B_R$ | 1 | Ideal maximum reverse beta |

And the terminal currents are:

$$I_C = I_T - \frac{I_S}{B_R} \left( \exp\left(\frac{V_{BC}}{N_R \cdot V_T}\right) - 1 \right)$$

$$I_E = -I_T - \frac{I_S}{B_F} \left( \exp\left(\frac{V_{BE}}{N_F \cdot V_T}\right) - 1 \right)$$

In forward active operation, $I_C \approx I_T$ and $I_B \approx I_T / B_F$, giving the familiar $\beta = I_C / I_B \approx B_F$.

## Base charge modulation

The Gummel-Poon model's key innovation is the *normalized base charge* $q_b$, which modulates the transport current to account for the Early effect and high-injection:

$$I_T = \frac{I_S}{q_b} \left( \exp\left(\frac{V_{BE}}{N_F \cdot V_T}\right) - \exp\left(\frac{V_{BC}}{N_R \cdot V_T}\right) \right)$$

The base charge factor is:

$$q_b = \frac{q_1}{2} \left( 1 + \sqrt{1 + 4 q_2} \right)$$

where:

$$q_1 = 1 + \frac{V_{BE}}{V_{AF}} + \frac{V_{BC}}{V_{AR}}$$

$$q_2 = \frac{I_S}{IKF} \exp\left(\frac{V_{BE}}{N_F V_T}\right) + \frac{I_S}{IKR} \exp\left(\frac{V_{BC}}{N_R V_T}\right)$$

The $q_1$ term captures the **Early effect**: as $V_{CE}$ increases (making $V_{BC}$ more negative), the base width narrows, $q_1$ decreases, and $I_T$ increases. $V_{AF}$ and $V_{AR}$ are the forward and reverse Early voltages (covered in [Chapter 6.2](ch06-02-early-effect.md)).

The $q_2$ term captures **high-injection effects**: at large currents, the injected minority carrier density becomes comparable to the base doping, widening the effective base and reducing gain. $IKF$ and $IKR$ are the forward and reverse knee currents where high-injection effects begin.

```text
  β (current gain)
   ^
   |         _______________
   |        /               \
   |       /                 \  <-- high injection (q2)
   |      /                   \     β rolls off
   |     /                     \
   |    /                       \
   |   / <-- low current         \
   |  /     recombination         \
   | /      (ISE, ISC terms)       \
   +-----|---------|---------|------> log(IC)
       low     IKF/BF     IKF    high
```

<!-- TODO: interactive Gummel plot -- sweep VBE, show IC and IB on log scale, annotate the three regions of beta -->

## How the BJT stamps into MNA

The linearized BJT, like the MOSFET, becomes a companion model for Newton-Raphson:

```text
      Collector
          |
     +----+----+
     |    |    |
    go   gm   Ieq
     |  Vbe    |
     |    |    |
     +----+----+
          |
       Emitter
          
     Base---gpi---Emitter
     Base---gmu---Collector
```

The key conductances:

- $g_m = \partial I_C / \partial V_{BE}$ -- transconductance (base-emitter voltage controls collector current)
- $g_o = \partial I_C / \partial V_{CE}$ -- output conductance (Early effect)
- $g_\pi = \partial I_B / \partial V_{BE}$ -- input conductance (base current)
- $g_\mu = \partial I_B / \partial V_{BC}$ -- reverse feedback (usually very small)

These stamp into the MNA matrix at the (collector, emitter), (base, emitter), and (base, collector) positions, along with the equivalent current sources.

## In spice-rs

The Gummel-Poon implementation lives in `device/bjt.rs`. The load function follows this structure:

```text
fn load_bjt(vbe: f64, vbc: f64) {
    // 1. Compute junction exponentials
    //    exp(VBE / NF*VT), exp(VBC / NR*VT)
    // 2. Compute base charge factor qb
    //    (Early effect + high injection)
    // 3. Compute transport current IT = IS/qb * (...)
    // 4. Compute terminal currents IC, IB, IE
    // 5. Compute conductances gm, go, gpi, gmu
    // 6. Compute junction charges (for transient)
    // 7. Stamp into MNA matrix
}
```

The exponential nonlinearity of the BJT makes Newton-Raphson convergence more challenging than for MOSFETs. A 26 mV change in $V_{BE}$ changes $I_C$ by a factor of $e \approx 2.718$. The simulator must use voltage limiting (clamping the NR step to prevent overshooting) and careful initial-guess strategies to converge reliably.

This is a place where faithful porting from the ngspice reference matters deeply. The convergence aids -- voltage limiting, junction voltage initialization, source stepping -- are not optional niceties. They are essential for the simulator to converge on circuits with BJTs.
