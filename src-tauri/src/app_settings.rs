use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewWindow};

const DEFAULT_SOURCE_FILE: &str = "agent-status.md";
const SETTINGS_DIRECTORY: &str = ".config/ctx-ppteer";
const SETTINGS_FILE: &str = "settings.json";

#[derive(Clone, Deserialize, Serialize)]
pub struct WindowState {
    monitor_name: Option<String>,
    relative_x: f64,
    relative_y: f64,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub source_path: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_text_scale")]
    pub text_scale: f64,
    #[serde(default = "default_pinned")]
    pub pinned: bool,
    #[serde(default)]
    pub window: Option<WindowState>,
}

fn default_theme() -> String {
    "system".into()
}
fn default_text_scale() -> f64 {
    1.0
}
fn default_pinned() -> bool {
    true
}

pub fn default_source_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Unable to find the application data directory: {error}"))?;
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Unable to create the application data directory: {error}"))?;
    Ok(directory.join(DEFAULT_SOURCE_FILE))
}

pub fn ensure_default_source(app: &AppHandle) -> Result<PathBuf, String> {
    let path = default_source_path(app)?;
    if !path.exists() {
        fs::write(&path, "")
            .map_err(|error| format!("Unable to create {}: {error}", path.display()))?;
    }
    Ok(path)
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| format!("Unable to find the home directory: {error}"))?;
    Ok(home.join(SETTINGS_DIRECTORY).join(SETTINGS_FILE))
}

fn default_settings(app: &AppHandle) -> Result<Settings, String> {
    Ok(Settings {
        source_path: ensure_default_source(app)?.display().to_string(),
        theme: default_theme(),
        text_scale: default_text_scale(),
        pinned: default_pinned(),
        window: None,
    })
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = settings_path(app)?;
    let directory = path.parent().ok_or("Settings directory unavailable")?;
    fs::create_dir_all(directory)
        .map_err(|error| format!("Unable to create {}: {error}", directory.display()))?;
    let contents = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("Unable to encode settings: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, contents)
        .map_err(|error| format!("Unable to write settings: {error}"))?;
    fs::rename(&temporary, &path).map_err(|error| format!("Unable to save settings: {error}"))
}

pub fn load(app: &AppHandle) -> Result<Settings, String> {
    let path = settings_path(app)?;
    match fs::read(&path) {
        Ok(contents) => serde_json::from_slice(&contents)
            .map_err(|error| format!("Unable to read {}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let settings = default_settings(app)?;
            save(app, &settings)?;
            Ok(settings)
        }
        Err(error) => Err(format!("Unable to read {}: {error}", path.display())),
    }
}

pub fn save_window_state(window: &WebviewWindow) {
    let (Ok(position), Ok(monitors)) = (window.outer_position(), window.available_monitors())
    else {
        return;
    };
    let Some(monitor) = monitors.iter().find(|monitor| {
        let area = monitor.work_area();
        position.x >= area.position.x
            && position.x < area.position.x + area.size.width as i32
            && position.y >= area.position.y
            && position.y < area.position.y + area.size.height as i32
    }) else {
        return;
    };
    let area = monitor.work_area();
    let window_state = WindowState {
        monitor_name: monitor.name().cloned(),
        relative_x: (position.x - area.position.x) as f64 / area.size.width.max(1) as f64,
        relative_y: (position.y - area.position.y) as f64 / area.size.height.max(1) as f64,
    };
    let Ok(mut settings) = load(&window.app_handle()) else {
        return;
    };
    settings.window = Some(window_state);
    let _ = save(&window.app_handle(), &settings);
}

pub fn restore_window_state(window: &WebviewWindow) {
    let Ok(settings) = load(&window.app_handle()) else {
        return;
    };
    let Some(state) = settings.window else { return };
    let Ok(monitors) = window.available_monitors() else {
        return;
    };
    let primary = window.primary_monitor().ok().flatten();
    let Some(monitor) = monitors
        .iter()
        .find(|monitor| monitor.name() == state.monitor_name.as_ref())
        .or(primary.as_ref())
    else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let area = monitor.work_area();
    let max_x = area.position.x + area.size.width.saturating_sub(size.width) as i32;
    let max_y = area.position.y + area.size.height.saturating_sub(size.height) as i32;
    let x = ((area.position.x as f64 + state.relative_x * area.size.width as f64).round() as i32)
        .clamp(area.position.x, max_x.max(area.position.x));
    let y = ((area.position.y as f64 + state.relative_y * area.size.height as f64).round() as i32)
        .clamp(area.position.y, max_y.max(area.position.y));
    let _ = window.set_position(PhysicalPosition::new(x, y));
}
