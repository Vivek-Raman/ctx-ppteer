import { invoke } from '@tauri-apps/api/core';
import type { Settings, SkillInstallation, Snapshot } from './types';

type CommandMap = {
  get_snapshot: { args: undefined; result: Snapshot };
  get_settings: { args: undefined; result: Settings };
  reset_settings: { args: undefined; result: Settings };
  open_settings: { args: undefined; result: void };
  set_appearance_settings: {
    args: { theme: Settings['theme']; primaryColor: string; textScale: number };
    result: void;
  };
  set_pinned: { args: { pinned: boolean }; result: void };
  set_project_sort_order: {
    args: { projectSortOrder: Settings['projectSortOrder'] };
    result: void;
  };
  set_auto_clear_after_minutes: { args: { autoClearAfterMinutes: number | null }; result: void };
  dismiss_point: {
    args: {
      folderName: string;
      sessionId: string | null;
      branch: string;
      updatedAt: string;
      pointId: string | null;
      pointIndex: number | null;
    };
    result: void;
  };
  get_skill_installation: { args: undefined; result: SkillInstallation };
  install_skill_for_target: { args: { target: string }; result: SkillInstallation };
  install_mcp_for_harness: { args: { harness: string }; result: SkillInstallation };
  uninstall_mcp_for_harness: { args: { harness: string }; result: SkillInstallation };
};

export function command<K extends keyof CommandMap>(
  name: K,
  ...args: CommandMap[K]['args'] extends undefined ? [] : [args: CommandMap[K]['args']]
): Promise<CommandMap[K]['result']> {
  return invoke<CommandMap[K]['result']>(name, args[0]);
}
