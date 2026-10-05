use chrono::{DateTime, Local};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use tauri::{
    menu::{Menu, SubmenuBuilder},
    path::BaseDirectory,
    AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewWindow, WindowEvent,
};
use tauri_plugin_dialog::DialogExt;
use toml_edit::{value, Array, DocumentMut};

pub mod mcp;
pub mod settings;
pub mod status_file;

const MAX_BYTES: u64 = 1024 * 1024;
const DEFAULT_SOURCE_FILE: &str = "agent-status.md";
const SKILL_NAME: &str = "ctx-ppteer";
const SETTINGS_DIRECTORY: &str = ".config/ctx-ppteer";
const SETTINGS_FILE: &str = "settings.json";

#[derive(Clone, Deserialize, Serialize)]
struct WindowState {
    monitor_name: Option<String>,
    relative_x: f64,
    relative_y: f64,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    source_path: String,
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_text_scale")]
    text_scale: f64,
    #[serde(default)]
    window: Option<WindowState>,
}

fn default_theme() -> String {
    "system".into()
}

fn default_text_scale() -> f64 {
    1.0
}

#[derive(Clone, Serialize)]
struct Snapshot {
    path: Option<String>,
    markdown: String,
    revision: u64,
    health: String,
    error: Option<String>,
    modified_at: Option<String>,
}
struct ViewerState {
    path: Mutex<Option<PathBuf>>,
    snapshot: Mutex<Snapshot>,
}

#[derive(Serialize)]
struct SkillInstallation {
    installed: bool,
    mcp_registered: bool,
    path: String,
    agents: Vec<AgentInstallation>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentInstallation {
    name: &'static str,
    skill_installed: bool,
    mcp_registered: bool,
    available: bool,
    error: Option<String>,
}

fn read_snapshot(path: &Path, revision: u64, previous: &Snapshot) -> Snapshot {
    let metadata = fs::metadata(path);
    let modified_at = metadata
        .as_ref()
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            let dt: DateTime<Local> = t.into();
            dt.to_rfc3339()
        });
    let base = || Snapshot {
        path: Some(path.display().to_string()),
        markdown: previous.markdown.clone(),
        revision,
        health: "stale".into(),
        error: None,
        modified_at: modified_at.clone(),
    };
    match metadata {
        Ok(meta) if !meta.is_file() => Snapshot {
            error: Some("The selected path is not a regular file".into()),
            health: "error".into(),
            ..base()
        },
        Ok(meta) if meta.len() > MAX_BYTES => Snapshot {
            error: Some("The file is larger than the 1 MiB limit".into()),
            health: "error".into(),
            ..base()
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Snapshot {
            health: "waiting".into(),
            error: Some("Waiting for the file to appear".into()),
            modified_at: None,
            ..base()
        },
        Err(e) => Snapshot {
            error: Some(format!("Unable to read file: {e}")),
            ..base()
        },
        Ok(_) => match fs::read(path)
            .and_then(|bytes| String::from_utf8(bytes).map_err(std::io::Error::other))
        {
            Ok(markdown) => Snapshot {
                markdown,
                health: "current".into(),
                error: None,
                ..base()
            },
            Err(e) => Snapshot {
                error: Some(format!("Unable to decode UTF-8: {e}")),
                ..base()
            },
        },
    }
}

fn update(app: &AppHandle, state: &ViewerState) {
    let Some(path) = state.path.lock().ok().and_then(|p| p.clone()) else {
        return;
    };
    let snapshot = {
        let previous = state.snapshot.lock().unwrap();
        read_snapshot(&path, previous.revision + 1, &previous)
    };
    *state.snapshot.lock().unwrap() = snapshot.clone();
    let _ = app.emit("source-update", snapshot);
}

fn default_source_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Unable to find the application data directory: {e}"))?;
    fs::create_dir_all(&directory)
        .map_err(|e| format!("Unable to create the application data directory: {e}"))?;
    Ok(directory.join(DEFAULT_SOURCE_FILE))
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|e| format!("Unable to find the home directory: {e}"))?;
    Ok(home.join(SETTINGS_DIRECTORY).join(SETTINGS_FILE))
}

fn default_settings(app: &AppHandle) -> Result<Settings, String> {
    Ok(Settings {
        source_path: default_source_path(app)?.display().to_string(),
        theme: default_theme(),
        text_scale: default_text_scale(),
        window: None,
    })
}

