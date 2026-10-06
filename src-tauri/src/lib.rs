use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    sync::Arc,
};
use tauri::{
    menu::{Menu, MenuItem},
    path::BaseDirectory,
    AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_dialog::DialogExt;
use toml_edit::{value, Array, DocumentMut};

mod app_settings;
mod app_state;
pub mod mcp;
pub mod settings;
pub mod status_file;

use app_settings::{
    default_source_path, ensure_default_source, load as load_settings, restore_window_state,
    save as save_settings, save_window_state, Settings,
};
use app_state::{new_viewer_state, start_watcher, update, Snapshot, ViewerState};

const SKILL_NAME: &str = "ctx-ppteer";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillInstallation {
    installed: bool,
    mcp_registered: bool,
    path: String,
    targets: Vec<SkillTarget>,
    additional_targets: Vec<SkillTarget>,
    agents: Vec<AgentInstallation>,
    additional_agents: Vec<AgentInstallation>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillTarget {
    id: String,
    name: String,
    path: String,
    installed: bool,
    up_to_date: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentInstallation {
    id: String,
    name: String,
    skill_installed: bool,
    mcp_registered: bool,
    available: bool,
    config_path: String,
    error: Option<String>,
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

fn codex_mcp_registered_at(path: &Path) -> Result<bool, String> {
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("Unable to read {}: {error}", path.display())),
    };
    let document = DocumentMut::from_str(&contents)
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    Ok(document_has_codex_mcp(&document))
}

fn codex_mcp_registered(app: &AppHandle) -> Result<bool, String> {
    codex_mcp_registered_at(&codex_config_path(app)?)
}

fn document_has_codex_mcp(document: &DocumentMut) -> bool {
    document
        .as_table()
        .get("mcp_servers")
        .is_some_and(|servers| {
            servers
                .as_table()
                .and_then(|servers| servers.get(SKILL_NAME))
                .is_some_and(|server| server.is_table() || server.is_inline_table())
                || servers
                    .as_inline_table()
                    .and_then(|servers| servers.get(SKILL_NAME))
                    .is_some_and(|server| server.is_inline_table())
        })
}

fn register_codex_mcp_at(path: &Path, executable: &Path) -> Result<(), String> {
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
    save_toml_config(path, &document)
}

fn save_toml_config(path: &Path, document: &DocumentMut) -> Result<(), String> {
    let temporary = path.with_extension("toml.tmp");
    fs::write(&temporary, document.to_string())
        .map_err(|error| format!("Unable to write {}: {error}", path.display()))?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("Unable to save {}: {error}", path.display()))
}

fn register_codex_mcp(app: &AppHandle, executable: &Path) -> Result<(), String> {
    register_codex_mcp_at(&codex_config_path(app)?, executable)
}

fn unregister_codex_mcp_at(path: &Path) -> Result<(), String> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("Unable to read {}: {error}", path.display())),
    };
    let mut document = DocumentMut::from_str(&contents)
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    let removed = document
        .as_table_mut()
        .get_mut("mcp_servers")
        .is_some_and(|servers| {
            servers
                .as_table_mut()
                .is_some_and(|servers| servers.remove(SKILL_NAME).is_some())
                || servers
                    .as_inline_table_mut()
                    .is_some_and(|servers| servers.remove(SKILL_NAME).is_some())
        });
    if removed {
        save_toml_config(path, &document)?;
    }
    Ok(())
}

fn unregister_codex_mcp(app: &AppHandle) -> Result<(), String> {
    unregister_codex_mcp_at(&codex_config_path(app)?)
}

