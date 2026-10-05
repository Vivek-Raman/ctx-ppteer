<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { renderMarkdown } from "./markdown";

  type Health = "waiting" | "current" | "stale" | "error";
  type Theme = "system" | "light" | "dark";
  type Snapshot = { path?: string; markdown?: string; revision?: number; health?: Health; error?: string; modified_at?: string | null };

  const commands = { snapshot: "get_snapshot", choose: "pick_source", setSource: "set_source", useDefault: "use_default_source", defaultSource: "get_default_source", refresh: "refresh_source", pin: "set_pinned" };
  const sourceOverrideKey = "source-override";
  const systemTheme = window.matchMedia("(prefers-color-scheme: dark)");
  let sourcePath = "";
  let defaultPath = "";
  let revision = -1;
  let pinned = true;
  let selectedText = false;
  let pending: Snapshot | null = null;
  let snapshot: Snapshot = { health: "waiting" };
  let documentHtml = "";
  let settingsOpen = false;
  let theme: Theme = "system";
  let textScale = "1";
  let settingsButton: HTMLButtonElement;
  let themeSelect: HTMLSelectElement;

  const command = <T>(name: string, args?: Record<string, unknown>) => invoke<T>(name, args);
  const healthLabel = (value: Snapshot) => value.health === "current" ? "Displaying current content" : value.health === "stale" ? value.error || "Showing last readable content" : value.health === "error" ? value.error || "Unable to read file" : "Waiting for a Markdown file";
  const formatDate = (value: string) => { const date = new Date(value); return Number.isNaN(date.valueOf()) ? value : date.toLocaleString([], { dateStyle: "short", timeStyle: "short" }); };

  function applyTheme(value: Theme) {
    const dark = value === "dark" || (value === "system" && systemTheme.matches);
    document.documentElement.classList.toggle("dark", dark);
    document.documentElement.style.colorScheme = dark ? "dark" : "light";
  }

  function applySnapshot(value: Snapshot) {
    if (typeof value.revision === "number" && value.revision <= revision) return;
    if (typeof value.revision === "number") revision = value.revision;
    if (value.path !== undefined) sourcePath = value.path;
    snapshot = value;
    if (selectedText) { pending = value; return; }
    pending = null;
    if (typeof value.markdown === "string") documentHtml = renderMarkdown(value.markdown);
  }

  function showError(error: unknown) { snapshot = { health: "error", error: error instanceof Error ? error.message : "The source operation failed" }; }
  async function reload() { applySnapshot(await command<Snapshot>(commands.snapshot)); }
  async function setSource(path: string) { sourcePath = path; revision = -1; await command(commands.setSource, { path }); localStorage.setItem(sourceOverrideKey, path); await reload(); }
  async function useDefaultSource() { revision = -1; await command(commands.useDefault); localStorage.removeItem(sourceOverrideKey); await reload(); }
  async function chooseFile() { const path = await command<string | null>(commands.choose); if (path) await setSource(path); }
  function setPinned(value: boolean) { pinned = value; void command(commands.pin, { pinned }).catch(console.error); }
  function updateTextScale() { document.documentElement.style.setProperty("--text-scale", textScale); localStorage.setItem("text-scale", textScale); }
  function updateSelection() { selectedText = Boolean(document.getSelection()?.toString()); if (!selectedText && pending) applySnapshot(pending); }
  async function openSettings() { settingsOpen = true; await tick(); themeSelect.focus(); }
  function closeSettings() { settingsOpen = false; void tick().then(() => settingsButton.focus()); }

  onMount(() => {
    const storedTheme = localStorage.getItem("theme");
    theme = storedTheme === "light" || storedTheme === "dark" ? storedTheme : "system";
    applyTheme(theme);
    textScale = localStorage.getItem("text-scale") || "1";
    updateTextScale();
    const onThemeChange = () => { if (theme === "system") applyTheme(theme); };
    systemTheme.addEventListener("change", onThemeChange);
    const onSelectionChange = () => updateSelection();
    document.addEventListener("selectionchange", onSelectionChange);
    let unlisten: UnlistenFn | undefined;
    void listen<Snapshot>("source-update", (event) => applySnapshot(event.payload)).then((stop) => { unlisten = stop; });
    void (async () => {
      defaultPath = await command<string>(commands.defaultSource);
      const sourceOverride = localStorage.getItem(sourceOverrideKey);
      if (sourceOverride) await setSource(sourceOverride);
      else await reload();
    })().catch(showError);
    return () => { systemTheme.removeEventListener("change", onThemeChange); document.removeEventListener("selectionchange", onSelectionChange); unlisten?.(); };
  });
</script>

<main aria-live="polite">
  {#if !settingsOpen}
    <section id="viewer">
      <header class="toolbar"><div class="view-controls" role="group" aria-label="Viewer controls">
        <button type="button" title="Read the source again" onclick={() => void command(commands.refresh).then(reload).catch(showError)}>Refresh</button>
        <button type="button" aria-pressed={pinned} title="Keep this window above other windows" onclick={() => setPinned(!pinned)}>{pinned ? "Pinned" : "Unpinned"}</button>
        <button bind:this={settingsButton} type="button" onclick={openSettings}>Settings</button>
        <label class="scale-control" title="Text scale"><span>Text</span><input bind:value={textScale} oninput={updateTextScale} type="range" min="0.85" max="1.35" step="0.05" /></label>
      </div></header>
      <div class="statusbar"><span class="health-dot" class:current={snapshot.health === "current"} class:stale={snapshot.health === "stale"} class:error={snapshot.health === "error"} aria-hidden="true"></span><span>{healthLabel(snapshot)}</span>{#if snapshot.modified_at}<time class="modified-time">File modified {formatDate(snapshot.modified_at)}</time>{/if}</div>
      <section class="document" aria-label="Markdown document" onselectstart={() => selectedText = true}>
        {#if documentHtml}{@html documentHtml}{:else}<div class="empty-state"><h1>Agent status</h1><p>Choose an aggregate Markdown file to begin watching it.</p></div>{/if}
      </section>
    </section>
  {:else}
    <section class="settings-page" aria-labelledby="settings-title">
      <header class="settings-header"><button type="button" onclick={closeSettings}>Back</button><h1 id="settings-title">Settings</h1></header>
      <div class="settings-content">
        <label class="setting-row" for="theme-select"><span><strong>Theme</strong><small>Choose how the app looks.</small></span><select bind:this={themeSelect} bind:value={theme} id="theme-select" onchange={() => { localStorage.setItem("theme", theme); applyTheme(theme); }}><option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option></select></label>
        <div class="setting-row source-setting"><span><strong>Markdown file</strong><small>Default: {defaultPath}</small></span><div class="source-input"><label class="path-label" for="source-path">Markdown file path</label><input bind:value={sourcePath} id="source-path" type="text" spellcheck="false" /><div class="source-actions"><button type="button" onclick={() => void chooseFile().catch(showError)}>Browse</button><button type="button" onclick={() => { if (sourcePath.trim()) void setSource(sourcePath.trim()).catch(showError); }}>Apply</button><button type="button" onclick={() => void useDefaultSource().catch(showError)}>Use default</button></div></div></div>
      </div>
    </section>
  {/if}
</main>
