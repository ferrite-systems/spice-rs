# sparse-rs API

sparse-rs is a pure Rust sparse matrix solver with two backends:

- **KLU** -- column-oriented LU factorization with BTF and AMD reordering, faithful port of SuiteSparse KLU
- **Markowitz** -- row-oriented factorization with Markowitz pivot selection, faithful port of Sparse 1.4

Both solve `Ax = b` where A is a sparse square matrix.

## Matrix construction

All backends use `CscMatrix` (compressed sparse column) as input:

```rust
use sparse_rs::CscMatrix;

let mat = CscMatrix::from_triplets(
    n,        // matrix dimension (n x n)
    &rows,    // row indices
    &cols,    // column indices
    &values,  // nonzero values
);
```

Duplicate entries at the same (row, col) are summed, matching the standard assembly convention for circuit matrices.

## KLU backend

Three-phase workflow: symbolic analysis, numeric factorization, solve.

```rust
use sparse_rs::klu::{symbolic, numeric, solve};

// Phase 1: symbolic analysis (depends only on sparsity pattern)
let sym = symbolic(&mat);

// Phase 2: numeric factorization
let num = numeric(&mat, &sym).expect("factorization failed");

// Phase 3: solve Ax = b (solution overwrites b in-place)
let mut b = vec![1.0, 2.0, 3.0];
solve(&num, &sym, &mut b).expect("solve failed");
// b now contains x
```

Symbolic analysis is expensive but only needs to run once for a given sparsity pattern. Numeric factorization and solve are fast and can be repeated when matrix values change but the pattern stays the same (as in Newton-Raphson iteration).

### Refactorization

```rust
// Matrix values changed, but same sparsity pattern
let num2 = numeric(&mat_updated, &sym).expect("refactorization failed");
solve(&num2, &sym, &mut b2).expect("solve failed");
```

### Convenience function

For one-shot solves:

```rust
use sparse_rs::klu::solve_from_triplets;

let mut b = vec![1.0, 2.0, 3.0];
solve_from_triplets(3, &rows, &cols, &values, &mut b).expect("solve failed");
```

## Markowitz backend

Two-phase workflow: combined ordering + factorization, then solve.

```rust
use sparse_rs::markowitz::{order_and_factor, solve};

let lu = order_and_factor(&mat).expect("factorization failed");

let mut b = vec![1.0, 2.0, 3.0];
solve(&lu, &mut b).expect("solve failed");
```

### With prescribed permutation

```rust
use sparse_rs::markowitz::order_and_factor_with_perm;

let perm = vec![2, 0, 1]; // prescribed column ordering
let lu = order_and_factor_with_perm(&mat, Some(&perm)).expect("factorization failed");
```

### In-place factorization

For repeated solves with value changes, use the `MarkowitzMatrix` type directly:

```rust
use sparse_rs::markowitz::{MarkowitzMatrix, order_and_factor_in_place, solve_with_matrix};

let mut matrix = MarkowitzMatrix::from_csc(&mat);
order_and_factor_in_place(&mut matrix).expect("factorization failed");

let mut b = vec![1.0, 2.0, 3.0];
solve_with_matrix(&matrix, &mut b).expect("solve failed");
```

### Complex solve

```rust
use sparse_rs::markowitz::{order_and_factor_complex_in_place, solve_complex_with_matrix};

// Interleaved real/imaginary pairs in matrix values and RHS
order_and_factor_complex_in_place(&mut matrix).expect("factorization failed");
solve_complex_with_matrix(&matrix, &mut b_complex).expect("solve failed");
```

## Complete example

Solving a 3x3 system:

```
 2x + 1y + 0z = 5
 1x + 3y + 2z = 15
 0x + 2y + 4z = 20
```

```rust
use sparse_rs::CscMatrix;
use sparse_rs::klu::{symbolic, numeric, solve};

fn main() {
    // Build matrix from triplets (row, col, value)
    let rows = vec![0, 1, 0, 1, 2, 1, 2];
    let cols = vec![0, 0, 1, 1, 1, 2, 2];
    let vals = vec![2.0, 1.0, 1.0, 3.0, 2.0, 2.0, 4.0];

    let mat = CscMatrix::from_triplets(3, &rows, &cols, &vals);

    // Factor and solve
    let sym = symbolic(&mat);
    let num = numeric(&mat, &sym).expect("factorization failed");

    let mut b = vec![5.0, 15.0, 20.0];
    solve(&num, &sym, &mut b).expect("solve failed");

    println!("x = {:.4}", b[0]); // x = 0.5000
    println!("y = {:.4}", b[1]); // y = 4.0000
    println!("z = {:.4}", b[2]); // z = 3.0000
}
```

## Choosing a backend

| | KLU | Markowitz |
|--|-----|-----------|
| Best for | Large sparse systems (100+ nodes) | Small to medium systems |
| Reordering | AMD + BTF block decomposition | Markowitz criterion |
| Refactorization | Fast (reuse symbolic) | Must re-order |
| Complex support | Not yet | Yes |
| Used by | spice-rs (default solver) | spice-rs (AC analysis complex solve) |
