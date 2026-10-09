<script lang="ts">
  import { onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { command } from "./commands";
  import { renderMarkdown } from "./markdown";
  import { applyPrimaryColor, applyTheme, systemTheme } from "./theme";
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

  const themes = ["system", "light", "dark"] as const;
  const textScales = [0.85, 1, 1.15, 1.35];

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
  function handleDocumentDoubleClick() {
    if (settings?.doubleClickToEdit ?? true) startEditing();
  }
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

  async function cycleTheme() {
    if (!settings) return;
    const currentIndex = themes.indexOf(settings.theme);
    const theme = themes[(currentIndex + 1) % themes.length];
    await command("set_appearance_settings", { theme, primaryColor: settings.primaryColor, textScale: settings.textScale });
  }

  async function changeTextScale(direction: -1 | 1) {
    if (!settings) return;
    const currentIndex = textScales.indexOf(settings.textScale);
    const nextIndex = Math.max(0, Math.min(textScales.length - 1, currentIndex + direction));
    if (nextIndex === currentIndex) return;
    await command("set_appearance_settings", { theme: settings.theme, primaryColor: settings.primaryColor, textScale: textScales[nextIndex] });
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
    void listen<Settings>("settings-update", (event) => { settings = event.payload; applyTheme(settings.theme); applyPrimaryColor(settings.primaryColor); document.documentElement.style.setProperty("--text-scale", String(settings.textScale)); }).then((stop) => { unlistenSettings = stop; });
    void (async () => {
      settings = await command<Settings>("get_settings");
      applyTheme(settings.theme);
      applyPrimaryColor(settings.primaryColor);
      document.documentElement.style.setProperty("--text-scale", String(settings.textScale));
      applySnapshot(await command<Snapshot>("get_snapshot"));
    })().catch(console.error);
    return () => { window.removeEventListener("focus", onFocus); window.removeEventListener("blur", onBlur); systemTheme.removeEventListener("change", onThemeChange); document.removeEventListener("selectionchange", updateSelection); unlistenSource?.(); unlistenSettings?.(); };
  });
</script>

