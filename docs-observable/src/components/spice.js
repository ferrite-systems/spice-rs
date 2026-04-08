// spice-rs WASM engine wrapper for Observable Framework
import initWasm, { SimulationEngine } from "./spice_rs_wasm.js";

let engine = null;
let initPromise = null;
let wasmUrl = null;

// Must be called before any simulation, with the URL from FileAttachment.
export function setWasmUrl(url) { wasmUrl = url; }

async function ensureEngine() {
  if (engine) return engine;
  if (!initPromise) {
    initPromise = (async () => {
      if (!wasmUrl) throw new Error("Call setWasmUrl() with FileAttachment URL before using spice.js");
      const response = await fetch(wasmUrl);
      const bytes = await response.arrayBuffer();
      await initWasm({module_or_path: bytes});
      engine = new SimulationEngine();
      return engine;
    })();
  }
  return initPromise;
}

export async function kdlToSvg(kdl) {
  const eng = await ensureEngine();
  return eng.kdl_to_svg(kdl);
}

export async function kdlToSpice(kdl) {
  const eng = await ensureEngine();
  return JSON.parse(eng.kdl_to_spice(kdl));
}

export async function extractParams(kdl) {
  const eng = await ensureEngine();
  return JSON.parse(eng.kdl_extract_params(kdl));
}

export async function simulate(netlist) {
  const eng = await ensureEngine();
  return JSON.parse(eng.dc_op(netlist));
}

// Run raw SPICE netlist through transient analysis
export async function runTran(netlist) {
  const eng = await ensureEngine();
  return JSON.parse(eng.tran(netlist));
}

// Run raw SPICE netlist through AC analysis
export async function runAc(netlist) {
  const eng = await ensureEngine();
  return JSON.parse(eng.ac(netlist));
}

// Run raw SPICE netlist through DC sweep
export async function runDcSweep(netlist) {
  const eng = await ensureEngine();
  return JSON.parse(eng.dc_sweep(netlist));
}

// Combined: render SVG + simulate + return structured results
// Returns { svg, measures: [{label, value, unit, kind}], netlist }
export async function simulateCircuit(kdl) {
  const eng = await ensureEngine();
  return JSON.parse(eng.simulate_circuit(kdl));
}

// ── SimBuilder: fluent API for instrumented simulation ──
//
// Separates circuit topology (KDL) from simulation instrumentation.
//
// Usage:
//   const sim = await SimBuilder.fromKdl(kdl)
//     .pulse("V1", {v1: 0, v2: 5, tr: "1n", pw: "1m", per: "2m"})
//     .tran({step: "1u", stop: "5m"})
//     .measure("Vout", "voltage", "vout")
//     .run();
//
//   sim.analysis === "tran"
//   sim.times → [0, 1e-6, ...]
//   sim.signals["v(vout)"] → [0, 0.1, ...]
//   sim.measures → [{label: "Vout", value: 3.2, unit: "V"}]

export class SimBuilder {
  #kdl;
  #sources = {};
  #analysis = {op: {}};
  #ic = {};
  #measures = [];
  #renderSvg = false;

  constructor(kdl) { this.#kdl = kdl; }
  static fromKdl(kdl) { return new SimBuilder(kdl); }

  // Source excitations
  dc(refDes, voltage) { this.#sources[refDes] = {dc: voltage}; return this; }

  pulse(refDes, {v1 = 0, v2, td = 0, tr = 1e-9, tf = 1e-9, pw, per}) {
    this.#sources[refDes] = {pulse: {v1, v2: parseEE(v2), td: parseEE(td), tr: parseEE(tr), tf: parseEE(tf), pw: parseEE(pw), per: parseEE(per)}};
    return this;
  }

  sine(refDes, {vo = 0, va, freq, td = 0, theta = 0, phase = 0}) {
    this.#sources[refDes] = {sine: {vo, va: parseEE(va), freq: parseEE(freq), td: parseEE(td), theta, phase}};
    return this;
  }

  acSource(refDes, {dc = 0, mag = 1, phase = 0}) {
    this.#sources[refDes] = {ac: {dc, mag, phase}};
    return this;
  }

  // Analysis type
  op() { this.#analysis = "op"; return this; }

  tran({step, stop, uic = false}) {
    this.#analysis = {tran: {step: parseEE(step), stop: parseEE(stop), uic}};
    return this;
  }

  dcSweep({src, start, stop, step}) {
    this.#analysis = {dc_sweep: {src, start, stop, step}};
    return this;
  }

  acSweep({sweep = "dec", npts = 20, fstart, fstop}) {
    this.#analysis = {ac: {sweep, npts, fstart: parseEE(fstart), fstop: parseEE(fstop)}};
    return this;
  }

  // Initial conditions
  setIc(node, value) { this.#ic[node] = value; return this; }

  // Measurements
  measure(label, kind, target) {
    if (kind === "voltage") this.#measures.push({label, kind: "voltage", net: target});
    else if (kind === "current") this.#measures.push({label, kind: "current", component: target});
    else if (kind === "power") this.#measures.push({label, kind: "power", component: target});
    return this;
  }

  // SVG rendering
  withSvg() { this.#renderSvg = true; return this; }

  // Execute
  async run() {
    const eng = await ensureEngine();
    const config = JSON.stringify({
      kdl: this.#kdl,
      sources: this.#sources,
      analysis: this.#analysis,
      ic: this.#ic,
      measures: this.#measures,
      render_svg: this.#renderSvg,
    });
    return JSON.parse(eng.simulate_builder(config));
  }
}

// Parse engineering notation: "1u" → 1e-6, "10k" → 10000
function parseEE(v) {
  if (typeof v === "number") return v;
  const s = String(v).trim();
  const suffixes = {T: 1e12, G: 1e9, meg: 1e6, Meg: 1e6, k: 1e3, K: 1e3, m: 1e-3, u: 1e-6, n: 1e-9, p: 1e-12, f: 1e-15};
  for (const [suf, mult] of Object.entries(suffixes)) {
    if (s.endsWith(suf)) return parseFloat(s.slice(0, -suf.length)) * mult;
  }
  return parseFloat(s);
}
