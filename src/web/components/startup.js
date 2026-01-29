/**
 * ADR-015: Startup Optimizer Component
 * PageRank-based startup impact scoring with sortable table,
 * category filters, boot time estimates, and bulk actions.
 */
(function(){
  'use strict';

  window.RuVectorPages = window.RuVectorPages || {};

  // ── State ──────────────────────────────────────────────────────
  var state = {
    items: [
      { id: 1,  name: 'Windows Defender',      category: 'system',     impact: 'high',   enabled: true,  pagerank: 0.95, memoryMb: 180 },
      { id: 2,  name: 'Audio Service',          category: 'system',     impact: 'medium', enabled: true,  pagerank: 0.88, memoryMb: 45  },
      { id: 3,  name: 'Network Manager',        category: 'system',     impact: 'high',   enabled: true,  pagerank: 0.92, memoryMb: 65  },
      { id: 4,  name: 'Bluetooth Support',       category: 'system',     impact: 'low',    enabled: true,  pagerank: 0.40, memoryMb: 22  },
      { id: 5,  name: 'Microsoft Teams',         category: 'user',       impact: 'high',   enabled: true,  pagerank: 0.35, memoryMb: 320 },
      { id: 6,  name: 'Slack',                   category: 'user',       impact: 'medium', enabled: true,  pagerank: 0.30, memoryMb: 240 },
      { id: 7,  name: 'Spotify',                 category: 'user',       impact: 'medium', enabled: true,  pagerank: 0.20, memoryMb: 180 },
      { id: 8,  name: 'Discord',                 category: 'user',       impact: 'medium', enabled: false, pagerank: 0.25, memoryMb: 210 },
      { id: 9,  name: 'OneDrive',                category: 'background', impact: 'medium', enabled: true,  pagerank: 0.55, memoryMb: 130 },
      { id: 10, name: 'Google Drive Sync',       category: 'background', impact: 'medium', enabled: true,  pagerank: 0.45, memoryMb: 95  },
      { id: 11, name: 'Dropbox',                 category: 'background', impact: 'low',    enabled: false, pagerank: 0.38, memoryMb: 110 },
      { id: 12, name: 'Adobe Creative Cloud',    category: 'background', impact: 'high',   enabled: true,  pagerank: 0.15, memoryMb: 350 },
      { id: 13, name: 'Windows Update',          category: 'updater',    impact: 'medium', enabled: true,  pagerank: 0.82, memoryMb: 55  },
      { id: 14, name: 'Chrome Updater',          category: 'updater',    impact: 'low',    enabled: true,  pagerank: 0.12, memoryMb: 30  },
      { id: 15, name: 'Java Update Scheduler',   category: 'updater',    impact: 'low',    enabled: true,  pagerank: 0.08, memoryMb: 42  },
      { id: 16, name: 'Steam Client',            category: 'user',       impact: 'medium', enabled: false, pagerank: 0.28, memoryMb: 160 },
      { id: 17, name: 'NVIDIA Container',        category: 'system',     impact: 'medium', enabled: true,  pagerank: 0.72, memoryMb: 85  },
      { id: 18, name: 'Cortana',                 category: 'background', impact: 'low',    enabled: false, pagerank: 0.10, memoryMb: 120 }
    ],
    sortCol: 'pagerank',
    sortAsc: false,
    filterCategory: 'all',
    bootTimeCurrent: 38,
    bootTimeOptimized: 22,
    undoStack: []
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

  function impactColor(impact) {
    if (impact === 'high') return '--accent-red';
    if (impact === 'medium') return '--accent-amber';
    return '--accent-green';
  }

  function categoryLabel(cat) {
    var labels = { system: 'System Critical', user: 'User Apps', background: 'Background Services', updater: 'Updaters' };
    return labels[cat] || cat;
  }

  function categoryColor(cat) {
    var colors = { system: '--accent-red', user: '--accent-cyan', background: '--accent-amber', updater: '--accent-purple' };
    return colors[cat] || '--text-dim';
  }

  // ── Sorting ────────────────────────────────────────────────────
  function sortItems(items) {
    var col = state.sortCol;
    var asc = state.sortAsc;
    var sorted = items.slice();
    sorted.sort(function(a, b) {
      var va = a[col], vb = b[col];
      if (typeof va === 'string') {
        va = va.toLowerCase();
        vb = vb.toLowerCase();
      }
      if (col === 'impact') {
        var order = { high: 3, medium: 2, low: 1 };
        va = order[a.impact] || 0;
        vb = order[b.impact] || 0;
      }
      if (col === 'enabled') {
        va = a.enabled ? 1 : 0;
        vb = b.enabled ? 1 : 0;
      }
      if (va < vb) return asc ? -1 : 1;
      if (va > vb) return asc ? 1 : -1;
      return 0;
    });
    return sorted;
  }

  function filteredItems() {
    var items = state.items;
    if (state.filterCategory !== 'all') {
      items = items.filter(function(it) { return it.category === state.filterCategory; });
    }
    return sortItems(items);
  }

  // ── Canvas: PageRank Bar Chart ─────────────────────────────────
  function drawPageRankChart(canvas) {
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

    var items = state.items.slice().sort(function(a, b) { return b.pagerank - a.pagerank; });
    var top10 = items.slice(0, 10);

    var pad = { top: 24, bottom: 8, left: 110, right: 40 };
    var plotW = W - pad.left - pad.right;
    var barH = 14;
    var gap = 6;

    // Title
    ctx.fillStyle = getCS('--text-dim');
    ctx.font = '9px ' + getCS('--font');
    ctx.textAlign = 'left';
    ctx.fillText('PAGERANK IMPORTANCE SCORES', pad.left, 14);

    top10.forEach(function(item, i) {
      var y = pad.top + i * (barH + gap);
      var barW = item.pagerank * plotW;
      var color = getCS(item.enabled ? categoryColor(item.category) : '--text-dim');

      // Name label
      ctx.fillStyle = item.enabled ? getCS('--text-secondary') : getCS('--text-dim');
      ctx.font = '10px ' + getCS('--font');
      ctx.textAlign = 'right';
      var displayName = item.name.length > 16 ? item.name.substring(0, 15) + '\u2026' : item.name;
      ctx.fillText(displayName, pad.left - 8, y + barH - 3);

      // Track
      ctx.fillStyle = getCS('--gauge-track');
      ctx.fillRect(pad.left, y, plotW, barH);

      // Bar fill
      ctx.fillStyle = color;
      ctx.globalAlpha = item.enabled ? 0.8 : 0.3;
      ctx.fillRect(pad.left, y, barW, barH);
      ctx.globalAlpha = 1;

      // Score label
      ctx.fillStyle = getCS('--text-primary');
      ctx.font = '10px ' + getCS('--mono');
      ctx.textAlign = 'left';
      ctx.fillText(item.pagerank.toFixed(2), pad.left + plotW + 6, y + barH - 3);
    });
  }

  // ── Canvas: Boot Time Comparison ───────────────────────────────
  function drawBootTime(canvas) {
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

    var pad = { top: 24, bottom: 20, left: 16, right: 16 };
    var centerX = W / 2;

    // Title
    ctx.fillStyle = getCS('--text-dim');
    ctx.font = '9px ' + getCS('--font');
    ctx.textAlign = 'center';
    ctx.fillText('BOOT TIME ESTIMATE', centerX, 14);

    // Current bar
    var barW = 60;
    var maxBarH = H - pad.top - pad.bottom - 30;
    var maxTime = Math.max(state.bootTimeCurrent, state.bootTimeOptimized, 60);

    var currentH = (state.bootTimeCurrent / maxTime) * maxBarH;
    var optH = (state.bootTimeOptimized / maxTime) * maxBarH;

    var barY = pad.top + 10;
    var currentX = centerX - barW - 12;
    var optX = centerX + 12;

    // Current
    ctx.fillStyle = getCS('--accent-amber');
    ctx.globalAlpha = 0.2;
    ctx.fillRect(currentX, barY + maxBarH - currentH, barW, currentH);
    ctx.globalAlpha = 0.8;
    ctx.fillRect(currentX, barY + maxBarH - currentH, barW, currentH);
    ctx.globalAlpha = 1;

    ctx.fillStyle = getCS('--text-primary');
    ctx.font = 'bold 16px ' + getCS('--font');
    ctx.textAlign = 'center';
    ctx.fillText(state.bootTimeCurrent + 's', currentX + barW / 2, barY + maxBarH - currentH - 8);

    ctx.fillStyle = getCS('--text-dim');
    ctx.font = '9px ' + getCS('--font');
    ctx.fillText('Current', currentX + barW / 2, H - 8);

    // Optimized
    ctx.fillStyle = getCS('--accent-green');
    ctx.globalAlpha = 0.2;
    ctx.fillRect(optX, barY + maxBarH - optH, barW, optH);
    ctx.globalAlpha = 0.8;
    ctx.fillRect(optX, barY + maxBarH - optH, barW, optH);
    ctx.globalAlpha = 1;

    ctx.fillStyle = getCS('--text-primary');
    ctx.font = 'bold 16px ' + getCS('--font');
    ctx.textAlign = 'center';
    ctx.fillText(state.bootTimeOptimized + 's', optX + barW / 2, barY + maxBarH - optH - 8);

    ctx.fillStyle = getCS('--text-dim');
    ctx.font = '9px ' + getCS('--font');
    ctx.fillText('Optimized', optX + barW / 2, H - 8);

    // Savings arrow
    var savings = state.bootTimeCurrent - state.bootTimeOptimized;
    if (savings > 0) {
      var pct = Math.round((savings / state.bootTimeCurrent) * 100);
      ctx.fillStyle = getCS('--accent-green');
      ctx.font = 'bold 11px ' + getCS('--font');
      ctx.textAlign = 'center';
      ctx.fillText('-' + savings + 's (' + pct + '%)', centerX, barY + maxBarH + 4);
    }
  }

  // ── Render: Category Tabs ──────────────────────────────────────
  function renderCategoryTabs(container) {
    var tabBar = createEl('div', '', container);
    tabBar.style.cssText = 'display:flex;gap:4px;margin-bottom:16px;flex-wrap:wrap;';

    var categories = [
      { id: 'all', label: 'All Items' },
      { id: 'system', label: 'System Critical' },
      { id: 'user', label: 'User Apps' },
      { id: 'background', label: 'Background Services' },
      { id: 'updater', label: 'Updaters' }
    ];

    categories.forEach(function(cat) {
      var tab = createEl('button', '', tabBar);
      var isActive = state.filterCategory === cat.id;
      tab.style.cssText = 'padding:6px 14px;border-radius:6px;font-size:11px;font-family:var(--font);cursor:pointer;transition:all 0.2s;border:1px solid ' + (isActive ? 'var(--accent-cyan)' : 'var(--border)') + ';background:' + (isActive ? 'var(--accent-cyan-dim)' : 'var(--bg-card)') + ';color:' + (isActive ? 'var(--accent-cyan)' : 'var(--text-secondary)') + ';font-weight:' + (isActive ? '600' : '400') + ';';
      setText(tab, cat.label);

      // Count badge
      var count = cat.id === 'all' ? state.items.length : state.items.filter(function(it) { return it.category === cat.id; }).length;
      var badge = createEl('span', '', tab);
      badge.style.cssText = 'margin-left:6px;font-size:9px;opacity:0.7;';
      setText(badge, '(' + count + ')');

      tab.addEventListener('click', function() {
        state.filterCategory = cat.id;
        render();
      });
    });
  }

  // ── Render: Startup Items Table ────────────────────────────────
  function renderTable(container) {
    var card = createEl('div', 'card', container);
    card.style.cssText = 'grid-column:1/-1;padding:0;overflow:hidden;';

    var table = createEl('table', '', card);
    table.style.cssText = 'width:100%;border-collapse:collapse;font-size:11px;';

    // Header
    var thead = createEl('thead', '', table);
    var hrow = createEl('tr', '', thead);
    hrow.style.cssText = 'background:var(--bg-secondary);';

    var columns = [
      { key: 'name',     label: 'Name',        width: '' },
      { key: 'category', label: 'Category',     width: '120px' },
      { key: 'impact',   label: 'Impact',       width: '80px' },
      { key: 'enabled',  label: 'Status',       width: '80px' },
      { key: 'pagerank', label: 'PageRank',     width: '90px' },
      { key: 'memoryMb', label: 'Memory',       width: '80px' },
      { key: 'action',   label: '',             width: '70px' }
    ];

    columns.forEach(function(col) {
      var th = createEl('th', '', hrow);
      th.style.cssText = 'padding:10px 12px;text-align:left;font-size:10px;font-weight:600;letter-spacing:0.5px;text-transform:uppercase;color:var(--text-dim);border-bottom:1px solid var(--border);cursor:' + (col.key !== 'action' ? 'pointer' : 'default') + ';white-space:nowrap;user-select:none;';
      if (col.width) th.style.width = col.width;

      setText(th, col.label);

      if (col.key !== 'action') {
        // Sort indicator
        if (state.sortCol === col.key) {
          var arrow = document.createTextNode(state.sortAsc ? ' \u25B2' : ' \u25BC');
          th.appendChild(arrow);
          th.style.color = 'var(--accent-cyan)';
        }
        th.addEventListener('click', (function(k) {
          return function() {
            if (state.sortCol === k) {
              state.sortAsc = !state.sortAsc;
            } else {
              state.sortCol = k;
              state.sortAsc = k === 'name' || k === 'category';
            }
            render();
          };
        })(col.key));
      }
    });

    // Body
    var tbody = createEl('tbody', '', table);
    var items = filteredItems();

    if (!items.length) {
      var emptyRow = createEl('tr', '', tbody);
      var emptyTd = createEl('td', '', emptyRow);
      emptyTd.colSpan = columns.length;
      emptyTd.style.cssText = 'padding:24px;text-align:center;color:var(--text-dim);font-size:12px;';
      setText(emptyTd, 'No items in this category');
      return;
    }

    items.forEach(function(item) {
      var tr = createEl('tr', '', tbody);
      tr.style.cssText = 'transition:background 0.2s;border-bottom:1px solid var(--border);';
      tr.addEventListener('mouseenter', function() { this.style.background = 'var(--bg-card-hover)'; });
      tr.addEventListener('mouseleave', function() { this.style.background = ''; });

      // Name
      var tdName = createEl('td', '', tr);
      tdName.style.cssText = 'padding:8px 12px;color:var(--text-primary);font-weight:500;';
      if (!item.enabled) tdName.style.opacity = '0.5';
      setText(tdName, item.name);

      // Category
      var tdCat = createEl('td', '', tr);
      tdCat.style.padding = '8px 12px';
      var catBadge = createEl('span', '', tdCat);
      catBadge.style.cssText = 'font-size:9px;padding:2px 8px;border-radius:4px;font-weight:500;background:var(--bg-primary);color:var(' + categoryColor(item.category) + ');';
      setText(catBadge, categoryLabel(item.category));

      // Impact
      var tdImpact = createEl('td', '', tr);
      tdImpact.style.padding = '8px 12px';
      var impBadge = createEl('span', '', tdImpact);
      var ic = impactColor(item.impact);
      impBadge.style.cssText = 'font-size:9px;padding:2px 8px;border-radius:4px;font-weight:600;text-transform:uppercase;letter-spacing:0.5px;color:var(' + ic + ');background:var(--bg-primary);';
      setText(impBadge, item.impact);

      // Status
      var tdStatus = createEl('td', '', tr);
      tdStatus.style.padding = '8px 12px';
      var statusDot = createEl('span', '', tdStatus);
      statusDot.style.cssText = 'display:inline-block;width:8px;height:8px;border-radius:50%;margin-right:6px;background:var(' + (item.enabled ? '--accent-green' : '--text-dim') + ');';
      var statusText = document.createTextNode(item.enabled ? 'Enabled' : 'Disabled');
      tdStatus.appendChild(statusText);
      tdStatus.style.cssText += 'font-size:11px;color:var(' + (item.enabled ? '--text-secondary' : '--text-dim') + ');';

      // PageRank - mini bar
      var tdPR = createEl('td', '', tr);
      tdPR.style.cssText = 'padding:8px 12px;';
      var prWrap = createEl('div', '', tdPR);
      prWrap.style.cssText = 'display:flex;align-items:center;gap:6px;';
      var prBar = createEl('div', '', prWrap);
      prBar.style.cssText = 'flex:1;height:6px;background:var(--gauge-track);border-radius:3px;overflow:hidden;';
      var prFill = createEl('div', '', prBar);
      prFill.style.cssText = 'height:100%;border-radius:3px;background:var(--accent-cyan);width:' + (item.pagerank * 100) + '%;';
      var prVal = createEl('span', '', prWrap);
      prVal.style.cssText = 'font-size:10px;font-family:var(--mono);color:var(--text-primary);width:30px;text-align:right;';
      setText(prVal, item.pagerank.toFixed(2));

      // Memory
      var tdMem = createEl('td', '', tr);
      tdMem.style.cssText = 'padding:8px 12px;font-family:var(--mono);font-size:10px;color:var(--accent-amber);';
      setText(tdMem, item.memoryMb + ' MB');

      // Action toggle
      var tdAction = createEl('td', '', tr);
      tdAction.style.cssText = 'padding:8px 12px;text-align:center;';
      var tog = createEl('div', 'toggle', tdAction);
      if (item.enabled) tog.classList.add('on');
      tog.style.cssText += 'display:inline-block;';
      tog.addEventListener('click', (function(itemId) {
        return function() {
          toggleItem(itemId);
        };
      })(item.id));
    });
  }

  // ── Render: Stats Cards ────────────────────────────────────────
  function renderStats(container) {
    var grid = createEl('div', '', container);
    grid.style.cssText = 'display:grid;grid-template-columns:repeat(4,1fr);gap:12px;margin-bottom:16px;';

    var enabledCount = state.items.filter(function(i) { return i.enabled; }).length;
    var disabledCount = state.items.length - enabledCount;
    var highImpact = state.items.filter(function(i) { return i.impact === 'high' && i.enabled; }).length;
    var totalMemory = state.items.filter(function(i) { return i.enabled; }).reduce(function(s, i) { return s + i.memoryMb; }, 0);

    var stats = [
      { label: 'Total Items', value: state.items.length.toString(), color: '--accent-cyan' },
      { label: 'Enabled', value: enabledCount.toString(), color: '--accent-green' },
      { label: 'High Impact', value: highImpact.toString(), color: '--accent-red' },
      { label: 'Boot Memory', value: totalMemory + ' MB', color: '--accent-amber' }
    ];

    stats.forEach(function(st) {
      var card = createEl('div', 'card', grid);
      card.style.textAlign = 'center';
      var val = createEl('div', '', card);
      val.style.cssText = 'font-size:24px;font-weight:700;color:var(' + st.color + ');line-height:1;margin-bottom:4px;';
      setText(val, st.value);
      var lbl = createEl('div', '', card);
      lbl.style.cssText = 'font-size:9px;color:var(--text-dim);text-transform:uppercase;letter-spacing:1px;';
      setText(lbl, st.label);
    });
  }

  // ── Render: Boot Time & PageRank Charts ────────────────────────
  function renderCharts(container) {
    var grid = createEl('div', 'page-grid', container);

    // Boot time chart
    var bootCard = createEl('div', 'card', grid);
    var bootWrap = createEl('div', '', bootCard);
    bootWrap.style.cssText = 'height:180px;border-radius:8px;overflow:hidden;';
    var bootCanvas = createEl('canvas', '', bootWrap);
    bootCanvas.id = 'startup-boot-canvas';
    bootCanvas.style.cssText = 'width:100%;height:100%;display:block;';

    // PageRank chart
    var prCard = createEl('div', 'card', grid);
    prCard.style.gridColumn = 'span 2';
    var prWrap = createEl('div', '', prCard);
    prWrap.style.cssText = 'height:260px;border-radius:8px;overflow:hidden;';
    var prCanvas = createEl('canvas', '', prWrap);
    prCanvas.id = 'startup-pagerank-canvas';
    prCanvas.style.cssText = 'width:100%;height:100%;display:block;';

    setTimeout(function() {
      drawBootTime(bootCanvas);
      drawPageRankChart(prCanvas);
    }, 50);
  }

  // ── Render: Bulk Actions ───────────────────────────────────────
  function renderActions(container) {
    var bar = createEl('div', '', container);
    bar.style.cssText = 'display:flex;gap:8px;margin-bottom:16px;flex-wrap:wrap;align-items:center;';

    var disableBtn = createEl('button', 'btn', bar);
    disableBtn.style.cssText = 'padding:8px 16px;background:var(--accent-amber-dim);border-color:var(--accent-amber);color:var(--accent-amber);font-weight:600;';
    setText(disableBtn, 'Disable All Non-Essential');
    disableBtn.addEventListener('click', function() {
      saveUndo();
      state.items.forEach(function(it) {
        if (it.category !== 'system') {
          it.enabled = false;
        }
      });
      recalcBootTime();
      ipcSend('optimize_startup');
      render();
      if (typeof showToast === 'function') {
        showToast('Startup Optimized', 'Disabled non-essential startup items', 'success');
      }
    });

    var resetBtn = createEl('button', 'btn', bar);
    setText(resetBtn, 'Reset to Defaults');
    resetBtn.addEventListener('click', function() {
      saveUndo();
      state.items.forEach(function(it) { it.enabled = true; });
      recalcBootTime();
      ipcSend('reset_startup');
      render();
    });

    var undoBtn = createEl('button', 'btn', bar);
    undoBtn.style.cssText += 'margin-left:auto;';
    if (!state.undoStack.length) {
      undoBtn.disabled = true;
      undoBtn.style.opacity = '0.4';
    }
    setText(undoBtn, 'Undo Last Change');
    undoBtn.addEventListener('click', function() {
      if (state.undoStack.length) {
        var prev = state.undoStack.pop();
        state.items = prev;
        recalcBootTime();
        render();
        if (typeof showToast === 'function') {
          showToast('Undone', 'Reverted last startup change', 'success');
        }
      }
    });

    var analyzeBtn = createEl('button', 'btn', bar);
    analyzeBtn.style.cssText = 'padding:8px 16px;background:var(--accent-cyan-dim);border-color:var(--accent-cyan);color:var(--accent-cyan);font-weight:600;';
    setText(analyzeBtn, 'Analyze Boot');
    analyzeBtn.addEventListener('click', function() {
      ipcSend('get_boot_estimate');
      ipcSend('get_startup_items');
      if (typeof showToast === 'function') {
        showToast('Analyzing', 'Running boot analysis...', 'success');
      }
    });
  }

  // ── Toggle Item ────────────────────────────────────────────────
  function toggleItem(id) {
    saveUndo();
    var item = state.items.find(function(it) { return it.id === id; });
    if (item) {
      item.enabled = !item.enabled;
      ipcSend('set_startup_item', { itemId: id, enabled: item.enabled });
      recalcBootTime();
      render();
    }
  }

  // ── Undo Support ───────────────────────────────────────────────
  function saveUndo() {
    var snapshot = state.items.map(function(it) {
      return Object.assign({}, it);
    });
    state.undoStack.push(snapshot);
    if (state.undoStack.length > 20) state.undoStack.shift();
  }

  // ── Recalculate boot time estimate ─────────────────────────────
  function recalcBootTime() {
    var enabledMem = state.items.filter(function(i) { return i.enabled; }).reduce(function(s, i) { return s + i.memoryMb; }, 0);
    var totalMem = state.items.reduce(function(s, i) { return s + i.memoryMb; }, 0);
    state.bootTimeCurrent = Math.round(20 + (enabledMem / totalMem) * 30);
    var essentialMem = state.items.filter(function(i) { return i.category === 'system'; }).reduce(function(s, i) { return s + i.memoryMb; }, 0);
    state.bootTimeOptimized = Math.round(15 + (essentialMem / totalMem) * 15);
  }

  // ── Full Render ────────────────────────────────────────────────
  function render() {
    var container = document.getElementById('page-startup');
    if (!container) return;

    container.innerHTML = '';
    var inner = createEl('div', 'page-inner', container);
    inner.style.overflowY = 'auto';
    inner.style.height = '100%';

    // Header
    var header = createEl('div', 'page-header', inner);
    var icon = createEl('span', 'page-icon', header);
    setText(icon, '\u26A1');
    var h2 = createEl('h2', '', header);
    setText(h2, 'Startup Optimizer');

    var statusBadge = createEl('div', '', inner);
    statusBadge.style.cssText = 'display:inline-block;font-size:9px;font-weight:600;letter-spacing:1px;text-transform:uppercase;padding:3px 10px;border-radius:4px;margin-bottom:16px;background:var(--accent-cyan-dim);color:var(--accent-cyan);';
    setText(statusBadge, 'ADR-015 \u00B7 Active');

    var desc = createEl('p', 'page-desc', inner);
    setText(desc, 'PageRank-based startup impact scoring. Analyzes boot-time programs, ranks them by dependency chains and resource cost, and applies staggered boot tiers to minimize login-to-ready time.');

    renderStats(inner);
    renderActions(inner);
    renderCategoryTabs(inner);
    renderTable(inner);
    renderCharts(inner);
  }

  // ── Init ───────────────────────────────────────────────────────
  window.RuVectorPages.startupInit = function(container) {
    recalcBootTime();
    render();
    ipcSend('get_startup_items');
    ipcSend('get_boot_estimate');
  };

  // ── Update (called from Rust IPC) ──────────────────────────────
  window.RuVectorPages.startupUpdate = function(data) {
    if (!data) return;
    if (data.items && Array.isArray(data.items)) {
      state.items = data.items;
    }
    if (typeof data.bootTimeCurrent === 'number') {
      state.bootTimeCurrent = data.bootTimeCurrent;
    }
    if (typeof data.bootTimeOptimized === 'number') {
      state.bootTimeOptimized = data.bootTimeOptimized;
    }
    recalcBootTime();
    render();
  };

  // ── Resize handler ─────────────────────────────────────────────
  window.addEventListener('resize', function() {
    var bootCanvas = document.getElementById('startup-boot-canvas');
    if (bootCanvas && bootCanvas.offsetParent) drawBootTime(bootCanvas);
    var prCanvas = document.getElementById('startup-pagerank-canvas');
    if (prCanvas && prCanvas.offsetParent) drawPageRankChart(prCanvas);
  });

})();
