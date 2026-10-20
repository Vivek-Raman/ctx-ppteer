use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::AppHandle;

use crate::{
    integrations::{
        config::{codex_mcp_registered_at, command_exists, json_mcp_registered},
        registry::{
            additional_codex_id, config_path, descriptor, Availability, ConfigAdapter, AGENTS,
        },
        skills::{self, AgentInstallation, SkillInstallation, SkillTarget},
    },
    paths, SKILL_NAME,
};

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

pub(crate) fn additional_codex_skill_targets(home: &Path) -> Vec<SkillTarget> {
    prefixed_harness_directories(home, ".codex-")
        .into_iter()
        .map(|directory| SkillTarget {
            id: additional_codex_id(&directory),
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

pub(crate) fn additional_codex_agents(home: &Path) -> Vec<AgentInstallation> {
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

pub(crate) fn harness_available(id: &str, home: &Path) -> bool {
    harness_available_with(id, home, command_exists)
}

pub(crate) fn harness_available_with<F>(id: &str, home: &Path, command_detected: F) -> bool
where
    F: Fn(&str) -> bool,
{
    let Some(agent) = descriptor(id) else {
        return false;
    };
    match agent.availability {
        Availability::DirectoryOrCommand(directory, commands) => {
            home.join(directory).is_dir()
                || commands.iter().any(|command| command_detected(command))
        }
        Availability::Claude => {
            home.join(".claude").is_dir()
                || home.join(".claude.json").is_file()
                || command_detected("claude")
        }
        Availability::Directory(directory) => home.join(directory).is_dir(),
    }
}

pub(crate) fn integration_status(app: &AppHandle) -> Result<SkillInstallation, String> {
    let home = paths::home_dir(app)?;
    let destination = skills::install_path(app)?;
    let bundled_hash = skills::bundled_source(app)
        .and_then(|source| skills::skill_hash(&source))
        .ok();
    let targets = skills::targets(app)?
        .into_iter()
        .map(|(id, name, path)| SkillTarget {
            id: id.into(),
            name: name.into(),
            installed: path.join("SKILL.md").is_file(),
            up_to_date: path.join("SKILL.md").is_file()
                && bundled_hash.as_ref().is_some_and(|hash| {
                    skills::skill_hash(&path)
                        .map(|installed| installed == *hash)
                        .unwrap_or(false)
                }),
            path: path.display().to_string(),
        })
        .collect::<Vec<_>>();
    let mut additional_targets = additional_codex_skill_targets(&home);
    for target in &mut additional_targets {
        target.up_to_date = target.installed
            && bundled_hash.as_ref().is_some_and(|hash| {
                skills::skill_hash(Path::new(&target.path))
                    .map(|installed| installed == *hash)
                    .unwrap_or(false)
            });
    }
    let agents = AGENTS
        .iter()
        .filter(|agent| agent.supports_mcp && harness_available(agent.id, &home))
        .map(|agent| {
            let path = config_path(&home, agent);
            let registration = match agent.adapter {
                ConfigAdapter::CodexToml => codex_mcp_registered_at(&path),
                ConfigAdapter::ClaudeCli | ConfigAdapter::StandardJson => {
                    json_mcp_registered(&path, &["mcpServers"])
                }
                ConfigAdapter::OpenCodeJson => json_mcp_registered(&path, &["mcp", "servers"]),
            };
            let (mcp_registered, error) = match registration {
                Ok(registered) => (registered, None),
                Err(error) => (false, Some(error)),
            };
            AgentInstallation {
                id: agent.id.into(),
                name: agent.name.into(),
                skill_installed: targets
                    .iter()
                    .find(|target| target.id == agent.id)
                    .is_some_and(|target| target.installed),
                mcp_registered,
                available: true,
                config_path: path.display().to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn no_commands(_: &str) -> bool {
        false
    }

    #[test]
    fn detects_only_the_descriptor_rules() {
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path();
        assert!(!harness_available_with("codex", home, no_commands));
        fs::create_dir(home.join(".codex")).unwrap();
        assert!(harness_available_with("codex", home, no_commands));
        assert!(!harness_available_with("antigravity", home, no_commands));
        fs::create_dir_all(home.join(".gemini/config")).unwrap();
        fs::write(home.join(".gemini/config/mcp_config.json"), "{}").unwrap();
        assert!(!harness_available_with("antigravity", home, no_commands));
        fs::create_dir(home.join(".gemini/antigravity-cli")).unwrap();
        assert!(harness_available_with("antigravity", home, no_commands));
    }

    #[test]
    fn command_detection_is_injectable() {
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path();
        assert!(harness_available_with("opencode", home, |command| command == "opencode"));
        assert!(!harness_available_with("cursor", home, |_| true));
        assert!(!harness_available_with("gemini", home, |_| true));
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
}
