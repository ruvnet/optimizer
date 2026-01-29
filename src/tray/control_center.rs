//! Control Center – WebView2-based system dashboard
//!
//! Opens a native window with an embedded WebView2 showing real-time
//! memory metrics, process information, and a neural world model
//! visualization powered by Three.js.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::settings::TraySettings;
use crate::accel::CpuCapabilities;
use crate::windows::memory::WindowsMemoryOptimizer;

/// Prevents opening multiple Control Center windows simultaneously.
static CC_OPEN: AtomicBool = AtomicBool::new(false);

/// Events sent from the IPC handler to the tao event loop.
#[derive(Debug)]
enum CenterEvent {
    RunScript(String),
}

/// Open the Control Center window (non-blocking, spawns a thread).
pub fn open(settings: Arc<Mutex<TraySettings>>) {
    if CC_OPEN
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        tracing::info!("Control Center already open");
        return;
    }

    std::thread::spawn(move || {
        tracing::info!("Opening Control Center");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Err(e) = run(settings) {
                tracing::error!("Control Center error: {}", e);
            }
        }));
        if let Err(panic) = result {
            let msg = if let Some(s) = panic.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic.downcast_ref::<String>() {
                s.clone()
            } else {
                format!("{:?}", panic)
            };
            tracing::error!("Control Center PANIC: {}", msg);
        }
        CC_OPEN.store(false, Ordering::SeqCst);
        tracing::info!("Control Center closed");
    });
}

/// Create and run the Control Center window until the user closes it.
fn run(settings: Arc<Mutex<TraySettings>>) -> Result<(), Box<dyn std::error::Error>> {
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::EventLoopBuilder;
    use tao::platform::run_return::EventLoopExtRunReturn;
    use tao::platform::windows::EventLoopBuilderExtWindows;
    use tao::window::WindowBuilder;
    use wry::WebViewBuilder;

    let mut event_loop = EventLoopBuilder::<CenterEvent>::with_user_event()
        .with_any_thread(true)
        .build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("RuVector Control Center")
        .with_inner_size(tao::dpi::LogicalSize::new(1280.0, 800.0))
        .with_min_inner_size(tao::dpi::LogicalSize::new(900.0, 600.0))
        .build(&event_loop)?;

    // Read current settings for initial UI state
    let (theme, welcome_shown) = {
        let s = settings.lock().unwrap();
        (s.theme.clone(), s.welcome_shown)
    };

    // Embed HTML and inject current settings via template replacement
    let html_src = include_str!("../web/index.html");

    // Inline component scripts (embedded at compile time)
    let component_scripts = [
        concat!("<script>", include_str!("../web/components/profiles.js"), "</script>"),
        concat!("<script>", include_str!("../web/components/health.js"), "</script>"),
        concat!("<script>", include_str!("../web/components/startup.js"), "</script>"),
        concat!("<script>", include_str!("../web/components/wsl2.js"), "</script>"),
        concat!("<script>", include_str!("../web/components/build.js"), "</script>"),
        concat!("<script>", include_str!("../web/components/leaks.js"), "</script>"),
        concat!("<script>", include_str!("../web/components/prefetch.js"), "</script>"),
        concat!("<script>", include_str!("../web/components/thermal.js"), "</script>"),
        concat!("<script>", include_str!("../web/components/plugins.js"), "</script>"),
        concat!("<script>", include_str!("../web/components/gpu.js"), "</script>"),
        concat!("<script>", include_str!("../web/components/bloatware.js"), "</script>"),
        concat!("<script>", include_str!("../web/components/timeline.js"), "</script>"),
        concat!("<script>", include_str!("../web/components/agent.js"), "</script>"),
    ].join("\n");

    let html = html_src
        .replace("<!-- {{COMPONENT_SCRIPTS}} -->", &component_scripts)
        .replace("{{THEME}}", &theme)
        .replace("{{WELCOME_SHOWN}}", &welcome_shown.to_string());

    let settings_ipc = settings.clone();
    let proxy_ipc = proxy.clone();

    let webview = WebViewBuilder::new(&window)
        .with_html(&html)
        .with_devtools(cfg!(debug_assertions))
        .with_ipc_handler(move |req| {
            let body: &str = req.body();
            handle_ipc(&settings_ipc, body, &proxy_ipc);
        })
        .build()?;

    // Push initial metrics and settings
    let init_json = gather_metrics_json();
    let _ = webview.evaluate_script(&format!(
        "if(window.updateMetrics)window.updateMetrics({})",
        init_json
    ));
    let settings_json = gather_settings_json(&settings);
    let _ = webview.evaluate_script(&format!(
        "if(window.updateSettings)window.updateSettings({})",
        settings_json
    ));

    event_loop.run_return(move |event, _, control_flow| {
        *control_flow = tao::event_loop::ControlFlow::Wait;

        if let Event::UserEvent(CenterEvent::RunScript(ref js)) = event {
            let _ = webview.evaluate_script(js);
        }
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = tao::event_loop::ControlFlow::Exit;
        }
    });

    Ok(())
}

