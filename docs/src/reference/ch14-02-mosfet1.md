# MOSFET Level 1 (Shichman-Hodges)

Model type: `NMOS` or `PMOS` with `LEVEL=1`

```spice
.MODEL NMOS1 NMOS (LEVEL=1 VTO=0.7 KP=110U GAMMA=0.4 LAMBDA=0.04)
```

## Threshold and transconductance

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| VTO    | 0      | V      | Zero-bias threshold voltage |
| KP     | 2e-5   | A/V^2  | Transconductance parameter |
| GAMMA  | 0      | V^0.5  | Body-effect parameter |
| PHI    | 0.6    | V      | Surface potential |
| LAMBDA | 0      | 1/V    | Channel-length modulation |

## Resistance

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| RD  | 0 | ohm | Drain ohmic resistance |
| RS  | 0 | ohm | Source ohmic resistance |

## Junction capacitance

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| CBD  | 0    | F     | Zero-bias bulk-drain junction capacitance |
| CBS  | 0    | F     | Zero-bias bulk-source junction capacitance |
| IS   | 1e-14| A     | Bulk junction saturation current |
| PB   | 0.8  | V     | Bulk junction potential |
| CGSO | 0    | F/m   | Gate-source overlap capacitance per unit channel width |
| CGDO | 0    | F/m   | Gate-drain overlap capacitance per unit channel width |
| CGBO | 0    | F/m   | Gate-bulk overlap capacitance per unit channel length |
| CJ   | 0    | F/m^2 | Bottom junction capacitance per unit area |
| CJSW | 0    | F/m   | Sidewall junction capacitance per unit periphery |
| MJ   | 0.5  | --    | Bottom grading coefficient |
| MJSW | 0.33 | --    | Sidewall grading coefficient |
| FC   | 0.5  | --    | Forward-bias capacitance coefficient |

## Geometry and process

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| TOX  | 1e-7  | m      | Oxide thickness |
| LD   | 0     | m      | Lateral diffusion |
| U0   | 600   | cm^2/V-s | Low-field surface mobility |
| NSUB | 0     | 1/cm^3 | Substrate doping |
| TPG  | 1     | --     | Gate material type (+1 opposite, -1 same, 0 Al gate) |
| NSS  | 0     | 1/cm^2 | Surface state density |

## Temperature

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| TNOM | 27 | C | Parameter measurement temperature |

## Noise

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| KF | 0 | -- | Flicker noise coefficient |
| AF | 1 | -- | Flicker noise exponent |

## Equations

Threshold voltage with body effect:

```
Vth = VTO + GAMMA * (sqrt(PHI - Vbs) - sqrt(PHI))
```

Drain current (Vgs > Vth):

```
Linear:     Ids = KP * W/L * ((Vgs - Vth)*Vds - 0.5*Vds^2) * (1 + LAMBDA*Vds)
Saturation: Ids = 0.5 * KP * W/L * (Vgs - Vth)^2 * (1 + LAMBDA*Vds)
```
