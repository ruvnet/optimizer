/**
 * ADR-018: Spectral Leak Detector
 * Time-series analysis with spectral decomposition to detect slow memory leaks.
 */
(function(){
  'use strict';

  window.RuVectorPages = window.RuVectorPages || {};

  // ── State ──────────────────────────────────────────────────────
  var state = {
    suspects: [],
    history: [],
    totalLeakRate: 0,
    monitoredCount: 0,
    alertThreshold: 85,
    notifyEmail: false,
    notifyToast: true,
    spectralData: [],
    _container: null,
    _fftCanvas: null,
    _refreshTimer: null
  };

  // Demo leak suspects
  var demoSuspects = [
    { name: 'chrome.exe', pid: 14232, growthRate: 42.3, currentMb: 2841, durationMin: 187, confidence: 0.94, periodicity: 12.5, history: [] },
    { name: 'node.exe', pid: 8920, growthRate: 18.7, currentMb: 1245, durationMin: 312, confidence: 0.87, periodicity: 8.2, history: [] },
    { name: 'electron.exe', pid: 5501, growthRate: 8.4, currentMb: 890, durationMin: 95, confidence: 0.72, periodicity: 0, history: [] },
    { name: 'java.exe', pid: 11040, growthRate: 5.1, currentMb: 567, durationMin: 420, confidence: 0.65, periodicity: 24.0, history: [] },
    { name: 'python.exe', pid: 3310, growthRate: 2.3, currentMb: 312, durationMin: 55, confidence: 0.41, periodicity: 0, history: [] }
  ];

  // Seed sparkline data for each suspect
  demoSuspects.forEach(function(s) {
    s.history = [];
    var base = s.currentMb - s.growthRate * 2;
    for (var i = 0; i < 40; i++) {
      s.history.push(base + (s.growthRate / 40) * i + (Math.random() - 0.3) * s.growthRate * 0.1);
    }
  });

  // ── Helpers ────────────────────────────────────────────────────
  function ipcSend(type, payload) {
    if (window.ipc) {
      window.ipc.postMessage(JSON.stringify(Object.assign({ type: type }, payload || {})));
    }
  }

  function el(id) { return state._container ? state._container.querySelector('#' + id) : null; }
  function setText(id, t) { var n = el(id); if (n) n.textContent = String(t); }

  function createEl(tag, cls, parent) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (parent) parent.appendChild(e);
    return e;
  }

  function getCSS(v) { return getComputedStyle(document.documentElement).getPropertyValue(v).trim(); }

  function confidenceColor(c) {
    if (c >= 0.9) return 'var(--accent-red)';
    if (c >= 0.7) return 'var(--accent-amber)';
    return 'var(--text-dim)';
  }

  function formatDuration(min) {
    if (min < 60) return min + 'm';
    var h = Math.floor(min / 60);
    var m = min % 60;
    return h + 'h ' + m + 'm';
  }

  // ── Build UI ───────────────────────────────────────────────────
  function buildUI(container) {
    state._container = container;
    container.innerHTML = '';
    state.suspects = demoSuspects.slice();

    var inner = createEl('div', 'page-inner', container);

    // Header
    var hdr = createEl('div', 'page-header', inner);
    createEl('span', 'page-icon', hdr).textContent = '\uD83D\uDD0D';
    createEl('h2', '', hdr).textContent = 'Spectral Leak Detector';

    createEl('div', 'page-status', inner).textContent = 'ADR-018';

    var desc = createEl('p', 'page-desc', inner);
    desc.textContent = 'Time-series analysis with spectral decomposition to detect slow memory leaks. Uses sliding windows, trend extraction, and confidence scoring to identify processes with monotonic growth patterns before they become critical.';

    var grid = createEl('div', 'page-grid', inner);

    // ── Card 1: Summary ──────────────────────────────────────────
    var c1 = createEl('div', 'ph-card', grid);
    createEl('div', 'ph-card-title', c1).textContent = 'Detection Summary';

    var summaryGrid = createEl('div', '', c1);
    summaryGrid.style.cssText = 'display:grid;grid-template-columns:1fr 1fr 1fr;gap:12px;margin-bottom:12px';

    buildSummaryMetric(summaryGrid, 'leaks-suspect-count', '0', 'Leak Suspects');
    buildSummaryMetric(summaryGrid, 'leaks-total-rate', '0', 'MB/hr Total Leak');
    buildSummaryMetric(summaryGrid, 'leaks-time-pressure', '--', 'Est. Time to Pressure');

    updateSummary();

    // ── Card 2: Alert Config ─────────────────────────────────────
    var c2 = createEl('div', 'ph-card', grid);
    createEl('div', 'ph-card-title', c2).textContent = 'Alert Configuration';

    // Threshold slider
    buildSlider(c2, 'Confidence Threshold', 'leaks-threshold', 30, 99, state.alertThreshold, '%', function(v) {
      state.alertThreshold = v;
    });

    // Notification toggles
    var notifyToast = createEl('div', 'toggle-row', c2);
    notifyToast.style.borderTop = '1px solid var(--border)';
    notifyToast.style.paddingTop = '8px';
    createEl('span', '', notifyToast).textContent = 'Toast Notifications';
    var tt = createEl('div', 'toggle on', notifyToast);
    tt.addEventListener('click', function() {
      tt.classList.toggle('on');
      state.notifyToast = tt.classList.contains('on');
    });

    var notifyEmail = createEl('div', 'toggle-row', c2);
    createEl('span', '', notifyEmail).textContent = 'Email Alerts';
    var et = createEl('div', 'toggle', notifyEmail);
    et.addEventListener('click', function() {
      et.classList.toggle('on');
      state.notifyEmail = et.classList.contains('on');
    });

    // Apply
    var applyWrap = createEl('div', 'ph-actions', c2);
    var applyBtn = createEl('button', 'btn', applyWrap);
    applyBtn.style.cssText = 'cursor:pointer;opacity:1';
    applyBtn.innerHTML = '<span class="ico">&#128190;</span> Save Config';
    applyBtn.addEventListener('click', function() {
      ipcSend('set_leak_config', {
        threshold: state.alertThreshold / 100,
        notify_toast: state.notifyToast,
        notify_email: state.notifyEmail
      });
      if (typeof window.showToast === 'function') {
        window.showToast('Leak Config', 'Alert settings saved', 'success');
      }
    });

    // ── Card 3: Suspects Table (full width) ──────────────────────
    var c3 = createEl('div', 'ph-card', grid);
    c3.style.gridColumn = '1 / -1';
    createEl('div', 'ph-card-title', c3).textContent = 'Leak Suspects';

    // Table header
    var tHead = createEl('div', '', c3);
    tHead.style.cssText = 'display:flex;align-items:center;gap:6px;padding:6px 0;border-bottom:1px solid var(--border);font-size:9px;font-weight:600;color:var(--text-dim);letter-spacing:0.5px;text-transform:uppercase';
    addCol(tHead, 'Process', 'flex:1.5');
    addCol(tHead, 'Growth', 'width:70px;text-align:right');
    addCol(tHead, 'Current', 'width:70px;text-align:right');
    addCol(tHead, 'Duration', 'width:65px;text-align:right');
    addCol(tHead, 'Confidence', 'width:75px;text-align:right');
    addCol(tHead, 'FFT Period', 'width:65px;text-align:right');
    addCol(tHead, 'Trend', 'width:100px');
    addCol(tHead, 'Monitor', 'width:50px;text-align:center');
    addCol(tHead, '', 'width:50px');

    var tBody = createEl('div', '', c3);
    tBody.id = 'leaks-suspects-body';
    renderSuspects(tBody);

    // ── Card 4: FFT Spectral Analysis (full width) ───────────────
    var c4 = createEl('div', 'ph-card', grid);
    c4.style.gridColumn = '1 / -1';
    createEl('div', 'ph-card-title', c4).textContent = 'Spectral Analysis (FFT Frequency Domain)';

    var fftWrap = createEl('div', '', c4);
    fftWrap.style.cssText = 'height:200px;position:relative;background:var(--bg-primary);border-radius:8px;overflow:hidden';
    var fftCanvas = document.createElement('canvas');
    fftCanvas.id = 'leaks-fft-chart';
    fftCanvas.style.cssText = 'width:100%;height:100%;display:block';
    fftWrap.appendChild(fftCanvas);
    state._fftCanvas = fftCanvas;

    seedSpectralData();

    // ── Card 5: Historical Leaks ─────────────────────────────────
    var c5 = createEl('div', 'ph-card', grid);
    c5.style.gridColumn = '1 / -1';
    createEl('div', 'ph-card-title', c5).textContent = 'Detection History';

    var histList = createEl('div', '', c5);
    histList.id = 'leaks-history-list';
    renderHistory(histList);

    // Actions
    var actions = createEl('div', 'ph-actions', inner);
    var scanBtn = createEl('button', 'btn', actions);
    scanBtn.style.cssText = 'cursor:pointer;opacity:1';
    scanBtn.innerHTML = '<span class="ico">&#128270;</span> Start Deep Scan';
    scanBtn.addEventListener('click', function() {
      scanBtn.disabled = true;
      scanBtn.textContent = 'Scanning...';
      ipcSend('get_leak_suspects');
      ipcSend('get_spectral_data');
      setTimeout(function() {
        scanBtn.disabled = false;
        scanBtn.innerHTML = '<span class="ico">&#128270;</span> Start Deep Scan';
      }, 3000);
    });

    // Refresh
    ipcSend('get_leak_suspects');
    ipcSend('get_leak_history');
    ipcSend('get_spectral_data');

    state._refreshTimer = setInterval(function() {
      ipcSend('get_leak_suspects');
      animateSuspects();
    }, 8000);

    setTimeout(function() { resizeFftCanvas(); drawFFT(); }, 100);
  }

  // ── Summary Metric Builder ─────────────────────────────────────
  function buildSummaryMetric(parent, id, value, label) {
    var box = createEl('div', '', parent);
    box.style.cssText = 'text-align:center;padding:8px;background:var(--bg-primary);border-radius:8px';
    var val = createEl('div', '', box);
    val.id = id;
    val.style.cssText = 'font-size:24px;font-weight:700;color:var(--accent-cyan);line-height:1';
    val.textContent = value;
    var lbl = createEl('div', '', box);
    lbl.style.cssText = 'font-size:9px;color:var(--text-dim);text-transform:uppercase;letter-spacing:0.8px;margin-top:4px';
    lbl.textContent = label;
  }

  function updateSummary() {
    var suspects = state.suspects.filter(function(s) { return s.confidence >= state.alertThreshold / 100; });
    var totalRate = 0;
    suspects.forEach(function(s) { totalRate += s.growthRate; });
    state.totalLeakRate = totalRate;

    setText('leaks-suspect-count', String(suspects.length));
    var rateEl = el('leaks-total-rate');
    if (rateEl) {
      rateEl.textContent = totalRate.toFixed(1);
      rateEl.style.color = totalRate > 50 ? 'var(--accent-red)' : totalRate > 20 ? 'var(--accent-amber)' : 'var(--accent-cyan)';
    }

    // Estimate time to pressure (assuming 16GB threshold at 85%)
    var freeEstMb = 4000; // rough estimate
    if (totalRate > 0) {
      var hoursToFull = freeEstMb / totalRate;
      setText('leaks-time-pressure', hoursToFull < 1 ? '<1h' : hoursToFull.toFixed(0) + 'h');
    } else {
      setText('leaks-time-pressure', '\u221E');
    }
  }

  // ── Suspects Table ─────────────────────────────────────────────
  function renderSuspects(container) {
    if (!container) return;
    container.innerHTML = '';

    if (!state.suspects || state.suspects.length === 0) {
      var empty = createEl('div', '', container);
      empty.style.cssText = 'font-size:11px;color:var(--text-dim);padding:12px 0';
      empty.textContent = 'No leak suspects detected. System looks healthy.';
      return;
    }

    state.suspects.forEach(function(s, idx) {
      var row = createEl('div', '', container);
      row.style.cssText = 'display:flex;align-items:center;gap:6px;padding:8px 0;border-bottom:1px solid var(--border);font-size:11px';

      // Process name
      var nameEl = createEl('span', '', row);
      nameEl.style.cssText = 'flex:1.5;color:var(--text-primary);font-weight:500';
      nameEl.textContent = s.name + ' (' + s.pid + ')';

      // Growth rate
      var growthEl = createEl('span', '', row);
      growthEl.style.cssText = 'width:70px;text-align:right;font-family:var(--mono);font-size:10px;color:var(--accent-red)';
      growthEl.textContent = s.growthRate.toFixed(1) + ' MB/h';

      // Current usage
      var curEl = createEl('span', '', row);
      curEl.style.cssText = 'width:70px;text-align:right;font-family:var(--mono);font-size:10px;color:var(--accent-amber)';
      curEl.textContent = s.currentMb.toFixed(0) + ' MB';

      // Duration
      var durEl = createEl('span', '', row);
      durEl.style.cssText = 'width:65px;text-align:right;font-family:var(--mono);font-size:10px;color:var(--text-secondary)';
      durEl.textContent = formatDuration(s.durationMin);

      // Confidence
      var confEl = createEl('span', '', row);
      confEl.style.cssText = 'width:75px;text-align:right;font-family:var(--mono);font-size:10px;font-weight:600';
      confEl.style.color = confidenceColor(s.confidence);
      confEl.textContent = (s.confidence * 100).toFixed(0) + '%';

      // FFT Periodicity
      var fftEl = createEl('span', '', row);
      fftEl.style.cssText = 'width:65px;text-align:right;font-family:var(--mono);font-size:10px;color:var(--text-dim)';
      fftEl.textContent = s.periodicity > 0 ? s.periodicity.toFixed(1) + 'h' : '--';

      // Sparkline
      var sparkWrap = createEl('span', '', row);
      sparkWrap.style.cssText = 'width:100px;height:24px;flex-shrink:0';
      var sparkCanvas = document.createElement('canvas');
      sparkCanvas.width = 100 * (window.devicePixelRatio || 1);
      sparkCanvas.height = 24 * (window.devicePixelRatio || 1);
      sparkCanvas.style.cssText = 'width:100px;height:24px;display:block';
      sparkWrap.appendChild(sparkCanvas);
      drawSparkline(sparkCanvas, s.history, s.confidence);

      // Monitor toggle
      var monWrap = createEl('span', '', row);
      monWrap.style.cssText = 'width:50px;text-align:center';
      var monToggle = createEl('div', 'toggle on', monWrap);
      monToggle.style.cssText += ';display:inline-block;transform:scale(0.8)';
      monToggle.addEventListener('click', function() { monToggle.classList.toggle('on'); });

      // Dismiss button
      var dismissWrap = createEl('span', '', row);
      dismissWrap.style.cssText = 'width:50px;text-align:center';
      var dismissBtn = createEl('button', '', dismissWrap);
      dismissBtn.style.cssText = 'background:none;border:1px solid var(--border);border-radius:4px;padding:2px 6px;font-size:9px;color:var(--text-dim);cursor:pointer;font-family:var(--font)';
      dismissBtn.textContent = 'Dismiss';
      dismissBtn.addEventListener('click', (function(suspect, rowEl) {
        return function() {
          ipcSend('dismiss_leak', { pid: suspect.pid });
          rowEl.style.opacity = '0.3';
          rowEl.style.transition = 'opacity 0.3s ease';
        };
      })(s, row));
    });

    updateSummary();
  }

  // ── Sparkline Drawing ──────────────────────────────────────────
  function drawSparkline(canvas, data, confidence) {
    if (!canvas || !data || data.length < 2) return;
    var ctx = canvas.getContext('2d');
    var w = canvas.width;
    var h = canvas.height;
    var dpr = window.devicePixelRatio || 1;

    ctx.clearRect(0, 0, w, h);

    var min = Infinity, max = -Infinity;
    data.forEach(function(v) { if (v < min) min = v; if (v > max) max = v; });
    var range = max - min || 1;

    // Fill gradient
    ctx.beginPath();
    for (var i = 0; i < data.length; i++) {
      var x = (i / (data.length - 1)) * w;
      var y = h - ((data[i] - min) / range) * (h * 0.8) - h * 0.1;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.lineTo(w, h);
    ctx.lineTo(0, h);
    ctx.closePath();

    var grad = ctx.createLinearGradient(0, 0, 0, h);
    var color = confidence >= 0.9 ? [224, 96, 96] : confidence >= 0.7 ? [212, 165, 116] : [128, 144, 168];
    grad.addColorStop(0, 'rgba(' + color.join(',') + ',0.3)');
    grad.addColorStop(1, 'rgba(' + color.join(',') + ',0.02)');
    ctx.fillStyle = grad;
    ctx.fill();

    // Line
    ctx.beginPath();
    for (var j = 0; j < data.length; j++) {
      var lx = (j / (data.length - 1)) * w;
      var ly = h - ((data[j] - min) / range) * (h * 0.8) - h * 0.1;
      if (j === 0) ctx.moveTo(lx, ly);
      else ctx.lineTo(lx, ly);
    }
    ctx.strokeStyle = 'rgb(' + color.join(',') + ')';
    ctx.lineWidth = 1.5 * dpr;
    ctx.stroke();
  }

  // ── FFT Spectral Chart ─────────────────────────────────────────
  function seedSpectralData() {
    state.spectralData = [];
    for (var i = 0; i < 64; i++) {
      var freq = i * 0.5;
      var magnitude = 0;
      // Create peaks at certain frequencies
      magnitude += 15 * Math.exp(-Math.pow(freq - 0.08, 2) / 0.001); // DC-like component
      magnitude += 8 * Math.exp(-Math.pow(freq - 2.4, 2) / 0.05);   // ~2.4h period
      magnitude += 12 * Math.exp(-Math.pow(freq - 8.3, 2) / 0.08);  // ~8h period
      magnitude += 5 * Math.exp(-Math.pow(freq - 12.5, 2) / 0.1);   // ~12.5h period
      magnitude += Math.random() * 1.5; // noise floor
      state.spectralData.push({ frequency: freq, magnitude: Math.max(0, magnitude) });
    }
  }

  function resizeFftCanvas() {
    var c = state._fftCanvas;
    if (!c) return;
    var rect = c.parentElement.getBoundingClientRect();
    c.width = rect.width * (window.devicePixelRatio || 1);
    c.height = rect.height * (window.devicePixelRatio || 1);
  }

  function drawFFT() {
    var canvas = state._fftCanvas;
    if (!canvas || !canvas.getContext) return;
    var ctx = canvas.getContext('2d');
    var w = canvas.width;
    var h = canvas.height;
    var dpr = window.devicePixelRatio || 1;
    var data = state.spectralData;

    ctx.clearRect(0, 0, w, h);
    if (data.length < 2) return;

    var pad = { top: 20 * dpr, right: 20 * dpr, bottom: 30 * dpr, left: 50 * dpr };
    var cw = w - pad.left - pad.right;
    var ch = h - pad.top - pad.bottom;

    var maxMag = 0;
    data.forEach(function(d) { if (d.magnitude > maxMag) maxMag = d.magnitude; });
    maxMag = Math.max(maxMag, 1);

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
      ctx.fillText((maxMag * g / 4).toFixed(0), pad.left - 6 * dpr, gy + 3 * dpr);
    }

    // X-axis label
    ctx.fillStyle = getCSS('--text-dim');
    ctx.font = (9 * dpr) + 'px ' + getCSS('--font');
    ctx.textAlign = 'center';
    ctx.fillText('Frequency (cycles/hour)', pad.left + cw / 2, pad.top + ch + 22 * dpr);

    // Frequency ticks
    for (var f = 0; f <= 30; f += 5) {
      var fx = pad.left + (f / 32) * cw;
      ctx.fillText(String(f), fx, pad.top + ch + 14 * dpr);
    }

    // Bars with gradient
    var barW = Math.max(2 * dpr, (cw / data.length) * 0.7);
    data.forEach(function(d, i) {
      var x = pad.left + (i / data.length) * cw;
      var barH = (d.magnitude / maxMag) * ch;
      var y = pad.top + ch - barH;

      var grad = ctx.createLinearGradient(x, y, x, pad.top + ch);
      grad.addColorStop(0, 'rgba(78,205,196,0.9)');
      grad.addColorStop(1, 'rgba(78,205,196,0.1)');
      ctx.fillStyle = grad;
      ctx.fillRect(x, y, barW, barH);
    });

    // Highlight peaks
    var peakThreshold = maxMag * 0.4;
    data.forEach(function(d, i) {
      if (d.magnitude > peakThreshold) {
        var px = pad.left + (i / data.length) * cw + barW / 2;
        var py = pad.top + ch - (d.magnitude / maxMag) * ch - 8 * dpr;
        ctx.fillStyle = getCSS('--accent-red');
        ctx.beginPath();
        ctx.arc(px, py, 3 * dpr, 0, Math.PI * 2);
        ctx.fill();

        ctx.fillStyle = getCSS('--text-primary');
        ctx.font = (8 * dpr) + 'px ' + getCSS('--mono');
        ctx.textAlign = 'center';
        ctx.fillText(d.frequency.toFixed(1) + 'c/h', px, py - 6 * dpr);
      }
    });

    // Y-axis label
    ctx.save();
    ctx.translate(14 * dpr, pad.top + ch / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.fillStyle = getCSS('--text-dim');
    ctx.font = (9 * dpr) + 'px ' + getCSS('--font');
    ctx.textAlign = 'center';
    ctx.fillText('Magnitude', 0, 0);
    ctx.restore();
  }

  // ── History Panel ──────────────────────────────────────────────
  function renderHistory(container) {
    if (!container) return;
    container.innerHTML = '';

    var demoHistory = [
      { process: 'msedge.exe', detectedAt: '2h ago', resolvedAt: '45m ago', resolution: 'Process restarted', leakMb: 1240 },
      { process: 'vscode.exe', detectedAt: '1d ago', resolvedAt: '20h ago', resolution: 'Extension disabled', leakMb: 890 },
      { process: 'teams.exe', detectedAt: '3d ago', resolvedAt: '3d ago', resolution: 'Auto-terminated', leakMb: 2100 }
    ];

    if (state.history && state.history.length > 0) {
      demoHistory = state.history;
    }

    demoHistory.forEach(function(h) {
      var row = createEl('div', '', container);
      row.style.cssText = 'display:flex;align-items:center;gap:10px;padding:8px 0;border-bottom:1px solid var(--border);font-size:11px';

      var dot = createEl('span', 'ph-dot', row);
      dot.style.background = 'var(--accent-green)';

      var info = createEl('div', '', row);
      info.style.flex = '1';
      var name = createEl('div', '', info);
      name.style.cssText = 'color:var(--text-primary);font-weight:500';
      name.textContent = h.process;
      var detail = createEl('div', '', info);
      detail.style.cssText = 'font-size:10px;color:var(--text-dim);margin-top:2px';
      detail.textContent = 'Detected ' + h.detectedAt + ' \u2022 Resolved ' + h.resolvedAt;

      var res = createEl('span', '', row);
      res.style.cssText = 'font-size:10px;color:var(--text-secondary);width:120px';
      res.textContent = h.resolution;

      var leak = createEl('span', '', row);
      leak.style.cssText = 'font-family:var(--mono);font-size:10px;color:var(--accent-amber);width:60px;text-align:right';
      leak.textContent = h.leakMb + ' MB';
    });
  }

  // ── Animate suspects (minor data shifts for liveness) ──────────
  function animateSuspects() {
    state.suspects.forEach(function(s) {
      s.currentMb += s.growthRate / 450 + (Math.random() - 0.3) * 2;
      s.history.push(s.currentMb);
      if (s.history.length > 40) s.history.shift();
    });
    renderSuspects(el('leaks-suspects-body'));
  }

  // ── Column helper ──────────────────────────────────────────────
  function addCol(parent, text, style) {
    var span = createEl('span', '', parent);
    span.style.cssText = style;
    span.textContent = text;
  }

  // ── Slider helper ──────────────────────────────────────────────
  function buildSlider(parent, label, id, min, max, initial, unit, onChange) {
    var row = createEl('div', '', parent);
    row.style.margin = '8px 0';
    var top = createEl('div', '', row);
    top.style.cssText = 'display:flex;justify-content:space-between;font-size:10px;color:var(--text-secondary);margin-bottom:4px';
    createEl('span', '', top).textContent = label;
    var valEl = createEl('span', '', top);
    valEl.id = id + '-val';
    valEl.style.cssText = 'font-family:var(--mono);color:var(--text-primary);font-weight:500';
    valEl.textContent = initial + (unit ? unit : '');

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
      valEl.textContent = v + (unit ? unit : '');
      if (onChange) onChange(v);
    });
  }

  // ── Update ─────────────────────────────────────────────────────
  function updateUI(data) {
    if (!data || !state._container) return;

    if (data.suspects) {
      state.suspects = data.suspects;
      // Ensure each suspect has a history array
      state.suspects.forEach(function(s) {
        if (!s.history || s.history.length === 0) {
          s.history = [];
          var base = s.currentMb - s.growthRate * 2;
          for (var i = 0; i < 40; i++) {
            s.history.push(base + (s.growthRate / 40) * i + (Math.random() - 0.3) * s.growthRate * 0.1);
          }
        }
      });
      renderSuspects(el('leaks-suspects-body'));
    }

    if (data.history) {
      state.history = data.history;
      renderHistory(el('leaks-history-list'));
    }

    if (data.spectral_data) {
      state.spectralData = data.spectral_data;
      drawFFT();
    }
  }

  // ── Cleanup ────────────────────────────────────────────────────
  function cleanup() {
    if (state._refreshTimer) { clearInterval(state._refreshTimer); state._refreshTimer = null; }
  }

  // ── Public API ─────────────────────────────────────────────────
  window.RuVectorPages.leaks_Init = function(container) {
    cleanup();
    buildUI(container);
    window.addEventListener('resize', function() { resizeFftCanvas(); drawFFT(); });
  };

  window.RuVectorPages.leaks_Update = function(data) { updateUI(data); };

  window.updateLeakSuspects = function(data) { updateUI(data); };
  window.updateLeakHistory = function(data) { if (data) updateUI({ history: data.history || data }); };
  window.updateSpectralData = function(data) { if (data) { state.spectralData = data.spectral_data || data; drawFFT(); } };
  window.dismissLeakResult = function(r) {
    if (r && r.success && typeof window.showToast === 'function') {
      window.showToast('Leak Dismissed', 'Process removed from suspects', 'success');
    }
    ipcSend('get_leak_suspects');
  };

})();