fn save_settings(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = settings_path(app)?;
    let directory = path.parent().ok_or("Settings directory unavailable")?;
    fs::create_dir_all(directory)
        .map_err(|e| format!("Unable to create {}: {e}", directory.display()))?;
    let contents = serde_json::to_vec_pretty(settings)
        .map_err(|e| format!("Unable to encode settings: {e}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, contents).map_err(|e| format!("Unable to write settings: {e}"))?;
    fs::rename(&temporary, &path).map_err(|e| format!("Unable to save settings: {e}"))
}

fn load_settings(app: &AppHandle) -> Result<Settings, String> {
    let path = settings_path(app)?;
    match fs::read(&path) {
        Ok(contents) => serde_json::from_slice(&contents)
            .map_err(|e| format!("Unable to read {}: {e}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let settings = default_settings(app)?;
            save_settings(app, &settings)?;
            Ok(settings)
        }
        Err(error) => Err(format!("Unable to read {}: {error}", path.display())),
    }
}

fn save_window_state(window: &WebviewWindow) {
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
    let Ok(mut settings) = load_settings(&window.app_handle()) else {
        return;
    };
    settings.window = Some(window_state);
    let _ = save_settings(&window.app_handle(), &settings);
}

fn restore_window_state(window: &WebviewWindow) {
    let Ok(settings) = load_settings(&window.app_handle()) else {
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

fn skill_install_path(app: &AppHandle) -> Result<PathBuf, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|e| format!("Unable to find the home directory: {e}"))?;
    Ok(home.join(".agents").join("skills").join(SKILL_NAME))
}

fn home_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .home_dir()
        .map_err(|e| format!("Unable to find the home directory: {e}"))
}

fn codex_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let home = home_dir(app)?;
    Ok(home.join(".codex").join("config.toml"))
}

fn codex_mcp_registered(app: &AppHandle) -> Result<bool, String> {
    let path = codex_config_path(app)?;
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("Unable to read {}: {error}", path.display())),
    };
    let document = DocumentMut::from_str(&contents)
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    Ok(document["mcp_servers"][SKILL_NAME].is_table())
}

fn register_codex_mcp(app: &AppHandle, executable: &Path) -> Result<(), String> {
    let path = codex_config_path(app)?;
    let directory = path
        .parent()
        .ok_or("Codex configuration directory unavailable")?;
    fs::create_dir_all(directory)
        .map_err(|error| format!("Unable to create {}: {error}", directory.display()))?;
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("Unable to read {}: {error}", path.display())),
    };
    let mut document = DocumentMut::from_str(&contents)
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    document["mcp_servers"][SKILL_NAME]["command"] =
        value(executable.to_string_lossy().to_string());
    document["mcp_servers"][SKILL_NAME]["args"] = value(Array::from_iter(["--mcp"]));
    let temporary = path.with_extension("toml.tmp");
    fs::write(&temporary, document.to_string())
        .map_err(|error| format!("Unable to write {}: {error}", path.display()))?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("Unable to save {}: {error}", path.display()))
}

fn read_json_config(path: &Path) -> Result<Value, String> {
    match fs::read(path) {
        Ok(contents) => serde_json::from_slice(&contents)
            .map_err(|error| format!("Unable to read {}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(error) => Err(format!("Unable to read {}: {error}", path.display())),
    }
}

fn object_mut<'a>(value: &'a mut Value, label: &str) -> Result<&'a mut Map<String, Value>, String> {
    value
        .as_object_mut()
        .ok_or_else(|| format!("{label} must contain a JSON object"))
}

fn nested_object_mut<'a>(
    root: &'a mut Value,
    keys: &[&str],
    label: &str,
) -> Result<&'a mut Map<String, Value>, String> {
    let mut current = object_mut(root, label)?;
    for key in keys {
        let entry = current.entry(*key).or_insert_with(|| json!({}));
        current = object_mut(entry, label)?;
    }
    Ok(current)
}

fn save_json_config(path: &Path, value: &Value) -> Result<(), String> {
    let directory = path.parent().ok_or("Configuration directory unavailable")?;
    fs::create_dir_all(directory)
        .map_err(|error| format!("Unable to create {}: {error}", directory.display()))?;
    let temporary = path.with_extension("json.tmp");
    let contents = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("Unable to encode {}: {error}", path.display()))?;
    fs::write(&temporary, contents)
        .map_err(|error| format!("Unable to write {}: {error}", path.display()))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("Unable to save {}: {error}", path.display()))
}

fn register_standard_json_mcp(path: &Path, executable: &Path) -> Result<(), String> {
    let mut config = read_json_config(path)?;
    let servers = nested_object_mut(&mut config, &["mcpServers"], &path.display().to_string())?;
    servers.insert(
        SKILL_NAME.into(),
        json!({ "command": executable, "args": ["--mcp"] }),
    );
    save_json_config(path, &config)
}

