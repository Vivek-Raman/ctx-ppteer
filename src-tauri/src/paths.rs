use std::{env, path::PathBuf};

use tauri::{AppHandle, Manager};

/// Shared by the desktop app and headless MCP, without needing app startup.
pub fn status_path() -> Result<PathBuf, String> {
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or("Unable to find the home directory")?;
    Ok(PathBuf::from(home).join(".local/share/ctx-ppteer/status.json"))
}

pub(crate) fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| format!("Unable to find home directory: {error}"))?;
    Ok(home.join(".config/ctx-ppteer/settings.json"))
}

pub(crate) fn home_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .home_dir()
        .map_err(|error| format!("Unable to find the home directory: {error}"))
}
