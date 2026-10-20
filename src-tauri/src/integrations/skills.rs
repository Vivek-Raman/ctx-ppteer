use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::{path::BaseDirectory, AppHandle, Manager};

use crate::{
    integrations::registry::{additional_codex_directory, skill_path, AGENTS},
    paths::home_dir,
    SKILL_NAME,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillInstallation {
    pub installed: bool,
    pub mcp_registered: bool,
    pub path: String,
    pub targets: Vec<SkillTarget>,
    pub additional_targets: Vec<SkillTarget>,
    pub agents: Vec<AgentInstallation>,
    pub additional_agents: Vec<AgentInstallation>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillTarget {
    pub id: String,
    pub name: String,
    pub path: String,
    pub installed: bool,
    pub up_to_date: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentInstallation {
    pub id: String,
    pub name: String,
    pub skill_installed: bool,
    pub mcp_registered: bool,
    pub available: bool,
    pub config_path: String,
    pub error: Option<String>,
}

pub(crate) fn install_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(home_dir(app)?
        .join(".agents")
        .join("skills")
        .join(SKILL_NAME))
}

pub(crate) fn bundled_source(app: &AppHandle) -> Result<PathBuf, String> {
    let source = app
        .path()
        .resolve(format!("skills/{SKILL_NAME}"), BaseDirectory::Resource)
        .map_err(|e| format!("Unable to find the bundled skill: {e}"))?;
    if !source.join("SKILL.md").is_file() {
        return Err("The bundled skill is missing SKILL.md".into());
    }
    Ok(source)
}

pub(crate) fn copy_directory(source: &Path, destination: &Path) -> Result<(), String> {
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

pub(crate) fn skill_hash(path: &Path) -> Result<String, String> {
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

pub(crate) fn install_at(source: &Path, destination: &Path) -> Result<bool, String> {
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

pub(crate) fn targets(
    app: &AppHandle,
) -> Result<Vec<(&'static str, &'static str, PathBuf)>, String> {
    let home = home_dir(app)?;
    Ok(std::iter::once(("agents", "Agents", install_path(app)?))
        .chain(
            AGENTS
                .iter()
                .filter(|agent| agent.supports_skill)
                .map(|agent| (agent.id, agent.name, skill_path(&home, agent))),
        )
        .collect())
}

pub(crate) fn target(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    if let Some(directory) = id.strip_prefix("codex:") {
        let home = home_dir(app)?;
        let directory = PathBuf::from(directory);
        if additional_codex_directory(&home, id).is_some() {
            return Ok(directory.join("skills").join(SKILL_NAME));
        }
    }
    targets(app)?
        .into_iter()
        .find(|(candidate, _, _)| *candidate == id)
        .map(|(_, _, path)| path)
        .ok_or_else(|| format!("Unsupported skill target: {id}"))
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
}
