// EE-convention input components for Observable Framework
// Uses raw HTML inputs — no dependency on Observable's Inputs library

// E24 preferred values (multiplied across decades)
const E24 = [
  1.0, 1.1, 1.2, 1.3, 1.5, 1.6, 1.8, 2.0, 2.2, 2.4, 2.7, 3.0,
  3.3, 3.6, 3.9, 4.3, 4.7, 5.1, 5.6, 6.2, 6.8, 7.5, 8.2, 9.1
];

function e24Range(minExp, maxExp) {
  const values = [];
  for (let exp = minExp; exp <= maxExp; exp++) {
    const mult = Math.pow(10, exp);
    for (const v of E24) {
      values.push(+(v * mult).toPrecision(6));
    }
  }
  return values;
}

// Format with EE SI prefix
export function formatEE(value, unit) {
  const abs = Math.abs(value);
  if (abs === 0) return `0${unit}`;
  if (abs >= 1e9)  return `${value.toExponential(2)}${unit}`;
  if (abs >= 1e6)  return `${+(value / 1e6).toPrecision(3)}M${unit}`;
  if (abs >= 1e3)  return `${+(value / 1e3).toPrecision(3)}k${unit}`;
  if (abs >= 1)    return `${+value.toPrecision(3)}${unit}`;
  if (abs >= 1e-3) return `${+(value * 1e3).toPrecision(3)}m${unit}`;
  if (abs >= 1e-6) return `${+(value * 1e6).toPrecision(3)}µ${unit}`;
  if (abs >= 1e-9) return `${+(value * 1e9).toPrecision(3)}n${unit}`;
  if (abs >= 1e-12) return `${+(value * 1e12).toPrecision(3)}p${unit}`;
  return `${value.toExponential(2)}${unit}`;
}

// Format for SPICE netlist
export function formatSpice(value) {
  const abs = Math.abs(value);
  if (abs >= 1e6)  return `${value / 1e6}meg`;
  if (abs >= 1e3)  return `${value / 1e3}k`;
  if (abs >= 1)    return `${value}`;
  if (abs >= 1e-3) return `${value * 1e3}m`;
  if (abs >= 1e-6) return `${value * 1e6}u`;
  if (abs >= 1e-9) return `${value * 1e9}n`;
  if (abs >= 1e-12) return `${value * 1e12}p`;
  return `${value}`;
}

function makeE24Slider(label, unit, values, defaultValue) {
  const defaultIdx = values.reduce((best, v, i) =>
    Math.abs(v - defaultValue) < Math.abs(values[best] - defaultValue) ? i : best, 0);

  const container = document.createElement("div");
  container.style.cssText = "display:flex;align-items:center;gap:0.5em;margin:0.2em 0;max-width:320px;";

  const lbl = document.createElement("label");
  lbl.textContent = label;
  lbl.style.cssText = "font-weight:600;color:#b87333;min-width:2.5em;font-family:'Iosevka',monospace;font-size:0.85em;";

  const slider = document.createElement("input");
  slider.type = "range";
  slider.min = 0;
  slider.max = values.length - 1;
  slider.step = 1;
  slider.value = defaultIdx;
  slider.style.cssText = "flex:1;accent-color:#b87333;min-width:80px;max-width:160px;";

  const display = document.createElement("span");
  display.style.cssText = "min-width:5em;font-family:'Iosevka',monospace;font-size:0.85em;text-align:right;";
  display.textContent = formatEE(values[defaultIdx], unit);

  container.appendChild(lbl);
  container.appendChild(slider);
  container.appendChild(display);

  // Make it behave as an Observable input
  container.value = values[defaultIdx];

  slider.addEventListener("input", () => {
    const idx = Math.round(slider.value);
    container.value = values[idx];
    display.textContent = formatEE(values[idx], unit);
    container.dispatchEvent(new Event("input", {bubbles: true}));
  });

  return container;
}

export function Resistance({label = "R", value = 1000} = {}) {
  return makeE24Slider(label, "Ω", e24Range(0, 6), value);
}

export function Capacitance({label = "C", value = 1e-6} = {}) {
  return makeE24Slider(label, "F", e24Range(-12, -2), value);
}

export function Inductance({label = "L", value = 1e-3} = {}) {
  return makeE24Slider(label, "H", e24Range(-9, 0), value);
}

export function Voltage({label = "V", value = 5, min = 0.1, max = 100, step = 0.1} = {}) {
  const container = document.createElement("div");
  container.style.cssText = "display:flex;align-items:center;gap:0.5em;margin:0.2em 0;max-width:320px;";

  const lbl = document.createElement("label");
  lbl.textContent = label;
  lbl.style.cssText = "font-weight:600;color:#b87333;min-width:2.5em;font-family:'Iosevka',monospace;font-size:0.85em;";

  const slider = document.createElement("input");
  slider.type = "range";
  slider.min = min;
  slider.max = max;
  slider.step = step;
  slider.value = value;
  slider.style.cssText = "flex:1;accent-color:#b87333;min-width:80px;max-width:160px;";

  const display = document.createElement("span");
  display.style.cssText = "min-width:4em;font-family:'Iosevka',monospace;font-size:0.85em;text-align:right;";
  display.textContent = `${value}V`;

  container.appendChild(lbl);
  container.appendChild(slider);
  container.appendChild(display);

  container.value = value;

  slider.addEventListener("input", () => {
    container.value = parseFloat(slider.value);
    display.textContent = `${parseFloat(slider.value)}V`;
    container.dispatchEvent(new Event("input", {bubbles: true}));
  });

  return container;
}

export function Frequency({label = "f", value = 60, min = 1, max = 10000, step = 1} = {}) {
  const container = document.createElement("div");
  container.style.cssText = "display:flex;align-items:center;gap:0.5em;margin:0.2em 0;max-width:320px;";

  const lbl = document.createElement("label");
  lbl.textContent = label;
  lbl.style.cssText = "font-weight:600;color:#b87333;min-width:2.5em;font-family:'Iosevka',monospace;font-size:0.85em;";

  const slider = document.createElement("input");
  slider.type = "range";
  slider.min = min;
  slider.max = max;
  slider.step = step;
  slider.value = value;
  slider.style.cssText = "flex:1;accent-color:#b87333;min-width:80px;max-width:160px;";

  const display = document.createElement("span");
  display.style.cssText = "min-width:5em;font-family:'Iosevka',monospace;font-size:0.85em;text-align:right;";
  display.textContent = formatEE(value, "Hz");

  container.appendChild(lbl);
  container.appendChild(slider);
  container.appendChild(display);
  container.value = value;

  slider.addEventListener("input", () => {
    container.value = parseFloat(slider.value);
    display.textContent = formatEE(parseFloat(slider.value), "Hz");
    container.dispatchEvent(new Event("input", {bubbles: true}));
  });

  return container;
}
