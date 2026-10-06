use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

const DEFAULT_SOURCE_FILE: &str = "agent-status.md";
const SETTINGS_DIRECTORY: &str = ".config/ctx-ppteer";
const SETTINGS_FILE: &str = "settings.json";
pub const PROJECT_DIRECTORY_ASC: &str = "project-directory-asc";

#[derive(Clone, Deserialize, Serialize)]
pub struct WindowState {
    monitor_name: Option<String>,
    relative_x: f64,
    relative_y: f64,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    maximized: bool,
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
    #[serde(default = "default_double_click_to_edit")]
    pub double_click_to_edit: bool,
    #[serde(default = "default_project_sort_order")]
    pub project_sort_order: String,
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
fn default_double_click_to_edit() -> bool {
    true
}
fn default_project_sort_order() -> String {
    PROJECT_DIRECTORY_ASC.into()
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
        double_click_to_edit: default_double_click_to_edit(),
        project_sort_order: default_project_sort_order(),
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
    let Ok(maximized) = window.is_maximized() else {
        return;
    };
    let Ok(mut settings) = load(&window.app_handle()) else {
        return;
    };

    // A maximized window's outer bounds describe the monitor, not the user's last
    // normal window bounds. Preserve those bounds and only update the mode.
    if maximized {
        if let Some(state) = settings.window.as_mut() {
            state.maximized = true;
            let _ = save(&window.app_handle(), &settings);
        }
        return;
    }

    let (Ok(position), Ok(size), Ok(monitors)) = (
        window.outer_position(),
        window.outer_size(),
        window.available_monitors(),
    ) else {
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
        width: Some(size.width),
        height: Some(size.height),
        maximized: false,
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
    let area = monitor.work_area();
    if let (Some(width), Some(height)) = (state.width, state.height) {
        let width = width.min(area.size.width).max(1);
        let height = height.min(area.size.height).max(1);
        let _ = window.set_size(PhysicalSize::new(width, height));
    }
    let Ok(size) = window.outer_size() else {
        return;
    };
    let max_x = area.position.x + area.size.width.saturating_sub(size.width) as i32;
    let max_y = area.position.y + area.size.height.saturating_sub(size.height) as i32;
    let x = ((area.position.x as f64 + state.relative_x * area.size.width as f64).round() as i32)
        .clamp(area.position.x, max_x.max(area.position.x));
    let y = ((area.position.y as f64 + state.relative_y * area.size.height as f64).round() as i32)
        .clamp(area.position.y, max_y.max(area.position.y));
    let _ = window.set_position(PhysicalPosition::new(x, y));
    if state.maximized {
        let _ = window.maximize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_window_state_saved_before_size_and_maximize_tracking() {
        let settings: Settings = serde_json::from_str(
            r#"{"sourcePath":"/tmp/status.md","window":{"monitor_name":"Display","relative_x":0.25,"relative_y":0.5}}"#,
        )
        .unwrap();

        let state = settings.window.unwrap();
        assert_eq!(state.width, None);
        assert_eq!(state.height, None);
        assert!(!state.maximized);
    }

    #[test]
    fn saves_complete_window_bounds() {
        let state = WindowState {
            monitor_name: Some("Display".into()),
            relative_x: 0.25,
            relative_y: 0.5,
            width: Some(800),
            height: Some(600),
            maximized: true,
        };

        let value = serde_json::to_value(state).unwrap();
        assert_eq!(value["width"], 800);
        assert_eq!(value["height"], 600);
        assert_eq!(value["maximized"], true);
    }

    #[test]
    fn defaults_project_sort_order_for_existing_settings() {
        let settings: Settings =
            serde_json::from_str(r#"{"sourcePath":"/tmp/status.md"}"#).unwrap();

        assert_eq!(settings.project_sort_order, PROJECT_DIRECTORY_ASC);
    }
}
