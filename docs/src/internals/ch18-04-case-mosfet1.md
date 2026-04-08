# Case Study: Porting MOSFET Level 1

The MOSFET Level 1 (Shichman-Hodges) model was one of the first nonlinear devices ported. It illustrates the porting method on a device with moderate complexity: DC I-V equations, voltage limiting, junction diodes, and Meyer charge model capacitances.

**spice-rs source:** `sim/spice-rs/src/device/mosfet1.rs` (1111 lines)
**ngspice source:** `reference/ngspice/src/spicelib/devices/mos1/mos1load.c`, `mos1temp.c`, `mos1set.c`

## Scope

The Level 1 model includes:

- **DC equations:** drain current (cutoff, linear, saturation), body effect, channel-length modulation
- **Junction diodes:** drain-bulk and source-bulk PN junctions with saturation current, junction potential
- **Temperature:** threshold voltage shift, mobility degradation, junction parameter scaling
- **Transient:** Meyer charge model (gate-source, gate-drain, gate-bulk capacitances), junction capacitances
- **Voltage limiting:** `DEVfetlim` for Vgs/Vds, `DEVpnjlim` for junction voltages

## Key challenge: Meyer charge model

The Meyer capacitance model computes gate charges (Qgs, Qgd, Qgb) as functions of the terminal voltages. The capacitances are voltage-dependent and change discontinuously at region boundaries (cutoff/linear/saturation). This creates numerical sensitivity — small voltage changes near a boundary cause large charge changes.

ngspice handles this with careful region detection and the `qmeyer` function (`mos1load.c`). The charge computation happens in a specific order: first compute Vgs, Von (threshold including body effect), Vdsat, then call `DEVqmeyer` to get the three charges based on the operating region.

The Rust translation preserves this exact order. Changing the order of operations (e.g., computing Vdsat before Von) would change which region is selected at boundary points and produce different charges.

## Validation results

Level 1 MOSFET circuits match ngspice at machine precision (~1e-14 relative error). This is the target for all well-conditioned device models. Example from the eval harness:

```
│ [L3] NMOS Level 1 DC                               │  PASS    │   7.105e-15 │   1.017e-14 │
│ [L3] NMOS Level 1 Body Effect                       │  PASS    │   1.421e-14 │   2.841e-14 │
│ [L5] CMOS Inverter                                  │  PASS    │   3.553e-15 │   2.367e-14 │
```

The errors are at the IEEE 754 double-precision floor. This confirms that the Rust code produces the same sequence of floating-point operations as the C code.

## Lessons learned

### Variable naming matters

ngspice uses `VTO` for the model parameter (flat-band threshold voltage), `vto` for the value after temperature adjustment, `Von` for the effective threshold including body effect, and `Vdsat` for the saturation voltage. These are all different quantities.

Early in the port, renaming `vto` to `vth` (a more "standard" name) caused confusion because `vth` could mean the model parameter, the temperature-adjusted value, or the body-effect-adjusted value depending on context. Keeping the ngspice names eliminated this ambiguity.

The rule: use the ngspice variable name unless there is a compelling reason not to. When translating `mos1load.c`, the Rust variable `vgs` corresponds to C `vgs`, `vds` to `vds`, `vbs` to `vbs`, `von` to `Von`, etc.

### The importance of mode flags

The `load()` function has radically different behavior depending on the mode flags:

- `MODEINITJCT`: Junction initialization. Set Vgs=Vds=Vbs=0 (or to initial conditions if provided). Skip normal computation.
- `MODEINITFIX`: Nodeset forcing is active. Use voltages from RHS, but with `von` set to `vgs` to force the device into a particular region.
- `MODEINITFLOAT`: Normal operation. Read voltages from previous solution, apply limiting, evaluate full model.
- `MODETRAN`: Transient mode. Compute charges and call `ni_integrate()` to get the companion model.
- `MODEINITSMSIG`: Small-signal initialization. Store operating-point conductances for AC analysis.

Getting these wrong produces silent failures: the DC point converges to a wrong value, or the transient starts from a wrong initial condition. The mode flag checks must match ngspice exactly.

### Parser validation

Before debugging any model accuracy issue, validate that all model parameters reached the device correctly. The eval harness's `--check-params` mode compares parsed parameters between spice-rs and ngspice. In the Level 1 port, a missing parser case for `NSS` (surface state density) caused incorrect threshold voltage computation. The `--check-params` check caught it immediately.
