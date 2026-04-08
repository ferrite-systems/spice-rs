/// Breakpoint list — port of ngspice CKTbreaks (cktsetbk.c, cktclrbk.c).
///
/// A sorted array of future breakpoint times. Sources register waveform edges
/// (PULSE rise/fall, PWL corners) so the transient engine can hit them exactly.
///
/// Invariants:
/// - Always sorted ascending
/// - Always at least 2 elements: [0] = next breakpoint, last = final_time sentinel
/// - CKTbreaks[0] >= current time
#[derive(Debug)]
pub struct Breakpoints {
    times: Vec<f64>,
    min_break: f64,
}

impl Breakpoints {
    /// Create initial breakpoint list: [0.0, final_time]
    /// min_break = max_step * 5e-5 (ngspice: CKTminBreak = CKTmaxStep * 5e-5)
    pub fn new(final_time: f64, max_step: f64) -> Self {
        Self {
            times: vec![0.0, final_time],
            min_break: max_step * 5e-5,
        }
    }

    /// Next breakpoint (CKTbreaks[0]).
    pub fn next(&self) -> f64 {
        self.times[0]
    }

    /// Following breakpoint (CKTbreaks[1]).
    pub fn following(&self) -> f64 {
        if self.times.len() > 1 {
            self.times[1]
        } else {
            self.times[0]
        }
    }

    pub fn min_break(&self) -> f64 {
        self.min_break
    }

    /// Insert a breakpoint in sorted order — port of CKTsetBreak (cktsetbk.c:20-102).
    ///
    /// Merges breakpoints that are within min_break of each other.
    /// Rejects breakpoints at or before current_time.
    pub fn set(&mut self, time: f64, current_time: f64) {
        // Reject breakpoints at current time (cktsetbk.c:33-37)
        if (time - current_time).abs() < 1e-15 * time.abs().max(1.0) {
            return;
        }
        if current_time > time {
            return; // in the past
        }

        // Find insertion point (sorted order)
        for i in 0..self.times.len() {
            if self.times[i] > time {
                // Check merge with existing breakpoint above
                if (self.times[i] - time) <= self.min_break {
                    self.times[i] = time; // replace with earlier
                    return;
                }
                // Check merge with breakpoint below
                if i > 0 && (time - self.times[i - 1]) <= self.min_break {
                    return; // too close to previous, skip
                }
                // Insert here
                self.times.insert(i, time);
                return;
            }
        }

        // Beyond end — check merge with last
        if let Some(&last) = self.times.last() {
            if (time - last) <= self.min_break {
                return;
            }
        }
        self.times.push(time);
    }

    /// Remove the first breakpoint — port of CKTclrBreak (cktclrbk.c:18-38).
    ///
    /// Never shrinks below 2 elements.
    pub fn clear_first(&mut self, final_time: f64) {
        if self.times.len() > 2 {
            self.times.remove(0);
        } else {
            self.times[0] = self.times[1];
            self.times[1] = final_time;
        }
    }

    /// Clear breakpoints that are at or before current_time.
    /// Port of dctran.c XSPICE breakpoint clearing loop (lines 654-665).
    pub fn clear_past(&mut self, current_time: f64, final_time: f64) {
        while self.times[0] <= current_time + self.min_break
            && self.times[0] < final_time
        {
            self.clear_first(final_time);
        }
    }
}
