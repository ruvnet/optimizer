(function(){
  'use strict';

  window.RuVectorPages = window.RuVectorPages || {};

  /* ── State ──────────────────────────────────────────────────── */
  var state = {
    gpus: [],
    selectedGpuIdx: 0,
    processes: [],
    vramHistory: [],
    historyMaxLen: 360,
    models: [],
    leakAlerts: [],
    container: null,
    canvasEl: null,
    initialized: false
  };

  // Default GPU for demo
  var defaultGpu = {
    name: 'NVIDIA GeForce RTX 4090',
    vendor: 'NVIDIA',
    vramTotalMB: 24576,
    vramUsedMB: 8742,
    tempC: 62,
    powerDrawW: 185,
    powerLimitW: 450,
    clockMHz: 2520,
    maxClockMHz: 2820,
    utilPct: 34,
    memUtilPct: 36,
    driverAPI: 'NVML 12.6',
    fanPct: 42,
    pcieGen: 4,
    pcieLanes: 16
  };

  /* ── IPC ─────────────────────────────────────────────────────── */
  function ipcSend(type, payload) {
    if (window.ipc) {
      var msg = Object.assign({ type: type }, payload || {});
      window.ipc.postMessage(JSON.stringify(msg));
    }
  }

  /* ── Helpers ─────────────────────────────────────────────────── */
  function getCSS(v) { return getComputedStyle(document.documentElement).getPropertyValue(v).trim(); }

  function formatMB(mb) {
    if (mb >= 1024) return (mb / 1024).toFixed(1) + ' GB';
    return mb.toFixed(0) + ' MB';
  }

  function pctColor(pct) {
    if (pct > 90) return getCSS('--accent-red');
    if (pct > 70) return getCSS('--accent-amber');
    return getCSS('--accent-cyan');
  }

  function getGpu() {
    return state.gpus.length > 0 ? state.gpus[state.selectedGpuIdx] : defaultGpu;
  }

  /* ── Large circular VRAM gauge ───────────────────────────────── */
  function drawVramGauge(canvas) {
    var gpu = getGpu();
    var pct = gpu.vramTotalMB > 0 ? (gpu.vramUsedMB / gpu.vramTotalMB) * 100 : 0;
    var ctx = canvas.getContext('2d');
    var dpr = window.devicePixelRatio || 1;
    var size = 160;
    canvas.width = size * dpr;
    canvas.height = size * dpr;
    canvas.style.width = size + 'px';
    canvas.style.height = size + 'px';
    ctx.scale(dpr, dpr);

    var cx = size / 2, cy = size / 2, r = size / 2 - 16;
    var startAngle = 0.75 * Math.PI;
    var endAngle = 2.25 * Math.PI;
    var valAngle = startAngle + (endAngle - startAngle) * Math.min(pct / 100, 1);
    var color = pctColor(pct);

    ctx.clearRect(0, 0, size, size);

    // Track
    ctx.beginPath();
    ctx.arc(cx, cy, r, startAngle, endAngle);
    ctx.strokeStyle = getCSS('--gauge-track');
    ctx.lineWidth = 12;
    ctx.lineCap = 'round';
    ctx.stroke();

    // Fill
    if (pct > 0) {
      ctx.beginPath();
      ctx.arc(cx, cy, r, startAngle, valAngle);
      ctx.strokeStyle = color;
      ctx.lineWidth = 12;
      ctx.lineCap = 'round';
      ctx.stroke();
    }

    // Ticks
    ctx.strokeStyle = getCSS('--text-dim');
    ctx.lineWidth = 1;
    for (var i = 0; i <= 10; i++) {
      var a = startAngle + (endAngle - startAngle) * (i / 10);
      ctx.beginPath();
      ctx.moveTo(cx + Math.cos(a) * (r - 8), cy + Math.sin(a) * (r - 8));
      ctx.lineTo(cx + Math.cos(a) * (r + 6), cy + Math.sin(a) * (r + 6));
      ctx.stroke();
    }

    // Center text
    ctx.textAlign = 'center';
    ctx.fillStyle = getCSS('--text-primary');
    ctx.font = '600 28px ' + getCSS('--font');
    ctx.fillText(Math.round(pct) + '%', cx, cy - 4);
    ctx.fillStyle = getCSS('--text-dim');
    ctx.font = '10px ' + getCSS('--font');
    ctx.fillText('VRAM', cx, cy + 12);
    ctx.font = '9px ' + getCSS('--mono');
    ctx.fillText(formatMB(gpu.vramUsedMB) + ' / ' + formatMB(gpu.vramTotalMB), cx, cy + 25);
  }

  /* ── Small info gauge ────────────────────────────────────────── */
  function createSmallGauge(id, label, unit, max, size) {
    size = size || 64;
    var wrap = document.createElement('div');
    wrap.style.cssText = 'display:flex;flex-direction:column;align-items:center;gap:2px';

    var canvas = document.createElement('canvas');
    canvas.setAttribute('data-sgauge', id);
    canvas.style.cssText = 'width:' + size + 'px;height:' + size + 'px';
    wrap.appendChild(canvas);

    var lbl = document.createElement('span');
    lbl.style.cssText = 'font-size:8px;color:var(--text-dim);text-transform:uppercase;letter-spacing:0.5px';
    lbl.textContent = label;
    wrap.appendChild(lbl);

    return { element: wrap, canvas: canvas, max: max, size: size };
  }

  function drawSmallGauge(canvas, value, max, color, size) {
    var ctx = canvas.getContext('2d');
    var dpr = window.devicePixelRatio || 1;
    canvas.width = size * dpr;
    canvas.height = size * dpr;
    canvas.style.width = size + 'px';
    canvas.style.height = size + 'px';
    ctx.scale(dpr, dpr);

    var cx = size / 2, cy = size / 2, r = size / 2 - 8;
    var startAngle = 0.75 * Math.PI;
    var endAngle = 2.25 * Math.PI;
    var pct = Math.min(value / max, 1);
    var valAngle = startAngle + (endAngle - startAngle) * pct;

    ctx.clearRect(0, 0, size, size);

    // Track
    ctx.beginPath();
    ctx.arc(cx, cy, r, startAngle, endAngle);
    ctx.strokeStyle = getCSS('--gauge-track');
    ctx.lineWidth = 6;
    ctx.lineCap = 'round';
    ctx.stroke();

    // Fill
    if (pct > 0) {
      ctx.beginPath();
      ctx.arc(cx, cy, r, startAngle, valAngle);
      ctx.strokeStyle = color;
      ctx.lineWidth = 6;
      ctx.lineCap = 'round';
      ctx.stroke();
    }

    // Center value
    ctx.textAlign = 'center';
    ctx.fillStyle = getCSS('--text-primary');
    ctx.font = '600 ' + Math.round(size * 0.22) + 'px ' + getCSS('--font');
    ctx.fillText(Math.round(value), cx, cy + 4);
  }

  /* ── VRAM timeline chart ─────────────────────────────────────── */
  function drawVramChart() {
    var canvas = state.canvasEl;
    if (!canvas) return;
    var ctx = canvas.getContext('2d');
    var dpr = window.devicePixelRatio || 1;
    var rect = canvas.parentElement.getBoundingClientRect();
    var W = rect.width;
    var H = rect.height;
    canvas.width = W * dpr;
    canvas.height = H * dpr;
    canvas.style.width = W + 'px';
    canvas.style.height = H + 'px';
    ctx.scale(dpr, dpr);

    var pad = { top: 20, right: 14, bottom: 30, left: 50 };
    var cw = W - pad.left - pad.right;
    var ch = H - pad.top - pad.bottom;

    var gpu = getGpu();
    var maxVRAM = gpu.vramTotalMB;

    ctx.fillStyle = getCSS('--bg-primary');
    ctx.fillRect(0, 0, W, H);

    if (state.vramHistory.length < 2) {
      ctx.fillStyle = getCSS('--text-dim');
      ctx.font = '11px ' + getCSS('--font');
      ctx.textAlign = 'center';
      ctx.fillText('Collecting VRAM usage data...', W / 2, H / 2);
      return;
    }

    var data = state.vramHistory;

    // Grid
    ctx.strokeStyle = getCSS('--border');
    ctx.lineWidth = 0.5;
    ctx.setLineDash([3, 3]);
    for (var g = 0; g <= 4; g++) {
      var gy = pad.top + ch - (g / 4) * ch;
      ctx.beginPath();
      ctx.moveTo(pad.left, gy);
      ctx.lineTo(pad.left + cw, gy);
      ctx.stroke();
    }
    ctx.setLineDash([]);

    // Y-axis labels
    ctx.fillStyle = getCSS('--text-dim');
    ctx.font = '9px ' + getCSS('--mono');
    ctx.textAlign = 'right';
    for (var y = 0; y <= 4; y++) {
      var val = (maxVRAM / 4) * y;
      var py = pad.top + ch - (y / 4) * ch;
      ctx.fillText(formatMB(val), pad.left - 6, py + 3);
    }

    // X-axis labels
    ctx.textAlign = 'center';
    var timeLabels = ['-60m', '-45m', '-30m', '-15m', 'Now'];
    timeLabels.forEach(function(lbl, i) {
      ctx.fillText(lbl, pad.left + (cw * i / (timeLabels.length - 1)), H - 6);
    });

    // Axis title
    ctx.font = '9px ' + getCSS('--font');
    ctx.save();
    ctx.translate(10, pad.top + ch / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.textAlign = 'center';
    ctx.fillText('VRAM Usage', 0, 0);
    ctx.restore();

    // Plot categories as stacked areas
    var categories = [
      { key: 'textures', color: getCSS('--accent-cyan'), label: 'Textures' },
      { key: 'framebuffers', color: getCSS('--accent-amber'), label: 'Framebuffers' },
      { key: 'models', color: getCSS('--accent-green'), label: 'AI Models' },
      { key: 'other', color: getCSS('--text-dim'), label: 'Other' }
    ];

    // Calculate stacked values
    var stackedData = data.map(function(pt) {
      var result = { total: 0 };
      categories.forEach(function(cat) {
        result[cat.key] = pt[cat.key] || 0;
        result.total += result[cat.key];
      });
      return result;
    });

    // Draw stacked areas (bottom to top)
    var cumulative = data.map(function() { return 0; });

    categories.forEach(function(cat) {
      ctx.beginPath();

      // Top line
      stackedData.forEach(function(pt, idx) {
        var px = pad.left + (idx / (data.length - 1)) * cw;
        var val = cumulative[idx] + pt[cat.key];
        var py = pad.top + ch - (val / maxVRAM) * ch;
        if (idx === 0) ctx.moveTo(px, py);
        else ctx.lineTo(px, py);
      });

      // Bottom line (reverse)
      for (var i = data.length - 1; i >= 0; i--) {
        var px2 = pad.left + (i / (data.length - 1)) * cw;
        var py2 = pad.top + ch - (cumulative[i] / maxVRAM) * ch;
        ctx.lineTo(px2, py2);
      }
      ctx.closePath();
      ctx.fillStyle = cat.color;
      ctx.globalAlpha = 0.2;
      ctx.fill();
      ctx.globalAlpha = 1;

      // Top line stroke
      ctx.beginPath();
      ctx.strokeStyle = cat.color;
      ctx.lineWidth = 1.5;
      stackedData.forEach(function(pt, idx) {
        var px3 = pad.left + (idx / (data.length - 1)) * cw;
        var val3 = cumulative[idx] + pt[cat.key];
        var py3 = pad.top + ch - (val3 / maxVRAM) * ch;
        if (idx === 0) ctx.moveTo(px3, py3);
        else ctx.lineTo(px3, py3);
      });
      ctx.stroke();

      // Update cumulative
      stackedData.forEach(function(pt, idx) {
        cumulative[idx] += pt[cat.key];
      });
    });

    // Total line
    ctx.beginPath();
    ctx.strokeStyle = getCSS('--text-primary');
    ctx.lineWidth = 2;
    ctx.setLineDash([4, 3]);
    stackedData.forEach(function(pt, idx) {
      var px4 = pad.left + (idx / (data.length - 1)) * cw;
      var py4 = pad.top + ch - (pt.total / maxVRAM) * ch;
      if (idx === 0) ctx.moveTo(px4, py4);
      else ctx.lineTo(px4, py4);
    });
    ctx.stroke();
    ctx.setLineDash([]);

    // Legend
    var lx = pad.left + 4;
    ctx.font = '9px ' + getCSS('--font');
    categories.concat([{ label: 'Total', color: getCSS('--text-primary') }]).forEach(function(cat) {
      ctx.fillStyle = cat.color;
      ctx.fillRect(lx, 8, 12, 3);
      ctx.fillStyle = getCSS('--text-secondary');
      ctx.textAlign = 'left';
      ctx.fillText(cat.label, lx + 16, 12);
      lx += ctx.measureText(cat.label).width + 30;
    });
  }

  /* ── Process VRAM table ──────────────────────────────────────── */
  function renderProcessTable(container) {
    container.innerHTML = '';

    var header = document.createElement('div');
    header.style.cssText = 'display:grid;grid-template-columns:1fr 80px 80px 90px;gap:6px;padding:6px 0;border-bottom:1px solid var(--border);font-size:9px;font-weight:600;text-transform:uppercase;letter-spacing:0.5px;color:var(--text-dim)';
    ['Process', 'Dedicated', 'Shared', 'Category'].forEach(function(h) {
      var s = document.createElement('span');
      s.textContent = h;
      header.appendChild(s);
    });
    container.appendChild(header);

    var procs = state.processes.length > 0 ? state.processes : getDemoProcesses();

    procs.slice(0, 15).forEach(function(p) {
      var row = document.createElement('div');
      row.style.cssText = 'display:grid;grid-template-columns:1fr 80px 80px 90px;gap:6px;padding:5px 0;border-bottom:1px solid var(--border);font-size:11px;align-items:center';

      var name = document.createElement('span');
      name.style.cssText = 'color:var(--text-primary);overflow:hidden;text-overflow:ellipsis;white-space:nowrap';
      name.textContent = p.name;

      var dedicated = document.createElement('span');
      dedicated.style.cssText = 'font-family:var(--mono);font-size:10px;color:var(--accent-cyan)';
      dedicated.textContent = formatMB(p.dedicatedMB);

      var shared = document.createElement('span');
      shared.style.cssText = 'font-family:var(--mono);font-size:10px;color:var(--accent-amber)';
      shared.textContent = formatMB(p.sharedMB);

      var cat = document.createElement('span');
      var catColors = { 'AI Model': '--accent-green', 'Game': '--accent-purple', 'Browser': '--accent-cyan', 'Desktop': '--text-dim', 'Video': '--accent-amber', 'System': '--text-secondary' };
      var catColor = catColors[p.category] || '--text-secondary';
      cat.style.cssText = 'font-size:9px;padding:2px 6px;border-radius:3px;background:var(' + catColor + '-dim, rgba(128,128,128,0.1));color:var(' + catColor + ');font-weight:500;text-align:center';
      cat.textContent = p.category;

      row.appendChild(name);
      row.appendChild(dedicated);
      row.appendChild(shared);
      row.appendChild(cat);
      container.appendChild(row);
    });
  }

  function getDemoProcesses() {
    return [
      { name: 'ollama.exe', dedicatedMB: 4200, sharedMB: 120, category: 'AI Model' },
      { name: 'chrome.exe', dedicatedMB: 850, sharedMB: 340, category: 'Browser' },
      { name: 'dwm.exe', dedicatedMB: 480, sharedMB: 60, category: 'Desktop' },
      { name: 'obs64.exe', dedicatedMB: 420, sharedMB: 90, category: 'Video' },
      { name: 'explorer.exe', dedicatedMB: 180, sharedMB: 45, category: 'System' },
      { name: 'Discord.exe', dedicatedMB: 320, sharedMB: 110, category: 'Desktop' },
      { name: 'msedge.exe', dedicatedMB: 280, sharedMB: 160, category: 'Browser' },
      { name: 'python.exe', dedicatedMB: 1600, sharedMB: 80, category: 'AI Model' },
      { name: 'vscode.exe', dedicatedMB: 210, sharedMB: 70, category: 'Desktop' }
    ];
  }

  /* ── AI Model layer manager ──────────────────────────────────── */
  function renderModelManager(container) {
    container.innerHTML = '';

    var models = state.models.length > 0 ? state.models : getDemoModels();

    if (models.length === 0) {
      var empty = document.createElement('div');
      empty.style.cssText = 'font-size:11px;color:var(--text-dim);padding:10px 0';
      empty.textContent = 'No AI models detected in VRAM.';
      container.appendChild(empty);
      return;
    }

    models.forEach(function(model, idx) {
      var wrap = document.createElement('div');
      wrap.style.cssText = 'padding:10px 0;' + (idx > 0 ? 'border-top:1px solid var(--border)' : '');

      // Model name + info
      var topRow = document.createElement('div');
      topRow.style.cssText = 'display:flex;justify-content:space-between;align-items:center;margin-bottom:6px';
      var nameEl = document.createElement('div');
      nameEl.style.cssText = 'font-size:12px;font-weight:500;color:var(--text-primary)';
      nameEl.textContent = model.name;
      var sizeEl = document.createElement('div');
      sizeEl.style.cssText = 'font-size:10px;color:var(--text-dim);font-family:var(--mono)';
      sizeEl.textContent = model.totalLayers + ' layers \u00B7 ' + formatMB(model.totalSizeMB);
      topRow.appendChild(nameEl);
      topRow.appendChild(sizeEl);
      wrap.appendChild(topRow);

      // Split visualization bar
      var splitBar = document.createElement('div');
      splitBar.style.cssText = 'display:flex;height:20px;border-radius:4px;overflow:hidden;margin-bottom:4px';

      var vramPct = (model.vramLayers / model.totalLayers) * 100;
      var ramPct = 100 - vramPct;

      var vramSeg = document.createElement('div');
      vramSeg.style.cssText = 'background:var(--accent-cyan);height:100%;transition:width .3s ease;display:flex;align-items:center;justify-content:center;font-size:8px;color:#0a0e1a;font-weight:600;min-width:' + (vramPct > 5 ? '0' : '20') + 'px';
      vramSeg.style.width = vramPct + '%';
      vramSeg.textContent = vramPct > 10 ? 'VRAM ' + model.vramLayers + 'L' : '';

      var ramSeg = document.createElement('div');
      ramSeg.style.cssText = 'background:var(--accent-amber);height:100%;transition:width .3s ease;display:flex;align-items:center;justify-content:center;font-size:8px;color:#0a0e1a;font-weight:600;min-width:' + (ramPct > 5 ? '0' : '20') + 'px';
      ramSeg.style.width = ramPct + '%';
      ramSeg.textContent = ramPct > 10 ? 'RAM ' + (model.totalLayers - model.vramLayers) + 'L' : '';

      splitBar.appendChild(vramSeg);
      splitBar.appendChild(ramSeg);
      wrap.appendChild(splitBar);

      // Slider
      var sliderRow = document.createElement('div');
      sliderRow.style.cssText = 'display:flex;align-items:center;gap:8px;margin-bottom:4px';

      var sliderLabel = document.createElement('span');
      sliderLabel.style.cssText = 'font-size:9px;color:var(--text-dim);width:60px;flex-shrink:0';
      sliderLabel.textContent = 'VRAM Layers';

      var slider = document.createElement('input');
      slider.type = 'range';
      slider.min = '0';
      slider.max = String(model.totalLayers);
      slider.value = String(model.vramLayers);
      slider.style.cssText = 'flex:1;accent-color:var(--accent-cyan);height:4px';

      var sliderVal = document.createElement('span');
      sliderVal.style.cssText = 'font-size:10px;color:var(--accent-cyan);font-family:var(--mono);width:40px;text-align:right';
      sliderVal.textContent = model.vramLayers + '/' + model.totalLayers;

      slider.addEventListener('input', function() {
        var newVal = parseInt(slider.value);
        model.vramLayers = newVal;
        sliderVal.textContent = newVal + '/' + model.totalLayers;
        var newVramPct = (newVal / model.totalLayers) * 100;
        vramSeg.style.width = newVramPct + '%';
        vramSeg.textContent = newVramPct > 10 ? 'VRAM ' + newVal + 'L' : '';
        ramSeg.style.width = (100 - newVramPct) + '%';
        ramSeg.textContent = (100 - newVramPct) > 10 ? 'RAM ' + (model.totalLayers - newVal) + 'L' : '';
        // Update speed estimate
        updateSpeedEstimate(speedEl, model, newVal);
      });
      slider.addEventListener('change', function() {
        ipcSend('set_model_layers', { model_id: model.id, vram_layers: parseInt(slider.value) });
      });

      sliderRow.appendChild(sliderLabel);
      sliderRow.appendChild(slider);
      sliderRow.appendChild(sliderVal);
      wrap.appendChild(sliderRow);

      // Speed estimate
      var speedEl = document.createElement('div');
      speedEl.style.cssText = 'font-size:10px;color:var(--text-secondary);display:flex;gap:12px';
      updateSpeedEstimate(speedEl, model, model.vramLayers);
      wrap.appendChild(speedEl);

      container.appendChild(wrap);
    });
  }

  function updateSpeedEstimate(el, model, vramLayers) {
    var fullVramSpeed = model.maxTPS || 45;
    var fullRamSpeed = model.minTPS || 8;
    var ratio = model.totalLayers > 0 ? vramLayers / model.totalLayers : 0;
    var estSpeed = fullRamSpeed + (fullVramSpeed - fullRamSpeed) * ratio;
    var estVramMB = (model.totalSizeMB || 0) * ratio;

    el.innerHTML = '';
    var speedSpan = document.createElement('span');
    speedSpan.textContent = 'Est. Speed: ~' + estSpeed.toFixed(1) + ' tok/s';
    speedSpan.style.color = ratio > 0.7 ? 'var(--accent-green)' : ratio > 0.3 ? 'var(--accent-amber)' : 'var(--accent-red)';
    var vramSpan = document.createElement('span');
    vramSpan.textContent = 'VRAM: ~' + formatMB(estVramMB);
    vramSpan.style.color = 'var(--text-dim)';
    el.appendChild(speedSpan);
    el.appendChild(vramSpan);
  }

  function getDemoModels() {
    return [
      { id: 'llama3-70b', name: 'Llama 3.1 70B (Q4_K_M)', totalLayers: 81, vramLayers: 60, totalSizeMB: 39000, maxTPS: 25, minTPS: 4 },
      { id: 'codellama-34b', name: 'CodeLlama 34B (Q5_K_M)', totalLayers: 49, vramLayers: 49, totalSizeMB: 22000, maxTPS: 42, minTPS: 8 },
      { id: 'mistral-7b', name: 'Mistral 7B (Q8_0)', totalLayers: 33, vramLayers: 33, totalSizeMB: 7600, maxTPS: 85, minTPS: 20 }
    ];
  }

  /* ── Leak detection panel ────────────────────────────────────── */
  function renderLeakAlerts(container) {
    container.innerHTML = '';
    var alerts = state.leakAlerts.length > 0 ? state.leakAlerts : getDemoLeakAlerts();

    if (alerts.length === 0) {
      var noAlerts = document.createElement('div');
      noAlerts.style.cssText = 'display:flex;align-items:center;gap:8px;padding:10px;font-size:11px;color:var(--accent-green);background:rgba(107,203,119,0.08);border-radius:6px';
      noAlerts.textContent = '\u2714 No VRAM leaks detected';
      container.appendChild(noAlerts);
      return;
    }

    alerts.forEach(function(alert) {
      var row = document.createElement('div');
      row.style.cssText = 'display:flex;align-items:flex-start;gap:8px;padding:8px;margin-bottom:6px;border-radius:6px;background:' + (alert.severity === 'high' ? 'rgba(224,96,96,0.08)' : 'rgba(212,165,116,0.08)');

      var icon = document.createElement('span');
      icon.style.cssText = 'font-size:14px;flex-shrink:0;margin-top:1px';
      icon.textContent = alert.severity === 'high' ? '\u26D4' : '\u26A0';

      var info = document.createElement('div');
      var title = document.createElement('div');
      title.style.cssText = 'font-size:11px;font-weight:500;color:' + (alert.severity === 'high' ? 'var(--accent-red)' : 'var(--accent-amber)');
      title.textContent = alert.process;
      var detail = document.createElement('div');
      detail.style.cssText = 'font-size:10px;color:var(--text-secondary);margin-top:2px';
      detail.textContent = alert.message;
      var time = document.createElement('div');
      time.style.cssText = 'font-size:9px;color:var(--text-dim);margin-top:2px;font-family:var(--mono)';
      time.textContent = alert.time;

      info.appendChild(title);
      info.appendChild(detail);
      info.appendChild(time);
      row.appendChild(icon);
      row.appendChild(info);
      container.appendChild(row);
    });
  }

  function getDemoLeakAlerts() {
    return [
      { process: 'chrome.exe', message: 'VRAM grew 120 MB in 30 min without active GPU use', severity: 'medium', time: '12 min ago' },
      { process: 'python.exe (torch)', message: 'Allocated 800 MB but only 200 MB referenced. Possible leak.', severity: 'high', time: '3 min ago' }
    ];
  }

  /* ── GPU tab selector ────────────────────────────────────────── */
  function renderGpuTabs(container) {
    container.innerHTML = '';
    var gpus = state.gpus.length > 0 ? state.gpus : [defaultGpu];

    if (gpus.length <= 1) return; // no tabs needed for single GPU

    var tabRow = document.createElement('div');
    tabRow.style.cssText = 'display:flex;gap:6px;margin-bottom:16px';

    gpus.forEach(function(gpu, idx) {
      var btn = document.createElement('button');
      var isActive = idx === state.selectedGpuIdx;
      btn.style.cssText = 'padding:6px 14px;border:1px solid ' + (isActive ? 'var(--accent-cyan)' : 'var(--border)') + ';border-radius:6px;background:' + (isActive ? 'var(--accent-cyan-dim)' : 'var(--bg-card)') + ';color:' + (isActive ? 'var(--accent-cyan)' : 'var(--text-secondary)') + ';font-size:11px;font-family:var(--font);cursor:pointer;transition:var(--transition);font-weight:500';
      btn.textContent = 'GPU ' + idx + ': ' + (gpu.name || 'Unknown');
      btn.addEventListener('click', function() {
        state.selectedGpuIdx = idx;
        updateUI();
      });
      tabRow.appendChild(btn);
    });

    container.appendChild(tabRow);
  }

  /* ── Create action button ────────────────────────────────────── */
  function createActionBtn(text, icon, onClick) {
    var btn = document.createElement('button');
    btn.className = 'btn';
    btn.style.cssText = 'display:flex;align-items:center;gap:8px;padding:9px 12px;border:1px solid var(--border);border-radius:8px;background:var(--bg-card);color:var(--text-primary);font-size:12px;font-family:var(--font);cursor:pointer;transition:var(--transition)';
    var icoSpan = document.createElement('span');
    icoSpan.className = 'ico';
    icoSpan.textContent = icon;
    btn.appendChild(icoSpan);
    var textSpan = document.createElement('span');
    textSpan.textContent = text;
    btn.appendChild(textSpan);
    btn.addEventListener('click', function() { if (onClick) onClick(btn); });
    return btn;
  }

  /* ══════════════════════════════════════════════════════════════ */
  /* INIT                                                          */
  /* ══════════════════════════════════════════════════════════════ */
  window.RuVectorPages.gpu_Init = function(container) {
    state.container = container;
    container.innerHTML = '';

    var inner = document.createElement('div');
    inner.className = 'page-inner';
    inner.style.cssText = 'padding:24px 28px;max-width:1400px';

    // Header
    var header = document.createElement('div');
    header.className = 'page-header';
    header.innerHTML = '<span class="page-icon">&#127918;</span><h2>GPU Memory Optimizer</h2>';
    inner.appendChild(header);

    var statusEl = document.createElement('div');
    statusEl.style.cssText = 'display:inline-block;font-size:9px;font-weight:600;letter-spacing:1px;text-transform:uppercase;padding:3px 10px;border-radius:4px;margin-bottom:16px;background:var(--accent-cyan-dim);color:var(--accent-cyan)';
    statusEl.textContent = 'ADR-022 \u00B7 Active';
    inner.appendChild(statusEl);

    var desc = document.createElement('p');
    desc.className = 'page-desc';
    desc.textContent = 'Monitors and optimizes GPU/VRAM via NVML (NVIDIA) and DXGI (all GPUs). Categorizes VRAM usage, enables AI model layer offloading between system RAM and VRAM for ML workloads.';
    inner.appendChild(desc);

    // GPU tabs
    var gpuTabContainer = document.createElement('div');
    gpuTabContainer.setAttribute('data-gpu-tabs', '');
    inner.appendChild(gpuTabContainer);

    // ── Top row: GPU info + VRAM gauge + small gauges ──
    var topRow = document.createElement('div');
    topRow.style.cssText = 'display:grid;grid-template-columns:1fr 200px 1fr;gap:16px;margin-bottom:16px';

    // GPU info card
    var infoCard = document.createElement('div');
    infoCard.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px';
    var infoTitle = document.createElement('div');
    infoTitle.className = 'card-title';
    infoTitle.textContent = 'GPU Information';
    infoCard.appendChild(infoTitle);

    var infoFields = document.createElement('div');
    infoFields.setAttribute('data-gpu-info', '');
    infoCard.appendChild(infoFields);
    topRow.appendChild(infoCard);

    // VRAM gauge card
    var gaugeCard = document.createElement('div');
    gaugeCard.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px;display:flex;flex-direction:column;align-items:center;justify-content:center';
    var gaugeTitle = document.createElement('div');
    gaugeTitle.className = 'card-title';
    gaugeTitle.style.textAlign = 'center';
    gaugeTitle.textContent = 'VRAM Usage';
    gaugeCard.appendChild(gaugeTitle);

    var vramCanvas = document.createElement('canvas');
    vramCanvas.setAttribute('data-vram-gauge', '');
    vramCanvas.style.cssText = 'width:160px;height:160px';
    gaugeCard.appendChild(vramCanvas);
    topRow.appendChild(gaugeCard);

    // Small gauges card
    var smallGaugesCard = document.createElement('div');
    smallGaugesCard.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px';
    var sgTitle = document.createElement('div');
    sgTitle.className = 'card-title';
    sgTitle.textContent = 'GPU Metrics';
    smallGaugesCard.appendChild(sgTitle);

    var sgRow = document.createElement('div');
    sgRow.style.cssText = 'display:grid;grid-template-columns:1fr 1fr;gap:12px;justify-items:center;margin-top:6px';
    sgRow.setAttribute('data-small-gauges', '');

    var tempG = createSmallGauge('temp', 'Temp', '\u00B0C', 100, 64);
    var utilG = createSmallGauge('util', 'GPU Load', '%', 100, 64);
    var powerG = createSmallGauge('power', 'Power', 'W', 450, 64);
    var clockG = createSmallGauge('clock', 'Clock', 'MHz', 3000, 64);

    sgRow.appendChild(tempG.element);
    sgRow.appendChild(utilG.element);
    sgRow.appendChild(powerG.element);
    sgRow.appendChild(clockG.element);
    smallGaugesCard.appendChild(sgRow);
    topRow.appendChild(smallGaugesCard);

    inner.appendChild(topRow);

    // ── Chart row ──
    var chartCard = document.createElement('div');
    chartCard.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px;margin-bottom:16px';
    var chartTitle = document.createElement('div');
    chartTitle.className = 'card-title';
    chartTitle.textContent = 'VRAM Usage Timeline (Last Hour)';
    chartCard.appendChild(chartTitle);

    var chartWrap = document.createElement('div');
    chartWrap.style.cssText = 'width:100%;height:200px;position:relative';
    var chartCanvas = document.createElement('canvas');
    chartCanvas.style.cssText = 'width:100%;height:100%;display:block';
    chartWrap.appendChild(chartCanvas);
    chartCard.appendChild(chartWrap);
    state.canvasEl = chartCanvas;
    inner.appendChild(chartCard);

    // ── Bottom row: processes + models + leaks + actions ──
    var bottomRow = document.createElement('div');
    bottomRow.style.cssText = 'display:grid;grid-template-columns:1fr 1fr;gap:16px';

    // Process VRAM table
    var procCard = document.createElement('div');
    procCard.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px';
    var procTitle = document.createElement('div');
    procTitle.className = 'card-title';
    procTitle.textContent = 'Per-Process VRAM Usage';
    procCard.appendChild(procTitle);
    var procContainer = document.createElement('div');
    procContainer.setAttribute('data-gpu-procs', '');
    procContainer.style.cssText = 'max-height:250px;overflow-y:auto';
    procCard.appendChild(procContainer);
    bottomRow.appendChild(procCard);

    // AI Model layer manager
    var modelCard = document.createElement('div');
    modelCard.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px';
    var modelTitle = document.createElement('div');
    modelTitle.className = 'card-title';
    modelTitle.textContent = 'AI Model Layer Manager';
    modelCard.appendChild(modelTitle);
    var modelContainer = document.createElement('div');
    modelContainer.setAttribute('data-model-manager', '');
    modelContainer.style.cssText = 'max-height:300px;overflow-y:auto';
    modelCard.appendChild(modelContainer);
    bottomRow.appendChild(modelCard);

    inner.appendChild(bottomRow);

    // ── Actions + leaks row ──
    var actionsRow = document.createElement('div');
    actionsRow.style.cssText = 'display:grid;grid-template-columns:1fr 1fr;gap:16px;margin-top:16px';

    // Actions card
    var actionsCard = document.createElement('div');
    actionsCard.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px';
    var actionsTitle = document.createElement('div');
    actionsTitle.className = 'card-title';
    actionsTitle.textContent = 'Optimization Actions';
    actionsCard.appendChild(actionsTitle);

    var actionsGrid = document.createElement('div');
    actionsGrid.style.cssText = 'display:flex;flex-direction:column;gap:8px';

    actionsGrid.appendChild(createActionBtn('Flush GPU Caches', '\uD83D\uDDD1', function(btn) {
      btn.disabled = true;
      btn.querySelector('span:last-child').textContent = 'Flushing...';
      ipcSend('flush_gpu_cache');
      setTimeout(function() {
        btn.disabled = false;
        btn.querySelector('span:last-child').textContent = 'Flush GPU Caches';
      }, 2000);
    }));

    actionsGrid.appendChild(createActionBtn('Disable Browser GPU Accel', '\uD83C\uDF10', function(btn) {
      ipcSend('optimize_vram', { action: 'disable_browser_gpu' });
    }));

    actionsGrid.appendChild(createActionBtn('Suggest Lower Textures', '\uD83C\uDFA8', function(btn) {
      ipcSend('optimize_vram', { action: 'suggest_lower_textures' });
    }));

    actionsGrid.appendChild(createActionBtn('Optimize VRAM', '\u26A1', function(btn) {
      btn.disabled = true;
      btn.querySelector('span:last-child').textContent = 'Optimizing...';
      ipcSend('optimize_vram', { action: 'full_optimize' });
      setTimeout(function() {
        btn.disabled = false;
        btn.querySelector('span:last-child').textContent = 'Optimize VRAM';
      }, 3000);
    }));

    actionsCard.appendChild(actionsGrid);
    actionsRow.appendChild(actionsCard);

    // Leak alerts card
    var leakCard = document.createElement('div');
    leakCard.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px';
    var leakTitle = document.createElement('div');
    leakTitle.className = 'card-title';
    leakTitle.textContent = 'VRAM Leak Detection';
    leakCard.appendChild(leakTitle);
    var leakContainer = document.createElement('div');
    leakContainer.setAttribute('data-leak-alerts', '');
    leakCard.appendChild(leakContainer);
    actionsRow.appendChild(leakCard);

    inner.appendChild(actionsRow);
    container.appendChild(inner);

    // Generate demo VRAM history
    if (state.vramHistory.length === 0) {
      var gpu = getGpu();
      for (var i = 0; i < 60; i++) {
        var base = gpu.vramUsedMB * 0.8;
        var textures = base * 0.3 + Math.sin(i * 0.1) * 200;
        var framebuffers = base * 0.15 + Math.sin(i * 0.08) * 100;
        var models = base * 0.45 + Math.sin(i * 0.05) * 300;
        var other = base * 0.1 + Math.random() * 100;
        state.vramHistory.push({
          ts: Date.now() - (60 - i) * 60000,
          textures: Math.max(0, textures),
          framebuffers: Math.max(0, framebuffers),
          models: Math.max(0, models),
          other: Math.max(0, other)
        });
      }
    }

    // Request initial data
    ipcSend('get_gpu_status');
    ipcSend('get_gpu_processes');
    ipcSend('get_vram_history');

    state.initialized = true;
    updateUI();
  };

  /* ══════════════════════════════════════════════════════════════ */
  /* UPDATE                                                        */
  /* ══════════════════════════════════════════════════════════════ */
  window.RuVectorPages.gpu_Update = function(data) {
    if (!data) return;

    if (data.gpus) state.gpus = data.gpus;
    if (data.processes) state.processes = data.processes;
    if (data.models) state.models = data.models;
    if (data.leak_alerts) state.leakAlerts = data.leak_alerts;
    if (data.vram_history) state.vramHistory = data.vram_history;

    // Single GPU update
    if (data.name !== undefined || data.vram_used_mb !== undefined) {
      var gpu = getGpu();
      if (data.name !== undefined) gpu.name = data.name;
      if (data.vendor !== undefined) gpu.vendor = data.vendor;
      if (data.vram_total_mb !== undefined) gpu.vramTotalMB = data.vram_total_mb;
      if (data.vram_used_mb !== undefined) gpu.vramUsedMB = data.vram_used_mb;
      if (data.temp_c !== undefined) gpu.tempC = data.temp_c;
      if (data.power_draw_w !== undefined) gpu.powerDrawW = data.power_draw_w;
      if (data.clock_mhz !== undefined) gpu.clockMHz = data.clock_mhz;
      if (data.util_pct !== undefined) gpu.utilPct = data.util_pct;
      if (data.mem_util_pct !== undefined) gpu.memUtilPct = data.mem_util_pct;
      if (data.driver_api !== undefined) gpu.driverAPI = data.driver_api;
      if (data.fan_pct !== undefined) gpu.fanPct = data.fan_pct;

      // Append to VRAM history
      state.vramHistory.push({
        ts: Date.now(),
        textures: (gpu.vramUsedMB || 0) * 0.3,
        framebuffers: (gpu.vramUsedMB || 0) * 0.15,
        models: (gpu.vramUsedMB || 0) * 0.45,
        other: (gpu.vramUsedMB || 0) * 0.1
      });
      if (state.vramHistory.length > state.historyMaxLen) {
        state.vramHistory.shift();
      }
    }

    if (state.initialized) updateUI();
  };

  /* ── Refresh display ─────────────────────────────────────────── */
  function updateUI() {
    if (!state.container) return;
    var gpu = getGpu();

    // GPU tabs
    var gpuTabs = state.container.querySelector('[data-gpu-tabs]');
    if (gpuTabs) renderGpuTabs(gpuTabs);

    // GPU info fields
    var infoEl = state.container.querySelector('[data-gpu-info]');
    if (infoEl) {
      infoEl.innerHTML = '';
      var fields = [
        ['Name', gpu.name],
        ['Vendor', gpu.vendor],
        ['Total VRAM', formatMB(gpu.vramTotalMB)],
        ['Temperature', gpu.tempC + '\u00B0C'],
        ['Power Draw', gpu.powerDrawW + 'W / ' + gpu.powerLimitW + 'W'],
        ['Clock Speed', gpu.clockMHz + ' MHz'],
        ['GPU Utilization', gpu.utilPct + '%'],
        ['Memory Util', gpu.memUtilPct + '%'],
        ['Driver API', gpu.driverAPI],
        ['Fan', gpu.fanPct + '%'],
        ['PCIe', 'Gen ' + gpu.pcieGen + ' x' + gpu.pcieLanes]
      ];
      fields.forEach(function(f) {
        var row = document.createElement('div');
        row.style.cssText = 'display:flex;justify-content:space-between;padding:4px 0;font-size:11px;border-bottom:1px solid var(--border)';
        var lbl = document.createElement('span');
        lbl.style.color = 'var(--text-secondary)';
        lbl.textContent = f[0];
        var val = document.createElement('span');
        val.style.cssText = 'font-family:var(--mono);color:var(--text-primary);font-size:10px';
        val.textContent = f[1];
        row.appendChild(lbl);
        row.appendChild(val);
        infoEl.appendChild(row);
      });
    }

    // VRAM gauge
    var vramGauge = state.container.querySelector('[data-vram-gauge]');
    if (vramGauge) drawVramGauge(vramGauge);

    // Small gauges
    var tempCanvas = state.container.querySelector('[data-sgauge="temp"]');
    if (tempCanvas) drawSmallGauge(tempCanvas, gpu.tempC, 100, pctColor(gpu.tempC), 64);
    var utilCanvas = state.container.querySelector('[data-sgauge="util"]');
    if (utilCanvas) drawSmallGauge(utilCanvas, gpu.utilPct, 100, pctColor(gpu.utilPct), 64);
    var powerCanvas = state.container.querySelector('[data-sgauge="power"]');
    if (powerCanvas) drawSmallGauge(powerCanvas, gpu.powerDrawW, gpu.powerLimitW || 450, pctColor((gpu.powerDrawW / (gpu.powerLimitW || 450)) * 100), 64);
    var clockCanvas = state.container.querySelector('[data-sgauge="clock"]');
    if (clockCanvas) drawSmallGauge(clockCanvas, gpu.clockMHz, gpu.maxClockMHz || 3000, getCSS('--accent-cyan'), 64);

    // Chart
    drawVramChart();

    // Process table
    var procEl = state.container.querySelector('[data-gpu-procs]');
    if (procEl) renderProcessTable(procEl);

    // Model manager
    var modelEl = state.container.querySelector('[data-model-manager]');
    if (modelEl) renderModelManager(modelEl);

    // Leak alerts
    var leakEl = state.container.querySelector('[data-leak-alerts]');
    if (leakEl) renderLeakAlerts(leakEl);
  }

})();
