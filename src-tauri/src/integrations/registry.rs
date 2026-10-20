use std::path::{Path, PathBuf};

/// The stable identity and filesystem contract for one supported agent.
/// Display names belong at the UI boundary and must not be used for joins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConfigAdapter {
    CodexToml,
    ClaudeCli,
    StandardJson,
    OpenCodeJson,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AgentDescriptor {
    pub id: &'static str,
    pub name: &'static str,
    pub skill_directory: &'static str,
    pub config_path: &'static str,
    pub adapter: ConfigAdapter,
    pub availability: Availability,
    pub supports_skill: bool,
    pub supports_mcp: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Availability {
    DirectoryOrCommand(&'static str, &'static [&'static str]),
    Claude,
    Directory(&'static str),
}

const CODEX_COMMAND: &[&str] = &["codex"];
const OPENCODE_COMMAND: &[&str] = &["opencode"];

pub(crate) const AGENTS: &[AgentDescriptor] = &[
    AgentDescriptor {
        id: "codex",
        name: "Codex",
        skill_directory: ".codex/skills",
        config_path: ".codex/config.toml",
        adapter: ConfigAdapter::CodexToml,
        availability: Availability::DirectoryOrCommand(".codex", CODEX_COMMAND),
        supports_skill: true,
        supports_mcp: true,
    },
    AgentDescriptor {
        id: "claude",
        name: "Claude Code",
        skill_directory: ".claude/skills",
        config_path: ".claude.json",
        adapter: ConfigAdapter::ClaudeCli,
        availability: Availability::Claude,
        supports_skill: true,
        supports_mcp: true,
    },
    AgentDescriptor {
        id: "cursor",
        name: "Cursor",
        skill_directory: ".cursor/skills",
        config_path: ".cursor/mcp.json",
        adapter: ConfigAdapter::StandardJson,
        availability: Availability::Directory(".cursor"),
        supports_skill: false,
        supports_mcp: true,
    },
    AgentDescriptor {
        id: "gemini",
        name: "Gemini",
        skill_directory: ".gemini/skills",
        config_path: ".gemini/config/mcp_config.json",
        adapter: ConfigAdapter::StandardJson,
        availability: Availability::Directory(".gemini"),
        supports_skill: true,
        supports_mcp: false,
    },
    AgentDescriptor {
        id: "antigravity",
        name: "Antigravity",
        skill_directory: ".gemini/antigravity-cli/skills",
        config_path: ".gemini/config/mcp_config.json",
        adapter: ConfigAdapter::StandardJson,
        availability: Availability::Directory(".gemini/antigravity-cli"),
        supports_skill: true,
        supports_mcp: true,
    },
    AgentDescriptor {
        id: "opencode",
        name: "OpenCode",
        skill_directory: ".config/opencode/skills",
        config_path: ".config/opencode/opencode.json",
        adapter: ConfigAdapter::OpenCodeJson,
        availability: Availability::DirectoryOrCommand(".config/opencode", OPENCODE_COMMAND),
        supports_skill: true,
        supports_mcp: true,
    },
];

pub(crate) fn descriptor(id: &str) -> Option<&'static AgentDescriptor> {
    AGENTS.iter().find(|agent| agent.id == id)
}

pub(crate) fn skill_path(home: &Path, agent: &AgentDescriptor) -> PathBuf {
    home.join(agent.skill_directory).join("ctx-ppteer")
}

pub(crate) fn config_path(home: &Path, agent: &AgentDescriptor) -> PathBuf {
    home.join(agent.config_path)
}

pub(crate) fn additional_codex_id(directory: &Path) -> String {
    format!("codex:{}", directory.display())
}

pub(crate) fn additional_codex_directory(home: &Path, id: &str) -> Option<PathBuf> {
    let directory = PathBuf::from(id.strip_prefix("codex:")?);
    (directory.parent() == Some(home)
        && directory.is_dir()
        && directory
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(".codex-")))
    .then_some(directory)
}