<main>
  <svg class="line-art" viewBox="0 0 1000 480" preserveAspectRatio="xMaxYMin meet" aria-hidden="true">
    <g fill="none" stroke="currentColor" stroke-linecap="round" vector-effect="non-scaling-stroke">
      <path d="M70-40C190-8 230 124 360 120C520 116 610 10 742 50C880 91 910 168 1060 140" stroke-width="2.5"><animate attributeName="opacity" values="0.55;0.9;0.55" keyTimes="0;0.42;1" begin="-4s" dur="18s" repeatCount="indefinite" /></path>
      <path class="line-art-medium" d="M190-40C290 19 290 171 420 168C555 165 650 84 780 123C910 165 938 246 1060 236" stroke-width="2.5"><animate attributeName="opacity" values="0.75;0.46;0.75" keyTimes="0;0.64;1" begin="-11s" dur="22s" repeatCount="indefinite" /></path>
      <path class="line-art-medium" d="M310-40C385 42 380 226 500 220C630 214 706 159 826 201C930 238 968 315 1060 307" stroke-width="2.5"><animate attributeName="opacity" values="0.38;0.76;0.38" keyTimes="0;0.31;1" begin="-7s" dur="20s" repeatCount="indefinite" /></path>
      <path class="line-art-extra" d="M420-40C478 72 474 283 583 273C694 263 758 213 859 251C944 284 982 357 1055 350" stroke-width="2.5"><animate attributeName="opacity" values="0.62;0.34;0.62" keyTimes="0;0.57;1" begin="-17s" dur="24s" repeatCount="indefinite" /></path>
    </g>
  </svg>
  <svg class="bottom-line-art" viewBox="0 0 1000 420" preserveAspectRatio="xMidYMax meet" aria-hidden="true">
    <g fill="none" stroke="currentColor" stroke-linecap="round" vector-effect="non-scaling-stroke">
      <path d="M-60 392C154 326 273 336 460 386C650 437 797 303 1060 365" stroke-width="2.5"><animate attributeName="opacity" values="0.45;0.72;0.45" keyTimes="0;0.48;1" begin="-9s" dur="20s" repeatCount="indefinite" /></path>
      <path d="M-45 444C176 352 360 378 544 431C745 488 862 361 1050 398" stroke-width="2.5"><animate attributeName="opacity" values="0.36;0.62;0.36" keyTimes="0;0.69;1" begin="-15s" dur="24s" repeatCount="indefinite" /></path>
    </g>
  </svg>
  <div class="app-chrome" data-tauri-drag-region aria-hidden="true"></div>
  <div class="document-actions" class:window-unfocused={!windowFocused}>
    {#if editing}
      <button class="ghost-button" onclick={cancelEditing} disabled={saving}>Cancel</button>
      <button class="save-button" onclick={() => void saveMarkdown()} disabled={saving}>{saving ? "Saving..." : "Save"}</button>
    {:else}
      <button class="ghost-button" onclick={startEditing}>Edit</button>
      <button class="icon-button" onclick={() => void cycleTheme()} aria-label={`Theme: ${settings?.theme ?? "system"}. Click to change`} title={`Theme: ${settings?.theme ?? "system"}`}>
        {settings?.theme === "dark" ? "☾" : settings?.theme === "light" ? "☀" : "◐"}
      </button>
      <button class="icon-button" onclick={() => void changeTextScale(-1)} disabled={!settings || settings.textScale <= textScales[0]} aria-label="Decrease text size" title="Decrease text size">A−</button>
      <button class="icon-button" onclick={() => void changeTextScale(1)} disabled={!settings || settings.textScale >= textScales[textScales.length - 1]} aria-label="Increase text size" title="Increase text size">A+</button>
      <button onclick={() => void command("open_settings")}>Settings</button>
    {/if}
  </div>
  {#if editing}
    <section class="editor-shell" aria-label="Markdown editor">
      <textarea bind:value={markdown} onkeydown={handleEditorKeydown} aria-label="Markdown source"></textarea>
      {#if saveError}<p class="save-error" role="alert">{saveError}</p>{/if}
    </section>
  {:else}
    <section class="document" aria-label="Markdown document" onselectstart={() => selectedText = true} ondblclick={handleDocumentDoubleClick}>{@html documentHtml}</section>
  {/if}
</main>

<style>
  button { flex: none; border: 1px solid transparent; border-radius: 0.4rem; padding: 0.5rem 0.7rem; color: var(--text); background: transparent; font: inherit; font-size: 0.8rem; font-weight: 600; cursor: pointer; }
  button:disabled { cursor: default; opacity: 0.65; }
  .line-art, .bottom-line-art { position: absolute; z-index: 0; height: auto; color: var(--accent); pointer-events: none; }.line-art { top: 0; right: 0; width: min(880px, 80%); aspect-ratio: 1000 / 480; opacity: 0.48; }.bottom-line-art { right: 0; bottom: 0; width: 100%; aspect-ratio: 1000 / 420; opacity: 0.55; }.document-actions { position: fixed; z-index: 2; top: 8px; right: 10px; display: flex; gap: 0.35rem; opacity: 1; transition: opacity 160ms ease; }
  .document-actions button { padding: 0.28rem 0.5rem; font-size: 0.7rem; }
  .document-actions button:not(:disabled):hover { border-color: var(--border); color: var(--accent); }
  .document-actions button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .icon-button { min-width: 1.8rem; padding-inline: 0.35rem !important; }
  .document-actions.window-unfocused { pointer-events: none; opacity: 0; }
  .ghost-button { border: 1px solid var(--border); color: var(--text); background: transparent; }
  .save-button { color: white; background: var(--accent); }
  .editor-shell { display: flex; flex: 1; flex-direction: column; min-height: 0; padding: 14px 16px 40px; }
  textarea { flex: 1; min-height: 0; resize: none; border: 0; padding: 0; color: var(--text); background: transparent; font: calc(1rem * var(--text-scale))/1.5 ui-monospace, SFMono-Regular, Menlo, monospace; }
  textarea:focus { outline: 0; }
  .save-error { margin: 0.5rem 0 0; color: #c43d35; font-size: 0.8rem; }
  @media (max-width: 440px), (max-height: 360px) { .line-art-extra { display: none; } }@media (max-width: 340px), (max-height: 280px) { .line-art-medium { display: none; } }@media (prefers-reduced-motion: reduce) { .line-art animate, .bottom-line-art animate { display: none; } }
</style>
