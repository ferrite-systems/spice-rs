# Transient Options

These options control the numerical integration method and timestep selection during `.TRAN` analysis.

## Integration method

| Option | Default | Values | Description |
|--------|---------|--------|-------------|
| METHOD | TRAP | TRAP, GEAR | Integration method. TRAP (trapezoidal) is second-order and the default. GEAR (Gear/BDF) is available at orders 1-6. |
| MAXORD | 2    | 1-6    | Maximum order for Gear integration. Ignored when METHOD=TRAP. Higher orders allow larger timesteps but may introduce ringing on sharp transitions. |

## Timestep control

| Option | Default | Unit | Description |
|--------|---------|------|-------------|
| TRTOL  | 7     | --   | Transient error tolerance factor. Multiplies the LTE estimate to determine whether a timestep is accepted. Higher values allow larger timesteps (less accurate). Lower values force smaller steps (more accurate). |
| CHGTOL | 1e-14 | C    | Charge tolerance for LTE-based timestep control. |

## How timestep control works

At each transient timepoint, the simulator:

1. Solves the NR equations at the trial timestep
2. Estimates the local truncation error (LTE) of each state variable
3. Compares LTE against `TRTOL * (ABSTOL + RELTOL * |value|)`
4. If LTE is too large, rejects the step and retries with a smaller timestep
5. If LTE is small enough, accepts the step and may increase the timestep for the next point

The maximum internal timestep is bounded by the `.TRAN` `tmax` parameter (default: tstop/50).

## Trapezoidal vs Gear

| Property | Trapezoidal | Gear (order 2) |
|----------|-------------|-----------------|
| Accuracy | Second-order | Second-order |
| Stability | A-stable | A-stable |
| Ringing | Can produce numerical ringing on stiff systems | No ringing |
| Damping | No numerical damping | Some numerical damping |
| Typical use | General purpose | Switching circuits, stiff systems |

For circuits with sharp switching edges (digital logic, power converters), `METHOD=GEAR` with `MAXORD=2` can avoid spurious oscillations that trapezoidal integration may produce.
