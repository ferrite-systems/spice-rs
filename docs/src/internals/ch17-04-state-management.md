# State Management

Device state — capacitor charges, inductor fluxes, junction voltages, terminal currents — is stored in a flat arena with multiple history levels. This matches ngspice's `CKTstates[0..7]` arrays.

## StateVectors

**Source:** `sim/spice-rs/src/state.rs`

```rust
pub struct StateVectors {
    states: [Vec<f64>; 8],
    num_states: usize,
}
```

Eight parallel arrays, each of length `num_states`. Indexed by `(level, offset)`:

- `states[0]` — current values (being computed this NR iteration)
- `states[1]` — previous accepted timepoint
- `states[2..7]` — older history (for higher-order integration methods)

### Allocation

During `circuit.setup()`, each device calls `states.allocate(count)` to claim a contiguous block of slots:

```rust
// In Mosfet1::setup():
self.state_offset = states.allocate(MOS1_NUM_STATES);  // 17 slots
```

The allocator is a simple bump: it returns the current `num_states` and increments by `count`. After all devices have allocated, `states.finalize()` resizes all 8 arrays to the final size.

This matches ngspice's pattern where each `DEVsetup` increments `CKTnumStates`, and the flat `CKTstates[]` arrays are allocated in `CKTsetup` after all devices report.

### Access pattern

Devices read and write state using their base offset plus a per-state constant:

```rust
// MOSFET Level 1 state offsets
const ST_VGS: usize = 0;
const ST_VDS: usize = 1;
const ST_VBS: usize = 2;
const ST_QGS: usize = 7;   // gate-source charge
const ST_CQGS: usize = 8;  // gate-source current (derivative of charge)
// ...

// Reading previous terminal voltage:
let vgs_old = states.get(1, self.state_offset + ST_VGS);

// Writing current charge:
states.set(0, self.state_offset + ST_QGS, qgs);
```

The `(level, offset)` convention maps directly to ngspice's `*(ckt->CKTstateN + offset)` pattern.

### History rotation

Between accepted transient timesteps, the state vectors are rotated:

```rust
states.rotate(max_order);
```

For trapezoidal integration (order 2), this rotates `states[0..=2]`:
- `states[2]` (oldest) is recycled as the new `states[0]`
- The old `states[0]` moves to `states[1]`
- The old `states[1]` moves to `states[2]`

This is O(1) — it swaps `Vec` ownership without copying data. It matches ngspice's pointer rotation in `dctran.c:742-750`:

```c
temp = CKTstates[ckt->CKTmaxOrder + 1];
for (i = ckt->CKTmaxOrder; i >= 0; i--)
    CKTstates[i + 1] = CKTstates[i];
CKTstates[0] = temp;
```

## Numerical integration

**Source:** `sim/spice-rs/src/integration.rs`

The integration system converts stored charges to currents using the companion model. This is the bridge between device physics (which computes charges) and the circuit equation (which needs conductances and current sources).

### Integration coefficients: `ni_com_cof()`

Port of ngspice `NIcomCof` (`nicomcof.c`). Computes the `ag[0..6]` coefficients from the current timestep `delta`, the integration order, and the `xmu` parameter (0.5 for standard trapezoidal).

For **backward Euler** (order 1):
```
ag[0] = 1/delta
ag[1] = -1/delta
```

For **trapezoidal** (order 2):
```
ag[0] = 1 / (delta * (1 - xmu))
ag[1] = xmu / (1 - xmu)
```

### Integration: `ni_integrate()`

Port of ngspice `NIintegrate` (`niinteg.c`). Given the `ag` coefficients, a charge state offset (`qcap`), and the capacitance value, it computes the current and returns the companion model `(geq, ceq)`:

- **`geq`** = equivalent conductance to stamp on the matrix diagonal
- **`ceq`** = equivalent current source to stamp in the RHS

For trapezoidal:
```
i = -i_old * ag[1] + ag[0] * (q_new - q_old)
ceq = i - ag[0] * q_new
geq = ag[0] * C
```

The device stamps `geq` as a conductance between the terminals and `ceq` as a current source. This linearizes the energy-storage element for the current NR iteration.

### Convention

Charges and currents are stored at adjacent offsets:
- `qcap` = charge (e.g., gate-source charge Qgs)
- `qcap + 1` = current (derivative of charge, i.e., Igs)

The device computes the charge in `load()`, calls `ni_integrate()` to get `(geq, ceq)`, then stamps those values. The integration function writes the computed current back to `states[0][qcap + 1]`.

### Truncation error: `ckt_terr()`

Port of ngspice `CKTterr` (`cktterr.c`). Estimates the local truncation error for a charge state and returns the maximum safe timestep. The transient engine takes the minimum across all devices' truncation estimates and uses it to accept or reject the current step.

## State during DC analysis

During DC operating point, only `states[0]` and `states[1]` are used. `states[0]` holds current values; `states[1]` is initialized to zero (or to `states[0]` after the first DC convergence in a sweep). The integration system is not active — devices in DC mode compute steady-state conductances directly without the companion model.
