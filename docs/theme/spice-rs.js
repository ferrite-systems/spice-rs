// spice-rs docs — runtime for interactive simulations + circuit widgets
// WASM is lazy-loaded on first interaction.

(function() {
  'use strict';

  var wasmReady = false;
  var wasmLoading = false;
  var engine = null;
  var pendingCallbacks = [];

  // ── WASM loading ──

  function wasmBaseUrl() {
    var rel = (typeof path_to_root !== 'undefined') ? path_to_root : '';
    if (!rel) {
      var depth = window.location.pathname.replace(/\/[^/]*$/, '').split('/').length - 1;
      rel = '../'.repeat(Math.max(0, depth));
    }
    var a = document.createElement('a');
    a.href = rel + 'wasm/';
    return a.href;
  }

  function ensureWasm(callback) {
    if (wasmReady && engine) { callback(engine); return; }
    pendingCallbacks.push(callback);
    if (wasmLoading) return;
    wasmLoading = true;

    var base = wasmBaseUrl();
    var script = document.createElement('script');
    script.type = 'module';
    script.textContent =
      'import init, { SimulationEngine } from "' + base + 'spice_rs_wasm.js";\n' +
      'await init({ module_or_path: "' + base + 'spice_rs_wasm_bg.wasm" });\n' +
      'window.__spiceRsEngine = new SimulationEngine();\n' +
      'window.dispatchEvent(new Event("spice-rs-ready"));\n';
    document.head.appendChild(script);

    window.addEventListener('spice-rs-ready', function() {
      engine = window.__spiceRsEngine;
      wasmReady = true;
      wasmLoading = false;
      pendingCallbacks.forEach(function(cb) { cb(engine); });
      pendingCallbacks.length = 0;
    });
  }

  // ── Ferrite circuit widgets (KDL → WASM → SVG + sim) ──

  function initCircuitWidgets() {
    document.querySelectorAll('.ferrite-circuit-widget').forEach(function(widget) {
      if (widget.dataset.initialized) return;
      widget.dataset.initialized = 'true';

      var kdlScript = widget.querySelector('script[type="application/kdl"]');
      if (!kdlScript) return;

      var originalKdl = kdlScript.textContent
        .replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>');
      var currentKdl = originalKdl;

      var svgBox = widget.querySelector('.ferrite-circuit-svg');
      var ctrlBox = widget.querySelector('.ferrite-circuit-controls');
      var actBox = widget.querySelector('.ferrite-circuit-actions');
      var nlBox = widget.querySelector('.ferrite-circuit-netlist');

      ensureWasm(function(eng) {
        // Render initial SVG
        try {
          svgBox.innerHTML = eng.kdl_to_svg(currentKdl);
        } catch(e) {
          svgBox.innerHTML = '<div class="ferrite-circuit-error">' + escapeHtml(e.toString()) + '</div>';
          return;
        }

        // Build param controls
        try {
          var params = JSON.parse(eng.kdl_extract_params(currentKdl));
          buildParamControls(ctrlBox, params, function(refDes, kind, newVal) {
            currentKdl = updateKdlParam(currentKdl, refDes, kind, newVal);
            try { svgBox.innerHTML = eng.kdl_to_svg(currentKdl); } catch(e) {}
            updateNetlistView(eng, nlBox, currentKdl);
          });
        } catch(e) {}

        // Run button
        var runBtn = document.createElement('button');
        runBtn.className = 'run-btn';
        runBtn.textContent = 'Run';
        actBox.appendChild(runBtn);

        // Collapsible SPICE netlist
        updateNetlistView(eng, nlBox, currentKdl);

        runBtn.addEventListener('click', function() {
          runBtn.disabled = true;
          runBtn.textContent = 'Running\u2026';
          try {
            var spiceResult = JSON.parse(eng.kdl_to_spice(currentKdl));
            var simResult = JSON.parse(eng.dc_op(spiceResult.netlist));
            var svgEl = svgBox.querySelector('svg');
            if (svgEl) {
              overlaySimResults(svgEl, simResult.nodes);
            }
          } catch(e) {
            console.error('[ferrite-circuit] Simulation error:', e);
          } finally {
            runBtn.disabled = false;
            runBtn.textContent = 'Run';
          }
        });
      });
    });
  }

  function buildParamControls(container, params, onChange) {
    container.innerHTML = '';
    if (params.length === 0) return;

    params.forEach(function(p) {
      var row = document.createElement('div');
      row.className = 'param-row';

      var label = document.createElement('label');
      label.textContent = p.ref_des;
      row.appendChild(label);

      var input = document.createElement('input');
      input.type = 'text';
      input.value = p.current;
      input.className = 'param-input';
      input.addEventListener('change', function() {
        onChange(p.ref_des, p.kind, input.value);
      });
      // Also update on Enter key
      input.addEventListener('keydown', function(e) {
        if (e.key === 'Enter') {
          e.preventDefault();
          onChange(p.ref_des, p.kind, input.value);
        }
      });
      row.appendChild(input);

      container.appendChild(row);
    });
  }

  function updateKdlParam(kdl, refDes, kind, newValue) {
    if (kind === 'value') {
      // Find component "RefDes" block and replace its value child
      var re = new RegExp(
        '(component\\s+"' + escapeRegex(refDes) + '"[\\s\\S]*?value\\s+)"([^"]*)"'
      );
      return kdl.replace(re, '$1"' + newValue + '"');
    } else if (kind === 'voltage') {
      var railLabel = refDes;
      var re = new RegExp(
        '(node\\s+"[^"]*"[^\\n]*?label="' + escapeRegex(railLabel) + '"[^\\n]*?voltage=)"([^"]*)"'
      );
      return kdl.replace(re, '$1"' + newValue + '"');
    }
    return kdl;
  }

  function updateNetlistView(eng, container, kdl) {
    try {
      var result = JSON.parse(eng.kdl_to_spice(kdl));
      var details = container.querySelector('details');
      if (!details) {
        details = document.createElement('details');
        var summary = document.createElement('summary');
        summary.textContent = 'SPICE netlist';
        details.appendChild(summary);
        container.appendChild(details);
      }
      var pre = details.querySelector('pre');
      if (!pre) {
        pre = document.createElement('pre');
        details.appendChild(pre);
      }
      pre.textContent = result.netlist;
    } catch(e) {}
  }

  function filterSimResults(nodes, annotationNodes) {
    var result = {};
    // Lowercase annotation names for matching
    var annLower = (annotationNodes || []).map(function(n) { return n.toLowerCase(); });
    for (var key in nodes) {
      // Skip branch currents
      if (key.indexOf('#branch') !== -1) continue;
      var m = key.match(/^v\((.+)\)$/);
      if (!m) continue;
      var netName = m[1];
      // Only show nodes that have annotations (interesting outputs)
      // If no annotations defined, show all node voltages
      if (annLower.length > 0) {
        var isAnnotated = false;
        for (var i = 0; i < annLower.length; i++) {
          if (netName.toLowerCase() === annLower[i] ||
              annLower[i].toLowerCase().startsWith(netName.toLowerCase())) {
            isAnnotated = true;
            break;
          }
        }
        if (!isAnnotated) continue;
      }
      result[netName] = nodes[key];
    }
    return result;
  }

  function overlaySimResults(svgEl, nodes) {
    // Find blue annotation text elements and update with sim values
    var texts = svgEl.querySelectorAll('text[fill="#4a6fa5"]');
    texts.forEach(function(txt) {
      var label = txt.textContent.replace(/\s*=\s*.*$/, '').trim();
      for (var key in nodes) {
        var m = key.match(/^v\((.+)\)$/);
        if (m) {
          var netName = m[1];
          if (netName.toLowerCase() === label.toLowerCase() ||
              label.toLowerCase().startsWith(netName.toLowerCase())) {
            txt.textContent = label + ' = ' + formatValue(nodes[key]) + 'V';
            return;
          }
        }
      }
    });
  }

  // ── Standalone spice-sim blocks (raw SPICE netlist widgets) ──

  function initSimBlocks() {
    document.querySelectorAll('.spice-sim-container').forEach(function(container) {
      if (container.dataset.initialized) return;
      container.dataset.initialized = 'true';

      var textarea = container.querySelector('textarea');
      var runBtn = container.querySelector('button.run-btn');
      var output = container.querySelector('.sim-output');
      var analysis = container.dataset.analysis || 'dc';

      if (!textarea || !runBtn || !output) return;

      runBtn.addEventListener('click', function() {
        runBtn.disabled = true;
        runBtn.textContent = 'Running\u2026';
        output.innerHTML = '';

        ensureWasm(function(eng) {
          try {
            var netlist = textarea.value;
            var resultJson;
            if (analysis === 'tran') { resultJson = eng.tran(netlist); }
            else if (analysis === 'ac') { resultJson = eng.ac(netlist); }
            else if (analysis === 'dc-sweep') { resultJson = eng.dc_sweep(netlist); }
            else { resultJson = eng.dc_op(netlist); }

            var result = JSON.parse(resultJson);
            renderResult(output, result, analysis);
          } catch (e) {
            output.innerHTML = '<pre class="sim-error">' + escapeHtml(e.toString()) + '</pre>';
          } finally {
            runBtn.disabled = false;
            runBtn.textContent = 'Run';
          }
        });
      });

      if (container.dataset.autorun === 'true') {
        runBtn.click();
      }
    });
  }

  // ── Result rendering ──

  function renderResult(container, result, analysis) {
    if (analysis === 'dc' && result.nodes) {
      renderDcTable(container, result.nodes);
    } else if (analysis === 'tran' && result.times) {
      renderWaveform(container, result.times, result.signals, result.names);
    } else if (analysis === 'ac' && result.frequencies) {
      renderBode(container, result);
    } else if (analysis === 'dc-sweep' && result.sweep_values) {
      renderWaveform(container, result.sweep_values, result.signals, result.names);
    } else {
      container.innerHTML = '<pre>' + escapeHtml(JSON.stringify(result, null, 2)) + '</pre>';
    }
  }

  function renderDcTable(container, nodes) {
    var html = '<table class="sim-results-table"><thead><tr><th>Node</th><th>Value</th></tr></thead><tbody>';
    var entries = Object.entries(nodes).sort(function(a, b) { return a[0].localeCompare(b[0]); });
    for (var i = 0; i < entries.length; i++) {
      html += '<tr><td><code>' + escapeHtml(entries[i][0]) + '</code></td>';
      html += '<td>' + formatValue(entries[i][1]) + '</td></tr>';
    }
    html += '</tbody></table>';
    container.innerHTML = html;
  }

  function getThemeColors() {
    var body = getComputedStyle(document.querySelector('.coal, .navy, .ayu, .light, .rust') || document.body);
    return {
      bg:     body.getPropertyValue('--bg').trim()     || '#faf4e8',
      fg:     body.getPropertyValue('--fg').trim()     || '#3b2f20',
      faint:  body.getPropertyValue('--spice-faint').trim()  || '#e8dcc8',
      copper: body.getPropertyValue('--spice-copper').trim() || '#b87333',
      blue:   body.getPropertyValue('--spice-blue').trim()   || '#4a6fa5',
      red:    body.getPropertyValue('--spice-red').trim()    || '#a04040',
    };
  }

  function renderWaveform(container, xvals, signals, names) {
    var canvas = document.createElement('canvas');
    canvas.width = 600; canvas.height = 300;
    canvas.style.width = '100%'; canvas.style.maxWidth = '600px'; canvas.style.height = 'auto';
    container.appendChild(canvas);

    var ctx = canvas.getContext('2d');
    var pad = { top: 20, right: 20, bottom: 40, left: 65 };
    var w = canvas.width - pad.left - pad.right;
    var h = canvas.height - pad.top - pad.bottom;
    var tc = getThemeColors();
    var traceColors = [tc.copper, tc.blue, tc.red, '#5a8a5a', '#8a5a8a'];

    var plotNames = names.filter(function(n) { return signals[n] && signals[n].length > 0; });
    if (plotNames.length === 0) return;

    var ymin = Infinity, ymax = -Infinity;
    for (var ni = 0; ni < plotNames.length; ni++) {
      var data = signals[plotNames[ni]];
      for (var i = 0; i < data.length; i++) {
        if (data[i] < ymin) ymin = data[i];
        if (data[i] > ymax) ymax = data[i];
      }
    }
    if (ymin === ymax) { ymin -= 1; ymax += 1; }
    var yMargin = (ymax - ymin) * 0.05;
    ymin -= yMargin; ymax += yMargin;
    var xmin = xvals[0], xmax = xvals[xvals.length - 1];

    ctx.fillStyle = tc.bg; ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.strokeStyle = tc.faint; ctx.lineWidth = 0.5;
    for (i = 0; i <= 4; i++) {
      var y = pad.top + (h * i / 4);
      ctx.beginPath(); ctx.moveTo(pad.left, y); ctx.lineTo(pad.left + w, y); ctx.stroke();
    }
    ctx.strokeStyle = tc.fg; ctx.lineWidth = 1;
    ctx.beginPath(); ctx.moveTo(pad.left, pad.top); ctx.lineTo(pad.left, pad.top + h); ctx.lineTo(pad.left + w, pad.top + h); ctx.stroke();
    ctx.fillStyle = tc.fg; ctx.font = '11px "Crimson Pro", Georgia, serif';
    ctx.textAlign = 'right'; ctx.textBaseline = 'middle';
    for (i = 0; i <= 4; i++) { ctx.fillText(formatValue(ymax - (ymax - ymin) * i / 4), pad.left - 8, pad.top + h * i / 4); }
    ctx.textAlign = 'center'; ctx.textBaseline = 'top';
    for (i = 0; i <= 4; i++) { ctx.fillText(formatSI(xmin + (xmax - xmin) * i / 4) + 's', pad.left + w * i / 4, pad.top + h + 8); }

    for (var si = 0; si < plotNames.length; si++) {
      data = signals[plotNames[si]];
      ctx.strokeStyle = traceColors[si % traceColors.length]; ctx.lineWidth = 1.8;
      ctx.beginPath();
      for (i = 0; i < xvals.length; i++) {
        var x = pad.left + ((xvals[i] - xmin) / (xmax - xmin)) * w;
        y = pad.top + h - ((data[i] - ymin) / (ymax - ymin)) * h;
        if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
      }
      ctx.stroke();
      var lastY = pad.top + h - ((data[data.length - 1] - ymin) / (ymax - ymin)) * h;
      ctx.fillStyle = traceColors[si % traceColors.length];
      ctx.font = '12px "Crimson Pro", Georgia, serif'; ctx.textAlign = 'left'; ctx.textBaseline = 'middle';
      ctx.fillText(plotNames[si], pad.left + w + 4, lastY);
    }
  }

  function renderBode(container, result) {
    var magSignals = {};
    for (var i = 0; i < result.names.length; i++) {
      var name = result.names[i];
      if (result.signals_mag[name]) {
        magSignals[name + ' (dB)'] = result.signals_mag[name].map(function(m) { return 20 * Math.log10(Math.max(m, 1e-20)); });
      }
    }
    renderWaveform(container, result.frequencies.map(function(f) { return Math.log10(f); }), magSignals, Object.keys(magSignals));
  }

  // ── Formatting ──

  function formatValue(val) {
    if (Math.abs(val) === 0) return '0';
    var a = Math.abs(val);
    if (a >= 1e6) return (val / 1e6).toFixed(3) + ' M';
    if (a >= 1e3) return (val / 1e3).toFixed(3) + ' k';
    if (a >= 1) return val.toFixed(4);
    if (a >= 1e-3) return (val * 1e3).toFixed(3) + ' m';
    if (a >= 1e-6) return (val * 1e6).toFixed(3) + ' \u00b5';
    if (a >= 1e-9) return (val * 1e9).toFixed(3) + ' n';
    if (a >= 1e-12) return (val * 1e12).toFixed(3) + ' p';
    return val.toExponential(3);
  }

  function formatSI(val) {
    if (val === 0) return '0';
    var a = Math.abs(val);
    if (a >= 1) return val.toFixed(1);
    if (a >= 1e-3) return (val * 1e3).toFixed(1) + 'm';
    if (a >= 1e-6) return (val * 1e6).toFixed(1) + '\u00b5';
    if (a >= 1e-9) return (val * 1e9).toFixed(1) + 'n';
    return val.toExponential(1);
  }

  function escapeHtml(str) {
    var div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
  }

  function escapeRegex(str) {
    return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }

  // ── Lifecycle ──

  function initAll() {
    initSimBlocks();
    initCircuitWidgets();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initAll);
  } else {
    initAll();
  }

  var observer = new MutationObserver(function(mutations) {
    for (var i = 0; i < mutations.length; i++) {
      if (mutations[i].type === 'childList' && mutations[i].target.id === 'content') {
        initAll();
        break;
      }
    }
  });
  var content = document.getElementById('content');
  if (content) { observer.observe(content, { childList: true, subtree: true }); }

  // Stop mdbook arrow-key page navigation when an input/textarea has focus
  document.addEventListener('keydown', function(e) {
    var tag = document.activeElement && document.activeElement.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA') {
      e.stopPropagation();
    }
  }, true);
})();
