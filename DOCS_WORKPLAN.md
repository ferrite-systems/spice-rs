# spice-rs Documentation Site — Workplan

Interactive documentation site for spice-rs (and sparse-rs). The goal: a 3blue1brown-quality guide to how SPICE works, running real simulations in the browser, with a warm low-fi visual identity (Tufte for EDA).

---

## Table of Contents

The site has three layers: **Learn** (conceptual, visual, interactive), **Reference** (API, models, parameters), and **Internals** (porting process, architecture, eval methodology).

### Part I — Learn SPICE

Progressive guide from first principles to real circuits. Every section has an interactive simulation.

```
1. What Is Circuit Simulation?
   1.1  Kirchhoff's Laws — the two rules that govern everything
   1.2  From Schematic to Equations — how a circuit becomes math
   1.3  Your First Simulation — resistor divider (interactive)

2. Modified Nodal Analysis
   2.1  Node Voltages — ground, nodes, the solution vector
   2.2  Conductance Stamps — how a resistor becomes a matrix entry
   2.3  Voltage Sources — why MNA needs branch currents
   2.4  Building the Matrix — assembling Gx = b (interactive stamp visualizer)
   2.5  Solving the System — what LU factorization does (links to sparse-rs)

3. DC Operating Point
   3.1  Linear Circuits — direct solve, one shot
   3.2  Nonlinear Circuits — why resistors aren't enough
   3.3  Newton-Raphson Iteration — linearize, solve, repeat (animated)
   3.4  Convergence — when NR gets stuck, and what to do about it
   3.5  Gmin Stepping & Source Stepping — convergence aids (interactive)

4. The Diode
   4.1  Shockley Equation — I = Is(e^(V/Vt) - 1) (interactive I-V curve)
   4.2  Linearization — companion model at each NR iteration
   4.3  Voltage Limiting — why you can't just exponentiate blindly
   4.4  Junction Capacitance — depletion and diffusion charge
   4.5  Circuits: Half-Wave Rectifier, Bridge Rectifier (interactive)

5. The MOSFET
   5.1  Three Regions — cutoff, linear, saturation (interactive I-V family)
   5.2  Level 1 (Shichman-Hodges) — the square-law model
   5.3  Body Effect — substrate bias and threshold shift
   5.4  Capacitances — gate oxide, overlap, junction (Meyer model)
   5.5  Level 2 & 3 — velocity saturation, narrow channel effects
   5.6  BSIM3 — the industry standard (parameter groups, key equations)
   5.7  BSIM4 — modern extensions (overview)
   5.8  Circuits: CMOS Inverter, NAND Gate, Ring Oscillator (interactive)

6. The BJT
   6.1  Gummel-Poon Model — transport current and base charge
   6.2  Early Effect — output conductance
   6.3  Parasitic Resistances & Capacitances
   6.4  Circuits: Common-Emitter Amp, Differential Pair (interactive)

7. The JFET
   7.1  Pinch-Off Model — depletion-mode FET
   7.2  Circuits: JFET Amplifier (interactive)

8. AC Analysis
   8.1  Small-Signal Linearization — operating point + perturbation
   8.2  Complex Impedance Matrix — frequency-domain MNA
   8.3  Bode Plots — magnitude and phase (interactive)
   8.4  Circuits: RC Lowpass, RL Highpass, RLC Resonance (interactive)

9. Transient Analysis
   9.1  Numerical Integration — turning differential equations into stamps
   9.2  Trapezoidal Rule — the default method
   9.3  Gear Methods — for stiff circuits
   9.4  Timestep Control — LTE and how the simulator chooses dt
   9.5  Breakpoints — handling abrupt source transitions
   9.6  Circuits: RC Charging, MOSFET Switching, Oscillator (interactive)

10. Sources & Waveforms
    10.1  DC, AC, Pulse, Sine, PWL, Exponential
    10.2  Dependent Sources — VCVS, VCCS, CCVS, CCCS
    10.3  Transmission Lines

11. Reactive Elements
    11.1  Capacitors — charge integration, initial conditions
    11.2  Inductors — flux integration, initial conditions
    11.3  Mutual Inductors — coupled flux
    11.4  Circuits: RLC Ringing, Coupled Inductors (interactive)

12. Advanced Analysis
    12.1  Sensitivity — dOutput/dParameter
    12.2  Transfer Function — gain and impedance
    12.3  Pole-Zero — stability analysis
```

