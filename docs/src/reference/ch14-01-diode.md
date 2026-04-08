# Diode Model Parameters

Model type: `D`

```spice
.MODEL DMOD D (IS=1e-14 N=1.05 RS=10 CJO=2P VJ=0.7 BV=100)
```

## DC parameters

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| IS  | 1e-14 | A     | Saturation current |
| N   | 1     | --    | Emission coefficient |
| RS  | 0     | ohm   | Series resistance |
| BV  | inf   | V     | Reverse breakdown voltage |
| IBV | 1e-3  | A     | Current at reverse breakdown voltage |

## Capacitance parameters

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| CJO | 0    | F     | Zero-bias junction capacitance |
| VJ  | 1    | V     | Junction potential |
| M   | 0.5  | --    | Grading coefficient |
| TT  | 0    | s     | Transit time (diffusion capacitance) |
| FC  | 0.5  | --    | Forward-bias depletion capacitance coefficient |

## Temperature parameters

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| EG  | 1.11 | eV    | Bandgap energy (silicon) |
| XTI | 3    | --    | Saturation current temperature exponent |

## Noise parameters

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| KF  | 0    | --    | Flicker noise coefficient |
| AF  | 1    | --    | Flicker noise exponent |

## Equations

The diode current is:

```
I = IS * (exp(V / (N * Vt)) - 1)
```

where `Vt = kT/q` is the thermal voltage (approximately 26mV at 27C).

Junction capacitance:

```
Cj = CJO / (1 - V/VJ)^M          for V < FC * VJ
Cj = CJO / (1 - FC)^(1+M) * ...   for V >= FC * VJ
```

Diffusion capacitance:

```
Cd = TT * dI/dV
```
