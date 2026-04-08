# JFET Model Parameters

Model type: `NJF` (N-channel) or `PJF` (P-channel)

```spice
.MODEL JMOD NJF (VTO=-2 BETA=1e-4 LAMBDA=2e-4 RD=10 RS=10 CGS=5P CGD=1P)
```

## DC parameters

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| VTO  | -2    | V     | Pinch-off voltage (negative for NJF, positive for PJF) |
| BETA | 1e-4  | A/V^2 | Transconductance coefficient |
| LAMBDA | 0   | 1/V   | Channel-length modulation |
| IS   | 1e-14 | A     | Gate junction saturation current |
| N    | 1     | --    | Gate junction emission coefficient |
| B    | 1     | --    | Doping tail parameter |

## Resistance

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| RD   | 0     | ohm   | Drain ohmic resistance |
| RS   | 0     | ohm   | Source ohmic resistance |

## Capacitance

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| CGS  | 0     | F     | Zero-bias gate-source junction capacitance |
| CGD  | 0     | F     | Zero-bias gate-drain junction capacitance |
| PB   | 1     | V     | Gate junction potential |
| FC   | 0.5   | --    | Forward-bias depletion capacitance coefficient |

## Noise

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| KF   | 0     | --    | Flicker noise coefficient |
| AF   | 1     | --    | Flicker noise exponent |

## Equations

Drain current (NJF, Vgs > VTO):

```
Linear (Vds < Vgs - VTO):
  Ids = BETA * (2*(Vgs - VTO)*Vds - Vds^2) * (1 + LAMBDA*Vds)

Saturation (Vds >= Vgs - VTO):
  Ids = BETA * (Vgs - VTO)^2 * (1 + LAMBDA*Vds)
```
