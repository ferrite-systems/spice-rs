---
title: Introduction
toc: true
---

# spice-rs

A faithful port of [ngspice](https://ngspice.sourceforge.io/) in Rust — with every simulation on this site running in your browser via WebAssembly.

```js
import {kdlToSvg, SimBuilder, setWasmUrl} from "./components/spice.js";
setWasmUrl(FileAttachment("./wasm/spice_rs_wasm_bg.wasm").href);
import {Resistance, Voltage, formatSpice, formatEE} from "./components/ee-inputs.js";
import {simPanel} from "./components/readout.js";
```

Try it — drag the resistor values and watch the simulated ${tex`V_{mid}`} update:

```js
const r1 = view(Resistance({label: "R1", value: 1000}));
const r2 = view(Resistance({label: "R2", value: 1000}));
```

```js
const dividerKdl = `circuit "Voltage Divider" {
    group "divider" {
        component "R1" type="resistor" { value "${formatSpice(r1)}"; port "1" net="vin"; port "2" net="vmid"; place col=0 row=0 }
        component "R2" type="resistor" { value "${formatSpice(r2)}"; port "1" net="vmid"; port "2" net="gnd"; place col=0 row=2 }
    }
    node "vin" role="supply" voltage="10" label="VDD"
    node "gnd" role="ground"
    node "vmid" label="Vmid"
}`;
const dividerSim = await SimBuilder.fromKdl(dividerKdl)
  .op()
  .measure("Vmid", "voltage", "vmid")
  .withSvg()
  .run();
display(simPanel(dividerSim));
```

---

## Why this project exists

spice-rs is an experiment in using LLMs to port a large, complex C codebase to Rust — and in discovering what it actually takes to make that work.

ngspice is roughly 500,000 lines of C — decades of numerical algorithms, device physics models, and solver heuristics accumulated since Berkeley SPICE in 1973. It is one of the most battle-tested scientific codebases in existence, and also one of the hardest to modify, extend, or embed in modern tooling. The code is deeply imperative, relies heavily on global state, and has almost no test coverage beyond "run a circuit and eyeball the waveform."

We wanted to know: can an LLM-assisted workflow faithfully translate this kind of code — not approximate it, not rewrite it from textbook equations, but produce a Rust implementation that matches ngspice at machine precision, device model by device model, timestep by timestep?

The answer is: yes, but not in the way you might expect. The LLM can read ngspice's C code and produce plausible Rust translations. But "plausible" is not "correct." Left to its own devices, the model consistently drifts toward textbook approximations, invents alternative approaches that seem reasonable, and misses the subtle implementation details — a sign convention buried in a macro, a node ordering assumption implicit in the setup function, a convergence heuristic that only matters for stiff circuits — that make the difference between "roughly right" and "bit-identical."

Getting to bit-identical required building an **evaluation harness** that could pinpoint exactly where and why the Rust output diverged from ngspice: which device, which parameter, which Newton-Raphson iteration, which matrix entry. That harness — not the LLM — is the core intellectual contribution of this project. It turned an intractable debugging problem ("the output is wrong somewhere") into a series of precise, mechanical fixes ("equation 7 has a sign error in the drain conductance stamp at NR iteration 3").

The pattern that emerged: the human designs the testing infrastructure and diagnoses root causes; the LLM does the high-volume translation and mechanical refactoring. Neither could do this alone. The LLM cannot reason about numerical correctness at the precision required, and a human cannot economically translate hundreds of thousands of lines of C by hand.

As of the latest eval run, spice-rs passes 199 of 226 validation circuits against ngspice at strict tolerances (abs=0.01, rel=0.01), with 176 of those bit-identical to ngspice output. The port covers resistors, capacitors, inductors, diodes, MOSFETs (Level 1/2/3 and BSIM3), BJTs, JFETs, coupled inductors, transmission lines, all source types, and all major analysis modes (.OP, .TRAN, .DC, .AC, .TF, .SENS, .PZ).

---

## How this site works

This is not a static textbook. Every circuit diagram, every simulation result, and every waveform plot on this site is **computed live in your browser**.

The site is built with [Observable Framework](https://observablehq.com/framework/), a reactive static-site generator for data-driven documents. Two systems work together:

### WASM-based simulator

The same Rust SPICE engine that passes the validation suite is compiled to WebAssembly and loaded client-side. There is no server — when you drag a slider and change a component value, the simulation re-runs entirely in your browser. A `SimBuilder` API separates circuit topology from simulation instrumentation — the same circuit can be analyzed as a DC operating point, AC frequency sweep, or transient step response without changing the circuit description. The builder generates a SPICE netlist internally, runs spice-rs in WASM, and returns structured results that [Observable Plot](https://observablehq.com/plot/) renders as interactive charts.

### Automated circuit rendering

Circuits are defined in a semantic format built on [KDL](https://kdl.dev), a general-purpose document language. The circuit definitions describe **topology** — what components exist, how they connect, and what role each plays. They carry more information than a SPICE netlist — component roles, group topologies, and placement hints enable the layout engine to produce readable schematics automatically. The engine reads the circuit definition, runs a topology-aware layout algorithm, and produces an SVG circuit diagram — all in WebAssembly. No pre-rendered images, no manual coordinates.

When you change a parameter, the circuit definition updates, the layout engine re-renders the SVG, the simulator re-runs, and the results are overlaid as annotations directly on the schematic. This **circuit definition** → **rendering** (SVG) → **simulation** (spice-rs) pipeline means every example on this site is a live, editable experiment.
