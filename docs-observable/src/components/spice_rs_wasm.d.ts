/* tslint:disable */
/* eslint-disable */

/**
 * The main simulation engine exposed to JavaScript.
 *
 * Usage from JS:
 * ```js
 * const engine = new SimulationEngine();
 * const result = JSON.parse(engine.dc_op("V1 in 0 DC 5\nR1 in 0 1k\n.OP\n.END"));
 * console.log(result.nodes);  // { "in": 5.0 }
 * ```
 */
export class SimulationEngine {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Run AC frequency sweep analysis.
     *
     * Returns JSON:
     * ```json
     * {
     *   "frequencies": [100.0, 1000.0, ...],
     *   "signals_re": { "v(out)": [...], ... },
     *   "signals_im": { "v(out)": [...], ... },
     *   "signals_mag": { "v(out)": [...], ... },
     *   "signals_phase": { "v(out)": [...], ... },
     *   "names": ["v(out)", ...]
     * }
     * ```
     */
    ac(netlist: string): string;
    /**
     * Run DC operating point analysis.
     *
     * Returns JSON: `{ "nodes": { "nodename": value, ... } }`
     */
    dc_op(netlist: string): string;
    /**
     * Run DC sweep analysis, returning sweep waveforms.
     *
     * Returns JSON:
     * ```json
     * {
     *   "sweep_values": [0.0, 0.1, ...],
     *   "signals": { "v(out)": [0.0, 0.05, ...], ... },
     *   "names": ["v(out)", ...]
     * }
     * ```
     */
    dc_sweep(netlist: string): string;
    /**
     * Extract editable parameters from a KDL circuit description.
     *
     * Returns JSON: `[{ "ref_des": "R1", "kind": "value", "current": "1k" }, ...]`
     */
    kdl_extract_params(kdl: string): string;
    /**
     * Parse KDL circuit and generate a SPICE netlist + annotation metadata.
     *
     * Returns JSON: `{ "netlist": "...", "annotation_nodes": ["Vmid", ...] }`
     */
    kdl_to_spice(kdl: string): string;
    /**
     * Parse KDL circuit description and render an SVG string.
     */
    kdl_to_svg(kdl: string): string;
    constructor();
    /**
     * Parse a netlist and return the equation map (node names and types).
     * Useful for the UI to know what signals are available.
     */
    parse_nodes(netlist: string): string;
    /**
     * Run any analysis and return results as JSON.
     *
     * Automatically detects the analysis type from the netlist (.OP, .TRAN, .AC, .DC, etc.)
     * and returns the appropriate result format.
     */
    simulate(netlist: string): string;
    /**
     * Run a simulation from a JSON builder config.
     *
     * This is the primary API for complex simulations (transient, AC, sweeps).
     * Takes a JSON config describing the circuit (KDL), source excitations,
     * analysis type, initial conditions, and measurements.
     *
     * Returns a JSON result with waveforms, measures, and optionally SVG.
     */
    simulate_builder(json_config: string): string;
    /**
     * Render SVG from KDL, simulate, and return structured results.
     *
     * Returns JSON: `{ "svg": "...", "measures": [{"label":"Vmid","value":5.0,"unit":"V"}, ...], "netlist": "..." }`
     * The SVG is clean — no values baked in. The JS renders the readout separately.
     */
    simulate_circuit(kdl: string): string;
    /**
     * Run transient analysis, returning full waveforms.
     *
     * Returns JSON:
     * ```json
     * {
     *   "times": [0.0, 1e-6, ...],
     *   "signals": { "v(out)": [0.0, 0.1, ...], ... },
     *   "names": ["v(out)", "v(in)", ...],
     *   "accepted": 1234,
     *   "rejected": 56
     * }
     * ```
     */
    tran(netlist: string): string;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_simulationengine_free: (a: number, b: number) => void;
    readonly simulationengine_ac: (a: number, b: number, c: number) => [number, number, number, number];
    readonly simulationengine_dc_op: (a: number, b: number, c: number) => [number, number, number, number];
    readonly simulationengine_dc_sweep: (a: number, b: number, c: number) => [number, number, number, number];
    readonly simulationengine_kdl_extract_params: (a: number, b: number, c: number) => [number, number, number, number];
    readonly simulationengine_kdl_to_spice: (a: number, b: number, c: number) => [number, number, number, number];
    readonly simulationengine_kdl_to_svg: (a: number, b: number, c: number) => [number, number, number, number];
    readonly simulationengine_parse_nodes: (a: number, b: number, c: number) => [number, number, number, number];
    readonly simulationengine_simulate: (a: number, b: number, c: number) => [number, number, number, number];
    readonly simulationengine_simulate_builder: (a: number, b: number, c: number) => [number, number, number, number];
    readonly simulationengine_simulate_circuit: (a: number, b: number, c: number) => [number, number, number, number];
    readonly simulationengine_tran: (a: number, b: number, c: number) => [number, number, number, number];
    readonly simulationengine_new: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
