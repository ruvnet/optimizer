/* ================================================================
   ADR-024: Time-Travel System State
   Self-contained IIFE component for RuVector Control Center
   Canvas 2D health score timeline chart with interactive hover
   ================================================================ */
(function(){
  'use strict';

  window.RuVectorPages = window.RuVectorPages || {};

  // ── State ──────────────────────────────────────────────────────
  var state = {
    initialized: false,
    currentScore: 82,
    bestToday: 94,
    checkpoints: [],
    healthHistory: [],
    selectedA: null,
    selectedB: null,
    diffResult: null,
    diagnosis: [],
    rollbackOptions: { services: true, startup: true, processes: false, powerPlan: false },
    showRollbackDialog: false,
    showCreateDialog: false,
    newCheckpointName: '',
    snapshotConfig: { interval_min: 15, on_health_drop: true, on_windows_update: true, on_driver_install: false },
    hoverIndex: -1
  };

  // ── Mock data ─────────────────────────────────────────────────
  var MOCK_HISTORY = (function() {
    var pts = [];
    var base = 85;
    var now = Date.now();
    for (var i = 0; i < 96; i++) {
      var t = now - (95 - i) * 15 * 60 * 1000;
      var drift = Math.sin(i * 0.15) * 8 + Math.sin(i * 0.04) * 5;
      var spike = (i === 40 || i === 65) ? -15 : 0;
      var score = Math.max(30, Math.min(100, Math.round(base + drift + spike + (Math.random() - 0.5) * 3)));
      pts.push({ time: t, score: score });
    }
    return pts;
  })();

  var MOCK_CHECKPOINTS = [
    { id: 'cp-1', name: 'Before Windows Update', time: Date.now() - 3600000 * 4, score: 91 },
    { id: 'cp-2', name: 'Post-Cleanup Baseline', time: Date.now() - 3600000 * 8, score: 94 },
    { id: 'cp-3', name: 'Morning Boot', time: Date.now() - 3600000 * 12, score: 88 },
    { id: 'cp-4', name: 'After Driver Install', time: Date.now() - 3600000 * 18, score: 76 }
  ];

  var MOCK_DIFF = {
    processes_added: [
      { name: 'SearchIndexer.exe', mem_mb: 145 },
      { name: 'RuntimeBroker.exe', mem_mb: 67 }
    ],
    processes_removed: [
      { name: 'OldService.exe', mem_mb: 34 }
    ],
    services_changed: [
      { name: 'Windows Update Medic', from: 'Manual', to: 'Automatic' },
      { name: 'Superfetch', from: 'Disabled', to: 'Automatic' }
    ],
    startup_added: [
      { name: 'Discord Update', impact: 'medium' }
    ],
    startup_removed: [],
    updates_installed: [
      { name: 'KB5034441 - Security Update', date: '2024-01-28' }
    ],
    env_changes: [
      { var: 'PATH', action: 'appended', value: 'C:\\Program Files\\NewApp' }
    ],
    memory_change_mb: -312
  };

  var MOCK_DIAGNOSIS = [
    { cause: 'Windows Search Indexer consuming excessive RAM', impact: 85, category: 'Process' },
    { cause: 'Superfetch re-enabled after Windows Update', impact: 72, category: 'Service' },
    { cause: 'New startup item added: Discord Update', impact: 45, category: 'Startup' },
    { cause: 'PATH environment variable grew by 1 entry', impact: 12, category: 'Environment' }
  ];

  // ── IPC helper ─────────────────────────────────────────────────
  function ipcSend(type, payload) {
    if (window.ipc) {
      var msg = Object.assign({ type: type }, payload || {});
      window.ipc.postMessage(JSON.stringify(msg));
    }
  }

  // ── DOM helper ────────────────────────────────────────────────
  function el(tag, attrs, children) {
    var e = document.createElement(tag);
    if (attrs) {
      Object.keys(attrs).forEach(function(k) {
        if (k === 'className') e.className = attrs[k];
        else if (k === 'style' && typeof attrs[k] === 'object') {
          Object.keys(attrs[k]).forEach(function(s) { e.style[s] = attrs[k][s]; });
        }
        else if (k.indexOf('on') === 0 && typeof attrs[k] === 'function') e.addEventListener(k.slice(2).toLowerCase(), attrs[k]);
        else if (k === 'textContent') e.textContent = attrs[k];
        else if (k === 'innerHTML') e.innerHTML = attrs[k];
        else e.setAttribute(k, attrs[k]);
      });
    }
    if (children) {
      (Array.isArray(children) ? children : [children]).forEach(function(c) {
        if (typeof c === 'string') e.appendChild(document.createTextNode(c));
        else if (c) e.appendChild(c);
      });
    }
    return e;
  }

  function formatTime(ts) {
    var d = new Date(ts);
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  }

  function formatDate(ts) {
    var d = new Date(ts);
    return d.toLocaleDateString([], { month: 'short', day: 'numeric' }) + ' ' + formatTime(ts);
  }

  function scoreColor(score) {
    if (score >= 85) return 'var(--accent-green)';
    if (score >= 65) return 'var(--accent-amber)';
    return 'var(--accent-red)';
  }

  // ── Canvas Chart ──────────────────────────────────────────────
  var chartCanvas = null;
  var chartCtx = null;
  var chartRect = null;

  function getComputedVar(name) {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }

  function drawChart() {
    if (!chartCanvas || !chartCtx) return;
    var history = state.healthHistory;
    if (history.length < 2) return;

    var dpr = window.devicePixelRatio || 1;
    var w = chartCanvas.clientWidth;
    var h = chartCanvas.clientHeight;
    chartCanvas.width = w * dpr;
    chartCanvas.height = h * dpr;
    var ctx = chartCtx;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    var pad = { top: 20, right: 16, bottom: 30, left: 36 };
    var cw = w - pad.left - pad.right;
    var ch = h - pad.top - pad.bottom;

    // Clear
    ctx.clearRect(0, 0, w, h);

    // Grid lines
    var textDim = getComputedVar('--text-dim') || '#505872';
    var borderColor = getComputedVar('--border') || '#252d45';
    ctx.strokeStyle = borderColor;
    ctx.lineWidth = 0.5;
    ctx.font = '9px ' + (getComputedVar('--mono') || 'Consolas');
    ctx.fillStyle = textDim;
    ctx.textAlign = 'right';
    ctx.textBaseline = 'middle';

    for (var g = 0; g <= 4; g++) {
      var val = 100 - g * 25;
      var gy = pad.top + (g / 4) * ch;
      ctx.beginPath();
      ctx.moveTo(pad.left, gy);
      ctx.lineTo(pad.left + cw, gy);
      ctx.stroke();
      ctx.fillText(String(val), pad.left - 6, gy);
    }

    // Time labels
    ctx.textAlign = 'center';
    ctx.textBaseline = 'top';
    var labelCount = 6;
    for (var l = 0; l <= labelCount; l++) {
      var idx = Math.floor(l / labelCount * (history.length - 1));
      var lx = pad.left + (idx / (history.length - 1)) * cw;
      ctx.fillText(formatTime(history[idx].time), lx, pad.top + ch + 8);
    }

    // Build path
    var points = history.map(function(p, i) {
      return {
        x: pad.left + (i / (history.length - 1)) * cw,
        y: pad.top + (1 - (p.score - 25) / 75) * ch
      };
    });

    // Gradient fill
    var gradient = ctx.createLinearGradient(0, pad.top, 0, pad.top + ch);
    var cyanColor = getComputedVar('--accent-cyan') || '#4ecdc4';
    gradient.addColorStop(0, cyanColor + '40');
    gradient.addColorStop(1, cyanColor + '05');

    ctx.beginPath();
    ctx.moveTo(points[0].x, pad.top + ch);
    points.forEach(function(p) { ctx.lineTo(p.x, p.y); });
    ctx.lineTo(points[points.length - 1].x, pad.top + ch);
    ctx.closePath();
    ctx.fillStyle = gradient;
    ctx.fill();

    // Line
    ctx.beginPath();
    ctx.moveTo(points[0].x, points[0].y);
    for (var i = 1; i < points.length; i++) {
      ctx.lineTo(points[i].x, points[i].y);
    }
    ctx.strokeStyle = cyanColor;
    ctx.lineWidth = 2;
    ctx.lineJoin = 'round';
    ctx.stroke();

    // Threshold lines
    var thresholds = [
      { val: 85, color: getComputedVar('--accent-green') || '#6bcb77', label: 'Good' },
      { val: 65, color: getComputedVar('--accent-amber') || '#d4a574', label: 'Fair' }
    ];
    thresholds.forEach(function(th) {
      var ty = pad.top + (1 - (th.val - 25) / 75) * ch;
      ctx.setLineDash([4, 4]);
      ctx.strokeStyle = th.color + '60';
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(pad.left, ty);
      ctx.lineTo(pad.left + cw, ty);
      ctx.stroke();
      ctx.setLineDash([]);
    });

    // Hover indicator
    if (state.hoverIndex >= 0 && state.hoverIndex < points.length) {
      var hp = points[state.hoverIndex];
      var hd = history[state.hoverIndex];

      // Vertical line
      ctx.strokeStyle = textDim;
      ctx.lineWidth = 0.5;
      ctx.setLineDash([2, 2]);
      ctx.beginPath();
      ctx.moveTo(hp.x, pad.top);
      ctx.lineTo(hp.x, pad.top + ch);
      ctx.stroke();
      ctx.setLineDash([]);

      // Dot
      ctx.beginPath();
      ctx.arc(hp.x, hp.y, 5, 0, Math.PI * 2);
      ctx.fillStyle = cyanColor;
      ctx.fill();
      ctx.strokeStyle = getComputedVar('--bg-card') || '#171d30';
      ctx.lineWidth = 2;
      ctx.stroke();

      // Tooltip
      var tooltipText = formatTime(hd.time) + '  Score: ' + hd.score;
      ctx.font = '10px ' + (getComputedVar('--font') || 'Segoe UI');
      var tw = ctx.measureText(tooltipText).width + 16;
      var tx = Math.min(Math.max(hp.x - tw / 2, pad.left), w - pad.right - tw);
      var ty2 = hp.y - 28;
      if (ty2 < pad.top) ty2 = hp.y + 12;

      ctx.fillStyle = getComputedVar('--bg-card') || '#171d30';
      ctx.strokeStyle = borderColor;
      ctx.lineWidth = 1;
      roundRect(ctx, tx, ty2, tw, 20, 4);
      ctx.fill();
      ctx.stroke();

      ctx.fillStyle = getComputedVar('--text-primary') || '#e0e4f0';
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(tooltipText, tx + tw / 2, ty2 + 10);
    }

    // Checkpoint markers
    state.checkpoints.forEach(function(cp) {
      var cpTime = cp.time;
      var minTime = history[0].time;
      var maxTime = history[history.length - 1].time;
      if (cpTime < minTime || cpTime > maxTime) return;
      var ratio = (cpTime - minTime) / (maxTime - minTime);
      var cx = pad.left + ratio * cw;

      ctx.strokeStyle = getComputedVar('--accent-purple') || '#a78bfa';
      ctx.lineWidth = 1;
      ctx.setLineDash([3, 3]);
      ctx.beginPath();
      ctx.moveTo(cx, pad.top);
      ctx.lineTo(cx, pad.top + ch);
      ctx.stroke();
      ctx.setLineDash([]);

      // Diamond marker
      ctx.fillStyle = getComputedVar('--accent-purple') || '#a78bfa';
      ctx.beginPath();
      ctx.moveTo(cx, pad.top - 2);
      ctx.lineTo(cx + 5, pad.top + 5);
      ctx.lineTo(cx, pad.top + 12);
      ctx.lineTo(cx - 5, pad.top + 5);
      ctx.closePath();
      ctx.fill();
    });
  }

  function roundRect(ctx, x, y, w, h, r) {
    ctx.beginPath();
    ctx.moveTo(x + r, y);
    ctx.lineTo(x + w - r, y);
    ctx.quadraticCurveTo(x + w, y, x + w, y + r);
    ctx.lineTo(x + w, y + h - r);
    ctx.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
    ctx.lineTo(x + r, y + h);
    ctx.quadraticCurveTo(x, y + h, x, y + h - r);
    ctx.lineTo(x, y + r);
    ctx.quadraticCurveTo(x, y, x + r, y);
    ctx.closePath();
  }

  function handleChartMouse(e) {
    if (!chartCanvas || !state.healthHistory.length) return;
    var rect = chartCanvas.getBoundingClientRect();
    var mx = e.clientX - rect.left;
    var pad = { left: 36, right: 16 };
    var cw = chartCanvas.clientWidth - pad.left - pad.right;
    var ratio = (mx - pad.left) / cw;
    ratio = Math.max(0, Math.min(1, ratio));
    var idx = Math.round(ratio * (state.healthHistory.length - 1));
    if (idx !== state.hoverIndex) {
      state.hoverIndex = idx;
      drawChart();
    }
  }

  function handleChartLeave() {
    state.hoverIndex = -1;
    drawChart();
  }

  // ── Render: health score cards ────────────────────────────────
  function renderScoreCards(container) {
    var row = el('div', { style: { display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px', marginBottom: '16px' } });

    // Current score
    var card1 = el('div', { className: 'ph-card', style: { textAlign: 'center' } });
    card1.appendChild(el('div', { className: 'ph-card-title', textContent: 'Current Health Score' }));
    var scoreDisplay = el('div', { style: { display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '12px' } });
    scoreDisplay.appendChild(el('div', {
      textContent: String(state.currentScore),
      style: { fontSize: '48px', fontWeight: '700', color: scoreColor(state.currentScore), lineHeight: '1' }
    }));
    scoreDisplay.appendChild(el('div', { textContent: '/ 100', style: { fontSize: '14px', color: 'var(--text-dim)' } }));
    card1.appendChild(scoreDisplay);
    card1.appendChild(el('div', { textContent: state.currentScore >= 85 ? 'Healthy' : state.currentScore >= 65 ? 'Fair' : 'Degraded', style: { fontSize: '11px', color: scoreColor(state.currentScore), marginTop: '4px', fontWeight: '600' } }));
    row.appendChild(card1);

    // Best today
    var card2 = el('div', { className: 'ph-card', style: { textAlign: 'center' } });
    card2.appendChild(el('div', { className: 'ph-card-title', textContent: 'Best Score Today' }));
    var bestDisplay = el('div', { style: { display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '12px' } });
    bestDisplay.appendChild(el('div', {
      textContent: String(state.bestToday),
      style: { fontSize: '48px', fontWeight: '700', color: scoreColor(state.bestToday), lineHeight: '1' }
    }));
    bestDisplay.appendChild(el('div', { textContent: '/ 100', style: { fontSize: '14px', color: 'var(--text-dim)' } }));
    card2.appendChild(bestDisplay);
    var delta = state.currentScore - state.bestToday;
    card2.appendChild(el('div', {
      textContent: (delta >= 0 ? '+' : '') + delta + ' from best',
      style: { fontSize: '11px', color: delta >= 0 ? 'var(--accent-green)' : 'var(--accent-red)', marginTop: '4px', fontWeight: '600' }
    }));
    row.appendChild(card2);
    container.appendChild(row);
  }

  // ── Render: timeline chart ────────────────────────────────────
  function renderChart(container) {
    var card = el('div', { className: 'ph-card', style: { marginBottom: '16px' } });
    card.appendChild(el('div', { className: 'ph-card-title', textContent: 'Health Score Timeline (24 Hours)' }));

    chartCanvas = el('canvas', {
      style: { width: '100%', height: '200px', display: 'block', borderRadius: '8px', cursor: 'crosshair' }
    });
    chartCanvas.addEventListener('mousemove', handleChartMouse);
    chartCanvas.addEventListener('mouseleave', handleChartLeave);
    card.appendChild(chartCanvas);
    container.appendChild(card);

    chartCtx = chartCanvas.getContext('2d');
    requestAnimationFrame(drawChart);
  }

  // ── Render: checkpoints list ──────────────────────────────────
  function renderCheckpoints(container) {
    var card = el('div', { className: 'ph-card', style: { marginBottom: '16px' } });
    var header = el('div', { style: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' } });
    header.appendChild(el('div', { className: 'ph-card-title', textContent: 'Named Checkpoints', style: { margin: '0' } }));

    var createBtn = el('button', {
      className: 'btn',
      style: { padding: '4px 12px', fontSize: '10px', cursor: 'pointer' },
      textContent: '+ Create Checkpoint',
      onClick: function() { state.showCreateDialog = true; rerender(); }
    });
    header.appendChild(createBtn);
    card.appendChild(header);

    if (state.checkpoints.length === 0) {
      card.appendChild(el('div', { textContent: 'No checkpoints saved. Create one to mark the current system state.', style: { fontSize: '11px', color: 'var(--text-dim)', padding: '12px 0' } }));
    } else {
      state.checkpoints.forEach(function(cp) {
        var row = el('div', {
          style: {
            display: 'grid', gridTemplateColumns: '1fr 130px 50px 140px',
            gap: '10px', padding: '8px 0', borderBottom: '1px solid var(--border)',
            alignItems: 'center', fontSize: '11px'
          }
        });
        row.appendChild(el('div', { style: { color: 'var(--text-primary)', fontWeight: '500' } }, [
          el('div', { textContent: cp.name }),
          el('div', { textContent: formatDate(cp.time), style: { fontSize: '9px', color: 'var(--text-dim)', fontFamily: 'var(--mono)', marginTop: '2px' } })
        ]));

        row.appendChild(el('span', { textContent: formatDate(cp.time), style: { fontSize: '9px', fontFamily: 'var(--mono)', color: 'var(--text-secondary)', display: 'none' } }));

        row.appendChild(el('span', { textContent: String(cp.score), style: { fontFamily: 'var(--mono)', fontWeight: '600', color: scoreColor(cp.score) } }));

        var actions = el('div', { style: { display: 'flex', gap: '4px' } });
        var isA = state.selectedA === cp.id;
        var isB = state.selectedB === cp.id;
        actions.appendChild(el('button', {
          className: 'btn',
          style: {
            padding: '2px 8px', fontSize: '9px', cursor: 'pointer',
            background: isA ? 'var(--accent-cyan-dim)' : '',
            borderColor: isA ? 'var(--accent-cyan)' : '',
            color: isA ? 'var(--accent-cyan)' : ''
          },
          textContent: isA ? 'Selected A' : 'Compare',
          onClick: (function(id) { return function() { selectForCompare(id); }; })(cp.id)
        }));
        actions.appendChild(el('button', {
          className: 'btn',
          style: { padding: '2px 8px', fontSize: '9px', cursor: 'pointer', borderColor: 'var(--accent-amber)', color: 'var(--accent-amber)' },
          textContent: 'Restore',
          onClick: (function(id) { return function() { state.showRollbackDialog = true; state.selectedA = id; rerender(); }; })(cp.id)
        }));
        row.appendChild(actions);
        card.appendChild(row);
      });
    }

    container.appendChild(card);
  }

  function selectForCompare(id) {
    if (!state.selectedA || state.selectedA === id) {
      state.selectedA = id;
    } else {
      state.selectedB = id;
      // Trigger comparison
      ipcSend('compare_snapshots', { a: state.selectedA, b: state.selectedB });
      state.diffResult = MOCK_DIFF;
      state.diagnosis = MOCK_DIAGNOSIS;
    }
    rerender();
  }

  // ── Render: diff view ─────────────────────────────────────────
  function renderDiffView(container) {
    if (!state.diffResult) return;
    var diff = state.diffResult;

    var card = el('div', { className: 'ph-card', style: { marginBottom: '16px' } });
    var header = el('div', { style: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' } });
    header.appendChild(el('div', { className: 'ph-card-title', textContent: 'State Diff', style: { margin: '0' } }));
    header.appendChild(el('button', {
      className: 'btn',
      style: { padding: '2px 8px', fontSize: '9px', cursor: 'pointer' },
      textContent: 'Clear Comparison',
      onClick: function() { state.selectedA = null; state.selectedB = null; state.diffResult = null; state.diagnosis = []; rerender(); }
    }));
    card.appendChild(header);

    // Memory change summary
    var memChange = diff.memory_change_mb;
    card.appendChild(el('div', {
      style: { fontSize: '12px', marginBottom: '12px', padding: '8px', borderRadius: '6px', background: memChange < 0 ? 'rgba(224,96,96,0.1)' : 'rgba(107,203,119,0.1)' }
    }, [
      el('span', { textContent: 'Available Memory Change: ', style: { color: 'var(--text-secondary)' } }),
      el('strong', { textContent: (memChange >= 0 ? '+' : '') + memChange + ' MB', style: { color: memChange < 0 ? 'var(--accent-red)' : 'var(--accent-green)' } })
    ]));

    var sections = [
      { title: 'Processes Added', items: diff.processes_added, render: function(i) { return i.name + ' (+' + i.mem_mb + ' MB)'; }, color: 'var(--accent-red)' },
      { title: 'Processes Removed', items: diff.processes_removed, render: function(i) { return i.name + ' (-' + i.mem_mb + ' MB)'; }, color: 'var(--accent-green)' },
      { title: 'Services Changed', items: diff.services_changed, render: function(i) { return i.name + ': ' + i.from + ' \u2192 ' + i.to; }, color: 'var(--accent-amber)' },
      { title: 'Startup Items Added', items: diff.startup_added, render: function(i) { return i.name + ' (impact: ' + i.impact + ')'; }, color: 'var(--accent-red)' },
      { title: 'Windows Updates Installed', items: diff.updates_installed, render: function(i) { return i.name; }, color: 'var(--accent-purple)' },
      { title: 'Environment Variables Changed', items: diff.env_changes, render: function(i) { return i.var + ' ' + i.action + ': ' + i.value; }, color: 'var(--text-secondary)' }
    ];

    sections.forEach(function(sec) {
      if (!sec.items || sec.items.length === 0) return;
      card.appendChild(el('div', { textContent: sec.title + ' (' + sec.items.length + ')', style: { fontSize: '10px', fontWeight: '600', color: sec.color, textTransform: 'uppercase', letterSpacing: '0.5px', marginTop: '10px', marginBottom: '4px' } }));
      sec.items.forEach(function(item) {
        var row = el('div', { style: { display: 'flex', alignItems: 'center', gap: '6px', padding: '4px 0', fontSize: '11px', color: 'var(--text-secondary)', borderBottom: '1px solid var(--border)' } });
        row.appendChild(el('span', { style: { width: '6px', height: '6px', borderRadius: '50%', background: sec.color, flexShrink: '0' } }));
        row.appendChild(el('span', { textContent: sec.render(item) }));
        card.appendChild(row);
      });
    });

    container.appendChild(card);
  }

  // ── Render: diagnosis panel ───────────────────────────────────
  function renderDiagnosis(container) {
    if (state.diagnosis.length === 0) return;
    var card = el('div', { className: 'ph-card', style: { marginBottom: '16px' } });
    card.appendChild(el('div', { className: 'ph-card-title', textContent: 'Degradation Diagnosis' }));

    state.diagnosis.forEach(function(d) {
      var row = el('div', { style: { display: 'grid', gridTemplateColumns: '1fr 80px 70px', gap: '8px', padding: '6px 0', borderBottom: '1px solid var(--border)', alignItems: 'center', fontSize: '11px' } });
      row.appendChild(el('span', { textContent: d.cause, style: { color: 'var(--text-primary)' } }));

      // Impact bar
      var barWrapper = el('div', { style: { display: 'flex', alignItems: 'center', gap: '4px' } });
      var bar = el('div', { style: { flex: '1', height: '6px', background: 'var(--gauge-track)', borderRadius: '3px', overflow: 'hidden' } });
      var impactColor = d.impact > 70 ? 'var(--accent-red)' : d.impact > 40 ? 'var(--accent-amber)' : 'var(--accent-cyan)';
      bar.appendChild(el('div', { style: { width: d.impact + '%', height: '100%', background: impactColor, borderRadius: '3px' } }));
      barWrapper.appendChild(bar);
      row.appendChild(barWrapper);

      row.appendChild(el('span', {
        textContent: d.category,
        style: { fontSize: '9px', fontWeight: '600', padding: '2px 6px', borderRadius: '4px', background: 'var(--bg-primary)', color: 'var(--text-dim)', textAlign: 'center' }
      }));
      card.appendChild(row);
    });

    container.appendChild(card);
  }

  // ── Render: rollback dialog ───────────────────────────────────
  function renderRollbackDialog(container) {
    if (!state.showRollbackDialog) return;

    var overlay = el('div', {
      style: {
        position: 'fixed', inset: '0', background: 'rgba(0,0,0,0.5)', zIndex: '50',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        backdropFilter: 'blur(4px)'
      },
      onClick: function(e) { if (e.target === overlay) { state.showRollbackDialog = false; rerender(); } }
    });

    var dialog = el('div', {
      style: {
        background: 'var(--bg-card)', border: '1px solid var(--border)', borderRadius: '12px',
        padding: '24px', maxWidth: '420px', width: '90%', boxShadow: 'var(--shadow)'
      }
    });

    dialog.appendChild(el('h3', { textContent: 'Rollback Confirmation', style: { fontSize: '16px', fontWeight: '600', color: 'var(--text-primary)', marginBottom: '8px' } }));
    dialog.appendChild(el('p', { textContent: 'Select which components to restore to the checkpoint state. This operation cannot be undone.', style: { fontSize: '11px', color: 'var(--text-secondary)', lineHeight: '1.5', marginBottom: '16px' } }));

    var options = [
      { key: 'services', label: 'Services (start type, state)' },
      { key: 'startup', label: 'Startup Items' },
      { key: 'processes', label: 'Running Processes' },
      { key: 'powerPlan', label: 'Power Plan Settings' }
    ];

    options.forEach(function(opt) {
      var checked = state.rollbackOptions[opt.key];
      var row = el('div', {
        style: { display: 'flex', alignItems: 'center', gap: '10px', padding: '8px 0', cursor: 'pointer', fontSize: '12px', color: 'var(--text-primary)' },
        onClick: function() { state.rollbackOptions[opt.key] = !state.rollbackOptions[opt.key]; rerender(); }
      });

      var checkbox = el('div', {
        style: {
          width: '16px', height: '16px', borderRadius: '4px', flexShrink: '0',
          border: '2px solid ' + (checked ? 'var(--accent-cyan)' : 'var(--border)'),
          background: checked ? 'var(--accent-cyan)' : 'transparent',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          fontSize: '10px', color: '#fff', fontWeight: '700'
        }
      });
      if (checked) checkbox.textContent = '\u2713';
      row.appendChild(checkbox);
      row.appendChild(el('span', { textContent: opt.label }));
      dialog.appendChild(row);
    });

    var btnRow = el('div', { style: { display: 'flex', gap: '8px', marginTop: '20px', justifyContent: 'flex-end' } });
    btnRow.appendChild(el('button', {
      className: 'btn',
      style: { padding: '8px 16px', fontSize: '11px', cursor: 'pointer' },
      textContent: 'Cancel',
      onClick: function() { state.showRollbackDialog = false; rerender(); }
    }));
    btnRow.appendChild(el('button', {
      className: 'btn',
      style: { padding: '8px 16px', fontSize: '11px', cursor: 'pointer', background: 'var(--accent-amber-dim)', borderColor: 'var(--accent-amber)', color: 'var(--accent-amber)', fontWeight: '600' },
      textContent: 'Rollback Now',
      onClick: function() {
        ipcSend('rollback_to', { checkpoint: state.selectedA, options: state.rollbackOptions });
        state.showRollbackDialog = false;
        rerender();
        if (typeof showToast === 'function') showToast('Rollback Started', 'Restoring system state from checkpoint...', 'success');
      }
    }));
    dialog.appendChild(btnRow);
    overlay.appendChild(dialog);
    container.appendChild(overlay);
  }

  // ── Render: create checkpoint dialog ──────────────────────────
  function renderCreateDialog(container) {
    if (!state.showCreateDialog) return;

    var overlay = el('div', {
      style: {
        position: 'fixed', inset: '0', background: 'rgba(0,0,0,0.5)', zIndex: '50',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        backdropFilter: 'blur(4px)'
      },
      onClick: function(e) { if (e.target === overlay) { state.showCreateDialog = false; rerender(); } }
    });

    var dialog = el('div', {
      style: {
        background: 'var(--bg-card)', border: '1px solid var(--border)', borderRadius: '12px',
        padding: '24px', maxWidth: '380px', width: '90%', boxShadow: 'var(--shadow)'
      }
    });

    dialog.appendChild(el('h3', { textContent: 'Create Checkpoint', style: { fontSize: '16px', fontWeight: '600', color: 'var(--text-primary)', marginBottom: '12px' } }));

    var input = el('input', {
      type: 'text',
      placeholder: 'Checkpoint name (e.g. "Before update")',
      style: {
        width: '100%', padding: '8px 12px', fontSize: '12px',
        background: 'var(--bg-primary)', color: 'var(--text-primary)',
        border: '1px solid var(--border)', borderRadius: '6px',
        fontFamily: 'var(--font)', marginBottom: '16px'
      }
    });
    input.addEventListener('input', function() { state.newCheckpointName = input.value; });
    dialog.appendChild(input);

    var btnRow = el('div', { style: { display: 'flex', gap: '8px', justifyContent: 'flex-end' } });
    btnRow.appendChild(el('button', {
      className: 'btn',
      style: { padding: '8px 16px', fontSize: '11px', cursor: 'pointer' },
      textContent: 'Cancel',
      onClick: function() { state.showCreateDialog = false; rerender(); }
    }));
    btnRow.appendChild(el('button', {
      className: 'btn',
      style: { padding: '8px 16px', fontSize: '11px', cursor: 'pointer', background: 'var(--accent-cyan-dim)', borderColor: 'var(--accent-cyan)', color: 'var(--accent-cyan)', fontWeight: '600' },
      textContent: 'Create',
      onClick: function() {
        var name = state.newCheckpointName || 'Checkpoint ' + (state.checkpoints.length + 1);
        ipcSend('create_checkpoint', { name: name });
        state.checkpoints.unshift({ id: 'cp-new-' + Date.now(), name: name, time: Date.now(), score: state.currentScore });
        state.showCreateDialog = false;
        state.newCheckpointName = '';
        rerender();
        if (typeof showToast === 'function') showToast('Checkpoint Created', 'Saved: ' + name, 'success');
      }
    }));
    dialog.appendChild(btnRow);
    overlay.appendChild(dialog);
    container.appendChild(overlay);
  }

  // ── Render: auto-snapshot settings ────────────────────────────
  function renderSnapshotConfig(container) {
    var card = el('div', { className: 'ph-card' });
    card.appendChild(el('div', { className: 'ph-card-title', textContent: 'Auto-Snapshot Settings' }));

    var cfg = state.snapshotConfig;

    // Interval
    var intervalRow = el('div', { className: 'select-row', style: { padding: '6px 0' } });
    intervalRow.appendChild(el('span', { textContent: 'Snapshot Interval', style: { fontSize: '11px', color: 'var(--text-secondary)' } }));
    var sel = el('select', {
      style: {
        background: 'var(--bg-primary)', color: 'var(--text-primary)',
        border: '1px solid var(--border)', borderRadius: '4px',
        padding: '2px 6px', fontSize: '11px', fontFamily: 'var(--font)', cursor: 'pointer'
      },
      onChange: function() { cfg.interval_min = parseInt(sel.value); ipcSend('set_snapshot_config', cfg); }
    });
    [5, 10, 15, 30, 60].forEach(function(v) {
      var opt = el('option', { value: String(v), textContent: v + ' min' });
      if (v === cfg.interval_min) opt.selected = true;
      sel.appendChild(opt);
    });
    intervalRow.appendChild(sel);
    card.appendChild(intervalRow);

    // Toggle triggers
    var triggers = [
      { key: 'on_health_drop', label: 'On Health Score Drop' },
      { key: 'on_windows_update', label: 'After Windows Update' },
      { key: 'on_driver_install', label: 'After Driver Install' }
    ];

    triggers.forEach(function(trig) {
      var row = el('div', { className: 'toggle-row' });
      row.appendChild(el('span', { textContent: trig.label }));
      var toggle = el('div', {
        className: 'toggle' + (cfg[trig.key] ? ' on' : ''),
        style: { cursor: 'pointer' },
        onClick: function() {
          cfg[trig.key] = !cfg[trig.key];
          toggle.className = 'toggle' + (cfg[trig.key] ? ' on' : '');
          ipcSend('set_snapshot_config', cfg);
        }
      });
      row.appendChild(toggle);
      card.appendChild(row);
    });

    container.appendChild(card);
  }

  // ── Main render ───────────────────────────────────────────────
  var rootContainer = null;

  function rerender() {
    if (!rootContainer) return;
    rootContainer.innerHTML = '';

    // Header
    var header = el('div', { className: 'page-header' });
    header.appendChild(el('span', { className: 'page-icon', innerHTML: '&#9203;' }));
    header.appendChild(el('h2', { textContent: 'Time-Travel System State' }));
    rootContainer.appendChild(header);

    rootContainer.appendChild(el('p', {
      className: 'page-desc',
      textContent: 'Capture system state snapshots and navigate through them like a timeline. Compare any two snapshots to see what changed, diagnose degradation causes, and optionally roll back to a known-good configuration.'
    }));

    // Score cards
    renderScoreCards(rootContainer);

    // Timeline chart
    renderChart(rootContainer);

    // Checkpoints
    renderCheckpoints(rootContainer);

    // Diff and diagnosis side by side
    if (state.diffResult || state.diagnosis.length > 0) {
      var diffGrid = el('div', { style: { display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' } });
      renderDiffView(diffGrid);
      renderDiagnosis(diffGrid);
      rootContainer.appendChild(diffGrid);
    }

    // Auto-snapshot config
    renderSnapshotConfig(rootContainer);

    // Dialogs (appended to body-level container)
    renderRollbackDialog(rootContainer);
    renderCreateDialog(rootContainer);
  }

  // ── Public API ────────────────────────────────────────────────
  window.RuVectorPages.Timeline_Init = function(container) {
    container.innerHTML = '';
    var inner = el('div', { className: 'page-inner' });
    container.appendChild(inner);
    rootContainer = inner;

    // Load mock data for demo
    state.healthHistory = MOCK_HISTORY;
    state.checkpoints = MOCK_CHECKPOINTS.slice();
    state.currentScore = MOCK_HISTORY[MOCK_HISTORY.length - 1].score;
    state.bestToday = Math.max.apply(null, MOCK_HISTORY.map(function(p) { return p.score; }));

    ipcSend('get_snapshots');
    rerender();
    state.initialized = true;

    // Redraw chart on resize
    window.addEventListener('resize', function() {
      if (chartCanvas) drawChart();
    });
  };

  window.RuVectorPages.Timeline_Update = function(data) {
    if (!data) return;
    if (data.health_history) {
      state.healthHistory = data.health_history;
      state.currentScore = data.health_history[data.health_history.length - 1].score;
      state.bestToday = Math.max.apply(null, data.health_history.map(function(p) { return p.score; }));
    }
    if (data.checkpoints) state.checkpoints = data.checkpoints;
    if (data.diff) state.diffResult = data.diff;
    if (data.diagnosis) state.diagnosis = data.diagnosis;
    if (data.snapshot_config) state.snapshotConfig = data.snapshot_config;
    rerender();
  };

  window.updateTimeline = function(data) {
    window.RuVectorPages.Timeline_Update(data);
  };

})();
