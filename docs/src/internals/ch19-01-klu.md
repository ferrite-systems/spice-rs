# KLU Deep Dive

KLU is a sparse direct solver designed for circuit simulation matrices. It combines three algorithms into a pipeline: BTF decomposition, AMD fill-reducing ordering, and Gilbert-Peierls sparse LU factorization.

**Source:** `sim/sparse-rs/src/klu/`

## Pipeline overview

```
Input matrix A (CSC)
        │
   ┌────▼────┐
   │   BTF    │  Hopcroft-Karp matching + SCC decomposition
   └────┬────┘  → block structure, permutation
        │
   ┌────▼────┐
   │   AMD    │  per-block fill-reducing column ordering
   └────┬────┘  → column permutation within each block
        │
   ┌────▼────────────┐
   │  Gilbert-Peierls │  left-looking LU with DFS-based sparse triangular solve
   └────┬────────────┘  → L, U factors, pivot permutation
        │
   ┌────▼────┐
   │  Solve  │  sparse forward/back substitution
   └─────────┘  → solution vector x
```

## Phase 1: Symbolic analysis

### BTF decomposition

**Source:** `sim/sparse-rs/src/klu/btf.rs`

BTF (Block Triangular Form) permutes the matrix into upper block-triangular form:

```
┌──────┬──────┬──────┐
│  B1  │  *   │  *   │
├──────┼──────┼──────┤
│  0   │  B2  │  *   │
├──────┼──────┼──────┤
│  0   │  0   │  B3  │
└──────┴──────┴──────┘
```

Each diagonal block Bk can be factored independently. Off-diagonal blocks are handled by back-substitution.

The algorithm:
1. **Hopcroft-Karp maximum matching** — find a permutation that puts nonzeros on the diagonal (maximum transversal). This transforms the structural singularity check into a matching problem.
2. **Strongly connected components (SCC)** — find the SCC decomposition of the directed graph defined by the matching. Each SCC becomes one diagonal block.

Singletons (1x1 blocks) are particularly valuable: they require no LU factorization, just a division. Circuit matrices often have many singletons (voltage source branch equations, for example).

### AMD ordering

**Source:** `sim/sparse-rs/src/klu/amd.rs`, `amd_quotient.rs`

Within each non-singleton BTF block, AMD (Approximate Minimum Degree) computes a column permutation that minimizes fill-in during LU factorization. AMD is an approximation to the NP-hard minimum fill problem.

The algorithm maintains a quotient graph representation of the elimination graph and greedily selects the column with the minimum degree (fewest connections) at each step. The quotient graph compresses eliminated nodes into supernodes, keeping the graph size manageable.

`amd_quotient.rs` contains the quotient graph implementation, ported from SuiteSparse AMD.

The result is a column permutation applied within each BTF block. Combined with the BTF permutation, this gives the full symbolic analysis: `SymbolicLu { btf, col_perm, row_perm, block_ranges, ... }`.

## Phase 2: Numeric factorization

### Gilbert-Peierls LU

**Source:** `sim/sparse-rs/src/klu/lu.rs` (the `numeric()` function)

For each BTF block, the Gilbert-Peierls algorithm computes sparse L and U factors using left-looking factorization:

For each column k:
1. **Sparse triangular solve** — solve `L * x = A[:,k]` where only the nonzero entries of the solution are computed. The nonzero pattern is found by a DFS (depth-first search) on the graph of L, starting from the nonzero entries of `A[:,k]`.
2. **Partial pivoting** — among the entries below the diagonal in x, select the largest as the pivot. Swap rows accordingly.
3. **Split** — entries above the pivot become column k of U; entries at and below become column k of L (with L scaled by 1/pivot).

The DFS-based sparse triangular solve is what makes Gilbert-Peierls efficient for sparse matrices: it only visits the rows that will have nonzero values, rather than iterating over all N rows.

L and U factors are stored in packed sparse format:
```rust
pub struct NumericLu {
    l_colptr: Vec<usize>,     // column pointers into l_entries
    l_entries: Vec<SpEntry>,  // (row, value) pairs
    u_colptr: Vec<usize>,
    u_entries: Vec<SpEntry>,
    u_diag: Vec<f64>,         // diagonal of U (for singletons and pivots)
    pivot_perm: Vec<usize>,   // row pivot permutation
    // ...
}
```

### Row scaling

Before factorization, each row is scaled by the reciprocal of its maximum absolute value (KLU scale=2). This improves numerical stability by normalizing the matrix rows. The scale factors are stored in `NumericLu.rs` and applied during solve.

## Phase 3: Solve

**Source:** `sim/sparse-rs/src/klu/lu.rs` (the `solve()` function)

Given L, U, and a RHS vector b:

1. Apply row scaling: `b[i] *= rs[i]`
2. Apply row permutation: reorder b according to pivot_perm
3. **Forward substitution** with L (unit lower triangular): process blocks in order, using sparse column access
4. **Back substitution** with U (upper triangular): process blocks in reverse order
5. Apply column permutation to get the solution in original ordering

For multi-block BTF: solve each block's subsystem, then use off-diagonal entries for inter-block back-substitution.

## Refactorization

**Source:** `sim/sparse-rs/src/klu/lu.rs` (the `refactor()` function)

When the matrix sparsity pattern hasn't changed (typical during NR iteration — only the numeric values change), refactorization reuses the symbolic analysis and pivot permutation. It recomputes L and U values using the existing nonzero structure.

This skips BTF, AMD, the DFS-based symbolic structure discovery, and pivot selection — it just fills in the numbers. For circuit matrices, refactorization is 2-5x faster than full numeric factorization and 10-100x faster than the full symbolic+numeric pipeline.

`refactor_inplace()` is a variant that updates the factors without a fresh allocation, further reducing overhead.

## API

```rust
// One-shot solve from triplets (convenience):
let x = solve_from_triplets(n, &triplets, &rhs)?;

// Pipeline for repeated solves:
let sym = symbolic(n, &col_ptr, &row_idx);       // once per topology
let num = numeric(&sym, &col_ptr, &row_idx, &values)?;  // once per value change
let x = solve(&sym, &num, &rhs);                 // once per RHS

// Refactorization (same pattern, new values):
let num2 = refactor(&sym, &num, &col_ptr, &row_idx, &new_values)?;
let x2 = solve(&sym, &num2, &rhs2);
```
