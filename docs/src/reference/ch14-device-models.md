# Device Model Reference

Device models are defined with the `.MODEL` statement and assign electrical parameters to device instances.

```
.MODEL name type (param=val ...)
```

Parameters not specified use their default values. All models support temperature scaling -- the simulator adjusts parameters from the nominal temperature (`TNOM`, default 27C) to the circuit temperature (`TEMP`).

## MOSFET level selection

MOSFET models use the `LEVEL` parameter to select the model equations:

| Level | Model | Typical use |
|-------|-------|-------------|
| 1     | Shichman-Hodges | Hand calculations, simple circuits |
| 2     | Grove-Frohman | Short-channel effects |
| 3     | Semi-empirical | Moderate accuracy |
| 8     | BSIM3v3 | Foundry-provided models |
| 14    | BSIM4 | Advanced foundry models |

```spice
.MODEL NMOS1 NMOS (LEVEL=1 VTO=0.7 KP=110U)
.MODEL NMOS8 NMOS (LEVEL=8 VERSION=3.3 VTH0=0.5 TOX=9e-9)
```

## In this chapter

- [Diode](ch14-01-diode.md) -- PN junction diode parameters
- [MOSFET Level 1](ch14-02-mosfet1.md) -- Shichman-Hodges model
- [MOSFET Level 2](ch14-03-mosfet2.md) -- Grove-Frohman extensions
- [MOSFET Level 3](ch14-04-mosfet3.md) -- Semi-empirical model
- [BSIM3v3](ch14-05-bsim3.md) -- Berkeley short-channel model
- [BSIM4](ch14-06-bsim4.md) -- Advanced short-channel model
- [BJT](ch14-07-bjt.md) -- Gummel-Poon bipolar transistor
- [JFET](ch14-08-jfet.md) -- Junction field-effect transistor
