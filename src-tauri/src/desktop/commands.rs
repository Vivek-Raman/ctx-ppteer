use std::{path::PathBuf, sync::Arc};
use tauri::{AppHandle, Manager, State};

use crate::{
    desktop::windows::show_settings_window,
    integrations::config::{
        install_claude_mcp, register_codex_mcp, register_codex_mcp_at, register_opencode_mcp,
        register_standard_json_mcp, uninstall_claude_mcp, unregister_codex_mcp_at,
        unregister_json_mcp,
    },
    integrations::registry::{additional_codex_directory, config_path, descriptor, ConfigAdapter},
    integrations::skills::{self, SkillInstallation},
    integrations::status::{harness_available, integration_status},
    paths,
    preferences::{Settings, SettingsOwner},
    status::snapshot::{update, Snapshot, ViewerState},
};

#[tauri::command]
pub(crate) fn get_snapshot(state: State<'_, Arc<ViewerState>>) -> Snapshot {
    state.snapshot.lock().unwrap().clone()
}

#[tauri::command]
pub(crate) fn dismiss_point(
    folder_name: String,
    session_id: Option<String>,
    branch: String,
    updated_at: String,
    point_id: Option<String>,
    point_index: Option<usize>,
    app: AppHandle,
    state: State<'_, Arc<ViewerState>>,
) -> Result<(), String> {
    if !state
        .store
        .dismiss(
            &folder_name,
            session_id.as_deref(),
            &branch,
            &updated_at,
            point_id.as_deref().unwrap_or(""),
            point_index,
        )
        .map_err(|error| error.to_string())?
    {
        return Err("This point changed before it could be dismissed. Please try again.".into());
    }
    update(&app, &state);
    Ok(())
}

#[tauri::command]
pub(crate) fn get_settings(owner: State<'_, Arc<SettingsOwner>>) -> Result<Settings, String> {
    Ok(owner.snapshot())
}

#[tauri::command]
pub(crate) fn reset_settings(
    app: AppHandle,
    owner: State<'_, Arc<SettingsOwner>>,
    state: State<'_, Arc<ViewerState>>,
) -> Result<Settings, String> {
    let settings = owner.reset()?;
    app.get_webview_window("main")
        .ok_or_else(|| "Main window unavailable".to_string())?
        .set_always_on_top(settings.pinned)
        .map_err(|error| error.to_string())?;
    update(&app, &state);
    Ok(settings)
}

#[tauri::command]
pub(crate) fn open_settings(app: AppHandle) {
    show_settings_window(&app);
}

#[tauri::command]
pub(crate) fn set_appearance_settings(
    theme: String,
    primary_color: String,
    text_scale: f64,
    owner: State<'_, Arc<SettingsOwner>>,
) -> Result<(), String> {
    owner.set_appearance(&theme, primary_color, text_scale)?;
    Ok(())
}

#[tauri::command]
pub(crate) fn set_project_sort_order(
    project_sort_order: String,
    owner: State<'_, Arc<SettingsOwner>>,
) -> Result<(), String> {
    owner.set_sort_order(&project_sort_order)?;
    Ok(())
}