### Part II — Reference

```
13. SPICE Netlist Syntax
    13.1  Netlist Format — title, elements, commands, .END
    13.2  Device Statements — R, C, L, V, I, D, M, Q, J, K, T, E, G, F, H
    13.3  Analysis Commands — .OP, .DC, .AC, .TRAN, .SENS, .TF, .PZ
    13.4  Control Statements — .MODEL, .SUBCKT, .PARAM, .OPTIONS, .INCLUDE, .LIB, .IC

14. Device Model Reference
    14.1  Diode — 15 parameters (IS, N, RS, CJO, VJ, M, TT, BV, IBV, ...)
    14.2  MOSFET Level 1 — parameter table, equations, defaults
    14.3  MOSFET Level 2 — extensions over Level 1
    14.4  MOSFET Level 3 — extensions over Level 2
    14.5  BSIM3v3 — 150+ parameters organized by group
    14.6  BSIM4 — 200+ parameters organized by group
    14.7  BJT (Gummel-Poon) — parameter table, equations
    14.8  JFET — parameter table, equations

15. Simulation Options (.OPTIONS)
    15.1  Convergence — ABSTOL, RELTOL, VNTOL, ITL1-ITL6, GMIN
    15.2  Transient — METHOD, MAXORD, TRTOL, CHGTOL
    15.3  General — TEMP, TNOM, PIVTOL, PIVREL

16. API Reference
    16.1  spice-rs Rust API — Circuit, SimConfig, analysis entry points
    16.2  spice-rs WASM API — JavaScript bindings, playground protocol
    16.3  sparse-rs API — KLU pipeline, Markowitz pipeline, CscMatrix
```

### Part III — Internals

```
17. Architecture
    17.1  Crate Structure — spice-rs, sparse-rs, how they compose
    17.2  Data Flow — netlist string → parse → circuit → MNA → solve → results
    17.3  Device Trait — how devices stamp into the matrix
    17.4  State Management — solution vectors, history, integration state

18. The Porting Process
    18.1  Philosophy — port, don't approximate
    18.2  Investigation-First — read C, document, then translate
    18.3  The Eval Harness — how we validate against ngspice
    18.4  Tolerance Policy — abs=0.01, rel=0.01, no exceptions
    18.5  Case Study: MOSFET Level 1 — from ngspice C to Rust
    18.6  Case Study: BSIM3 — porting a 5000-line model
    18.7  Case Study: Markowitz Pivot — arena-based linked lists in Rust

19. sparse-rs Internals
    19.1  Why Two Backends — KLU vs Markowitz, history and tradeoffs
    19.2  KLU Deep Dive — BTF → AMD → Gilbert-Peierls LU
    19.3  Markowitz Deep Dive — pivot cascade, arena matrix, 1-indexed arrays
    19.4  Benchmarks — sparse-rs vs SuiteSparse C

20. Validation & Eval
    20.1  The 223 Test Circuits — inventory, categories, what they cover
    20.2  Eval Reports — reading a divergence report
    20.3  Trace Export — point-by-point waveform comparison
    20.4  Current Status — pass/fail/bit-identical breakdown
```

---

## Circuit & Model Inventory for Docs

### Featured Interactive Circuits (Part I)

Each of these gets a live WASM simulation in the docs with editable parameters:

