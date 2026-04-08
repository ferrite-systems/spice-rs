# Case Study: Porting BSIM3v3

BSIM3v3 is the most complex device model in spice-rs. The ngspice source for the load function alone (`b3ld.c`) is approximately 5000 lines of C. The full model spans `b3ld.c` (load), `b3temp.c` (temperature), `b3set.c` (setup/defaults), and `b3v3def.h` (150+ model parameters).

**spice-rs source:** `sim/spice-rs/src/device/bsim3.rs` (2886 lines)
**ngspice source:** `reference/ngspice/src/spicelib/devices/bsim3/b3ld.c`, `b3temp.c`, `b3set.c`

## Strategy: function-by-function translation

The BSIM3 load function is too large to translate as a single unit. The approach:

1. **Identify logical blocks.** `b3ld.c` is structured as sequential blocks: voltage limiting, effective parameters (Vth, mobility, Rds), drain current (linear/saturation), output conductance, junction diodes, charge model (CAPMOD 1/2/3), integration, matrix stamps.

2. **Translate one block at a time.** After each block, run test circuits to verify correctness up to that point. This catches translation errors immediately rather than at the end.

3. **Test after each block.** A simple BSIM3 NMOS with fixed terminal voltages exercises each block. Compare DC operating point values (Vth, Ids, gm, gds) after each block is ported.

## Key challenge: 150+ model parameters

BSIM3v3 has over 150 model parameters (`b3v3def.h`). Many are interdependent — for example, `PCLM` (channel-length modulation parameter) affects `Rout`, which affects `gds`, which affects the DC operating point, which affects all transient results.

Each parameter has:
- A default value (defined in `b3set.c`)
- A "given" flag (whether the user specified it in the `.MODEL` line)
- Temperature dependence (computed in `b3temp.c`)
- Multiple places where it enters the equations (often in complex expressions)

### The parser problem

The most common "model accuracy" bugs in BSIM3 porting were actually **parser bugs**. The `.MODEL` line parser must:

1. Recognize all 150+ parameter names (some with non-obvious aliases)
2. Handle SPICE value suffixes (e.g., `1e-7` vs `100n`)
3. Set the "given" flag correctly (ungiven parameters use computed defaults)
4. Handle version-specific parameters (BSIM3v3.2 vs v3.3)

A single missing parameter (e.g., `K3` not being parsed) would cause `b3temp.c`'s default computation to kick in, giving a different value than ngspice (which received the parameter correctly). This looks like a model accuracy bug but is trivially fixed by adding the parameter to the parser.

The `--check-params` eval mode was built specifically for this problem: it extracts all parsed parameters from both engines and compares them, catching parser misses before they cascade into mysterious accuracy failures.

## Constants: use the BSIM3 values

BSIM3 defines its own physical constants that differ from NIST values:

```rust
const EPSOX: f64 = 3.453133e-11;    // not 3.45e-11 or eps0 * 3.9
const EPSSI: f64 = 1.03594e-10;     // not eps0 * 11.7
const CHARGE_Q: f64 = 1.60219e-19;  // not 1.602176634e-19
const KB: f64 = 1.3806226e-23;      // not 1.380649e-23
const PI: f64 = 3.141592654;        // not std::f64::consts::PI
```

These are the values the Berkeley BSIM team used for parameter extraction. Using "more accurate" NIST constants would shift every threshold voltage, every mobility, every capacitance by a small but measurable amount. The model was calibrated with these constants — use them.

## Current status

11 BSIM3 circuits pass the eval harness at standard tolerance (abs=0.01, rel=0.01). Coverage includes:

- Single NMOS/PMOS operating point
- CMOS inverter with BSIM3 models
- Common-source amplifier
- Differential pair
- Various geometry and bias conditions

The charge model (CAPMOD=2, the default) is fully ported, enabling transient simulation of BSIM3 circuits.

## Lessons learned

### Translate the defaults too

`b3set.c` contains the default parameter computation. Many parameters have conditional defaults: "if X was given and Y was not, compute Y from X." The Rust translation of these defaults must match exactly. Getting a default wrong has the same effect as a parser bug — the device sees a different parameter value.

### Watch for sign conventions

BSIM3 internally works with absolute values and applies sign corrections at the end based on NMOS/PMOS type. The `type` parameter (+1/-1) is multiplied into voltages at the beginning and currents at the end. Getting this wrong flips the sign of all terminal quantities for PMOS devices.

### Test with PMOS

NMOS and PMOS exercise different code paths due to the sign convention. A port that works for NMOS but fails for PMOS usually has a sign error in the type multiplication.
