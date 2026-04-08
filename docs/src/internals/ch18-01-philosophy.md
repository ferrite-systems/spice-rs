# Philosophy: Port, Don't Approximate

The core principle of the spice-rs port is a single rule:

> Read the actual ngspice C code. Follow the same logic. Don't invent alternative approaches. If your implementation doesn't match ngspice, go back and read more C. The answer is always in the reference source.

## Why this matters

SPICE is not a textbook algorithm. It is a textbook algorithm plus 50 years of patches, fixes, workarounds, and numerical tricks accumulated by the Berkeley team, the ngspice maintainers, and the broader SPICE community. The published papers describe the theory. The code describes reality.

Examples of things that exist in the code but not in the papers:

- **Voltage limiting** in device models. The Shichman-Hodges MOSFET equations are smooth, but Newton-Raphson will overshoot wildly on a junction turn-on without the `DEVfetlim` / `DEVpnjlim` voltage limiters. These limiters clamp the voltage change per iteration using device-specific heuristics. They are not part of the MOSFET model — they are convergence aids buried in the load function.

- **The ipass mechanism.** When `.NODESET` is used, ngspice runs the first NR convergence with diagonal elements forcing nodes to their set values (`MODEINITFIX`). After convergence, it flips to `MODEINITFLOAT`, removes the forcing, and runs one more iteration. This is not documented anywhere obvious — it is in `niiter.c:402-407`.

- **Source stepping fallback.** If gmin stepping fails, ngspice switches to source stepping: scale all independent sources from 0 to 1 in `numSrcSteps` increments, converging the NR loop at each step. The final step at `src_fact=1.0` gives the true operating point. The stepping schedule and convergence handling have specific logic that matters.

- **Integration order control.** The transient engine starts at order 1 (backward Euler) and steps up to order 2 (trapezoidal) after the first accepted step. After a breakpoint (source discontinuity), it drops back to order 1. The conditions for order changes are specific and affect truncation error estimation.

Each of these is a case where "simplifying" the ngspice logic would break real circuits.

## What this means in practice

**Use ngspice variable names.** When the C code uses `vgs`, the Rust code uses `vgs`. When it uses `qgs`, we use `qgs`. When it uses `cqgs`, we use `cqgs`. This makes it possible to put the C and Rust code side by side and verify line by line. Renaming variables to "more Rust-like" names makes verification harder for zero benefit.

**Preserve control flow.** If the C code tests `if (mode & MODETRAN)` before computing charges, the Rust code tests `if mode.is_tran()` at the same point. Don't restructure the logic into "cleaner" Rust patterns if it changes the order of operations or the conditions under which code runs.

**Keep magic constants.** ngspice defines `EPSOX = 3.453133e-11` and `EPSSI = 1.03594e-10` in the BSIM3 model. These are not the NIST values for permittivity — they are the values that the BSIM3 team calibrated against. Using "more accurate" constants breaks parameter extraction and produces different results. Use the same constants.

**Comment the C source location.** Every function, every significant block, should reference the ngspice file and line number it was ported from:

```rust
// Port of mos1load.c:163-171 — cutoff region
if vgs <= von {
    gds = 0.0;
    ids = 0.0;
    gm = 0.0;
    gmbs = 0.0;
}
```

This makes it possible for any contributor to find the original C code and verify the translation.

## When to deviate

There are legitimate reasons to write Rust-idiomatic code:

- **Memory safety.** Replace raw pointer arithmetic with array indexing. Replace `malloc`/`free` with arena allocation. Replace global mutable state with explicit parameters.
- **Error handling.** Replace `goto error` patterns with `Result<T, E>`.
- **Type safety.** Use enums instead of integer flag constants where the set of values is fixed and known.

But never deviate on **numerical behavior**. The sequence of floating-point operations, the comparison operators, the iteration limits — these must match ngspice.
