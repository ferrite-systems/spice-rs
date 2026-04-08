# Case Study: Porting the Markowitz Sparse Solver

The Markowitz solver in sparse-rs is a port of Sparse 1.3 (Kundert, 1988) — the same sparse solver that ngspice uses by default. This is the most structurally complex piece of the port because the C code relies heavily on raw pointers, linked lists, and 1-indexed arrays.

**sparse-rs source:** `sim/sparse-rs/src/markowitz/` (matrix.rs, factor.rs, pivot.rs, solve.rs)
**Reference source:** Sparse 1.3 as embedded in ngspice (`reference/ngspice/src/spicelib/sparse/`)

## The C data model

Sparse 1.3 uses a linked-list sparse matrix where each nonzero element has four pointers: next in row, next in column, and the (row, col) position. The matrix frame holds arrays of row/column head pointers, diagonal pointers, and Markowitz counts. Everything is heap-allocated with `malloc` and linked together via C pointers.

Key C structures:
- `MatrixFrame` — the matrix container
- `ElementRecord` — a single nonzero (value, row, col, NextInRow, NextInCol)
- `AllocationRecord` — memory management for element pools

The code is 1-indexed (rows and columns 1..N), uses NULL as a sentinel, and relies on pointer arithmetic for traversal.

## The Rust translation

### Arena-based indices instead of pointers

The central translation decision: replace all `Element*` pointers with `u32` arena indices into a `Vec<Element>`.

```rust
pub struct MarkowitzMatrix {
    elements: Vec<Element>,       // arena
    first_in_row: Vec<u32>,       // head pointers (1-indexed)
    first_in_col: Vec<u32>,
    diag: Vec<u32>,               // diagonal element indices
    markowitz_row: Vec<i32>,      // Markowitz counts
    markowitz_col: Vec<i32>,
    markowitz_prod: Vec<i64>,     // Markowitz products
    int_to_ext_row: Vec<usize>,   // permutation arrays
    int_to_ext_col: Vec<usize>,
    ext_to_int_row: Vec<usize>,
    ext_to_int_col: Vec<usize>,
    // ...
}

pub struct Element {
    pub real: f64,
    pub imag: f64,
    pub row: u32,
    pub col: u32,
    pub next_in_row: u32,         // arena index, not pointer
    pub next_in_col: u32,
}
```

The sentinel value `NONE = u32::MAX` replaces `NULL`. Element 0 in the arena is a dummy sentinel, never used as a real element.

This gives the same O(1) linked-list traversal as the C code, with bounds-checked access and no unsafe code. The arena is append-only during setup and stable during factorization.

### Preserving 1-indexed convention

The C code uses 1-based indexing throughout (rows 1..N, columns 1..N, array indices 1..size). Rather than converting to 0-based Rust indexing (which would require +1/-1 adjustments on every access and invite off-by-one errors), sparse-rs preserves the 1-indexed convention:

```rust
// first_in_row[0] is unused, first_in_row[1..=size] are the head pointers
pub(crate) first_in_row: Vec<u32>,  // length = size + 1
```

This makes the Rust code line-up with the C code during verification: `matrix->FirstInRow[i]` in C corresponds to `self.first_in_row[i]` in Rust at the same index.

## The pivot cascade

The Markowitz pivot search is the heart of the solver. It selects the pivot element at each elimination step to minimize fill-in while maintaining numerical stability.

**Source:** `sim/sparse-rs/src/markowitz/pivot.rs`

The cascade (from `spfactor.c`):

### 1. Search for singletons

A singleton is a row or column with exactly one nonzero in the unreduced portion (Markowitz product = 0). Singletons can be eliminated with zero fill-in. The search processes all singletons before moving to the general case.

Row singletons and column singletons are handled separately: a row singleton's pivot is the only element in its row; a column singleton's pivot is the only element in its column. For column singletons, the code checks that the pivot magnitude meets the threshold relative to the largest element in the column.

### 2. Quick diagonal search

Scan diagonal elements (unreduced rows/columns) and pick the one with the smallest Markowitz product, subject to the threshold test. The threshold test requires `|pivot| >= threshold * max_in_column`. The default threshold is 1e-3.

This search is "quick" because it only looks at diagonal elements and stops early when it finds a Markowitz product of 1 (can't improve).

### 3. Careful diagonal search

Same as quick, but does a full threshold check against every element in the column, not just a quick magnitude test. This catches cases where the quick search missed an acceptable diagonal because its rough magnitude estimate was too conservative.

### 4. Full matrix search

Last resort. Scan every element in the unreduced portion of the matrix. This is expensive but guarantees finding a pivot if the matrix is non-singular.

## Factorization and solve

**Factor** (`sim/sparse-rs/src/markowitz/factor.rs`): After selecting each pivot, the code eliminates the column below and the row to the right. Fill-in elements are created as needed (appended to the arena) and linked into the row/column lists. Markowitz counts are updated as elements are created or removed. The pivot's reciprocal is stored on the diagonal (L stores 1/pivot, U has implicit unit diagonal).

**Solve** (`sim/sparse-rs/src/markowitz/solve.rs`): Forward substitution using L, then backward substitution using U. The solve operates on the RHS vector in-place, using the pivot permutation to map between external and internal ordering.

**Refactorization**: After the first `order_and_factor()`, subsequent calls with the same sparsity pattern reuse the pivot ordering and just update numeric values. This is 2-5x faster than a full factorization and is the normal path during NR iteration (where the matrix structure doesn't change, only the values).

## Validation

The Markowitz solver is cross-validated against the KLU backend: both solvers are given the same matrix and RHS, and their solutions must agree to machine precision. The `sparse-eval` harness runs this comparison on a suite of test matrices.

For the spice-rs integration, the Markowitz solver is validated indirectly through the full eval harness: if 200 circuits produce correct results, the solver is working correctly for those matrices. Any solver bug would manifest as a circuit failing to converge or producing wrong node voltages.
