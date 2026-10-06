export type Theme = "system" | "light" | "dark";
export type ProjectSortOrder = "project-directory-asc";

export type Snapshot = {
  markdown?: string;
  revision?: number;
};

export type AgentInstallation = {
  id: string;
  name: string;
  skillInstalled: boolean;
  mcpRegistered: boolean;
  available: boolean;
  configPath: string;
  error?: string;
};

export type SkillTarget = {
  id: string;
  name: string;
  path: string;
  installed: boolean;
  upToDate: boolean;
};

export type SkillInstallation = {
  installed: boolean;
  mcpRegistered: boolean;
  path: string;
  targets: SkillTarget[];
  additionalTargets: SkillTarget[];
  agents: AgentInstallation[];
  additionalAgents: AgentInstallation[];
};

export type Settings = {
  sourcePath: string;
  theme: Theme;
  textScale: number;
  pinned: boolean;
  doubleClickToEdit: boolean;
  projectSortOrder: ProjectSortOrder;
};