fn register_opencode_mcp(path: &Path, executable: &Path) -> Result<(), String> {
    let mut config = read_json_config(path)?;
    let servers = nested_object_mut(
        &mut config,
        &["mcp", "servers"],
        &path.display().to_string(),
    )?;
    servers.insert(
        SKILL_NAME.into(),
        json!({ "type": "local", "command": [executable, "--mcp"], "codemode": false }),
    );
    save_json_config(path, &config)
}

fn json_mcp_registered(path: &Path, keys: &[&str]) -> Result<bool, String> {
    let config = match fs::read(path) {
        Ok(contents) => serde_json::from_slice::<Value>(&contents)
            .map_err(|error| format!("Unable to read {}: {error}", path.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("Unable to read {}: {error}", path.display())),
    };
    let mut value = &config;
    for key in keys {
        let Some(next) = value.get(key) else {
            return Ok(false);
        };
        value = next;
    }
    Ok(value.get(SKILL_NAME).is_some())
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|e| format!("Unable to create {}: {e}", destination.display()))?;
    for entry in
        fs::read_dir(source).map_err(|e| format!("Unable to read {}: {e}", source.display()))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_directory(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path)
                .map_err(|e| format!("Unable to copy {}: {e}", source_path.display()))?;
        }
    }
    Ok(())
}

fn install_skill_at(source: &Path, destination: &Path) -> Result<bool, String> {
    if destination.join("SKILL.md").is_file() {
        return Ok(true);
    }
    if destination.exists() {
        return Err(format!(
            "{} already exists. Remove or rename it before installing the skill.",
            destination.display()
        ));
    }
    copy_directory(source, destination)?;
    Ok(true)
}

fn integration_paths(app: &AppHandle) -> Result<Vec<(&'static str, PathBuf)>, String> {
    let home = home_dir(app)?;
    Ok(vec![
        ("Agents", home.join(".agents/skills").join(SKILL_NAME)),
        ("Codex", home.join(".codex/skills").join(SKILL_NAME)),
        ("Claude Code", home.join(".claude/skills").join(SKILL_NAME)),
        (
            "Antigravity",
            home.join(".gemini/antigravity-cli/skills").join(SKILL_NAME),
        ),
        (
            "OpenCode",
            home.join(".config/opencode/skills").join(SKILL_NAME),
        ),
    ])
}

