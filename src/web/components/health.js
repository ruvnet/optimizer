/**
 * ADR-014: System Health Score Component
 * Composite health score with circular gauge, grade letter,
 * subscore breakdowns, history sparkline, and recommendations.
 */
(function(){
  'use strict';

  window.RuVectorPages = window.RuVectorPages || {};

  // ── State ──────────────────────────────────────────────────────
  var state = {
    score: 82,
    prevScore: 79,
    grade: 'B+',
    subscores: [
      { name: 'Memory',       key: 'memory',    score: 85, icon: '\u{1F4BE}' },
      { name: 'CPU',          key: 'cpu',        score: 78, icon: '\u{1F4BB}' },
      { name: 'Disk I/O',     key: 'disk',       score: 92, icon: '\u{1F4BD}' },
      { name: 'Thermal',      key: 'thermal',    score: 95, icon: '\u{1F321}' },
      { name: 'Process',      key: 'process',    score: 70, icon: '\u2699' }
    ],
    history: [],
    recommendations: [
      { text: 'Close unused browser tabs to reduce memory pressure', severity: 'warning', impact: '+5 points' },
      { text: 'Defragment working set to improve process efficiency', severity: 'warning', impact: '+3 points' },
      { text: 'CPU usage is elevated from background indexing', severity: 'info', impact: '+2 points' },
      { text: 'Thermal state is excellent - no action needed', severity: 'good', impact: '' },
      { text: 'Consider disabling startup items with high memory impact', severity: 'warning', impact: '+4 points' }
    ],
    refreshTimer: null
  };

  // ── IPC Helper ─────────────────────────────────────────────────
  function ipcSend(type, payload) {
    if (window.ipc) {
      var msg = Object.assign({ type: type }, payload || {});
      window.ipc.postMessage(JSON.stringify(msg));
    }
  }

  // ── Utility ────────────────────────────────────────────────────
  function getCS(varName) {
    return getComputedStyle(document.documentElement).getPropertyValue(varName).trim();
  }

  function createEl(tag, cls, parent) {
    var el = document.createElement(tag);
    if (cls) el.className = cls;
    if (parent) parent.appendChild(el);
    return el;
  }

  function setText(el, txt) {
    el.textContent = txt;
  }

  function scoreColor(s) {
    if (s >= 80) return '--accent-green';
    if (s >= 60) return '--accent-amber';
    return '--accent-red';
  }

  function scoreGrade(s) {
    if (s >= 95) return 'A+';
    if (s >= 90) return 'A';
    if (s >= 85) return 'A-';
    if (s >= 80) return 'B+';
    if (s >= 75) return 'B';
    if (s >= 70) return 'B-';
    if (s >= 65) return 'C+';
    if (s >= 60) return 'C';
    if (s >= 50) return 'D';
    return 'F';
  }

  // ── Generate simulated history ─────────────────────────────────
  function generateHistory() {
    var hist = [];
    var now = Date.now();
    var base = 75;
    for (var i = 0; i < 96; i++) { // 24h at 15-min intervals
      var noise = Math.sin(i * 0.3) * 8 + Math.random() * 6 - 3;
      var val = Math.max(20, Math.min(100, base + noise + (i * 0.05)));
      hist.push({
        timestamp: now - (96 - i) * 900000,
        score: Math.round(val)
      });
      base = val;
    }
    state.history = hist;
    state.score = hist[hist.length - 1].score;
    state.prevScore = hist[hist.length - 5] ? hist[hist.length - 5].score : state.score;
    state.grade = scoreGrade(state.score);
  }

  // ── Canvas: Circular Gauge ─────────────────────────────────────
  function drawGauge(canvas, score) {
    var ctx = canvas.getContext('2d');
    var dpr = window.devicePixelRatio || 1;
    var size = 180;
    canvas.width = size * dpr;
    canvas.height = size * dpr;
    canvas.style.width = size + 'px';
    canvas.style.height = size + 'px';
    ctx.scale(dpr, dpr);

    var cx = size / 2;
    var cy = size / 2;
    var radius = 70;
    var lineWidth = 10;
    var startAngle = 0.75 * Math.PI;
    var endAngle = 2.25 * Math.PI;
    var totalAngle = endAngle - startAngle;

    // Track
    ctx.beginPath();
    ctx.arc(cx, cy, radius, startAngle, endAngle);
    ctx.strokeStyle = getCS('--gauge-track');
    ctx.lineWidth = lineWidth;
    ctx.lineCap = 'round';
    ctx.stroke();

    // Fill
    var frac = Math.max(0, Math.min(1, score / 100));
    var fillEnd = startAngle + totalAngle * frac;
    var colorVar = scoreColor(score);
    var color = getCS(colorVar);

    // Gradient effect using multiple arcs
    ctx.beginPath();
    ctx.arc(cx, cy, radius, startAngle, fillEnd);
    ctx.strokeStyle = color;
    ctx.lineWidth = lineWidth;
    ctx.lineCap = 'round';
    ctx.stroke();

    // Glow
    ctx.beginPath();
    ctx.arc(cx, cy, radius, startAngle, fillEnd);
    ctx.strokeStyle = color;
    ctx.lineWidth = lineWidth + 4;
    ctx.lineCap = 'round';
    ctx.globalAlpha = 0.15;
    ctx.stroke();
    ctx.globalAlpha = 1;

    // Score text
    ctx.fillStyle = getCS('--text-primary');
    ctx.font = 'bold 36px ' + getCS('--font');
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(score.toString(), cx, cy - 8);

    // Grade
    ctx.fillStyle = color;
    ctx.font = 'bold 16px ' + getCS('--font');
    ctx.fillText(state.grade, cx, cy + 20);

    // Label
    ctx.fillStyle = getCS('--text-dim');
    ctx.font = '9px ' + getCS('--font');
    ctx.fillText('HEALTH SCORE', cx, cy + 38);

    // Tick marks
    ctx.strokeStyle = getCS('--text-dim');
    ctx.lineWidth = 1;
    ctx.globalAlpha = 0.3;
    for (var t = 0; t <= 10; t++) {
      var tickAngle = startAngle + (t / 10) * totalAngle;
      var innerR = radius - lineWidth / 2 - 4;
      var outerR = radius - lineWidth / 2 - (t % 5 === 0 ? 12 : 8);
      ctx.beginPath();
      ctx.moveTo(cx + Math.cos(tickAngle) * innerR, cy + Math.sin(tickAngle) * innerR);
      ctx.lineTo(cx + Math.cos(tickAngle) * outerR, cy + Math.sin(tickAngle) * outerR);
      ctx.stroke();
    }
    ctx.globalAlpha = 1;
  }

  // ── Canvas: History Sparkline ──────────────────────────────────
  function drawHistory(canvas) {
    var ctx = canvas.getContext('2d');
    var dpr = window.devicePixelRatio || 1;
    var rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.scale(dpr, dpr);
    var W = rect.width;
    var H = rect.height;

    ctx.fillStyle = getCS('--bg-primary');
    ctx.fillRect(0, 0, W, H);

    if (!state.history.length) {
      ctx.fillStyle = getCS('--text-dim');
      ctx.font = '11px ' + getCS('--font');
      ctx.textAlign = 'center';
      ctx.fillText('No history data', W / 2, H / 2);
      return;
    }

    var pad = { top: 24, bottom: 28, left: 36, right: 16 };
    var plotW = W - pad.left - pad.right;
    var plotH = H - pad.top - pad.bottom;

    // Scale
    var minScore = 100, maxScore = 0;
    state.history.forEach(function(h) {
      if (h.score < minScore) minScore = h.score;
      if (h.score > maxScore) maxScore = h.score;
    });
    minScore = Math.max(0, minScore - 10);
    maxScore = Math.min(100, maxScore + 10);
    var scoreRange = maxScore - minScore || 1;

    // Grid lines
    ctx.strokeStyle = getCS('--border');
    ctx.lineWidth = 1;
    ctx.setLineDash([3, 3]);
    ctx.fillStyle = getCS('--text-dim');
    ctx.font = '9px ' + getCS('--mono');
    ctx.textAlign = 'right';
    for (var g = 0; g <= 4; g++) {
      var gVal = Math.round(minScore + (g / 4) * scoreRange);
      var gy = pad.top + plotH - (g / 4) * plotH;
      ctx.beginPath();
      ctx.moveTo(pad.left, gy);
      ctx.lineTo(W - pad.right, gy);
      ctx.stroke();
      ctx.fillText(gVal.toString(), pad.left - 6, gy + 3);
    }
    ctx.setLineDash([]);

    // Threshold zones
    var zone80y = pad.top + plotH - ((80 - minScore) / scoreRange) * plotH;
    var zone60y = pad.top + plotH - ((60 - minScore) / scoreRange) * plotH;

    if (zone80y > pad.top && zone80y < pad.top + plotH) {
      ctx.fillStyle = getCS('--accent-green');
      ctx.globalAlpha = 0.04;
      ctx.fillRect(pad.left, pad.top, plotW, zone80y - pad.top);
      ctx.globalAlpha = 1;
    }
    if (zone60y > pad.top && zone60y < pad.top + plotH) {
      ctx.fillStyle = getCS('--accent-amber');
      ctx.globalAlpha = 0.04;
      ctx.fillRect(pad.left, zone80y, plotW, zone60y - zone80y);
      ctx.globalAlpha = 1;
    }

    // Area fill
    ctx.beginPath();
    var pts = [];
    state.history.forEach(function(h, i) {
      var x = pad.left + (i / (state.history.length - 1)) * plotW;
      var y = pad.top + plotH - ((h.score - minScore) / scoreRange) * plotH;
      pts.push({ x: x, y: y });
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });
    // Close area
    ctx.lineTo(pad.left + plotW, pad.top + plotH);
    ctx.lineTo(pad.left, pad.top + plotH);
    ctx.closePath();

    var grad = ctx.createLinearGradient(0, pad.top, 0, pad.top + plotH);
    var lastColor = scoreColor(state.score);
    grad.addColorStop(0, getCS(lastColor));
    grad.addColorStop(1, 'transparent');
    ctx.fillStyle = grad;
    ctx.globalAlpha = 0.15;
    ctx.fill();
    ctx.globalAlpha = 1;

    // Line
    ctx.beginPath();
    pts.forEach(function(p, i) {
      if (i === 0) ctx.moveTo(p.x, p.y);
      else ctx.lineTo(p.x, p.y);
    });
    ctx.strokeStyle = getCS(lastColor);
    ctx.lineWidth = 2;
    ctx.stroke();

    // Current point
    if (pts.length) {
      var last = pts[pts.length - 1];
      ctx.beginPath();
      ctx.arc(last.x, last.y, 4, 0, Math.PI * 2);
      ctx.fillStyle = getCS(lastColor);
      ctx.fill();
      ctx.beginPath();
      ctx.arc(last.x, last.y, 7, 0, Math.PI * 2);
      ctx.strokeStyle = getCS(lastColor);
      ctx.lineWidth = 1;
      ctx.globalAlpha = 0.3;
      ctx.stroke();
      ctx.globalAlpha = 1;
    }

    // Time labels
    ctx.fillStyle = getCS('--text-dim');
    ctx.font = '9px ' + getCS('--mono');
    ctx.textAlign = 'center';
    var firstT = state.history[0].timestamp;
    var lastT = state.history[state.history.length - 1].timestamp;
    var tRange = lastT - firstT || 1;
    for (var ti = 0; ti <= 5; ti++) {
      var tx = pad.left + (ti / 5) * plotW;
      var tt = new Date(firstT + (ti / 5) * tRange);
      ctx.fillText(tt.getHours().toString().padStart(2, '0') + ':' + tt.getMinutes().toString().padStart(2, '0'), tx, H - 8);
    }

    // Title
    ctx.fillStyle = getCS('--text-dim');
    ctx.font = '9px ' + getCS('--font');
    ctx.textAlign = 'left';
    ctx.fillText('HEALTH SCORE HISTORY (24H)', pad.left, 14);
  }

  // ── Canvas: Mini subscore bar chart ────────────────────────────
  function drawSubscoreBars(canvas) {
    var ctx = canvas.getContext('2d');
    var dpr = window.devicePixelRatio || 1;
    var rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.scale(dpr, dpr);
    var W = rect.width;
    var H = rect.height;

    ctx.fillStyle = getCS('--bg-primary');
    ctx.fillRect(0, 0, W, H);

    var pad = { top: 20, bottom: 8, left: 8, right: 8 };
    var barH = 16;
    var gap = 8;
    var maxBarW = W - pad.left - pad.right - 50;

    ctx.fillStyle = getCS('--text-dim');
    ctx.font = '9px ' + getCS('--font');
    ctx.textAlign = 'left';
    ctx.fillText('SUBSCORE COMPARISON', pad.left, 14);

    state.subscores.forEach(function(sub, i) {
      var y = pad.top + i * (barH + gap);
      var barW = (sub.score / 100) * maxBarW;
      var color = getCS(scoreColor(sub.score));

      // Track
      ctx.fillStyle = getCS('--gauge-track');
      ctx.fillRect(pad.left, y, maxBarW, barH);

      // Fill
      ctx.fillStyle = color;
      ctx.globalAlpha = 0.8;
      ctx.fillRect(pad.left, y, barW, barH);
      ctx.globalAlpha = 1;

      // Label
      ctx.fillStyle = getCS('--text-primary');
      ctx.font = '10px ' + getCS('--font');
      ctx.textAlign = 'left';
      ctx.fillText(sub.score.toString(), pad.left + maxBarW + 8, y + barH - 4);
    });
  }

  // ── Render: Gauge Card ─────────────────────────────────────────
  function renderGaugeCard(container) {
    var card = createEl('div', 'card', container);
    card.style.cssText = 'text-align:center;padding:20px;';

    var canvasEl = createEl('canvas', '', card);
    canvasEl.id = 'health-gauge-canvas';

    // Trend indicator
    var trendWrap = createEl('div', '', card);
    trendWrap.style.cssText = 'margin-top:12px;display:flex;align-items:center;justify-content:center;gap:8px;';

    var delta = state.score - state.prevScore;
    var arrow = delta >= 0 ? '\u25B2' : '\u25BC';
    var trendColor = delta >= 0 ? '--accent-green' : '--accent-red';

    var trendEl = createEl('span', '', trendWrap);
    trendEl.style.cssText = 'font-size:14px;color:var(' + trendColor + ');';
    setText(trendEl, arrow);

    var deltaEl = createEl('span', '', trendWrap);
    deltaEl.style.cssText = 'font-size:12px;color:var(' + trendColor + ');font-weight:600;';
    setText(deltaEl, (delta >= 0 ? '+' : '') + delta + ' pts');

    var trendLabel = createEl('span', '', trendWrap);
    trendLabel.style.cssText = 'font-size:10px;color:var(--text-dim);';
    setText(trendLabel, 'vs 1h ago');

    setTimeout(function() { drawGauge(canvasEl, state.score); }, 30);
  }

  // ── Render: Subscores Card ─────────────────────────────────────
  function renderSubscores(container) {
    var card = createEl('div', 'card', container);
    var title = createEl('div', 'card-title', card);
    setText(title, 'Score Breakdown');

    state.subscores.forEach(function(sub) {
      var row = createEl('div', '', card);
      row.style.cssText = 'display:flex;align-items:center;gap:10px;padding:6px 0;border-bottom:1px solid var(--border);';

      var icon = createEl('span', '', row);
      icon.style.fontSize = '14px';
      setText(icon, sub.icon);

      var nameEl = createEl('span', '', row);
      nameEl.style.cssText = 'font-size:11px;color:var(--text-secondary);width:60px;flex-shrink:0;';
      setText(nameEl, sub.name);

      var barWrap = createEl('div', 'ph-bar', row);
      barWrap.style.flex = '1';
      var barFill = createEl('div', 'ph-bar-fill', barWrap);
      barFill.style.width = sub.score + '%';
      barFill.style.background = 'var(' + scoreColor(sub.score) + ')';

      var valEl = createEl('span', '', row);
      valEl.style.cssText = 'font-size:11px;font-family:var(--mono);color:var(--text-primary);width:28px;text-align:right;font-weight:600;';
      setText(valEl, sub.score.toString());

      // Grade letter
      var gradeEl = createEl('span', '', row);
      gradeEl.style.cssText = 'font-size:9px;font-weight:600;padding:1px 6px;border-radius:4px;color:var(' + scoreColor(sub.score) + ');background:var(--bg-primary);';
      setText(gradeEl, scoreGrade(sub.score));
    });
  }

  // ── Render: History Card ───────────────────────────────────────
  function renderHistoryCard(container) {
    var card = createEl('div', 'card', container);
    card.style.gridColumn = '1 / -1';

    var canvasWrap = createEl('div', '', card);
    canvasWrap.style.cssText = 'height:180px;border-radius:8px;overflow:hidden;';
    var canvas = createEl('canvas', '', canvasWrap);
    canvas.id = 'health-history-canvas';
    canvas.style.cssText = 'width:100%;height:100%;display:block;';

    setTimeout(function() { drawHistory(canvas); }, 50);
  }

  // ── Render: Subscore Bar Chart Card ────────────────────────────
  function renderBarChart(container) {
    var card = createEl('div', 'card', container);

    var canvasWrap = createEl('div', '', card);
    canvasWrap.style.cssText = 'height:150px;border-radius:8px;overflow:hidden;';
    var canvas = createEl('canvas', '', canvasWrap);
    canvas.id = 'health-bars-canvas';
    canvas.style.cssText = 'width:100%;height:100%;display:block;';

    setTimeout(function() { drawSubscoreBars(canvas); }, 50);
  }

  // ── Render: Recommendations Card ───────────────────────────────
  function renderRecommendations(container) {
    var card = createEl('div', 'card', container);
    card.style.gridColumn = '1 / -1';
    var title = createEl('div', 'card-title', card);
    setText(title, 'Recommendations');

    state.recommendations.forEach(function(rec) {
      var row = createEl('div', '', card);
      row.style.cssText = 'display:flex;align-items:flex-start;gap:10px;padding:8px 0;border-bottom:1px solid var(--border);';

      var sevColor = rec.severity === 'good' ? '--accent-green' : rec.severity === 'warning' ? '--accent-amber' : '--accent-cyan';
      var sevIcon = rec.severity === 'good' ? '\u2714' : rec.severity === 'warning' ? '\u26A0' : '\u2139';

      var iconEl = createEl('span', '', row);
      iconEl.style.cssText = 'font-size:12px;color:var(' + sevColor + ');flex-shrink:0;margin-top:1px;';
      setText(iconEl, sevIcon);

      var textWrap = createEl('div', '', row);
      textWrap.style.flex = '1';
      var textEl = createEl('div', '', textWrap);
      textEl.style.cssText = 'font-size:11px;color:var(--text-secondary);line-height:1.4;';
      setText(textEl, rec.text);

      if (rec.impact) {
        var impactEl = createEl('span', '', row);
        impactEl.style.cssText = 'font-size:9px;font-weight:600;padding:2px 8px;border-radius:4px;background:var(' + sevColor + '-dim, var(--bg-primary));color:var(' + sevColor + ');flex-shrink:0;white-space:nowrap;';
        setText(impactEl, rec.impact);
      }
    });

    // Actions
    var btnRow = createEl('div', '', card);
    btnRow.style.cssText = 'display:flex;gap:8px;margin-top:12px;';

    var applyBtn = createEl('button', 'btn', btnRow);
    applyBtn.style.cssText = 'padding:8px 20px;background:var(--accent-cyan-dim);border-color:var(--accent-cyan);color:var(--accent-cyan);font-weight:600;';
    setText(applyBtn, 'Apply All Recommendations');
    applyBtn.addEventListener('click', function() {
      ipcSend('apply_health_recommendations');
      if (typeof showToast === 'function') {
        showToast('Applying Fixes', 'Running optimization recommendations...', 'success');
      }
    });

    var refreshBtn = createEl('button', 'btn', btnRow);
    setText(refreshBtn, 'Refresh Score');
    refreshBtn.addEventListener('click', function() {
      ipcSend('get_health_score');
      ipcSend('get_health_history');
      ipcSend('get_health_recommendations');
    });
  }

  // ── Full Render ────────────────────────────────────────────────
  function render() {
    var container = document.getElementById('page-health');
    if (!container) return;

    container.innerHTML = '';
    var inner = createEl('div', 'page-inner', container);
    inner.style.overflowY = 'auto';
    inner.style.height = '100%';

    // Header
    var header = createEl('div', 'page-header', inner);
    var icon = createEl('span', 'page-icon', header);
    setText(icon, '\u{1F49A}');
    var h2 = createEl('h2', '', header);
    setText(h2, 'System Health Score');

    var statusBadge = createEl('div', '', inner);
    statusBadge.style.cssText = 'display:inline-block;font-size:9px;font-weight:600;letter-spacing:1px;text-transform:uppercase;padding:3px 10px;border-radius:4px;margin-bottom:16px;background:var(--accent-cyan-dim);color:var(--accent-cyan);';
    setText(statusBadge, 'ADR-014 \u00B7 Active');

    var desc = createEl('p', 'page-desc', inner);
    setText(desc, 'A composite health score across memory, CPU, disk I/O, thermal, and process dimensions. Grades from A+ to F with actionable recommendations to improve your system health.');

    // Auto-refresh indicator
    var refreshNote = createEl('div', '', inner);
    refreshNote.style.cssText = 'font-size:10px;color:var(--text-dim);margin-bottom:16px;display:flex;align-items:center;gap:6px;';
    var pulseEl = createEl('span', '', refreshNote);
    pulseEl.style.cssText = 'width:6px;height:6px;border-radius:50%;background:var(--accent-green);display:inline-block;animation:labelPulse 2s ease-in-out infinite;';
    setText(refreshNote, '');
    refreshNote.appendChild(pulseEl);
    var refreshText = document.createTextNode(' Auto-refreshing every 5 seconds');
    refreshNote.appendChild(refreshText);

    // Grid
    var grid = createEl('div', 'page-grid', inner);

    renderGaugeCard(grid);
    renderSubscores(grid);
    renderBarChart(grid);
    renderHistoryCard(grid);
    renderRecommendations(grid);
  }

  // ── Init ───────────────────────────────────────────────────────
  window.RuVectorPages.healthInit = function(container) {
    generateHistory();
    render();
    // Request data
    ipcSend('get_health_score');
    ipcSend('get_health_history');
    ipcSend('get_health_recommendations');
    // Auto-refresh
    if (state.refreshTimer) clearInterval(state.refreshTimer);
    state.refreshTimer = setInterval(function() {
      ipcSend('get_health_score');
    }, 5000);
  };

  // ── Update (called from Rust IPC) ──────────────────────────────
  window.RuVectorPages.healthUpdate = function(data) {
    if (!data) return;
    if (typeof data.score === 'number') {
      state.prevScore = state.score;
      state.score = data.score;
      state.grade = scoreGrade(state.score);
    }
    if (data.subscores && Array.isArray(data.subscores)) {
      state.subscores = data.subscores;
    }
    if (data.history && Array.isArray(data.history)) {
      state.history = data.history;
    }
    if (data.recommendations && Array.isArray(data.recommendations)) {
      state.recommendations = data.recommendations;
    }
    render();
  };

  // ── Cleanup on page switch ─────────────────────────────────────
  var origShowPage = window.showPage;
  if (typeof origShowPage === 'function') {
    window.showPage = function(id) {
      origShowPage(id);
      if (id !== 'health' && state.refreshTimer) {
        clearInterval(state.refreshTimer);
        state.refreshTimer = null;
      } else if (id === 'health' && !state.refreshTimer) {
        ipcSend('get_health_score');
        state.refreshTimer = setInterval(function() {
          ipcSend('get_health_score');
        }, 5000);
      }
    };
  }

  // ── Resize handler ─────────────────────────────────────────────
  window.addEventListener('resize', function() {
    var hCanvas = document.getElementById('health-history-canvas');
    if (hCanvas && hCanvas.offsetParent) drawHistory(hCanvas);
    var bCanvas = document.getElementById('health-bars-canvas');
    if (bCanvas && bCanvas.offsetParent) drawSubscoreBars(bCanvas);
    var gCanvas = document.getElementById('health-gauge-canvas');
    if (gCanvas && gCanvas.offsetParent) drawGauge(gCanvas, state.score);
  });

})();