// ── IPC Message Handler ────────────────────────────────────────────

fn handle_ipc(
    settings: &Arc<Mutex<TraySettings>>,
    msg: &str,
    proxy: &tao::event_loop::EventLoopProxy<CenterEvent>,
) {
    let req: serde_json::Value = match serde_json::from_str(msg) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!("Invalid IPC message: {}", e);
            return;
        }
    };

    match req["type"].as_str() {
        Some("get_metrics") => {
            let json = gather_metrics_json();
            push_js(proxy, &format!("window.updateMetrics({})", json));
        }
        Some("get_processes") => {
            let json = gather_processes_json();
            push_js(proxy, &format!("window.updateProcesses({})", json));
        }
        Some("optimize") => {
            let aggressive = req["aggressive"].as_bool().unwrap_or(false);
            let json = run_optimize(aggressive);
            push_js(proxy, &format!("window.optimizeResult({})", json));
        }
        Some("get_settings") => {
            let json = gather_settings_json(settings);
            push_js(proxy, &format!("window.updateSettings({})", json));
        }
        Some("set_setting") => {
            if let Some(key) = req["key"].as_str() {
                apply_setting(settings, key, &req["value"]);
            }
        }
        Some("optimize_apps") => {
            let json = run_optimize_apps();
            push_js(proxy, &format!("window.optimizeAppsResult({})", json));
        }
        Some("set_theme") => {
            if let Some(t) = req["theme"].as_str() {
                if let Ok(mut s) = settings.lock() {
                    s.theme = t.to_string();
                    let _ = s.save();
                }
            }
        }
        Some("welcome_done") => {
            if let Ok(mut s) = settings.lock() {
                s.welcome_shown = true;
                let _ = s.save();
            }
        }
        _ => {
            tracing::debug!("Unknown IPC type: {:?}", req["type"]);
        }
    }
}

fn push_js(proxy: &tao::event_loop::EventLoopProxy<CenterEvent>, js: &str) {
    let _ = proxy.send_event(CenterEvent::RunScript(js.to_string()));
}

// ── Data Gathering ─────────────────────────────────────────────────

fn gather_metrics_json() -> String {
    let mut m = serde_json::Map::new();

    if let Ok(status) = WindowsMemoryOptimizer::get_memory_status() {
        m.insert(
            "memory_load".into(),
            serde_json::json!(status.memory_load_percent),
        );
        m.insert(
            "total_mb".into(),
            serde_json::json!(status.total_physical_mb),
        );
        m.insert(
            "used_mb".into(),
            serde_json::json!(status.used_physical_mb()),
        );
        m.insert(
            "available_mb".into(),
            serde_json::json!(status.available_physical_mb),
        );
    }

    let caps = CpuCapabilities::detect();
    m.insert("cpu_model".into(), serde_json::json!(caps.model));
    m.insert("cpu_cores".into(), serde_json::json!(caps.core_count));
    m.insert("avx2".into(), serde_json::json!(caps.has_avx2));
    m.insert("avx512".into(), serde_json::json!(caps.has_avx512));
    m.insert(
        "simd_speedup".into(),
        serde_json::json!(caps.estimated_speedup()),
    );

    serde_json::Value::Object(m).to_string()
}

fn gather_processes_json() -> String {
    use sysinfo::System;

    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut procs: Vec<(String, u64, u32)> = sys
        .processes()
        .iter()
        .map(|(pid, p)| {
            (
                p.name().to_string_lossy().to_string(),
                p.memory(),
                pid.as_u32(),
            )
        })
        .collect();

    procs.sort_by(|a, b| b.1.cmp(&a.1));
    procs.truncate(30);

    let list: Vec<serde_json::Value> = procs
        .iter()
        .map(|(name, mem, pid)| {
            serde_json::json!({
                "name": name,
                "memory_mb": (*mem as f64 / 1024.0 / 1024.0),
                "pid": pid
            })
        })
        .collect();

    serde_json::json!(list).to_string()
}

fn run_optimize(aggressive: bool) -> String {
    let optimizer = WindowsMemoryOptimizer::new();
    match optimizer.optimize(aggressive) {
        Ok(r) => serde_json::json!({
            "success": true,
            "freed_mb": r.freed_mb,
            "processes": r.processes_trimmed,
            "duration_ms": r.duration_ms
        })
        .to_string(),
        Err(e) => serde_json::json!({
            "success": false,
            "error": e.to_string()
        })
        .to_string(),
    }
}

