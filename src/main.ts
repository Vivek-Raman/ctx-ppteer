import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { renderMarkdown } from "./markdown";
import "./styles.css";

type Health = "waiting" | "current" | "stale" | "error";
type Theme = "system" | "light" | "dark";
type Snapshot = {
  path?: string;
  markdown?: string;
  revision?: number;
  health?: Health;
  error?: string;
  modified_at?: string | null;
};

// Keep command names in one place so the Rust command surface can evolve without
// scattering IPC details through the view.
const commands = {
  snapshot: "get_snapshot",
  choose: "pick_source",
  setSource: "set_source",
  refresh: "refresh_source",
  pin: "set_pinned",
};

const byId = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const sourcePath = byId<HTMLInputElement>("source-path");
const documentView = byId<HTMLElement>("document");
const healthText = byId<HTMLElement>("health-text");
const healthIndicator = byId<HTMLElement>("health-indicator");
const modifiedTime = byId<HTMLElement>("modified-time");
const pinButton = byId<HTMLButtonElement>("pin-source");
const viewer = byId<HTMLElement>("viewer");
const settingsPage = byId<HTMLElement>("settings-page");
const themeSelect = byId<HTMLSelectElement>("theme-select");
const systemTheme = window.matchMedia("(prefers-color-scheme: dark)");
let revision = -1;
let pinned = true;
let selectedText = false;
let pending: Snapshot | null = null;

function healthLabel(snapshot: Snapshot): string {
  if (snapshot.health === "current") return "Displaying current content";
  if (snapshot.health === "stale") return snapshot.error || "Showing last readable content";
  if (snapshot.health === "error") return snapshot.error || "Unable to read file";
  return "Waiting for a Markdown file";
}

function applySnapshot(snapshot: Snapshot) {
  if (typeof snapshot.revision === "number" && snapshot.revision <= revision) return;
  if (typeof snapshot.revision === "number") revision = snapshot.revision;
  if (snapshot.path !== undefined) sourcePath.value = snapshot.path;
  healthText.textContent = healthLabel(snapshot);
  healthIndicator.className = `health-dot ${snapshot.health || "waiting"}`;
  modifiedTime.textContent = snapshot.modified_at ? `File modified ${formatDate(snapshot.modified_at)}` : "";
  if (selectedText) {
    pending = snapshot;
    return;
  }
  pending = null;
  if (typeof snapshot.markdown === "string") documentView.innerHTML = renderMarkdown(snapshot.markdown);
}

function formatDate(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.valueOf()) ? value : date.toLocaleString([], { dateStyle: "short", timeStyle: "short" });
}

async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(name, args);
}

async function chooseFile() {
  const path = await command<string | null>(commands.choose);
  if (path) await setSource(path);
}

async function setSource(path: string) {
  sourcePath.value = path;
  revision = -1;
  await command(commands.setSource, { path });
  await reload();
}

async function reload() {
  const snapshot = await command<Snapshot>(commands.snapshot);
  applySnapshot(snapshot);
}

function setPinned(value: boolean) {
  pinned = value;
  pinButton.setAttribute("aria-pressed", String(pinned));
  pinButton.textContent = pinned ? "Pinned" : "Unpinned";
  void command(commands.pin, { pinned }).catch(console.error);
}

byId<HTMLButtonElement>("choose-source").addEventListener("click", () => void chooseFile().catch(showError));
byId<HTMLButtonElement>("apply-source").addEventListener("click", () => {
  if (sourcePath.value.trim()) void setSource(sourcePath.value.trim()).catch(showError);
});
byId<HTMLButtonElement>("refresh-source").addEventListener("click", () => void command(commands.refresh).then(reload).catch(showError));
pinButton.addEventListener("click", () => setPinned(!pinned));
byId<HTMLButtonElement>("settings-button").addEventListener("click", () => {
  viewer.hidden = true;
  settingsPage.hidden = false;
  themeSelect.focus();
});
byId<HTMLButtonElement>("back-to-viewer").addEventListener("click", () => {
  settingsPage.hidden = true;
  viewer.hidden = false;
  byId<HTMLButtonElement>("settings-button").focus();
});
themeSelect.addEventListener("change", () => {
  const theme = themeSelect.value as Theme;
  localStorage.setItem("theme", theme);
  applyTheme(theme);
});
byId<HTMLInputElement>("text-scale").addEventListener("input", (event) => {
  const value = (event.target as HTMLInputElement).value;
  document.documentElement.style.setProperty("--text-scale", value);
  localStorage.setItem("text-scale", value);
});
documentView.addEventListener("selectstart", () => { selectedText = true; });
document.addEventListener("selectionchange", () => {
  selectedText = Boolean(document.getSelection()?.toString());
  if (!selectedText && pending) applySnapshot(pending);
});

function showError(error: unknown) {
  healthIndicator.className = "health-dot error";
  healthText.textContent = error instanceof Error ? error.message : "The source operation failed";
}

function applyTheme(theme: Theme) {
  const dark = theme === "dark" || (theme === "system" && systemTheme.matches);
  document.documentElement.classList.toggle("dark", dark);
  document.documentElement.style.colorScheme = dark ? "dark" : "light";
}

async function boot() {
  const storedTheme = localStorage.getItem("theme");
  const theme: Theme = storedTheme === "light" || storedTheme === "dark" ? storedTheme : "system";
  themeSelect.value = theme;
  applyTheme(theme);
  systemTheme.addEventListener("change", () => {
    if (themeSelect.value === "system") applyTheme("system");
  });
  const storedScale = localStorage.getItem("text-scale");
  if (storedScale) {
    document.documentElement.style.setProperty("--text-scale", storedScale);
    byId<HTMLInputElement>("text-scale").value = storedScale;
  }
  const unlisten: UnlistenFn = await listen<Snapshot>("source-update", (event) => applySnapshot(event.payload));
  window.addEventListener("beforeunload", () => unlisten());
  try { await reload(); } catch (error) { showError(error); }
}

void boot();
