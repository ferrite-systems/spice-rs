# BSIM3v3: The Industry Standard

The Berkeley Short-channel IGFET Model, version 3 (BSIM3v3), is the most widely used MOSFET model in the semiconductor industry. When a foundry gives you a "model card" for their process, it is almost certainly BSIM3 or its successor BSIM4.

BSIM3 is not an incremental improvement over Level 1-3. It is a fundamentally different approach: instead of a handful of physics-based parameters, it uses 150+ parameters organized into groups, each group capturing a specific physical effect that matters at sub-micron dimensions.

## Why BSIM3 exists

Level 1-3 were designed for channel lengths above 1 $\mu$m. As transistors shrank below 0.5 $\mu$m, several effects became dominant that these models either ignore or handle poorly:

```text
  Channel length:  10μm     1μm      0.25μm    0.13μm    65nm
                    |        |          |         |        |
  Level 1       ████████
  Level 2/3              ████████
  BSIM3                          ████████████████████
  BSIM4                                    ██████████████████
```

The critical short-channel effects that BSIM3 captures:

**Drain-Induced Barrier Lowering (DIBL):** The drain voltage lowers the source-channel barrier, reducing the effective threshold. At long channel lengths this is negligible. At 0.25 $\mu$m, it can shift the threshold by hundreds of millivolts.

**Channel-Length Modulation (CLM):** Level 1's simple $\lambda$ parameter is a first-order approximation. BSIM3 models the actual depletion region extension near the drain, which depends nonlinearly on bias.

**Velocity Saturation:** Carriers reach maximum velocity in very short channels, making the I-V characteristics more linear than quadratic. BSIM3 handles the smooth transition from square-law to velocity-saturated behavior.

**Quantum Mechanical Effects:** At thin oxide thicknesses (<5 nm), the inversion layer charge is pushed slightly away from the oxide interface by quantum confinement. This effectively increases the oxide thickness and reduces the gate capacitance.

**Polysilicon Depletion:** When the gate is made of polysilicon (not metal), it can partially deplete, adding a series capacitance that reduces the effective gate control.

## The parameter groups

BSIM3's 150+ parameters are organized into functional groups. You do not need to understand every parameter -- most are extracted by the foundry. But knowing the groups gives you a map of what physics the model captures:

| Group | Key Parameters | Physical Effect |
|-------|---------------|-----------------|
| Threshold voltage | VTH0, K1, K2, K3 | Base threshold, body effect, narrow-channel |
| Mobility | U0, UA, UB, UC | Low-field mobility, field degradation |
| Velocity saturation | VSAT, A0, AGS | Carrier velocity limit, source-end velocity |
| DIBL | ETA0, ETAB, DSUB | Drain-induced threshold shift |
| CLM | PCLM, PDIBLC1, PDIBLC2 | Output conductance in saturation |
| Subthreshold | VOFF, NFACTOR | Leakage current below threshold |
| Output conductance | PSCBE1, PSCBE2 | Substrate current-induced effects |
| Capacitance | CLC, CLE, CGSO, CGDO | Intrinsic + overlap capacitances |
| Noise | NOIA, NOIB, NOIC | Flicker and thermal noise |
| Layout | WINT, LINT, DWG, DWB | Effective W/L corrections |

## The unified current equation

Unlike Level 1-3, which have separate equations for linear and saturation, BSIM3 uses a *single equation* that smoothly transitions between regions:

$$I_{DS} = \mu_{eff} \cdot C_{ox} \cdot \frac{W_{eff}}{L_{eff}} \cdot \frac{(V_{GS} - V_{TH})^2}{1 + \frac{V_{GS} - V_{TH}}{E_{sat} L_{eff}}} \cdot \frac{1}{1 + R_{DS} \cdot g_{ds0}}$$

This is a simplified sketch -- the full equation in the BSIM3 manual spans multiple pages. But the structure reveals the key ideas:

- The $(V_{GS} - V_{TH})^2$ term is the familiar square law
- The $1/(1 + (V_{GS}-V_{TH})/(E_{sat} L_{eff}))$ factor smoothly transitions from square-law to velocity-saturated behavior
- $\mu_{eff}$ is itself a function of vertical and lateral fields
- $W_{eff}$ and $L_{eff}$ are the drawn dimensions corrected for process effects (etching, diffusion)
- $R_{DS}$ is the parasitic source/drain resistance

