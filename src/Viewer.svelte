<script lang="ts">
  import { onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { command } from "./commands";
  import { renderMarkdown } from "./markdown";
  import { applyTheme, systemTheme } from "./theme";
  import type { Settings, Snapshot } from "./app-types";

  let revision = -1;
  let selectedText = false;
  let pending: Snapshot | null = null;
  let documentHtml = "";
  let settings: Settings | null = null;
  let windowFocused = true;
  let markdown = "";
  let editing = false;
  let saving = false;
  let saveError = "";

  function applySnapshot(value: Snapshot) {
    if (typeof value.revision === "number" && value.revision <= revision) return;
    if (typeof value.revision === "number") revision = value.revision;
    if (selectedText || editing) { pending = value; return; }
    pending = null;
    if (typeof value.markdown === "string") {
      markdown = value.markdown;
      documentHtml = renderMarkdown(value.markdown);
    }
  }
  function updateSelection() { selectedText = Boolean(document.getSelection()?.toString()); if (!selectedText && pending) applySnapshot(pending); }
  function startEditing() { editing = true; saveError = ""; }
  function cancelEditing() {
    editing = false;
    saveError = "";
    if (pending) applySnapshot(pending);
  }
  async function saveMarkdown() {
    saving = true;
    saveError = "";
    try {
      await command("save_markdown", { markdown });
      editing = false;
    } catch (error) {
      saveError = error instanceof Error ? error.message : "Unable to save changes";
    } finally {
      saving = false;
    }
  }
  function handleEditorKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") cancelEditing();
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      if (!saving) void saveMarkdown();
    }
  }

  onMount(() => {
    const onFocus = () => { windowFocused = true; };
    const onBlur = () => { windowFocused = false; };
    const onThemeChange = () => { if (settings?.theme === "system") applyTheme(settings.theme); };
    windowFocused = document.hasFocus();
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    systemTheme.addEventListener("change", onThemeChange);
    document.addEventListener("selectionchange", updateSelection);
    let unlistenSource: UnlistenFn | undefined;
    let unlistenSettings: UnlistenFn | undefined;
    void listen<Snapshot>("source-update", (event) => applySnapshot(event.payload)).then((stop) => { unlistenSource = stop; });
    void listen<Settings>("settings-update", (event) => { settings = event.payload; applyTheme(settings.theme); document.documentElement.style.setProperty("--text-scale", String(settings.textScale)); }).then((stop) => { unlistenSettings = stop; });
    void (async () => {
      settings = await command<Settings>("get_settings");
      applyTheme(settings.theme);
      document.documentElement.style.setProperty("--text-scale", String(settings.textScale));
      applySnapshot(await command<Snapshot>("get_snapshot"));
    })().catch(console.error);
    return () => { window.removeEventListener("focus", onFocus); window.removeEventListener("blur", onBlur); systemTheme.removeEventListener("change", onThemeChange); document.removeEventListener("selectionchange", updateSelection); unlistenSource?.(); unlistenSettings?.(); };
  });
</script>

<main>
  <div class="app-chrome" data-tauri-drag-region aria-hidden="true"></div>
  <div class="document-actions" class:window-unfocused={!windowFocused}>
    {#if editing}
      <button class="ghost-button" onclick={cancelEditing} disabled={saving}>Cancel</button>
      <button class="save-button" onclick={() => void saveMarkdown()} disabled={saving}>{saving ? "Saving..." : "Save"}</button>
    {:else}
      <button class="ghost-button" onclick={startEditing}>Edit</button>
      <button onclick={() => void command("open_settings")}>Settings</button>
    {/if}
  </div>
  {#if editing}
    <section class="editor-shell" aria-label="Markdown editor">
      <textarea bind:value={markdown} onkeydown={handleEditorKeydown} aria-label="Markdown source"></textarea>
      {#if saveError}<p class="save-error" role="alert">{saveError}</p>{/if}
    </section>
  {:else}
    <section class="document" aria-label="Markdown document" onselectstart={() => selectedText = true}>{@html documentHtml}</section>
  {/if}
</main>

<style>
  button { flex: none; border: 0; border-radius: 0.4rem; padding: 0.5rem 0.7rem; color: #111; background: #f2f2f2; font: inherit; font-size: 0.8rem; font-weight: 600; cursor: pointer; }
  button:disabled { cursor: default; opacity: 0.65; }
  .document-actions { position: fixed; z-index: 2; top: 8px; right: 10px; display: flex; gap: 0.35rem; opacity: 1; transition: opacity 160ms ease; }
  .document-actions button { padding: 0.28rem 0.5rem; font-size: 0.7rem; }
  .document-actions.window-unfocused { pointer-events: none; opacity: 0; }
  .ghost-button { border: 1px solid var(--border); color: var(--text); background: transparent; }
  .save-button { color: white; background: var(--accent); }
  .editor-shell { display: flex; flex: 1; flex-direction: column; min-height: 0; padding: 14px 16px 40px; }
  textarea { flex: 1; min-height: 0; resize: none; border: 0; padding: 0; color: var(--text); background: transparent; font: calc(1rem * var(--text-scale))/1.5 ui-monospace, SFMono-Regular, Menlo, monospace; }
  textarea:focus { outline: 0; }
  .save-error { margin: 0.5rem 0 0; color: #c43d35; font-size: 0.8rem; }
</style>