| # | Circuit | Section | Concept Demonstrated | Components |
|---|---------|---------|---------------------|------------|
| 1 | Resistor Divider | 1.3 | First simulation, Ohm's law | 2R, 1V |
| 2 | MNA Stamp Visualizer | 2.4 | Matrix assembly | R, V, I (selectable) |
| 3 | RC DC Operating Point | 3.1 | Linear solve | 1R, 1C, 1V |
| 4 | Diode Forward Bias | 3.3 | Newton-Raphson iteration | 1R, 1D, 1V |
| 5 | Diode I-V Curve | 4.1 | Shockley equation | 1D (parameter sweep) |
| 6 | Half-Wave Rectifier | 4.5 | Diode switching | 1R, 1D, 1V(sin) |
| 7 | Diode Bridge | 4.5 | Full-wave rectification | 4D, 1R, 1V(sin) |
| 8 | NMOS I-V Family | 5.1 | Three regions of operation | 1M (VGS/VDS sweep) |
| 9 | CMOS Inverter DC | 5.8 | Complementary MOS | 2M, 1V |
| 10 | CMOS Inverter Tran | 5.8 | Digital switching | 2M, 1V(pulse) |
| 11 | CMOS NAND | 5.8 | Logic gate | 4M, 1V(pulse) |
| 12 | NPN Common-Emitter | 6.4 | BJT amplifier | 1Q, 3R, 2V |
| 13 | BJT Diff Pair | 6.4 | Differential amplifier | 2Q, 5R, 3V |
| 14 | JFET Amplifier | 7.2 | Depletion-mode FET | 1J, 2R, 2V |
| 15 | RC Lowpass Bode | 8.4 | Frequency response | 1R, 1C, 1V(ac) |
| 16 | RLC Resonance | 8.4 | Resonant peak | 1R, 1L, 1C, 1V(ac) |
| 17 | RC Step Response | 9.6 | Transient charging | 1R, 1C, 1V(pulse) |
| 18 | MOSFET Switching | 9.6 | Transient with nonlinear device | 2M, 1R, 1V(pulse) |
| 19 | RLC Ringing | 11.4 | Underdamped oscillation | 1R, 1L, 1C, 1V(pulse) |
| 20 | Coupled Inductors | 11.4 | Mutual coupling | 2L, 1K, 2R, 1V(ac) |

### Device Models Documented (Part II)

| Device | Parameters | Key Equations | From Eval Suite |
|--------|-----------|---------------|-----------------|
| Diode | 15 | Shockley, junction cap, BV | 13 circuits |
| MOSFET L1 | ~20 | Square-law I-V, Meyer caps | 8 circuits |
| MOSFET L2 | ~25 | Velocity saturation extensions | 3 circuits |
| MOSFET L3 | ~25 | Narrow-channel, mobility | 3 circuits |
| BSIM3v3 | 150+ | DIBL, CLM, SCE, QM effects | 11 circuits |
| BSIM4 | 200+ | FinFET extensions | 10 circuits |
| BJT (GP) | ~40 | Gummel-Poon transport, Early | 13 circuits |
| JFET | ~15 | Pinch-off, Pittman model | 3 circuits |

### Eval Suite Coverage Map (Part III)

| Category | Count | Analysis Types |
|----------|-------|----------------|
| Single passive (L1) | 5 | DC |
| Passive combos (L2) | 6 | AC, TRAN |
| Single nonlinear (L3) | 10 | DC |
| NR stress tests (L4) | 6 | DC sweep |
| Device interactions (L5) | 5 | DC |
| Full circuits (L6+) | 191 | DC, AC, TRAN |
| **Total** | **223** | |

---

## Workstreams

Three parallel workstreams plus content authoring. Each can progress independently after W0 scaffolding.

### W0 — Scaffold (sequential, do first)

```
W0.1  Choose doc framework
      - mdbook (Rust-native, simple, extensible via preprocessors)
      - mdbook-admonish for callouts, mdbook-mermaid for diagrams
      - Custom preprocessor for <spice-sim> blocks → WASM widget embed
      - Directory: sim/spice-rs/docs/

W0.2  Workspace integration
      - `cargo docs` alias builds the mdbook site
      - CI builds docs on push, deploys to GitHub Pages
      - Rustdoc for API reference (linked from mdbook)

W0.3  Create directory structure
      sim/spice-rs/docs/
        book.toml              # mdbook config
        src/
          SUMMARY.md           # TOC
          learn/               # Part I chapters
          reference/           # Part II chapters
          internals/           # Part III chapters
        theme/                 # Custom CSS, fonts, colors
        preprocessor/          # Custom mdbook preprocessor (spice-sim blocks)
        wasm/                  # WASM build artifacts + JS runtime
        components/            # SVG component library + rendering JS

W0.4  Initial deployment
      - Bare site with TOC, one placeholder chapter, custom theme
      - Deployed to GitHub Pages
      - Proves the pipeline works end-to-end
```

### W1 — WASM Simulation Runtime

Compile spice-rs to WASM. Embed interactive simulations in doc pages.

