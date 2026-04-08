# Benchmarks

The `sparse-eval` crate (`sim/sparse-eval/`) benchmarks sparse-rs against SuiteSparse C implementations. It validates correctness and measures performance on a suite of test matrices.

**Source:** `sim/sparse-eval/src/main.rs`, `solver_compare.rs`, `amd_compare.rs`

## sparse-eval suite

The main benchmark runs test matrices through both sparse-rs KLU and SuiteSparse C KLU, comparing:

- **Numerical accuracy**: maximum absolute and relative error between solutions
- **Performance**: microseconds per solve
- **Correctness**: both solvers must produce the same result within machine precision

```bash
cargo run --release --bin sparse-eval
```

Output:

```
┌──────────────────────────────┬────────┬────────────┬────────────┬────────────┬────────────┬─────────┐
│ Matrix                       │ Size   │ Max AbsErr │ Max RelErr │ sparse-rs  │   KLU(C)   │ Speedup │
├──────────────────────────────┼────────┼────────────┼────────────┼────────────┼────────────┼─────��───┤
│ resistor_3x3                 │    3   │   0.00e+00 │   0.00e+00 │     12 us  │      8 us  │   0.7x  │
│ rc_lowpass_4x4               │    4   │   0.00e+00 │   0.00e+00 │     14 us  │      9 us  │   0.6x  │
│ ...                          │        │            │            │            │            │         │
```

The test matrices are generated from representative circuit topologies at various sizes.

## Performance characteristics

### sparse-rs KLU vs SuiteSparse C KLU

Correctness is validated: solutions match to machine precision on all test matrices.

Performance: sparse-rs is typically within 2x of SuiteSparse C. The gap comes from:
- SuiteSparse uses hand-optimized C with careful cache layout
- sparse-rs uses safe Rust with bounds checking
- SuiteSparse has decades of micro-optimization

For circuit simulation, the solver is not the bottleneck — device model evaluation dominates. The 2x overhead on the solver translates to a much smaller overhead on total simulation time.

### Markowitz vs KLU

These are different algorithms optimized for different goals:

**Markowitz (Sparse 1.3):**
- Strong diagonal preference — preserves physical node ordering
- Optimized for circuit matrices where the diagonal is often the best pivot
- Integrated refactorization (reuse pivot ordering when only values change)
- Used by ngspice as the default solver

**KLU:**
- BTF decomposition exploits block structure
- AMD ordering is better at minimizing fill-in for general sparse matrices
- Gilbert-Peierls is more cache-friendly than the linked-list traversal in Markowitz
- Better for larger matrices or matrices with poor diagonal dominance

For typical SPICE circuits (tens to hundreds of nodes), the choice between Markowitz and KLU makes little practical difference. Both converge to the same solution. Markowitz is used in spice-rs for ngspice parity.

### Refactorization

Both backends support refactorization — reusing the symbolic structure and pivot ordering when only numeric values change. This is the critical performance optimization for SPICE simulation, where the NR loop calls the solver many times with the same matrix structure.

Refactorization is typically 2-5x faster than full numeric factorization:
- Skips pivot search (Markowitz) or DFS pattern discovery (KLU)
- Reuses the same memory layout
- Only updates numeric values

For the Markowitz backend, the first NR iteration calls `order_and_factor()` (full factorization with pivot selection), and subsequent iterations call `factor()` (refactorization with the same pivot order).

## Companion benchmarks

### amd-compare

```bash
cargo run --release --bin amd-compare
```

Compares AMD ordering quality between sparse-rs and SuiteSparse. Measures the fill-in (number of nonzeros in L+U) produced by each ordering on the same matrices. Quality should be identical since both implement the same algorithm.

### solver-compare

```bash
cargo run --release --bin solver-compare
```

Detailed comparison of the full solve pipeline (symbolic + numeric + solve) between sparse-rs KLU and SuiteSparse KLU. Includes per-phase timing and accuracy metrics.

## Building sparse-eval

`sparse-eval` links against SuiteSparse C libraries at build time via a `build.rs` script that uses the `cc` crate. The SuiteSparse source is in `reference/SuiteSparse/` (a git submodule). The build compiles the C code and links it statically.

```bash
cd sim/sparse-eval && cargo run --release
```
