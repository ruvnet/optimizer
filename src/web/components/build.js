/**
 * ADR-017: Build Environment Optimizer
 * Detects active build processes and applies memory boost actions.
 */
(function(){
  'use strict';

  window.RuVectorPages = window.RuVectorPages || {};

  // ── State ──────────────────────────────────────────────────────
  var state = {
    tools: [],
    activeBuild: null,
    threadCount: 4,
    maxThreads: 16,
    tmpfsEnabled: false,
    cpuPriority: 'normal',
    memoryLimitMb: 8192,
    ioPriority: 'normal',
    buildHistory: [],
    recommendations: [],
    _container: null,
    _chartCanvas: null,
    _refreshTimer: null
  };

  // Default tools for demo
  var defaultTools = [
    { name: 'Cargo (Rust)', icon: '\uD83E\uDD80', version: '--', cacheSize: '--', cacheBytes: 0, status: 'unknown', color: 'var(--accent-amber)' },
    { name: 'npm / Node.js', icon: '\uD83D\uDCE6', version: '--', cacheSize: '--', cacheBytes: 0, status: 'unknown', color: 'var(--accent-green)' },
    { name: 'Webpack / Vite', icon: '\u26A1', version: '--', cacheSize: '--', cacheBytes: 0, status: 'unknown', color: 'var(--accent-cyan)' },
    { name: 'Docker', icon: '\uD83D\uDC33', version: '--', cacheSize: '--', cacheBytes: 0, status: 'unknown', color: 'var(--accent-purple)' },
    { name: 'MSBuild / VS', icon: '\uD83D\uDEE0', version: '--', cacheSize: '--', cacheBytes: 0, status: 'unknown', color: 'var(--accent-red)' }
  ];

  var defaultRecommendations = [
    { text: 'Enable incremental builds for Cargo', priority: 'high', applied: false },
    { text: 'Use sccache for shared compilation cache', priority: 'high', applied: false },
    { text: 'Configure npm cache to local SSD', priority: 'medium', applied: false },
    { text: 'Enable webpack persistent caching', priority: 'medium', applied: false },
    { text: 'Set Docker buildkit for parallel layers', priority: 'low', applied: false }
  ];

  // ── IPC ────────────────────────────────────────────────────────
  function ipcSend(type, payload) {
    if (window.ipc) {
      var msg = Object.assign({ type: type }, payload || {});
      window.ipc.postMessage(JSON.stringify(msg));
    }
  }

  function el(id) { return state._container ? state._container.querySelector('#' + id) : null; }
  function setText(id, text) { var n = el(id); if (n) n.textContent = String(text); }

  function createEl(tag, cls, parent) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (parent) parent.appendChild(e);
    return e;
  }

  function getCSS(v) { return getComputedStyle(document.documentElement).getPropertyValue(v).trim(); }

  // ── Build UI ───────────────────────────────────────────────────
  function buildUI(container) {
    state._container = container;
    container.innerHTML = '';
    state.tools = defaultTools.slice();
    state.recommendations = defaultRecommendations.slice();

    var inner = createEl('div', 'page-inner', container);

    // Header
    var hdr = createEl('div', 'page-header', inner);
    createEl('span', 'page-icon', hdr).textContent = '\uD83D\uDEE0';
    createEl('h2', '', hdr).textContent = 'Build Environment Optimizer';

    var badge = createEl('div', 'page-status', inner);
    badge.textContent = 'ADR-017';

    var desc = createEl('p', 'page-desc', inner);
    desc.textContent = 'Detects active build processes (cargo, npm, webpack, MSBuild, cmake) and applies memory boost actions: priority elevation, working set expansion, and optional RAM disk for temp/output.';

    var grid = createEl('div', 'page-grid', inner);

    // ── Card 1: Current Build Detection ──────────────────────────
    var c1 = createEl('div', 'ph-card', grid);
    createEl('div', 'ph-card-title', c1).textContent = 'Current Build Status';

    var buildStatus = createEl('div', '', c1);
    buildStatus.id = 'build-status-box';
    buildStatus.style.cssText = 'display:flex;align-items:center;gap:12px;padding:12px;border-radius:8px;background:var(--bg-primary);margin-bottom:12px';

    var bsDot = createEl('div', '', buildStatus);
    bsDot.id = 'build-active-dot';
    bsDot.style.cssText = 'width:14px;height:14px;border-radius:50%;background:var(--text-dim);flex-shrink:0';

    var bsInfo = createEl('div', '', buildStatus);
    var bsTitle = createEl('div', '', bsInfo);
    bsTitle.id = 'build-active-title';
    bsTitle.style.cssText = 'font-size:13px;font-weight:600;color:var(--text-primary)';
    bsTitle.textContent = 'No Active Build';
    var bsSub = createEl('div', '', bsInfo);
    bsSub.id = 'build-active-sub';
    bsSub.style.cssText = 'font-size:10px;color:var(--text-dim);margin-top:2px';
    bsSub.textContent = 'Monitoring for build processes...';

    // Progress bar (hidden by default)
    var progWrap = createEl('div', '', c1);
    progWrap.id = 'build-progress-wrap';
    progWrap.style.cssText = 'display:none;margin-bottom:8px';
    var progLabel = createEl('div', '', progWrap);
    progLabel.style.cssText = 'display:flex;justify-content:space-between;font-size:10px;color:var(--text-secondary);margin-bottom:3px';
    createEl('span', '', progLabel).textContent = 'Progress';
    var progPct = createEl('span', '', progLabel);
    progPct.id = 'build-progress-pct';
    progPct.style.fontFamily = 'var(--mono)';
    progPct.textContent = '0%';
    var progTrack = createEl('div', '', progWrap);
    progTrack.style.cssText = 'height:6px;background:var(--gauge-track);border-radius:3px;overflow:hidden';
    var progBar = createEl('div', '', progTrack);
    progBar.id = 'build-progress-bar';
    progBar.style.cssText = 'height:100%;width:0%;background:var(--accent-cyan);border-radius:3px;transition:width 0.4s ease';

    buildField(c1, 'Build Tool', 'build-tool-name', '--');
    buildField(c1, 'Est. Memory', 'build-est-mem', '-- MB');
    buildField(c1, 'Duration', 'build-duration', '--');

    // ── Card 2: Build Tools Table ────────────────────────────────
    var c2 = createEl('div', 'ph-card', grid);
    createEl('div', 'ph-card-title', c2).textContent = 'Detected Build Tools';

    var toolsTable = createEl('div', '', c2);
    toolsTable.id = 'build-tools-table';
    renderToolsTable(toolsTable, state.tools);

    // ── Card 3: Resource Allocation ──────────────────────────────
    var c3 = createEl('div', 'ph-card', grid);
    createEl('div', 'ph-card-title', c3).textContent = 'Resource Allocation';

    // Thread slider
    buildSlider(c3, 'Parallel Threads', 'build-threads', 1, state.maxThreads, state.threadCount, '', function(v) {
      state.threadCount = v;
    });

    // Memory limit slider
    buildSlider(c3, 'Memory Limit', 'build-mem-limit', 512, 32768, state.memoryLimitMb, 'MB', function(v) {
      state.memoryLimitMb = v;
    });

    // CPU Priority
    var cpuRow = createEl('div', 'select-row', c3);
    cpuRow.style.cssText = 'padding:8px 0;border-top:1px solid var(--border)';
    var cpuLabel = createEl('span', '', cpuRow);
    cpuLabel.style.cssText = 'font-size:11px;color:var(--text-secondary)';
    cpuLabel.textContent = 'CPU Priority';
    var cpuSelect = document.createElement('select');
    cpuSelect.style.cssText = 'background:var(--bg-primary);color:var(--text-primary);border:1px solid var(--border);border-radius:4px;padding:2px 6px;font-size:11px;font-family:var(--font);cursor:pointer';
    ['low', 'normal', 'high', 'realtime'].forEach(function(p) {
      var opt = document.createElement('option');
      opt.value = p;
      opt.textContent = p.charAt(0).toUpperCase() + p.slice(1);
      if (p === state.cpuPriority) opt.selected = true;
      cpuSelect.appendChild(opt);
    });
    cpuRow.appendChild(cpuSelect);
    cpuSelect.addEventListener('change', function() { state.cpuPriority = cpuSelect.value; });

    // I/O Priority
    var ioRow = createEl('div', 'select-row', c3);
    ioRow.style.cssText = 'padding:8px 0;border-top:1px solid var(--border)';
    var ioLabel = createEl('span', '', ioRow);
    ioLabel.style.cssText = 'font-size:11px;color:var(--text-secondary)';
    ioLabel.textContent = 'I/O Priority';
    var ioSelect = document.createElement('select');
    ioSelect.style.cssText = 'background:var(--bg-primary);color:var(--text-primary);border:1px solid var(--border);border-radius:4px;padding:2px 6px;font-size:11px;font-family:var(--font);cursor:pointer';
    ['low', 'normal', 'high'].forEach(function(p) {
      var opt = document.createElement('option');
      opt.value = p;
      opt.textContent = p.charAt(0).toUpperCase() + p.slice(1);
      if (p === state.ioPriority) opt.selected = true;
      ioSelect.appendChild(opt);
    });
    ioRow.appendChild(ioSelect);
    ioSelect.addEventListener('change', function() { state.ioPriority = ioSelect.value; });

    // tmpfs toggle
    var tmpRow = createEl('div', 'toggle-row', c3);
    tmpRow.style.borderTop = '1px solid var(--border)';
    tmpRow.style.paddingTop = '8px';
    createEl('span', '', tmpRow).textContent = 'tmpfs / RAM Disk for Output';
    var tmpToggle = createEl('div', 'toggle', tmpRow);
    tmpToggle.id = 'build-tmpfs-toggle';
    tmpToggle.addEventListener('click', function() {
      tmpToggle.classList.toggle('on');
      state.tmpfsEnabled = tmpToggle.classList.contains('on');
    });

    // Apply button
    var applyWrap = createEl('div', 'ph-actions', c3);
    var applyBtn = createEl('button', 'btn', applyWrap);
    applyBtn.style.cssText = 'cursor:pointer;opacity:1';
    applyBtn.innerHTML = '<span class="ico">&#9989;</span> Apply Settings';
    applyBtn.addEventListener('click', function() {
      ipcSend('set_build_config', {
        threads: state.threadCount,
        memory_limit_mb: state.memoryLimitMb,
        cpu_priority: state.cpuPriority,
        io_priority: state.ioPriority,
        tmpfs_enabled: state.tmpfsEnabled
      });
      if (typeof window.showToast === 'function') {
        window.showToast('Build Config', 'Settings applied to active builds', 'success');
      }
    });

    // ── Card 4: Recommendations ──────────────────────────────────
    var c4 = createEl('div', 'ph-card', grid);
    createEl('div', 'ph-card-title', c4).textContent = 'Optimization Recommendations';

    var recList = createEl('div', '', c4);
    recList.id = 'build-recommendations';
    renderRecommendations(recList);

    // Optimize button
    var optWrap = createEl('div', 'ph-actions', c4);
    var optBtn = createEl('button', 'btn', optWrap);
    optBtn.style.cssText = 'cursor:pointer;opacity:1';
    optBtn.innerHTML = '<span class="ico">&#128640;</span> Auto-Optimize All';
    optBtn.addEventListener('click', function() {
      ipcSend('optimize_builds');
      state.recommendations.forEach(function(r) { r.applied = true; });
      renderRecommendations(el('build-recommendations'));
    });

    // ── Card 5: Build History Chart ──────────────────────────────
    var c5 = createEl('div', 'ph-card', grid);
    c5.style.gridColumn = '1 / -1';
    createEl('div', 'ph-card-title', c5).textContent = 'Build Time History (Last 7 Days)';

    var chartWrap = createEl('div', '', c5);
    chartWrap.style.cssText = 'height:180px;position:relative;background:var(--bg-primary);border-radius:8px;overflow:hidden';
    var canvas = document.createElement('canvas');
    canvas.id = 'build-history-chart';
    canvas.style.cssText = 'width:100%;height:100%;display:block';
    chartWrap.appendChild(canvas);
    state._chartCanvas = canvas;

    // Seed demo data
    seedHistory();

    // Request data
    ipcSend('get_build_tools');
    ipcSend('get_build_history');

    state._refreshTimer = setInterval(function() {
      ipcSend('get_build_tools');
    }, 10000);

    setTimeout(function() { resizeCanvas(); drawChart(); }, 100);
  }

  // ── Tools Table ────────────────────────────────────────────────
  function renderToolsTable(container, tools) {
    container.innerHTML = '';

    tools.forEach(function(tool) {
      var row = createEl('div', '', container);
      row.style.cssText = 'display:flex;align-items:center;gap:10px;padding:8px 0;border-bottom:1px solid var(--border);font-size:11px';

      var iconEl = createEl('span', '', row);
      iconEl.style.cssText = 'font-size:16px;width:24px;text-align:center;flex-shrink:0';
      iconEl.textContent = tool.icon || '';

      var nameEl = createEl('span', '', row);
      nameEl.style.cssText = 'flex:1;color:var(--text-primary);font-weight:500';
      nameEl.textContent = tool.name;

      var verEl = createEl('span', '', row);
      verEl.style.cssText = 'width:60px;font-family:var(--mono);font-size:10px;color:var(--text-secondary)';
      verEl.textContent = tool.version;

      var cacheEl = createEl('span', '', row);
      cacheEl.style.cssText = 'width:70px;text-align:right;font-family:var(--mono);font-size:10px;color:var(--accent-amber)';
      cacheEl.textContent = tool.cacheSize;

      var statusEl = createEl('span', '', row);
      statusEl.style.cssText = 'width:18px;height:18px;border-radius:50%;flex-shrink:0;display:flex;align-items:center;justify-content:center;font-size:10px';
      if (tool.status === 'optimized') {
        statusEl.style.background = 'rgba(107,203,119,0.15)';
        statusEl.style.color = 'var(--accent-green)';
        statusEl.textContent = '\u2713';
      } else if (tool.status === 'detected') {
        statusEl.style.background = 'rgba(78,205,196,0.15)';
        statusEl.style.color = 'var(--accent-cyan)';
        statusEl.textContent = '\u25CF';
      } else {
        statusEl.style.background = 'var(--bg-primary)';
        statusEl.style.color = 'var(--text-dim)';
        statusEl.textContent = '\u2013';
      }

      // Clear cache button
      var clearBtn = createEl('button', '', row);
      clearBtn.style.cssText = 'background:var(--bg-primary);border:1px solid var(--border);border-radius:4px;padding:2px 8px;font-size:9px;color:var(--text-secondary);cursor:pointer;font-family:var(--font)';
      clearBtn.textContent = 'Clear';
      clearBtn.addEventListener('click', (function(t) {
        return function() {
          ipcSend('clear_cache', { tool: t.name });
          if (typeof window.showToast === 'function') {
            window.showToast('Cache Cleared', t.name + ' cache purged', 'success');
          }
        };
      })(tool));
    });
  }

  // ── Recommendations ────────────────────────────────────────────
  function renderRecommendations(container) {
    if (!container) return;
    container.innerHTML = '';

    state.recommendations.forEach(function(rec, idx) {
      var row = createEl('div', '', container);
      row.style.cssText = 'display:flex;align-items:center;gap:8px;padding:6px 0;border-bottom:1px solid var(--border);font-size:11px';

      var priDot = createEl('span', '', row);
      priDot.style.cssText = 'width:6px;height:6px;border-radius:50%;flex-shrink:0';
      priDot.style.background = rec.priority === 'high' ? 'var(--accent-red)' : rec.priority === 'medium' ? 'var(--accent-amber)' : 'var(--accent-cyan)';

      var text = createEl('span', '', row);
      text.style.cssText = 'flex:1;color:' + (rec.applied ? 'var(--text-dim)' : 'var(--text-secondary)');
      if (rec.applied) text.style.textDecoration = 'line-through';
      text.textContent = rec.text;

      var statusIcon = createEl('span', '', row);
      statusIcon.style.cssText = 'font-size:12px';
      statusIcon.textContent = rec.applied ? '\u2705' : '';
    });
  }

  // ── Field / Slider Helpers ─────────────────────────────────────
  function buildField(parent, label, id, defaultVal) {
    var row = createEl('div', 'ph-field', parent);
    createEl('span', 'ph-field-label', row).textContent = label;
    var val = createEl('span', 'ph-field-value', row);
    val.id = id;
    val.textContent = defaultVal;
  }

  function buildSlider(parent, label, id, min, max, initial, unit, onChange) {
    var row = createEl('div', '', parent);
    row.style.cssText = 'margin:8px 0';
    var top = createEl('div', '', row);
    top.style.cssText = 'display:flex;justify-content:space-between;font-size:10px;color:var(--text-secondary);margin-bottom:4px';
    createEl('span', '', top).textContent = label;
    var valEl = createEl('span', '', top);
    valEl.id = id + '-val';
    valEl.style.cssText = 'font-family:var(--mono);color:var(--text-primary);font-weight:500';
    valEl.textContent = initial + (unit ? ' ' + unit : '');

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
      valEl.textContent = v + (unit ? ' ' + unit : '');
      if (onChange) onChange(v);
    });
  }

  // ── History ────────────────────────────────────────────────────
  function seedHistory() {
    state.buildHistory = [];
    var tools = ['cargo', 'npm', 'webpack', 'docker'];
    var days = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
    for (var d = 0; d < 7; d++) {
      for (var t = 0; t < tools.length; t++) {
        if (Math.random() > 0.4) {
          state.buildHistory.push({
            day: days[d],
            dayIndex: d,
            tool: tools[t],
            duration_s: 30 + Math.random() * 300,
            toolIndex: t
          });
        }
      }
    }
  }

  // ── Chart Drawing ──────────────────────────────────────────────
  function resizeCanvas() {
    var c = state._chartCanvas;
    if (!c) return;
    var rect = c.parentElement.getBoundingClientRect();
    c.width = rect.width * (window.devicePixelRatio || 1);
    c.height = rect.height * (window.devicePixelRatio || 1);
  }

  function drawChart() {
    var canvas = state._chartCanvas;
    if (!canvas || !canvas.getContext) return;
    var ctx = canvas.getContext('2d');
    var w = canvas.width;
    var h = canvas.height;
    var dpr = window.devicePixelRatio || 1;
    var data = state.buildHistory;

    ctx.clearRect(0, 0, w, h);

    var pad = { top: 20 * dpr, right: 20 * dpr, bottom: 30 * dpr, left: 50 * dpr };
    var cw = w - pad.left - pad.right;
    var ch = h - pad.top - pad.bottom;

    var days = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
    var toolColors = {
      'cargo': getCSS('--accent-amber'),
      'npm': getCSS('--accent-green'),
      'webpack': getCSS('--accent-cyan'),
      'docker': getCSS('--accent-purple')
    };

    var maxDuration = 0;
    data.forEach(function(d) { if (d.duration_s > maxDuration) maxDuration = d.duration_s; });
    maxDuration = Math.max(maxDuration, 60);

    // Grid
    ctx.strokeStyle = getCSS('--border');
    ctx.lineWidth = 0.5 * dpr;
    for (var g = 0; g <= 4; g++) {
      var gy = pad.top + ch * (1 - g / 4);
      ctx.beginPath();
      ctx.moveTo(pad.left, gy);
      ctx.lineTo(pad.left + cw, gy);
      ctx.stroke();

      ctx.fillStyle = getCSS('--text-dim');
      ctx.font = (9 * dpr) + 'px ' + getCSS('--mono');
      ctx.textAlign = 'right';
      ctx.fillText(Math.round(maxDuration * g / 4) + 's', pad.left - 6 * dpr, gy + 3 * dpr);
    }

    // Day labels
    ctx.fillStyle = getCSS('--text-dim');
    ctx.font = (9 * dpr) + 'px ' + getCSS('--font');
    ctx.textAlign = 'center';
    for (var di = 0; di < 7; di++) {
      var dx = pad.left + (di + 0.5) / 7 * cw;
      ctx.fillText(days[di], dx, pad.top + ch + 18 * dpr);
    }

    // Bars (grouped by day)
    var barGroupWidth = cw / 7;
    var barWidth = barGroupWidth / 5;

    data.forEach(function(d) {
      var groupX = pad.left + d.dayIndex * barGroupWidth;
      var barX = groupX + (d.toolIndex + 0.5) * barWidth;
      var barH = (d.duration_s / maxDuration) * ch;
      var barY = pad.top + ch - barH;

      ctx.fillStyle = toolColors[d.tool] || getCSS('--accent-cyan');
      ctx.globalAlpha = 0.8;
      ctx.beginPath();
      roundRect(ctx, barX, barY, barWidth * 0.8, barH, 2 * dpr);
      ctx.fill();
      ctx.globalAlpha = 1;
    });

    // Legend
    var legendX = pad.left;
    var legendY = pad.top - 6 * dpr;
    ctx.font = (8 * dpr) + 'px ' + getCSS('--font');
    ctx.textAlign = 'left';
    var legendTools = ['cargo', 'npm', 'webpack', 'docker'];
    legendTools.forEach(function(t, i) {
      var lx = legendX + i * 80 * dpr;
      ctx.fillStyle = toolColors[t];
      ctx.fillRect(lx, legendY - 6 * dpr, 8 * dpr, 8 * dpr);
      ctx.fillStyle = getCSS('--text-dim');
      ctx.fillText(t, lx + 12 * dpr, legendY);
    });
  }

  function roundRect(ctx, x, y, w, h, r) {
    ctx.moveTo(x + r, y);
    ctx.lineTo(x + w - r, y);
    ctx.quadraticCurveTo(x + w, y, x + w, y + r);
    ctx.lineTo(x + w, y + h);
    ctx.lineTo(x, y + h);
    ctx.lineTo(x, y + r);
    ctx.quadraticCurveTo(x, y, x + r, y);
  }

  // ── Update ─────────────────────────────────────────────────────
  function updateUI(data) {
    if (!data || !state._container) return;

    if (data.tools) {
      state.tools = data.tools;
      var tbl = el('build-tools-table');
      if (tbl) renderToolsTable(tbl, data.tools);
    }

    if (data.active_build) {
      state.activeBuild = data.active_build;
      var dot = el('build-active-dot');
      var title = el('build-active-title');
      var sub = el('build-active-sub');
      if (dot) dot.style.background = 'var(--accent-green)';
      if (dot) dot.style.animation = 'labelPulse 1.5s ease-in-out infinite';
      if (title) title.textContent = data.active_build.tool + ' Build Running';
      if (sub) sub.textContent = data.active_build.target || 'Building...';
      setText('build-tool-name', data.active_build.tool);
      setText('build-est-mem', (data.active_build.memory_mb || 0) + ' MB');
      setText('build-duration', (data.active_build.duration_s || 0) + 's');

      if (data.active_build.progress !== undefined) {
        var pw = el('build-progress-wrap');
        if (pw) pw.style.display = 'block';
        setText('build-progress-pct', Math.round(data.active_build.progress) + '%');
        var pb = el('build-progress-bar');
        if (pb) pb.style.width = data.active_build.progress + '%';
      }
    } else if (data.active_build === null) {
      state.activeBuild = null;
      var d2 = el('build-active-dot');
      if (d2) { d2.style.background = 'var(--text-dim)'; d2.style.animation = 'none'; }
      setText('build-active-title', 'No Active Build');
      setText('build-active-sub', 'Monitoring for build processes...');
      var pw2 = el('build-progress-wrap');
      if (pw2) pw2.style.display = 'none';
    }

    if (data.max_threads) {
      state.maxThreads = data.max_threads;
      var s = el('build-threads');
      if (s) s.max = String(data.max_threads);
    }

    if (data.history) {
      state.buildHistory = data.history;
      drawChart();
    }

    if (data.recommendations) {
      state.recommendations = data.recommendations;
      renderRecommendations(el('build-recommendations'));
    }
  }

  // ── Cleanup ────────────────────────────────────────────────────
  function cleanup() {
    if (state._refreshTimer) { clearInterval(state._refreshTimer); state._refreshTimer = null; }
  }

  // ── Public API ─────────────────────────────────────────────────
  window.RuVectorPages.build_Init = function(container) {
    cleanup();
    buildUI(container);
    window.addEventListener('resize', function() { resizeCanvas(); drawChart(); });
  };

  window.RuVectorPages.build_Update = function(data) { updateUI(data); };

  window.updateBuildTools = function(data) { updateUI(data); };
  window.updateBuildHistory = function(data) { if (data && data.history) { state.buildHistory = data.history; drawChart(); } };
  window.clearCacheResult = function(r) {
    if (r && r.success && typeof window.showToast === 'function') {
      window.showToast('Cache Cleared', 'Freed ' + (r.freed_mb || 0).toFixed(0) + ' MB', 'success');
    }
    ipcSend('get_build_tools');
  };

})();
