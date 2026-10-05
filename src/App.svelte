<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { renderMarkdown } from "./markdown";

  type Health = "waiting" | "current" | "stale" | "error";
  type Theme = "system" | "light" | "dark";
  type Snapshot = { path?: string; markdown?: string; revision?: number; health?: Health; error?: string };
  type SkillInstallation = { installed: boolean; path: string };
  type Settings = { sourcePath: string; theme: Theme; textScale: number };

  const commands = { snapshot: "get_snapshot", choose: "pick_source", setSource: "set_source", useDefault: "use_default_source", pin: "set_pinned", settings: "get_settings", setAppearance: "set_appearance_settings", skillStatus: "get_skill_installation", installSkill: "install_skill" };
  const systemTheme = window.matchMedia("(prefers-color-scheme: dark)");
  let revision = -1;
  let pinned = true;
  let selectedText = false;
  let pending: Snapshot | null = null;
  let documentHtml = "";
  let theme: Theme = "system";
  let textScale = "1";
  let skill: SkillInstallation | null = null;
  let installingSkill = false;
  let skillError = "";

  const command = <T>(name: string, args?: Record<string, unknown>) => invoke<T>(name, args);

  function applyTheme(value: Theme) {
    const dark = value === "dark" || (value === "system" && systemTheme.matches);
    document.documentElement.classList.toggle("dark", dark);
    document.documentElement.style.colorScheme = dark ? "dark" : "light";
    void getCurrentWindow().setTheme(value === "system" ? null : value).catch(console.error);
  }

  function applySnapshot(value: Snapshot) {
    if (typeof value.revision === "number" && value.revision <= revision) return;
    if (typeof value.revision === "number") revision = value.revision;
    if (selectedText) { pending = value; return; }
    pending = null;
    if (typeof value.markdown === "string") documentHtml = renderMarkdown(value.markdown);
  }

  async function reload() { applySnapshot(await command<Snapshot>(commands.snapshot)); }
  async function loadSettings() {
    const settings = await command<Settings>(commands.settings);
    theme = settings.theme;
    textScale = String(settings.textScale);
    applyTheme(theme);
    updateTextScale(false);
  }
  async function loadSkillStatus() { skill = await command<SkillInstallation>(commands.skillStatus); }
  async function installSkill() {
    installingSkill = true;
    skillError = "";
    try { skill = await command<SkillInstallation>(commands.installSkill); }
    catch (error) { skillError = error instanceof Error ? error.message : String(error); }
    finally { installingSkill = false; }
  }
  async function setSource(path: string) { revision = -1; await command(commands.setSource, { path }); await reload(); }
  async function useDefaultSource() { revision = -1; await command(commands.useDefault); await reload(); }
  async function chooseFile() { const path = await command<string | null>(commands.choose); if (path) await setSource(path); }
  function setPinned(value: boolean) { pinned = value; void command(commands.pin, { pinned }).catch(console.error); }
  function saveAppearance() { void command(commands.setAppearance, { theme, textScale: Number(textScale) }).catch(console.error); }
  function updateTextScale(save = true) { document.documentElement.style.setProperty("--text-scale", textScale); if (save) saveAppearance(); }
  function updateSelection() { selectedText = Boolean(document.getSelection()?.toString()); if (!selectedText && pending) applySnapshot(pending); }
  function changeTextScale(step: number) { textScale = String(Math.min(1.35, Math.max(0.85, Number(textScale) + step))); updateTextScale(); }

  function handleMenuAction(action: string) {
    if (action === "choose-source") void chooseFile().catch(console.error);
    else if (action === "use-default-source") void useDefaultSource().catch(console.error);
    else if (action === "theme-system" || action === "theme-light" || action === "theme-dark") { theme = action.replace("theme-", "") as Theme; applyTheme(theme); saveAppearance(); }
    else if (action === "text-smaller") changeTextScale(-0.05);
    else if (action === "text-larger") changeTextScale(0.05);
    else if (action === "text-reset") { textScale = "1"; updateTextScale(); }
    else if (action === "toggle-pinned") setPinned(!pinned);
  }

  onMount(() => {
    const onThemeChange = () => { if (theme === "system") applyTheme(theme); };
    systemTheme.addEventListener("change", onThemeChange);
    document.addEventListener("selectionchange", updateSelection);
    let unlistenSource: UnlistenFn | undefined;
    let unlistenMenu: UnlistenFn | undefined;
    void listen<Snapshot>("source-update", (event) => applySnapshot(event.payload)).then((stop) => { unlistenSource = stop; });
    void listen<string>("viewer-menu-action", (event) => handleMenuAction(event.payload)).then((stop) => { unlistenMenu = stop; });
    void (async () => {
      await loadSettings();
      await loadSkillStatus();
      await reload();
    })().catch(console.error);
    return () => { systemTheme.removeEventListener("change", onThemeChange); document.removeEventListener("selectionchange", updateSelection); unlistenSource?.(); unlistenMenu?.(); };
  });
</script>

<main>
  <div class="app-chrome" data-tauri-drag-region aria-hidden="true"></div>
  {#if skill && !skill.installed}
    <aside class="skill-setup" aria-label="Skill setup">
      <div>
        <strong>Connect an agent</strong>
        <p>Install the ctx-ppteer skill so an agent can write status updates for this window.</p>
      </div>
      <button onclick={() => void installSkill()} disabled={installingSkill}>
        {installingSkill ? "Installing…" : "Install skill"}
      </button>
      {#if skillError}<p class="skill-error">{skillError}</p>{/if}
    </aside>
  {/if}
  <section class="document" aria-label="Markdown document" onselectstart={() => selectedText = true}>
    {@html documentHtml}
  </section>
</main>

<style>
  .skill-setup { display: flex; align-items: center; gap: 1rem; padding: 0.85rem 1rem; border-bottom: 1px solid #303030; background: #202020; }
  .skill-setup div { flex: 1; }
  .skill-setup strong { font-size: 0.88rem; }
  .skill-setup p { margin: 0.2rem 0 0; color: #aaa; font-size: 0.78rem; line-height: 1.35; }
  button { flex: none; border: 0; border-radius: 0.4rem; padding: 0.5rem 0.7rem; color: #111; background: #f2f2f2; font: inherit; font-size: 0.8rem; font-weight: 600; cursor: pointer; }
  button:disabled { cursor: wait; opacity: 0.65; }
  .skill-error { color: #ff9999 !important; }
</style>