<!-- TODO: interactive BSIM3 I-V comparison -- plot Level 1 vs BSIM3 for the same device, sweep channel length to show where they diverge -->

## Threshold voltage in BSIM3

The BSIM3 threshold is far more complex than Level 1's $V_{TO} + \gamma(\sqrt{\phi - V_{BS}} - \sqrt{\phi})$:

$$V_{TH} = V_{TH0} + K_1 \sqrt{\phi_s - V_{BS}} - K_2 V_{BS} - \Delta V_{TH,SCE} - \Delta V_{TH,DIBL}$$

where:
- $V_{TH0}$ is the long-channel threshold at zero bias
- $K_1$ is the first-order body effect coefficient
- $K_2$ captures the non-uniform doping profile
- $\Delta V_{TH,SCE}$ is the short-channel threshold reduction
- $\Delta V_{TH,DIBL}$ is the drain-induced barrier lowering correction

Each of these terms is itself a function of geometry and bias, with its own set of parameters.

## Effective dimensions

Real transistors are not the idealized rectangles of the mask layout. Etching, lateral diffusion, and stress effects change the effective channel length and width:

$$L_{eff} = L_{drawn} - 2 \cdot LINT - \Delta L(V_{DS}, V_{GS})$$

$$W_{eff} = W_{drawn} - 2 \cdot WINT - \Delta W(V_{BS})$$

where $LINT$ and $WINT$ are the basic length/width reductions, and the $\Delta$ terms capture bias-dependent effects. Getting these corrections right is essential for matching silicon measurements.

## Capacitance model

BSIM3 replaces the Meyer capacitance model (used in Level 1-3) with a charge-based model. Instead of computing capacitances directly, it computes the *charges* on each terminal ($Q_G$, $Q_D$, $Q_S$, $Q_B$) and takes numerical derivatives:

$$C_{ij} = \frac{\partial Q_i}{\partial V_j}$$

This guarantees charge conservation -- the charges are continuous across region boundaries, eliminating the spurious current spikes that plague the Meyer model during transient analysis.

## In spice-rs

The BSIM3 implementation lives in `device/bsim3.rs` at approximately 2700 lines. This makes it the second-largest device model in spice-rs after BSIM4.

The structure follows the same load-function pattern as simpler models, but the computation inside is vastly more involved:

```text
fn load_bsim3(vgs, vds, vbs) {
    // 1. Compute effective dimensions (Leff, Weff)
    // 2. Compute threshold voltage (with SCE, DIBL, body effect)
    // 3. Compute effective mobility (field-dependent)
    // 4. Compute drain current (unified equation)
    // 5. Compute transconductances (gm, gds, gmbs)
    // 6. Compute terminal charges (Qg, Qd, Qs, Qb)
    // 7. Compute capacitances from charge derivatives
    // 8. Stamp everything into MNA matrix
}
```

Porting BSIM3 faithfully from the reference C code is one of the most demanding tasks in spice-rs development. The model has numerous internal flags, mode switches, and parameter interactions that must be preserved exactly. A single sign error or missing conditional can shift a device's operating point by millivolts -- enough to fail the test suite.

## Reading a BSIM3 model card

A foundry model card for BSIM3 looks something like this:

```text
.model nch nmos level=49 version=3.3
+tnom=27 toxe=1.8e-009 toxp=1.5e-009 toxm=1.8e-009
+dtox=3e-010 epsrox=3.9 wint=5e-009 lint=0
+vth0=0.489 k1=0.582 k2=-0.073 k3=80
+dvt0=1.2 dvt1=0.42 dvt2=0.05
+nlx=1.74e-007 w0=0 k3b=0.4
+vsat=1.58e+005 ua=-1.38e-009 ub=2.3e-018
+uc=-4.6e-011 rdsw=155 prwb=0 prwgs=0
+wr=1 u0=261 a0=1.2 ags=0.35
...
```

There may be 200+ lines. The foundry extracts these parameters from measurements on test structures fabricated in the same process as your design. You do not typically hand-tune them -- you trust the foundry's extraction.

The progression from Level 1's four parameters to BSIM3's 150+ is not complexity for its own sake. Each parameter captures a physical effect that is measurable in silicon and necessary for accurate circuit simulation at sub-micron dimensions.
