/**
 * spice-rs Circuit Diagram Renderer
 *
 * Renders circuit schematics from a declarative JSON description into an
 * inline SVG element.  Components reference <symbol> definitions from the
 * companion symbols.svg sprite sheet.
 *
 * Design tokens (defaults — override via options):
 *   wireColor:      "#3b2f20"   dark brown ink
 *   componentColor: "#b87333"   copper accent
 *   voltageColor:   "#4a6fa5"   blue annotations
 *   currentColor:   "#a04040"   red current arrows
 *   wireWidth:      1.5
 *   componentWidth:  1.8
 */

const SVG_NS = "http://www.w3.org/2000/svg";
const XLINK_NS = "http://www.w3.org/1999/xlink";

/* ------------------------------------------------------------------ */
/*  Symbol metadata — maps each symbol id to its native viewBox size  */
/*  and terminal positions (in viewBox coordinates).                   */
/* ------------------------------------------------------------------ */

const SYMBOL_META = {
  "resistor": {
    vb: [0, 0, 60, 24],
    terminals: { left: [0, 12], right: [60, 12] },
    orient: "horizontal",
  },
  "capacitor": {
    vb: [0, 0, 40, 24],
    terminals: { left: [0, 12], right: [40, 12] },
    orient: "horizontal",
  },
  "inductor": {
    vb: [0, 0, 60, 20],
    terminals: { left: [0, 16], right: [60, 16] },
    orient: "horizontal",
  },
  "voltage-source": {
    vb: [0, 0, 40, 60],
    terminals: { pos: [20, 0], neg: [20, 60] },
    orient: "vertical",
  },
  "current-source": {
    vb: [0, 0, 40, 60],
    terminals: { pos: [20, 0], neg: [20, 60] },
    orient: "vertical",
  },
  "diode": {
    vb: [0, 0, 40, 24],
    terminals: { left: [0, 12], right: [40, 12] },
    orient: "horizontal",
  },
  "nmos": {
    vb: [0, 0, 44, 60],
    terminals: { gate: [0, 30], drain: [34, 0], source: [34, 60] },
    orient: "vertical",
  },
  "pmos": {
    vb: [0, 0, 48, 60],
    terminals: { gate: [0, 30], drain: [38, 0], source: [38, 60] },
    orient: "vertical",
  },
  "npn": {
    vb: [0, 0, 44, 60],
    terminals: { base: [0, 30], collector: [34, 0], emitter: [34, 60] },
    orient: "vertical",
  },
  "pnp": {
    vb: [0, 0, 44, 60],
    terminals: { base: [0, 30], collector: [34, 60], emitter: [34, 0] },
    orient: "vertical",
  },
  "ground": {
    vb: [0, 0, 24, 20],
    terminals: { top: [12, 0] },
    orient: "vertical",
  },
  "node-dot": {
    vb: [0, 0, 8, 8],
    terminals: { center: [4, 4] },
    orient: "horizontal",
  },
};

/* ------------------------------------------------------------------ */
/*  Helpers                                                            */
/* ------------------------------------------------------------------ */

function svgEl(tag, attrs) {
  const el = document.createElementNS(SVG_NS, tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (k === "href") {
      el.setAttributeNS(XLINK_NS, "xlink:href", v);
      el.setAttribute("href", v);           // SVG 2
    } else {
      el.setAttribute(k, v);
    }
  }
  return el;
}

/**
 * Compute the world-space position of a terminal on a placed component.
 *
 * A component is placed at (comp.x, comp.y).  If the component's orient
 * differs from the symbol's native orient, we apply a 90-degree rotation
 * around the component center.
 */
function terminalWorld(comp, termName) {
  const meta = SYMBOL_META[comp.type];
  if (!meta) return [comp.x, comp.y];
  const termLocal = meta.terminals[termName];
  if (!termLocal) return [comp.x, comp.y];

  const vb = meta.vb;
  const w = vb[2] - vb[0];
  const h = vb[3] - vb[1];
  const scale = comp.scale || 1;

  let tx = termLocal[0] * scale;
  let ty = termLocal[1] * scale;

  const needsRotate = comp.orient && comp.orient !== meta.orient;
  if (needsRotate) {
    // Rotate 90 degrees CW around symbol center: (cx,cy)
    const cx = (w * scale) / 2;
    const cy = (h * scale) / 2;
    const rx = -(ty - cy) + cx;
    const ry = (tx - cx) + cy;
    tx = rx;
    ty = ry;
  }

  return [comp.x + tx, comp.y + ty];
}

/* ------------------------------------------------------------------ */
/*  Renderer                                                           */
/* ------------------------------------------------------------------ */

