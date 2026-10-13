use std::{env, path::PathBuf};

/// Shared by the desktop app and headless MCP, without needing app startup.
pub fn status_path() -> Result<PathBuf, String> {
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or("Unable to find the home directory")?;
    Ok(PathBuf::from(home).join(".local/share/ctx-ppteer/status.json"))
}
