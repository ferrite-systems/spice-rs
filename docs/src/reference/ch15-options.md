# Simulation Options

The `.OPTIONS` statement controls simulation accuracy, convergence behavior, and general parameters.

```spice
.OPTIONS RELTOL=1e-4 ABSTOL=1e-14 TEMP=85
```

Options can appear on one line or across multiple `.OPTIONS` statements. When the same option is set more than once, the last value wins.

## In this chapter

- [Convergence Options](ch15-01-convergence.md) -- tolerances and iteration limits for the Newton-Raphson solver
- [Transient Options](ch15-02-transient.md) -- integration method, order, and timestep control
- [General Options](ch15-03-general.md) -- temperature, pivoting, and miscellaneous settings
