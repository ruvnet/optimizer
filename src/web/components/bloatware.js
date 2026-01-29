/* ================================================================
   ADR-023: Bloatware & Telemetry Silencer
   Self-contained IIFE component for RuVector Control Center
   ================================================================ */
(function(){
  'use strict';

  window.RuVectorPages = window.RuVectorPages || {};

  // ── State ──────────────────────────────────────────────────────
  var state = {
    initialized: false,
    scanning: false,
    activeTab: 'oem',
    telemetryLevel: 'minimal',
    items: {},
    undoHistory: [],
    summary: { ram_mb: 0, cpu_pct: 0, network_kb_day: 0, boot_delay_ms: 0 },
    corporateDetected: false
  };

  var CATEGORIES = [
    { key: 'oem',       label: 'OEM Preinstalls' },
    { key: 'microsoft', label: 'Microsoft Bloatware' },
    { key: 'telemetry', label: 'Telemetry Services' },
    { key: 'background',label: 'Background Junk' },
    { key: 'startup',   label: 'Startup Junk' }
  ];

  var SAFETY_COLORS = {
    safe:     { bg: 'var(--accent-green)',    text: '#fff', label: 'Safe' },
    moderate: { bg: 'var(--accent-amber)',    text: '#fff', label: 'Moderate' },
    caution:  { bg: '#e08040',               text: '#fff', label: 'Caution' },
    expert:   { bg: 'var(--accent-red)',      text: '#fff', label: 'Expert' }
  };

  var TELEMETRY_LEVELS = [
    { key: 'minimal',    label: 'Minimal',    desc: 'Disable non-essential telemetry. Keeps Windows Update and critical error reporting.' },
    { key: 'aggressive', label: 'Aggressive',  desc: 'Block all optional telemetry endpoints. Disables Cortana, diagnostics tracking, ad services.' },
    { key: 'paranoid',   label: 'Paranoid',    desc: 'Block ALL outbound telemetry including error reporting. May break some Windows features.' }
  ];

  // ── Mock data for demo ─────────────────────────────────────────
  var MOCK_ITEMS = {
    oem: [
      { id: 'oem-1', name: 'Dell SupportAssist', ram_mb: 145, cpu_pct: 2.1, status: 'active', safety: 'safe' },
      { id: 'oem-2', name: 'HP Smart', ram_mb: 89, cpu_pct: 0.8, status: 'active', safety: 'safe' },
      { id: 'oem-3', name: 'Lenovo Vantage Service', ram_mb: 112, cpu_pct: 1.5, status: 'active', safety: 'safe' },
      { id: 'oem-4', name: 'McAfee WebAdvisor', ram_mb: 203, cpu_pct: 3.2, status: 'active', safety: 'moderate' },
      { id: 'oem-5', name: 'Norton Security Agent', ram_mb: 178, cpu_pct: 2.8, status: 'disabled', safety: 'moderate' }
    ],
    microsoft: [
      { id: 'ms-1', name: 'Xbox Game Bar', ram_mb: 67, cpu_pct: 0.5, status: 'active', safety: 'safe' },
      { id: 'ms-2', name: 'Cortana', ram_mb: 134, cpu_pct: 1.8, status: 'active', safety: 'safe' },
      { id: 'ms-3', name: 'Microsoft Teams (Personal)', ram_mb: 245, cpu_pct: 2.4, status: 'active', safety: 'safe' },
      { id: 'ms-4', name: 'OneDrive Sync Engine', ram_mb: 156, cpu_pct: 1.2, status: 'active', safety: 'moderate' },
      { id: 'ms-5', name: 'Windows Feedback Hub', ram_mb: 43, cpu_pct: 0.2, status: 'active', safety: 'safe' },
      { id: 'ms-6', name: 'Clipchamp', ram_mb: 38, cpu_pct: 0.1, status: 'active', safety: 'safe' }
    ],
    telemetry: [
      { id: 'tel-1', name: 'DiagTrack Service', ram_mb: 52, cpu_pct: 0.9, status: 'active', safety: 'moderate' },
      { id: 'tel-2', name: 'Connected User Experiences', ram_mb: 34, cpu_pct: 0.4, status: 'active', safety: 'moderate' },
      { id: 'tel-3', name: 'Windows Error Reporting', ram_mb: 28, cpu_pct: 0.2, status: 'active', safety: 'caution' },
      { id: 'tel-4', name: 'Customer Experience Program', ram_mb: 18, cpu_pct: 0.1, status: 'active', safety: 'safe' },
      { id: 'tel-5', name: 'Application Insights Telemetry', ram_mb: 45, cpu_pct: 0.6, status: 'active', safety: 'moderate' }
    ],
    background: [
      { id: 'bg-1', name: 'Windows Search Indexer', ram_mb: 198, cpu_pct: 4.5, status: 'active', safety: 'caution' },
      { id: 'bg-2', name: 'SysMain (Superfetch)', ram_mb: 312, cpu_pct: 1.8, status: 'active', safety: 'expert' },
      { id: 'bg-3', name: 'Windows Update Medic', ram_mb: 45, cpu_pct: 0.3, status: 'active', safety: 'expert' },
      { id: 'bg-4', name: 'Print Spooler', ram_mb: 22, cpu_pct: 0.1, status: 'active', safety: 'moderate' },
      { id: 'bg-5', name: 'WMI Provider Host', ram_mb: 67, cpu_pct: 1.1, status: 'active', safety: 'caution' }
    ],
    startup: [
      { id: 'st-1', name: 'Spotify Web Helper', ram_mb: 89, cpu_pct: 0.4, status: 'active', safety: 'safe' },
      { id: 'st-2', name: 'Discord Updater', ram_mb: 56, cpu_pct: 0.3, status: 'active', safety: 'safe' },
      { id: 'st-3', name: 'Adobe Creative Cloud', ram_mb: 167, cpu_pct: 1.6, status: 'active', safety: 'safe' },
      { id: 'st-4', name: 'Steam Client Bootstrapper', ram_mb: 134, cpu_pct: 0.9, status: 'active', safety: 'safe' },
      { id: 'st-5', name: 'Java Update Scheduler', ram_mb: 42, cpu_pct: 0.2, status: 'active', safety: 'safe' }
    ]
  };

  // ── IPC helper ─────────────────────────────────────────────────
  function ipcSend(type, payload) {
    if (window.ipc) {
      var msg = Object.assign({ type: type }, payload || {});
      window.ipc.postMessage(JSON.stringify(msg));
    }
  }

  // ── DOM Helpers ────────────────────────────────────────────────
  function el(tag, attrs, children) {
    var e = document.createElement(tag);
    if (attrs) {
      Object.keys(attrs).forEach(function(k) {
        if (k === 'className') e.className = attrs[k];
        else if (k === 'style' && typeof attrs[k] === 'object') {
          Object.keys(attrs[k]).forEach(function(s) { e.style[s] = attrs[k][s]; });
        }
        else if (k.indexOf('on') === 0) e.addEventListener(k.slice(2).toLowerCase(), attrs[k]);
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

  // ── Compute summary ───────────────────────────────────────────
  function computeSummary() {
    var total = { ram_mb: 0, cpu_pct: 0, network_kb_day: 0, boot_delay_ms: 0 };
    CATEGORIES.forEach(function(cat) {
      var items = state.items[cat.key] || [];
      items.forEach(function(it) {
        if (it.status === 'active') {
          total.ram_mb += it.ram_mb;
          total.cpu_pct += it.cpu_pct;
        }
      });
    });
    total.network_kb_day = Math.round(total.ram_mb * 0.8);
    total.boot_delay_ms = Math.round(total.ram_mb * 3.2);
    state.summary = total;
  }

  // ── Render summary card ───────────────────────────────────────
  function renderSummary(container) {
    var s = state.summary;
    var card = el('div', { className: 'ph-card', style: { gridColumn: 'span 2' } });
    card.appendChild(el('div', { className: 'ph-card-title', textContent: 'System Impact Summary' }));

    var grid = el('div', { style: { display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: '16px', textAlign: 'center' } });

    var metrics = [
      { value: s.ram_mb + ' MB', label: 'Total RAM Impact', color: 'var(--accent-red)' },
      { value: s.cpu_pct.toFixed(1) + '%', label: 'CPU Usage', color: 'var(--accent-amber)' },
      { value: (s.network_kb_day / 1024).toFixed(1) + ' MB', label: 'Network / Day', color: 'var(--accent-purple)' },
      { value: (s.boot_delay_ms / 1000).toFixed(1) + 's', label: 'Boot Delay', color: 'var(--accent-cyan)' }
    ];

    metrics.forEach(function(m) {
      var col = el('div');
      col.appendChild(el('div', { textContent: m.value, style: { fontSize: '24px', fontWeight: '700', color: m.color, lineHeight: '1', marginBottom: '4px' } }));
      col.appendChild(el('div', { textContent: m.label, style: { fontSize: '9px', color: 'var(--text-dim)', textTransform: 'uppercase', letterSpacing: '0.5px' } }));
      grid.appendChild(col);
    });

    card.appendChild(grid);
    container.appendChild(card);
  }

  // ── Render corporate warning ──────────────────────────────────
  function renderCorporateWarning(container) {
    if (!state.corporateDetected) return;
    var warn = el('div', {
      className: 'ph-card',
      style: { gridColumn: 'span 2', borderColor: 'var(--accent-amber)', background: 'var(--accent-amber-dim)' }
    });
    warn.appendChild(el('div', { style: { display: 'flex', alignItems: 'center', gap: '8px', fontSize: '12px', fontWeight: '600', color: 'var(--accent-amber)' } }, [
      el('span', { textContent: '\u26A0', style: { fontSize: '16px' } }),
      el('span', { textContent: 'Corporate Environment Detected' })
    ]));
    warn.appendChild(el('div', { textContent: 'This machine appears to be domain-joined or MDM-managed. Some items cannot be removed without administrator approval. Managed items are marked and removal is blocked.', style: { fontSize: '11px', color: 'var(--text-secondary)', marginTop: '6px', lineHeight: '1.5' } }));
    container.appendChild(warn);
  }

  // ── Render category tabs ──────────────────────────────────────
  function renderTabs(container) {
    var tabBar = el('div', { style: { display: 'flex', gap: '4px', marginBottom: '16px', flexWrap: 'wrap' } });
    CATEGORIES.forEach(function(cat) {
      var active = state.activeTab === cat.key;
      var count = (state.items[cat.key] || []).filter(function(i) { return i.status === 'active'; }).length;
      var btn = el('button', {
        className: 'btn',
        style: {
          padding: '6px 14px',
          fontSize: '11px',
          background: active ? 'var(--accent-cyan-dim)' : 'var(--bg-card)',
          borderColor: active ? 'var(--accent-cyan)' : 'var(--border)',
          color: active ? 'var(--accent-cyan)' : 'var(--text-secondary)',
          fontWeight: active ? '600' : '400',
          cursor: 'pointer'
        },
        onClick: function() {
          state.activeTab = cat.key;
          rerender();
        }
      });
      btn.textContent = cat.label;
      if (count > 0) {
        var badge = el('span', {
          style: {
            marginLeft: '6px', fontSize: '9px', padding: '1px 5px',
            borderRadius: '8px', background: 'var(--accent-red)', color: '#fff'
          },
          textContent: String(count)
        });
        btn.appendChild(badge);
      }
      tabBar.appendChild(btn);
    });
    container.appendChild(tabBar);
  }

  // ── Render item list for active category ──────────────────────
  function renderItemList(container) {
    var items = state.items[state.activeTab] || [];
    var card = el('div', { className: 'ph-card', style: { gridColumn: 'span 2' } });

    // Header row with bulk actions
    var header = el('div', { style: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' } });
    var catLabel = CATEGORIES.find(function(c) { return c.key === state.activeTab; });
    header.appendChild(el('div', { className: 'ph-card-title', textContent: catLabel ? catLabel.label : '', style: { margin: '0' } }));

    var bulkBtns = el('div', { style: { display: 'flex', gap: '6px' } });
    var btnDisableAll = el('button', {
      className: 'btn',
      style: { padding: '4px 10px', fontSize: '10px', cursor: 'pointer' },
      textContent: 'Disable All',
      onClick: function() { bulkAction('disable'); }
    });
    var btnRemoveAll = el('button', {
      className: 'btn',
      style: { padding: '4px 10px', fontSize: '10px', cursor: 'pointer', borderColor: 'var(--accent-red)', color: 'var(--accent-red)' },
      textContent: 'Remove All',
      onClick: function() { bulkAction('remove'); }
    });
    bulkBtns.appendChild(btnDisableAll);
    bulkBtns.appendChild(btnRemoveAll);
    header.appendChild(bulkBtns);
    card.appendChild(header);

    // Table header
    var tableHead = el('div', { style: { display: 'grid', gridTemplateColumns: '1fr 80px 60px 80px 80px 140px', gap: '8px', padding: '6px 0', borderBottom: '1px solid var(--border)', fontSize: '9px', fontWeight: '600', letterSpacing: '0.5px', textTransform: 'uppercase', color: 'var(--text-dim)' } });
    ['Name', 'RAM', 'CPU', 'Status', 'Safety', 'Actions'].forEach(function(h) {
      tableHead.appendChild(el('span', { textContent: h }));
    });
    card.appendChild(tableHead);

    // Item rows
    if (items.length === 0) {
      card.appendChild(el('div', { textContent: 'No items found in this category. Run a scan to detect bloatware.', style: { padding: '20px 0', textAlign: 'center', fontSize: '11px', color: 'var(--text-dim)' } }));
    } else {
      items.forEach(function(item) {
        var row = el('div', {
          style: {
            display: 'grid',
            gridTemplateColumns: '1fr 80px 60px 80px 80px 140px',
            gap: '8px',
            padding: '8px 0',
            borderBottom: '1px solid var(--border)',
            alignItems: 'center',
            fontSize: '11px',
            opacity: item.status === 'removed' ? '0.4' : '1'
          }
        });

        // Name
        row.appendChild(el('span', { textContent: item.name, style: { color: 'var(--text-primary)', fontWeight: '500' } }));

        // RAM
        row.appendChild(el('span', { textContent: item.ram_mb + ' MB', style: { fontFamily: 'var(--mono)', fontSize: '10px', color: 'var(--accent-amber)' } }));

        // CPU
        row.appendChild(el('span', { textContent: item.cpu_pct + '%', style: { fontFamily: 'var(--mono)', fontSize: '10px', color: 'var(--text-secondary)' } }));

        // Status
        var statusColor = item.status === 'active' ? 'var(--accent-green)' : item.status === 'disabled' ? 'var(--accent-amber)' : 'var(--text-dim)';
        var statusEl = el('span', { style: { display: 'inline-flex', alignItems: 'center', gap: '4px' } });
        statusEl.appendChild(el('span', { style: { width: '6px', height: '6px', borderRadius: '50%', background: statusColor, display: 'inline-block' } }));
        statusEl.appendChild(el('span', { textContent: item.status.charAt(0).toUpperCase() + item.status.slice(1), style: { fontSize: '10px', color: statusColor } }));
        row.appendChild(statusEl);

        // Safety badge
        var sc = SAFETY_COLORS[item.safety] || SAFETY_COLORS.safe;
        row.appendChild(el('span', {
          textContent: sc.label,
          style: {
            fontSize: '9px', fontWeight: '600', padding: '2px 8px',
            borderRadius: '4px', background: sc.bg, color: sc.text,
            textAlign: 'center', display: 'inline-block'
          }
        }));

        // Actions
        var actionsCell = el('div', { style: { display: 'flex', gap: '4px' } });
        if (item.status !== 'removed') {
          var removeBtn = el('button', {
            className: 'btn',
            style: { padding: '2px 8px', fontSize: '9px', cursor: 'pointer', borderColor: 'var(--accent-red)', color: 'var(--accent-red)' },
            textContent: item.status === 'active' ? 'Remove' : 'Remove',
            onClick: (function(it) { return function() { removeItem(it); }; })(item)
          });
          var keepBtn = el('button', {
            className: 'btn',
            style: { padding: '2px 8px', fontSize: '9px', cursor: 'pointer' },
            textContent: 'Keep',
            onClick: (function(it) { return function() { keepItem(it); }; })(item)
          });
          actionsCell.appendChild(removeBtn);
          actionsCell.appendChild(keepBtn);
        } else {
          actionsCell.appendChild(el('span', { textContent: 'Removed', style: { fontSize: '9px', color: 'var(--text-dim)', fontStyle: 'italic' } }));
        }
        row.appendChild(actionsCell);

        card.appendChild(row);
      });
    }

    container.appendChild(card);
  }

  // ── Render telemetry level selector ───────────────────────────
  function renderTelemetrySelector(container) {
    var card = el('div', { className: 'ph-card' });
    card.appendChild(el('div', { className: 'ph-card-title', textContent: 'Telemetry Level' }));

    TELEMETRY_LEVELS.forEach(function(level) {
      var active = state.telemetryLevel === level.key;
      var row = el('div', {
        style: {
          display: 'flex', alignItems: 'flex-start', gap: '10px',
          padding: '10px', marginBottom: '6px', borderRadius: '8px',
          border: '1px solid ' + (active ? 'var(--accent-cyan)' : 'var(--border)'),
          background: active ? 'var(--accent-cyan-dim)' : 'transparent',
          cursor: 'pointer'
        },
        onClick: function() {
          state.telemetryLevel = level.key;
          ipcSend('set_telemetry_level', { level: level.key });
          rerender();
        }
      });

      // Radio circle
      var radio = el('div', {
        style: {
          width: '16px', height: '16px', borderRadius: '50%', flexShrink: '0', marginTop: '1px',
          border: '2px solid ' + (active ? 'var(--accent-cyan)' : 'var(--border)'),
          background: active ? 'var(--accent-cyan)' : 'transparent',
          display: 'flex', alignItems: 'center', justifyContent: 'center'
        }
      });
      if (active) {
        radio.appendChild(el('div', { style: { width: '6px', height: '6px', borderRadius: '50%', background: '#fff' } }));
      }
      row.appendChild(radio);

      var textCol = el('div');
      textCol.appendChild(el('div', { textContent: level.label, style: { fontSize: '12px', fontWeight: '600', color: active ? 'var(--accent-cyan)' : 'var(--text-primary)' } }));
      textCol.appendChild(el('div', { textContent: level.desc, style: { fontSize: '10px', color: 'var(--text-secondary)', marginTop: '2px', lineHeight: '1.4' } }));
      row.appendChild(textCol);

      card.appendChild(row);
    });

    container.appendChild(card);
  }

  // ── Render undo history panel ─────────────────────────────────
  function renderUndoHistory(container) {
    var card = el('div', { className: 'ph-card' });
    card.appendChild(el('div', { className: 'ph-card-title', textContent: 'Undo History' }));

    if (state.undoHistory.length === 0) {
      card.appendChild(el('div', { textContent: 'No actions to undo', style: { fontSize: '11px', color: 'var(--text-dim)', padding: '8px 0' } }));
    } else {
      var list = el('div', { style: { maxHeight: '200px', overflowY: 'auto' } });
      state.undoHistory.slice().reverse().forEach(function(entry) {
        var row = el('div', {
          style: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '6px 0', borderBottom: '1px solid var(--border)', fontSize: '11px' }
        });
        var info = el('div');
        info.appendChild(el('div', { textContent: entry.action + ': ' + entry.name, style: { color: 'var(--text-primary)' } }));
        info.appendChild(el('div', { textContent: entry.time, style: { fontSize: '9px', color: 'var(--text-dim)', fontFamily: 'var(--mono)' } }));
        row.appendChild(info);

        var undoBtn = el('button', {
          className: 'btn',
          style: { padding: '2px 8px', fontSize: '9px', cursor: 'pointer' },
          textContent: 'Undo',
          onClick: (function(e) { return function() { undoAction(e); }; })(entry)
        });
        row.appendChild(undoBtn);
        list.appendChild(row);
      });
      card.appendChild(list);
    }

    container.appendChild(card);
  }

  // ── Render progress indicator ─────────────────────────────────
  function renderProgress(container) {
    if (!state.scanning) return;
    var overlay = el('div', {
      id: 'bloat-progress',
      style: {
        gridColumn: 'span 2', padding: '20px', textAlign: 'center',
        background: 'var(--bg-card)', border: '1px solid var(--border)',
        borderRadius: 'var(--radius)'
      }
    });
    overlay.appendChild(el('div', { textContent: 'Scanning for bloatware...', style: { fontSize: '13px', fontWeight: '600', color: 'var(--accent-cyan)', marginBottom: '12px' } }));

    // Progress bar
    var barOuter = el('div', { style: { width: '100%', height: '6px', background: 'var(--gauge-track)', borderRadius: '3px', overflow: 'hidden' } });
    var barInner = el('div', {
      id: 'bloat-progress-bar',
      style: { width: '0%', height: '100%', background: 'var(--accent-cyan)', borderRadius: '3px', transition: 'width 0.3s ease' }
    });
    barOuter.appendChild(barInner);
    overlay.appendChild(barOuter);
    overlay.appendChild(el('div', { id: 'bloat-progress-text', textContent: 'Initializing scan...', style: { fontSize: '10px', color: 'var(--text-dim)', marginTop: '8px' } }));
    container.appendChild(overlay);
  }

  // ── Actions ───────────────────────────────────────────────────
  function scanBloatware() {
    state.scanning = true;
    rerender();
    ipcSend('scan_bloatware');

    // Simulate scan progress for demo
    var progress = 0;
    var steps = ['Scanning OEM packages...', 'Checking Microsoft services...', 'Detecting telemetry...', 'Analyzing background processes...', 'Checking startup items...'];
    var interval = setInterval(function() {
      progress += 20;
      var bar = document.getElementById('bloat-progress-bar');
      var txt = document.getElementById('bloat-progress-text');
      if (bar) bar.style.width = progress + '%';
      if (txt) txt.textContent = steps[Math.min(Math.floor(progress / 20) - 1, steps.length - 1)] || 'Finishing...';
      if (progress >= 100) {
        clearInterval(interval);
        setTimeout(function() {
          state.scanning = false;
          state.items = JSON.parse(JSON.stringify(MOCK_ITEMS));
          computeSummary();
          rerender();
        }, 400);
      }
    }, 600);
  }

  function removeItem(item) {
    state.undoHistory.push({
      id: item.id,
      name: item.name,
      action: 'Removed',
      prevStatus: item.status,
      category: state.activeTab,
      time: new Date().toLocaleTimeString()
    });
    item.status = 'removed';
    ipcSend('remove_bloatware', { id: item.id, action: 'remove' });
    computeSummary();
    rerender();
  }

  function keepItem(item) {
    if (item.status === 'disabled') {
      item.status = 'active';
    }
    rerender();
  }

  function bulkAction(action) {
    var items = state.items[state.activeTab] || [];
    items.forEach(function(item) {
      if (item.status !== 'removed') {
        state.undoHistory.push({
          id: item.id,
          name: item.name,
          action: action === 'remove' ? 'Removed' : 'Disabled',
          prevStatus: item.status,
          category: state.activeTab,
          time: new Date().toLocaleTimeString()
        });
        item.status = action === 'remove' ? 'removed' : 'disabled';
      }
    });
    ipcSend('remove_bloatware', { category: state.activeTab, action: action });
    computeSummary();
    rerender();
  }

  function undoAction(entry) {
    var items = state.items[entry.category] || [];
    var item = items.find(function(i) { return i.id === entry.id; });
    if (item) {
      item.status = entry.prevStatus;
    }
    state.undoHistory = state.undoHistory.filter(function(e) { return e !== entry; });
    ipcSend('undo_bloatware', { id: entry.id });
    computeSummary();
    rerender();
  }

  // ── Main render ───────────────────────────────────────────────
  var rootContainer = null;

  function rerender() {
    if (!rootContainer) return;
    rootContainer.innerHTML = '';

    // Page header
    var header = el('div', { className: 'page-header' });
    header.appendChild(el('span', { className: 'page-icon', innerHTML: '&#128683;' }));
    header.appendChild(el('h2', { textContent: 'Bloatware & Telemetry Silencer' }));
    rootContainer.appendChild(header);

    rootContainer.appendChild(el('p', {
      className: 'page-desc',
      textContent: 'Identifies and removes resource-wasting bloatware, OEM preinstalls, and telemetry services. Three safety levels protect critical system components. Corporate/MDM detection prevents accidental removal of managed software.'
    }));

    // Scan button
    var scanRow = el('div', { style: { display: 'flex', gap: '10px', marginBottom: '16px', alignItems: 'center' } });
    var scanBtn = el('button', {
      className: 'btn',
      style: { padding: '8px 18px', cursor: state.scanning ? 'wait' : 'pointer', opacity: state.scanning ? '0.5' : '1' },
      textContent: state.scanning ? 'Scanning...' : 'Scan for Bloatware',
      onClick: function() { if (!state.scanning) scanBloatware(); }
    });
    scanBtn.disabled = state.scanning;
    scanRow.appendChild(scanBtn);
    rootContainer.appendChild(scanRow);

    var grid = el('div', { className: 'page-grid' });

    // Progress indicator
    renderProgress(grid);

    if (!state.scanning) {
      // Summary
      if (Object.keys(state.items).length > 0) {
        renderSummary(grid);
      }

      // Corporate warning
      renderCorporateWarning(grid);

      // Category tabs
      if (Object.keys(state.items).length > 0) {
        var tabWrapper = el('div', { style: { gridColumn: 'span 2' } });
        renderTabs(tabWrapper);
        grid.appendChild(tabWrapper);
      }
    }

    rootContainer.appendChild(grid);

    // Item list (below grid to span full width)
    if (!state.scanning && Object.keys(state.items).length > 0) {
      var listGrid = el('div', { className: 'page-grid' });
      renderItemList(listGrid);
      rootContainer.appendChild(listGrid);
    }

    // Bottom row: telemetry + undo
    if (!state.scanning) {
      var bottomGrid = el('div', { className: 'page-grid' });
      renderTelemetrySelector(bottomGrid);
      renderUndoHistory(bottomGrid);
      rootContainer.appendChild(bottomGrid);
    }
  }

  // ── Public API ────────────────────────────────────────────────
  window.RuVectorPages.Bloatware_Init = function(container) {
    rootContainer = container;
    container.innerHTML = '';
    var inner = el('div', { className: 'page-inner' });
    container.appendChild(inner);
    rootContainer = inner;

    // Load data if available
    ipcSend('get_bloatware_items');
    ipcSend('get_undo_history');

    rerender();
    state.initialized = true;
  };

  window.RuVectorPages.Bloatware_Update = function(data) {
    if (!data) return;
    if (data.items) {
      state.items = data.items;
      computeSummary();
    }
    if (data.undo_history) {
      state.undoHistory = data.undo_history;
    }
    if (data.telemetry_level) {
      state.telemetryLevel = data.telemetry_level;
    }
    if (data.corporate_detected !== undefined) {
      state.corporateDetected = data.corporate_detected;
    }
    if (data.summary) {
      state.summary = data.summary;
    }
    rerender();
  };

  // Expose callbacks for Rust IPC
  window.updateBloatwareItems = function(data) {
    window.RuVectorPages.Bloatware_Update(data);
  };

})();
