/**
 * ADR-016: WSL2 Memory Governor
 * Bridges Windows and Linux memory subsystems via /proc/meminfo monitoring.
 * Applies governor policies to prevent WSL2 from consuming all host memory.
 */
(function(){
  'use strict';

  window.RuVectorPages = window.RuVectorPages || {};

  // ── State ──────────────────────────────────────────────────────
  var state = {
    detected: null,
    distro: '--',
    kernel: '--',
    vmMemoryGb: 0,
    maxMemoryGb: 8,
    hostRamGb: 32,
    currentUsageGb: 0,
    swapSizeGb: 2,
    swapPath: '/swap/file',
    pressure: 'low',
    autoGovernor: false,
    processes: [],
    history: [],
    wslConfig: {
      memory: '8GB',
      swap: '2GB',
      processors: 4,
      localhostForwarding: true
    },
    _container: null,
    _chartCanvas: null,
    _animFrame: null,
    _refreshTimer: null
  };

  // ── IPC Helper ─────────────────────────────────────────────────
  function ipcSend(type, payload) {
    if (window.ipc) {
      var msg = Object.assign({ type: type }, payload || {});
      window.ipc.postMessage(JSON.stringify(msg));
    }
  }

  // ── Safe DOM helpers ───────────────────────────────────────────
  function el(id) {
    return state._container ? state._container.querySelector('#' + id) : null;
  }

  function setText(id, text) {
    var node = el(id);
    if (node) node.textContent = String(text);
  }

  function createEl(tag, cls, parent) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (parent) parent.appendChild(e);
    return e;
  }

  // ── Pressure colors ────────────────────────────────────────────
  function pressureColor(level) {
    switch (level) {
      case 'critical': return 'var(--accent-red)';
      case 'high': return 'var(--accent-amber)';
      case 'medium': return 'var(--accent-purple)';
      default: return 'var(--accent-green)';
    }
  }

  function pressureLabel(level) {
    return (level || 'low').charAt(0).toUpperCase() + (level || 'low').slice(1);
  }

  // ── Build page HTML ────────────────────────────────────────────
  function buildUI(container) {
    state._container = container;
    container.innerHTML = '';

    var inner = createEl('div', 'page-inner', container);

    // Header
    var hdr = createEl('div', 'page-header', inner);
    var ico = createEl('span', 'page-icon', hdr);
    ico.textContent = '\uD83D\uDC27';
    var h2 = createEl('h2', '', hdr);
    h2.textContent = 'WSL2 Memory Governor';

    var badge = createEl('div', 'page-status', inner);
    badge.textContent = 'ADR-016';
    badge.id = 'wsl2-status-badge';

    var desc = createEl('p', 'page-desc', inner);
    desc.textContent = 'Bridges Windows and Linux memory subsystems via /proc/meminfo monitoring. Applies governor policies to prevent WSL2 from consuming all host memory, with Docker container awareness.';

    var grid = createEl('div', 'page-grid', inner);

    // ── Card 1: Detection Status ─────────────────────────────────
    var c1 = createEl('div', 'ph-card', grid);
    createEl('div', 'ph-card-title', c1).textContent = 'WSL2 Detection';

    var statusRow = createEl('div', '', c1);
    statusRow.id = 'wsl2-detection-status';
    statusRow.style.cssText = 'display:flex;align-items:center;gap:10px;margin-bottom:12px;padding:10px;border-radius:8px;background:var(--bg-primary)';

    var statusDot = createEl('span', '', statusRow);
    statusDot.id = 'wsl2-detect-dot';
    statusDot.style.cssText = 'width:12px;height:12px;border-radius:50%;background:var(--text-dim);flex-shrink:0';

    var statusText = createEl('span', '', statusRow);
    statusText.id = 'wsl2-detect-text';
    statusText.style.cssText = 'font-size:12px;font-weight:500';
    statusText.textContent = 'Checking...';

    buildField(c1, 'Distro', 'wsl2-distro', '--');
    buildField(c1, 'Kernel Version', 'wsl2-kernel', '--');
    buildField(c1, 'VM Memory', 'wsl2-vm-mem', '-- GB');

    // ── Card 2: Memory Pressure ──────────────────────────────────
    var c2 = createEl('div', 'ph-card', grid);
    createEl('div', 'ph-card-title', c2).textContent = 'Memory Pressure';

    var pressureBox = createEl('div', '', c2);
    pressureBox.style.cssText = 'text-align:center;padding:12px 0';

    var pressureIndicator = createEl('div', '', pressureBox);
    pressureIndicator.id = 'wsl2-pressure-indicator';
    pressureIndicator.style.cssText = 'display:inline-block;padding:8px 24px;border-radius:20px;font-size:14px;font-weight:600;letter-spacing:1px;text-transform:uppercase;background:var(--accent-green);color:#0a0e1a;transition:all 0.3s ease';
    pressureIndicator.textContent = 'LOW';

    // Usage gauge
    var gaugeWrap = createEl('div', '', c2);
    gaugeWrap.style.cssText = 'margin-top:14px';

    var gaugeLabel = createEl('div', '', gaugeWrap);
    gaugeLabel.style.cssText = 'display:flex;justify-content:space-between;font-size:10px;color:var(--text-secondary);margin-bottom:4px';
    var gl1 = createEl('span', '', gaugeLabel);
    gl1.textContent = 'WSL2 Memory Usage';
    var gl2 = createEl('span', '', gaugeLabel);
    gl2.id = 'wsl2-usage-pct';
    gl2.style.cssText = 'font-family:var(--mono)';
    gl2.textContent = '0%';

    var gaugeTrack = createEl('div', '', gaugeWrap);
    gaugeTrack.style.cssText = 'height:10px;background:var(--gauge-track);border-radius:5px;overflow:hidden';
    var gaugeFill = createEl('div', '', gaugeTrack);
    gaugeFill.id = 'wsl2-usage-bar';
    gaugeFill.style.cssText = 'height:100%;width:0%;background:var(--accent-cyan);border-radius:5px;transition:width 0.6s ease,background 0.3s ease';

    buildField(c2, 'Current Usage', 'wsl2-current-usage', '0 GB');
    buildField(c2, 'Allocated Max', 'wsl2-max-alloc', '0 GB');

    // ── Card 3: .wslconfig Editor ────────────────────────────────
    var c3 = createEl('div', 'ph-card', grid);
    createEl('div', 'ph-card-title', c3).textContent = '.wslconfig Settings';

    // Memory slider
    buildSliderRow(c3, 'Max Memory', 'wsl2-mem-slider', 1, 64, 8, 'GB', function(val) {
      state.maxMemoryGb = val;
      state.wslConfig.memory = val + 'GB';
    });

    // Swap slider
    buildSliderRow(c3, 'Swap Size', 'wsl2-swap-slider', 0, 16, 2, 'GB', function(val) {
      state.swapSizeGb = val;
      state.wslConfig.swap = val + 'GB';
    });

    // Processors slider
    buildSliderRow(c3, 'Processors', 'wsl2-proc-slider', 1, 32, 4, '', function(val) {
      state.wslConfig.processors = val;
    });

    // Localhost forwarding toggle
    var toggleRow = createEl('div', 'toggle-row', c3);
    toggleRow.style.marginTop = '8px';
    var toggleLabel = createEl('span', '', toggleRow);
    toggleLabel.textContent = 'Localhost Forwarding';
    var toggle = createEl('div', 'toggle on', toggleRow);
    toggle.id = 'wsl2-localhost-toggle';
    toggle.addEventListener('click', function() {
      toggle.classList.toggle('on');
      state.wslConfig.localhostForwarding = toggle.classList.contains('on');
    });

    // Apply button
    var applyWrap = createEl('div', 'ph-actions', c3);
    var applyBtn = createEl('button', 'btn', applyWrap);
    applyBtn.style.cssText = 'cursor:pointer;opacity:1';
    applyBtn.innerHTML = '<span class="ico">&#128190;</span> Apply Config';
    applyBtn.addEventListener('click', function() {
      ipcSend('set_wsl2_config', { config: state.wslConfig });
      showToastSafe('WSL2 Config', 'Configuration saved. Restart WSL2 to apply.', 'success');
    });

    // ── Card 4: Auto-Governor ────────────────────────────────────
    var c4 = createEl('div', 'ph-card', grid);
    createEl('div', 'ph-card-title', c4).textContent = 'Governor Controls';

    var govToggle = createEl('div', 'toggle-row', c4);
    var govLabel = createEl('span', '', govToggle);
    govLabel.textContent = 'Auto-Governor';
    var govSwitch = createEl('div', 'toggle', govToggle);
    govSwitch.id = 'wsl2-auto-governor';
    govSwitch.addEventListener('click', function() {
      govSwitch.classList.toggle('on');
      state.autoGovernor = govSwitch.classList.contains('on');
      ipcSend('set_wsl2_config', { auto_governor: state.autoGovernor });
    });

    var govDesc = createEl('div', '', c4);
    govDesc.style.cssText = 'font-size:10px;color:var(--text-dim);margin:6px 0 12px;line-height:1.5';
    govDesc.textContent = 'Automatically adjusts WSL2 memory allocation based on host memory pressure.';

    var reclaimBtn = createEl('button', 'btn', c4);
    reclaimBtn.style.cssText = 'cursor:pointer;opacity:1;width:100%';
    reclaimBtn.innerHTML = '<span class="ico">&#9889;</span> Reclaim Memory Now';
    reclaimBtn.addEventListener('click', function() {
      reclaimBtn.disabled = true;
      reclaimBtn.querySelector('.ico').textContent = '\u23F3';
      ipcSend('reclaim_wsl2_memory');
      setTimeout(function() {
        reclaimBtn.disabled = false;
        reclaimBtn.innerHTML = '<span class="ico">&#9889;</span> Reclaim Memory Now';
      }, 3000);
    });

    // ── Card 5: Process List ─────────────────────────────────────
    var c5 = createEl('div', 'ph-card', grid);
    c5.style.gridColumn = '1 / -1';
    createEl('div', 'ph-card-title', c5).textContent = 'WSL2 Processes';

    var procHeader = createEl('div', 'proc-row', c5);
    procHeader.style.cssText = 'font-size:10px;font-weight:600;color:var(--text-dim);letter-spacing:0.5px;text-transform:uppercase;border-bottom:1px solid var(--border);padding-bottom:6px';
    addProcCol(procHeader, 'Process Name', 'flex:2');
    addProcCol(procHeader, 'PID', 'width:60px;text-align:right');
    addProcCol(procHeader, 'Memory', 'width:80px;text-align:right');
    addProcCol(procHeader, 'CPU %', 'width:60px;text-align:right');

    var procList = createEl('div', 'proc-list', c5);
    procList.id = 'wsl2-proc-list';
    procList.style.maxHeight = '240px';
    var procEmpty = createEl('div', '', procList);
    procEmpty.style.cssText = 'font-size:11px;color:var(--text-dim);padding:8px 0';
    procEmpty.textContent = 'No WSL2 processes detected';

    // ── Card 6: History Chart ────────────────────────────────────
    var c6 = createEl('div', 'ph-card', grid);
    c6.style.gridColumn = '1 / -1';
    createEl('div', 'ph-card-title', c6).textContent = 'WSL2 Memory Usage Over Time';

    var chartWrap = createEl('div', '', c6);
    chartWrap.style.cssText = 'height:180px;position:relative;background:var(--bg-primary);border-radius:8px;overflow:hidden';
    var canvas = document.createElement('canvas');
    canvas.id = 'wsl2-history-chart';
    canvas.style.cssText = 'width:100%;height:100%;display:block';
    chartWrap.appendChild(canvas);
    state._chartCanvas = canvas;

    // Seed demo history
    seedHistory();

    // Request initial data
    ipcSend('get_wsl2_status');
    ipcSend('get_wsl2_processes');

    // Start refresh timer
    state._refreshTimer = setInterval(function() {
      ipcSend('get_wsl2_status');
      ipcSend('get_wsl2_processes');
      pushHistoryPoint();
      drawChart();
    }, 5000);

    // Initial chart draw
    setTimeout(function() { resizeCanvas(); drawChart(); }, 100);
  }

  // ── Field Builder ──────────────────────────────────────────────
  function buildField(parent, label, id, defaultVal) {
    var row = createEl('div', 'ph-field', parent);
    var lbl = createEl('span', 'ph-field-label', row);
    lbl.textContent = label;
    var val = createEl('span', 'ph-field-value', row);
    val.id = id;
    val.textContent = defaultVal;
    return row;
  }

  // ── Slider Row Builder ─────────────────────────────────────────
  function buildSliderRow(parent, label, id, min, max, initial, unit, onChange) {
    var row = createEl('div', '', parent);
    row.style.cssText = 'margin:8px 0';

    var top = createEl('div', '', row);
    top.style.cssText = 'display:flex;justify-content:space-between;font-size:10px;color:var(--text-secondary);margin-bottom:4px';
    var topLabel = createEl('span', '', top);
    topLabel.textContent = label;
    var topVal = createEl('span', '', top);
    topVal.id = id + '-val';
    topVal.style.cssText = 'font-family:var(--mono);color:var(--text-primary);font-weight:500';
    topVal.textContent = initial + (unit ? ' ' + unit : '');

    var slider = document.createElement('input');
    slider.type = 'range';
    slider.id = id;
    slider.min = String(min);
    slider.max = String(max);
    slider.value = String(initial);
    slider.style.cssText = 'width:100%;height:4px;appearance:none;-webkit-appearance:none;background:var(--gauge-track);border-radius:2px;outline:none;cursor:pointer';
    row.appendChild(slider);

    slider.addEventListener('input', function() {
      var v = parseInt(slider.value, 10);
      topVal.textContent = v + (unit ? ' ' + unit : '');
      if (onChange) onChange(v);
    });

    return row;
  }

  // ── Process column helper ──────────────────────────────────────
  function addProcCol(parent, text, style) {
    var span = createEl('span', '', parent);
    span.style.cssText = style;
    span.textContent = text;
  }

  // ── History Seeding ────────────────────────────────────────────
  function seedHistory() {
    state.history = [];
    var now = Date.now();
    for (var i = 60; i >= 0; i--) {
      state.history.push({
        time: now - i * 5000,
        usage: 2.5 + Math.random() * 3 + Math.sin(i * 0.15) * 0.8,
        allocated: 8
      });
    }
  }

  function pushHistoryPoint() {
    var usage = state.currentUsageGb || (2.5 + Math.random() * 3);
    state.history.push({
      time: Date.now(),
      usage: usage,
      allocated: state.maxMemoryGb
    });
    if (state.history.length > 120) state.history.shift();
  }

  // ── Canvas Chart ───────────────────────────────────────────────
  function resizeCanvas() {
    var canvas = state._chartCanvas;
    if (!canvas) return;
    var rect = canvas.parentElement.getBoundingClientRect();
    canvas.width = rect.width * (window.devicePixelRatio || 1);
    canvas.height = rect.height * (window.devicePixelRatio || 1);
  }

  function drawChart() {
    var canvas = state._chartCanvas;
    if (!canvas || !canvas.getContext) return;
    var ctx = canvas.getContext('2d');
    var w = canvas.width;
    var h = canvas.height;
    var dpr = window.devicePixelRatio || 1;
    var data = state.history;

    ctx.clearRect(0, 0, w, h);

    if (data.length < 2) return;

    var maxVal = state.maxMemoryGb || 8;
    var pad = { top: 20 * dpr, right: 50 * dpr, bottom: 24 * dpr, left: 10 * dpr };
    var cw = w - pad.left - pad.right;
    var ch = h - pad.top - pad.bottom;

    // Grid lines
    ctx.strokeStyle = getCSS('--border');
    ctx.lineWidth = 0.5 * dpr;
    for (var g = 0; g <= 4; g++) {
      var gy = pad.top + ch * (1 - g / 4);
      ctx.beginPath();
      ctx.moveTo(pad.left, gy);
      ctx.lineTo(pad.left + cw, gy);
      ctx.stroke();

      // Labels
      ctx.fillStyle = getCSS('--text-dim');
      ctx.font = (9 * dpr) + 'px ' + getCSS('--mono');
      ctx.textAlign = 'left';
      ctx.fillText((maxVal * g / 4).toFixed(0) + 'G', pad.left + cw + 4 * dpr, gy + 3 * dpr);
    }

    // Allocated line (dashed)
    var allocY = pad.top + ch * (1 - maxVal / maxVal);
    ctx.strokeStyle = getCSS('--accent-amber');
    ctx.lineWidth = 1 * dpr;
    ctx.setLineDash([4 * dpr, 4 * dpr]);
    ctx.beginPath();
    ctx.moveTo(pad.left, allocY);
    ctx.lineTo(pad.left + cw, allocY);
    ctx.stroke();
    ctx.setLineDash([]);

    // Usage area fill
    ctx.beginPath();
    for (var i = 0; i < data.length; i++) {
      var x = pad.left + (i / (data.length - 1)) * cw;
      var y = pad.top + ch * (1 - Math.min(data[i].usage, maxVal) / maxVal);
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.lineTo(pad.left + cw, pad.top + ch);
    ctx.lineTo(pad.left, pad.top + ch);
    ctx.closePath();

    var grad = ctx.createLinearGradient(0, pad.top, 0, pad.top + ch);
    grad.addColorStop(0, 'rgba(78,205,196,0.35)');
    grad.addColorStop(1, 'rgba(78,205,196,0.02)');
    ctx.fillStyle = grad;
    ctx.fill();

    // Usage line
    ctx.beginPath();
    for (var j = 0; j < data.length; j++) {
      var lx = pad.left + (j / (data.length - 1)) * cw;
      var ly = pad.top + ch * (1 - Math.min(data[j].usage, maxVal) / maxVal);
      if (j === 0) ctx.moveTo(lx, ly);
      else ctx.lineTo(lx, ly);
    }
    ctx.strokeStyle = getCSS('--accent-cyan');
    ctx.lineWidth = 2 * dpr;
    ctx.stroke();

    // Time labels
    ctx.fillStyle = getCSS('--text-dim');
    ctx.font = (8 * dpr) + 'px ' + getCSS('--font');
    ctx.textAlign = 'center';
    ctx.fillText('5 min ago', pad.left, pad.top + ch + 16 * dpr);
    ctx.fillText('Now', pad.left + cw, pad.top + ch + 16 * dpr);
  }

  function getCSS(varName) {
    return getComputedStyle(document.documentElement).getPropertyValue(varName).trim();
  }

  // ── Toast passthrough ──────────────────────────────────────────
  function showToastSafe(title, msg, type) {
    if (typeof window.showToast === 'function') {
      window.showToast(title, msg, type);
    }
  }

  // ── Update Function (called from Rust IPC) ─────────────────────
  function updateUI(data) {
    if (!data || !state._container) return;

    if (data.detected !== undefined) {
      state.detected = data.detected;
      var dot = el('wsl2-detect-dot');
      var txt = el('wsl2-detect-text');
      if (dot) dot.style.background = data.detected ? 'var(--accent-green)' : 'var(--accent-red)';
      if (txt) txt.textContent = data.detected ? 'WSL2 Detected' : 'WSL2 Not Detected';
    }

    if (data.distro) { state.distro = data.distro; setText('wsl2-distro', data.distro); }
    if (data.kernel) { state.kernel = data.kernel; setText('wsl2-kernel', data.kernel); }
    if (data.vm_memory_gb !== undefined) { state.vmMemoryGb = data.vm_memory_gb; setText('wsl2-vm-mem', data.vm_memory_gb.toFixed(1) + ' GB'); }

    if (data.current_usage_gb !== undefined) {
      state.currentUsageGb = data.current_usage_gb;
      setText('wsl2-current-usage', data.current_usage_gb.toFixed(1) + ' GB');
      var pct = state.maxMemoryGb > 0 ? (data.current_usage_gb / state.maxMemoryGb * 100) : 0;
      pct = Math.min(100, Math.max(0, pct));
      setText('wsl2-usage-pct', pct.toFixed(0) + '%');
      var bar = el('wsl2-usage-bar');
      if (bar) {
        bar.style.width = pct + '%';
        bar.style.background = pct > 85 ? 'var(--accent-red)' : pct > 70 ? 'var(--accent-amber)' : 'var(--accent-cyan)';
      }
    }

    if (data.max_memory_gb !== undefined) {
      state.maxMemoryGb = data.max_memory_gb;
      setText('wsl2-max-alloc', data.max_memory_gb.toFixed(0) + ' GB');
    }

    if (data.host_ram_gb !== undefined) {
      state.hostRamGb = data.host_ram_gb;
      var slider = el('wsl2-mem-slider');
      if (slider) slider.max = String(data.host_ram_gb);
    }

    if (data.pressure) {
      state.pressure = data.pressure;
      var ind = el('wsl2-pressure-indicator');
      if (ind) {
        ind.textContent = pressureLabel(data.pressure);
        ind.style.background = pressureColor(data.pressure);
      }
    }

    if (data.auto_governor !== undefined) {
      state.autoGovernor = data.auto_governor;
      var gov = el('wsl2-auto-governor');
      if (gov) {
        if (data.auto_governor) gov.classList.add('on');
        else gov.classList.remove('on');
      }
    }

    if (data.processes) {
      state.processes = data.processes;
      renderProcesses(data.processes);
    }
  }

  // ── Process Rendering ──────────────────────────────────────────
  function renderProcesses(procs) {
    var list = el('wsl2-proc-list');
    if (!list) return;
    list.innerHTML = '';

    if (!procs || procs.length === 0) {
      var empty = createEl('div', '', list);
      empty.style.cssText = 'font-size:11px;color:var(--text-dim);padding:8px 0';
      empty.textContent = 'No WSL2 processes detected';
      return;
    }

    procs.sort(function(a, b) { return (b.memory_mb || 0) - (a.memory_mb || 0); });

    procs.slice(0, 20).forEach(function(p) {
      var row = createEl('div', 'proc-row', list);
      var name = createEl('span', 'proc-name', row);
      name.textContent = p.name || 'unknown';
      name.style.flex = '2';

      var pid = createEl('span', '', row);
      pid.style.cssText = 'width:60px;text-align:right;font-size:10px;color:var(--text-dim);font-family:var(--mono)';
      pid.textContent = String(p.pid || '--');

      var mem = createEl('span', 'proc-mem', row);
      mem.textContent = (p.memory_mb || 0).toFixed(0) + ' MB';

      var cpu = createEl('span', '', row);
      cpu.style.cssText = 'width:60px;text-align:right;font-size:10px;color:var(--accent-purple);font-family:var(--mono)';
      cpu.textContent = (p.cpu_pct || 0).toFixed(1) + '%';
    });
  }

  // ── Cleanup ────────────────────────────────────────────────────
  function cleanup() {
    if (state._refreshTimer) {
      clearInterval(state._refreshTimer);
      state._refreshTimer = null;
    }
    if (state._animFrame) {
      cancelAnimationFrame(state._animFrame);
      state._animFrame = null;
    }
  }

  // ── Public API ─────────────────────────────────────────────────
  window.RuVectorPages.wsl2_Init = function(container) {
    cleanup();
    buildUI(container);
    window.addEventListener('resize', function() {
      resizeCanvas();
      drawChart();
    });
  };

  window.RuVectorPages.wsl2_Update = function(data) {
    updateUI(data);
  };

  // IPC callback
  window.updateWsl2Status = function(data) {
    updateUI(data);
  };

  window.updateWsl2Processes = function(data) {
    if (data && data.processes) {
      state.processes = data.processes;
      renderProcesses(data.processes);
    }
  };

  window.reclaimWsl2Result = function(r) {
    if (r && r.success) {
      showToastSafe('Memory Reclaimed', 'Freed ' + (r.freed_mb || 0).toFixed(0) + ' MB from WSL2', 'success');
    } else {
      showToastSafe('Reclaim Failed', (r && r.error) || 'Unknown error', 'error');
    }
    ipcSend('get_wsl2_status');
  };

})();