```
W1.1  WASM build target
      - Feature-gate file I/O (parser .INCLUDE/.LIB becomes no-op in WASM)
      - Add wasm-bindgen dependency behind `wasm` feature flag
      - Verify spice-rs + sparse-rs compile to wasm32-unknown-unknown
      - Build with wasm-pack (--target web)

W1.2  JavaScript API
      - SimulationEngine class wrapping spice-rs
      - Methods:
          simulate(netlist: string) → SimResult
          dc_op(netlist) → { nodes: Map<string, number>, branches: Map<string, number> }
          ac(netlist) → { freqs: Float64Array, nodes: Map<string, Complex[]> }
          tran(netlist) → { times: Float64Array, nodes: Map<string, Float64Array> }
      - Error reporting (parse errors, convergence failures) as structured objects
      - Memory management (free results, reuse engine instance)

W1.3  Simulation widget
      - Web component: <spice-sim> with attributes:
          netlist="..."        # inline or file reference
          analysis="dc|ac|tran"
          outputs="v(out),i(v1)"
          editable="R1,C1"    # which parameters the user can tweak
      - Layout: netlist editor (left) + results plot (right)
      - Netlist editor: syntax-highlighted, editable, run-on-change
      - Plot: lightweight (Canvas 2D or SVG), no heavy charting lib
      - Responsive: stacks vertically on mobile

W1.4  mdbook preprocessor
      - Scan markdown for ```spice-sim fenced blocks
      - Replace with <spice-sim> web component + bundled netlist
      - Example markdown:
        ```spice-sim
        analysis: tran
        outputs: v(out)
        editable: R1, C1
        ---
        V1 in 0 DC 0 PULSE(0 5 0 1n 1n 0.5m 1m)
        R1 in out 1k
        C1 out 0 1u
        .TRAN 5m
        .END
        ```
      - Preprocessor emits HTML with <spice-sim> element and inline netlist

W1.5  Performance budget
      - WASM binary size target: < 500KB gzipped
      - Simulation latency: < 100ms for typical doc circuits
      - Lazy-load: WASM fetched on first interaction, not on page load
