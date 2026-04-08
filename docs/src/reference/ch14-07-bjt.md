# BJT Model Parameters (Gummel-Poon)

Model type: `NPN` or `PNP`

```spice
.MODEL NPN1 NPN (IS=1e-15 BF=200 NF=1 VAF=100 IKF=0.04
+ ISE=1e-13 NE=1.5 BR=5 NR=1 VAR=20
+ RB=100 RE=1 RC=10
+ CJE=2P VJE=0.7 MJE=0.33
+ CJC=1P VJC=0.75 MJC=0.33
+ TF=0.3N TR=6N)
```

## Forward DC parameters

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| IS  | 1e-16 | A    | Transport saturation current |
| BF  | 100   | --   | Ideal maximum forward beta |
| NF  | 1     | --   | Forward current emission coefficient |
| VAF | inf   | V    | Forward Early voltage |
| IKF | inf   | A    | Corner for forward beta high-current roll-off |
| ISE | 0     | A    | Base-emitter leakage saturation current |
| NE  | 1.5   | --   | Base-emitter leakage emission coefficient |

## Reverse DC parameters

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| BR  | 1     | --   | Ideal maximum reverse beta |
| NR  | 1     | --   | Reverse current emission coefficient |
| VAR | inf   | V    | Reverse Early voltage |
| IKR | inf   | A    | Corner for reverse beta high-current roll-off |
| ISC | 0     | A    | Base-collector leakage saturation current |
| NC  | 2     | --   | Base-collector leakage emission coefficient |

## Resistance

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| RB  | 0   | ohm  | Zero-bias base resistance |
| RBM | RB  | ohm  | Minimum base resistance at high currents |
| IRB | inf | A    | Current where base resistance falls halfway to RBM |
| RE  | 0   | ohm  | Emitter resistance |
| RC  | 0   | ohm  | Collector resistance |

## Base-emitter capacitance

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| CJE | 0   | F    | Zero-bias base-emitter depletion capacitance |
| VJE | 0.75| V    | Base-emitter built-in potential |
| MJE | 0.33| --   | Base-emitter grading coefficient |
| TF  | 0   | s    | Ideal forward transit time |
| XTF | 0   | --   | Transit time bias dependence coefficient |
| VTF | inf | V    | Transit time dependency on Vbc |
| ITF | 0   | A    | Transit time dependency on Ic |
| PTF | 0   | deg  | Excess phase at 1/(2*pi*TF) Hz |

## Base-collector capacitance

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| CJC | 0   | F    | Zero-bias base-collector depletion capacitance |
| VJC | 0.75| V    | Base-collector built-in potential |
| MJC | 0.33| --   | Base-collector grading coefficient |
| XCJC| 1   | --   | Fraction of Cbc connected to internal base |
| TR  | 0   | s    | Ideal reverse transit time |

## Collector-substrate capacitance

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| CJS | 0   | F    | Zero-bias collector-substrate capacitance |
| VJS | 0.75| V    | Substrate junction built-in potential |
| MJS | 0   | --   | Substrate junction grading coefficient |

## Temperature

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| EG  | 1.11| eV   | Bandgap energy (silicon) |
| XTI | 3   | --   | Temperature exponent for IS |
| XTB | 0   | --   | Forward and reverse beta temperature exponent |

## Noise

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| KF  | 0   | --   | Flicker noise coefficient |
| AF  | 1   | --   | Flicker noise exponent |

## Gummel-Poon equations

The Gummel-Poon model extends the Ebers-Moll model with:

- **Base-width modulation** (Early effect) via `VAF` and `VAR`
- **High-injection roll-off** via `IKF` and `IKR`
- **Low-current non-ideal base current** via `ISE`/`NE` and `ISC`/`NC`

Collector current:

```
Ic = IS/qb * (exp(Vbe/(NF*Vt)) - exp(Vbc/(NR*Vt)))
```

where `qb` is the normalized base charge accounting for Early effect and high-injection:

```
q1 = 1/(1 - Vbc/VAF - Vbe/VAR)
q2 = IS*(exp(Vbe/(NF*Vt))/IKF + exp(Vbc/(NR*Vt))/IKR)
qb = q1/2 * (1 + sqrt(1 + 4*q2))
```
