/**
 * ADR-019: Predictive Prefetcher
 * Markov-chain based application launch prediction with temporal weights.
 */
(function(){
  'use strict';

  window.RuVectorPages = window.RuVectorPages || {};

  // ── State ──────────────────────────────────────────────────────
  var state = {
    enabled: true,
    hitRate: 0,
    missRate: 0,
    overallScore: 0,
    maxPrefetchMb: 512,
    confidenceThreshold: 70,
    trainingSamples: 0,
    modelAccuracy: 0,
    lastTrainingTime: '--',
    predictions: [],
    prefetchQueue: [],
    appPatterns: [],
    attentionWeights: [],
    _container: null,
    _heatmapCanvas: null,
    _accuracyCanvas: null,
    _attentionCanvas: null,
    _refreshTimer: null
  };

  // Demo predictions
  var demoPredictions = [
    { app: 'VS Code', time: '09:00', confidence: 92, status: 'prefetched', sizeMb: 85 },
    { app: 'Chrome', time: '09:05', confidence: 88, status: 'prefetched', sizeMb: 120 },
    { app: 'Slack', time: '09:10', confidence: 81, status: 'waiting', sizeMb: 65 },
    { app: 'Terminal', time: '09:15', confidence: 76, status: 'waiting', sizeMb: 12 },
    { app: 'Docker Desktop', time: '09:30', confidence: 64, status: 'waiting', sizeMb: 180 },
    { app: 'Postman', time: '10:00', confidence: 55, status: 'missed', sizeMb: 90 },
    { app: 'Figma', time: '14:00', confidence: 48, status: 'missed', sizeMb: 110 }
  ];

  var demoPrefetchQueue = [
    { app: 'VS Code', progress: 100, sizeMb: 85 },
    { app: 'Chrome', progress: 78, sizeMb: 120 },
    { app: 'Slack', progress: 0, sizeMb: 65 }
  ];

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

  function statusColor(s) {
    switch (s) {
      case 'prefetched': return 'var(--accent-green)';
      case 'waiting': return 'var(--accent-cyan)';
      case 'missed': return 'var(--accent-red)';
      default: return 'var(--text-dim)';
    }
  }

  // ── Build UI ───────────────────────────────────────────────────
  function buildUI(container) {
    state._container = container;
    container.innerHTML = '';
    state.predictions = demoPredictions.slice();
    state.prefetchQueue = demoPrefetchQueue.slice();
    state.hitRate = 74;
    state.missRate = 26;
    state.overallScore = 82;
    state.trainingSamples = 1847;
    state.modelAccuracy = 78.3;
    state.lastTrainingTime = '12 min ago';

    var inner = createEl('div', 'page-inner', container);

    // Header
    var hdr = createEl('div', 'page-header', inner);
    createEl('span', 'page-icon', hdr).textContent = '\uD83D\uDE80';
    createEl('h2', '', hdr).textContent = 'Predictive Prefetcher';

    createEl('div', 'page-status', inner).textContent = 'ADR-019';

    var desc = createEl('p', 'page-desc', inner);
    desc.textContent = 'Markov-chain based application launch prediction. Learns your usage patterns with temporal weights and prefetches application data before you need it, reducing perceived launch times.';

    var grid = createEl('div', 'page-grid', inner);

    // ── Card 1: Prediction Accuracy ──────────────────────────────
    var c1 = createEl('div', 'ph-card', grid);
    createEl('div', 'ph-card-title', c1).textContent = 'Prediction Accuracy';

    var accGrid = createEl('div', '', c1);
    accGrid.style.cssText = 'display:grid;grid-template-columns:1fr 1fr 1fr;gap:10px;margin-bottom:14px';

    buildMetricBox(accGrid, 'pf-hit-rate', '74%', 'Hit Rate', 'var(--accent-green)');
    buildMetricBox(accGrid, 'pf-miss-rate', '26%', 'Miss Rate', 'var(--accent-red)');
    buildMetricBox(accGrid, 'pf-overall', '82', 'Overall Score', 'var(--accent-cyan)');

    // Accuracy chart
    var accChartWrap = createEl('div', '', c1);
    accChartWrap.style.cssText = 'height:100px;background:var(--bg-primary);border-radius:8px;overflow:hidden';
    var accCanvas = document.createElement('canvas');
    accCanvas.id = 'pf-accuracy-chart';
    accCanvas.style.cssText = 'width:100%;height:100%;display:block';
    accChartWrap.appendChild(accCanvas);
    state._accuracyCanvas = accCanvas;

    // ── Card 2: Learning Status ──────────────────────────────────
    var c2 = createEl('div', 'ph-card', grid);
    createEl('div', 'ph-card-title', c2).textContent = 'Learning Status';

    buildField(c2, 'Training Samples', 'pf-samples', '1,847');
    buildField(c2, 'Model Accuracy', 'pf-model-acc', '78.3%');
    buildField(c2, 'Last Training', 'pf-last-train', '12 min ago');
    buildField(c2, 'Markov States', 'pf-states', '42');
    buildField(c2, 'Transitions', 'pf-transitions', '156');

    // Train button
    var trainWrap = createEl('div', 'ph-actions', c2);
    var trainBtn = createEl('button', 'btn', trainWrap);
    trainBtn.style.cssText = 'cursor:pointer;opacity:1';
    trainBtn.innerHTML = '<span class="ico">&#129504;</span> Retrain Model';
    trainBtn.addEventListener('click', function() {
      trainBtn.disabled = true;
      trainBtn.textContent = 'Training...';
      ipcSend('train_prefetcher');
      setTimeout(function() {
        trainBtn.disabled = false;
        trainBtn.innerHTML = '<span class="ico">&#129504;</span> Retrain Model';
        setText('pf-last-train', 'Just now');
        if (typeof window.showToast === 'function') {
          window.showToast('Training Complete', 'Prefetch model updated with latest patterns', 'success');
        }
      }, 3000);
    });

    // ── Card 3: Settings ─────────────────────────────────────────
    var c3 = createEl('div', 'ph-card', grid);
    createEl('div', 'ph-card-title', c3).textContent = 'Prefetch Settings';

    // Enable toggle
    var enableRow = createEl('div', 'toggle-row', c3);
    createEl('span', '', enableRow).textContent = 'Enable Prefetching';
    var enableToggle = createEl('div', 'toggle on', enableRow);
    enableToggle.id = 'pf-enable-toggle';
    enableToggle.addEventListener('click', function() {
      enableToggle.classList.toggle('on');
      state.enabled = enableToggle.classList.contains('on');
      ipcSend('set_prefetch_config', { enabled: state.enabled });
    });

    // Max memory slider
    buildSlider(c3, 'Max Prefetch Memory', 'pf-max-mem', 64, 2048, state.maxPrefetchMb, 'MB', function(v) {
      state.maxPrefetchMb = v;
    });

    // Confidence threshold slider
    buildSlider(c3, 'Min Confidence', 'pf-confidence', 20, 95, state.confidenceThreshold, '%', function(v) {
      state.confidenceThreshold = v;
    });

    // Apply settings
    var settingsWrap = createEl('div', 'ph-actions', c3);
    var settingsBtn = createEl('button', 'btn', settingsWrap);
    settingsBtn.style.cssText = 'cursor:pointer;opacity:1';
    settingsBtn.innerHTML = '<span class="ico">&#9989;</span> Apply Settings';
    settingsBtn.addEventListener('click', function() {
      ipcSend('set_prefetch_config', {
        enabled: state.enabled,
        max_prefetch_mb: state.maxPrefetchMb,
        confidence_threshold: state.confidenceThreshold / 100
      });
      if (typeof window.showToast === 'function') {
        window.showToast('Settings Saved', 'Prefetch configuration updated', 'success');
      }
    });

    // ── Card 4: Prefetch Queue ───────────────────────────────────
    var c4 = createEl('div', 'ph-card', grid);
    createEl('div', 'ph-card-title', c4).textContent = 'Prefetch Queue';

    var queueList = createEl('div', '', c4);
    queueList.id = 'pf-queue-list';
    renderQueue(queueList);

    // ── Card 5: Active Predictions (full width) ──────────────────
    var c5 = createEl('div', 'ph-card', grid);
    c5.style.gridColumn = '1 / -1';
    createEl('div', 'ph-card-title', c5).textContent = 'Active Predictions';

    var predHeader = createEl('div', '', c5);
    predHeader.style.cssText = 'display:flex;align-items:center;gap:8px;padding:6px 0;border-bottom:1px solid var(--border);font-size:9px;font-weight:600;color:var(--text-dim);letter-spacing:0.5px;text-transform:uppercase';
    addCol(predHeader, 'Application', 'flex:1.5');
    addCol(predHeader, 'Predicted Time', 'width:90px;text-align:center');
    addCol(predHeader, 'Confidence', 'width:80px;text-align:right');
    addCol(predHeader, 'Size', 'width:60px;text-align:right');
    addCol(predHeader, 'Status', 'width:90px;text-align:center');

    var predBody = createEl('div', '', c5);
    predBody.id = 'pf-predictions-body';
    renderPredictions(predBody);

    // ── Card 6: App Usage Heatmap (full width) ───────────────────
    var c6 = createEl('div', 'ph-card', grid);
    c6.style.gridColumn = '1 / -1';
    createEl('div', 'ph-card-title', c6).textContent = 'App Usage Patterns (Hourly Heatmap)';

    var heatWrap = createEl('div', '', c6);
    heatWrap.style.cssText = 'height:220px;position:relative;background:var(--bg-primary);border-radius:8px;overflow:hidden';
    var heatCanvas = document.createElement('canvas');
    heatCanvas.id = 'pf-heatmap-chart';
    heatCanvas.style.cssText = 'width:100%;height:100%;display:block';
    heatWrap.appendChild(heatCanvas);
    state._heatmapCanvas = heatCanvas;

    seedPatterns();

    // ── Card 7: Neural Attention Weights ─────────────────────────
    var c7 = createEl('div', 'ph-card', grid);
    c7.style.gridColumn = '1 / -1';
    createEl('div', 'ph-card-title', c7).textContent = 'Neural Engine - Attention Weights';

    var attWrap = createEl('div', '', c7);
    attWrap.style.cssText = 'height:120px;position:relative;background:var(--bg-primary);border-radius:8px;overflow:hidden';
    var attCanvas = document.createElement('canvas');
    attCanvas.id = 'pf-attention-chart';
    attCanvas.style.cssText = 'width:100%;height:100%;display:block';
    attWrap.appendChild(attCanvas);
    state._attentionCanvas = attCanvas;

    seedAttentionWeights();

    // Request data
    ipcSend('get_prefetch_status');
    ipcSend('get_predictions');
    ipcSend('get_app_patterns');

    state._refreshTimer = setInterval(function() {
      ipcSend('get_prefetch_status');
      ipcSend('get_predictions');
    }, 10000);

    setTimeout(function() {
      resizeCanvases();
      drawHeatmap();
      drawAccuracyChart();
      drawAttentionChart();
    }, 100);
  }

  // ── Metric Box ─────────────────────────────────────────────────
  function buildMetricBox(parent, id, value, label, color) {
    var box = createEl('div', '', parent);
    box.style.cssText = 'text-align:center;padding:10px;background:var(--bg-primary);border-radius:8px';
    var val = createEl('div', '', box);
    val.id = id;
    val.style.cssText = 'font-size:28px;font-weight:700;line-height:1;color:' + color;
    val.textContent = value;
    var lbl = createEl('div', '', box);
    lbl.style.cssText = 'font-size:9px;color:var(--text-dim);text-transform:uppercase;letter-spacing:0.8px;margin-top:4px';
    lbl.textContent = label;
  }

  // ── Field / Slider / Column helpers ────────────────────────────
  function buildField(parent, label, id, defaultVal) {
    var row = createEl('div', 'ph-field', parent);
    createEl('span', 'ph-field-label', row).textContent = label;
    var val = createEl('span', 'ph-field-value', row);
    val.id = id;
    val.textContent = defaultVal;
  }

  function buildSlider(parent, label, id, min, max, initial, unit, onChange) {
    var row = createEl('div', '', parent);
    row.style.margin = '8px 0';
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

  function addCol(parent, text, style) {
    var span = createEl('span', '', parent);
    span.style.cssText = style;
    span.textContent = text;
  }

  // ── Predictions Table ──────────────────────────────────────────
  function renderPredictions(container) {
    if (!container) return;
    container.innerHTML = '';

    state.predictions.forEach(function(p) {
      var row = createEl('div', '', container);
      row.style.cssText = 'display:flex;align-items:center;gap:8px;padding:8px 0;border-bottom:1px solid var(--border);font-size:11px';

      var nameEl = createEl('span', '', row);
      nameEl.style.cssText = 'flex:1.5;color:var(--text-primary);font-weight:500';
      nameEl.textContent = p.app;

      var timeEl = createEl('span', '', row);
      timeEl.style.cssText = 'width:90px;text-align:center;font-family:var(--mono);font-size:10px;color:var(--text-secondary)';
      timeEl.textContent = p.time;

      var confEl = createEl('span', '', row);
      confEl.style.cssText = 'width:80px;text-align:right;font-family:var(--mono);font-size:10px;font-weight:600';
      confEl.style.color = p.confidence >= 80 ? 'var(--accent-green)' : p.confidence >= 60 ? 'var(--accent-amber)' : 'var(--text-dim)';
      confEl.textContent = p.confidence + '%';

      var sizeEl = createEl('span', '', row);
      sizeEl.style.cssText = 'width:60px;text-align:right;font-family:var(--mono);font-size:10px;color:var(--text-dim)';
      sizeEl.textContent = p.sizeMb + 'MB';

      var statusEl = createEl('span', '', row);
      statusEl.style.cssText = 'width:90px;text-align:center;font-size:9px;font-weight:600;letter-spacing:0.5px;text-transform:uppercase;padding:2px 8px;border-radius:10px';
      statusEl.style.color = statusColor(p.status);
      statusEl.style.background = p.status === 'prefetched' ? 'rgba(107,203,119,0.12)' :
                                   p.status === 'waiting' ? 'rgba(78,205,196,0.12)' :
                                   'rgba(224,96,96,0.12)';
      statusEl.textContent = p.status;
    });
  }

  // ── Prefetch Queue ─────────────────────────────────────────────
  function renderQueue(container) {
    if (!container) return;
    container.innerHTML = '';

    if (!state.prefetchQueue || state.prefetchQueue.length === 0) {
      var empty = createEl('div', '', container);
      empty.style.cssText = 'font-size:11px;color:var(--text-dim);padding:8px 0';
      empty.textContent = 'Queue empty';
      return;
    }

    state.prefetchQueue.forEach(function(q) {
      var row = createEl('div', '', container);
      row.style.cssText = 'padding:8px 0;border-bottom:1px solid var(--border)';

      var top = createEl('div', '', row);
      top.style.cssText = 'display:flex;justify-content:space-between;font-size:11px;margin-bottom:4px';
      var name = createEl('span', '', top);
      name.style.color = 'var(--text-primary)';
      name.style.fontWeight = '500';
      name.textContent = q.app;
      var info = createEl('span', '', top);
      info.style.cssText = 'font-family:var(--mono);font-size:10px;color:var(--text-dim)';
      info.textContent = q.sizeMb + 'MB \u2022 ' + q.progress + '%';

      var track = createEl('div', '', row);
      track.style.cssText = 'height:4px;background:var(--gauge-track);border-radius:2px;overflow:hidden';
      var bar = createEl('div', '', track);
      bar.style.cssText = 'height:100%;border-radius:2px;transition:width 0.6s ease';
      bar.style.width = q.progress + '%';
      bar.style.background = q.progress === 100 ? 'var(--accent-green)' : 'var(--accent-cyan)';
    });
  }

  // ── Seed pattern data for heatmap ──────────────────────────────
  function seedPatterns() {
    state.appPatterns = [];
    var apps = ['VS Code', 'Chrome', 'Slack', 'Terminal', 'Docker', 'Postman', 'Figma'];
    var days = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];

    apps.forEach(function(app, ai) {
      for (var d = 0; d < 7; d++) {
        for (var h = 0; h < 24; h++) {
          var intensity = 0;
          // Work hours bias
          if (h >= 9 && h <= 17 && d < 5) {
            intensity = Math.random() * 0.6;
            if (ai < 3) intensity += 0.3; // VS Code, Chrome, Slack more common
          }
          // Morning startup
          if (h >= 8 && h <= 10 && d < 5 && ai < 4) {
            intensity = Math.min(1, intensity + 0.4);
          }
          // Lunch dip
          if (h >= 12 && h <= 13) {
            intensity *= 0.3;
          }
          // Weekend low
          if (d >= 5) {
            intensity *= 0.15;
            if (ai === 1) intensity += Math.random() * 0.3; // Chrome on weekends
          }
          state.appPatterns.push({
            app: app,
            appIndex: ai,
            day: d,
            hour: h,
            intensity: Math.min(1, Math.max(0, intensity))
          });
        }
      }
    });
  }

  // ── Seed attention weights ─────────────────────────────────────
  function seedAttentionWeights() {
    state.attentionWeights = [];
    var features = ['Time-of-day', 'Day-of-week', 'Previous App', 'Session Length', 'CPU Load', 'Memory Pressure', 'Build Active', 'Network Activity'];
    features.forEach(function(f) {
      state.attentionWeights.push({
        feature: f,
        weight: Math.random() * 0.8 + 0.1
      });
    });
    // Normalize
    var sum = 0;
    state.attentionWeights.forEach(function(w) { sum += w.weight; });
    state.attentionWeights.forEach(function(w) { w.weight /= sum; });
    // Sort descending
    state.attentionWeights.sort(function(a, b) { return b.weight - a.weight; });
  }

  // ── Canvas Management ──────────────────────────────────────────
  function resizeCanvases() {
    [state._heatmapCanvas, state._accuracyCanvas, state._attentionCanvas].forEach(function(c) {
      if (!c) return;
      var rect = c.parentElement.getBoundingClientRect();
      c.width = rect.width * (window.devicePixelRatio || 1);
      c.height = rect.height * (window.devicePixelRatio || 1);
    });
  }

  // ── Accuracy Chart (line over time) ────────────────────────────
  function drawAccuracyChart() {
    var canvas = state._accuracyCanvas;
    if (!canvas || !canvas.getContext) return;
    var ctx = canvas.getContext('2d');
    var w = canvas.width;
    var h = canvas.height;
    var dpr = window.devicePixelRatio || 1;

    ctx.clearRect(0, 0, w, h);

    // Generate smooth accuracy curve
    var data = [];
    for (var i = 0; i < 30; i++) {
      data.push(50 + i * 0.8 + Math.sin(i * 0.4) * 5 + (Math.random() - 0.5) * 4);
    }

    var pad = { top: 8 * dpr, right: 8 * dpr, bottom: 8 * dpr, left: 8 * dpr };
    var cw = w - pad.left - pad.right;
    var ch = h - pad.top - pad.bottom;

    // Fill
    ctx.beginPath();
    for (var j = 0; j < data.length; j++) {
      var x = pad.left + (j / (data.length - 1)) * cw;
      var y = pad.top + ch * (1 - (data[j] - 40) / 50);
      if (j === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.lineTo(pad.left + cw, pad.top + ch);
    ctx.lineTo(pad.left, pad.top + ch);
    ctx.closePath();
    var grad = ctx.createLinearGradient(0, pad.top, 0, pad.top + ch);
    grad.addColorStop(0, 'rgba(107,203,119,0.3)');
    grad.addColorStop(1, 'rgba(107,203,119,0.02)');
    ctx.fillStyle = grad;
    ctx.fill();

    // Line
    ctx.beginPath();
    for (var k = 0; k < data.length; k++) {
      var lx = pad.left + (k / (data.length - 1)) * cw;
      var ly = pad.top + ch * (1 - (data[k] - 40) / 50);
      if (k === 0) ctx.moveTo(lx, ly);
      else ctx.lineTo(lx, ly);
    }
    ctx.strokeStyle = getCSS('--accent-green');
    ctx.lineWidth = 2 * dpr;
    ctx.stroke();
  }

  // ── Heatmap (24x7 grid) ────────────────────────────────────────
  function drawHeatmap() {
    var canvas = state._heatmapCanvas;
    if (!canvas || !canvas.getContext) return;
    var ctx = canvas.getContext('2d');
    var w = canvas.width;
    var h = canvas.height;
    var dpr = window.devicePixelRatio || 1;
    var data = state.appPatterns;

    ctx.clearRect(0, 0, w, h);

    var days = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
    var pad = { top: 16 * dpr, right: 12 * dpr, bottom: 28 * dpr, left: 40 * dpr };
    var cw = w - pad.left - pad.right;
    var ch = h - pad.top - pad.bottom;

    var cellW = cw / 24;
    var cellH = ch / 7;

    // Aggregate by day/hour (sum all apps)
    var heatGrid = [];
    for (var d = 0; d < 7; d++) {
      heatGrid[d] = [];
      for (var hr = 0; hr < 24; hr++) {
        heatGrid[d][hr] = 0;
      }
    }
    data.forEach(function(p) {
      heatGrid[p.day][p.hour] += p.intensity;
    });

    // Find max for normalization
    var maxVal = 0;
    for (var dd = 0; dd < 7; dd++) {
      for (var hh = 0; hh < 24; hh++) {
        if (heatGrid[dd][hh] > maxVal) maxVal = heatGrid[dd][hh];
      }
    }
    maxVal = Math.max(maxVal, 0.01);

    // Draw cells
    for (var dy = 0; dy < 7; dy++) {
      for (var hx = 0; hx < 24; hx++) {
        var intensity = heatGrid[dy][hx] / maxVal;
        var cx = pad.left + hx * cellW;
        var cy = pad.top + dy * cellH;

        // Color interpolation: dark bg -> cyan -> green
        var r, g, b;
        if (intensity < 0.3) {
          var t = intensity / 0.3;
          r = Math.round(20 + t * 50);
          g = Math.round(30 + t * 170);
          b = Math.round(50 + t * 150);
        } else {
          var t2 = (intensity - 0.3) / 0.7;
          r = Math.round(70 + t2 * 37);
          g = Math.round(200 - t2 * 0);
          b = Math.round(200 - t2 * 83);
        }

        ctx.fillStyle = 'rgb(' + r + ',' + g + ',' + b + ')';
        ctx.fillRect(cx + 1, cy + 1, cellW - 2, cellH - 2);

        // Round corners via clip would be expensive, skip for perf
      }
    }

    // Day labels
    ctx.fillStyle = getCSS('--text-dim');
    ctx.font = (9 * dpr) + 'px ' + getCSS('--font');
    ctx.textAlign = 'right';
    for (var dl = 0; dl < 7; dl++) {
      ctx.fillText(days[dl], pad.left - 6 * dpr, pad.top + dl * cellH + cellH / 2 + 3 * dpr);
    }

    // Hour labels
    ctx.textAlign = 'center';
    for (var hl = 0; hl < 24; hl += 3) {
      ctx.fillText(String(hl) + ':00', pad.left + hl * cellW + cellW / 2, pad.top + ch + 16 * dpr);
    }

    // Legend
    var legendX = w - pad.right - 120 * dpr;
    var legendY = pad.top + ch + 14 * dpr;
    ctx.font = (8 * dpr) + 'px ' + getCSS('--font');
    ctx.fillStyle = getCSS('--text-dim');
    ctx.textAlign = 'left';
    ctx.fillText('Low', legendX, legendY);
    for (var li = 0; li < 8; li++) {
      var lInt = li / 7;
      var lr, lg, lb;
      if (lInt < 0.3) {
        lr = Math.round(20 + (lInt / 0.3) * 50);
        lg = Math.round(30 + (lInt / 0.3) * 170);
        lb = Math.round(50 + (lInt / 0.3) * 150);
      } else {
        var lt = (lInt - 0.3) / 0.7;
        lr = Math.round(70 + lt * 37);
        lg = Math.round(200);
        lb = Math.round(200 - lt * 83);
      }
      ctx.fillStyle = 'rgb(' + lr + ',' + lg + ',' + lb + ')';
      ctx.fillRect(legendX + 28 * dpr + li * 8 * dpr, legendY - 7 * dpr, 7 * dpr, 7 * dpr);
    }
    ctx.fillStyle = getCSS('--text-dim');
    ctx.fillText('High', legendX + 96 * dpr, legendY);
  }

  // ── Attention Weights Chart (horizontal bars) ──────────────────
  function drawAttentionChart() {
    var canvas = state._attentionCanvas;
    if (!canvas || !canvas.getContext) return;
    var ctx = canvas.getContext('2d');
    var w = canvas.width;
    var h = canvas.height;
    var dpr = window.devicePixelRatio || 1;
    var data = state.attentionWeights;

    ctx.clearRect(0, 0, w, h);
    if (!data || data.length === 0) return;

    var pad = { top: 8 * dpr, right: 50 * dpr, bottom: 8 * dpr, left: 110 * dpr };
    var cw = w - pad.left - pad.right;
    var ch = h - pad.top - pad.bottom;
    var barH = Math.min(14 * dpr, ch / data.length - 2 * dpr);
    var gap = (ch - barH * data.length) / (data.length + 1);

    var maxWeight = 0;
    data.forEach(function(d) { if (d.weight > maxWeight) maxWeight = d.weight; });

    data.forEach(function(d, i) {
      var y = pad.top + gap * (i + 1) + barH * i;
      var barW = (d.weight / maxWeight) * cw;

      // Label
      ctx.fillStyle = getCSS('--text-secondary');
      ctx.font = (9 * dpr) + 'px ' + getCSS('--font');
      ctx.textAlign = 'right';
      ctx.fillText(d.feature, pad.left - 8 * dpr, y + barH / 2 + 3 * dpr);

      // Bar with gradient
      var grad = ctx.createLinearGradient(pad.left, 0, pad.left + barW, 0);
      grad.addColorStop(0, 'rgba(167,139,250,0.8)');
      grad.addColorStop(1, 'rgba(78,205,196,0.8)');
      ctx.fillStyle = grad;
      ctx.beginPath();
      roundRect(ctx, pad.left, y, barW, barH, 3 * dpr);
      ctx.fill();

      // Value
      ctx.fillStyle = getCSS('--text-dim');
      ctx.font = (8 * dpr) + 'px ' + getCSS('--mono');
      ctx.textAlign = 'left';
      ctx.fillText((d.weight * 100).toFixed(1) + '%', pad.left + barW + 6 * dpr, y + barH / 2 + 3 * dpr);
    });
  }

  function roundRect(ctx, x, y, w, h, r) {
    ctx.moveTo(x + r, y);
    ctx.lineTo(x + w - r, y);
    ctx.quadraticCurveTo(x + w, y, x + w, y + r);
    ctx.lineTo(x + w, y + h - r);
    ctx.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
    ctx.lineTo(x + r, y + h);
    ctx.quadraticCurveTo(x, y + h, x, y + h - r);
    ctx.lineTo(x, y + r);
    ctx.quadraticCurveTo(x, y, x + r, y);
  }

  // ── Update ─────────────────────────────────────────────────────
  function updateUI(data) {
    if (!data || !state._container) return;

    if (data.hit_rate !== undefined) {
      state.hitRate = data.hit_rate;
      setText('pf-hit-rate', Math.round(data.hit_rate) + '%');
    }
    if (data.miss_rate !== undefined) {
      state.missRate = data.miss_rate;
      setText('pf-miss-rate', Math.round(data.miss_rate) + '%');
    }
    if (data.overall_score !== undefined) {
      state.overallScore = data.overall_score;
      setText('pf-overall', Math.round(data.overall_score));
    }
    if (data.training_samples !== undefined) {
      state.trainingSamples = data.training_samples;
      setText('pf-samples', data.training_samples.toLocaleString());
    }
    if (data.model_accuracy !== undefined) {
      state.modelAccuracy = data.model_accuracy;
      setText('pf-model-acc', data.model_accuracy.toFixed(1) + '%');
    }
    if (data.last_training_time) {
      state.lastTrainingTime = data.last_training_time;
      setText('pf-last-train', data.last_training_time);
    }
    if (data.states !== undefined) setText('pf-states', String(data.states));
    if (data.transitions !== undefined) setText('pf-transitions', String(data.transitions));

    if (data.predictions) {
      state.predictions = data.predictions;
      renderPredictions(el('pf-predictions-body'));
    }

    if (data.prefetch_queue) {
      state.prefetchQueue = data.prefetch_queue;
      renderQueue(el('pf-queue-list'));
    }

    if (data.app_patterns) {
      state.appPatterns = data.app_patterns;
      drawHeatmap();
    }

    if (data.attention_weights) {
      state.attentionWeights = data.attention_weights;
      drawAttentionChart();
    }

    if (data.enabled !== undefined) {
      state.enabled = data.enabled;
      var toggle = el('pf-enable-toggle');
      if (toggle) {
        if (data.enabled) toggle.classList.add('on');
        else toggle.classList.remove('on');
      }
    }
  }

  // ── Cleanup ────────────────────────────────────────────────────
  function cleanup() {
    if (state._refreshTimer) { clearInterval(state._refreshTimer); state._refreshTimer = null; }
  }

  // ── Public API ─────────────────────────────────────────────────
  window.RuVectorPages.prefetch_Init = function(container) {
    cleanup();
    buildUI(container);
    window.addEventListener('resize', function() {
      resizeCanvases();
      drawHeatmap();
      drawAccuracyChart();
      drawAttentionChart();
    });
  };

  window.RuVectorPages.prefetch_Update = function(data) { updateUI(data); };

  window.updatePrefetchStatus = function(data) { updateUI(data); };
  window.updatePredictions = function(data) { if (data) updateUI(data); };
  window.updateAppPatterns = function(data) {
    if (data && data.app_patterns) {
      state.appPatterns = data.app_patterns;
      drawHeatmap();
    }
  };
  window.trainPrefetcherResult = function(r) {
    if (r && r.success && typeof window.showToast === 'function') {
      window.showToast('Training Complete', 'Model accuracy: ' + (r.accuracy || state.modelAccuracy).toFixed(1) + '%', 'success');
    }
    ipcSend('get_prefetch_status');
  };

})();
