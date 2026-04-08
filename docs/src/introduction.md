# spice-rs

A faithful port of [ngspice](https://ngspice.sourceforge.io/) in Rust.

spice-rs is a complete SPICE circuit simulator — parser, MNA assembly, Newton-Raphson solver, DC/AC/transient analysis, and device models from diodes through BSIM4 — written in pure Rust with zero C dependencies. The sparse matrix solver ([sparse-rs](https://github.com/nickvdyck/sparse-rs)) is a standalone port of SuiteSparse KLU.

Every simulation on this site runs in your browser via WebAssembly. There is no server. The same Rust code that passes 200 validation circuits against ngspice is compiled to WASM and executes locally when you press **Run**.

---

## How to read this site

**[Part I — Learn SPICE](./learn/ch01-what-is-circuit-simulation.md)** builds understanding from first principles. It starts with Kirchhoff's laws and ends with BSIM3. Every chapter has interactive simulations you can edit and re-run. If you know electronics but have never looked inside a SPICE engine, start here.

**[Part II — Reference](./reference/ch13-netlist-syntax.md)** is the lookup section. Netlist syntax, device model parameter tables, simulation options, and the Rust/WASM API.

**[Part III — Internals](./internals/ch17-architecture.md)** documents how spice-rs was built: the porting methodology, architecture decisions, sparse solver algorithms, and the 223-circuit validation suite. If you're contributing to spice-rs or porting your own simulator, this is for you.

---

## Quick start

### Rust

```toml
[dependencies]
spice-rs = "0.1"
```

```rust
use spice_rs::{Circuit, SimConfig};

let netlist = "
V1 in 0 DC 5
R1 in out 1k
R2 out 0 1k
.OP
.END
";

let circuit = Circuit::from_netlist(netlist).unwrap();
let result = circuit.run(&SimConfig::default()).unwrap();
// result.node_voltage("out") ≈ 2.5
```

### Browser (WASM)

```js
import init, { SimulationEngine } from './spice_rs_wasm.js';

await init();
const engine = new SimulationEngine();
const result = engine.dc_op(`
  V1 in 0 DC 5
  R1 in out 1k
  R2 out 0 1k
  .OP
  .END
`);
console.log(result.nodes); // { "in": 5.0, "out": 2.5 }
```

---

## Status

- **200 passed**, 3 failed, 3 errors against ngspice reference
- Standard tolerance: abs=0.01, rel=0.01
- 176 circuits bit-identical with ngspice

| Feature | Status |
|---------|--------|
| DC operating point | Done |
| AC frequency sweep | Done |
| Transient analysis | Done |
| Sensitivity / TF / PZ | Done |
| Diode | Done |
| MOSFET Level 1/2/3 | Done |
| BSIM3v3 | Done |
| BSIM4 | In progress |
| BJT Gummel-Poon | Done |
| JFET | Done |
| SPICE netlist parser | Done |
| Sparse solver (KLU + Markowitz) | Done |
