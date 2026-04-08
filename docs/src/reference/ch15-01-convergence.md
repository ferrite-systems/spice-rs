# Convergence Options

These options control the Newton-Raphson iteration loop used to solve nonlinear circuit equations at each operating point or timestep.

## Tolerances

| Option | Default | Unit | Description |
|--------|---------|------|-------------|
| ABSTOL | 1e-12 | A    | Absolute current tolerance. NR iteration converges when all branch current changes are below this value. |
| RELTOL | 1e-3  | --   | Relative tolerance. NR converges when all variable changes are below `RELTOL * max(|Vnew|, |Vold|) + VNTOL`. |
| VNTOL  | 1e-6  | V    | Absolute voltage tolerance. Combined with RELTOL to determine voltage convergence. |
| CHGTOL | 1e-14 | C    | Absolute charge tolerance. Used in transient analysis for charge-conservation convergence. |

## Iteration limits

| Option | Default | Description |
|--------|---------|-------------|
| ITL1   | 100    | Maximum iterations for DC operating point. If NR does not converge in this many iterations, the simulator applies GMIN stepping or source stepping. |
| ITL2   | 50     | Maximum iterations for each DC sweep point. |
| ITL4   | 10     | Maximum iterations per transient timepoint. If NR does not converge, the timestep is reduced and the step is retried. |

## GMIN

| Option | Default | Unit | Description |
|--------|---------|------|-------------|
| GMIN   | 1e-12 | S (mho) | Minimum conductance added to every node. Prevents singular matrices from floating nodes. Also used in GMIN stepping for DC convergence. |

## Convergence algorithm

The Newton-Raphson loop at each point checks two conditions:

1. **Voltage convergence**: for each node voltage V,
   ```
   |Vnew - Vold| < RELTOL * max(|Vnew|, |Vold|) + VNTOL
   ```

2. **Current convergence**: for each branch current I,
   ```
   |Inew - Iold| < ABSTOL + RELTOL * max(|Inew|, |Iold|)
   ```

Both conditions must be satisfied for convergence.

When DC operating point fails to converge within `ITL1` iterations, spice-rs applies:

1. **GMIN stepping**: progressively reduces the GMIN conductance from a large value down to the user-specified GMIN
2. **Source stepping**: if GMIN stepping fails, ramps all sources from 0 to their final values
