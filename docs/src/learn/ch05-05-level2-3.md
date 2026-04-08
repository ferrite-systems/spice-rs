# Level 2 and Level 3

Level 1 captures the textbook MOSFET: square-law current, channel-length modulation, body effect. But real silicon departs from the square law in important ways, especially as channel lengths shrink. Level 2 and Level 3 bridge the gap between the pedagogical model and the empirical models (BSIM3/4) that foundries provide.

Both models extend Level 1 by adding physics that matters below about 2 $\mu$m channel length.

## Level 2: physics-based corrections

Level 2, sometimes called the "Grove-Frohman" model, keeps the same three-region structure but adds several physical effects:

### Velocity saturation (VMAX)

In Level 1, saturation occurs when the channel pinches off at $V_{DS} = V_{GS} - V_{TH}$. But in short channels, carriers reach their maximum drift velocity before pinch-off occurs. The saturation voltage drops:

$$V_{DSAT} = \min\left(V_{GS} - V_{TH}, \; \frac{v_{max} \cdot L}{\mu}\right)$$

where $v_{max}$ is the maximum carrier velocity (VMAX parameter) and $\mu$ is the mobility. When $v_{max} \cdot L / \mu < V_{GS} - V_{TH}$, the device saturates earlier and carries less current than the square law predicts.

```text
  IDS
   ^
   |          ____________  Level 1 (square law)
   |        /
   |      / /____________  Level 2 (velocity saturation)
   |    / /
   |  / /
   | //
   |/
   +-----------------------------> VDS
```

This is the single most important correction for short-channel devices.

### Subthreshold conduction (NEFF)

Level 1 has a hard cutoff: $V_{GS} < V_{TH}$ means $I_{DS} = 0$. In reality, current drops *exponentially* below threshold but never truly reaches zero. Level 2 adds a subthreshold region:

$$I_{DS,sub} \propto \exp\left(\frac{V_{GS} - V_{TH}}{n \cdot V_T}\right)$$

where $n$ is the subthreshold swing factor (related to the NEFF parameter) and $V_T = kT/q$ is the thermal voltage. This exponential tail determines the leakage current when the device is "off" -- critical for power consumption in digital circuits.

### Mobility degradation (UCRIT, UEXP)

Carrier mobility decreases under high vertical electric fields (the gate field pushes carriers against the oxide interface, increasing scattering). Level 2 models this as:

$$\mu_{eff} = \mu_0 \cdot \left(\frac{E_{crit}}{E_{eff}}\right)^{UEXP}$$

where $E_{crit}$ is the critical field (UCRIT) and $UEXP$ is the degradation exponent.

### Key Level 2 parameters

| Parameter | Symbol | Effect |
|-----------|--------|--------|
| VMAX | $v_{max}$ | Maximum carrier velocity -- sets velocity saturation |
| NEFF | $n$ | Subthreshold slope factor |
| UCRIT | $E_{crit}$ | Critical field for mobility degradation |
| UEXP | | Mobility degradation exponent |
| DELTA | $\delta$ | Narrow-channel width correction |
| XJ | $X_j$ | Junction depth (for short-channel charge sharing) |

## Level 3: semi-empirical simplification

Level 3 addresses the same physical effects as Level 2 but uses simpler, semi-empirical formulas that are computationally cheaper and often fit measured data better.

The philosophy shift is significant: Level 2 tries to derive current from device physics (doping, geometry, field equations). Level 3 uses empirical factors calibrated to measurements.

### Key differences from Level 2

**Simplified saturation voltage:**

$$V_{DSAT} = \frac{V_{GS} - V_{TH}}{1 + FB}$$

where $FB$ is a factor that accounts for short-channel and narrow-channel effects. This is simpler than Level 2's iterative velocity saturation calculation.

**Empirical mobility model:**

$$\mu_{eff} = \frac{\mu_0}{1 + \theta (V_{GS} - V_{TH})}$$

where $\theta$ (THETA) is a single mobility degradation parameter. One parameter instead of Level 2's two.

**Narrow-channel effects (DELTA, KAPPA):**

Level 3 explicitly models the threshold shift due to narrow channel width and the short-channel charge-sharing effect, using DELTA for width effects and KAPPA for output conductance correction.

### Key Level 3 parameters

| Parameter | Symbol | Effect |
|-----------|--------|--------|
| THETA | $\theta$ | Mobility degradation (single-parameter) |
| ETA | $\eta$ | DIBL coefficient (drain-induced barrier lowering) |
| KAPPA | $\kappa$ | Saturation field factor |
| DELTA | $\delta$ | Narrow-channel width correction |

## Level 2 vs Level 3: when to use which

Neither Level 2 nor Level 3 is used much in modern design. They occupy a historical middle ground:

```text
  Simplicity  ←─────────────────────────────→  Accuracy
  
  Level 1        Level 3     Level 2        BSIM3     BSIM4
  (textbook)   (empirical) (physics)    (industry)  (deep sub-μm)
  
  4 params      ~20 params  ~20 params   150+ params  200+ params
  >2μm          0.5-2μm     0.5-2μm      <0.5μm       <0.1μm
```

**Level 2** is useful when you want to understand the physics of short-channel effects -- velocity saturation, subthreshold conduction, mobility degradation. The equations map directly to physical mechanisms.

**Level 3** is useful when you want a compact model with reasonable accuracy for moderate channel lengths. It converges more reliably than Level 2 because its equations are smoother.

**In practice**, foundries supply BSIM3 or BSIM4 model cards. Level 2 and 3 exist in spice-rs for compatibility with legacy netlists and for educational value.

## In spice-rs

Level 2 is implemented in `device/mosfet2.rs` (~800 lines) and Level 3 in `device/mosfet3.rs` (~700 lines). Both follow the same load-function structure as Level 1:

1. Compute effective mobility
2. Determine saturation voltage
3. Select region and compute $I_{DS}$
4. Compute $g_m$, $g_{ds}$, $g_{mbs}$
5. Compute Meyer capacitances (same model as Level 1)
6. Stamp into MNA matrix

The additional physics adds computation inside steps 1-4 but does not change the overall architecture. This is a recurring theme: more complex models add physics *inside* the device evaluation, but the interface to the simulator matrix remains the same.
