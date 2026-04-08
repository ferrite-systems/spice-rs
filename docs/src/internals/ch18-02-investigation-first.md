# Investigation-First Method

Every subsystem in spice-rs was ported using the same five-step method. The method is designed to prevent the most common failure mode: writing Rust code that looks plausible but diverges from ngspice in subtle ways.

## The five steps

### 1. Read the ngspice C code

Read the actual source files. Not the comments, not the man page, not the SPICE3 user manual — the code. For a MOSFET model, this means reading `mos1load.c`, `mos1temp.c`, `mos1set.c`, and `mos1defs.h`. For the NR solver, this means reading `niiter.c`, `niditer.c`, and `cktop.c`.

Read it with a C debugger mindset: follow every pointer, understand every macro expansion, trace every control flow path. The ngspice source is at `reference/ngspice/` (a git submodule).

### 2. Document the algorithm in plain English

Before writing any Rust, write a plain-English description of what the C code does. This description lives in `sim/spice-rs/docs/investigations/` and covers:

- What data structures are used (and their ngspice names)
- What the main control flow is (loops, branches, mode checks)
- What numerical tricks are present (limiting, clamping, special cases)
- What the inputs and outputs are
- What ngspice variable names map to what physical quantities

This step catches misunderstandings early. If you can't explain what the C code does in English, you can't translate it to Rust correctly.

### 3. Identify key data structures and control flow

Map the C data structures to Rust equivalents:

| ngspice C | spice-rs Rust |
|-----------|---------------|
| `CKTcircuit` (topology) | `Circuit` |
| `CKTcircuit` (mutable state) | `SimState` |
| `MatrixFrame` + `Element` | `MarkowitzMatrix` + `Element` |
| `CKTstates[0..7]` | `StateVectors.states[0..7]` |
| `CKTrhs` / `CKTrhsOld` | `MnaSystem.rhs` / `MnaSystem.rhs_old` |
| `SPICEdev` function pointers | `Device` trait methods |
| `double*` element pointers | `MatElt` (u32 arena index) |

Map the control flow:

| ngspice function | spice-rs function |
|-----------------|-------------------|
| `CKTop` | `dc_operating_point()` |
| `NIiter` | `ni_iter()` |
| `DCtran` | `transient()` |
| `CKTload` | the `load()` loop in `ni_iter()` |
| `CKTtemp` | `circuit.temperature()` |
| `CKTsetup` | `circuit.setup()` |

### 4. Translate to Rust

Now write the Rust code. The translation should be recognizable to someone reading the C — same variable names, same structure, same order of operations. Add comments referencing the C file and line numbers.

Keep the translation mechanical. If the C code has a loop from 1 to N, the Rust code has a loop from 1 to N. If the C code checks `mode & MODEINITJCT`, the Rust code checks `mode.contains(MODEINITJCT)`. Don't refactor, don't optimize, don't "improve" until the translation matches ngspice output.

### 5. Validate against ngspice output

Run test circuits through both engines and compare. Start with the simplest circuit that exercises the subsystem:

- Porting a resistor? Test a single resistor divider.
- Porting a MOSFET? Test a single NMOS with fixed gate voltage.
- Porting the transient engine? Test an RC step response.

The comparison must be exact. For well-conditioned circuits (passives, simple devices), the results should be bit-identical. For complex circuits (high-gain, stiff), they must be within `abs=0.01, rel=0.01` — and if they're not, that's a bug in the port, not a tolerance problem.

## This is constrained translation, not engineering

The critical mindset shift: this is not greenfield Rust development. It is constrained translation. The C code is the specification. The Rust code is an implementation of that specification.

When something doesn't match:

1. **Don't hypothesize.** Don't reason about "maybe the NR loop is overshooting" or "maybe the integration order is wrong." Hypotheses are usually wrong and waste time.
2. **Instrument and run.** Add `eprintln!` tracing to both sides and compare the output step by step. The divergence point tells you exactly which line of code is wrong.
3. **Read more C.** The answer is always in the reference source. If your Rust code doesn't match, you missed something in the C. Go back and read it again, more carefully.

This approach produced 176 bit-identical circuits out of 224. The remaining divergences are in actively-ported subsystems (BSIM4) or known convergence edge cases.
