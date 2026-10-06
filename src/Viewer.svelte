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

  function applySnapshot(value: Snapshot) {
    if (typeof value.revision === "number" && value.revision <= revision) return;
    if (typeof value.revision === "number") revision = value.revision;
    if (selectedText) { pending = value; return; }
    pending = null;
    if (typeof value.markdown === "string") documentHtml = renderMarkdown(value.markdown);
  }
  function updateSelection() { selectedText = Boolean(document.getSelection()?.toString()); if (!selectedText && pending) applySnapshot(pending); }

  onMount(() => {
    const onThemeChange = () => { if (settings?.theme === "system") applyTheme(settings.theme); };
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
    return () => { systemTheme.removeEventListener("change", onThemeChange); document.removeEventListener("selectionchange", updateSelection); unlistenSource?.(); unlistenSettings?.(); };
  });
</script>

<main>
  <div class="app-chrome" data-tauri-drag-region aria-hidden="true"></div>
  <button class="settings-button" onclick={() => void command("open_settings")}>Settings</button>
  <section class="document" aria-label="Markdown document" onselectstart={() => selectedText = true}>{@html documentHtml}</section>
</main>

<style>
  button { flex: none; border: 0; border-radius: 0.4rem; padding: 0.5rem 0.7rem; color: #111; background: #f2f2f2; font: inherit; font-size: 0.8rem; font-weight: 600; cursor: pointer; }
  .settings-button { position: fixed; z-index: 2; top: 8px; right: 10px; padding: 0.28rem 0.5rem; font-size: 0.7rem; }
</style>
