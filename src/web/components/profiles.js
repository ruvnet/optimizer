/**
 * ADR-013: Workspace Profiles Component
 * Context-aware memory optimization profiles with auto-detection,
 * profile switching timeline, and custom profile editor.
 */
(function(){
  'use strict';

  window.RuVectorPages = window.RuVectorPages || {};

  // ── State ──────────────────────────────────────────────────────
  var state = {
    profiles: [
      { id: 'dev',      name: 'Development', icon: '\u{1F4BB}', color: '--accent-cyan',   memAlloc: { ide: 60, build: 25, browser: 10, system: 5 }, priority: 'above_normal', autoDetectApps: ['code.exe','devenv.exe','rider64.exe','idea64.exe'] },
      { id: 'gaming',   name: 'Gaming',      icon: '\u{1F3AE}', color: '--accent-green',  memAlloc: { game: 70, gpu: 15, voice: 10, system: 5 },    priority: 'high',         autoDetectApps: ['steam.exe','epicgameslauncher.exe'] },
      { id: 'creative', name: 'Creative',    icon: '\u{1F3A8}', color: '--accent-amber',  memAlloc: { editor: 55, render: 30, preview: 10, system: 5 }, priority: 'above_normal', autoDetectApps: ['photoshop.exe','premiere.exe','blender.exe'] },
      { id: 'office',   name: 'Office',      icon: '\u{1F4C4}', color: '--accent-purple', memAlloc: { browser: 45, office: 35, comms: 15, system: 5 },  priority: 'normal',       autoDetectApps: ['outlook.exe','teams.exe','winword.exe'] },
      { id: 'server',   name: 'Server',      icon: '\u{1F5A5}', color: '--accent-red',    memAlloc: { services: 50, db: 30, cache: 15, system: 5 },     priority: 'realtime',     autoDetectApps: ['docker.exe','node.exe','postgres.exe'] }
    ],
    activeId: 'dev',
    settings: {
      autoDetect: true,
      detectionMethod: 'foreground',
      switchDelay: 5,
      hotkey: 'Ctrl+Shift+P'
    },
    foregroundApp: 'code.exe',
    history: [],
    customEditing: null,
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

  // ── Generate simulated history for demo ────────────────────────
  function generateHistory() {
    var hist = [];
    var ids = ['dev','gaming','creative','office','dev','dev','gaming','dev','office','dev','creative','dev'];
    var now = Date.now();
    for (var i = ids.length - 1; i >= 0; i--) {
      hist.push({
        profileId: ids[i],
        timestamp: now - (ids.length - 1 - i) * 2 * 3600000
      });
    }
    state.history = hist;
  }

  // ── Canvas: Profile Switching Timeline ─────────────────────────
  function drawTimeline(canvas) {
    var ctx = canvas.getContext('2d');
    var dpr = window.devicePixelRatio || 1;
    var rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.scale(dpr, dpr);
    var W = rect.width;
    var H = rect.height;

    // Background
    ctx.fillStyle = getCS('--bg-primary');
    ctx.fillRect(0, 0, W, H);

    if (!state.history.length) {
      ctx.fillStyle = getCS('--text-dim');
      ctx.font = '11px ' + getCS('--font');
      ctx.textAlign = 'center';
      ctx.fillText('No history data', W / 2, H / 2);
      return;
    }

    var padding = { top: 24, bottom: 28, left: 50, right: 16 };
    var plotW = W - padding.left - padding.right;
    var plotH = H - padding.top - padding.bottom;

    // Time axis
    var minT = state.history[0].timestamp;
    var maxT = state.history[state.history.length - 1].timestamp;
    var range = maxT - minT || 1;

    // Profile color map
    var colorMap = {};
    var yMap = {};
    state.profiles.forEach(function(p, idx) {
      colorMap[p.id] = getCS(p.color);
      yMap[p.id] = idx;
    });

    // Grid lines
    ctx.strokeStyle = getCS('--border');
    ctx.lineWidth = 1;
    ctx.setLineDash([3, 3]);
    state.profiles.forEach(function(p, idx) {
      var y = padding.top + (idx / (state.profiles.length - 1)) * plotH;
      ctx.beginPath();
      ctx.moveTo(padding.left, y);
      ctx.lineTo(W - padding.right, y);
      ctx.stroke();
      // Label
      ctx.fillStyle = getCS('--text-dim');
      ctx.font = '9px ' + getCS('--font');
      ctx.textAlign = 'right';
      ctx.fillText(p.name.substring(0, 6), padding.left - 6, y + 3);
    });
    ctx.setLineDash([]);

    // Draw timeline segments
    for (var i = 0; i < state.history.length; i++) {
      var entry = state.history[i];
      var x = padding.left + ((entry.timestamp - minT) / range) * plotW;
      var yIdx = yMap[entry.profileId] !== undefined ? yMap[entry.profileId] : 0;
      var y = padding.top + (yIdx / (state.profiles.length - 1)) * plotH;

      // Dot
      ctx.beginPath();
      ctx.arc(x, y, 5, 0, Math.PI * 2);
      ctx.fillStyle = colorMap[entry.profileId] || getCS('--accent-cyan');
      ctx.fill();

      // Line to next
      if (i < state.history.length - 1) {
        var next = state.history[i + 1];
        var nx = padding.left + ((next.timestamp - minT) / range) * plotW;
        var nIdx = yMap[next.profileId] !== undefined ? yMap[next.profileId] : 0;
        var ny = padding.top + (nIdx / (state.profiles.length - 1)) * plotH;
        ctx.strokeStyle = colorMap[entry.profileId] || getCS('--accent-cyan');
        ctx.lineWidth = 2;
        ctx.globalAlpha = 0.4;
        ctx.beginPath();
        ctx.moveTo(x, y);
        ctx.lineTo(nx, ny);
        ctx.stroke();
        ctx.globalAlpha = 1;
      }
    }

    // Time labels
    ctx.fillStyle = getCS('--text-dim');
    ctx.font = '9px ' + getCS('--mono');
    ctx.textAlign = 'center';
    for (var t = 0; t <= 4; t++) {
      var tx = padding.left + (t / 4) * plotW;
      var tTime = new Date(minT + (t / 4) * range);
      ctx.fillText(tTime.getHours().toString().padStart(2, '0') + ':' + tTime.getMinutes().toString().padStart(2, '0'), tx, H - 8);
    }

    // Title
    ctx.fillStyle = getCS('--text-dim');
    ctx.font = '9px ' + getCS('--font');
    ctx.textAlign = 'left';
    ctx.fillText('PROFILE SWITCHING TIMELINE (24H)', padding.left, 14);
  }

  // ── Render: Active Profile Card ────────────────────────────────
  function renderActiveProfile(container) {
    var card = createEl('div', 'card', container);
    card.style.cssText = 'text-align:center;padding:24px 14px;';

    var prof = state.profiles.find(function(p) { return p.id === state.activeId; }) || state.profiles[0];

    var iconEl = createEl('div', '', card);
    iconEl.style.cssText = 'font-size:48px;line-height:1;margin-bottom:8px;';
    setText(iconEl, prof.icon);

    var nameEl = createEl('div', '', card);
    nameEl.style.cssText = 'font-size:20px;font-weight:700;color:var(' + prof.color + ');margin-bottom:4px;';
    setText(nameEl, prof.name);

    var subEl = createEl('div', '', card);
    subEl.style.cssText = 'font-size:10px;color:var(--text-dim);text-transform:uppercase;letter-spacing:1px;margin-bottom:12px;';
    setText(subEl, 'Active Profile');

    var appEl = createEl('div', '', card);
    appEl.style.cssText = 'font-size:11px;color:var(--text-secondary);padding:6px 12px;background:var(--bg-primary);border-radius:6px;display:inline-block;';
    appEl.id = 'profiles-fg-app';
    setText(appEl, 'Foreground: ' + state.foregroundApp);
  }

  // ── Render: Profile Selector ───────────────────────────────────
  function renderProfileSelector(container) {
    var card = createEl('div', 'card', container);
    var title = createEl('div', 'card-title', card);
    setText(title, 'Available Profiles');

    state.profiles.forEach(function(prof) {
      var row = createEl('div', '', card);
      row.style.cssText = 'display:flex;align-items:center;gap:10px;padding:8px 0;border-bottom:1px solid var(--border);cursor:pointer;transition:background 0.2s;';
      if (prof.id === state.activeId) {
        row.style.background = 'var(--accent-cyan-dim)';
        row.style.margin = '0 -14px';
        row.style.padding = '8px 14px';
        row.style.borderRadius = '6px';
      }

      var dot = createEl('span', '', row);
      dot.style.cssText = 'width:10px;height:10px;border-radius:50%;flex-shrink:0;background:var(' + prof.color + ');';

      var icon = createEl('span', '', row);
      icon.style.fontSize = '16px';
      setText(icon, prof.icon);

      var nameWrap = createEl('div', '', row);
      nameWrap.style.flex = '1';
      var nm = createEl('div', '', nameWrap);
      nm.style.cssText = 'font-size:12px;color:var(--text-primary);font-weight:500;';
      setText(nm, prof.name);
      var desc = createEl('div', '', nameWrap);
      desc.style.cssText = 'font-size:9px;color:var(--text-dim);';
      setText(desc, 'Priority: ' + prof.priority);

      if (prof.id === state.activeId) {
        var badge = createEl('span', '', row);
        badge.style.cssText = 'font-size:9px;padding:2px 8px;border-radius:4px;background:var(--accent-cyan-dim);color:var(--accent-cyan);font-weight:600;';
        setText(badge, 'ACTIVE');
      } else {
        var btn = createEl('button', 'btn', row);
        btn.style.cssText = 'padding:4px 10px;font-size:10px;';
        setText(btn, 'Switch');
        btn.addEventListener('click', (function(pid) {
          return function(e) {
            e.stopPropagation();
            switchProfile(pid);
          };
        })(prof.id));
      }
    });
  }

  // ── Render: Memory Allocation Bars ─────────────────────────────
  function renderMemoryAlloc(container) {
    var card = createEl('div', 'card', container);
    var title = createEl('div', 'card-title', card);
    setText(title, 'Memory Allocation');

    var prof = state.profiles.find(function(p) { return p.id === state.activeId; }) || state.profiles[0];
    var alloc = prof.memAlloc;
    var colors = ['--accent-cyan', '--accent-amber', '--accent-green', '--text-dim', '--accent-purple'];
    var idx = 0;

    Object.keys(alloc).forEach(function(key) {
      var row = createEl('div', 'ph-bar-row', card);

      var label = createEl('span', 'ph-bar-label', row);
      setText(label, key.charAt(0).toUpperCase() + key.slice(1));

      var bar = createEl('div', 'ph-bar', row);
      var fill = createEl('div', 'ph-bar-fill', bar);
      fill.style.width = alloc[key] + '%';
      fill.style.background = 'var(' + colors[idx % colors.length] + ')';

      var val = createEl('span', 'ph-bar-value', row);
      setText(val, alloc[key] + '%');

      idx++;
    });
  }

  // ── Render: Settings Card ──────────────────────────────────────
  function renderSettings(container) {
    var card = createEl('div', 'card', container);
    var title = createEl('div', 'card-title', card);
    setText(title, 'Profile Settings');

    // Auto-detect toggle
    var autoRow = createEl('div', 'toggle-row', card);
    var autoLabel = createEl('span', '', autoRow);
    setText(autoLabel, 'Auto-Detect');
    var autoTog = createEl('div', 'toggle', autoRow);
    autoTog.id = 'profiles-auto-detect';
    if (state.settings.autoDetect) autoTog.classList.add('on');
    autoTog.addEventListener('click', function() {
      state.settings.autoDetect = !state.settings.autoDetect;
      autoTog.classList.toggle('on');
      ipcSend('set_profile_setting', { key: 'autoDetect', value: state.settings.autoDetect });
    });

    // Detection method
    var methodRow = createEl('div', 'select-row', card);
    methodRow.style.cssText = 'padding:5px 0;border-top:1px solid var(--border);';
    var methodLabel = createEl('span', '', methodRow);
    methodLabel.style.cssText = 'font-size:11px;color:var(--text-secondary);';
    setText(methodLabel, 'Detection Method');
    var methodSel = createEl('select', '', methodRow);
    methodSel.style.cssText = 'background:var(--bg-primary);color:var(--text-primary);border:1px solid var(--border);border-radius:4px;padding:2px 6px;font-size:11px;font-family:var(--font);cursor:pointer;';
    ['foreground', 'process_list', 'window_title', 'gpu_usage'].forEach(function(m) {
      var opt = createEl('option', '', methodSel);
      opt.value = m;
      setText(opt, m.replace(/_/g, ' '));
      if (m === state.settings.detectionMethod) opt.selected = true;
    });
    methodSel.addEventListener('change', function() {
      state.settings.detectionMethod = this.value;
      ipcSend('set_profile_setting', { key: 'detectionMethod', value: this.value });
    });

    // Switch delay slider
    var delayRow = createEl('div', '', card);
    delayRow.style.cssText = 'padding:8px 0;border-top:1px solid var(--border);';
    var delayHeader = createEl('div', '', delayRow);
    delayHeader.style.cssText = 'display:flex;justify-content:space-between;align-items:center;margin-bottom:4px;';
    var delayLabel = createEl('span', '', delayHeader);
    delayLabel.style.cssText = 'font-size:11px;color:var(--text-secondary);';
    setText(delayLabel, 'Switch Delay');
    var delayVal = createEl('span', '', delayHeader);
    delayVal.style.cssText = 'font-size:11px;color:var(--text-primary);font-family:var(--mono);';
    delayVal.id = 'profiles-delay-val';
    setText(delayVal, state.settings.switchDelay + 's');

    var delaySlider = createEl('input', '', delayRow);
    delaySlider.type = 'range';
    delaySlider.min = '0';
    delaySlider.max = '30';
    delaySlider.value = state.settings.switchDelay;
    delaySlider.style.cssText = 'width:100%;accent-color:var(--accent-cyan);cursor:pointer;';
    delaySlider.addEventListener('input', function() {
      state.settings.switchDelay = parseInt(this.value);
      var valEl = document.getElementById('profiles-delay-val');
      if (valEl) setText(valEl, this.value + 's');
    });
    delaySlider.addEventListener('change', function() {
      ipcSend('set_profile_setting', { key: 'switchDelay', value: state.settings.switchDelay });
    });

    // Hotkey display
    var hkRow = createEl('div', '', card);
    hkRow.style.cssText = 'display:flex;align-items:center;justify-content:space-between;padding:5px 0;border-top:1px solid var(--border);font-size:11px;color:var(--text-secondary);';
    var hkLabel = createEl('span', '', hkRow);
    setText(hkLabel, 'Hotkey');
    var hkVal = createEl('span', '', hkRow);
    hkVal.style.cssText = 'font-family:var(--mono);font-size:10px;padding:2px 8px;background:var(--bg-primary);border-radius:4px;color:var(--text-primary);';
    setText(hkVal, state.settings.hotkey);
  }

  // ── Render: Timeline Chart Card ────────────────────────────────
  function renderTimelineCard(container) {
    var card = createEl('div', 'card', container);
    card.style.gridColumn = '1 / -1';
    var title = createEl('div', 'card-title', card);
    setText(title, 'Profile Switching Timeline (24h)');

    var canvasWrap = createEl('div', '', card);
    canvasWrap.style.cssText = 'height:160px;border-radius:8px;overflow:hidden;';
    var canvas = createEl('canvas', '', canvasWrap);
    canvas.id = 'profiles-timeline-canvas';
    canvas.style.cssText = 'width:100%;height:100%;display:block;';

    setTimeout(function() { drawTimeline(canvas); }, 50);
  }

  // ── Render: Custom Profile Editor ──────────────────────────────
  function renderEditor(container) {
    var card = createEl('div', 'card', container);
    card.style.gridColumn = '1 / -1';
    var title = createEl('div', 'card-title', card);
    setText(title, 'Custom Profile Editor');

    var form = createEl('div', '', card);
    form.style.cssText = 'display:grid;grid-template-columns:1fr 1fr;gap:12px;';

    // Name input
    var nameWrap = createEl('div', '', form);
    var nameLabel = createEl('label', '', nameWrap);
    nameLabel.style.cssText = 'font-size:10px;color:var(--text-dim);text-transform:uppercase;letter-spacing:0.5px;display:block;margin-bottom:4px;';
    setText(nameLabel, 'Profile Name');
    var nameInput = createEl('input', '', nameWrap);
    nameInput.type = 'text';
    nameInput.placeholder = 'My Custom Profile';
    nameInput.id = 'profiles-custom-name';
    nameInput.style.cssText = 'width:100%;padding:6px 10px;background:var(--bg-primary);color:var(--text-primary);border:1px solid var(--border);border-radius:6px;font-size:12px;font-family:var(--font);';

    // Priority select
    var priWrap = createEl('div', '', form);
    var priLabel = createEl('label', '', priWrap);
    priLabel.style.cssText = 'font-size:10px;color:var(--text-dim);text-transform:uppercase;letter-spacing:0.5px;display:block;margin-bottom:4px;';
    setText(priLabel, 'Process Priority');
    var priSel = createEl('select', '', priWrap);
    priSel.id = 'profiles-custom-priority';
    priSel.style.cssText = 'width:100%;padding:6px 10px;background:var(--bg-primary);color:var(--text-primary);border:1px solid var(--border);border-radius:6px;font-size:12px;font-family:var(--font);cursor:pointer;';
    ['idle', 'below_normal', 'normal', 'above_normal', 'high', 'realtime'].forEach(function(p) {
      var opt = createEl('option', '', priSel);
      opt.value = p;
      setText(opt, p.replace(/_/g, ' '));
    });

    // Memory sliders
    var sliderSection = createEl('div', '', form);
    sliderSection.style.gridColumn = '1 / -1';
    var slLabel = createEl('div', '', sliderSection);
    slLabel.style.cssText = 'font-size:10px;color:var(--text-dim);text-transform:uppercase;letter-spacing:0.5px;margin-bottom:8px;';
    setText(slLabel, 'Memory Allocation');

    var categories = ['Primary App', 'Secondary', 'Background', 'System Reserve'];
    var sliderValues = [50, 25, 15, 10];
    var sliderColors = ['--accent-cyan', '--accent-amber', '--accent-green', '--text-dim'];

    categories.forEach(function(cat, i) {
      var row = createEl('div', '', sliderSection);
      row.style.cssText = 'display:flex;align-items:center;gap:10px;margin-bottom:6px;';

      var catLabel = createEl('span', '', row);
      catLabel.style.cssText = 'font-size:11px;color:var(--text-secondary);width:100px;flex-shrink:0;';
      setText(catLabel, cat);

      var slider = createEl('input', '', row);
      slider.type = 'range';
      slider.min = '0';
      slider.max = '100';
      slider.value = sliderValues[i];
      slider.style.cssText = 'flex:1;accent-color:var(' + sliderColors[i] + ');cursor:pointer;';

      var valSpan = createEl('span', '', row);
      valSpan.style.cssText = 'font-size:11px;color:var(--text-primary);font-family:var(--mono);width:36px;text-align:right;';
      setText(valSpan, sliderValues[i] + '%');

      slider.addEventListener('input', function() {
        setText(valSpan, this.value + '%');
      });
    });

    // Buttons
    var btnRow = createEl('div', '', form);
    btnRow.style.cssText = 'grid-column:1/-1;display:flex;gap:8px;margin-top:4px;';

    var saveBtn = createEl('button', 'btn', btnRow);
    saveBtn.style.cssText = 'padding:8px 20px;background:var(--accent-cyan-dim);border-color:var(--accent-cyan);color:var(--accent-cyan);font-weight:600;';
    setText(saveBtn, 'Create Profile');
    saveBtn.addEventListener('click', function() {
      var nameEl = document.getElementById('profiles-custom-name');
      var priEl = document.getElementById('profiles-custom-priority');
      if (nameEl && nameEl.value.trim()) {
        ipcSend('create_profile', {
          name: nameEl.value.trim(),
          priority: priEl ? priEl.value : 'normal'
        });
        if (typeof showToast === 'function') {
          showToast('Profile Created', 'Created profile: ' + nameEl.value.trim(), 'success');
        }
        nameEl.value = '';
      }
    });

    var importBtn = createEl('button', 'btn', btnRow);
    setText(importBtn, 'Import Profile');
    importBtn.addEventListener('click', function() {
      ipcSend('import_profile');
    });

    var exportBtn = createEl('button', 'btn', btnRow);
    setText(exportBtn, 'Export Current');
    exportBtn.addEventListener('click', function() {
      ipcSend('export_profile', { profileId: state.activeId });
    });

    var deleteBtn = createEl('button', 'btn', btnRow);
    deleteBtn.style.cssText = 'padding:8px 16px;border-color:var(--accent-red);color:var(--accent-red);margin-left:auto;';
    setText(deleteBtn, 'Delete Selected');
    deleteBtn.addEventListener('click', function() {
      if (state.activeId !== 'dev') {
        ipcSend('delete_profile', { profileId: state.activeId });
      }
    });
  }

  // ── Switch Profile ─────────────────────────────────────────────
  function switchProfile(id) {
    state.activeId = id;
    ipcSend('set_profile', { profileId: id });
    state.history.push({ profileId: id, timestamp: Date.now() });
    render();
    if (typeof showToast === 'function') {
      var prof = state.profiles.find(function(p) { return p.id === id; });
      showToast('Profile Switched', 'Now using ' + (prof ? prof.name : id) + ' profile', 'success');
    }
  }

  // ── Full Render ────────────────────────────────────────────────
  function render() {
    var container = document.getElementById('page-profiles');
    if (!container) return;

    container.innerHTML = '';
    var inner = createEl('div', 'page-inner', container);
    inner.style.overflowY = 'auto';
    inner.style.height = '100%';

    // Header
    var header = createEl('div', 'page-header', inner);
    var icon = createEl('span', 'page-icon', header);
    setText(icon, '\u{1F4C2}');
    var h2 = createEl('h2', '', header);
    setText(h2, 'Workspace Profiles');

    var statusBadge = createEl('div', '', inner);
    statusBadge.style.cssText = 'display:inline-block;font-size:9px;font-weight:600;letter-spacing:1px;text-transform:uppercase;padding:3px 10px;border-radius:4px;margin-bottom:16px;background:var(--accent-cyan-dim);color:var(--accent-cyan);';
    setText(statusBadge, 'ADR-013 \u00B7 Active');

    var desc = createEl('p', 'page-desc', inner);
    setText(desc, 'Context-aware memory optimization profiles that auto-detect your workload and apply tuned memory strategies. Switch profiles manually or let the AI engine detect and apply them automatically.');

    // Grid
    var grid = createEl('div', 'page-grid', inner);
    grid.style.gridTemplateColumns = 'repeat(auto-fill, minmax(280px, 1fr))';

    renderActiveProfile(grid);
    renderProfileSelector(grid);
    renderMemoryAlloc(grid);
    renderSettings(grid);
    renderTimelineCard(grid);
    renderEditor(grid);
  }

  // ── Init ───────────────────────────────────────────────────────
  window.RuVectorPages.profilesInit = function(container) {
    generateHistory();
    render();
    // Request data from backend
    ipcSend('get_profiles');
    ipcSend('get_profile_history');
  };

  // ── Update (called from Rust IPC) ──────────────────────────────
  window.RuVectorPages.profilesUpdate = function(data) {
    if (!data) return;
    if (data.profiles && Array.isArray(data.profiles)) {
      state.profiles = data.profiles;
    }
    if (data.activeId) {
      state.activeId = data.activeId;
    }
    if (data.foregroundApp) {
      state.foregroundApp = data.foregroundApp;
      var appEl = document.getElementById('profiles-fg-app');
      if (appEl) setText(appEl, 'Foreground: ' + data.foregroundApp);
    }
    if (data.history && Array.isArray(data.history)) {
      state.history = data.history;
    }
    if (data.settings) {
      Object.assign(state.settings, data.settings);
    }
    render();
  };

  // ── Resize handler for canvas ──────────────────────────────────
  window.addEventListener('resize', function() {
    var canvas = document.getElementById('profiles-timeline-canvas');
    if (canvas && canvas.offsetParent) {
      drawTimeline(canvas);
    }
  });

})();
