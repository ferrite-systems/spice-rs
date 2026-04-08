// Simulation panel — schematic + measurement readout as one visual unit
import {formatEE} from "./ee-inputs.js";

// Compact inline readout of measure results
export function measureReadout(measures) {
  if (!measures || measures.length === 0) return document.createElement("span");

  const panel = document.createElement("div");
  panel.style.cssText = `
    display: inline-flex;
    gap: 1.2em;
    padding: 0.35em 0.8em;
    background: rgba(74, 111, 165, 0.06);
    border-left: 3px solid #4a6fa5;
    border-radius: 0 3px 3px 0;
    font-family: 'Iosevka', 'Fira Code', monospace;
    font-size: 0.85em;
    margin: 0.4em 0;
  `;

  for (const m of measures) {
    const item = document.createElement("span");
    item.style.cssText = "white-space: nowrap; color: #4a6fa5;";
    item.innerHTML = `<b>${m.label}</b> <i style="margin-left:0.2em">${formatEE(m.value, m.unit)}</i>`;
    panel.appendChild(item);
  }

  return panel;
}

// Full simulation panel: readout on top, schematic below
export function simPanel(sim) {
  const wrapper = document.createElement("div");
  wrapper.style.cssText = "margin: 0.5em 0 1em;";

  // Readout first (near the sliders)
  if (sim.measures && sim.measures.length > 0) {
    wrapper.appendChild(measureReadout(sim.measures));
  }

  // Schematic below
  const svgDiv = document.createElement("div");
  svgDiv.className = "ferrite-circuit";
  svgDiv.innerHTML = sim.svg;
  wrapper.appendChild(svgDiv);

  return wrapper;
}
