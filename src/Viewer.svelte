<script lang="ts">
  import type { FlipParams } from "svelte/animate";
  import { fade, fly } from "svelte/transition";
  import { onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { command } from "./commands";
  import { applyPrimaryColor, applyTheme, systemTheme } from "./theme";
  import type { Settings, Snapshot, StatusEntry } from "./app-types";

  let revision = -1;
  let selectedText = false;
  let pending: Snapshot | null = null;
  let entries: StatusEntry[] = [];
  let statusError = "";
  let dismissing = "";
  let pendingRemoval = "";
  let settings: Settings | null = null;
  $: projects = [...new Set(entries.map((entry) => entry.folder_name))].sort((left, right) => {
    if (settings?.projectSortOrder === "project-directory-asc") return left.toLowerCase().localeCompare(right.toLowerCase());
    const lastUpdatedAt = (folder: string) => Math.max(...entries.filter((entry) => entry.folder_name === folder).map((entry) => new Date(entry.updated_at).getTime()));
    const difference = lastUpdatedAt(left) - lastUpdatedAt(right);
    if (difference) return settings?.projectSortOrder === "updated-at-asc" ? difference : -difference;
    return left.toLowerCase().localeCompare(right.toLowerCase());
  });
  let windowFocused = true;
  let now = Date.now();
  let expiryCheck: number | undefined;
  const themes = ["system", "light", "dark"] as const;
  const textScales = [0.85, 1, 1.15, 1.35];
  const fadeLeadTime = 10 * 60_000;
  const dismissalExitDuration = 320;

  function applySnapshot(value: Snapshot) {
    if (value.revision <= revision) return;
    if (selectedText || pendingRemoval) { pending = value; return; }
    revision = value.revision;
    pending = null;
    entries = value.document.entries;
    statusError = value.error ?? "";
    scheduleExpiryCheck();
  }
  function updateSelection() {
    selectedText = Boolean(document.getSelection()?.toString());
    if (!selectedText && pending) applySnapshot(pending);
  }
  function safeLink(link: string | null) {
    if (!link) return null;
    try { const url = new URL(link); return ["https:", "http:"].includes(url.protocol) ? url.href : null; }
    catch { return null; }
  }
  function pointKey(entry: StatusEntry, point: StatusEntry["points"][number], pointIndex: number) {
    return `${entry.folder_name}:${entry.session_id ?? ""}:${point.submitted_at ?? `${entry.updated_at}:${pointIndex}`}`;
  }
  function moveWithoutScaling(_: Element, { from, to }: FlipParams) {
    const x = from.left - to.left;
    const y = from.top - to.top;
    return {
      duration: 300,
      css: (t: number) => `transform: translate(${x * (1 - t)}px, ${y * (1 - t)}px)`,
    };
  }
  function relativeTime(value: string) {
    const minutes = Math.max(0, Math.floor((Date.now() - new Date(value).getTime()) / 60_000));
    if (!minutes) return "now";
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    const remainingMinutes = minutes % 60;
    if (hours < 24) return `${hours}h${remainingMinutes ? ` ${remainingMinutes}m` : ""}`;
    const days = Math.floor(hours / 24);
    const remainingHours = hours % 24;
    return `${days}d${remainingHours ? ` ${remainingHours}h` : ""}`;
  }
  function expiresSoon(value: string) {
    const autoClearAfterMinutes = settings?.autoClearAfterMinutes;
    if (autoClearAfterMinutes === null || autoClearAfterMinutes === undefined) return false;
    const remainingMilliseconds = new Date(value).getTime() + autoClearAfterMinutes * 60_000 - now;
    return remainingMilliseconds <= fadeLeadTime;
  }
  function scheduleExpiryCheck() {
    if (expiryCheck !== undefined) window.clearTimeout(expiryCheck);
    const autoClearAfterMinutes = settings?.autoClearAfterMinutes;
    if (autoClearAfterMinutes === null || autoClearAfterMinutes === undefined) return;
    const expiryDuration = autoClearAfterMinutes * 60_000;
    const nextFadeAt = entries.flatMap((entry) => entry.points.map((point) => new Date(point.submitted_at ?? entry.updated_at).getTime() + expiryDuration - fadeLeadTime)).filter((fadeAt) => fadeAt > now).reduce((soonest, fadeAt) => Math.min(soonest, fadeAt), Infinity);
    if (nextFadeAt === Infinity) return;
    expiryCheck = window.setTimeout(() => {
      now = Date.now();
      scheduleExpiryCheck();
    }, nextFadeAt - now);
  }
  async function dismissPoint(entry: StatusEntry, pointIndex: number) {
    const id = `${entry.folder_name}:${entry.session_id ?? ""}:${entry.updated_at}:${pointIndex}`;
    const removalKey = pointKey(entry, entry.points[pointIndex], pointIndex);
    dismissing = id;
    pendingRemoval = removalKey;
    try {
      await Promise.all([
        command("dismiss_point", { folderName: entry.folder_name, sessionId: entry.session_id, branch: entry.branch, updatedAt: entry.updated_at, pointIndex }),
        new Promise<void>((resolve) => window.setTimeout(resolve, dismissalExitDuration)),
      ]);
      pendingRemoval = "";
      applySnapshot(pending ?? await command<Snapshot>("get_snapshot"));
    } catch (error) {
      pendingRemoval = "";
      statusError = error instanceof Error ? error.message : String(error);
    } finally {
      dismissing = "";
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
    void listen<Settings>("settings-update", (event) => { settings = event.payload; applyTheme(settings.theme); applyPrimaryColor(settings.primaryColor); document.documentElement.style.setProperty("--text-scale", String(settings.textScale)); scheduleExpiryCheck(); }).then((stop) => { unlistenSettings = stop; });
    void (async () => {
      settings = await command<Settings>("get_settings");
      applyTheme(settings.theme);
      applyPrimaryColor(settings.primaryColor);
      document.documentElement.style.setProperty("--text-scale", String(settings.textScale));
      applySnapshot(await command<Snapshot>("get_snapshot"));
    })().catch(console.error);
    return () => { if (expiryCheck !== undefined) window.clearTimeout(expiryCheck); window.removeEventListener("focus", onFocus); window.removeEventListener("blur", onBlur); systemTheme.removeEventListener("change", onThemeChange); document.removeEventListener("selectionchange", updateSelection); unlistenSource?.(); unlistenSettings?.(); };
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
      <button class="icon-button" onclick={() => void cycleTheme()} aria-label={`Theme: ${settings?.theme ?? "system"}. Click to change`} title={`Theme: ${settings?.theme ?? "system"}`}>
        {settings?.theme === "dark" ? "☾" : settings?.theme === "light" ? "☀" : "◐"}
      </button>
      <button class="icon-button" onclick={() => void changeTextScale(-1)} disabled={!settings || settings.textScale <= textScales[0]} aria-label="Decrease text size" title="Decrease text size">A−</button>
      <button class="icon-button" onclick={() => void changeTextScale(1)} disabled={!settings || settings.textScale >= textScales[textScales.length - 1]} aria-label="Increase text size" title="Increase text size">A+</button>
      <button onclick={() => void command("open_settings")}>Settings</button>
  </div>
  <section class="document" aria-label="Project status" onselectstart={() => selectedText = true}>
    {#each projects as folder (folder)}
      <div class="project-group" animate:moveWithoutScaling out:fade={{ duration: 180 }}>
        <h2>{folder}</h2>
        {#each [...new Set(entries.filter((entry) => entry.folder_name === folder).map((entry) => entry.branch))] as branch (`${folder}:${branch}`)}
          <div class="branch-group" animate:moveWithoutScaling out:fade={{ duration: 180 }}>
            <p>{branch}</p>
            <ul>
              {#each entries.filter((entry) => entry.folder_name === folder && entry.branch === branch) as entry}
                {#each entry.points as point, pointIndex (pointKey(entry, point, pointIndex))}
                  {@const pointId = `${entry.folder_name}:${entry.session_id ?? ""}:${entry.updated_at}:${pointIndex}`}
                  <li class="status-point" class:removing={pendingRemoval === pointKey(entry, point, pointIndex)} class:expiring-soon={expiresSoon(point.submitted_at ?? entry.updated_at)} animate:moveWithoutScaling>
                    <span class="point-text">{#if point.done}<strong>DONE</strong>: {" "}{/if}{#if safeLink(point.link)}<a href={safeLink(point.link)} target="_blank" rel="noopener noreferrer">{point.text}</a>{:else}{point.text}{/if}</span>
                    <span class="point-actions">
                      <span class="point-age">{relativeTime(point.submitted_at ?? entry.updated_at)}</span>
                      <button class="dismiss-point" onclick={() => void dismissPoint(entry, pointIndex)} disabled={dismissing === pointId} aria-label={`Dismiss ${point.text}`} title="Dismiss point">×</button>
                    </span>
                  </li>
                {/each}
              {/each}
            </ul>
          </div>
        {/each}
      </div>
    {/each}
    {#if statusError}<p class="status-error" role="alert">{statusError}</p>{/if}
  </section>
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
  .status-error { color: #c43d35; font-size: 0.8rem; }
  .status-point { position: relative; transition: opacity 160ms ease; }
  .status-point.removing { pointer-events: none; animation: dismiss-point 320ms ease-in forwards; }
  @keyframes dismiss-point { to { opacity: 0; transform: translateX(18px); } }
  .status-point.expiring-soon { opacity: 0.42; }
  .point-actions { position: absolute; top: 50%; right: 0; display: inline-flex; align-items: center; gap: 0.3rem; padding: 0.14rem 0.28rem 0.14rem 0.5rem; border-radius: 999px; transform: translateY(-50%); opacity: 0; pointer-events: none; color: var(--muted); background: color-mix(in srgb, var(--bg) 72%, transparent); backdrop-filter: blur(10px) saturate(1.15); -webkit-backdrop-filter: blur(10px) saturate(1.15); box-shadow: 0 2px 8px color-mix(in srgb, var(--text) 8%, transparent), inset 0 0 0 1px color-mix(in srgb, var(--bg) 28%, transparent); font-size: 0.78em; white-space: nowrap; transition: opacity 120ms ease; }
  .status-point:hover .point-actions, .status-point:focus-within .point-actions { opacity: 1; pointer-events: auto; }
  .dismiss-point { padding: 0 0.28rem; border: 0; border-radius: 0.3rem; color: inherit; background: transparent; font-size: 1.25em; line-height: 1; }
  .dismiss-point:not(:disabled):hover { color: #c43d35; background: color-mix(in srgb, #c43d35 12%, transparent); }
  .dismiss-point:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  @media (max-width: 440px), (max-height: 360px) { .line-art-extra { display: none; } }@media (max-width: 340px), (max-height: 280px) { .line-art-medium { display: none; } }@media (prefers-reduced-motion: reduce) { .line-art animate, .bottom-line-art animate { display: none; } }
</style>
