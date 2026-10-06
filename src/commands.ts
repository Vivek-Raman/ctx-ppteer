import { invoke } from "@tauri-apps/api/core";

export function command<T>(name: string, args?: Record<string, unknown>) {
  return invoke<T>(name, args);
}