fn gather_settings_json(settings: &Arc<Mutex<TraySettings>>) -> String {
    if let Ok(s) = settings.lock() {
        serde_json::json!({
            "auto_optimize": s.auto_optimize,
            "threshold": s.threshold,
            "interval_secs": s.interval_secs,
            "ai_game_mode": s.ai_mode.game_mode,
            "ai_focus_mode": s.ai_mode.focus_mode,
            "ai_thermal": s.ai_mode.thermal_prediction,
            "ai_preload": s.ai_mode.predictive_preload,
            "version": env!("CARGO_PKG_VERSION")
        })
        .to_string()
    } else {
        "{}".to_string()
    }
}

fn apply_setting(settings: &Arc<Mutex<TraySettings>>, key: &str, value: &serde_json::Value) {
    if let Ok(mut s) = settings.lock() {
        match key {
            "auto_optimize" => {
                if let Some(v) = value.as_bool() {
                    s.auto_optimize = v;
                }
            }
            "threshold" => {
                if let Some(v) = value.as_u64() {
                    s.threshold = v as u32;
                }
            }
            "interval_secs" => {
                if let Some(v) = value.as_u64() {
                    s.interval_secs = v;
                }
            }
            "ai_game_mode" => {
                if let Some(v) = value.as_bool() {
                    s.ai_mode.game_mode = v;
                }
            }
            "ai_focus_mode" => {
                if let Some(v) = value.as_bool() {
                    s.ai_mode.focus_mode = v;
                }
            }
            "ai_thermal" => {
                if let Some(v) = value.as_bool() {
                    s.ai_mode.thermal_prediction = v;
                }
            }
            "ai_preload" => {
                if let Some(v) = value.as_bool() {
                    s.ai_mode.predictive_preload = v;
                }
            }
            _ => {
                tracing::debug!("Unknown setting key: {}", key);
                return;
            }
        }
        let _ = s.save();
    }
}

fn run_optimize_apps() -> String {
    use sysinfo::System;
    use std::collections::HashMap;
    use std::time::Instant;

    let start = Instant::now();

    const APP_PATTERNS: &[(&str, &[&str])] = &[
        ("Brave", &["brave.exe", "brave"]),
        ("Chrome", &["chrome.exe", "chrome"]),
        ("Edge", &["msedge.exe", "msedge"]),
        ("VSCode", &["code.exe", "code"]),
        ("Discord", &["discord.exe", "discord"]),
        ("Spotify", &["spotify.exe", "spotify"]),
        ("WhatsApp", &["whatsapp.exe", "whatsapp"]),
        ("Slack", &["slack.exe", "slack"]),
        ("Teams", &["teams.exe", "ms-teams.exe", "teams"]),
        ("Zoom", &["zoom.exe", "zoom", "zoomus", "cpthost.exe"]),
        ("Obsidian", &["obsidian.exe", "obsidian"]),
        ("Notion", &["notion.exe", "notion"]),
        ("Figma", &["figma.exe", "figma"]),
    ];

    let mut system = System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut apps: HashMap<&str, Vec<u32>> = HashMap::new();
    for (pid, process) in system.processes() {
        let name = process.name().to_string_lossy().to_lowercase();
        for (app_name, patterns) in APP_PATTERNS {
            if patterns.iter().any(|p| name.contains(p)) {
                apps.entry(*app_name).or_default().push(pid.as_u32());
                break;
            }
        }
    }

    if apps.is_empty() {
        return serde_json::json!({
            "success": true,
            "freed_mb": 0.0,
            "processes": 0,
            "duration_ms": start.elapsed().as_millis() as u64
        })
        .to_string();
    }

    let mut total_trimmed = 0usize;
    let mut freed_mb = 0.0f64;

    for (_app_name, pids) in &apps {
        for pid in pids {
            match WindowsMemoryOptimizer::trim_process_working_set(*pid) {
                Ok(bytes_freed) => {
                    if bytes_freed > 0 {
                        freed_mb += bytes_freed as f64 / 1024.0 / 1024.0;
                        total_trimmed += 1;
                    }
                }
                Err(e) => {
                    tracing::debug!("Failed to trim app process {}: {}", pid, e);
                }
            }
        }
    }

    serde_json::json!({
        "success": true,
        "freed_mb": freed_mb,
        "processes": total_trimmed,
        "duration_ms": start.elapsed().as_millis() as u64
    })
    .to_string()
}
