# MOSFET Level 2 (Grove-Frohman)

Model type: `NMOS` or `PMOS` with `LEVEL=2`

```spice
.MODEL NMOS2 NMOS (LEVEL=2 VTO=0.7 KP=110U UCRIT=1e4 UEXP=0.1)
```

Level 2 extends Level 1 with analytical models for short-channel and narrow-channel effects. It inherits all Level 1 parameters and adds the following.

## Additional parameters

| Parameter | Default | Unit | Description |
|-----------|---------|------|-------------|
| NFS   | 0     | 1/cm^2 | Fast surface state density |
| UCRIT | 1e4   | V/cm   | Critical field for mobility degradation |
| UEXP  | 0     | --     | Critical field exponent |
| UTRA  | 0     | --     | Transverse field coefficient |
| VMAX  | 0     | m/s    | Maximum carrier drift velocity |
| NEFF  | 1     | --     | Total channel charge coefficient |
| XJ    | 0     | m      | Metallurgical junction depth |
| DELTA | 0     | --     | Width effect on threshold voltage |

## Key differences from Level 1

- **Mobility degradation**: Surface mobility decreases with lateral electric field via `UCRIT` and `UEXP`.
- **Velocity saturation**: When `VMAX` is set, drain current saturates when carriers reach maximum drift velocity rather than by pinch-off alone.
- **Narrow-channel effect**: `DELTA` adjusts the threshold voltage for narrow devices.
- **Subthreshold conduction**: `NFS` enables weak-inversion current below threshold.
- **Channel-length modulation**: Computed from depletion width rather than the empirical `LAMBDA` parameter (though `LAMBDA` is still accepted as an override).
