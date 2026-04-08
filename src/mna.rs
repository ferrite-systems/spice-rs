use std::collections::HashMap;
use sparse_rs::markowitz::MarkowitzMatrix;
use sparse_rs::markowitz::matrix::NONE;

/// Matrix element handle — caches the arena index from get_element.
/// Matches ngspice's `double*` pointers cached during TSTALLOC.
pub type MatElt = u32;

/// Modified Nodal Analysis matrix system.
///
/// ONE persistent matrix, matching ngspice's architecture:
/// - spCreate → new()
/// - spGetElement → make_element() (with TRANSLATE)
/// - spClear → clear()
/// - *(ptr) += value → stamp_elt() / stamp()
/// - spOrderAndFactor → solve() with needs_order=true
/// - spFactor → solve() with needs_order=false (refactor)
///
/// The matrix is NEVER moved, copied, or split. clear() zeros values,
/// load stamps new values via cached handles, solve factors in-place.
pub struct MnaSystem {
    pub size: usize,
    pub matrix: MarkowitzMatrix,
    needs_order: bool,
    skip_preorder: bool,
    /// Element handle cache: (ext_row, ext_col) → arena index.
    elt_cache: HashMap<(u32, u32), u32>,
    pub rhs: Vec<f64>,
    pub rhs_old: Vec<f64>,
    /// Imaginary RHS for AC analysis. Matches ngspice CKTirhs.
    pub irhs: Vec<f64>,
    /// Previous imaginary RHS. Matches ngspice CKTirhsOld.
    pub irhs_old: Vec<f64>,
    // TRANSLATE: external→internal node mapping (port of spbuild.c:450-491).
    pub ext_to_int: Vec<usize>,
    pub int_to_ext: Vec<usize>,
    pub next_int: usize,
    /// Whether TRANSLATE maps have been synced to the matrix.
    translate_synced: bool,
}

impl MnaSystem {
    pub fn new(size: usize) -> Self {
        Self {
            size,
            matrix: MarkowitzMatrix::new(size),
            needs_order: true,
            skip_preorder: false,
            elt_cache: HashMap::new(),
            rhs: vec![0.0; size + 1],
            rhs_old: vec![0.0; size + 1],
            irhs: vec![0.0; size + 1],
            irhs_old: vec![0.0; size + 1],
            ext_to_int: vec![0; size + 1],
            int_to_ext: vec![0; size + 1],
            next_int: 1,
            translate_synced: false,
        }
    }

    /// Translate an external node number to an internal matrix index.
    /// Port of ngspice Translate() in spbuild.c:450-491.
    fn translate_node(&mut self, ext: usize) -> usize {
        if ext == 0 { return 0; }
        if self.ext_to_int[ext] == 0 {
            let int_idx = self.next_int;
            self.next_int += 1;
            self.ext_to_int[ext] = int_idx;
            self.int_to_ext[int_idx] = ext;
        }
        self.ext_to_int[ext]
    }

