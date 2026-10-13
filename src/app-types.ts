export type Theme = "system" | "light" | "dark";
export type ProjectSortOrder = "project-directory-asc";

export type StatusPoint = { text: string; done: boolean; link: string | null };
export type StatusEntry = {
  folder_name: string;
  session_id: string | null;
  branch: string;
  points: StatusPoint[];
  updated_at: string;
};
export type Snapshot = {
  document: { version: number; entries: StatusEntry[] };
  revision: number;
  error: string | null;
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
  theme: Theme;
  primaryColor: string;
  textScale: number;
  pinned: boolean;
  projectSortOrder: ProjectSortOrder;
};
