use serde::Deserialize;
use std::{env, fs, path::PathBuf};

const SETTINGS_DIRECTORY: &str = ".config/ctx-ppteer";
const SETTINGS_FILE: &str = "settings.json";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    source_path: String,
}

pub fn path() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("CTX_PPTEER_SETTINGS_PATH") {
        return Ok(PathBuf::from(path));
    }
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or("Unable to find the home directory")?;
    Ok(PathBuf::from(home)
        .join(SETTINGS_DIRECTORY)
        .join(SETTINGS_FILE))
}

pub fn source_path() -> Result<PathBuf, String> {
    let path = path()?;
    let contents =
        fs::read(&path).map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    let settings: Settings = serde_json::from_slice(&contents)
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    if settings.source_path.trim().is_empty() {
        return Err(format!("{} has no sourcePath", path.display()));
    }
    Ok(PathBuf::from(settings.source_path))
}
