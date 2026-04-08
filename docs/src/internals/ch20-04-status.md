# Current Status

Summary of spice-rs parity with ngspice as of the latest eval run.

## Overall

| Metric | Count |
|--------|-------|
| Total test circuits | 224 |
| **Passed** | 200 |
| **Bit-identical** | 176 |
| **Failed** | 3 |
| **Errors** | 3 |

Standard tolerance: abs=0.01, rel=0.01 (never loosened to pass a test).

## Bit-identical results

176 of 224 circuits produce results that are bit-for-bit identical to ngspice (zero absolute error, zero relative error). This means the Rust code produces the exact same sequence of IEEE 754 floating-point operations as the C code.

Bit-identical circuits span all complexity layers: passives (L1-L2), single devices (L3), NR stress tests (L4), device interactions (L5), and many full circuits (L6+).

## Passing but not bit-identical

24 circuits pass (within tolerance) but have nonzero error, typically at the 1e-12 to 1e-14 level. Sources of these tiny divergences:

- **Floating-point non-associativity**: Different evaluation order of summations produces different rounding. This happens when Rust's optimizer reorders instructions differently than GCC.
- **Library function differences**: `f64::exp()`, `f64::ln()`, `f64::sqrt()` may produce different last-bit results between Rust's libm and C's libm on the same platform.
- **Convergence path sensitivity**: For circuits near the convergence boundary, a last-bit difference in one NR iteration can change whether an extra iteration is taken, producing a slightly different (but equally valid) converged result.

These are not bugs — they are inherent to the comparison of two independently-compiled floating-point programs.

## Failed circuits (3)

The 3 failed circuits are BSIM4 models:

- **BSIM4 NMOS DC** — BSIM4 load function port in progress
- **BSIM4 PMOS DC** — same root cause as NMOS
- **BSIM4 CMOS Inverter** — depends on BSIM4 device model

BSIM4 is the most complex device model (~8000 lines of C in the load function alone). The port is underway, following the same function-by-function translation method used for BSIM3.

## Error circuits (3)

The 3 error circuits are convergence failures where spice-rs fails to find the DC operating point:

- These circuits converge in ngspice using specific convergence aid sequences (gmin stepping followed by source stepping with particular step counts).
- The convergence path depends on subtle differences in the stepping schedule and NR iteration behavior.
- Investigation is active — the diverge-deep diagnostic mode is being used to identify the exact NR iteration where convergence diverges.

## Device model coverage

| Device | Status | Test circuits |
|--------|--------|--------------|
| Resistor | Complete, bit-identical | L1-L2, many L6+ |
| Capacitor | Complete, bit-identical | L1-L2, many L6+ |
| Inductor | Complete, bit-identical | L1-L2, many L6+ |
| Mutual Inductor | Complete, bit-identical | L6+ |
| Voltage Source (DC, AC, PULSE, SIN, PWL) | Complete, bit-identical | All layers |
| Current Source (DC, AC, PULSE, SIN, PWL) | Complete, bit-identical | All layers |
| Diode | Complete, bit-identical | L3-L6+ |
| MOSFET Level 1 | Complete, bit-identical | L3-L6+ |
| MOSFET Level 2 | Complete, passing | L6+ |
| MOSFET Level 3 | Complete, passing | L6+ |
| BSIM3v3 | Complete, passing | 11 circuits in L6+ |
| BSIM4 | In progress | 3 failing |
| BJT (NPN/PNP) | Complete, bit-identical | L3-L6+ |
| JFET (N/P) | Complete, bit-identical | L3-L6+ |
| VCVS | Complete, bit-identical | L1, L6+ |
| VCCS | Complete, bit-identical | L6+ |
| CCCS | Complete, bit-identical | L6+ |
| CCVS | Complete, bit-identical | L6+ |
| Transmission Line | Complete, passing | L6+ |

## Analysis type coverage

| Analysis | Status |
|----------|--------|
| DC Operating Point (.OP) | Complete |
| DC Sweep (.DC) | Complete |
| Transient (.TRAN) | Complete |
| AC (.AC) | Complete |
| Transfer Function (.TF) | Complete |
| Sensitivity (.SENS) | Complete |
| Pole-Zero (.PZ) | Complete |

## Solver coverage

| Solver | Status |
|--------|--------|
| Markowitz (default) | Complete, used for all eval circuits |
| KLU | Complete, validated via sparse-eval |

## Next targets

1. **BSIM4 completion** — port the remaining load function sections
2. **Convergence parity** — investigate the 3 error circuits
3. **Additional test circuits** — expand coverage for edge cases (temperature sweeps, Monte Carlo parameter variation)
