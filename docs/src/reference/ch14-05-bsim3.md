# BSIM3v3 Model Parameters

Model type: `NMOS` or `PMOS` with `LEVEL=8`

```spice
.MODEL NMOS8 NMOS (LEVEL=8 VERSION=3.3 TNOM=27
+ VTH0=0.5 K1=0.6 K2=-0.1
+ TOX=9e-9 U0=300 VSAT=1.5e5
+ CGSO=2.5e-10 CGDO=2.5e-10)
```

BSIM3v3 (Berkeley Short-Channel IGFET Model, version 3.3) is the industry-standard model for sub-micron MOSFETs. It has 150+ parameters, typically extracted by foundries and provided in process design kits.

Parameters are organized below by functional group.

## Model selection

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| VERSION | 3.3  | --   | Model version |
| TNOM    | 27   | C    | Parameter extraction temperature |
| TOX     | 1.5e-8 | m  | Gate oxide thickness |
| TOXE    | 1.5e-8 | m  | Electrical oxide thickness |
| DTOX    | 0    | m    | TOX - TOXE |
| EPSROX  | 3.9  | --   | Gate oxide dielectric constant |
| WINT    | 0    | m    | Channel width offset |
| LINT    | 0    | m    | Channel length offset |

## Threshold voltage

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| VTH0 (VTHO) | 0.7 | V  | Long-channel threshold voltage at Vbs=0 |
| K1      | 0.5  | V^0.5 | First-order body effect coefficient |
| K2      | 0    | --     | Second-order body effect coefficient |
| K3      | 80   | --     | Narrow width effect coefficient |
| K3B     | 0    | 1/V    | Body effect of narrow width |
| DVT0    | 2.2  | --     | Short-channel effect coefficient 0 |
| DVT1    | 0.53 | --     | Short-channel effect coefficient 1 |
| DVT2    | -0.032 | 1/V  | Short-channel effect coefficient 2 |
| DVT0W   | 0    | --     | Narrow width effect on Vth, coefficient 0 |
| DVT1W   | 5.3e6 | 1/m  | Narrow width effect on Vth, coefficient 1 |
| DVT2W   | -0.032 | 1/V  | Narrow width effect on Vth, coefficient 2 |
| NLXL    | 1.74e-7 | m  | Lateral non-uniform doping length |
| W0      | 0    | m      | Narrow width effect parameter |
| VFB     | -1   | V      | Flat-band voltage |

## Mobility

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| U0      | 670/250 | cm^2/V-s | Low-field mobility (NMOS/PMOS) |
| UA      | 2.25e-9 | m/V  | First-order mobility degradation coefficient |
| UB      | 5.87e-19 | (m/V)^2 | Second-order mobility degradation coefficient |
| UC      | -4.65e-11 | 1/V | Body-bias sensitivity of mobility degradation |

## Drain saturation current

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| VSAT    | 8e4  | m/s    | Saturation velocity |
| A0      | 1    | --     | Non-uniform depletion width effect coefficient |
| AGS     | 0.2  | 1/V    | Gate bias coefficient of Abulk |
| B0      | 0    | m      | Non-uniform depletion width, bulk charge effect |
| B1      | 0    | m      | Non-uniform depletion width, bulk charge effect |
| A1      | 0    | 1/V    | Non-saturation effect coefficient |
| A2      | 1    | --     | Non-saturation effect coefficient |

## Subthreshold region

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| VOFF    | -0.11 | V    | Subthreshold offset voltage |
| NFACTOR | 1    | --     | Subthreshold swing factor |
| CIT     | 0    | F/m^2  | Interface trap capacitance |
| CDSC    | 2.4e-4 | F/m^2 | Drain/source to channel coupling capacitance |

## DIBL (Drain-Induced Barrier Lowering)

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| ETA0    | 0.08 | --     | DIBL coefficient in subthreshold |
| ETAB    | -0.07 | 1/V  | Body-bias coefficient for DIBL |
| DSUB    | 0.56 | --     | DIBL coefficient in strong inversion |

## Output conductance (Rout)

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| PCLM    | 1.3  | --     | Channel-length modulation parameter |
| PDIBLC1 | 0.39 | --     | First DIBL correction parameter |
| PDIBLC2 | 0.0086 | --   | Second DIBL correction parameter |
| PDIBLCB | 0    | 1/V    | Body effect on DIBL correction |
| DROUT   | 0.56 | --     | Length dependence of DIBL correction on Rout |
| PSCBE1  | 4.24e8 | V/m  | Substrate current induced body effect parameter 1 |
| PSCBE2  | 1e-5 | V/m    | Substrate current induced body effect parameter 2 |
| PVAG    | 0    | --     | Gate dependence of output resistance |

## Capacitance

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| CGSO    | 0    | F/m    | Gate-source overlap capacitance per width |
| CGDO    | 0    | F/m    | Gate-drain overlap capacitance per width |
| CGBO    | 0    | F/m    | Gate-bulk overlap capacitance per length |
| CLC     | 1e-7 | m      | Constant term for the short-channel model |
| CLE     | 0.6  | --     | Exponential term for the short-channel model |
| CF      | 0    | F/m    | Fringing field capacitance |
| CKAPPA  | 0.6  | --     | Coefficient for lightly doped region overlap cap |
| DLC     | 0    | m      | Length offset for capacitance |
| DWC     | 0    | m      | Width offset for capacitance |

## Junction diode

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| CJ      | 5e-4 | F/m^2  | Bottom junction capacitance per area |
| CJSW    | 5e-10 | F/m   | Sidewall junction capacitance per perimeter |
| CJSWG   | 0    | F/m    | Gate-side sidewall junction capacitance |
| MJ      | 0.5  | --     | Bottom junction grading coefficient |
| MJSW    | 0.33 | --     | Sidewall junction grading coefficient |
| MJSWG   | 0.33 | --     | Gate-side sidewall grading coefficient |
| PB      | 1    | V      | Bottom junction built-in potential |
| PBSW    | 1    | V      | Sidewall junction built-in potential |
| PBSWG   | 1    | V      | Gate-side sidewall built-in potential |
| JS      | 1e-4 | A/m^2  | Bulk junction saturation current density |

## Geometry scaling

BSIM3v3 supports automatic length/width scaling. Each parameter `P` can have associated `LP`, `WP`, and `PP` variants:

```
P_eff = P + LP/Leff + WP/Weff + PP/(Leff * Weff)
```

For example: `VTH0`, `LVTH0`, `WVTH0`, `PVTH0`.
