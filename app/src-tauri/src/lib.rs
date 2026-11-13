pub mod decode;
pub mod engine;
pub mod history;
pub mod meter;
pub mod npcap;
pub mod packet;
pub mod raknet;
pub mod settings;

use engine::Engine;
use meter::{Meter, Snapshot};
use npcap::{Device, Next, Npcap};
use serde::Serialize;
use settings::Settings;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Capture only what can be game traffic: UDP, minus DNS/mDNS/SSDP/QUIC noise.
const BPF: &str = "udp and not port 53 and not port 5353 and not port 1900 and not port 443";
const OVERLAY: &str = "overlay";

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatus {
    pub running: bool,
    pub device: Option<String>,
    pub device_label: Option<String>,
    pub error: Option<String>,
    pub packets: u64,
    pub game_messages: u64,
    /// Seconds since the last game message (None = none seen yet).
    pub last_game_traffic: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Update {
    status: CaptureStatus,
    snapshot: Snapshot,
}

struct CaptureThread {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

struct AppState {
    npcap: Result<Arc<Npcap>, String>,
    meter: Arc<Mutex<Meter>>,
    status: Arc<Mutex<CaptureStatus>>,
    last_traffic: Arc<Mutex<Option<f64>>>,
    capture: Mutex<Option<CaptureThread>>,
    settings: Mutex<Settings>,
    settings_path: PathBuf,
    /// Saved fights go here unless the user picked another folder.
    default_history_dir: PathBuf,
}

impl AppState {
    fn history_dir(&self) -> PathBuf {
        match &self.settings.lock().unwrap().history_dir {
            Some(d) if !d.trim().is_empty() => PathBuf::from(d),
            _ => self.default_history_dir.clone(),
        }
    }
}

/// Write fights that just ended to the history folder.
fn save_finished(app: &AppHandle, state: &AppState, t: f64) {
    let fights = state.meter.lock().unwrap().take_finished(t);
    if fights.is_empty() {
        return;
    }
    let (enabled, min) = {
        let s = state.settings.lock().unwrap();
        (s.save_fights, s.min_saved_fight)
    };
    if !enabled {
        return;
    }
    let dir = state.history_dir();
    let mut saved = false;
    for f in fights.iter().filter(|f| f.duration >= min) {
        match history::save(&dir, f) {
            Ok(_) => saved = true,
            Err(e) => log::warn!("saving fight: {e}"),
        }
    }
    if saved {
        let _ = app.emit("fights-changed", ());
    }
}

fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

fn apply_meter_settings(meter: &mut Meter, s: &Settings) {
    meter.idle_timeout = s.idle_timeout.max(1.0);
    meter.stop_on_boss_kill = s.stop_on_boss_kill;
    meter.ranks = if s.ranks.is_empty() { None } else { Some(s.ranks.clone()) };
    meter.include_unknown = s.include_unknown;
}

fn apply_overlay_settings(app: &AppHandle, s: &Settings) {
    if let Some(w) = app.get_webview_window(OVERLAY) {
        let _ = if s.overlay.visible { w.show() } else { w.hide() };
        let _ = w.set_ignore_cursor_events(s.overlay.click_through);
        let _ = w.set_always_on_top(true);
    }
}

/// Make `wanted` the only global shortcut. Errors if it can't be parsed or another app owns it.
fn register_hotkey(app: &AppHandle, wanted: Option<&str>) -> Result<(), String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let Some(keys) = wanted.map(str::trim).filter(|k| !k.is_empty()) else { return Ok(()) };
    let shortcut: Shortcut = keys.parse().map_err(|e| format!("\"{keys}\" is not a valid shortcut: {e}"))?;
    gs.register(shortcut)
        .map_err(|e| format!("Couldn't use {keys}: another program may already use it ({e})."))
}

/// The overlay hotkey was pressed: flip the overlay and tell both windows.
fn toggle_overlay(app: &AppHandle) {
    let state = app.state::<AppState>();
    let settings = {
        let mut s = state.settings.lock().unwrap();
        s.overlay.visible = !s.overlay.visible;
        s.clone()
    };
    apply_overlay_settings(app, &settings);
    save_settings(&state, &settings);
    let _ = app.emit("settings", &settings);
}

fn save_settings(state: &AppState, s: &Settings) {
    if let Some(dir) = state.settings_path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(s) {
        let _ = std::fs::write(&state.settings_path, json);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceInfo {
    #[serde(flatten)]
    device: Device,
    /// The adapter Windows uses for internet traffic.
    is_default: bool,
}

#[tauri::command]
fn list_interfaces(state: State<AppState>) -> Result<Vec<DeviceInfo>, String> {
    let npcap = state.npcap.as_ref().map_err(|e| e.clone())?;
    let default = npcap.default_device().map(|d| d.name);
    Ok(npcap
        .devices()?
        .into_iter()
        .map(|d| DeviceInfo { is_default: Some(&d.name) == default.as_ref(), device: d })
        .collect())
}

fn stop_capture_inner(state: &AppState) {
    if let Some(c) = state.capture.lock().unwrap().take() {
        c.stop.store(true, Ordering::Relaxed);
        let _ = c.thread.join();
    }
    state.status.lock().unwrap().running = false;
}

fn start_capture_inner(state: &AppState) -> Result<(), String> {
    stop_capture_inner(state);
    let npcap = state.npcap.as_ref().map_err(|e| e.clone())?.clone();
    let wanted = state.settings.lock().unwrap().interface.clone();
    let device = match wanted {
        Some(name) => npcap
            .devices()?
            .into_iter()
            .find(|d| d.name == name)
            .ok_or_else(|| "The selected network adapter is not available.".to_string())?,
        None => npcap
            .default_device()
            .ok_or_else(|| "Could not find your internet adapter. Pick one in Settings.".to_string())?,
    };
    let label = if device.description.is_empty() { device.name.clone() } else { device.description.clone() };
    let mut cap = match npcap.open_live(&device.name, BPF) {
        Ok(c) => c,
        Err(e) => {
            let mut st = state.status.lock().unwrap();
            st.error = Some(e.clone());
            return Err(e);
        }
    };
    {
        let mut st = state.status.lock().unwrap();
        *st = CaptureStatus { running: true, device: Some(device.name.clone()), device_label: Some(label), ..Default::default() };
    }
    let stop = Arc::new(AtomicBool::new(false));
    let (meter, status, last_traffic, stop2) =
        (state.meter.clone(), state.status.clone(), state.last_traffic.clone(), stop.clone());
    let thread = std::thread::spawn(move || {
        let mut engine = Engine::default();
        let datalink = cap.datalink;
        while !stop2.load(Ordering::Relaxed) {
            match cap.next() {
                Ok(Next::Packet { time, data }) => {
                    let game = {
                        let mut m = meter.lock().unwrap();
                        engine.on_frame(&mut m, datalink, time, data)
                    };
                    if game {
                        *last_traffic.lock().unwrap() = Some(time);
                    }
                    let mut st = status.lock().unwrap();
                    st.packets = engine.packets;
                    st.game_messages = engine.game_messages;
                }
                Ok(Next::Timeout) => {}
                Ok(Next::Eof) => break,
                Err(e) => {
                    let mut st = status.lock().unwrap();
                    st.error = Some(e);
                    st.running = false;
                    return;
                }
            }
        }
    });
    *state.capture.lock().unwrap() = Some(CaptureThread { stop, thread });
    Ok(())
}

#[tauri::command]
fn start_capture(state: State<AppState>) -> Result<(), String> {
    start_capture_inner(&state)
}

#[tauri::command]
fn stop_capture(state: State<AppState>) {
    stop_capture_inner(&state);
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn set_settings(app: AppHandle, state: State<AppState>, settings: Settings) -> Result<(), String> {
    let interface_changed = state.settings.lock().unwrap().interface != settings.interface;
    let dir_changed = state.settings.lock().unwrap().history_dir != settings.history_dir;
    let old_hotkey = state.settings.lock().unwrap().overlay_hotkey.clone();
    if old_hotkey != settings.overlay_hotkey {
        if let Err(e) = register_hotkey(&app, settings.overlay_hotkey.as_deref()) {
            let _ = register_hotkey(&app, old_hotkey.as_deref());
            return Err(e);
        }
    }
    *state.settings.lock().unwrap() = settings.clone();
    apply_meter_settings(&mut state.meter.lock().unwrap(), &settings);
    apply_overlay_settings(&app, &settings);
    save_settings(&state, &settings);
    app.emit("settings", &settings).map_err(|e| e.to_string())?;
    if dir_changed {
        let _ = app.emit("fights-changed", ());
    }
    if interface_changed && state.status.lock().unwrap().running {
        start_capture_inner(&state)?;
    }
    Ok(())
}

#[tauri::command]
fn reset_meter(state: State<AppState>) {
    state.meter.lock().unwrap().reset();
}

#[tauri::command]
fn clear_history(state: State<AppState>) {
    state.meter.lock().unwrap().clear_history();
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryDirInfo {
    path: String,
    is_default: bool,
    default_path: String,
}

#[tauri::command]
fn get_history_dir(state: State<AppState>) -> HistoryDirInfo {
    let path = state.history_dir();
    HistoryDirInfo {
        is_default: path == state.default_history_dir,
        path: path.display().to_string(),
        default_path: state.default_history_dir.display().to_string(),
    }
}

#[tauri::command]
fn list_fights(state: State<AppState>) -> Vec<history::SavedFight> {
    history::list(&state.history_dir())
}

#[tauri::command]
fn delete_fight(app: AppHandle, state: State<AppState>, file: String) -> Result<(), String> {
    history::delete(&state.history_dir(), &file)?;
    let _ = app.emit("fights-changed", ());
    Ok(())
}

#[tauri::command]
fn open_history_dir(state: State<AppState>) -> Result<(), String> {
    let dir = state.history_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::process::Command::new("explorer").arg(&dir).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

fn build_update(state: &AppState) -> Update {
    let t = now();
    let mut status = state.status.lock().unwrap().clone();
    status.last_game_traffic = state.last_traffic.lock().unwrap().map(|lt| (t - lt).max(0.0));
    let snapshot = state.meter.lock().unwrap().snapshot(t);
    Update { status, snapshot }
}

#[tauri::command]
fn get_update(state: State<AppState>) -> Update {
    build_update(&state)
}

/// One display, in physical pixels on the virtual desktop.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MonitorInfo {
    name: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale: f64,
    primary: bool,
}

/// Overlay window position and size in physical pixels.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OverlayGeometry {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

fn overlay_geometry(app: &AppHandle) -> Result<OverlayGeometry, String> {
    let w = app.get_webview_window(OVERLAY).ok_or("overlay window missing")?;
    let pos = w.outer_position().map_err(|e| e.to_string())?;
    let size = w.outer_size().map_err(|e| e.to_string())?;
    Ok(OverlayGeometry { x: pos.x, y: pos.y, width: size.width, height: size.height })
}

#[tauri::command]
fn list_monitors(app: AppHandle) -> Result<Vec<MonitorInfo>, String> {
    let primary = app.primary_monitor().map_err(|e| e.to_string())?.map(|m| *m.position());
    Ok(app
        .available_monitors()
        .map_err(|e| e.to_string())?
        .into_iter()
        .enumerate()
        .map(|(i, m)| MonitorInfo {
            name: m.name().cloned().unwrap_or_else(|| format!("Display {}", i + 1)),
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
            scale: m.scale_factor(),
            primary: Some(*m.position()) == primary,
        })
        .collect())
}

#[tauri::command]
fn get_overlay_geometry(app: AppHandle) -> Result<OverlayGeometry, String> {
    overlay_geometry(&app)
}

#[tauri::command]
fn set_overlay_position(app: AppHandle, x: i32, y: i32) -> Result<(), String> {
    let w = app.get_webview_window(OVERLAY).ok_or("overlay window missing")?;
    w.set_position(tauri::PhysicalPosition::new(x, y)).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::POSITION | tauri_plugin_window_state::StateFlags::SIZE,
                )
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        toggle_overlay(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build())?;
            }
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let settings: Settings = std::fs::read_to_string(&settings_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            let mut meter = Meter::new();
            apply_meter_settings(&mut meter, &settings);
            let state = AppState {
                npcap: Npcap::load(),
                meter: Arc::new(Mutex::new(meter)),
                status: Arc::new(Mutex::new(CaptureStatus::default())),
                last_traffic: Arc::new(Mutex::new(None)),
                capture: Mutex::new(None),
                settings: Mutex::new(settings.clone()),
                settings_path,
                default_history_dir: app.path().app_data_dir()?.join("fights"),
            };
            if let Err(e) = &state.npcap {
                state.status.lock().unwrap().error = Some(e.clone());
            } else if settings.auto_start {
                let _ = start_capture_inner(&state);
            }
            app.manage(state);
            apply_overlay_settings(app.handle(), &settings);
            if let Err(e) = register_hotkey(app.handle(), settings.overlay_hotkey.as_deref()) {
                log::warn!("overlay hotkey: {e}");
            }

            // Push the live numbers to both windows 5 times a second.
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_millis(200));
                let state = handle.state::<AppState>();
                save_finished(&handle, &state, now());
                let update = build_update(&state);
                let _ = handle.emit("update", &update);
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            // Keep the Layout page in sync when the overlay moves or resizes.
            if window.label() == OVERLAY && matches!(event, WindowEvent::Moved(_) | WindowEvent::Resized(_)) {
                if let Ok(g) = overlay_geometry(window.app_handle()) {
                    let _ = window.app_handle().emit("overlay-geometry", g);
                }
            }
            // Closing the main window quits the app (and the overlay with it).
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { .. } = event {
                    let state = window.app_handle().state::<AppState>();
                    stop_capture_inner(&state);
                    // Save the fight that's still running.
                    state.meter.lock().unwrap().end_fight();
                    save_finished(window.app_handle(), &state, now());
                    window.app_handle().exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_interfaces,
            start_capture,
            stop_capture,
            get_settings,
            set_settings,
            reset_meter,
            clear_history,
            get_history_dir,
            list_fights,
            delete_fight,
            open_history_dir,
            get_update,
            list_monitors,
            get_overlay_geometry,
            set_overlay_position
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
