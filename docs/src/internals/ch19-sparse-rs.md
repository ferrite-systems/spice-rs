# sparse-rs Internals

sparse-rs is a pure Rust sparse direct solver library providing two independent backends for solving `Ax = b` where `A` is a sparse matrix.

**Source:** `sim/sparse-rs/src/`

## Two backends

### KLU

Port of SuiteSparse KLU by Timothy A. Davis. A general-purpose sparse direct solver using:
- **BTF** (Block Triangular Form) decomposition to break the matrix into independent diagonal blocks
- **AMD** (Approximate Minimum Degree) ordering to minimize fill-in within each block
- **Gilbert-Peierls** left-looking LU factorization with partial pivoting

KLU uses a three-phase pipeline: symbolic analysis (once per matrix pattern), numeric factorization (once per value change), and solve (once per RHS). Refactorization reuses the symbolic structure for 2-5x speedup.

**Source:** `sim/sparse-rs/src/klu/` (lu.rs, btf.rs, amd.rs, amd_quotient.rs, colamd.rs)

### Markowitz

Port of Sparse 1.3 (Kundert, 1988). A circuit-simulation-optimized solver using:
- Arena-based linked-list sparse matrix with u32 indices
- Markowitz pivot criterion with diagonal preference
- Four-level pivot cascade for fill-minimizing pivot selection

Markowitz is ngspice's default solver. spice-rs uses this backend for the MNA system.

**Source:** `sim/sparse-rs/src/markowitz/` (matrix.rs, factor.rs, pivot.rs, solve.rs)

## Shared interface

Both backends accept input in CSC (Compressed Sparse Column) format via `CscMatrix`:

```rust
pub struct CscMatrix {
    pub n: usize,
    pub col_ptr: Vec<usize>,
    pub row_idx: Vec<usize>,
    pub values: Vec<f64>,
}
```

However, in the spice-rs MNA integration, the Markowitz backend is used directly through `MarkowitzMatrix` (which manages its own linked-list storage) rather than through `CscMatrix`. The CSC interface is primarily used by KLU and by the `sparse-eval` benchmark harness.

## Chapters

- [KLU Deep Dive](ch19-01-klu.md) — BTF, AMD, Gilbert-Peierls, refactorization
- [Markowitz Deep Dive](ch19-02-markowitz.md) — arena matrix, pivot cascade, diagonal preference
- [Benchmarks](ch19-03-benchmarks.md) — performance comparison against SuiteSparse C
