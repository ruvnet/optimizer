/* ================================================================
   ADR-025: Agentic Desktop Automation
   Self-contained IIFE component for RuVector Control Center
   Canvas 2D for learning metrics chart, full interactive UI
   ================================================================ */
(function(){
  'use strict';

  window.RuVectorPages = window.RuVectorPages || {};

  // ── State ──────────────────────────────────────────────────────
  var state = {
    initialized: false,
    agentActive: false,
    currentModel: null,
    vramUsage: 0,
    vramTotal: 8192,
    inferenceSpeed: 0,
    models: [
      { tier: 0, name: 'OmniParser', params: '0.3B', vram_mb: 512, loaded: false, desc: 'Screen element parser. Identifies UI components in screenshots.' },
      { tier: 1, name: 'Grounding-2B', params: '2B', vram_mb: 2048, loaded: false, desc: 'Spatial grounding model. Locates elements by description.' },
      { tier: 2, name: 'Reasoning-7B', params: '7B', vram_mb: 6144, loaded: false, desc: 'Full reasoning model. Plans multi-step automation sequences.' }
    ],
    watchAndLearn: {
      recording: false,
      totalTrajectories: 0,
      perApp: []
    },
    adapters: [],
    currentTask: null,
    safety: {
      requireUserPresent: true,
      maxActionsPerMin: 10,
      confirmFinancial: true,
      confirmDelete: true,
      confirmSend: true
    },
    training: {
      active: false,
      jobName: null,
      progress: 0,
      ewcStatus: 'idle'
    },
    metrics: {
      successRate: 0,
      improvementBaseline: 0,
      episodesCompleted: 0
    },
    hardware: {
      gpu: 'Unknown',
      vramTotal: 8192,
      recommendedTier: 1
    },
    aidefence: {
      threatsBlocked: 0,
      scansPerformed: 0,
      piiDetections: 0
    },
    auditLog: []
  };

  // ── Mock data ─────────────────────────────────────────────────
  var MOCK_ADAPTERS = [
    { app: 'Visual Studio Code', version: '1.2.0', accuracy: 94.2, samples: 1240, lastTrained: Date.now() - 86400000 },
    { app: 'Chrome Browser', version: '1.0.3', accuracy: 87.5, samples: 890, lastTrained: Date.now() - 172800000 },
    { app: 'Windows Explorer', version: '0.8.1', accuracy: 78.3, samples: 456, lastTrained: Date.now() - 259200000 },
    { app: 'Microsoft Teams', version: '0.5.0', accuracy: 72.1, samples: 234, lastTrained: Date.now() - 432000000 }
  ];

  var MOCK_TRAJECTORIES = [
    { app: 'VS Code', count: 342 },
    { app: 'Chrome', count: 256 },
    { app: 'Explorer', count: 134 },
    { app: 'Teams', count: 89 },
    { app: 'Terminal', count: 67 }
  ];

  var MOCK_AUDIT = [
    { time: Date.now() - 60000, action: 'Closed idle browser tab (Chrome)', type: 'memory' },
    { time: Date.now() - 180000, action: 'Suspended background updater (Spotify)', type: 'process' },
    { time: Date.now() - 300000, action: 'Adjusted power plan to Balanced', type: 'power' },
    { time: Date.now() - 420000, action: 'Cleared temp files (freed 234 MB)', type: 'cleanup' },
    { time: Date.now() - 600000, action: 'AIDefence blocked suspicious prompt injection', type: 'security' },
    { time: Date.now() - 720000, action: 'Reduced VS Code memory pressure via extension unload', type: 'memory' },
    { time: Date.now() - 900000, action: 'Detected PII in clipboard, cleared automatically', type: 'security' },
    { time: Date.now() - 1200000, action: 'Optimized startup sequence (saved 1.2s)', type: 'startup' },
    { time: Date.now() - 1500000, action: 'Learning: recorded VS Code workflow trajectory', type: 'learning' },
    { time: Date.now() - 1800000, action: 'Consolidated LoRA adapters via EWC++', type: 'learning' }
  ];

  var MOCK_LEARNING_HISTORY = (function() {
    var pts = [];
    for (var i = 0; i < 50; i++) {
      var baseReward = 0.3 + (i / 50) * 0.5;
      var noise = (Math.random() - 0.5) * 0.15;
      pts.push({ episode: i + 1, reward: Math.max(0, Math.min(1, baseReward + noise)) });
    }
    return pts;
  })();

  // ── IPC ────────────────────────────────────────────────────────
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

  function formatTimeAgo(ts) {
    var diff = Date.now() - ts;
    if (diff < 60000) return Math.round(diff / 1000) + 's ago';
    if (diff < 3600000) return Math.round(diff / 60000) + 'm ago';
    if (diff < 86400000) return Math.round(diff / 3600000) + 'h ago';
    return Math.round(diff / 86400000) + 'd ago';
  }

  function formatDate(ts) {
    return new Date(ts).toLocaleDateString([], { month: 'short', day: 'numeric' });
  }

  function getComputedVar(name) {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }

  // ── Canvas: Learning curve chart ──────────────────────────────
  var learningCanvas = null;
  var learningCtx = null;

  function drawLearningChart() {
    if (!learningCanvas || !learningCtx) return;
    var data = MOCK_LEARNING_HISTORY;
    if (data.length < 2) return;

    var dpr = window.devicePixelRatio || 1;
    var w = learningCanvas.clientWidth;
    var h = learningCanvas.clientHeight;
    learningCanvas.width = w * dpr;
    learningCanvas.height = h * dpr;
    var ctx = learningCtx;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    var pad = { top: 16, right: 12, bottom: 26, left: 32 };
    var cw = w - pad.left - pad.right;
    var ch = h - pad.top - pad.bottom;

    ctx.clearRect(0, 0, w, h);

    var textDim = getComputedVar('--text-dim') || '#505872';
    var borderColor = getComputedVar('--border') || '#252d45';
    var cyanColor = getComputedVar('--accent-cyan') || '#4ecdc4';
    var greenColor = getComputedVar('--accent-green') || '#6bcb77';

    // Grid
    ctx.strokeStyle = borderColor;
    ctx.lineWidth = 0.5;
    ctx.font = '9px ' + (getComputedVar('--mono') || 'Consolas');
    ctx.fillStyle = textDim;
    ctx.textAlign = 'right';
    ctx.textBaseline = 'middle';

    for (var g = 0; g <= 4; g++) {
      var val = (1 - g * 0.25).toFixed(2);
      var gy = pad.top + (g / 4) * ch;
      ctx.beginPath();
      ctx.moveTo(pad.left, gy);
      ctx.lineTo(pad.left + cw, gy);
      ctx.stroke();
      ctx.fillText(val, pad.left - 4, gy);
    }

    // X axis labels
    ctx.textAlign = 'center';
    ctx.textBaseline = 'top';
    for (var l = 0; l <= 5; l++) {
      var idx = Math.floor(l / 5 * (data.length - 1));
      var lx = pad.left + (idx / (data.length - 1)) * cw;
      ctx.fillText('Ep ' + data[idx].episode, lx, pad.top + ch + 6);
    }

    // Build points
    var points = data.map(function(p, i) {
      return {
        x: pad.left + (i / (data.length - 1)) * cw,
        y: pad.top + (1 - p.reward) * ch
      };
    });

    // Gradient fill
    var gradient = ctx.createLinearGradient(0, pad.top, 0, pad.top + ch);
    gradient.addColorStop(0, greenColor + '30');
    gradient.addColorStop(1, greenColor + '05');

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
    ctx.strokeStyle = greenColor;
    ctx.lineWidth = 2;
    ctx.lineJoin = 'round';
    ctx.stroke();

    // Trend line
    var firstY = points[0].y;
    var lastY = points[points.length - 1].y;
    ctx.setLineDash([4, 4]);
    ctx.strokeStyle = cyanColor + '60';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(points[0].x, firstY);
    ctx.lineTo(points[points.length - 1].x, lastY);
    ctx.stroke();
    ctx.setLineDash([]);

    // Latest dot
    var last = points[points.length - 1];
    ctx.beginPath();
    ctx.arc(last.x, last.y, 4, 0, Math.PI * 2);
    ctx.fillStyle = greenColor;
    ctx.fill();
  }

  // ── Render: agent status card ─────────────────────────────────
  function renderAgentStatus(container) {
    var card = el('div', { className: 'ph-card' });
    card.appendChild(el('div', { className: 'ph-card-title', textContent: 'Agent Status' }));

    // Active toggle
    var toggleRow = el('div', { style: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' } });
    toggleRow.appendChild(el('span', {
      textContent: state.agentActive ? 'Active' : 'Inactive',
      style: { fontSize: '14px', fontWeight: '600', color: state.agentActive ? 'var(--accent-green)' : 'var(--text-dim)' }
    }));
    var toggle = el('div', {
      className: 'toggle' + (state.agentActive ? ' on' : ''),
      style: { cursor: 'pointer' },
      onClick: function() {
        state.agentActive = !state.agentActive;
        ipcSend('toggle_agent', { active: state.agentActive });
        rerender();
      }
    });
    toggleRow.appendChild(toggle);
    card.appendChild(toggleRow);

    // Info fields
    var fields = [
      { label: 'Current Model', value: state.currentModel || 'None loaded' },
      { label: 'VRAM Usage', value: state.vramUsage + ' / ' + state.vramTotal + ' MB' },
      { label: 'Inference Speed', value: state.inferenceSpeed > 0 ? state.inferenceSpeed + ' ms/frame' : 'N/A' }
    ];

    fields.forEach(function(f) {
      var row = el('div', { className: 'ph-field' });
      row.appendChild(el('span', { className: 'ph-field-label', textContent: f.label }));
      row.appendChild(el('span', { textContent: f.value, style: { fontFamily: 'var(--mono)', fontSize: '11px', color: 'var(--text-primary)' } }));
      card.appendChild(row);
    });

    // VRAM bar
    if (state.vramTotal > 0) {
      var pct = Math.round((state.vramUsage / state.vramTotal) * 100);
      var barRow = el('div', { style: { marginTop: '8px' } });
      barRow.appendChild(el('div', { textContent: 'VRAM: ' + pct + '%', style: { fontSize: '9px', color: 'var(--text-dim)', marginBottom: '4px' } }));
      var bar = el('div', { style: { width: '100%', height: '6px', background: 'var(--gauge-track)', borderRadius: '3px', overflow: 'hidden' } });
      var barColor = pct > 80 ? 'var(--accent-red)' : pct > 50 ? 'var(--accent-amber)' : 'var(--accent-cyan)';
      bar.appendChild(el('div', { style: { width: pct + '%', height: '100%', background: barColor, borderRadius: '3px', transition: 'width 0.3s ease' } }));
      barRow.appendChild(bar);
      card.appendChild(barRow);
    }

    container.appendChild(card);
  }

  // ── Render: model tiers ───────────────────────────────────────
  function renderModelTiers(container) {
    var card = el('div', { className: 'ph-card' });
    card.appendChild(el('div', { className: 'ph-card-title', textContent: 'Model Tiers' }));

    state.models.forEach(function(model) {
      var tierColors = ['var(--accent-cyan)', 'var(--accent-amber)', 'var(--accent-purple)'];
      var color = tierColors[model.tier] || 'var(--text-secondary)';

      var row = el('div', {
        style: {
          display: 'flex', justifyContent: 'space-between', alignItems: 'center',
          padding: '10px', marginBottom: '6px', borderRadius: '8px',
          border: '1px solid ' + (model.loaded ? color : 'var(--border)'),
          background: model.loaded ? color.replace(')', ',0.08)').replace('var(', 'rgba(').replace('--accent-cyan', '78,205,196').replace('--accent-amber', '212,165,116').replace('--accent-purple', '167,139,250') : 'transparent'
        }
      });

      var info = el('div');
      var tierLabel = el('div', { style: { display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '2px' } });
      tierLabel.appendChild(el('span', {
        textContent: 'Tier ' + model.tier,
        style: { fontSize: '9px', fontWeight: '700', padding: '1px 6px', borderRadius: '3px', background: color, color: '#fff' }
      }));
      tierLabel.appendChild(el('span', { textContent: model.name, style: { fontSize: '12px', fontWeight: '600', color: 'var(--text-primary)' } }));
      tierLabel.appendChild(el('span', { textContent: model.params, style: { fontSize: '10px', color: 'var(--text-dim)', fontFamily: 'var(--mono)' } }));
      info.appendChild(tierLabel);
      info.appendChild(el('div', { textContent: model.desc, style: { fontSize: '10px', color: 'var(--text-secondary)', lineHeight: '1.4' } }));
      info.appendChild(el('div', { textContent: 'VRAM: ' + model.vram_mb + ' MB', style: { fontSize: '9px', color: 'var(--text-dim)', fontFamily: 'var(--mono)', marginTop: '2px' } }));
      row.appendChild(info);

      var btn = el('button', {
        className: 'btn',
        style: {
          padding: '4px 12px', fontSize: '10px', cursor: 'pointer', flexShrink: '0',
          borderColor: model.loaded ? 'var(--accent-red)' : color,
          color: model.loaded ? 'var(--accent-red)' : color
        },
        textContent: model.loaded ? 'Unload' : 'Load',
        onClick: (function(m) { return function() {
          m.loaded = !m.loaded;
          if (m.loaded) {
            state.currentModel = m.name;
            state.vramUsage += m.vram_mb;
            state.inferenceSpeed = m.tier === 0 ? 50 : m.tier === 1 ? 200 : 1000;
          } else {
            state.vramUsage = Math.max(0, state.vramUsage - m.vram_mb);
            var loaded = state.models.filter(function(x) { return x.loaded; });
            state.currentModel = loaded.length > 0 ? loaded[loaded.length - 1].name : null;
            state.inferenceSpeed = loaded.length > 0 ? (loaded[loaded.length - 1].tier === 0 ? 50 : loaded[loaded.length - 1].tier === 1 ? 200 : 1000) : 0;
          }
          ipcSend('load_model', { tier: m.tier, action: m.loaded ? 'load' : 'unload' });
          rerender();
        }; })(model)
      });
      row.appendChild(btn);
      card.appendChild(row);
    });

    container.appendChild(card);
  }

  // ── Render: watch-and-learn status ────────────────────────────
  function renderWatchAndLearn(container) {
    var card = el('div', { className: 'ph-card' });
    card.appendChild(el('div', { className: 'ph-card-title', textContent: 'Watch & Learn' }));

    // Recording indicator
    var recording = state.watchAndLearn.recording;
    var recRow = el('div', { style: { display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '10px' } });
    recRow.appendChild(el('div', {
      style: {
        width: '10px', height: '10px', borderRadius: '50%',
        background: recording ? 'var(--accent-red)' : 'var(--text-dim)',
        animation: recording ? 'labelPulse 1s ease-in-out infinite' : 'none'
      }
    }));
    recRow.appendChild(el('span', {
      textContent: recording ? 'Recording trajectories...' : 'Not recording',
      style: { fontSize: '11px', color: recording ? 'var(--accent-red)' : 'var(--text-dim)', fontWeight: '500' }
    }));
    card.appendChild(recRow);

    card.appendChild(el('div', { className: 'ph-field' }, [
      el('span', { className: 'ph-field-label', textContent: 'Total Trajectories' }),
      el('span', { textContent: String(state.watchAndLearn.totalTrajectories), style: { fontFamily: 'var(--mono)', fontSize: '11px', color: 'var(--accent-cyan)', fontWeight: '600' } })
    ]));

    // Per-app counts
    if (state.watchAndLearn.perApp.length > 0) {
      card.appendChild(el('div', { textContent: 'Per Application', style: { fontSize: '9px', fontWeight: '600', color: 'var(--text-dim)', textTransform: 'uppercase', letterSpacing: '0.5px', marginTop: '8px', marginBottom: '4px' } }));
      state.watchAndLearn.perApp.forEach(function(app) {
        var row = el('div', { style: { display: 'flex', justifyContent: 'space-between', padding: '3px 0', fontSize: '11px' } });
        row.appendChild(el('span', { textContent: app.app, style: { color: 'var(--text-secondary)' } }));
        row.appendChild(el('span', { textContent: String(app.count), style: { fontFamily: 'var(--mono)', color: 'var(--text-primary)' } }));
        card.appendChild(row);
      });
    }

    container.appendChild(card);
  }

  // ── Render: LoRA adapter registry ─────────────────────────────
  function renderAdapters(container) {
    var card = el('div', { className: 'ph-card', style: { gridColumn: 'span 2' } });
    card.appendChild(el('div', { className: 'ph-card-title', textContent: 'LoRA Adapter Registry' }));

    if (state.adapters.length === 0) {
      card.appendChild(el('div', { textContent: 'No adapters trained yet. Enable Watch & Learn to start collecting training data.', style: { fontSize: '11px', color: 'var(--text-dim)', padding: '12px 0' } }));
    } else {
      // Table header
      var thead = el('div', { style: { display: 'grid', gridTemplateColumns: '1fr 70px 70px 90px 100px', gap: '8px', padding: '6px 0', borderBottom: '1px solid var(--border)', fontSize: '9px', fontWeight: '600', letterSpacing: '0.5px', textTransform: 'uppercase', color: 'var(--text-dim)' } });
      ['Application', 'Version', 'Accuracy', 'Samples', 'Last Trained'].forEach(function(h) {
        thead.appendChild(el('span', { textContent: h }));
      });
      card.appendChild(thead);

      state.adapters.forEach(function(adapter) {
        var row = el('div', { style: { display: 'grid', gridTemplateColumns: '1fr 70px 70px 90px 100px', gap: '8px', padding: '8px 0', borderBottom: '1px solid var(--border)', alignItems: 'center', fontSize: '11px' } });
        row.appendChild(el('span', { textContent: adapter.app, style: { color: 'var(--text-primary)', fontWeight: '500' } }));
        row.appendChild(el('span', { textContent: adapter.version, style: { fontFamily: 'var(--mono)', fontSize: '10px', color: 'var(--text-secondary)' } }));

        var accColor = adapter.accuracy >= 90 ? 'var(--accent-green)' : adapter.accuracy >= 75 ? 'var(--accent-amber)' : 'var(--accent-red)';
        row.appendChild(el('span', { textContent: adapter.accuracy.toFixed(1) + '%', style: { fontFamily: 'var(--mono)', fontSize: '10px', color: accColor, fontWeight: '600' } }));
        row.appendChild(el('span', { textContent: adapter.samples.toLocaleString(), style: { fontFamily: 'var(--mono)', fontSize: '10px', color: 'var(--text-secondary)' } }));
        row.appendChild(el('span', { textContent: formatDate(adapter.lastTrained), style: { fontSize: '10px', color: 'var(--text-dim)' } }));
        card.appendChild(row);
      });
    }

    container.appendChild(card);
  }

  // ── Render: safety settings ───────────────────────────────────
  function renderSafety(container) {
    var card = el('div', { className: 'ph-card' });
    card.appendChild(el('div', { className: 'ph-card-title', textContent: 'Safety Settings' }));

    var s = state.safety;

    // User present toggle
    var presentRow = el('div', { className: 'toggle-row' });
    presentRow.appendChild(el('span', { textContent: 'Require User Present' }));
    var presentToggle = el('div', {
      className: 'toggle' + (s.requireUserPresent ? ' on' : ''),
      style: { cursor: 'pointer' },
      onClick: function() {
        s.requireUserPresent = !s.requireUserPresent;
        ipcSend('set_agent_config', { safety: s });
        rerender();
      }
    });
    presentRow.appendChild(presentToggle);
    card.appendChild(presentRow);

    // Max actions slider
    var sliderRow = el('div', { style: { padding: '8px 0', borderTop: '1px solid var(--border)' } });
    var sliderLabel = el('div', { style: { display: 'flex', justifyContent: 'space-between', fontSize: '11px', color: 'var(--text-secondary)', marginBottom: '6px' } });
    sliderLabel.appendChild(el('span', { textContent: 'Max Actions / Minute' }));
    sliderLabel.appendChild(el('span', { textContent: String(s.maxActionsPerMin), style: { fontFamily: 'var(--mono)', color: 'var(--accent-cyan)', fontWeight: '600' } }));
    sliderRow.appendChild(sliderLabel);

    var slider = el('input', {
      type: 'range',
      min: '1',
      max: '30',
      value: String(s.maxActionsPerMin),
      style: {
        width: '100%', height: '4px', appearance: 'none', background: 'var(--gauge-track)',
        borderRadius: '2px', outline: 'none', cursor: 'pointer',
        accentColor: getComputedVar('--accent-cyan') || '#4ecdc4'
      },
      onInput: function() {
        s.maxActionsPerMin = parseInt(slider.value);
        sliderLabel.lastChild.textContent = String(s.maxActionsPerMin);
      },
      onChange: function() {
        ipcSend('set_agent_config', { safety: s });
      }
    });
    sliderRow.appendChild(slider);
    card.appendChild(sliderRow);

    // Confirm toggles
    var confirms = [
      { key: 'confirmFinancial', label: 'Confirm Financial Actions' },
      { key: 'confirmDelete', label: 'Confirm Delete Actions' },
      { key: 'confirmSend', label: 'Confirm Send/Share Actions' }
    ];

    confirms.forEach(function(conf) {
      var row = el('div', { className: 'toggle-row' });
      row.appendChild(el('span', { textContent: conf.label }));
      var toggle = el('div', {
        className: 'toggle' + (s[conf.key] ? ' on' : ''),
        style: { cursor: 'pointer' },
        onClick: (function(k) { return function() {
          s[k] = !s[k];
          toggle.className = 'toggle' + (s[k] ? ' on' : '');
          ipcSend('set_agent_config', { safety: s });
        }; })(conf.key)
      });
      row.appendChild(toggle);
      card.appendChild(row);
    });

    container.appendChild(card);
  }

  // ── Render: training status ───────────────────────────────────
  function renderTraining(container) {
    var card = el('div', { className: 'ph-card' });
    card.appendChild(el('div', { className: 'ph-card-title', textContent: 'Training Status' }));

    var t = state.training;

    if (t.active) {
      card.appendChild(el('div', { textContent: t.jobName || 'Training in progress...', style: { fontSize: '12px', fontWeight: '500', color: 'var(--accent-cyan)', marginBottom: '8px' } }));
      var bar = el('div', { style: { width: '100%', height: '6px', background: 'var(--gauge-track)', borderRadius: '3px', overflow: 'hidden', marginBottom: '4px' } });
      bar.appendChild(el('div', { style: { width: t.progress + '%', height: '100%', background: 'var(--accent-cyan)', borderRadius: '3px', transition: 'width 0.3s ease' } }));
      card.appendChild(bar);
      card.appendChild(el('div', { textContent: t.progress + '% complete', style: { fontSize: '10px', color: 'var(--text-dim)', fontFamily: 'var(--mono)' } }));
    } else {
      card.appendChild(el('div', { textContent: 'No active training jobs', style: { fontSize: '11px', color: 'var(--text-dim)', padding: '4px 0' } }));
    }

    var ewcRow = el('div', { className: 'ph-field', style: { marginTop: '8px' } });
    ewcRow.appendChild(el('span', { className: 'ph-field-label', textContent: 'EWC++ Consolidation' }));
    var ewcColor = t.ewcStatus === 'running' ? 'var(--accent-cyan)' : t.ewcStatus === 'complete' ? 'var(--accent-green)' : 'var(--text-dim)';
    ewcRow.appendChild(el('span', { textContent: t.ewcStatus.charAt(0).toUpperCase() + t.ewcStatus.slice(1), style: { fontSize: '10px', fontWeight: '600', color: ewcColor } }));
    card.appendChild(ewcRow);

    container.appendChild(card);
  }

  // ── Render: self-learning metrics ─────────────────────────────
  function renderMetrics(container) {
    var card = el('div', { className: 'ph-card' });
    card.appendChild(el('div', { className: 'ph-card-title', textContent: 'Self-Learning Metrics' }));

    var m = state.metrics;
    var metricItems = [
      { label: 'Success Rate', value: m.successRate + '%', color: m.successRate >= 80 ? 'var(--accent-green)' : 'var(--accent-amber)' },
      { label: 'Improvement Over Baseline', value: '+' + m.improvementBaseline + '%', color: 'var(--accent-cyan)' },
      { label: 'Episodes Completed', value: String(m.episodesCompleted), color: 'var(--text-primary)' }
    ];

    metricItems.forEach(function(mi) {
      var row = el('div', { className: 'ph-field' });
      row.appendChild(el('span', { className: 'ph-field-label', textContent: mi.label }));
      row.appendChild(el('span', { textContent: mi.value, style: { fontFamily: 'var(--mono)', fontSize: '11px', fontWeight: '600', color: mi.color } }));
      card.appendChild(row);
    });

    // Learning curve chart
    card.appendChild(el('div', { textContent: 'Reward Curve', style: { fontSize: '9px', fontWeight: '600', color: 'var(--text-dim)', textTransform: 'uppercase', letterSpacing: '0.5px', marginTop: '10px', marginBottom: '4px' } }));

    learningCanvas = el('canvas', { style: { width: '100%', height: '120px', display: 'block', borderRadius: '6px' } });
    card.appendChild(learningCanvas);
    container.appendChild(card);

    learningCtx = learningCanvas.getContext('2d');
    requestAnimationFrame(drawLearningChart);
  }

  // ── Render: hardware requirements ─────────────────────────────
  function renderHardware(container) {
    var card = el('div', { className: 'ph-card' });
    card.appendChild(el('div', { className: 'ph-card-title', textContent: 'Hardware Requirements' }));

    var hw = state.hardware;
    var fields = [
      { label: 'Detected GPU', value: hw.gpu },
      { label: 'Available VRAM', value: hw.vramTotal + ' MB' },
      { label: 'Recommended Tier', value: 'Tier ' + hw.recommendedTier }
    ];

    fields.forEach(function(f) {
      var row = el('div', { className: 'ph-field' });
      row.appendChild(el('span', { className: 'ph-field-label', textContent: f.label }));
      row.appendChild(el('span', { textContent: f.value, style: { fontFamily: 'var(--mono)', fontSize: '11px', color: 'var(--text-primary)' } }));
      card.appendChild(row);
    });

    // Tier compatibility
    card.appendChild(el('div', { textContent: 'Tier Compatibility', style: { fontSize: '9px', fontWeight: '600', color: 'var(--text-dim)', textTransform: 'uppercase', letterSpacing: '0.5px', marginTop: '8px', marginBottom: '4px' } }));
    state.models.forEach(function(m) {
      var fits = m.vram_mb <= hw.vramTotal;
      var row = el('div', { style: { display: 'flex', alignItems: 'center', gap: '6px', padding: '3px 0', fontSize: '11px' } });
      row.appendChild(el('span', { textContent: fits ? '\u2713' : '\u2717', style: { color: fits ? 'var(--accent-green)' : 'var(--accent-red)', fontWeight: '700' } }));
      row.appendChild(el('span', { textContent: 'Tier ' + m.tier + ': ' + m.name + ' (' + m.vram_mb + ' MB)', style: { color: fits ? 'var(--text-secondary)' : 'var(--text-dim)' } }));
      card.appendChild(row);
    });

    container.appendChild(card);
  }

  // ── Render: AIDefence security status ─────────────────────────
  function renderAIDefence(container) {
    var card = el('div', { className: 'ph-card' });
    card.appendChild(el('div', { className: 'ph-card-title', textContent: 'AIDefence Security' }));

    var def = state.aidefence;
    var fields = [
      { label: 'Threats Blocked', value: String(def.threatsBlocked), color: def.threatsBlocked > 0 ? 'var(--accent-red)' : 'var(--accent-green)' },
      { label: 'Scans Performed', value: String(def.scansPerformed), color: 'var(--accent-cyan)' },
      { label: 'PII Detections', value: String(def.piiDetections), color: def.piiDetections > 0 ? 'var(--accent-amber)' : 'var(--text-dim)' }
    ];

    fields.forEach(function(f) {
      var row = el('div', { className: 'ph-field' });
      row.appendChild(el('span', { className: 'ph-field-label', textContent: f.label }));
      row.appendChild(el('span', { textContent: f.value, style: { fontFamily: 'var(--mono)', fontSize: '12px', fontWeight: '600', color: f.color } }));
      card.appendChild(row);
    });

    // Security grade
    var grade = def.threatsBlocked === 0 ? 'A' : def.threatsBlocked < 3 ? 'B' : 'C';
    var gradeColor = grade === 'A' ? 'var(--accent-green)' : grade === 'B' ? 'var(--accent-amber)' : 'var(--accent-red)';
    var gradeEl = el('div', { style: { display: 'flex', alignItems: 'center', gap: '10px', marginTop: '10px', padding: '8px', borderRadius: '6px', background: 'var(--bg-primary)' } });
    gradeEl.appendChild(el('div', {
      textContent: grade,
      style: {
        width: '36px', height: '36px', borderRadius: '50%', display: 'flex',
        alignItems: 'center', justifyContent: 'center', fontSize: '18px',
        fontWeight: '700', border: '2px solid ' + gradeColor, color: gradeColor
      }
    }));
    gradeEl.appendChild(el('div', { textContent: 'Security Grade', style: { fontSize: '10px', color: 'var(--text-dim)', textTransform: 'uppercase', letterSpacing: '0.5px' } }));
    card.appendChild(gradeEl);

    container.appendChild(card);
  }

  // ── Render: audit log ─────────────────────────────────────────
  function renderAuditLog(container) {
    var card = el('div', { className: 'ph-card', style: { gridColumn: 'span 2' } });
    var header = el('div', { style: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '10px' } });
    header.appendChild(el('div', { className: 'ph-card-title', textContent: 'Audit Log', style: { margin: '0' } }));
    header.appendChild(el('button', {
      className: 'btn',
      style: { padding: '4px 10px', fontSize: '9px', cursor: 'pointer' },
      textContent: 'Refresh',
      onClick: function() { ipcSend('get_audit_log'); }
    }));
    card.appendChild(header);

    var typeIcons = {
      memory: '\u{1F4BE}',
      process: '\u{2699}',
      power: '\u{26A1}',
      cleanup: '\u{1F9F9}',
      security: '\u{1F6E1}',
      startup: '\u{1F680}',
      learning: '\u{1F9E0}'
    };

    var typeColors = {
      memory: 'var(--accent-cyan)',
      process: 'var(--accent-amber)',
      power: 'var(--accent-green)',
      cleanup: 'var(--text-secondary)',
      security: 'var(--accent-red)',
      startup: 'var(--accent-purple)',
      learning: 'var(--accent-cyan)'
    };

    var logList = el('div', { style: { maxHeight: '280px', overflowY: 'auto' } });

    if (state.auditLog.length === 0) {
      logList.appendChild(el('div', { textContent: 'No actions recorded yet', style: { fontSize: '11px', color: 'var(--text-dim)', padding: '12px 0' } }));
    } else {
      state.auditLog.forEach(function(entry) {
        var row = el('div', { style: { display: 'flex', alignItems: 'flex-start', gap: '10px', padding: '6px 0', borderBottom: '1px solid var(--border)' } });

        // Type badge
        var badge = el('span', {
          textContent: entry.type.charAt(0).toUpperCase() + entry.type.slice(1),
          style: {
            fontSize: '8px', fontWeight: '600', padding: '2px 6px',
            borderRadius: '3px', background: (typeColors[entry.type] || 'var(--text-dim)') + '20',
            color: typeColors[entry.type] || 'var(--text-dim)',
            textTransform: 'uppercase', letterSpacing: '0.5px', flexShrink: '0', marginTop: '1px'
          }
        });
        row.appendChild(badge);

        var info = el('div', { style: { flex: '1' } });
        info.appendChild(el('div', { textContent: entry.action, style: { fontSize: '11px', color: 'var(--text-primary)', lineHeight: '1.4' } }));
        info.appendChild(el('div', { textContent: formatTimeAgo(entry.time), style: { fontSize: '9px', color: 'var(--text-dim)', fontFamily: 'var(--mono)', marginTop: '1px' } }));
        row.appendChild(info);

        logList.appendChild(row);
      });
    }

    card.appendChild(logList);
    container.appendChild(card);
  }

  // ── Main render ───────────────────────────────────────────────
  var rootContainer = null;

  function rerender() {
    if (!rootContainer) return;
    rootContainer.innerHTML = '';

    // Header
    var header = el('div', { className: 'page-header' });
    header.appendChild(el('span', { className: 'page-icon', innerHTML: '&#129302;' }));
    header.appendChild(el('h2', { textContent: 'Agentic Desktop Automation' }));
    rootContainer.appendChild(header);

    rootContainer.appendChild(el('p', {
      className: 'page-desc',
      textContent: 'AI agent that observes your desktop via a 3-tier vision model, learns optimization patterns through online reinforcement learning with LoRA fine-tuning, and autonomously manages system resources. Protected by AIDefence against prompt injection and PII leaks.'
    }));

    // Top row: status + model tiers
    var topGrid = el('div', { style: { display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px', marginBottom: '16px' } });
    renderAgentStatus(topGrid);
    renderModelTiers(topGrid);
    rootContainer.appendChild(topGrid);

    // Mid row: watch & learn + safety + training
    var midGrid = el('div', { style: { display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '16px', marginBottom: '16px' } });
    renderWatchAndLearn(midGrid);
    renderSafety(midGrid);
    renderTraining(midGrid);
    rootContainer.appendChild(midGrid);

    // Adapters table (full width)
    var adapterGrid = el('div', { className: 'page-grid', style: { marginBottom: '16px' } });
    renderAdapters(adapterGrid);
    rootContainer.appendChild(adapterGrid);

    // Bottom row: metrics + hardware + AIDefence
    var bottomGrid = el('div', { style: { display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '16px', marginBottom: '16px' } });
    renderMetrics(bottomGrid);
    renderHardware(bottomGrid);
    renderAIDefence(bottomGrid);
    rootContainer.appendChild(bottomGrid);

    // Audit log (full width)
    var logGrid = el('div', { className: 'page-grid' });
    renderAuditLog(logGrid);
    rootContainer.appendChild(logGrid);
  }

  // ── Public API ────────────────────────────────────────────────
  window.RuVectorPages.Agent_Init = function(container) {
    container.innerHTML = '';
    var inner = el('div', { className: 'page-inner' });
    container.appendChild(inner);
    rootContainer = inner;

    // Load mock data for demo
    state.watchAndLearn.totalTrajectories = 888;
    state.watchAndLearn.perApp = MOCK_TRAJECTORIES;
    state.adapters = MOCK_ADAPTERS;
    state.auditLog = MOCK_AUDIT;
    state.metrics = { successRate: 84.2, improvementBaseline: 12.5, episodesCompleted: 50 };
    state.hardware = { gpu: 'NVIDIA RTX 4070 Ti', vramTotal: 12288, recommendedTier: 2 };
    state.aidefence = { threatsBlocked: 2, scansPerformed: 1456, piiDetections: 3 };
    state.training = { active: true, jobName: 'VS Code Adapter v1.3.0', progress: 67, ewcStatus: 'running' };
    state.watchAndLearn.recording = true;

    ipcSend('get_agent_status');
    ipcSend('get_adapters');
    ipcSend('get_trajectories');
    ipcSend('get_audit_log');
    ipcSend('get_aidefence_stats');

    rerender();
    state.initialized = true;

    window.addEventListener('resize', function() {
      if (learningCanvas) drawLearningChart();
    });
  };

  window.RuVectorPages.Agent_Update = function(data) {
    if (!data) return;
    if (data.active !== undefined) state.agentActive = data.active;
    if (data.current_model) state.currentModel = data.current_model;
    if (data.vram_usage !== undefined) state.vramUsage = data.vram_usage;
    if (data.inference_speed !== undefined) state.inferenceSpeed = data.inference_speed;
    if (data.watch_and_learn) {
      state.watchAndLearn = data.watch_and_learn;
    }
    if (data.adapters) state.adapters = data.adapters;
    if (data.safety) state.safety = data.safety;
    if (data.training) state.training = data.training;
    if (data.metrics) state.metrics = data.metrics;
    if (data.hardware) state.hardware = data.hardware;
    if (data.aidefence) state.aidefence = data.aidefence;
    if (data.audit_log) state.auditLog = data.audit_log;
    rerender();
  };

  window.updateAgentStatus = function(data) {
    window.RuVectorPages.Agent_Update(data);
  };

})();
