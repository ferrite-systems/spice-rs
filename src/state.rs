/// Device state vector storage — matches ngspice CKTstates[8] flat arrays.
///
/// Each device claims a contiguous offset range during setup. The offset
/// is assigned by bumping `num_states`. Devices then read/write states
/// using their base offset + per-state index.
///
/// History levels:
/// - state[0] = current values (being computed)
/// - state[1] = previous accepted timepoint
/// - state[2..7] = older history (for higher-order integration)
pub struct StateVectors {
    states: [Vec<f64>; 8],
    num_states: usize,
}

impl StateVectors {
    pub fn new() -> Self {
        Self {
            states: Default::default(),
            num_states: 0,
        }
    }

    /// Allocate `count` contiguous state slots, returning the base offset.
    /// Called during device setup (analogous to CKTnumStates accumulation).
    pub fn allocate(&mut self, count: usize) -> usize {
        let offset = self.num_states;
        self.num_states += count;
        offset
    }

    /// Finalize allocation — resize all history arrays.
    /// Called after all devices have allocated (analogous to CKTsetup post-loop).
    pub fn finalize(&mut self) {
        for s in &mut self.states {
            s.resize(self.num_states, 0.0);
        }
    }

    /// Total number of state variables allocated.
    pub fn len(&self) -> usize {
        self.num_states
    }

    /// Access state level `level` at `offset`.
    pub fn get(&self, level: usize, offset: usize) -> f64 {
        self.states[level][offset]
    }

    /// Set state level `level` at `offset`.
    pub fn set(&mut self, level: usize, offset: usize, value: f64) {
        self.states[level][offset] = value;
    }

    /// Mutable access to state0 (current values).
    pub fn state0(&self) -> &[f64] {
        &self.states[0]
    }

    pub fn state0_mut(&mut self) -> &mut [f64] {
        &mut self.states[0]
    }

    pub fn state1(&self) -> &[f64] {
        &self.states[1]
    }

    /// Zero all values in state0.
    pub fn zero_state0(&mut self) {
        for v in &mut self.states[0] {
            *v = 0.0;
        }
    }

    /// Copy all values from one history level to another.
    /// Matches ngspice's memcpy(CKTstateN, CKTstateM, ...).
    pub fn copy_level(&mut self, src: usize, dst: usize) {
        let data: Vec<f64> = self.states[src].clone();
        self.states[dst] = data;
    }

    /// Copy state0 to state1 — used by DC sweep on first convergence (dctrcurv.c:459).
    pub fn copy_state0_to_state1(&mut self) {
        let n = self.num_states;
        if n > 0 {
            let src: Vec<f64> = self.states[0][..n].to_vec();
            self.states[1][..n].copy_from_slice(&src);
        }
    }

    /// Rotate state vectors — pointer-style rotation matching ngspice dctran.c:742-750.
    ///
    /// ```text
    /// temp = states[max_order+1];
    /// for i in max_order..=0: states[i+1] = states[i];
    /// states[0] = temp;
    /// ```
    ///
    /// This is O(1) — swaps Vec ownership, doesn't copy data.
    pub fn rotate(&mut self, max_order: usize) {
        // Rotate: shift states[0..=max_order] up by one, wrap the top.
        // e.g., for max_order=1: state2 becomes state0, state0→state1, state1→state2
        let top = max_order + 1;
        if top < self.states.len() {
            // Rotate the slice [0..=top] such that index top moves to 0.
            self.states[..=top].rotate_right(1);
        }
    }
}
