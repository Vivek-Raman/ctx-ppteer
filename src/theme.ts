import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Theme } from "./app-types";

export const systemTheme = window.matchMedia("(prefers-color-scheme: dark)");

export function applyTheme(theme: Theme) {
  const dark = theme === "dark" || (theme === "system" && systemTheme.matches);
  document.documentElement.classList.toggle("dark", dark);
  document.documentElement.style.colorScheme = dark ? "dark" : "light";
  void getCurrentWindow().setTheme(theme === "system" ? null : theme).catch(console.error);
}
