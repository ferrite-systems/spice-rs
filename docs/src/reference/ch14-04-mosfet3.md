# MOSFET Level 3 (Semi-empirical)

Model type: `NMOS` or `PMOS` with `LEVEL=3`

```spice
.MODEL NMOS3 NMOS (LEVEL=3 VTO=0.7 THETA=0.1 ETA=0.1 KAPPA=0.5)
```

Level 3 uses a semi-empirical approach to model short-channel effects. It is simpler to extract parameters for than Level 2 while handling similar phenomena. It inherits all Level 1 parameters and adds the following.

## Additional parameters

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| THETA | 0     | 1/V    | Mobility modulation |
| ETA   | 0     | --     | Static feedback (DIBL effect) |
| KAPPA | 0.2   | --     | Saturation field factor |
| VMAX  | 0     | m/s    | Maximum carrier drift velocity |
| NFS   | 0     | 1/cm^2 | Fast surface state density |
| XJ    | 0     | m      | Metallurgical junction depth |
| DELTA | 0     | --     | Width effect on threshold voltage |
| INPUT | --    | --     | Gate material (same as TPG in Level 1) |

## Key differences from Level 1

- **Mobility degradation**: Modeled empirically via `THETA` (vertical field effect on mobility).
- **DIBL**: Drain-induced barrier lowering through `ETA` parameter, which reduces threshold voltage at high drain bias.
- **Velocity saturation**: `VMAX` limits the current when carriers reach maximum drift velocity, with `KAPPA` controlling the saturation voltage calculation.
- **Narrow-channel effect**: `DELTA` provides threshold adjustment for narrow geometries.
- **Subthreshold conduction**: `NFS` enables weak-inversion current.

## When to use Level 3

Level 3 is a reasonable choice for technology nodes from about 1um down to 0.35um. For geometries below 0.25um, BSIM3v3 (Level 8) is recommended.
