(function(){
  'use strict';

  window.RuVectorPages = window.RuVectorPages || {};

  /* ── State ──────────────────────────────────────────────────── */
  var state = {
    cpuTemp: 0,
    gpuTemp: 0,
    fanSpeed: 0,
    maxFanSpeed: 3000,
    throttleState: 'none',
    thresholds: { warning: 70, throttle: 85, critical: 95 },
    powerPlan: 'balanced',
    silentMode: false,
    coreMigration: true,
    thermalPrediction: true,
    processes: [],
    history: [],        // [{ts, cpuTemp, gpuTemp, fanSpeed}]
    historyMaxLen: 360, // 1 hour at 10s intervals
    zones: { green: 100, yellow: 0, red: 0 },
    predictionMinutes: null,
    animFrame: null,
    container: null,
    canvasCtx: null,
    canvasEl: null,
    initialized: false
  };

  /* ── IPC helper ──────────────────────────────────────────────── */
  function ipcSend(type, payload) {
    if (window.ipc) {
      var msg = Object.assign({ type: type }, payload || {});
      window.ipc.postMessage(JSON.stringify(msg));
    }
  }

  /* ── Circular gauge (SVG-free, pure DOM) ─────────────────────── */
  function createCircularGauge(id, label, unit, max, size) {
    size = size || 100;
    var wrap = document.createElement('div');
    wrap.style.cssText = 'position:relative;width:' + size + 'px;height:' + size + 'px;flex-shrink:0';

    var canvas = document.createElement('canvas');
    canvas.width = size * 2;
    canvas.height = size * 2;
    canvas.style.cssText = 'width:100%;height:100%';
    canvas.setAttribute('data-gauge-id', id);
    wrap.appendChild(canvas);

    var lbl = document.createElement('div');
    lbl.style.cssText = 'position:absolute;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center;pointer-events:none';
    var valSpan = document.createElement('span');
    valSpan.style.cssText = 'font-size:' + Math.round(size * 0.22) + 'px;font-weight:600;color:var(--text-primary);line-height:1';
    valSpan.setAttribute('data-gauge-val', id);
    valSpan.textContent = '--';
    var unitSpan = document.createElement('span');
    unitSpan.style.cssText = 'font-size:9px;color:var(--text-dim);margin-top:2px';
    unitSpan.textContent = unit;
    var labelSpan = document.createElement('span');
    labelSpan.style.cssText = 'font-size:8px;color:var(--text-dim);margin-top:1px;text-transform:uppercase;letter-spacing:0.5px';
    labelSpan.textContent = label;
    lbl.appendChild(valSpan);
    lbl.appendChild(unitSpan);
    lbl.appendChild(labelSpan);
    wrap.appendChild(lbl);

    return { element: wrap, canvas: canvas, max: max };
  }

  function drawGauge(canvas, value, max, color) {
    var ctx = canvas.getContext('2d');
    var w = canvas.width;
    var cx = w / 2, cy = w / 2, r = w / 2 - 14;
    var startAngle = 0.75 * Math.PI;
    var endAngle = 2.25 * Math.PI;
    var pct = Math.min(value / max, 1);
    var valAngle = startAngle + (endAngle - startAngle) * pct;

    ctx.clearRect(0, 0, w, w);

    // track
    ctx.beginPath();
    ctx.arc(cx, cy, r, startAngle, endAngle);
    ctx.strokeStyle = getCSS('--gauge-track');
    ctx.lineWidth = 10;
    ctx.lineCap = 'round';
    ctx.stroke();

    // fill
    if (pct > 0) {
      ctx.beginPath();
      ctx.arc(cx, cy, r, startAngle, valAngle);
      ctx.strokeStyle = color;
      ctx.lineWidth = 10;
      ctx.lineCap = 'round';
      ctx.stroke();
    }

    // tick marks
    ctx.strokeStyle = getCSS('--text-dim');
    ctx.lineWidth = 1;
    for (var i = 0; i <= 10; i++) {
      var a = startAngle + (endAngle - startAngle) * (i / 10);
      var inner = r - 6;
      var outer = r + 4;
      ctx.beginPath();
      ctx.moveTo(cx + Math.cos(a) * inner, cy + Math.sin(a) * inner);
      ctx.lineTo(cx + Math.cos(a) * outer, cy + Math.sin(a) * outer);
      ctx.stroke();
    }
  }

  function getCSS(varName) {
    return getComputedStyle(document.documentElement).getPropertyValue(varName).trim();
  }

  function tempColor(temp) {
    if (temp >= state.thresholds.critical) return getCSS('--accent-red');
    if (temp >= state.thresholds.throttle) return getCSS('--accent-red');
    if (temp >= state.thresholds.warning) return getCSS('--accent-amber');
    return getCSS('--accent-cyan');
  }

  /* ── History chart (Canvas 2D) ───────────────────────────────── */
  function drawHistoryChart() {
    var canvas = state.canvasEl;
    if (!canvas) return;
    var ctx = canvas.getContext('2d');
    var dpr = window.devicePixelRatio || 1;
    var rect = canvas.parentElement.getBoundingClientRect();
    var W = rect.width;
    var H = rect.height;
    canvas.width = W * dpr;
    canvas.height = H * dpr;
    canvas.style.width = W + 'px';
    canvas.style.height = H + 'px';
    ctx.scale(dpr, dpr);

    var pad = { top: 20, right: 50, bottom: 30, left: 45 };
    var cw = W - pad.left - pad.right;
    var ch = H - pad.top - pad.bottom;

    // Background
    ctx.fillStyle = getCSS('--bg-primary');
    ctx.fillRect(0, 0, W, H);

    if (state.history.length < 2) {
      ctx.fillStyle = getCSS('--text-dim');
      ctx.font = '11px ' + getCSS('--font');
      ctx.textAlign = 'center';
      ctx.fillText('Collecting thermal data...', W / 2, H / 2);
      return;
    }

    var data = state.history;
    var maxTemp = 105;
    var minTemp = 20;
    var maxFan = state.maxFanSpeed || 3000;

    // Grid lines
    ctx.strokeStyle = getCSS('--border');
    ctx.lineWidth = 0.5;
    ctx.setLineDash([3, 3]);
    for (var t = 20; t <= 100; t += 10) {
      var gy = pad.top + ch - ((t - minTemp) / (maxTemp - minTemp)) * ch;
      ctx.beginPath();
      ctx.moveTo(pad.left, gy);
      ctx.lineTo(pad.left + cw, gy);
      ctx.stroke();
    }
    ctx.setLineDash([]);

    // Threshold lines
    var thresholds = [
      { val: state.thresholds.warning, color: getCSS('--accent-amber'), label: 'Warn' },
      { val: state.thresholds.throttle, color: getCSS('--accent-red'), label: 'Throttle' },
      { val: state.thresholds.critical, color: '#ff2020', label: 'Critical' }
    ];
    ctx.setLineDash([6, 4]);
    ctx.lineWidth = 1;
    thresholds.forEach(function(th) {
      var ty = pad.top + ch - ((th.val - minTemp) / (maxTemp - minTemp)) * ch;
      ctx.strokeStyle = th.color;
      ctx.beginPath();
      ctx.moveTo(pad.left, ty);
      ctx.lineTo(pad.left + cw, ty);
      ctx.stroke();
      ctx.fillStyle = th.color;
      ctx.font = '9px ' + getCSS('--font');
      ctx.textAlign = 'left';
      ctx.fillText(th.label + ' ' + th.val + '\u00B0', pad.left + cw + 4, ty + 3);
    });
    ctx.setLineDash([]);

    // Y-axis labels (temperature)
    ctx.fillStyle = getCSS('--text-dim');
    ctx.font = '9px ' + getCSS('--mono');
    ctx.textAlign = 'right';
    for (var t2 = 20; t2 <= 100; t2 += 20) {
      var ly = pad.top + ch - ((t2 - minTemp) / (maxTemp - minTemp)) * ch;
      ctx.fillText(t2 + '\u00B0', pad.left - 6, ly + 3);
    }

    // Y-axis right labels (fan RPM)
    ctx.textAlign = 'left';
    ctx.fillStyle = getCSS('--accent-purple');
    for (var f = 0; f <= maxFan; f += Math.round(maxFan / 4)) {
      var fy = pad.top + ch - (f / maxFan) * ch;
      ctx.fillText(f + '', pad.left + cw + 4, fy + 3);
    }

    // X-axis labels (time)
    ctx.fillStyle = getCSS('--text-dim');
    ctx.textAlign = 'center';
    var timeLabels = ['-60m', '-45m', '-30m', '-15m', 'Now'];
    timeLabels.forEach(function(lbl, i) {
      var tx = pad.left + (cw * i / (timeLabels.length - 1));
      ctx.fillText(lbl, tx, H - 6);
    });

    // Axis labels
    ctx.font = '9px ' + getCSS('--font');
    ctx.fillStyle = getCSS('--text-dim');
    ctx.save();
    ctx.translate(10, pad.top + ch / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.textAlign = 'center';
    ctx.fillText('Temperature (\u00B0C)', 0, 0);
    ctx.restore();

    // Plot functions
    function plotLine(key, color, yScale) {
      ctx.beginPath();
      ctx.strokeStyle = color;
      ctx.lineWidth = 2;
      ctx.lineJoin = 'round';
      data.forEach(function(pt, idx) {
        var px = pad.left + (idx / (data.length - 1)) * cw;
        var val = pt[key] || 0;
        var py;
        if (yScale === 'fan') {
          py = pad.top + ch - (val / maxFan) * ch;
        } else {
          py = pad.top + ch - ((val - minTemp) / (maxTemp - minTemp)) * ch;
        }
        if (idx === 0) ctx.moveTo(px, py);
        else ctx.lineTo(px, py);
      });
      ctx.stroke();
    }

    function plotArea(key, color, yScale) {
      ctx.beginPath();
      data.forEach(function(pt, idx) {
        var px = pad.left + (idx / (data.length - 1)) * cw;
        var val = pt[key] || 0;
        var py;
        if (yScale === 'fan') {
          py = pad.top + ch - (val / maxFan) * ch;
        } else {
          py = pad.top + ch - ((val - minTemp) / (maxTemp - minTemp)) * ch;
        }
        if (idx === 0) ctx.moveTo(px, py);
        else ctx.lineTo(px, py);
      });
      // close area
      ctx.lineTo(pad.left + cw, pad.top + ch);
      ctx.lineTo(pad.left, pad.top + ch);
      ctx.closePath();
      ctx.fillStyle = color;
      ctx.globalAlpha = 0.08;
      ctx.fill();
      ctx.globalAlpha = 1;
    }

    // Fan speed area + line
    plotArea('fanSpeed', getCSS('--accent-purple'), 'fan');
    plotLine('fanSpeed', getCSS('--accent-purple') + '80', 'fan');

    // GPU temp area + line
    plotArea('gpuTemp', getCSS('--accent-amber'), 'temp');
    plotLine('gpuTemp', getCSS('--accent-amber'), 'temp');

    // CPU temp line (on top)
    plotArea('cpuTemp', getCSS('--accent-cyan'), 'temp');
    plotLine('cpuTemp', getCSS('--accent-cyan'), 'temp');

    // Current value indicators (dots at the right end)
    if (data.length > 0) {
      var last = data[data.length - 1];
      var dotSize = 4;
      var px = pad.left + cw;

      [[last.cpuTemp, getCSS('--accent-cyan'), 'temp'],
       [last.gpuTemp, getCSS('--accent-amber'), 'temp'],
       [last.fanSpeed, getCSS('--accent-purple'), 'fan']].forEach(function(d) {
        var val = d[0] || 0;
        var py;
        if (d[2] === 'fan') py = pad.top + ch - (val / maxFan) * ch;
        else py = pad.top + ch - ((val - minTemp) / (maxTemp - minTemp)) * ch;
        ctx.beginPath();
        ctx.arc(px, py, dotSize, 0, Math.PI * 2);
        ctx.fillStyle = d[1];
        ctx.fill();
      });
    }

    // Legend
    var legendY = 10;
    var legendItems = [
      { label: 'CPU Temp', color: getCSS('--accent-cyan') },
      { label: 'GPU Temp', color: getCSS('--accent-amber') },
      { label: 'Fan Speed', color: getCSS('--accent-purple') }
    ];
    ctx.font = '9px ' + getCSS('--font');
    var lx = pad.left + 4;
    legendItems.forEach(function(item) {
      ctx.fillStyle = item.color;
      ctx.fillRect(lx, legendY - 4, 12, 3);
      ctx.fillStyle = getCSS('--text-secondary');
      ctx.textAlign = 'left';
      ctx.fillText(item.label, lx + 16, legendY);
      lx += ctx.measureText(item.label).width + 30;
    });
  }

  /* ── Thermal zone map ─────────────────────────────────────────── */
  function drawZoneMap(container) {
    container.innerHTML = '';
    var zones = [
      { label: 'Green', min: 0, max: state.thresholds.warning, pct: state.zones.green, color: '--accent-green' },
      { label: 'Yellow', min: state.thresholds.warning, max: state.thresholds.throttle, pct: state.zones.yellow, color: '--accent-amber' },
      { label: 'Red', min: state.thresholds.throttle, max: state.thresholds.critical, pct: state.zones.red, color: '--accent-red' }
    ];
    zones.forEach(function(z) {
      var row = document.createElement('div');
      row.style.cssText = 'display:flex;align-items:center;gap:8px;padding:4px 0';

      var lbl = document.createElement('span');
      lbl.style.cssText = 'font-size:10px;color:var(--text-secondary);width:50px;flex-shrink:0';
      lbl.textContent = z.label;

      var barWrap = document.createElement('div');
      barWrap.style.cssText = 'flex:1;height:8px;background:var(--gauge-track);border-radius:4px;overflow:hidden';
      var barFill = document.createElement('div');
      barFill.style.cssText = 'height:100%;border-radius:4px;transition:width .6s ease;width:' + z.pct + '%;background:var(' + z.color + ')';
      barWrap.appendChild(barFill);

      var val = document.createElement('span');
      val.style.cssText = 'font-size:10px;color:var(--text-dim);width:65px;text-align:right;font-family:var(--mono)';
      val.textContent = z.min + '-' + z.max + '\u00B0C';

      row.appendChild(lbl);
      row.appendChild(barWrap);
      row.appendChild(val);
      container.appendChild(row);
    });
  }

  /* ── Process thermal impact table ─────────────────────────────── */
  function drawProcessTable(container) {
    container.innerHTML = '';

    // Header
    var header = document.createElement('div');
    header.style.cssText = 'display:grid;grid-template-columns:1fr 50px 80px 60px 80px;gap:6px;padding:6px 0;border-bottom:1px solid var(--border);font-size:9px;font-weight:600;text-transform:uppercase;letter-spacing:0.5px;color:var(--text-dim)';
    ['Process', 'CPU%', 'Thermal', 'Priority', 'Status'].forEach(function(h) {
      var s = document.createElement('span');
      s.textContent = h;
      header.appendChild(s);
    });
    container.appendChild(header);

    var procs = state.processes.length > 0 ? state.processes : generateDemoProcesses();

    procs.slice(0, 12).forEach(function(p) {
      var row = document.createElement('div');
      row.style.cssText = 'display:grid;grid-template-columns:1fr 50px 80px 60px 80px;gap:6px;padding:5px 0;border-bottom:1px solid var(--border);font-size:11px;align-items:center';

      var name = document.createElement('span');
      name.style.cssText = 'color:var(--text-primary);overflow:hidden;text-overflow:ellipsis;white-space:nowrap';
      name.textContent = p.name;

      var cpu = document.createElement('span');
      cpu.style.cssText = 'font-family:var(--mono);font-size:10px;color:' + (p.cpu > 50 ? 'var(--accent-red)' : p.cpu > 25 ? 'var(--accent-amber)' : 'var(--text-secondary)');
      cpu.textContent = p.cpu.toFixed(1) + '%';

      var thermal = document.createElement('div');
      thermal.style.cssText = 'display:flex;align-items:center;gap:4px';
      var thermalBar = document.createElement('div');
      thermalBar.style.cssText = 'flex:1;height:6px;background:var(--gauge-track);border-radius:3px;overflow:hidden';
      var thermalFill = document.createElement('div');
      var tc = p.thermalContrib > 30 ? '--accent-red' : p.thermalContrib > 15 ? '--accent-amber' : '--accent-green';
      thermalFill.style.cssText = 'height:100%;border-radius:3px;width:' + Math.min(p.thermalContrib, 100) + '%;background:var(' + tc + ')';
      thermalBar.appendChild(thermalFill);
      var thermalVal = document.createElement('span');
      thermalVal.style.cssText = 'font-size:9px;color:var(--text-dim);font-family:var(--mono);width:24px;text-align:right';
      thermalVal.textContent = p.thermalContrib + '%';
      thermal.appendChild(thermalBar);
      thermal.appendChild(thermalVal);

      var prio = document.createElement('span');
      prio.style.cssText = 'font-size:10px;color:var(--text-secondary)';
      prio.textContent = p.priority;

      var statusEl = document.createElement('span');
      var statusColor = p.throttled ? '--accent-red' : '--accent-green';
      statusEl.style.cssText = 'font-size:9px;padding:2px 6px;border-radius:3px;background:var(' + statusColor + '-dim, rgba(0,0,0,0.1));color:var(' + statusColor + ');font-weight:500';
      statusEl.textContent = p.throttled ? 'Throttled' : 'Normal';

      if (p.throttled) {
        row.style.cursor = 'pointer';
        row.title = 'Click to un-throttle';
        row.addEventListener('click', function() {
          ipcSend('throttle_process', { pid: p.pid, throttle: false });
        });
      } else if (p.thermalContrib > 20) {
        row.style.cursor = 'pointer';
        row.title = 'Click to throttle';
        row.addEventListener('click', function() {
          ipcSend('throttle_process', { pid: p.pid, throttle: true });
        });
      }

      row.appendChild(name);
      row.appendChild(cpu);
      row.appendChild(thermal);
      row.appendChild(prio);
      row.appendChild(statusEl);
      container.appendChild(row);
    });
  }

  function generateDemoProcesses() {
    return [
      { name: 'chrome.exe', pid: 1234, cpu: 12.3, thermalContrib: 18, priority: 'Normal', throttled: false },
      { name: 'ollama.exe', pid: 2345, cpu: 45.2, thermalContrib: 42, priority: 'High', throttled: false },
      { name: 'node.exe', pid: 3456, cpu: 8.7, thermalContrib: 10, priority: 'Normal', throttled: false },
      { name: 'vscode.exe', pid: 4567, cpu: 6.1, thermalContrib: 7, priority: 'Normal', throttled: false },
      { name: 'explorer.exe', pid: 5678, cpu: 1.2, thermalContrib: 2, priority: 'Normal', throttled: false },
      { name: 'Discord.exe', pid: 6789, cpu: 3.4, thermalContrib: 5, priority: 'Normal', throttled: false },
      { name: 'WindowsTerminal', pid: 7890, cpu: 2.1, thermalContrib: 3, priority: 'Normal', throttled: false },
      { name: 'rust-analyzer', pid: 8901, cpu: 15.4, thermalContrib: 22, priority: 'Below', throttled: true }
    ];
  }

  /* ── Create toggle ───────────────────────────────────────────── */
  function createToggle(label, active, onChange) {
    var row = document.createElement('div');
    row.style.cssText = 'display:flex;align-items:center;justify-content:space-between;padding:6px 0;font-size:11px;color:var(--text-secondary)';

    var lbl = document.createElement('span');
    lbl.textContent = label;

    var toggle = document.createElement('div');
    toggle.className = 'toggle' + (active ? ' on' : '');
    toggle.style.cssText = 'position:relative;width:34px;height:18px;flex-shrink:0;background:' + (active ? 'var(--accent-cyan)' : 'var(--border)') + ';border-radius:9px;cursor:pointer;transition:var(--transition)';

    var knob = document.createElement('div');
    knob.style.cssText = 'position:absolute;top:2px;left:' + (active ? '18' : '2') + 'px;width:14px;height:14px;border-radius:50%;background:var(--text-primary);transition:var(--transition)';
    toggle.appendChild(knob);

    toggle.addEventListener('click', function() {
      var isOn = toggle.classList.toggle('on');
      toggle.style.background = isOn ? 'var(--accent-cyan)' : 'var(--border)';
      knob.style.left = isOn ? '18px' : '2px';
      if (onChange) onChange(isOn);
    });

    row.appendChild(lbl);
    row.appendChild(toggle);
    return row;
  }

  /* ── Create button ───────────────────────────────────────────── */
  function createBtn(text, icon, onClick) {
    var btn = document.createElement('button');
    btn.className = 'btn';
    btn.innerHTML = '<span class="ico">' + icon + '</span> ';
    var span = document.createElement('span');
    span.textContent = text;
    btn.appendChild(span);
    btn.addEventListener('click', function() {
      if (onClick) onClick(btn);
    });
    return btn;
  }

  /* ── Threshold slider ────────────────────────────────────────── */
  function createThresholdSlider(label, key, min, max) {
    var row = document.createElement('div');
    row.style.cssText = 'padding:6px 0';

    var top = document.createElement('div');
    top.style.cssText = 'display:flex;justify-content:space-between;align-items:center;margin-bottom:4px';
    var lbl = document.createElement('span');
    lbl.style.cssText = 'font-size:10px;color:var(--text-secondary)';
    lbl.textContent = label;
    var val = document.createElement('span');
    val.style.cssText = 'font-size:10px;color:var(--accent-cyan);font-family:var(--mono)';
    val.textContent = state.thresholds[key] + '\u00B0C';

    top.appendChild(lbl);
    top.appendChild(val);

    var input = document.createElement('input');
    input.type = 'range';
    input.min = min;
    input.max = max;
    input.value = state.thresholds[key];
    input.style.cssText = 'width:100%;accent-color:var(--accent-cyan);height:4px';
    input.addEventListener('input', function() {
      state.thresholds[key] = parseInt(input.value);
      val.textContent = input.value + '\u00B0C';
    });
    input.addEventListener('change', function() {
      ipcSend('set_thermal_config', { thresholds: state.thresholds });
    });

    row.appendChild(top);
    row.appendChild(input);
    return row;
  }

  /* ── Power plan buttons ──────────────────────────────────────── */
  function createPowerPlanRow(container) {
    var plans = [
      { id: 'power_saver', label: 'Power Saver', icon: '\uD83D\uDD0B' },
      { id: 'balanced', label: 'Balanced', icon: '\u2696' },
      { id: 'high_performance', label: 'Performance', icon: '\u26A1' },
      { id: 'ultimate', label: 'Ultimate', icon: '\uD83D\uDD25' }
    ];

    var row = document.createElement('div');
    row.style.cssText = 'display:flex;gap:6px;flex-wrap:wrap;margin-top:6px';
    row.setAttribute('data-power-plans', '');

    plans.forEach(function(plan) {
      var btn = document.createElement('button');
      btn.style.cssText = 'flex:1;min-width:70px;padding:6px 8px;border:1px solid var(--border);border-radius:6px;background:' + (state.powerPlan === plan.id ? 'var(--accent-cyan-dim)' : 'var(--bg-card)') + ';color:' + (state.powerPlan === plan.id ? 'var(--accent-cyan)' : 'var(--text-secondary)') + ';font-size:10px;font-family:var(--font);cursor:pointer;transition:var(--transition);text-align:center';
      if (state.powerPlan === plan.id) {
        btn.style.borderColor = 'var(--accent-cyan)';
      }
      btn.textContent = plan.label;
      btn.addEventListener('click', function() {
        state.powerPlan = plan.id;
        ipcSend('set_thermal_config', { power_plan: plan.id });
        // Update all buttons
        var btns = row.children;
        for (var i = 0; i < btns.length; i++) {
          var isPlan = plans[i].id === plan.id;
          btns[i].style.background = isPlan ? 'var(--accent-cyan-dim)' : 'var(--bg-card)';
          btns[i].style.color = isPlan ? 'var(--accent-cyan)' : 'var(--text-secondary)';
          btns[i].style.borderColor = isPlan ? 'var(--accent-cyan)' : 'var(--border)';
        }
      });
      row.appendChild(btn);
    });
    container.appendChild(row);
  }

  /* ── Prediction card ─────────────────────────────────────────── */
  function createPredictionCard() {
    var card = document.createElement('div');
    card.className = 'card';
    card.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px';

    var title = document.createElement('div');
    title.className = 'card-title';
    title.textContent = 'Thermal Prediction';
    card.appendChild(title);

    var body = document.createElement('div');
    body.style.cssText = 'display:flex;align-items:center;gap:14px';

    var indicator = document.createElement('div');
    indicator.style.cssText = 'width:48px;height:48px;border-radius:50%;display:flex;align-items:center;justify-content:center;font-size:20px;flex-shrink:0';
    indicator.setAttribute('data-prediction-icon', '');
    indicator.textContent = '\u2714';

    var info = document.createElement('div');
    var mainText = document.createElement('div');
    mainText.style.cssText = 'font-size:13px;font-weight:500;color:var(--text-primary)';
    mainText.setAttribute('data-prediction-text', '');
    mainText.textContent = 'No throttling expected';
    var subText = document.createElement('div');
    subText.style.cssText = 'font-size:10px;color:var(--text-secondary);margin-top:2px';
    subText.setAttribute('data-prediction-sub', '');
    subText.textContent = 'Current load is within safe thermal limits';

    info.appendChild(mainText);
    info.appendChild(subText);
    body.appendChild(indicator);
    body.appendChild(info);
    card.appendChild(body);

    return card;
  }

  /* ── Active throttling indicator ─────────────────────────────── */
  function createThrottleIndicator() {
    var card = document.createElement('div');
    card.className = 'card';
    card.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px';

    var title = document.createElement('div');
    title.className = 'card-title';
    title.textContent = 'Active Throttling';
    card.appendChild(title);

    var indicator = document.createElement('div');
    indicator.setAttribute('data-throttle-indicator', '');
    indicator.style.cssText = 'display:flex;align-items:center;gap:8px;padding:8px 10px;border-radius:6px;font-size:12px';

    card.appendChild(indicator);
    return card;
  }

  /* ══════════════════════════════════════════════════════════════ */
  /* INIT                                                          */
  /* ══════════════════════════════════════════════════════════════ */
  window.RuVectorPages.thermal_Init = function(container) {
    state.container = container;
    container.innerHTML = '';

    var inner = document.createElement('div');
    inner.className = 'page-inner';
    inner.style.cssText = 'padding:24px 28px;max-width:1400px';

    // Header
    var header = document.createElement('div');
    header.className = 'page-header';
    header.innerHTML = '<span class="page-icon">&#127777;</span><h2>Thermal-Aware Scheduler</h2>';
    inner.appendChild(header);

    var status = document.createElement('div');
    status.className = 'page-status';
    status.style.cssText = 'display:inline-block;font-size:9px;font-weight:600;letter-spacing:1px;text-transform:uppercase;padding:3px 10px;border-radius:4px;margin-bottom:16px;background:var(--accent-cyan-dim);color:var(--accent-cyan)';
    status.textContent = 'ADR-020 \u00B7 Active';
    inner.appendChild(status);

    var desc = document.createElement('p');
    desc.className = 'page-desc';
    desc.textContent = 'Monitors CPU and GPU thermal zones, adjusts memory operations to avoid thermal throttling, implements core migration strategies, and provides a Silent Mode for quiet operation.';
    inner.appendChild(desc);

    // ── Top row: gauges + prediction + throttle ──
    var topRow = document.createElement('div');
    topRow.style.cssText = 'display:grid;grid-template-columns:1fr 1fr 1fr;gap:16px;margin-bottom:16px';

    // Gauge card
    var gaugeCard = document.createElement('div');
    gaugeCard.className = 'card';
    gaugeCard.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px';
    var gaugeTitle = document.createElement('div');
    gaugeTitle.className = 'card-title';
    gaugeTitle.textContent = 'Temperature';
    gaugeCard.appendChild(gaugeTitle);

    var gaugeRow = document.createElement('div');
    gaugeRow.style.cssText = 'display:flex;justify-content:space-around;align-items:center;gap:12px';

    var cpuGauge = createCircularGauge('cpu-temp', 'CPU', '\u00B0C', 105, 90);
    var gpuGauge = createCircularGauge('gpu-temp', 'GPU', '\u00B0C', 105, 90);
    var fanGauge = createCircularGauge('fan-speed', 'Fan', 'RPM', state.maxFanSpeed, 90);

    gaugeRow.appendChild(cpuGauge.element);
    gaugeRow.appendChild(gpuGauge.element);
    gaugeRow.appendChild(fanGauge.element);
    gaugeCard.appendChild(gaugeRow);

    // Throttle state indicator
    var throttleState = document.createElement('div');
    throttleState.style.cssText = 'margin-top:10px;text-align:center;padding:6px;border-radius:6px;font-size:11px;font-weight:500';
    throttleState.setAttribute('data-throttle-state', '');
    throttleState.textContent = 'Throttle: None';
    throttleState.style.background = 'var(--accent-green-dim, rgba(107,203,119,0.1))';
    throttleState.style.color = 'var(--accent-green)';
    gaugeCard.appendChild(throttleState);

    topRow.appendChild(gaugeCard);

    // Prediction card
    topRow.appendChild(createPredictionCard());

    // Active throttling card
    topRow.appendChild(createThrottleIndicator());

    inner.appendChild(topRow);

    // ── Chart row ──
    var chartCard = document.createElement('div');
    chartCard.className = 'card';
    chartCard.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px;margin-bottom:16px';
    var chartTitle = document.createElement('div');
    chartTitle.className = 'card-title';
    chartTitle.textContent = 'Temperature History (Last Hour)';
    chartCard.appendChild(chartTitle);

    var chartWrap = document.createElement('div');
    chartWrap.style.cssText = 'width:100%;height:220px;position:relative';
    var chartCanvas = document.createElement('canvas');
    chartCanvas.style.cssText = 'width:100%;height:100%;display:block';
    chartWrap.appendChild(chartCanvas);
    chartCard.appendChild(chartWrap);
    state.canvasEl = chartCanvas;
    inner.appendChild(chartCard);

    // ── Bottom row: zones + processes + settings ──
    var bottomRow = document.createElement('div');
    bottomRow.style.cssText = 'display:grid;grid-template-columns:280px 1fr 280px;gap:16px';

    // Thermal zones
    var zoneCard = document.createElement('div');
    zoneCard.className = 'card';
    zoneCard.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px';
    var zoneTitle = document.createElement('div');
    zoneTitle.className = 'card-title';
    zoneTitle.textContent = 'Thermal Zones';
    zoneCard.appendChild(zoneTitle);
    var zoneContainer = document.createElement('div');
    zoneContainer.setAttribute('data-zone-map', '');
    zoneCard.appendChild(zoneContainer);
    drawZoneMap(zoneContainer);

    // Power plan
    var planTitle = document.createElement('div');
    planTitle.className = 'card-title';
    planTitle.style.marginTop = '14px';
    planTitle.textContent = 'Power Plan';
    zoneCard.appendChild(planTitle);
    createPowerPlanRow(zoneCard);

    bottomRow.appendChild(zoneCard);

    // Process table
    var procCard = document.createElement('div');
    procCard.className = 'card';
    procCard.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px;overflow:hidden';
    var procTitle = document.createElement('div');
    procTitle.className = 'card-title';
    procTitle.textContent = 'Process Thermal Impact';
    procCard.appendChild(procTitle);
    var procContainer = document.createElement('div');
    procContainer.setAttribute('data-proc-table', '');
    procContainer.style.cssText = 'max-height:280px;overflow-y:auto';
    procCard.appendChild(procContainer);
    drawProcessTable(procContainer);
    bottomRow.appendChild(procCard);

    // Settings card
    var settingsCard = document.createElement('div');
    settingsCard.className = 'card';
    settingsCard.style.cssText = 'background:var(--bg-card);border:1px solid var(--border);border-radius:var(--radius);padding:14px';
    var settingsTitle = document.createElement('div');
    settingsTitle.className = 'card-title';
    settingsTitle.textContent = 'Throttling Settings';
    settingsCard.appendChild(settingsTitle);

    settingsCard.appendChild(createThresholdSlider('Warning Threshold', 'warning', 50, 80));
    settingsCard.appendChild(createThresholdSlider('Throttle Threshold', 'throttle', 70, 95));
    settingsCard.appendChild(createThresholdSlider('Critical Threshold', 'critical', 85, 105));

    var divider = document.createElement('div');
    divider.style.cssText = 'height:1px;background:var(--border);margin:8px 0';
    settingsCard.appendChild(divider);

    settingsCard.appendChild(createToggle('Silent Mode', state.silentMode, function(v) {
      state.silentMode = v;
      ipcSend('set_thermal_config', { silent_mode: v });
    }));
    settingsCard.appendChild(createToggle('Core Migration', state.coreMigration, function(v) {
      state.coreMigration = v;
      ipcSend('set_thermal_config', { core_migration: v });
    }));
    settingsCard.appendChild(createToggle('Thermal Prediction', state.thermalPrediction, function(v) {
      state.thermalPrediction = v;
      ipcSend('set_thermal_config', { thermal_prediction: v });
    }));

    bottomRow.appendChild(settingsCard);
    inner.appendChild(bottomRow);

    container.appendChild(inner);

    // Initial draw
    drawHistoryChart();

    // Generate demo history data
    if (state.history.length === 0) {
      for (var i = 0; i < 60; i++) {
        state.history.push({
          ts: Date.now() - (60 - i) * 60000,
          cpuTemp: 45 + Math.sin(i * 0.15) * 12 + Math.random() * 5,
          gpuTemp: 38 + Math.sin(i * 0.12) * 10 + Math.random() * 4,
          fanSpeed: 800 + Math.sin(i * 0.15) * 400 + Math.random() * 200
        });
      }
    }

    // Request initial data
    ipcSend('get_thermal_status');
    ipcSend('get_thermal_history');
    ipcSend('get_fan_status');

    state.initialized = true;
    updateUI();
  };

  /* ══════════════════════════════════════════════════════════════ */
  /* UPDATE (called from Rust via IPC or polling)                  */
  /* ══════════════════════════════════════════════════════════════ */
  window.RuVectorPages.thermal_Update = function(data) {
    if (!data) return;

    if (data.cpu_temp !== undefined) state.cpuTemp = data.cpu_temp;
    if (data.gpu_temp !== undefined) state.gpuTemp = data.gpu_temp;
    if (data.fan_speed !== undefined) state.fanSpeed = data.fan_speed;
    if (data.max_fan_speed !== undefined) state.maxFanSpeed = data.max_fan_speed;
    if (data.throttle_state !== undefined) state.throttleState = data.throttle_state;
    if (data.power_plan !== undefined) state.powerPlan = data.power_plan;
    if (data.thresholds) {
      if (data.thresholds.warning !== undefined) state.thresholds.warning = data.thresholds.warning;
      if (data.thresholds.throttle !== undefined) state.thresholds.throttle = data.thresholds.throttle;
      if (data.thresholds.critical !== undefined) state.thresholds.critical = data.thresholds.critical;
    }
    if (data.zones) {
      state.zones = data.zones;
    }
    if (data.processes) {
      state.processes = data.processes;
    }
    if (data.history) {
      state.history = data.history;
    }
    if (data.prediction_minutes !== undefined) {
      state.predictionMinutes = data.prediction_minutes;
    }

    // Append current reading to history
    if (data.cpu_temp !== undefined) {
      state.history.push({
        ts: Date.now(),
        cpuTemp: state.cpuTemp,
        gpuTemp: state.gpuTemp,
        fanSpeed: state.fanSpeed
      });
      if (state.history.length > state.historyMaxLen) {
        state.history.shift();
      }
    }

    if (state.initialized) updateUI();
  };

  /* ── Refresh display ─────────────────────────────────────────── */
  function updateUI() {
    if (!state.container) return;

    // Gauges
    var cpuCanvas = state.container.querySelector('[data-gauge-id="cpu-temp"]');
    if (cpuCanvas) {
      drawGauge(cpuCanvas, state.cpuTemp, 105, tempColor(state.cpuTemp));
      var cpuVal = state.container.querySelector('[data-gauge-val="cpu-temp"]');
      if (cpuVal) cpuVal.textContent = Math.round(state.cpuTemp);
    }

    var gpuCanvas = state.container.querySelector('[data-gauge-id="gpu-temp"]');
    if (gpuCanvas) {
      drawGauge(gpuCanvas, state.gpuTemp, 105, tempColor(state.gpuTemp));
      var gpuVal = state.container.querySelector('[data-gauge-val="gpu-temp"]');
      if (gpuVal) gpuVal.textContent = Math.round(state.gpuTemp);
    }

    var fanCanvas = state.container.querySelector('[data-gauge-id="fan-speed"]');
    if (fanCanvas) {
      drawGauge(fanCanvas, state.fanSpeed, state.maxFanSpeed, getCSS('--accent-purple'));
      var fanVal = state.container.querySelector('[data-gauge-val="fan-speed"]');
      if (fanVal) fanVal.textContent = Math.round(state.fanSpeed);
    }

    // Throttle state
    var tsEl = state.container.querySelector('[data-throttle-state]');
    if (tsEl) {
      if (state.throttleState === 'none') {
        tsEl.textContent = 'Throttle: None';
        tsEl.style.background = 'rgba(107,203,119,0.1)';
        tsEl.style.color = 'var(--accent-green)';
      } else if (state.throttleState === 'thermal') {
        tsEl.textContent = 'Throttle: Thermal Active';
        tsEl.style.background = 'rgba(224,96,96,0.1)';
        tsEl.style.color = 'var(--accent-red)';
      } else {
        tsEl.textContent = 'Throttle: ' + state.throttleState;
        tsEl.style.background = 'rgba(212,165,116,0.1)';
        tsEl.style.color = 'var(--accent-amber)';
      }
    }

    // Prediction
    var predIcon = state.container.querySelector('[data-prediction-icon]');
    var predText = state.container.querySelector('[data-prediction-text]');
    var predSub = state.container.querySelector('[data-prediction-sub]');
    if (predIcon && predText && predSub) {
      if (state.predictionMinutes === null || state.predictionMinutes > 60) {
        predIcon.textContent = '\u2714';
        predIcon.style.cssText = 'width:48px;height:48px;border-radius:50%;display:flex;align-items:center;justify-content:center;font-size:20px;flex-shrink:0;background:rgba(107,203,119,0.15);color:var(--accent-green)';
        predText.textContent = 'No throttling expected';
        predSub.textContent = 'Current load is within safe thermal limits';
      } else if (state.predictionMinutes > 10) {
        predIcon.textContent = '\u26A0';
        predIcon.style.cssText = 'width:48px;height:48px;border-radius:50%;display:flex;align-items:center;justify-content:center;font-size:20px;flex-shrink:0;background:rgba(212,165,116,0.15);color:var(--accent-amber)';
        predText.textContent = 'Throttle in ~' + Math.round(state.predictionMinutes) + ' min';
        predSub.textContent = 'Temperature trending upward under sustained load';
      } else {
        predIcon.textContent = '\u26D4';
        predIcon.style.cssText = 'width:48px;height:48px;border-radius:50%;display:flex;align-items:center;justify-content:center;font-size:20px;flex-shrink:0;background:rgba(224,96,96,0.15);color:var(--accent-red)';
        predText.textContent = 'Throttle imminent (~' + Math.round(state.predictionMinutes) + ' min)';
        predSub.textContent = 'Consider reducing load or enabling Silent Mode';
      }
    }

    // Active throttling indicator
    var throttleInd = state.container.querySelector('[data-throttle-indicator]');
    if (throttleInd) {
      var throttledProcs = state.processes.filter(function(p) { return p.throttled; });
      if (throttledProcs.length === 0) {
        throttleInd.style.background = 'rgba(107,203,119,0.08)';
        throttleInd.style.color = 'var(--accent-green)';
        throttleInd.textContent = 'No processes are being throttled';
      } else {
        throttleInd.innerHTML = '';
        throttleInd.style.background = 'rgba(224,96,96,0.08)';
        throttleInd.style.color = 'var(--accent-red)';
        var countLine = document.createElement('div');
        countLine.style.cssText = 'font-weight:500;margin-bottom:4px';
        countLine.textContent = throttledProcs.length + ' process' + (throttledProcs.length > 1 ? 'es' : '') + ' throttled';
        throttleInd.appendChild(countLine);
        throttledProcs.slice(0, 4).forEach(function(p) {
          var line = document.createElement('div');
          line.style.cssText = 'font-size:10px;color:var(--text-secondary);padding:2px 0';
          line.textContent = p.name + ' (CPU: ' + p.cpu.toFixed(1) + '%)';
          throttleInd.appendChild(line);
        });
      }
    }

    // Zone map
    var zoneMap = state.container.querySelector('[data-zone-map]');
    if (zoneMap) drawZoneMap(zoneMap);

    // Process table
    var procTable = state.container.querySelector('[data-proc-table]');
    if (procTable) drawProcessTable(procTable);

    // History chart
    drawHistoryChart();
  }

})();