#[tauri::command]
pub(crate) fn set_auto_clear_after_minutes(
    auto_clear_after_minutes: Option<u64>,
    app: AppHandle,
    owner: State<'_, Arc<SettingsOwner>>,
    state: State<'_, Arc<ViewerState>>,
) -> Result<(), String> {
    let settings = owner.set_retention(auto_clear_after_minutes)?;
    if let Some(minutes) = settings.auto_clear_after_minutes {
        if let Err(error) = state
            .store
            .prune(std::time::Duration::from_secs(minutes.saturating_mul(60)))
        {
            return Err(format!(
                "Auto-clear setting saved, but pruning failed: {error}"
            ));
        }
        update(&app, &state);
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn set_pinned(
    pinned: bool,
    app: AppHandle,
    owner: State<'_, Arc<SettingsOwner>>,
) -> Result<(), String> {
    owner.set_pinned(pinned)?;
    app.get_webview_window("main")
        .ok_or_else(|| "Main window unavailable".to_string())?
        .set_always_on_top(pinned)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_skill_installation(app: AppHandle) -> Result<SkillInstallation, String> {
    integration_status(&app)
}

#[tauri::command]
pub(crate) fn install_skill(app: AppHandle) -> Result<SkillInstallation, String> {
    let source = skills::bundled_source(&app)?;
    for (_, _, destination) in skills::targets(&app)? {
        skills::install_at(&source, &destination)?;
    }
    integration_status(&app)
}

#[tauri::command]
pub(crate) fn install_skill_for_target(
    target: String,
    app: AppHandle,
) -> Result<SkillInstallation, String> {
    let source = skills::bundled_source(&app)?;
    let destination = skills::target(&app, &target)?;
    skills::install_at(&source, &destination)?;
    integration_status(&app)
}

#[tauri::command]
pub(crate) fn install_mcp_for_harness(
    harness: String,
    app: AppHandle,
) -> Result<SkillInstallation, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("Unable to locate the ctx-ppteer executable: {error}"))?;
    let home = paths::home_dir(&app)?;
    if let Some(directory) = harness.strip_prefix("codex:") {
        let directory = PathBuf::from(directory);
        if additional_codex_directory(&home, &harness).is_some() {
            register_codex_mcp_at(&directory.join("config.toml"), &executable)?;
            return integration_status(&app);
        }
    }
    if !harness_available(&harness, &home) {
        return Err(format!("{harness} was not found on this computer"));
    }
    let agent = descriptor(&harness).ok_or_else(|| format!("Unsupported harness: {harness}"))?;
    if !agent.supports_mcp {
        return Err(format!("{harness} does not support MCP installation"));
    }
    match agent.adapter {
        ConfigAdapter::CodexToml => register_codex_mcp(&config_path(&home, agent), &executable)?,
        ConfigAdapter::ClaudeCli => {
            if !install_claude_mcp(&executable)? {
                return Err(
                    "Claude Code is not installed; its MCP tool was not registered.".into(),
                );
            }
        }
        ConfigAdapter::StandardJson => {
            register_standard_json_mcp(&config_path(&home, agent), &executable)?
        }
        ConfigAdapter::OpenCodeJson => {
            register_opencode_mcp(&config_path(&home, agent), &executable)?
        }
    }
    integration_status(&app)
}

#[tauri::command]
pub(crate) fn uninstall_mcp_for_harness(
    harness: String,
    app: AppHandle,
) -> Result<SkillInstallation, String> {
    let home = paths::home_dir(&app)?;
    if let Some(directory) = harness.strip_prefix("codex:") {
        let directory = PathBuf::from(directory);
        if additional_codex_directory(&home, &harness).is_some() {
            unregister_codex_mcp_at(&directory.join("config.toml"))?;
            return integration_status(&app);
        }
    }
    if !harness_available(&harness, &home) {
        return Err(format!("{harness} was not found on this computer"));
    }
    let agent = descriptor(&harness).ok_or_else(|| format!("Unsupported harness: {harness}"))?;
    if !agent.supports_mcp {
        return Err(format!("{harness} does not support MCP removal"));
    }
    match agent.adapter {
        ConfigAdapter::CodexToml => {
            crate::integrations::config::unregister_codex_mcp_at(&config_path(&home, agent))?
        }
        ConfigAdapter::ClaudeCli => {
            if !uninstall_claude_mcp()? {
                return Err("Claude Code is not installed; its MCP tool was not removed.".into());
            }
        }
        ConfigAdapter::StandardJson => {
            unregister_json_mcp(&config_path(&home, agent), &["mcpServers"])?
        }
        ConfigAdapter::OpenCodeJson => {
            unregister_json_mcp(&config_path(&home, agent), &["mcp", "servers"])?
        }
    }
    integration_status(&app)
}
