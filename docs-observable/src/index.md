---
title: Introduction
toc: true
---

# spice-rs

This project is an experiment, an attempt at porting [ngspice](https://ngspice.sourceforge.io/) to Rust using LLMs.

`spice-rs` is a complete SPICE circuit simulator written in pure Rust with zero C dependencies, including `sparse-rs`, a Rust port of SuiteSparse KLU. The documentation here includes a WASM instance of the simulator running in the browser.

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

spice-rs is an experiment in using LLMs to port a complex codebase from C to Rust, and in discovering what it actually takes to make that work. The current port was produced using Claude Code with the Opus 4.6 model.

`ngspice` is more than 500,000 lines of C accumulated since Berkeley SPICE in 1973. The code is deeply imperative, sensitive to small numerical changes, and has almost no test coverage beyond "run a circuit and eyeball the waveform."

The question this project explores: can an LLM-assisted workflow faithfully translate this kind of code? Not approximate it, not rewrite it from textbook equations, but produce a Rust implementation that matches ngspice at machine precision, device model by device model, timestep by timestep.

The answer is yes, but not in the way one might expect. The LLM can read ngspice's C code and produce plausible Rust translations. But "plausible" is not "correct." Left to its own devices, Claude Code consistently drifts toward approximations or takes shortcuts that reshape the SPICE implementation. The impact of these alterations ranges from insignificant to drastic. However, with the development of a test framework that evaluated internal ngspice state against spice-rs output at every simulation step it was possible to detect and correct these discrepancies.

Prompting LLMs to replicate a complex project like `ngspice` was insufficient to produce a usable port. In every case Claude was either unaware of the divergences it produced or explained them away as expected, insurmountable differences. But when presented with test data that demonstrated the exact cause it corrected the code and produced an implementation with bit-identical results.

As of the latest eval run, spice-rs passes 199 of 226 validation circuits against ngspice at strict tolerances (abs=0.01, rel=0.01), with 176 of those bit-identical to ngspice output. The port covers resistors, capacitors, inductors, diodes, MOSFETs (Level 1/2/3 and BSIM3), BJTs, JFETs, coupled inductors, transmission lines, all source types, and all major analysis modes (.OP, .TRAN, .DC, .AC, .TF, .SENS, .PZ).

---

## How this site works

This is not a static textbook. Every circuit diagram, every simulation result, and every waveform plot on this site is **computed live in your browser**.

The site is built with [Observable Framework](https://observablehq.com/framework/), a reactive static-site generator for data-driven documents. Two systems work together:

### WASM-based simulator

The Rust port of `ngspice` is compiled to WebAssembly and loaded client-side. There is no server: when you drag a slider and change a component value, the simulation re-runs entirely in your browser. A `SimBuilder` API separates circuit topology from simulation instrumentation — the same circuit can be analyzed as a DC operating point, AC frequency sweep, or transient step response without changing the circuit description. The builder generates a SPICE netlist internally, runs spice-rs in WASM, and returns structured results that [Observable Plot](https://observablehq.com/plot/) renders as interactive charts.

### Automated circuit rendering

Circuits are defined using a custom KDL-derived circuit format. The circuit definitions describe **topology** — what components exist, how they connect, and what role each plays. They carry more information than a SPICE netlist — component roles, group topologies, and placement hints enable the layout engine to produce readable schematics automatically. The engine reads the circuit definition, runs a topology-aware layout algorithm, and produces an SVG circuit diagram — all in WebAssembly.

When you change a parameter, the circuit definition updates, the layout engine re-renders the SVG, the simulator re-runs, and the results are overlaid as annotations directly on the schematic. This **circuit definition** → **rendering** (SVG) → **simulation** (spice-rs) pipeline means every example on this site is a live, editable experiment.