/**
 * Render a circuit diagram.
 *
 * @param {object}      circuit   — JSON circuit description
 * @param {HTMLElement}  container — DOM element to append the SVG into
 * @param {object}      [opts]    — optional overrides
 * @returns {SVGSVGElement}
 */
function renderCircuit(circuit, container, opts) {
  opts = Object.assign({
    symbolsUrl:     "symbols.svg",
    wireColor:      "#3b2f20",
    componentColor: "#b87333",
    voltageColor:   "#4a6fa5",
    currentColor:   "#a04040",
    wireWidth:      1.5,
    componentWidth: 1.8,
    width:          null,   // auto-detect
    height:         null,
    padding:        40,
    fontSize:       13,
    fontFamily:     "'Crimson Pro', 'Crimson Text', Georgia, serif",
  }, opts);

  /* -- Determine canvas size ---------------------------------------- */
  let maxX = 0, maxY = 0;
  for (const c of circuit.components || []) {
    const meta = SYMBOL_META[c.type];
    const scale = c.scale || 1;
    if (meta) {
      const w = meta.vb[2] * scale;
      const h = meta.vb[3] * scale;
      const needsRotate = c.orient && c.orient !== meta.orient;
      maxX = Math.max(maxX, c.x + (needsRotate ? h : w));
      maxY = Math.max(maxY, c.y + (needsRotate ? w : h));
    }
  }
  for (const w of circuit.wires || []) {
    for (const pt of w.path || []) {
      maxX = Math.max(maxX, pt[0]);
      maxY = Math.max(maxY, pt[1]);
    }
  }
  for (const a of circuit.annotations || []) {
    maxX = Math.max(maxX, a.x + 40);
    maxY = Math.max(maxY, a.y + 20);
  }
  const svgW = opts.width  || (maxX + opts.padding * 2);
  const svgH = opts.height || (maxY + opts.padding * 2);

  /* -- Root SVG ----------------------------------------------------- */
  const svg = svgEl("svg", {
    width: svgW,
    height: svgH,
    viewBox: `0 0 ${svgW} ${svgH}`,
    "xmlns": SVG_NS,
  });

  /* -- Embed symbol defs from external file (inline defs block) ----- */
  const defs = svgEl("defs");
  svg.appendChild(defs);

  /* -- Build a component lookup for wire resolution ----------------- */
  const compMap = {};
  for (const c of circuit.components || []) {
    compMap[c.id] = c;
  }

  /* -- Layer groups ------------------------------------------------- */
  const gWires  = svgEl("g", { class: "circuit-wires" });
  const gComps  = svgEl("g", { class: "circuit-components" });
  const gLabels = svgEl("g", { class: "circuit-labels" });
  const gAnnot  = svgEl("g", { class: "circuit-annotations" });
  svg.appendChild(gWires);
  svg.appendChild(gComps);
  svg.appendChild(gLabels);
  svg.appendChild(gAnnot);

  /* -- Draw wires --------------------------------------------------- */
  for (const wire of circuit.wires || []) {
    const points = [];

    // Resolve "from" — either "CompId.terminal" or a raw coordinate
    if (wire.from && typeof wire.from === "string" && wire.from.includes(".")) {
      const [cid, tname] = wire.from.split(".");
      if (compMap[cid]) points.push(terminalWorld(compMap[cid], tname));
    }

    for (const pt of wire.path || []) {
      points.push(pt);
    }

    if (wire.to && typeof wire.to === "string" && wire.to.includes(".")) {
      const [cid, tname] = wire.to.split(".");
      if (compMap[cid]) points.push(terminalWorld(compMap[cid], tname));
    }

    if (points.length >= 2) {
      const d = points.map((p, i) => (i === 0 ? `M${p[0]},${p[1]}` : `L${p[0]},${p[1]}`)).join(" ");
      gWires.appendChild(svgEl("path", {
        d,
        fill: "none",
        stroke: opts.wireColor,
        "stroke-width": opts.wireWidth,
        "stroke-linecap": "round",
        "stroke-linejoin": "round",
      }));
    }
  }

  /* -- Place components --------------------------------------------- */
  for (const comp of circuit.components || []) {
    const meta = SYMBOL_META[comp.type];
    if (!meta) continue;

    const scale = comp.scale || 1;
    const vb = meta.vb;
    const w = vb[2] * scale;
    const h = vb[3] * scale;
    const needsRotate = comp.orient && comp.orient !== meta.orient;

    const use = svgEl("use", {
      href: `${opts.symbolsUrl}#${comp.type}`,
      width: w,
      height: h,
      color: opts.componentColor,
    });

    if (needsRotate) {
      // Rotate 90 CW around the symbol's placed center, then translate
      const cx = comp.x + h / 2;
      const cy = comp.y + w / 2;
      use.setAttribute("transform",
        `translate(${comp.x}, ${comp.y}) ` +
        `translate(${w / 2}, ${h / 2}) ` +
        `rotate(90) ` +
        `translate(${-w / 2}, ${-h / 2})`
      );
    } else {
      use.setAttribute("x", comp.x);
      use.setAttribute("y", comp.y);
    }

    gComps.appendChild(use);

    /* -- Labels ----------------------------------------------------- */
    const labelOffset = comp.labelOffset || {};
    const lx = comp.x + (labelOffset.x || 0);
    const ly = comp.y + (labelOffset.y || -6);

    if (comp.label) {
      const txt = svgEl("text", {
        x: lx,
        y: ly,
        fill: opts.wireColor,
        "font-size": opts.fontSize,
        "font-family": opts.fontFamily,
        "font-weight": "600",
      });
      txt.textContent = comp.label;
      gLabels.appendChild(txt);
    }
    if (comp.value) {
      const txt = svgEl("text", {
        x: lx,
        y: ly + opts.fontSize + 2,
        fill: opts.wireColor,
        "font-size": opts.fontSize - 1,
        "font-family": opts.fontFamily,
        "font-style": "italic",
      });
      txt.textContent = comp.value;
      gLabels.appendChild(txt);
    }
  }

  /* -- Node dots ---------------------------------------------------- */
  for (const dot of circuit.dots || []) {
    const use = svgEl("use", {
      href: `${opts.symbolsUrl}#node-dot`,
      x: dot[0] - 4,
      y: dot[1] - 4,
      width: 8,
      height: 8,
      color: opts.wireColor,
    });
    gComps.appendChild(use);
  }

  /* -- Annotations -------------------------------------------------- */
  for (const ann of circuit.annotations || []) {
    if (ann.type === "voltage") {
      const txt = svgEl("text", {
        x: ann.x,
        y: ann.y,
        fill: opts.voltageColor,
        "font-size": opts.fontSize,
        "font-family": opts.fontFamily,
        "font-weight": "600",
      });
      txt.textContent = ann.text;
      gAnnot.appendChild(txt);
    } else if (ann.type === "current") {
      // Draw a small arrow along the specified direction
      const x = ann.x, y = ann.y;
      const dir = ann.dir || "right";
      let d;
      const s = 8; // arrow half-size
      if (dir === "right") {
        d = `M${x - s},${y} L${x + s},${y} M${x + s - 4},${y - 4} L${x + s},${y} L${x + s - 4},${y + 4}`;
      } else if (dir === "left") {
        d = `M${x + s},${y} L${x - s},${y} M${x - s + 4},${y - 4} L${x - s},${y} L${x - s + 4},${y + 4}`;
      } else if (dir === "down") {
        d = `M${x},${y - s} L${x},${y + s} M${x - 4},${y + s - 4} L${x},${y + s} L${x + 4},${y + s - 4}`;
      } else if (dir === "up") {
        d = `M${x},${y + s} L${x},${y - s} M${x - 4},${y - s + 4} L${x},${y - s} L${x + 4},${y - s + 4}`;
      }
      gAnnot.appendChild(svgEl("path", {
        d,
        fill: "none",
        stroke: opts.currentColor,
        "stroke-width": 1.5,
        "stroke-linecap": "round",
        "stroke-linejoin": "round",
      }));
      if (ann.text) {
        const tx = dir === "left" ? x - s - 4 : x + s + 4;
        const ty = (dir === "up" || dir === "down") ? y - s - 4 : y + 4;
        const anchor = dir === "left" ? "end" : "start";
        const txt = svgEl("text", {
          x: tx,
          y: ty,
          fill: opts.currentColor,
          "font-size": opts.fontSize - 1,
          "font-family": opts.fontFamily,
          "text-anchor": anchor,
        });
        txt.textContent = ann.text;
        gAnnot.appendChild(txt);
      }
    }
  }

  /* -- Inject into DOM ---------------------------------------------- */
  container.appendChild(svg);
  return svg;
}

/* ------------------------------------------------------------------ */
/*  Utility: resolve terminal world position (public)                  */
/* ------------------------------------------------------------------ */

function getTerminal(circuit, ref) {
  if (!ref || !ref.includes(".")) return null;
  const [cid, tname] = ref.split(".");
  const comp = (circuit.components || []).find(c => c.id === cid);
  if (!comp) return null;
  return terminalWorld(comp, tname);
}

/* ------------------------------------------------------------------ */
/*  Exports                                                            */
/* ------------------------------------------------------------------ */

if (typeof module !== "undefined" && module.exports) {
  module.exports = { renderCircuit, getTerminal, SYMBOL_META };
}
