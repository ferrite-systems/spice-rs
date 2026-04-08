# BSIM4 Model Parameters

Model type: `NMOS` or `PMOS` with `LEVEL=14`

```spice
.MODEL NMOS14 NMOS (LEVEL=14 VERSION=4.5 TNOM=27
+ VTH0=0.4 TOX=1.8e-9 U0=300 VSAT=1.2e5
+ RDSW=200 RDSWMIN=0)
```

BSIM4 extends BSIM3v3 with 200+ additional parameters for deep sub-micron and nanoscale MOSFETs. It is the standard model for technology nodes from 130nm down to 22nm. Foundries provide complete BSIM4 model cards in their PDKs.

## Key additions over BSIM3v3

### Gate tunneling current

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| AIGBACC | 1.36e-2 | --  | Accumulation gate current parameter A |
| BIGBACC | 1.71e-3 | --  | Accumulation gate current parameter B |
| CIGBACC | 0.075  | --   | Accumulation gate current parameter C |
| AIGBINV | 1.11e-2 | --  | Inversion gate current parameter A |
| BIGBINV | 9.49e-4 | --  | Inversion gate current parameter B |
| CIGBINV | 0.006  | --   | Inversion gate current parameter C |
| AIGC    | 1.36e-2 | --  | Gate-to-channel tunneling current parameter A |
| BIGC    | 1.71e-3 | --  | Gate-to-channel tunneling current parameter B |
| CIGC    | 0.075  | --   | Gate-to-channel tunneling current parameter C |
| TOXREF  | 3e-9   | m    | Nominal gate oxide thickness for tunneling |

### Source/drain resistance

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| RDSW    | 200  | ohm-um | Source/drain resistance per unit width |
| RDSWMIN | 0    | ohm-um | Minimum RDSW |
| PRWB    | 0    | 1/V^0.5 | Body effect on RDSW |
| PRWGS   | 0    | 1/V    | Gate bias effect on RDSW |
| WR      | 1    | --     | Width offset from Weff for Rds |

### Gate-induced drain leakage (GIDL)

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| AGIDL   | 0    | A/V    | GIDL pre-exponential coefficient |
| BGIDL   | 2.3e9 | V/m  | GIDL exponential coefficient |
| CGIDL   | 0.5  | V      | GIDL reference voltage |
| EGIDL   | 0.8  | V      | GIDL activation energy |

### Stress effects (STI / LOD)

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| SAREF   | 1e-6 | m      | Reference distance SA |
| SBREF   | 1e-6 | m      | Reference distance SB |
| KU0     | 0    | --     | Mobility stress coefficient |
| KVTH0   | 0    | V      | Threshold stress coefficient |

### New capacitance model

BSIM4 introduces `CAPMOD=2` (charge-based model) as the default, replacing the older BSIM3 capacitance formulations:

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| NGATE   | 0    | 1/cm^3 | Poly-gate doping concentration |
| VFBCV   | -1   | V      | Flat-band voltage for C-V |
| ACDE    | 1    | --     | Exponential coefficient for charge thickness |
| MOIN    | 15   | --     | Coefficient for gate-bias dependent surface potential |

## Usage notes

- BSIM4 model cards from foundries are typically 200-500 lines. Do not hand-edit them.
- The `VERSION` parameter should match the version used during parameter extraction (4.0 through 4.8).
- All BSIM3v3 geometry-scaling conventions (`L`, `W`, `P` prefixed variants) are retained.
- spice-rs supports BSIM4 versions 4.0 through 4.8.