fn read_json_config(path: &Path) -> Result<Value, String> {
    match fs::read(path) {
        Ok(contents) if contents.iter().all(u8::is_ascii_whitespace) => Ok(json!({})),
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

fn existing_nested_object_mut<'a>(
    value: &'a mut Value,
    keys: &[&str],
) -> Option<&'a mut Map<String, Value>> {
    let (key, remaining) = keys.split_first()?;
    let object = value.as_object_mut()?;
    let next = object.get_mut(*key)?;
    if remaining.is_empty() {
        next.as_object_mut()
    } else {
        existing_nested_object_mut(next, remaining)
    }
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

fn unregister_json_mcp(path: &Path, keys: &[&str]) -> Result<(), String> {
    let mut config = read_json_config(path)?;
    let removed = existing_nested_object_mut(&mut config, keys)
        .is_some_and(|servers| servers.remove(SKILL_NAME).is_some());
    if removed {
        save_json_config(path, &config)?;
    }
    Ok(())
}

fn json_mcp_registered(path: &Path, keys: &[&str]) -> Result<bool, String> {
    let config = read_json_config(path)?;
    let mut value = &config;
    for key in keys {
        let Some(next) = value.get(key) else {
            return Ok(false);
        };
        value = next;
    }
    Ok(value.get(SKILL_NAME).is_some())
}

fn command_exists(command: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|directory| {
        let path = directory.join(command);
        path.is_file() || (cfg!(windows) && directory.join(format!("{command}.exe")).is_file())
    })
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

fn skill_hash(path: &Path) -> Result<String, String> {
    fn hash_directory(root: &Path, directory: &Path, hasher: &mut Sha256) -> Result<(), String> {
        let mut entries = fs::read_dir(directory)
            .map_err(|error| format!("Unable to read {}: {error}", directory.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Unable to read {}: {error}", directory.display()))?;
        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|error| format!("Unable to hash {}: {error}", path.display()))?;
            let relative = relative.to_string_lossy();
            let file_type = entry
                .file_type()
                .map_err(|error| format!("Unable to inspect {}: {error}", path.display()))?;
            if file_type.is_dir() {
                hasher.update(b"directory\0");
                hasher.update(relative.as_bytes());
                hasher.update(b"\0");
                hash_directory(root, &path, hasher)?;
            } else if file_type.is_file() {
                hasher.update(b"file\0");
                hasher.update(relative.as_bytes());
                hasher.update(b"\0");
                hasher.update(
                    fs::read(&path)
                        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?,
                );
            }
        }
        Ok(())
    }

    let mut hasher = Sha256::new();
    hash_directory(path, path, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn install_skill_at(source: &Path, destination: &Path) -> Result<bool, String> {
    if destination.join("SKILL.md").is_file() {
        fs::remove_dir_all(destination)
            .map_err(|error| format!("Unable to replace {}: {error}", destination.display()))?;
        copy_directory(source, destination)?;
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

fn skill_targets(app: &AppHandle) -> Result<Vec<(&'static str, &'static str, PathBuf)>, String> {
    let home = home_dir(app)?;
    Ok(vec![
        (
            "agents",
            "Agents",
            home.join(".agents/skills").join(SKILL_NAME),
        ),
        (
            "codex",
            "Codex",
            home.join(".codex/skills").join(SKILL_NAME),
        ),
        (
            "claude",
            "Claude Code",
            home.join(".claude/skills").join(SKILL_NAME),
        ),
        (
            "gemini",
            "Gemini",
            home.join(".gemini/skills").join(SKILL_NAME),
        ),
        (
            "antigravity",
            "Antigravity",
            home.join(".gemini/antigravity-cli/skills").join(SKILL_NAME),
        ),
        (
            "opencode",
            "OpenCode",
            home.join(".config/opencode/skills").join(SKILL_NAME),
        ),
    ])
}

fn prefixed_harness_directories(home: &Path, prefix: &str) -> Vec<PathBuf> {
    let mut directories = fs::read_dir(home)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_str()?;
            (path.is_dir() && name.starts_with(prefix) && name.len() > prefix.len()).then_some(path)
        })
        .collect::<Vec<_>>();
    directories.sort();
    directories
}

fn additional_codex_skill_targets(home: &Path) -> Vec<SkillTarget> {
    prefixed_harness_directories(home, ".codex-")
        .into_iter()
        .map(|directory| SkillTarget {
            id: format!("codex:{}", directory.display()),
            name: format!(
                "Codex ({})",
                directory.file_name().unwrap().to_string_lossy()
            ),
            path: directory
                .join("skills")
                .join(SKILL_NAME)
                .display()
                .to_string(),
            installed: directory
                .join("skills")
                .join(SKILL_NAME)
                .join("SKILL.md")
                .is_file(),
            up_to_date: false,
        })
        .collect()
}

fn additional_codex_agents(home: &Path) -> Vec<AgentInstallation> {
    prefixed_harness_directories(home, ".codex-")
        .into_iter()
        .map(|directory| {
            let config_path = directory.join("config.toml");
            let (mcp_registered, error) = match codex_mcp_registered_at(&config_path) {
                Ok(registered) => (registered, None),
                Err(error) => (false, Some(error)),
            };
            AgentInstallation {
                id: format!("codex:{}", directory.display()),
                name: format!(
                    "Codex ({})",
                    directory.file_name().unwrap().to_string_lossy()
                ),
                skill_installed: directory
                    .join("skills")
                    .join(SKILL_NAME)
                    .join("SKILL.md")
                    .is_file(),
                mcp_registered,
                available: true,
                config_path: config_path.display().to_string(),
                error,
            }
        })
        .collect()
}

fn skill_target(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    if let Some(directory) = id.strip_prefix("codex:") {
        let home = home_dir(app)?;
        let directory = PathBuf::from(directory);
        if directory.parent() == Some(home.as_path())
            && directory.is_dir()
            && directory
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".codex-"))
        {
            return Ok(directory.join("skills").join(SKILL_NAME));
        }
    }
    skill_targets(app)?
        .into_iter()
        .find(|(candidate, _, _)| *candidate == id)
        .map(|(_, _, path)| path)
        .ok_or_else(|| format!("Unsupported skill target: {id}"))
}

fn harness_available(id: &str, home: &Path) -> bool {
    match id {
        "codex" => home.join(".codex").exists() || command_exists("codex"),
        "claude" => {
            home.join(".claude").exists()
                || home.join(".claude.json").is_file()
                || command_exists("claude")
        }
        "cursor" => home.join(".cursor").exists(),
        "antigravity" => home.join(".gemini/antigravity-cli").exists(),
        "opencode" => home.join(".config/opencode").exists() || command_exists("opencode"),
        _ => false,
    }
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

fn uninstall_claude_mcp() -> Result<bool, String> {
    let output = match Command::new("claude")
        .args(["mcp", "remove", SKILL_NAME, "--scope", "user"])
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
            "Claude Code rejected removal of the MCP configuration: {}",
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
    let claude_config = home.join(".claude.json");
    let bundled_hash = bundled_skill_source(app)
        .and_then(|source| skill_hash(&source))
        .ok();
    let targets = skill_targets(app)?
        .into_iter()
        .map(|(id, name, path)| SkillTarget {
            id: id.into(),
            name: name.into(),
            installed: path.join("SKILL.md").is_file(),
            up_to_date: path.join("SKILL.md").is_file()
                && bundled_hash.as_ref().is_none_or(|bundled_hash| {
                    skill_hash(&path)
                        .map(|installed_hash| installed_hash == *bundled_hash)
                        .unwrap_or(false)
                }),
            path: path.display().to_string(),
        })
        .collect::<Vec<_>>();
    let mut additional_targets = additional_codex_skill_targets(&home);
    for target in &mut additional_targets {
        target.up_to_date = target.installed
            && bundled_hash.as_ref().is_none_or(|bundled_hash| {
                skill_hash(Path::new(&target.path))
                    .map(|installed_hash| installed_hash == *bundled_hash)
                    .unwrap_or(false)
            });
    }
    let candidates = [
        (
            "codex",
            "Codex",
            codex_config_path(app)?,
            codex_mcp_registered(app),
        ),
        (
            "claude",
            "Claude Code",
            claude_config.clone(),
            json_mcp_registered(&claude_config, &["mcpServers"]),
        ),
        (
            "cursor",
            "Cursor",
            cursor_config.clone(),
            json_mcp_registered(&cursor_config, &["mcpServers"]),
        ),
        (
            "antigravity",
            "Antigravity",
            agy_config.clone(),
            json_mcp_registered(&agy_config, &["mcpServers"]),
        ),
        (
            "opencode",
            "OpenCode",
            opencode_config.clone(),
            json_mcp_registered(&opencode_config, &["mcp", "servers"]),
        ),
    ];
    let agents = candidates
        .into_iter()
        .filter(|(id, _, _, _)| harness_available(id, &home))
        .map(|(id, name, config_path, registration)| {
            let (mcp_registered, error) = match registration {
                Ok(registered) => (registered, None),
                Err(error) => (false, Some(error)),
            };
            AgentInstallation {
                id: id.into(),
                name: name.into(),
                skill_installed: targets
                    .iter()
                    .find(|target| target.name == name)
                    .is_some_and(|target| target.installed),
                mcp_registered,
                available: true,
                config_path: config_path.display().to_string(),
                error,
            }
        })
        .collect::<Vec<_>>();
    let additional_agents = additional_codex_agents(&home);
    Ok(SkillInstallation {
        installed: destination.join("SKILL.md").is_file(),
        mcp_registered: !agents.is_empty() && agents.iter().all(|agent| agent.mcp_registered),
        path: destination.display().to_string(),
        targets,
        additional_targets,
        agents,
        additional_agents,
    })
}

#[tauri::command]
fn get_snapshot(state: State<'_, Arc<ViewerState>>) -> Snapshot {
    state.snapshot.lock().unwrap().clone()
}
#[tauri::command]
fn save_markdown(
    markdown: String,
    app: AppHandle,
    state: State<'_, Arc<ViewerState>>,
) -> Result<(), String> {
    let path = state
        .path
        .lock()
        .map_err(|_| "State unavailable")?
        .clone()
        .ok_or("No Markdown source is selected")?;
    status_file::write_markdown(&path, &markdown)?;
    update(&app, &state);
    Ok(())
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
    let _ = app.emit("settings-update", &settings);
    state
        .path
        .lock()
        .map_err(|_| "State unavailable")?
        .replace(path);
    update(&app, &state);
    Ok(())
}
#[tauri::command]
fn use_default_source(
    app: AppHandle,
    state: State<'_, Arc<ViewerState>>,
) -> Result<Settings, String> {
    let path = ensure_default_source(&app)?;
    let mut settings = load_settings(&app)?;
    settings.source_path = path.display().to_string();
    save_settings(&app, &settings)?;
    let _ = app.emit("settings-update", &settings);
    state
        .path
        .lock()
        .map_err(|_| "State unavailable")?
        .replace(path);
    update(&app, &state);
    Ok(settings)
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
fn reveal_source(app: AppHandle) -> Result<(), String> {
    let path = PathBuf::from(load_settings(&app)?.source_path);
    let target = if path.exists() {
        path
    } else {
        path.parent()
            .map(Path::to_path_buf)
            .ok_or("The source path has no parent directory")?
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        if target.is_file() {
            command.arg("-R");
        }
        command.arg(&target);
        command
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer.exe");
        if target.is_file() {
            command.arg(format!("/select,{}", target.display()));
        } else {
            command.arg(&target);
        }
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = Command::new("xdg-open");
    #[cfg(all(unix, not(target_os = "macos")))]
    command.arg(&target);

    let status = command
        .status()
        .map_err(|e| format!("Unable to reveal {}: {e}", target.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Unable to reveal {}", target.display()))
    }
}
#[tauri::command]
fn open_settings(app: AppHandle) {
    show_settings_window(&app);
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
    save_settings(&app, &settings)?;
    let _ = app.emit("settings-update", settings);
    Ok(())
}
#[tauri::command]
fn get_skill_installation(app: AppHandle) -> Result<SkillInstallation, String> {
    integration_status(&app)
}
#[tauri::command]
fn install_skill(app: AppHandle) -> Result<SkillInstallation, String> {
    let source = bundled_skill_source(&app)?;
    for (_, _, destination) in skill_targets(&app)? {
        install_skill_at(&source, &destination)?;
    }
    integration_status(&app)
}

#[tauri::command]
fn install_skill_for_target(target: String, app: AppHandle) -> Result<SkillInstallation, String> {
    let source = bundled_skill_source(&app)?;
    let destination = skill_target(&app, &target)?;
    install_skill_at(&source, &destination)?;
    integration_status(&app)
}

#[tauri::command]
fn install_mcp_for_harness(harness: String, app: AppHandle) -> Result<SkillInstallation, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("Unable to locate the ctx-ppteer executable: {error}"))?;
    let home = home_dir(&app)?;
    if let Some(directory) = harness.strip_prefix("codex:") {
        let directory = PathBuf::from(directory);
        if directory.parent() == Some(home.as_path())
            && directory.is_dir()
            && directory
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".codex-"))
        {
            register_codex_mcp_at(&directory.join("config.toml"), &executable)?;
            return integration_status(&app);
        }
    }
    if !harness_available(&harness, &home) {
        return Err(format!("{harness} was not found on this computer"));
    }
    match harness.as_str() {
        "codex" => register_codex_mcp(&app, &executable)?,
        "claude" => {
            if !install_claude_mcp(&executable)? {
                return Err(
                    "Claude Code is not installed; its MCP tool was not registered.".into(),
                );
            }
        }
        "cursor" => register_standard_json_mcp(&home.join(".cursor/mcp.json"), &executable)?,
        "antigravity" => {
            register_standard_json_mcp(&home.join(".gemini/config/mcp_config.json"), &executable)?
        }
        "opencode" => {
            register_opencode_mcp(&home.join(".config/opencode/opencode.json"), &executable)?
        }
        _ => return Err(format!("Unsupported harness: {harness}")),
    }
    integration_status(&app)
}

#[tauri::command]
fn uninstall_mcp_for_harness(harness: String, app: AppHandle) -> Result<SkillInstallation, String> {
    let home = home_dir(&app)?;
    if let Some(directory) = harness.strip_prefix("codex:") {
        let directory = PathBuf::from(directory);
        if directory.parent() == Some(home.as_path())
            && directory.is_dir()
            && directory
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".codex-"))
        {
            unregister_codex_mcp_at(&directory.join("config.toml"))?;
            return integration_status(&app);
        }
    }
    if !harness_available(&harness, &home) {
        return Err(format!("{harness} was not found on this computer"));
    }
    match harness.as_str() {
        "codex" => unregister_codex_mcp(&app)?,
        "claude" => {
            if !uninstall_claude_mcp()? {
                return Err("Claude Code is not installed; its MCP tool was not removed.".into());
            }
        }
        "cursor" => unregister_json_mcp(&home.join(".cursor/mcp.json"), &["mcpServers"])?,
        "antigravity" => unregister_json_mcp(
            &home.join(".gemini/config/mcp_config.json"),
            &["mcpServers"],
        )?,
        "opencode" => unregister_json_mcp(
            &home.join(".config/opencode/opencode.json"),
            &["mcp", "servers"],
        )?,
        _ => return Err(format!("Unsupported harness: {harness}")),
    }
    integration_status(&app)
}
#[tauri::command]
fn set_pinned(pinned: bool, app: AppHandle) -> Result<(), String> {
    app.get_webview_window("main")
        .ok_or("Main window unavailable")?
        .set_always_on_top(pinned)
        .map_err(|e| e.to_string())?;
    let mut settings = load_settings(&app)?;
    settings.pinned = pinned;
    save_settings(&app, &settings)?;
    let _ = app.emit("settings-update", settings);
    Ok(())
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
    let menu = Menu::default(handle)?;
    let settings = MenuItem::with_id(handle, "settings", "Settings…", true, None::<&str>)?;
    if let Some(application) = menu.items()?.first().and_then(|item| item.as_submenu()) {
        // On macOS, the first default submenu is the native application menu.
        // Put Settings below About, before the standard separator and Services entry.
        application.insert(&settings, 1)?;
    }
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| {
        if event.id().as_ref() == "settings" {
            show_settings_window(app);
        }
    });
    Ok(())
}

