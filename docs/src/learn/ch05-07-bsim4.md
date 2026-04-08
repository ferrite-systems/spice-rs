# BSIM4

BSIM4 is the successor to BSIM3v3, extending the model to deep sub-micron technology nodes (90 nm and below) and beyond. With 200+ parameters and approximately 5000 lines of code in spice-rs, it is the most complex device model in the simulator.

This is a brief overview chapter. BSIM4 is not a conceptual departure from BSIM3 -- it adds corrections and new physics on top of the same architecture.

## What BSIM4 adds

The key extensions over BSIM3 address effects that become significant below 100 nm:

### Gate leakage current

Below approximately 2 nm oxide thickness, quantum mechanical tunneling allows current to flow directly through the gate oxide. BSIM3 assumes zero gate current. BSIM4 adds gate tunneling models for three paths:

- Gate to channel ($I_{GSidl}$)
- Gate to source overlap ($I_{GS}$)
- Gate to drain overlap ($I_{GD}$)

This gate current is small per transistor but adds up across billions of devices, becoming a dominant source of standby power.

### Stress effects

Mechanical stress in the silicon lattice changes carrier mobility. Modern processes intentionally apply stress (e.g., SiGe in source/drain for PMOS) to boost performance. BSIM4 includes a stress model that adjusts mobility, threshold voltage, and velocity saturation based on layout-dependent stress parameters.

### New noise models

BSIM4 provides improved flicker noise models (holistic noise model) and adds induced gate noise for RF applications. The noise parameters are calibrated to measured data from the fabrication process.

### FinFET support

Starting with BSIM4 version 4.8, the model supports non-planar transistor geometries (FinFET). The effective width becomes a function of the fin height and number of fins rather than a simple drawn width.

### Asymmetric source/drain

In advanced processes, the source and drain are not identical. BSIM4 supports asymmetric modeling of source-side and drain-side parameters (different resistance, different overlap capacitance).

## Parameter organization

BSIM4's parameters are organized similarly to BSIM3, with additional groups:

| Group | New in BSIM4 | Physics |
|-------|-------------|---------|
| Gate current | AIGBACC, BIGBACC, ... | Oxide tunneling |
| Stress | SA, SB, SD, SAREF | Layout-dependent stress |
| Noise | TNOIA, TNOIB, NTNOI | Improved flicker noise |
| Geometry | NF, MIN, GEOMOD | Multi-finger, multi-fin |
| Gate resistance | RSHG, DMCGT | Distributed gate RC |
| Well proximity | SCA, SCB, SCC | Well proximity effects |

## BSIM3 vs BSIM4: when to use which

The choice is usually made by the foundry, not the designer. The model card that comes with a technology node determines which model the simulator uses:

```text
  Technology node:   180nm    130nm    90nm     65nm     45nm and below
                      |        |        |        |        |
  BSIM3            ████████████████
  BSIM4                         ████████████████████████████
                                  ^
                                  |
                           overlap region:
                           foundry chooses
```

Most foundries at 90 nm and below provide BSIM4 exclusively. Some legacy IP at 130-180 nm may use BSIM3 model cards.

## In spice-rs

The BSIM4 implementation lives in `device/bsim4.rs` at approximately 5000 lines -- the largest single file in the codebase. It follows the same structure as BSIM3:

1. Compute effective geometry
2. Compute threshold voltage
3. Compute mobility and current
4. Compute transconductances
5. Compute charges and capacitances
6. Compute gate leakage (new)
7. Stamp into MNA matrix

The porting strategy for BSIM4 in spice-rs is direct translation from the reference C code. The model is too complex for independent re-derivation -- the only reliable approach is line-by-line translation with numerical verification against ngspice.

For most users, BSIM4 is a black box: the foundry provides the model card, the simulator computes the physics. The value of understanding its structure is in debugging convergence issues and interpreting simulation results when they seem unexpected -- knowing which parameter group to investigate when a device does not behave as expected.