```

### W2 — Visual Design Language

Component and circuit rendering for inline diagrams. Warm, minimal, Tufte-inspired.

```
W2.1  Design identity
      - Palette: warm paper background (#faf4e8), dark brown ink (#3b2f20),
        accent copper (#b87333), signal blue (#4a6fa5), muted red (#a04040)
      - Typography: Serif body (ET Book / Crimson Pro), monospace code (Iosevka)
      - Principles: Tufte-style margin notes, high data-ink ratio,
        no chartjunk, components drawn with care not flash
      - Layout: wide content column, margin notes for annotations

W2.2  Component symbol library (SVG)
      - Resistor (zigzag, US style)
      - Capacitor (parallel plates)
      - Inductor (coil)
      - Voltage source (circle +/-)
      - Current source (circle with arrow)
      - Diode (triangle + bar)
      - MOSFET (NMOS/PMOS with gate, body)
      - BJT (NPN/PNP with arrow)
      - JFET (N/P channel)
      - Ground symbol
      - Wire / node / junction dot
      - Op-amp (triangle)
      - Each symbol: ~40×40px base, clean strokes, warm ink color
      - Delivered as individual SVGs + a combined sprite sheet

W2.3  Circuit diagram renderer
      - Input: declarative circuit description (JSON or inline DSL)
      - Output: SVG rendered inline in the document
      - Features:
          Schematic-style layout (orthogonal wires)
          Component labels (R1 = 1kΩ) in serif font
          Node voltage annotations (from simulation results)
          Current flow arrows (optional, animated)
          Highlighted paths for explanation (e.g., "current flows here")
      - Implementation: lightweight JS, no heavy framework
      - Responsive: scales with viewport

W2.4  Equation rendering
      - KaTeX for inline and display math
      - Styled to match serif body text
      - Key equations get "equation cards" — boxed, numbered, margin-referenced

W2.5  Plot styling
      - Consistent with design identity (warm palette, serif axis labels)
      - Axis: brown ink, thin lines, Tufte-style range frames (no box)
      - Grid: very faint, optional
      - Traces: copper for primary signal, blue for secondary, muted red for reference
      - Labels: direct on trace (no legend box when possible)
      - Interactive: hover for value readout, zoom on drag

W2.6  Animation framework
      - For NR iteration visualization: show matrix values updating
      - For transient stepping: animate waveform drawing left-to-right
      - For current flow: animated dashes along wires
      - Simple CSS + requestAnimationFrame, no animation library
      - All animations optional (reduced-motion respected)

W2.7  Matrix visualizer
      - Render MNA matrix as a grid with colored cells
      - Show stamp contributions per component (highlight which device
        contributed which entry)
      - Animate: add components one by one, watch matrix fill in
      - Sparse structure: show zeros as empty, nonzeros as filled
      - For sparse-rs: show pivot sequence, fill-in, BTF blocks
```

### W3 — Content Authoring

Write the actual chapters. Depends on W0 scaffold; can start before W1/W2 are complete (use placeholder widgets).

```
Phase A — Core Loop (chapters 1-4)
      These chapters teach the fundamental simulation loop.
      Target: someone who knows electronics but not SPICE internals.

      1. What Is Circuit Simulation? — motivation, KCL/KVL recap
      2. Modified Nodal Analysis — stamps, matrix, solve
      3. DC Operating Point — linear solve, NR for nonlinear
      4. The Diode — first real device model, companion model concept

      Interactive: resistor divider sim, stamp visualizer,
      NR convergence animation, diode I-V explorer

Phase B — Device Models (chapters 5-7)
      MOSFET (L1 through BSIM overview), BJT, JFET.
      Each chapter: physics → equations → SPICE implementation → interactive circuit.

Phase C — Analysis Types (chapters 8-12)
      AC, transient, sources, reactive elements, advanced analysis.
      Heavy use of interactive plots (Bode, waveforms).

Phase D — Reference (chapters 13-16)
      Netlist syntax, model parameter tables, simulation options, API docs.
      Mostly text + tables, less interactive.

Phase E — Internals (chapters 17-20)
      Architecture, porting methodology, sparse-rs deep dive, eval suite.
      Target audience: contributors, other Rust SPICE porters.
```

---

## Iteration Plan

### Sprint 0 — Foundation (W0 + W1.1 + W2.1)
- [ ] mdbook scaffold with custom theme
- [ ] spice-rs compiles to WASM (proof-of-concept)
- [ ] Design identity locked (palette, fonts, one sample page mockup)
- [ ] One chapter drafted (ch. 1 or 2) with placeholder widgets
- **Exit criteria:** deployed site with custom theme and one real chapter

### Sprint 1 — First Interactive Chapter (W1.2-W1.4 + W2.2-W2.3 + W3 Phase A ch.1-2)
- [ ] JS API for dc_op analysis
- [ ] <spice-sim> web component (basic: netlist + text results)
- [ ] Component SVG library (R, C, V, wire, ground)
- [ ] Circuit diagram renderer (basic: series/parallel layouts)
- [ ] Chapters 1-2 written with live sims and diagrams

### Sprint 2 — NR & Diode (W1.3 polish + W2.5-W2.6 + W3 Phase A ch.3-4)
- [ ] Plot rendering (Bode, waveform, I-V curves)
- [ ] NR iteration animation
- [ ] Diode SVG symbol
- [ ] Chapters 3-4 written

### Sprint 3 — Devices (W2.2 complete + W3 Phase B)
- [ ] Full component symbol library
- [ ] MOSFET, BJT, JFET chapters
- [ ] I-V family plot widget
- [ ] AC + transient WASM API

### Sprint 4 — Analysis & Reference (W1 complete + W3 Phase C-D)
- [ ] AC analysis chapter with Bode plots
- [ ] Transient chapter with waveform explorer
- [ ] Netlist reference, model parameter tables
- [ ] API docs (Rust + WASM)

### Sprint 5 — Internals & Polish (W2.7 + W3 Phase E)
- [ ] Matrix visualizer
- [ ] Sparse-rs deep dive with pivot animation
- [ ] Porting process case studies
- [ ] Eval suite documentation
- [ ] Cross-browser testing, performance optimization

---

## Technical Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Doc framework | mdbook | Rust-native, simple, extensible, good enough |
| WASM target | wasm-pack + wasm-bindgen | Standard Rust→WASM pipeline |
| Interactivity | Web Components (<spice-sim>) | Framework-agnostic, encapsulated, works in mdbook |
| Plotting | Canvas 2D (custom) | Lightweight, styled to match design identity, no dep |
| Circuit diagrams | SVG (custom renderer) | Scalable, styleable, inline in DOM |
| Math | KaTeX | Fast, server-side pre-render option, good font |
| Fonts | ET Book (body), Iosevka (code) | Tufte lineage, clean monospace |
| Preprocessor | mdbook preprocessor (Rust binary) | Runs at build time, fast, typed |

## Non-Goals (for now)

- Schematic editor / drag-and-drop circuit builder
- Netlist file upload / arbitrary user circuits
- Multi-page simulation (subcircuits, .INCLUDE chains)
- Mobile-native app
- Video content