    /// Get diagonal element value at internal position i (for tracing).
    pub fn diag_val(&self, i: usize) -> f64 {
        let idx = self.matrix.diag[i];
        if idx != sparse_rs::markowitz::matrix::NONE { self.matrix.el(idx).real } else { 0.0 }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    /// Get or create a matrix element — call during device SETUP only.
    /// Row/col are external (1-based MNA equation numbers). 0 = ground.
    pub fn make_element(&mut self, row: usize, col: usize) -> MatElt {
        if row == 0 || col == 0 {
            return NONE;
        }
        let int_row = self.translate_node(row);
        let int_col = self.translate_node(col);
        let handle = self.matrix.get_element(int_row as u32, int_col as u32);
        self.elt_cache.insert((row as u32, col as u32), handle);
        handle
    }

    /// Find or create a matrix element by (row, col) and return its arena index.
    /// Used by PZ analysis for direct element access (column operations).
    /// `row` and `col` are external (1-based) equation numbers.
    pub fn find_or_create_element(&mut self, row: usize, col: usize) -> MatElt {
        if row == 0 || col == 0 {
            return NONE;
        }
        let key = (row as u32, col as u32);
        if let Some(&h) = self.elt_cache.get(&key) {
            return h;
        }
        let int_row = self.translate_node(row);
        let int_col = self.translate_node(col);
        let h = self.matrix.get_element(int_row as u32, int_col as u32);
        self.elt_cache.insert(key, h);
        h
    }

    /// Stamp a value into a matrix element by handle.
    pub fn stamp_elt(&mut self, handle: MatElt, value: f64) {
        if handle == NONE || value == 0.0 {
            return;
        }
        self.matrix.el_mut(handle).real += value;
    }

    /// Stamp by (row, col) — caches element handles on first call.
    pub fn stamp(&mut self, row: usize, col: usize, value: f64) {
        if row == 0 || col == 0 || value == 0.0 {
            return;
        }
        let key = (row as u32, col as u32);
        let handle = if let Some(&h) = self.elt_cache.get(&key) {
            h
        } else {
            let int_row = self.translate_node(row);
            let int_col = self.translate_node(col);
            let h = self.matrix.get_element(int_row as u32, int_col as u32);
            self.elt_cache.insert(key, h);
            h
        };
        self.matrix.el_mut(handle).real += value;
    }

    /// Add a value to the RHS at equation `row` (1-based, 0 = ground ignored).
    pub fn stamp_rhs(&mut self, row: usize, value: f64) {
        if row == 0 { return; }
        self.rhs[row] += value;
    }

    /// Stamp into the imaginary part of a matrix element by handle.
    /// Port of ngspice `*(ptr + 1) += val` pattern used in device acLoad functions.
    pub fn stamp_elt_imag(&mut self, handle: MatElt, value: f64) {
        if handle == NONE || value == 0.0 {
            return;
        }
        self.matrix.el_mut(handle).imag += value;
    }

    /// Stamp imaginary value by (row, col).
    pub fn stamp_imag(&mut self, row: usize, col: usize, value: f64) {
        if row == 0 || col == 0 || value == 0.0 {
            return;
        }
        let key = (row as u32, col as u32);
        let handle = if let Some(&h) = self.elt_cache.get(&key) {
            h
        } else {
            let int_row = self.translate_node(row);
            let int_col = self.translate_node(col);
            let h = self.matrix.get_element(int_row as u32, int_col as u32);
            self.elt_cache.insert(key, h);
            h
        };
        self.matrix.el_mut(handle).imag += value;
    }

    /// Add a value to the imaginary RHS at equation `row`.
    pub fn stamp_irhs(&mut self, row: usize, value: f64) {
        if row == 0 { return; }
        self.irhs[row] += value;
    }

    /// Add gmin to all diagonal elements. Matches LoadGmin (spsmp.c:459-478).
    pub fn add_diag_gmin(&mut self, gmin: f64) {
        if gmin == 0.0 { return; }
        for i in 1..=self.size {
            let idx = self.matrix.diag[i];
            if idx != NONE {
                self.matrix.el_mut(idx).real += gmin;
            }
        }
    }

    /// Clear all matrix element values and RHS. Matches SMPclear.
    pub fn clear(&mut self) {
        self.matrix.clear();
        for v in &mut self.rhs { *v = 0.0; }
    }

    /// Clear for complex AC analysis. Matches SMPcClear (zeros real+imag+RHS).
    pub fn clear_complex(&mut self) {
        self.matrix.clear(); // zeros both real and imag
        for v in &mut self.rhs { *v = 0.0; }
        for v in &mut self.irhs { *v = 0.0; }
    }

    /// Force a reorder on the next solve call.
    /// Matches ngspice NISHOULDREORDER (niiter.c:116-119).
    /// The matrix stays in place — just sets a flag. clear()+load()
    /// stamp fresh values, then solve() re-orders and re-factors.
    pub fn force_reorder(&mut self) {
        self.needs_order = true;
        // ngspice's spMNA_Preorder checks `if (RowsLinked) return;`.
        // At MODEINITTRAN after a normal DC OP, rows ARE linked → preorder skipped.
        // At MODEINITTRAN after UIC (no DC OP factorization), rows are NOT linked
        // → preorder must run to swap columns for zero diagonals.
        self.skip_preorder = self.matrix.rows_linked;
    }

    /// Factor and solve the system.
    /// First call: mna_preorder + order_and_factor (full Markowitz).
    /// force_reorder: order_and_factor without preorder (reorder).
    /// Otherwise: refactor (reuse existing pivot order).
    pub fn solve(&mut self) -> Result<(), crate::error::SimError> {
        let n = self.size;
        if n == 0 { return Ok(()); }

        if self.needs_order {
            // Sync TRANSLATE maps on first ordering (before preorder).
            if !self.translate_synced && self.next_int > 1 {
                for int_idx in 1..self.next_int {
                    let ext_idx = self.int_to_ext[int_idx];
                    self.matrix.int_to_ext_row[int_idx] = ext_idx;
                    self.matrix.int_to_ext_col[int_idx] = ext_idx;
                    self.matrix.ext_to_int_row[ext_idx] = int_idx;
                    self.matrix.ext_to_int_col[ext_idx] = int_idx;
                }
                self.translate_synced = true;
            }

            if !self.skip_preorder {
                self.matrix.mna_preorder();
            }
            self.skip_preorder = false;

            // order_and_factor in-place (Markowitz pivot search + elimination)
            sparse_rs::markowitz::order_and_factor_in_place(&mut self.matrix)
                .map_err(|_| crate::error::SimError::SingularMatrix(0))?;
            self.needs_order = false;
        } else {
            // Refactor with existing pivot order
            sparse_rs::markowitz::refactor_in_place(&mut self.matrix)
                .map_err(|_| crate::error::SimError::SingularMatrix(0))?;
        }

        // Solve: permute RHS → forward/back substitution → unpermute solution
        let mut b: Vec<f64> = self.rhs[1..=n].to_vec();
        sparse_rs::markowitz::solve_with_matrix(&self.matrix, &mut b)
            .map_err(|_| crate::error::SimError::SingularMatrix(0))?;

        self.rhs[0] = 0.0;
        for i in 0..n { self.rhs[i + 1] = b[i]; }

        Ok(())
    }

    /// Forward/back substitution only — uses the already-factored matrix.
    /// Port of ngspice's SMPsolve: `spSolve(Matrix, RHS, RHS, NULL, NULL)`.
    /// The matrix must have been factored by a prior call to `solve()`.
    /// Operates on `self.rhs` in-place (the caller sets up the RHS first).
    pub fn solve_only(&mut self) -> Result<(), crate::error::SimError> {
        let n = self.size;
        if n == 0 { return Ok(()); }

        let mut b: Vec<f64> = self.rhs[1..=n].to_vec();
        sparse_rs::markowitz::solve_with_matrix(&self.matrix, &mut b)
            .map_err(|_| crate::error::SimError::SingularMatrix(0))?;

        self.rhs[0] = 0.0;
        for i in 0..n { self.rhs[i + 1] = b[i]; }

        Ok(())
    }

    /// Factor and solve the complex system (G + jwC)x = b for AC analysis.
    ///
    /// Port of NIacIter from niaciter.c.
    /// On first call, performs SMPcReorder (full ordering with complex values).
    /// On subsequent calls, performs SMPcLUfac (refactor in-place).
    /// Then solves with SMPcSolve.
    pub fn solve_complex(&mut self) -> Result<(), crate::error::SimError> {
        let n = self.size;
        if n == 0 { return Ok(()); }

        if self.needs_order {
            // Sync TRANSLATE maps on first ordering.
            if !self.translate_synced && self.next_int > 1 {
                for int_idx in 1..self.next_int {
                    let ext_idx = self.int_to_ext[int_idx];
                    self.matrix.int_to_ext_row[int_idx] = ext_idx;
                    self.matrix.int_to_ext_col[int_idx] = ext_idx;
                    self.matrix.ext_to_int_row[ext_idx] = int_idx;
                    self.matrix.ext_to_int_col[ext_idx] = int_idx;
                }
                self.translate_synced = true;
            }

            if !self.skip_preorder {
                self.matrix.mna_preorder();
            }
            self.skip_preorder = false;

            // order_and_factor — this uses the real values for pivot selection,
            // matching SMPcReorder which calls spOrderAndFactor with complex mode.
            // Since AC reuses the pivot order from the DC OP factorization,
            // and we've already ordered during DC OP, we do a complex refactor.
            // But on the first AC call, we need to order.
            sparse_rs::markowitz::order_and_factor_in_place(&mut self.matrix)
                .map_err(|_| crate::error::SimError::SingularMatrix(0))?;
            self.needs_order = false;
        }

        // Complex refactorization using the existing pivot ordering.
        // Matches SMPcLUfac → spSetComplex + spFactor → FactorComplexMatrix.
        sparse_rs::markowitz::factor_complex_in_place(&mut self.matrix)
            .map_err(|_| crate::error::SimError::SingularMatrix(0))?;

        // Complex solve: permute RHS → complex forward/back substitution → unpermute
        let mut b_re: Vec<f64> = self.rhs[1..=n].to_vec();
        let mut b_im: Vec<f64> = self.irhs[1..=n].to_vec();
        sparse_rs::markowitz::solve_complex_with_matrix(&self.matrix, &mut b_re, &mut b_im)
            .map_err(|_| crate::error::SimError::SingularMatrix(0))?;

        // Store results back
        self.rhs[0] = 0.0;
        self.irhs[0] = 0.0;
        for i in 0..n {
            self.rhs[i + 1] = b_re[i];
            self.irhs[i + 1] = b_im[i];
        }

        Ok(())
    }

    /// Swap imaginary rhs and irhs_old. Matches SWAP(CKTirhs, CKTirhsOld).
    pub fn swap_irhs(&mut self) {
        std::mem::swap(&mut self.irhs, &mut self.irhs_old);
    }

    /// Dump all matrix elements as flat [row, col, value, ...] triples in
    /// **external** coordinates. Used for matrix comparison in diverge-deep.
    ///
    /// This is called before solve() (before preorder), so we use the
    /// MNA-level int_to_ext map which is the same for rows and columns
    /// at this point. This matches what ngspice does: its trace fires
    /// after preorder, so ngspice uses IntToExtRowMap/IntToExtColMap
    /// (which may differ); we use the pre-preorder MNA mapping.
    pub fn dump_matrix_elements(&self) -> Vec<f64> {
        let n = self.size;
        eprintln!("DUMP_MATRIX int_to_ext[1..={}]: {:?}", n, &self.int_to_ext[1..=n]);
        let mut out = Vec::new();
        for col in 1..=n {
            let ext_col = self.int_to_ext[col] as f64;
            let mut idx = self.matrix.first_in_col[col];
            while idx != sparse_rs::markowitz::matrix::NONE {
                let el = self.matrix.el(idx);
                let ext_row = self.int_to_ext[el.row as usize] as f64;
                out.push(ext_row);
                out.push(ext_col);
                out.push(el.real);
                idx = el.next_in_col;
            }
        }
        out
    }

    /// Swap rhs and rhs_old. Matches SWAP(CKTrhs, CKTrhsOld).
    pub fn swap_rhs(&mut self) {
        std::mem::swap(&mut self.rhs, &mut self.rhs_old);
    }

    pub fn rhs_old_val(&self, eq: usize) -> f64 { self.rhs_old[eq] }
    pub fn rhs_val(&self, eq: usize) -> f64 { self.rhs[eq] }
    pub fn zero_ground(&mut self) { self.rhs[0] = 0.0; self.rhs_old[0] = 0.0; }

    /// Return the ext_to_int TRANSLATE map for parity checking.
    pub fn ext_to_int_map(&self) -> &[usize] { &self.ext_to_int[..self.size + 1] }

    /// Return the pivot permutation after factorization.
    /// For each elimination step i (0-based), returns the external row/col chosen.
    /// Only valid after at least one call to `solve()`.
    pub fn pivot_permutation(&self) -> (Vec<usize>, Vec<usize>) {
        let n = self.size;
        let mut rows = Vec::with_capacity(n);
        let mut cols = Vec::with_capacity(n);
        for i in 1..=n {
            rows.push(self.matrix.int_to_ext_row[i]);
            cols.push(self.matrix.int_to_ext_col[i]);
        }
        (rows, cols)
    }

    /// Ensure a diagonal element exists for the given external node number.
    /// Matches CKTic's SMPmakeElt(matrix, node->number, node->number).
    /// Returns the element handle.
    pub fn ensure_diag(&mut self, ext_node: usize) -> MatElt {
        self.make_element(ext_node, ext_node)
    }

    /// Zero all voltage-type elements in the given row, keeping current-type.
    /// Port of ZeroNoncurRow (cktload.c:183-201).
    ///
    /// Iterates all nodes: for each (row, col) that exists in the matrix,
    /// if the column node is a voltage type, zeros the element.
    /// Returns true if any current-type columns were found.
    pub fn zero_noncur_row(
        &mut self,
        ext_row: usize,
        nodes: &[crate::node::Node],
    ) -> bool {
        let mut has_currents = false;

        // Iterate all nodes (matching ngspice: for (n = nodes; n; n = n->next))
        for (ext_col, node) in nodes.iter().enumerate() {
            if ext_col == 0 { continue; } // skip ground

            // Look up element handle from cache (like SMPfindElt with create=0)
            let key = (ext_row as u32, ext_col as u32);
            if let Some(&handle) = self.elt_cache.get(&key) {
                if handle == NONE { continue; }
                if node.node_type == crate::node::NodeType::Current {
                    has_currents = true;
                } else {
                    // Zero voltage-type element
                    self.matrix.el_mut(handle).real = 0.0;
                }
            }
        }

        has_currents
    }

    /// Set a matrix element by handle to an absolute value (not additive).
    pub fn set_elt(&mut self, handle: MatElt, value: f64) {
        if handle == NONE { return; }
        self.matrix.el_mut(handle).real = value;
    }

    /// Get element handle from cache for (ext_row, ext_col).
    /// Returns NONE if not found.
    pub fn find_elt(&self, ext_row: usize, ext_col: usize) -> MatElt {
        let key = (ext_row as u32, ext_col as u32);
        self.elt_cache.get(&key).copied().unwrap_or(NONE)
    }
}