fn show_settings_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(
        app,
        "settings",
        WebviewUrl::App("index.html#/settings".into()),
    )
    .title("ctx-ppteer Settings")
    .inner_size(520.0, 430.0)
    .min_inner_size(420.0, 360.0)
    .resizable(true)
    .center()
    .build();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_skill_contents_and_paths() {
        let directory = tempfile::tempdir().unwrap();
        let skill = directory.path().join("skill");
        fs::create_dir_all(skill.join("nested")).unwrap();
        fs::write(skill.join("SKILL.md"), "initial").unwrap();
        fs::write(skill.join("nested/notes.md"), "notes").unwrap();

        let original = skill_hash(&skill).unwrap();
        fs::write(skill.join("nested/notes.md"), "updated notes").unwrap();
        assert_ne!(original, skill_hash(&skill).unwrap());
        fs::rename(skill.join("nested"), skill.join("renamed")).unwrap();
        assert_ne!(original, skill_hash(&skill).unwrap());
    }

    #[test]
    fn finds_only_suffixed_codex_directories() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir_all(directory.path().join(".codex")).unwrap();
        fs::create_dir_all(directory.path().join(".codex-zeta")).unwrap();
        fs::create_dir_all(directory.path().join(".codex-work")).unwrap();
        fs::write(directory.path().join(".codex-file"), "not a directory").unwrap();

        let found = prefixed_harness_directories(directory.path(), ".codex-");
        let names = found
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(names, [".codex-work", ".codex-zeta"]);
    }

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
    fn unregisters_standard_json_mcp_without_losing_existing_servers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp.json");
        fs::write(
            &path,
            r#"{"mcpServers":{"ctx-ppteer":{"command":"ctx-ppteer"},"existing":{"command":"existing"}}}"#,
        )
        .unwrap();

        unregister_json_mcp(&path, &["mcpServers"]).unwrap();

        let config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(config["mcpServers"]["existing"].is_object());
        assert!(config["mcpServers"].get(SKILL_NAME).is_none());
    }

    #[test]
    fn treats_an_empty_json_config_as_unconfigured() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp.json");
        fs::write(&path, "\n").unwrap();

        assert!(!json_mcp_registered(&path, &["mcpServers"]).unwrap());
        register_standard_json_mcp(&path, Path::new("/Applications/ctx-ppteer")).unwrap();

        let config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(config["mcpServers"][SKILL_NAME].is_object());
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

    #[test]
    fn recognizes_missing_codex_mcp_servers_table() {
        let document = DocumentMut::from_str("").unwrap();

        assert!(!document_has_codex_mcp(&document));
    }

    #[test]
    fn recognizes_inline_codex_mcp_server() {
        let document = DocumentMut::from_str(
            r#"[mcp_servers]
ctx-ppteer = { command = "/Applications/ctx-ppteer", args = ["--mcp"] }
"#,
        )
        .unwrap();

        assert!(document_has_codex_mcp(&document));
    }

    #[test]
    fn recognizes_inline_codex_mcp_servers_table() {
        let document = DocumentMut::from_str(
            r#"mcp_servers = { ctx-ppteer = { command = "/Applications/ctx-ppteer", args = ["--mcp"] } }
"#,
        )
        .unwrap();

        assert!(document_has_codex_mcp(&document));
    }

    #[test]
    fn registers_into_inline_codex_mcp_servers_table() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        fs::write(
            &path,
            r#"mcp_servers = { existing = { command = "existing" } }
"#,
        )
        .unwrap();

        register_codex_mcp_at(&path, Path::new("/Applications/ctx-ppteer")).unwrap();

        let document = DocumentMut::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(document_has_codex_mcp(&document));
        assert!(document
            .as_table()
            .get("mcp_servers")
            .and_then(|servers| servers.as_inline_table())
            .and_then(|servers| servers.get("existing"))
            .is_some());
    }

    #[test]
    fn unregisters_inline_codex_mcp_servers_table() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        fs::write(
            &path,
            r#"mcp_servers = { ctx-ppteer = { command = "/Applications/ctx-ppteer", args = ["--mcp"] }, existing = { command = "existing" } }
"#,
        )
        .unwrap();

        unregister_codex_mcp_at(&path).unwrap();

        let document = DocumentMut::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(!document_has_codex_mcp(&document));
        assert!(document
            .as_table()
            .get("mcp_servers")
            .and_then(|servers| servers.as_inline_table())
            .and_then(|servers| servers.get("existing"))
            .is_some());
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = new_viewer_state();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            save_markdown,
            set_source,
            use_default_source,
            get_default_source,
            get_settings,
            reveal_source,
            open_settings,
            set_appearance_settings,
            get_skill_installation,
            install_skill,
            install_skill_for_target,
            install_mcp_for_harness,
            uninstall_mcp_for_harness,
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
            app.get_webview_window("main")
                .ok_or_else(|| std::io::Error::other("Main window unavailable"))?
                .set_always_on_top(settings.pinned)
                .map_err(std::io::Error::other)?;
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
