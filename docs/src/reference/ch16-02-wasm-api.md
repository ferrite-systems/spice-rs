# WASM API

The `spice-rs-wasm` package exposes spice-rs to JavaScript via WebAssembly. It works in browsers and Node.js.

## Installation

Build from source with `wasm-pack`:

```bash
cd sim/spice-rs-wasm
wasm-pack build --target web
```

This produces a `pkg/` directory with `.wasm`, `.js`, and `.d.ts` files.

## SimulationEngine

All methods accept a SPICE netlist string and return a JSON string.

### Constructor

```js
const engine = new SimulationEngine();
```

### `dc_op(netlist) -> string`

Runs DC operating point analysis.

Returns:
```json
{
  "nodes": { "vdd": 3.3, "mid": 1.65 }
}
```

### `tran(netlist) -> string`

Runs transient analysis with full waveforms.

Returns:
```json
{
  "times": [0.0, 1e-9, 2e-9],
  "signals": { "v(out)": [0.0, 0.001, 0.003] },
  "names": ["v(out)", "v(in)"],
  "accepted": 1234,
  "rejected": 56
}
```

### `dc_sweep(netlist) -> string`

Runs DC parameter sweep.

Returns:
```json
{
  "sweep_values": [0.0, 0.1, 0.2],
  "signals": { "v(out)": [0.0, 0.05, 0.1] },
  "names": ["v(out)"]
}
```

### `ac(netlist) -> string`

Runs AC frequency sweep. Returns real/imaginary parts plus magnitude and phase.

Returns:
```json
{
  "frequencies": [100.0, 1000.0, 10000.0],
  "signals_re": { "v(out)": [0.99, 0.85, 0.15] },
  "signals_im": { "v(out)": [-0.01, -0.53, -0.99] },
  "signals_mag": { "v(out)": [0.99, 1.0, 1.0] },
  "signals_phase": { "v(out)": [-0.58, -31.9, -81.4] },
  "names": ["v(out)"]
}
```

### `simulate(netlist) -> string`

Auto-detects analysis type and runs it. Returns:

```json
{
  "analysis": "Op",
  "nodes": { "vdd": 3.3, "mid": 1.65 }
}
```

### `parse_nodes(netlist) -> string`

Returns the equation map (available signals) without running a simulation.

```json
[
  { "eq": 1, "name": "vdd", "node_type": "voltage" },
  { "eq": 2, "name": "mid", "node_type": "voltage" }
]
```

## Schematic rendering methods

### `kdl_to_svg(kdl) -> string`

Parses a KDL circuit description and returns an SVG string.

```js
const svg = engine.kdl_to_svg(kdlSource);
document.getElementById("schematic").innerHTML = svg;
```

### `kdl_to_spice(kdl) -> string`

Generates a SPICE netlist from a KDL circuit description.

Returns:
```json
{
  "netlist": "V1 vdd 0 DC 3.3\nR1 vdd mid 10K\n...",
  "annotation_nodes": ["mid"]
}
```

### `kdl_extract_params(kdl) -> string`

Extracts editable parameters from a KDL circuit.

Returns:
```json
[
  { "ref_des": "R1", "kind": "value", "current": "10K" },
  { "ref_des": "Vdd", "kind": "voltage", "current": "3.3" }
]
```

## Complete example

```js
import init, { SimulationEngine } from './pkg/spice_rs_wasm.js';

async function main() {
    await init();
    const engine = new SimulationEngine();

    // DC operating point
    const netlist = `
Voltage Divider
V1 vdd 0 DC 3.3
R1 vdd mid 10K
R2 mid 0 10K
.OP
.END
`;

    const result = JSON.parse(engine.dc_op(netlist));
    console.log("mid =", result.nodes.mid, "V");
    // mid = 1.65 V

    // Transient analysis
    const tranNetlist = `
RC Filter
V1 in 0 PULSE(0 1 0 1N 1N 5U 10U)
R1 in out 1K
C1 out 0 1N
.TRAN 10N 10U
.END
`;

    const tran = JSON.parse(engine.tran(tranNetlist));
    console.log("Timepoints:", tran.times.length);
    console.log("Signals:", tran.names);

    // Plot tran.times vs tran.signals["v(out)"]
}

main();
```

## Error handling

All methods throw a `JsError` on failure. The error message contains the spice-rs error string.

```js
try {
    engine.dc_op("invalid netlist");
} catch (e) {
    console.error("Simulation failed:", e.message);
}
```
