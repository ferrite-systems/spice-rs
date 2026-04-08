# General Options

## Temperature

| Option | Default | Unit | Description |
|--------|---------|------|-------------|
| TEMP   | 27    | C    | Circuit simulation temperature. All device models are evaluated at this temperature. |
| TNOM   | 27    | C    | Nominal temperature at which model parameters were measured. Device models use TNOM as the reference point for temperature scaling. |

Temperature affects:

- Thermal voltage: `Vt = k * (TEMP + 273.15) / q`
- Junction saturation currents (IS, ISE, ISC)
- Mobility (U0)
- Threshold voltage (VTO, VTH0)
- Resistance (RD, RS, RE, RC, RB)

## Pivot tolerances

| Option | Default | Unit | Description |
|--------|---------|------|-------------|
| PIVTOL | 1e-13 | --   | Absolute minimum pivot value for the sparse matrix solver. Elements smaller than this are treated as zero during factorization. |
| PIVREL | 1e-3  | --   | Relative pivot tolerance. During factorization, a pivot is accepted if it exceeds `PIVREL * max_element_in_column`. Smaller values allow more fill-in but may improve accuracy. |

## Other

| Option | Default | Description |
|--------|---------|-------------|
| NOOPITER | off  | Skip the DC operating point iteration (use initial conditions directly). Rarely used. |
| KEEPOPINFO | off | Retain the operating point information in memory for post-analysis queries. |