fn install_claude_mcp(executable: &Path) -> Result<bool, String> {
    let output = match Command::new("claude")
        .args(["mcp", "add", SKILL_NAME, "--scope", "user", "--"])
        .arg(executable)
        .arg("--mcp")
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("Unable to run Claude Code: {error}")),
    };
    if output.status.success() {
        Ok(true)
    } else {
        Err(format!(
            "Claude Code rejected the MCP configuration: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn bundled_skill_source(app: &AppHandle) -> Result<PathBuf, String> {
    let source = app
        .path()
        .resolve(format!("skills/{SKILL_NAME}"), BaseDirectory::Resource)
        .map_err(|e| format!("Unable to find the bundled skill: {e}"))?;
    if !source.join("SKILL.md").is_file() {
        return Err("The bundled skill is missing SKILL.md".into());
    }
    Ok(source)
}

fn integration_status(app: &AppHandle) -> Result<SkillInstallation, String> {
    let home = home_dir(app)?;
    let destination = skill_install_path(app)?;
    let cursor_config = home.join(".cursor/mcp.json");
    let agy_config = home.join(".gemini/config/mcp_config.json");
    let opencode_config = home.join(".config/opencode/opencode.json");
    let codex_registered = codex_mcp_registered(app)?;
    let cursor_registered = json_mcp_registered(&cursor_config, &["mcpServers"])?;
    let agy_registered = json_mcp_registered(&agy_config, &["mcpServers"])?;
    let opencode_registered = json_mcp_registered(&opencode_config, &["mcp", "servers"])?;
    let agents = integration_paths(app)?
        .into_iter()
        .map(|(name, path)| AgentInstallation {
            name,
            skill_installed: path.join("SKILL.md").is_file(),
            mcp_registered: match name {
                "Codex" => codex_registered,
                "Antigravity" => agy_registered,
                "OpenCode" => opencode_registered,
                _ => false,
            },
            available: true,
            error: None,
        })
        .chain(std::iter::once(AgentInstallation {
            name: "Cursor",
            skill_installed: false,
            mcp_registered: cursor_registered,
            available: true,
            error: None,
        }))
        .collect();
    Ok(SkillInstallation {
        installed: destination.join("SKILL.md").is_file(),
        mcp_registered: codex_registered
            && cursor_registered
            && agy_registered
            && opencode_registered,
        path: destination.display().to_string(),
        agents,
    })
}

fn start_watcher(app: AppHandle, state: Arc<ViewerState>) {
    thread::spawn(move || {
        let (tx, rx) = mpsc::channel();
        let Ok(mut watcher): Result<RecommendedWatcher, _> = notify::recommended_watcher(tx) else {
            return;
        };
        let mut watched_parent: Option<PathBuf> = None;
        let mut last_reconcile = Instant::now();
        loop {
            let path = state.path.lock().ok().and_then(|p| p.clone());
            if let Some(path) = path {
                let parent = path.parent().unwrap_or(Path::new(".")).to_path_buf();
                if watched_parent.as_ref() != Some(&parent) {
                    if let Some(old) = watched_parent.take() {
                        let _ = watcher.unwatch(&old);
                    }
                    if watcher.watch(&parent, RecursiveMode::NonRecursive).is_ok() {
                        watched_parent = Some(parent);
                    }
                }
                if rx.recv_timeout(Duration::from_millis(250)).is_ok() {
                    thread::sleep(Duration::from_millis(120));
                    update(&app, &state);
                }
                if last_reconcile.elapsed() >= Duration::from_secs(30) {
                    update(&app, &state);
                    last_reconcile = Instant::now();
                }
            } else {
                thread::sleep(Duration::from_millis(500));
            }
        }
    });
}

#[tauri::command]
fn get_snapshot(state: State<'_, Arc<ViewerState>>) -> Snapshot {
    state.snapshot.lock().unwrap().clone()
}
#[tauri::command]
fn set_source(
    path: String,
    app: AppHandle,
    state: State<'_, Arc<ViewerState>>,
) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("A source path is required".into());
    }
    let path = PathBuf::from(path);
    let mut settings = load_settings(&app)?;
    settings.source_path = path.display().to_string();
    save_settings(&app, &settings)?;
    state
        .path
        .lock()
        .map_err(|_| "State unavailable")?
        .replace(path);
    update(&app, &state);
    Ok(())
}
#[tauri::command]
fn use_default_source(app: AppHandle, state: State<'_, Arc<ViewerState>>) -> Result<(), String> {
    let path = default_source_path(&app)?;
    let mut settings = load_settings(&app)?;
    settings.source_path = path.display().to_string();
    save_settings(&app, &settings)?;
    state
        .path
        .lock()
        .map_err(|_| "State unavailable")?
        .replace(path);
    update(&app, &state);
    Ok(())
}
#[tauri::command]
fn get_default_source(app: AppHandle) -> Result<String, String> {
    Ok(default_source_path(&app)?.display().to_string())
}
#[tauri::command]
fn get_settings(app: AppHandle) -> Result<Settings, String> {
    load_settings(&app)
}
#[tauri::command]
fn set_appearance_settings(theme: String, text_scale: f64, app: AppHandle) -> Result<(), String> {
    if !matches!(theme.as_str(), "system" | "light" | "dark") {
        return Err("Theme must be system, light, or dark".into());
    }
    if !(0.85..=1.35).contains(&text_scale) {
        return Err("Text scale must be between 0.85 and 1.35".into());
    }
    let mut settings = load_settings(&app)?;
    settings.theme = theme;
    settings.text_scale = text_scale;
    save_settings(&app, &settings)
}
#[tauri::command]
fn get_skill_installation(app: AppHandle) -> Result<SkillInstallation, String> {
    integration_status(&app)
}
#[tauri::command]
fn install_skill(app: AppHandle) -> Result<SkillInstallation, String> {
    let source = bundled_skill_source(&app)?;
    let executable = std::env::current_exe()
        .map_err(|error| format!("Unable to locate the ctx-ppteer executable: {error}"))?;
    for (_, destination) in integration_paths(&app)? {
        install_skill_at(&source, &destination)?;
    }

    register_codex_mcp(&app, &executable)?;
    let home = home_dir(&app)?;
    register_standard_json_mcp(&home.join(".cursor/mcp.json"), &executable)?;
    register_standard_json_mcp(&home.join(".gemini/config/mcp_config.json"), &executable)?;
    register_opencode_mcp(&home.join(".config/opencode/opencode.json"), &executable)?;

    let claude_result = install_claude_mcp(&executable);
    let mut result = integration_status(&app)?;
    if let Some(claude) = result
        .agents
        .iter_mut()
        .find(|agent| agent.name == "Claude Code")
    {
        match claude_result {
            Ok(true) => claude.mcp_registered = true,
            Ok(false) => {
                claude.available = false;
                claude.error =
                    Some("Claude Code is not installed; its MCP tool was not registered.".into());
            }
            Err(error) => claude.error = Some(error),
        }
    }
    Ok(result)
}
#[tauri::command]
fn set_pinned(pinned: bool, app: AppHandle) -> Result<(), String> {
    app.get_webview_window("main")
        .ok_or("Main window unavailable")?
        .set_always_on_top(pinned)
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn pick_source(app: AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .add_filter("Markdown", &["md", "markdown", "txt"])
        .blocking_pick_file()
        .map(|p| p.to_string())
}

fn install_menu(app: &tauri::App) -> tauri::Result<()> {
    let handle = app.handle();
    let source = SubmenuBuilder::new(handle, "Source")
        .text("choose-source", "Choose Markdown File…")
        .text("use-default-source", "Use Default File")
        .build()?;
    let appearance = SubmenuBuilder::new(handle, "Appearance")
        .text("theme-system", "Use System Theme")
        .text("theme-light", "Use Light Theme")
        .text("theme-dark", "Use Dark Theme")
        .separator()
        .text("text-smaller", "Smaller Text")
        .text("text-larger", "Larger Text")
        .text("text-reset", "Reset Text Size")
        .build()?;
    let viewer = SubmenuBuilder::new(handle, "Viewer")
        .text("toggle-pinned", "Toggle Always on Top")
        .build()?;
    let menu = Menu::default(handle)?;
    menu.append(&source)?;
    menu.append(&appearance)?;
    menu.append(&viewer)?;
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| {
        let id = event.id().as_ref();
        if matches!(
            id,
            "choose-source"
                | "use-default-source"
                | "theme-system"
                | "theme-light"
                | "theme-dark"
                | "text-smaller"
                | "text-larger"
                | "text-reset"
                | "toggle-pinned"
        ) {
            let _ = app.emit("viewer-menu-action", id);
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_standard_json_mcp_without_losing_existing_servers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp.json");
        fs::write(
            &path,
            r#"{"mcpServers":{"existing":{"command":"existing"}}}"#,
        )
        .unwrap();

        register_standard_json_mcp(&path, Path::new("/Applications/ctx-ppteer")).unwrap();

        let config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(config["mcpServers"]["existing"].is_object());
        assert_eq!(config["mcpServers"][SKILL_NAME]["args"][0], "--mcp");
    }

    #[test]
    fn registers_opencode_mcp_without_losing_existing_servers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("opencode.json");
        fs::write(
            &path,
            r#"{"mcp":{"servers":{"existing":{"type":"local"}}}}"#,
        )
        .unwrap();

        register_opencode_mcp(&path, Path::new("/Applications/ctx-ppteer")).unwrap();

        let config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(config["mcp"]["servers"]["existing"].is_object());
        assert_eq!(config["mcp"]["servers"][SKILL_NAME]["type"], "local");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = Arc::new(ViewerState {
        path: Mutex::new(None),
        snapshot: Mutex::new(Snapshot {
            path: None,
            markdown: String::new(),
            revision: 0,
            health: "waiting".into(),
            error: None,
            modified_at: None,
        }),
    });
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            set_source,
            use_default_source,
            get_default_source,
            get_settings,
            set_appearance_settings,
            get_skill_installation,
            install_skill,
            set_pinned,
            pick_source
        ])
        .setup(|app| {
            install_menu(app)?;
            if let Some(window) = app.get_webview_window("main") {
                restore_window_state(&window);
                let state_window = window.clone();
                window.on_window_event(move |event| {
                    if matches!(event, WindowEvent::Moved(_) | WindowEvent::Resized(_)) {
                        save_window_state(&state_window);
                    }
                });
            }
            let state = app.state::<Arc<ViewerState>>().inner().clone();
            let settings = load_settings(app.handle()).map_err(std::io::Error::other)?;
            if settings.source_path.trim().is_empty() {
                return Err(std::io::Error::other("Settings contain no sourcePath").into());
            }
            let path = PathBuf::from(settings.source_path);
            state
                .path
                .lock()
                .map_err(|_| std::io::Error::other("State unavailable"))?
                .replace(path);
            update(app.handle(), &state);
            start_watcher(app.handle().clone(), state);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
