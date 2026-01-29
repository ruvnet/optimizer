(function(){
  'use strict';

  window.RuVectorPages = window.RuVectorPages || {};

  /* ── State ──────────────────────────────────────────────────── */
  var state = {
    installed: [],
    marketplace: [],
    featured: [],
    categories: ['All', 'Gaming', 'Thermal', 'Developer', 'Automation', 'Security', 'System'],
    selectedCategory: 'All',
    searchQuery: '',
    wasmRuntime: { version: 'Wasmer 4.3.7', modules: 0, totalMemoryMB: 0 },
    selectedPlugin: null,
    configPanel: null,
    container: null,
    view: 'installed', // 'installed' | 'marketplace' | 'detail'
    initialized: false
  };

  /* ── IPC ─────────────────────────────────────────────────────── */
  function ipcSend(type, payload) {
    if (window.ipc) {
      var msg = Object.assign({ type: type }, payload || {});
      window.ipc.postMessage(JSON.stringify(msg));
    }
  }

  /* ── Helpers ─────────────────────────────────────────────────── */
  function getCSS(v) { return getComputedStyle(document.documentElement).getPropertyValue(v).trim(); }

  function badge(text, color) {
    var s = document.createElement('span');
    s.style.cssText = 'font-size:9px;padding:2px 6px;border-radius:3px;font-weight:500;letter-spacing:0.3px;background:rgba(' + hexToRgb(color) + ',0.12);color:' + color;
    s.textContent = text;
    return s;
  }

  function hexToRgb(hex) {
    var css = hex;
    if (css.indexOf('var(') === 0) {
      css = getCSS(css.replace('var(', '').replace(')', ''));
    }
    if (!css || css.charAt(0) !== '#') return '128,128,128';
    var r = parseInt(css.slice(1,3), 16);
    var g = parseInt(css.slice(3,5), 16);
    var b = parseInt(css.slice(5,7), 16);
    return r + ',' + g + ',' + b;
  }

  function createBtn(text, icon, primary, onClick) {
    var btn = document.createElement('button');
    btn.className = 'btn';
    if (primary) {
      btn.style.cssText = 'display:flex;align-items:center;gap:8px;padding:7px 14px;border:1px solid var(--accent-cyan);border-radius:6px;background:var(--accent-cyan-dim);color:var(--accent-cyan);font-size:11px;font-family:var(--font);cursor:pointer;transition:var(--transition);font-weight:500';
    } else {
      btn.style.cssText = 'display:flex;align-items:center;gap:8px;padding:7px 14px;border:1px solid var(--border);border-radius:6px;background:var(--bg-card);color:var(--text-secondary);font-size:11px;font-family:var(--font);cursor:pointer;transition:var(--transition)';
    }
    if (icon) {
      var ico = document.createElement('span');
      ico.style.fontSize = '13px';
      ico.textContent = icon;
      btn.appendChild(ico);
    }
    var span = document.createElement('span');
    span.textContent = text;
    btn.appendChild(span);
    if (onClick) btn.addEventListener('click', onClick);
    return btn;
  }

  function createToggleSwitch(active, onChange) {
    var wrap = document.createElement('div');
    wrap.style.cssText = 'position:relative;width:34px;height:18px;flex-shrink:0;background:' + (active ? 'var(--accent-cyan)' : 'var(--border)') + ';border-radius:9px;cursor:pointer;transition:var(--transition)';

    var knob = document.createElement('div');
    knob.style.cssText = 'position:absolute;top:2px;left:' + (active ? '18' : '2') + 'px;width:14px;height:14px;border-radius:50%;background:var(--text-primary);transition:var(--transition)';
    wrap.appendChild(knob);

    wrap.addEventListener('click', function() {
      var isOn = !wrap.classList.contains('active-toggle');
      if (isOn) wrap.classList.add('active-toggle');
      else wrap.classList.remove('active-toggle');
      wrap.style.background = isOn ? 'var(--accent-cyan)' : 'var(--border)';
      knob.style.left = isOn ? '18px' : '2px';
      if (onChange) onChange(isOn);
    });
    if (active) wrap.classList.add('active-toggle');

    return wrap;
  }

  /* ── Demo data ───────────────────────────────────────────────── */
  function getDemoInstalled() {
    return [
      { id: 'chrome-opt', name: 'Chrome Tab Optimizer', version: '1.2.0', author: 'RuVector Team', status: 'active', category: 'Developer', capabilities: ['memory_read', 'process_control'], verified: true, signed: true, cpuPct: 0.3, memMB: 4.2, description: 'Automatically suspends inactive Chrome tabs to free memory.' },
      { id: 'game-boost', name: 'Game Memory Booster', version: '2.0.1', author: 'GameDev Labs', status: 'active', category: 'Gaming', capabilities: ['memory_write', 'priority_boost'], verified: true, signed: true, cpuPct: 0.1, memMB: 2.8, description: 'Optimizes memory for gaming by pre-loading assets and adjusting priorities.' },
      { id: 'docker-limit', name: 'Docker Memory Limiter', version: '0.9.3', author: 'ContainerCo', status: 'disabled', category: 'Developer', capabilities: ['memory_read', 'container_control'], verified: false, signed: true, cpuPct: 0, memMB: 0, description: 'Limits Docker container memory usage based on system pressure.' },
      { id: 'ram-disk', name: 'RAM Disk Manager', version: '1.5.0', author: 'SpeedTools', status: 'active', category: 'System', capabilities: ['memory_write', 'disk_control', 'admin'], verified: true, signed: true, cpuPct: 0.5, memMB: 8.1, description: 'Creates and manages RAM disks for ultra-fast temporary storage.' },
      { id: 'thermal-guard', name: 'Thermal Guardian', version: '1.0.2', author: 'RuVector Team', status: 'active', category: 'Thermal', capabilities: ['thermal_read', 'fan_control'], verified: true, signed: true, cpuPct: 0.2, memMB: 3.1, description: 'Advanced fan curve management with thermal zone monitoring.' }
    ];
  }

  function getDemoMarketplace() {
    return [
      { id: 'mem-compress', name: 'Memory Compressor Pro', version: '3.1.0', author: 'SysOpt Inc', category: 'System', rating: 4.8, downloads: 12400, description: 'Compresses idle memory pages for up to 40% savings.', verified: true, signed: true, capabilities: ['memory_write', 'kernel_driver'] },
      { id: 'ai-prefetch', name: 'AI Prefetch Engine', version: '1.0.0', author: 'ML Works', category: 'Automation', rating: 4.5, downloads: 3200, description: 'Predicts application launches and preloads memory.', verified: true, signed: true, capabilities: ['memory_read', 'process_monitor'] },
      { id: 'security-scan', name: 'Memory Security Scanner', version: '2.4.1', author: 'SecureOps', category: 'Security', rating: 4.9, downloads: 8900, description: 'Scans for memory-resident malware and suspicious patterns.', verified: true, signed: true, capabilities: ['memory_read', 'security_scan'] },
      { id: 'dev-profiler', name: 'Dev Memory Profiler', version: '1.3.2', author: 'DevTools Co', category: 'Developer', rating: 4.6, downloads: 5600, description: 'Real-time memory profiling for development workflows.', verified: false, signed: true, capabilities: ['memory_read', 'process_monitor'] },
      { id: 'game-cache', name: 'Game Cache Warmer', version: '1.1.0', author: 'GameDev Labs', category: 'Gaming', rating: 4.3, downloads: 7800, description: 'Pre-loads game assets into memory for faster loading.', verified: true, signed: false, capabilities: ['memory_write', 'disk_read'] },
      { id: 'thermal-ai', name: 'Thermal AI Optimizer', version: '0.8.0', author: 'ThermoTech', category: 'Thermal', rating: 4.1, downloads: 1200, description: 'Uses ML to predict and prevent thermal throttling.', verified: false, signed: false, capabilities: ['thermal_read', 'process_control'] },
      { id: 'auto-clean', name: 'Auto Memory Cleaner', version: '2.0.0', author: 'CleanSys', category: 'Automation', rating: 4.7, downloads: 15600, description: 'Scheduled memory cleanup with configurable thresholds.', verified: true, signed: true, capabilities: ['memory_write'] },
      { id: 'wsl-bridge', name: 'WSL Memory Bridge', version: '1.2.1', author: 'LinuxTools', category: 'Developer', rating: 4.4, downloads: 4100, description: 'Shares optimized memory between WSL and Windows.', verified: true, signed: true, capabilities: ['memory_read', 'wsl_bridge'] }
    ];
  }

  /* ── Installed plugins grid ──────────────────────────────────── */
  function renderInstalledGrid(container) {
    container.innerHTML = '';

    var plugins = state.installed.length > 0 ? state.installed : getDemoInstalled();

    if (plugins.length === 0) {
      var empty = document.createElement('div');
      empty.style.cssText = 'text-align:center;padding:40px;color:var(--text-dim);font-size:12px';
      empty.textContent = 'No plugins installed. Browse the marketplace to get started.';
      container.appendChild(empty);
      return;
    }

    var grid = document.createElement('div');
    grid.style.cssText = 'display:grid;grid-template-columns:repeat(auto-fill,minmax(300px,1fr));gap:14px';

    plugins.forEach(function(plugin) {
      var card = document.createElement('div');
      card.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px;transition:var(--transition);cursor:pointer';
      card.addEventListener('mouseenter', function() { card.style.background = 'var(--bg-card-hover)'; });
      card.addEventListener('mouseleave', function() { card.style.background = 'var(--bg-card)'; });

      // Top row: name + toggle
      var topRow = document.createElement('div');
      topRow.style.cssText = 'display:flex;justify-content:space-between;align-items:center;margin-bottom:8px';

      var nameWrap = document.createElement('div');
      var nameEl = document.createElement('div');
      nameEl.style.cssText = 'font-size:13px;font-weight:500;color:var(--text-primary)';
      nameEl.textContent = plugin.name;
      var verEl = document.createElement('div');
      verEl.style.cssText = 'font-size:9px;color:var(--text-dim);margin-top:1px';
      verEl.textContent = 'v' + plugin.version + ' by ' + plugin.author;
      nameWrap.appendChild(nameEl);
      nameWrap.appendChild(verEl);

      topRow.appendChild(nameWrap);
      topRow.appendChild(createToggleSwitch(plugin.status === 'active', function(isOn) {
        ipcSend('toggle_plugin', { plugin_id: plugin.id, enabled: isOn });
        plugin.status = isOn ? 'active' : 'disabled';
      }));

      card.appendChild(topRow);

      // Description
      var desc = document.createElement('div');
      desc.style.cssText = 'font-size:11px;color:var(--text-secondary);line-height:1.5;margin-bottom:8px';
      desc.textContent = plugin.description;
      card.appendChild(desc);

      // Badges row
      var badgeRow = document.createElement('div');
      badgeRow.style.cssText = 'display:flex;flex-wrap:wrap;gap:4px;margin-bottom:8px';

      var catColor = getCategoryColor(plugin.category);
      badgeRow.appendChild(badge(plugin.category, catColor));

      if (plugin.verified) badgeRow.appendChild(badge('\u2714 Verified', getCSS('--accent-green')));
      if (plugin.signed) badgeRow.appendChild(badge('\uD83D\uDD12 Signed', getCSS('--accent-cyan')));
      if (!plugin.signed) badgeRow.appendChild(badge('\u26A0 Unsigned', getCSS('--accent-amber')));

      plugin.capabilities.forEach(function(cap) {
        var isRisky = cap === 'admin' || cap === 'kernel_driver' || cap === 'memory_write';
        badgeRow.appendChild(badge(cap, isRisky ? getCSS('--accent-amber') : getCSS('--text-secondary')));
      });

      card.appendChild(badgeRow);

      // Metrics row
      if (plugin.status === 'active') {
        var metricsRow = document.createElement('div');
        metricsRow.style.cssText = 'display:flex;gap:12px;font-size:10px;color:var(--text-dim);border-top:1px solid var(--border);padding-top:8px;font-family:var(--mono)';

        var cpuMetric = document.createElement('span');
        cpuMetric.textContent = 'CPU: ' + (plugin.cpuPct || 0).toFixed(1) + '%';
        var memMetric = document.createElement('span');
        memMetric.textContent = 'MEM: ' + (plugin.memMB || 0).toFixed(1) + ' MB';

        metricsRow.appendChild(cpuMetric);
        metricsRow.appendChild(memMetric);
        card.appendChild(metricsRow);
      }

      // Click for detail
      card.addEventListener('click', function(e) {
        if (e.target.closest('.active-toggle, [style*="cursor:pointer"]')) return;
        state.selectedPlugin = plugin;
        state.view = 'detail';
        renderView();
      });

      grid.appendChild(card);
    });

    container.appendChild(grid);
  }

  function getCategoryColor(cat) {
    var colors = {
      Gaming: getCSS('--accent-purple'),
      Thermal: getCSS('--accent-red'),
      Developer: getCSS('--accent-cyan'),
      Automation: getCSS('--accent-green'),
      Security: getCSS('--accent-amber'),
      System: getCSS('--text-secondary')
    };
    return colors[cat] || getCSS('--text-secondary');
  }

  /* ── Marketplace browser ─────────────────────────────────────── */
  function renderMarketplace(container) {
    container.innerHTML = '';

    // Search + filter row
    var filterRow = document.createElement('div');
    filterRow.style.cssText = 'display:flex;gap:10px;margin-bottom:16px;flex-wrap:wrap;align-items:center';

    var searchInput = document.createElement('input');
    searchInput.type = 'text';
    searchInput.placeholder = 'Search plugins...';
    searchInput.value = state.searchQuery;
    searchInput.style.cssText = 'flex:1;min-width:200px;padding:8px 12px;background:var(--bg-primary);color:var(--text-primary);border:1px solid var(--border);border-radius:6px;font-size:12px;font-family:var(--font);outline:none';
    searchInput.addEventListener('input', function() {
      state.searchQuery = searchInput.value;
      renderMarketplaceGrid(gridContainer);
    });
    searchInput.addEventListener('focus', function() { searchInput.style.borderColor = 'var(--accent-cyan)'; });
    searchInput.addEventListener('blur', function() { searchInput.style.borderColor = 'var(--border)'; });
    filterRow.appendChild(searchInput);

    // Category pills
    var pillRow = document.createElement('div');
    pillRow.style.cssText = 'display:flex;gap:4px;flex-wrap:wrap';
    state.categories.forEach(function(cat) {
      var pill = document.createElement('button');
      var isActive = state.selectedCategory === cat;
      pill.style.cssText = 'padding:4px 10px;border:1px solid ' + (isActive ? 'var(--accent-cyan)' : 'var(--border)') + ';border-radius:12px;background:' + (isActive ? 'var(--accent-cyan-dim)' : 'var(--bg-card)') + ';color:' + (isActive ? 'var(--accent-cyan)' : 'var(--text-secondary)') + ';font-size:10px;font-family:var(--font);cursor:pointer;transition:var(--transition)';
      pill.textContent = cat;
      pill.addEventListener('click', function() {
        state.selectedCategory = cat;
        renderMarketplace(container);
      });
      pillRow.appendChild(pill);
    });
    filterRow.appendChild(pillRow);

    container.appendChild(filterRow);

    // Featured carousel
    var featured = state.featured.length > 0 ? state.featured : getDemoMarketplace().slice(0, 3);
    var carousel = document.createElement('div');
    carousel.style.cssText = 'display:grid;grid-template-columns:repeat(3,1fr);gap:12px;margin-bottom:16px';

    featured.forEach(function(fp) {
      var card = document.createElement('div');
      card.style.cssText = 'background:linear-gradient(135deg,var(--bg-card) 0%,var(--bg-card-hover) 100%);border:1px solid var(--accent-cyan);border-radius:var(--radius);padding:14px;cursor:pointer;transition:var(--transition)';

      var label = document.createElement('div');
      label.style.cssText = 'font-size:8px;font-weight:600;text-transform:uppercase;letter-spacing:1px;color:var(--accent-cyan);margin-bottom:6px';
      label.textContent = '\u2B50 Featured';

      var name = document.createElement('div');
      name.style.cssText = 'font-size:13px;font-weight:500;color:var(--text-primary);margin-bottom:3px';
      name.textContent = fp.name;

      var descEl = document.createElement('div');
      descEl.style.cssText = 'font-size:10px;color:var(--text-secondary);line-height:1.4;margin-bottom:6px';
      descEl.textContent = fp.description;

      var meta = document.createElement('div');
      meta.style.cssText = 'display:flex;gap:8px;font-size:9px;color:var(--text-dim)';
      meta.textContent = '\u2605 ' + fp.rating + ' \u00B7 ' + formatDownloads(fp.downloads) + ' downloads';

      card.appendChild(label);
      card.appendChild(name);
      card.appendChild(descEl);
      card.appendChild(meta);

      card.addEventListener('click', function() {
        state.selectedPlugin = fp;
        state.view = 'detail';
        renderView();
      });

      carousel.appendChild(card);
    });
    container.appendChild(carousel);

    // Grid
    var gridContainer = document.createElement('div');
    renderMarketplaceGrid(gridContainer);
    container.appendChild(gridContainer);
  }

  function renderMarketplaceGrid(container) {
    container.innerHTML = '';
    var plugins = state.marketplace.length > 0 ? state.marketplace : getDemoMarketplace();

    var filtered = plugins.filter(function(p) {
      var matchCat = state.selectedCategory === 'All' || p.category === state.selectedCategory;
      var matchSearch = !state.searchQuery || p.name.toLowerCase().indexOf(state.searchQuery.toLowerCase()) >= 0 || p.description.toLowerCase().indexOf(state.searchQuery.toLowerCase()) >= 0;
      return matchCat && matchSearch;
    });

    if (filtered.length === 0) {
      var empty = document.createElement('div');
      empty.style.cssText = 'text-align:center;padding:30px;color:var(--text-dim);font-size:12px';
      empty.textContent = 'No plugins match your search.';
      container.appendChild(empty);
      return;
    }

    var grid = document.createElement('div');
    grid.style.cssText = 'display:grid;grid-template-columns:repeat(auto-fill,minmax(280px,1fr));gap:12px';

    filtered.forEach(function(plugin) {
      var card = document.createElement('div');
      card.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px;transition:var(--transition);cursor:pointer';
      card.addEventListener('mouseenter', function() { card.style.background = 'var(--bg-card-hover)'; });
      card.addEventListener('mouseleave', function() { card.style.background = 'var(--bg-card)'; });

      var topRow = document.createElement('div');
      topRow.style.cssText = 'display:flex;justify-content:space-between;align-items:flex-start;margin-bottom:6px';

      var nameEl = document.createElement('div');
      nameEl.style.cssText = 'font-size:12px;font-weight:500;color:var(--text-primary)';
      nameEl.textContent = plugin.name;

      var ratingEl = document.createElement('div');
      ratingEl.style.cssText = 'font-size:10px;color:var(--accent-amber);white-space:nowrap';
      ratingEl.textContent = '\u2605 ' + plugin.rating;

      topRow.appendChild(nameEl);
      topRow.appendChild(ratingEl);
      card.appendChild(topRow);

      var meta = document.createElement('div');
      meta.style.cssText = 'font-size:9px;color:var(--text-dim);margin-bottom:6px';
      meta.textContent = plugin.author + ' \u00B7 v' + plugin.version + ' \u00B7 ' + formatDownloads(plugin.downloads) + ' downloads';
      card.appendChild(meta);

      var desc = document.createElement('div');
      desc.style.cssText = 'font-size:11px;color:var(--text-secondary);line-height:1.4;margin-bottom:8px';
      desc.textContent = plugin.description;
      card.appendChild(desc);

      var badgeRow = document.createElement('div');
      badgeRow.style.cssText = 'display:flex;flex-wrap:wrap;gap:4px;margin-bottom:8px';
      badgeRow.appendChild(badge(plugin.category, getCategoryColor(plugin.category)));
      if (plugin.verified) badgeRow.appendChild(badge('\u2714 Verified', getCSS('--accent-green')));
      if (plugin.signed) badgeRow.appendChild(badge('\uD83D\uDD12 Signed', getCSS('--accent-cyan')));
      if (!plugin.verified && !plugin.signed) badgeRow.appendChild(badge('\u26A0 Unverified', getCSS('--accent-red')));
      card.appendChild(badgeRow);

      // Install button
      var installed = (state.installed.length > 0 ? state.installed : getDemoInstalled()).some(function(ip) { return ip.id === plugin.id; });
      var installBtn = createBtn(installed ? 'Installed' : 'Install', installed ? '\u2714' : '\u2B07', !installed, function() {
        if (!installed) {
          ipcSend('install_plugin', { plugin_id: plugin.id });
          installBtn.querySelector('span:last-child').textContent = 'Installing...';
          installBtn.style.opacity = '0.6';
          installBtn.style.pointerEvents = 'none';
        }
      });
      if (installed) {
        installBtn.style.opacity = '0.6';
        installBtn.style.cursor = 'default';
      }
      card.appendChild(installBtn);

      card.addEventListener('click', function(e) {
        if (e.target.closest('.btn')) return;
        state.selectedPlugin = plugin;
        state.view = 'detail';
        renderView();
      });

      grid.appendChild(card);
    });

    container.appendChild(grid);
  }

  function formatDownloads(n) {
    if (n >= 1000) return (n / 1000).toFixed(1) + 'k';
    return String(n);
  }

  /* ── Plugin detail view ──────────────────────────────────────── */
  function renderDetailView(container) {
    container.innerHTML = '';
    var plugin = state.selectedPlugin;
    if (!plugin) { state.view = 'installed'; renderView(); return; }

    // Back button
    var backBtn = createBtn('Back', '\u2190', false, function() {
      state.selectedPlugin = null;
      state.view = state.view === 'detail' ? 'installed' : 'marketplace';
      renderView();
    });
    backBtn.style.marginBottom = '16px';
    container.appendChild(backBtn);

    // Detail layout
    var layout = document.createElement('div');
    layout.style.cssText = 'display:grid;grid-template-columns:1fr 300px;gap:16px';

    // Left: info
    var infoCard = document.createElement('div');
    infoCard.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:20px';

    var nameEl = document.createElement('h3');
    nameEl.style.cssText = 'font-size:18px;font-weight:600;color:var(--text-primary);margin-bottom:4px';
    nameEl.textContent = plugin.name;

    var metaEl = document.createElement('div');
    metaEl.style.cssText = 'font-size:11px;color:var(--text-secondary);margin-bottom:12px';
    metaEl.textContent = 'v' + plugin.version + ' by ' + plugin.author;
    if (plugin.rating) metaEl.textContent += ' \u00B7 \u2605 ' + plugin.rating;
    if (plugin.downloads) metaEl.textContent += ' \u00B7 ' + formatDownloads(plugin.downloads) + ' downloads';

    var descEl = document.createElement('p');
    descEl.style.cssText = 'font-size:12px;color:var(--text-secondary);line-height:1.6;margin-bottom:16px';
    descEl.textContent = plugin.description;

    infoCard.appendChild(nameEl);
    infoCard.appendChild(metaEl);
    infoCard.appendChild(descEl);

    // Badges
    var badgeRow = document.createElement('div');
    badgeRow.style.cssText = 'display:flex;flex-wrap:wrap;gap:6px;margin-bottom:16px';
    if (plugin.category) badgeRow.appendChild(badge(plugin.category, getCategoryColor(plugin.category)));
    if (plugin.verified) badgeRow.appendChild(badge('\u2714 Verified', getCSS('--accent-green')));
    else badgeRow.appendChild(badge('\u2718 Not Verified', getCSS('--accent-red')));
    if (plugin.signed) badgeRow.appendChild(badge('\uD83D\uDD12 Signed', getCSS('--accent-cyan')));
    else badgeRow.appendChild(badge('\u26A0 Unsigned', getCSS('--accent-amber')));
    infoCard.appendChild(badgeRow);

    // Capabilities section
    var capTitle = document.createElement('div');
    capTitle.style.cssText = 'font-size:10px;font-weight:600;text-transform:uppercase;letter-spacing:1px;color:var(--text-dim);margin-bottom:8px';
    capTitle.textContent = 'Requested Capabilities';
    infoCard.appendChild(capTitle);

    var capList = document.createElement('div');
    capList.style.cssText = 'display:flex;flex-direction:column;gap:4px;margin-bottom:16px';
    (plugin.capabilities || []).forEach(function(cap) {
      var capRow = document.createElement('div');
      capRow.style.cssText = 'display:flex;align-items:center;gap:8px;padding:6px 8px;background:var(--bg-primary);border-radius:4px;font-size:11px';
      var isRisky = cap === 'admin' || cap === 'kernel_driver' || cap === 'memory_write' || cap === 'fan_control';
      var icon = document.createElement('span');
      icon.style.cssText = 'color:' + (isRisky ? 'var(--accent-amber)' : 'var(--accent-green)');
      icon.textContent = isRisky ? '\u26A0' : '\u2714';
      var text = document.createElement('span');
      text.style.color = 'var(--text-secondary)';
      text.textContent = cap.replace(/_/g, ' ');
      var riskLabel = document.createElement('span');
      riskLabel.style.cssText = 'margin-left:auto;font-size:9px;color:' + (isRisky ? 'var(--accent-amber)' : 'var(--text-dim)');
      riskLabel.textContent = isRisky ? 'Elevated' : 'Standard';
      capRow.appendChild(icon);
      capRow.appendChild(text);
      capRow.appendChild(riskLabel);
      capList.appendChild(capRow);
    });
    infoCard.appendChild(capList);

    // Action buttons
    var btnRow = document.createElement('div');
    btnRow.style.cssText = 'display:flex;gap:8px';
    var installed = (state.installed.length > 0 ? state.installed : getDemoInstalled()).some(function(ip) { return ip.id === plugin.id; });
    if (installed) {
      btnRow.appendChild(createBtn('Uninstall', '\uD83D\uDDD1', false, function() {
        ipcSend('uninstall_plugin', { plugin_id: plugin.id });
      }));
      btnRow.appendChild(createBtn('Configure', '\u2699', false, function() {
        ipcSend('get_plugin_config', { plugin_id: plugin.id });
      }));
    } else {
      btnRow.appendChild(createBtn('Install Plugin', '\u2B07', true, function() {
        ipcSend('install_plugin', { plugin_id: plugin.id });
      }));
    }
    infoCard.appendChild(btnRow);

    layout.appendChild(infoCard);

    // Right: WASM runtime status
    var rightCol = document.createElement('div');
    rightCol.style.cssText = 'display:flex;flex-direction:column;gap:14px';

    // Runtime card
    var rtCard = document.createElement('div');
    rtCard.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px';
    var rtTitle = document.createElement('div');
    rtTitle.className = 'card-title';
    rtTitle.textContent = 'WASM Runtime';
    rtCard.appendChild(rtTitle);

    var fields = [
      ['Wasmer Version', state.wasmRuntime.version],
      ['Loaded Modules', String(state.wasmRuntime.modules)],
      ['Total Memory', state.wasmRuntime.totalMemoryMB.toFixed(1) + ' MB']
    ];
    fields.forEach(function(f) {
      var row = document.createElement('div');
      row.style.cssText = 'display:flex;justify-content:space-between;padding:5px 0;font-size:11px;border-bottom:1px solid var(--border)';
      var lbl = document.createElement('span');
      lbl.style.color = 'var(--text-secondary)';
      lbl.textContent = f[0];
      var val = document.createElement('span');
      val.style.cssText = 'font-family:var(--mono);color:var(--text-primary)';
      val.textContent = f[1];
      row.appendChild(lbl);
      row.appendChild(val);
      rtCard.appendChild(row);
    });
    rightCol.appendChild(rtCard);

    // Resource usage card (if installed)
    if (installed && (plugin.cpuPct !== undefined || plugin.memMB !== undefined)) {
      var resCard = document.createElement('div');
      resCard.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px';
      var resTitle = document.createElement('div');
      resTitle.className = 'card-title';
      resTitle.textContent = 'Resource Usage';
      resCard.appendChild(resTitle);

      // CPU bar
      var cpuRow = document.createElement('div');
      cpuRow.style.cssText = 'margin-bottom:8px';
      var cpuLabel = document.createElement('div');
      cpuLabel.style.cssText = 'display:flex;justify-content:space-between;font-size:10px;color:var(--text-secondary);margin-bottom:3px';
      cpuLabel.innerHTML = '<span>CPU</span><span style="font-family:var(--mono)">' + (plugin.cpuPct || 0).toFixed(1) + '%</span>';
      var cpuBar = document.createElement('div');
      cpuBar.style.cssText = 'height:6px;background:var(--gauge-track);border-radius:3px;overflow:hidden';
      var cpuFill = document.createElement('div');
      cpuFill.style.cssText = 'height:100%;border-radius:3px;background:var(--accent-cyan);width:' + Math.min((plugin.cpuPct || 0) * 10, 100) + '%';
      cpuBar.appendChild(cpuFill);
      cpuRow.appendChild(cpuLabel);
      cpuRow.appendChild(cpuBar);
      resCard.appendChild(cpuRow);

      // Memory bar
      var memRow = document.createElement('div');
      var memLabel = document.createElement('div');
      memLabel.style.cssText = 'display:flex;justify-content:space-between;font-size:10px;color:var(--text-secondary);margin-bottom:3px';
      memLabel.innerHTML = '<span>Memory</span><span style="font-family:var(--mono)">' + (plugin.memMB || 0).toFixed(1) + ' MB</span>';
      var memBar = document.createElement('div');
      memBar.style.cssText = 'height:6px;background:var(--gauge-track);border-radius:3px;overflow:hidden';
      var memFill = document.createElement('div');
      memFill.style.cssText = 'height:100%;border-radius:3px;background:var(--accent-amber);width:' + Math.min((plugin.memMB || 0) * 2, 100) + '%';
      memBar.appendChild(memFill);
      memRow.appendChild(memLabel);
      memRow.appendChild(memBar);
      resCard.appendChild(memRow);

      rightCol.appendChild(resCard);
    }

    layout.appendChild(rightCol);
    container.appendChild(layout);
  }

  /* ── Main render ─────────────────────────────────────────────── */
  function renderView() {
    if (!state.container) return;
    var content = state.container.querySelector('[data-plugins-content]');
    if (!content) return;

    // Update tab buttons
    var tabs = state.container.querySelectorAll('[data-tab]');
    tabs.forEach(function(tab) {
      var isActive = tab.getAttribute('data-tab') === state.view || (state.view === 'detail');
      tab.style.background = isActive ? 'var(--accent-cyan-dim)' : 'var(--bg-card)';
      tab.style.color = isActive ? 'var(--accent-cyan)' : 'var(--text-secondary)';
      tab.style.borderColor = isActive ? 'var(--accent-cyan)' : 'var(--border)';
    });

    if (state.view === 'detail') {
      renderDetailView(content);
    } else if (state.view === 'marketplace') {
      renderMarketplace(content);
    } else {
      renderInstalledGrid(content);
    }
  }

  /* ══════════════════════════════════════════════════════════════ */
  /* INIT                                                          */
  /* ══════════════════════════════════════════════════════════════ */
  window.RuVectorPages.plugins_Init = function(container) {
    state.container = container;
    container.innerHTML = '';

    var inner = document.createElement('div');
    inner.className = 'page-inner';
    inner.style.cssText = 'padding:24px 28px;max-width:1400px';

    // Header
    var header = document.createElement('div');
    header.className = 'page-header';
    header.innerHTML = '<span class="page-icon">&#129520;</span><h2>WASM Plugin Marketplace</h2>';
    inner.appendChild(header);

    var status = document.createElement('div');
    status.style.cssText = 'display:inline-block;font-size:9px;font-weight:600;letter-spacing:1px;text-transform:uppercase;padding:3px 10px;border-radius:4px;margin-bottom:16px;background:var(--accent-cyan-dim);color:var(--accent-cyan)';
    status.textContent = 'ADR-021 \u00B7 Active';
    inner.appendChild(status);

    var desc = document.createElement('p');
    desc.className = 'page-desc';
    desc.textContent = 'Extend RuVector with sandboxed WASM plugins. Browse, install, and manage plugins from a curated marketplace. Plugins run in Wasmer 4.3 with capability-based security and Ed25519 signing.';
    inner.appendChild(desc);

    // Tab row
    var tabRow = document.createElement('div');
    tabRow.style.cssText = 'display:flex;gap:6px;margin-bottom:16px';

    var tabs = [
      { id: 'installed', label: 'Installed Plugins', icon: '\uD83D\uDCE6' },
      { id: 'marketplace', label: 'Marketplace', icon: '\uD83D\uDED2' }
    ];
    tabs.forEach(function(tab) {
      var btn = document.createElement('button');
      var isActive = state.view === tab.id;
      btn.style.cssText = 'padding:8px 16px;border:1px solid ' + (isActive ? 'var(--accent-cyan)' : 'var(--border)') + ';border-radius:6px;background:' + (isActive ? 'var(--accent-cyan-dim)' : 'var(--bg-card)') + ';color:' + (isActive ? 'var(--accent-cyan)' : 'var(--text-secondary)') + ';font-size:12px;font-family:var(--font);cursor:pointer;transition:var(--transition);font-weight:500';
      btn.textContent = tab.icon + ' ' + tab.label;
      btn.setAttribute('data-tab', tab.id);
      btn.addEventListener('click', function() {
        state.view = tab.id;
        state.selectedPlugin = null;
        renderView();
      });
      tabRow.appendChild(btn);
    });
    inner.appendChild(tabRow);

    // WASM Runtime status bar
    var rtBar = document.createElement('div');
    rtBar.style.cssText = 'display:flex;gap:16px;padding:8px 14px;background:var(--bg-card);border:1px solid var(--border);border-radius:6px;margin-bottom:16px;font-size:10px;color:var(--text-secondary);align-items:center';
    rtBar.setAttribute('data-wasm-bar', '');

    var items = [
      { label: 'Runtime', value: state.wasmRuntime.version, color: '--accent-cyan' },
      { label: 'Modules', value: String(state.wasmRuntime.modules), color: '--accent-green' },
      { label: 'Memory', value: state.wasmRuntime.totalMemoryMB.toFixed(1) + ' MB', color: '--accent-amber' },
      { label: 'Security', value: 'Ed25519 + Sandbox', color: '--accent-purple' }
    ];
    items.forEach(function(item, idx) {
      if (idx > 0) {
        var sep = document.createElement('span');
        sep.style.cssText = 'color:var(--border)';
        sep.textContent = '\u2502';
        rtBar.appendChild(sep);
      }
      var span = document.createElement('span');
      span.innerHTML = '<span style="color:var(--text-dim)">' + item.label + ':</span> <span style="color:var(' + item.color + ');font-family:var(--mono)"></span>';
      span.querySelector('span:last-child').textContent = item.value;
      rtBar.appendChild(span);
    });
    inner.appendChild(rtBar);

    // Content area
    var content = document.createElement('div');
    content.setAttribute('data-plugins-content', '');
    inner.appendChild(content);

    container.appendChild(inner);

    // Render initial view
    renderView();

    // Request data
    ipcSend('get_plugins');
    ipcSend('get_marketplace');

    state.initialized = true;
  };

  /* ══════════════════════════════════════════════════════════════ */
  /* UPDATE                                                        */
  /* ══════════════════════════════════════════════════════════════ */
  window.RuVectorPages.plugins_Update = function(data) {
    if (!data) return;

    if (data.installed) state.installed = data.installed;
    if (data.marketplace) state.marketplace = data.marketplace;
    if (data.featured) state.featured = data.featured;
    if (data.wasm_runtime) {
      if (data.wasm_runtime.version) state.wasmRuntime.version = data.wasm_runtime.version;
      if (data.wasm_runtime.modules !== undefined) state.wasmRuntime.modules = data.wasm_runtime.modules;
      if (data.wasm_runtime.total_memory_mb !== undefined) state.wasmRuntime.totalMemoryMB = data.wasm_runtime.total_memory_mb;
    }
    if (data.plugin_config && state.selectedPlugin) {
      state.configPanel = data.plugin_config;
    }

    // Update WASM bar
    if (state.container) {
      var bar = state.container.querySelector('[data-wasm-bar]');
      if (bar) {
        var monoSpans = bar.querySelectorAll('span[style*="font-family"]');
        if (monoSpans.length >= 3) {
          monoSpans[0].textContent = state.wasmRuntime.version;
          monoSpans[1].textContent = String(state.wasmRuntime.modules);
          monoSpans[2].textContent = state.wasmRuntime.totalMemoryMB.toFixed(1) + ' MB';
        }
      }
    }

    if (state.initialized) renderView();
  };

})();
