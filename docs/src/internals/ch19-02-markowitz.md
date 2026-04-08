# Markowitz Deep Dive

The Markowitz backend is a port of Sparse 1.3 (Kenneth Kundert, 1988), the same sparse solver used as ngspice's default. It is optimized for circuit simulation: diagonal pivoting preference, threshold-based pivot selection, and an elimination ordering that exploits the structure of MNA matrices.

**Source:** `sim/sparse-rs/src/markowitz/`

## Arena-based linked-list matrix

**Source:** `sim/sparse-rs/src/markowitz/matrix.rs`

The matrix is stored as doubly-linked lists (by row and by column) with all elements in a flat arena:

```rust
pub struct MarkowitzMatrix {
    size: usize,
    elements: Vec<Element>,       // arena, index 0 = dummy sentinel
    first_in_row: Vec<u32>,       // 1-indexed head pointers
    first_in_col: Vec<u32>,
    diag: Vec<u32>,               // direct pointers to diagonal elements
    markowitz_row: Vec<i32>,      // nonzero count per row
    markowitz_col: Vec<i32>,      // nonzero count per column
    markowitz_prod: Vec<i64>,     // row_count * col_count per row/col
    int_to_ext_row: Vec<usize>,   // permutation: internal → external
    int_to_ext_col: Vec<usize>,
    ext_to_int_row: Vec<usize>,   // permutation: external → internal
    ext_to_int_col: Vec<usize>,
    singletons: usize,            // count of rows/cols with Markowitz product 0
    // ...
}

pub struct Element {
    pub real: f64,
    pub imag: f64,                // for AC analysis
    pub row: u32,
    pub col: u32,
    pub next_in_row: u32,         // arena index (NONE = u32::MAX for NULL)
    pub next_in_col: u32,
}
```

All arrays are 1-indexed to match the C code. Index 0 is unused or holds sentinels. `NONE = u32::MAX` replaces NULL pointers. This preserves the exact indexing arithmetic from the C source, making line-by-line verification possible.

### Element access

Elements are accessed by arena index:

```rust
fn el(&self, idx: u32) -> &Element { &self.elements[idx as usize] }
fn el_mut(&mut self, idx: u32) -> &mut Element { &mut self.elements[idx as usize] }
```

Creating a new element appends to the arena and links it into the appropriate row and column lists. The `get_element(row, col)` method returns the arena index of an existing element or creates a new one.

### Clearing values

`clear_matrix()` zeros all element values without modifying the structure. This is called at the start of each NR iteration before devices restamp their values.

## Markowitz pivot criterion

The Markowitz criterion selects the pivot that minimizes `(row_count - 1) * (col_count - 1)`, where `row_count` is the number of nonzeros in the pivot's row and `col_count` is the number in its column (within the unreduced submatrix). This product approximates the fill-in that the pivot will cause.

A Markowitz product of 0 means either the row or column has exactly one nonzero — a singleton that can be eliminated with zero fill-in.

## The four-level pivot cascade

**Source:** `sim/sparse-rs/src/markowitz/pivot.rs`

At each elimination step, the pivot search proceeds through four levels, stopping as soon as an acceptable pivot is found:

### Level 1: Search for singletons

If `singletons > 0`, scan for rows or columns with Markowitz product 0.

**Row singletons:** A row with one nonzero. That element is the pivot — no choice needed. Eliminates the row with zero fill-in.

**Column singletons:** A column with one nonzero. The element is a candidate, but it must pass the threshold test: `|pivot| >= threshold * max_in_column`. If it fails (the singleton is much smaller than other elements in that column), fall through to the next level.

Singletons are common in circuit matrices: voltage source branch equations, for example, often create column singletons.

### Level 2: Quick diagonal search

Scan diagonal elements of the unreduced submatrix. For each diagonal element, compute the Markowitz product and check whether `|diag| >= threshold * max_in_column` (a quick estimate using only the column maximum). Track the diagonal element with the smallest Markowitz product.

Stop early if a product of 1 is found (optimal for non-singleton).

This level has a bias toward diagonal pivoting (the `DIAGONAL_PIVOTING = YES` configuration matching ngspice). Diagonal pivots preserve the physical meaning of node voltages in the solution.

### Level 3: Careful diagonal search

Same as Level 2, but with a full threshold check: compute `max_in_column` exactly by traversing the column, and verify `|diag| >= threshold * max_in_column`. This catches cases where the quick estimate was too loose.

### Level 4: Full matrix search

Scan every element in the unreduced submatrix. For each element, compute the Markowitz product and do the full threshold check. This is the last resort and guarantees finding a pivot if the matrix is non-singular.

The cascade ensures that diagonal pivots are strongly preferred (Levels 2-3 only consider diagonals), general pivots are used only when diagonals are inadequate (Level 4), and singletons are always exploited (Level 1).

## Factorization

**Source:** `sim/sparse-rs/src/markowitz/factor.rs`

### `order_and_factor()`

First-time factorization that determines the pivot ordering:

For each elimination step `s = 1..N`:
1. Call `search_for_pivot()` to select the pivot element.
2. Swap rows and columns to move the pivot to position (s, s).
3. Update Markowitz counts for affected rows and columns.
4. Eliminate: for each element below the pivot in column s, compute the multiplier and update all elements in that row to the right of column s. Create fill-in elements as needed.

The pivot ordering (row and column permutations) is stored for reuse by refactorization.

L stores `1/pivot` on the diagonal (not the pivot itself). U has an implicit unit diagonal. The multipliers for L are stored in-place in the lower triangle of the matrix.

### `factor()` (refactorization)

Reuses the pivot ordering from `order_and_factor()`. Walks through the same elimination steps in the same order, but skips the pivot search. Only recomputes numeric values.

For circuit simulation, `factor()` is the normal path after the first NR iteration. The matrix structure doesn't change between iterations — only the device conductances change.

## Solve

**Source:** `sim/sparse-rs/src/markowitz/solve.rs`

Forward substitution (L) then backward substitution (U), operating on the RHS vector in-place. The permutation arrays map between the user's external ordering and the solver's internal (pivoted) ordering.

```
Forward (L):  for s = 1..N: rhs[s] -= sum(L[s,j] * rhs[j] for j < s)
              rhs[s] *= diag[s]  (which is 1/pivot)
Backward (U): for s = N..1: rhs[s] -= sum(U[s,j] * rhs[j] for j > s)
```

The traversal follows the linked-list structure: for each pivot row, walk the row list to accumulate products.

## Complex mode (AC analysis)

For AC analysis, elements have both `real` and `imag` fields. The factorization and solve operate on complex values. In DC/transient mode, `imag` is always zero and the solver effectively operates on reals only. This matches ngspice's Sparse 1.3, which always allocates the imaginary field but ignores it in real-mode operations.
