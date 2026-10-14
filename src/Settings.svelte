<script lang="ts">
  import { onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { command } from "./commands";
  import { applyPrimaryColor, applyTheme, systemTheme } from "./theme";
  import type { Settings, SkillInstallation } from "./app-types";

  let settings: Settings | null = null;
  let skill: SkillInstallation | null = null;
  let installingSkill: string | null = null;
  let installingMcp: string | null = null;
  let skillError = "";
  let saving = false;
  let settingsError = "";
  let autoClearValue = 1;
  let autoClearUnit: "minutes" | "hours" = "hours";
  const primaryColors = [
    { name: "Terracotta", value: "#c98c5a" },
    { name: "Rose", value: "#c96b7b" },
    { name: "Coral", value: "#d97757" },
    { name: "Gold", value: "#c59a38" },
    { name: "Olive", value: "#8b9b50" },
    { name: "Sage", value: "#5f9b85" },
    { name: "Teal", value: "#3f9a9a" },
    { name: "Blue", value: "#5d8fc2" },
    { name: "Violet", value: "#8a75bc" },
  ];

  async function resetSettings() {
    saving = true; settingsError = "";
    try { settings = await command<Settings>("reset_settings"); }
    catch (error) { settingsError = error instanceof Error ? error.message : String(error); }
    finally { saving = false; }
  }
  async function saveAppearance() {
    if (!settings) return;
    saving = true; settingsError = "";
    try { await command("set_appearance_settings", { theme: settings.theme, primaryColor: settings.primaryColor, textScale: Number(settings.textScale) }); await command("set_pinned", { pinned: settings.pinned }); }
    catch (error) { settingsError = error instanceof Error ? error.message : String(error); }
    finally { saving = false; }
  }
  function selectPrimaryColor(primaryColor: string) {
    if (!settings) return;
    settings.primaryColor = primaryColor;
    void saveAppearance();
  }
  async function saveProjectSortOrder() {
    if (!settings) return;
    saving = true; settingsError = "";
    try { await command("set_project_sort_order", { projectSortOrder: settings.projectSortOrder }); }
    catch (error) { settingsError = error instanceof Error ? error.message : String(error); }
    finally { saving = false; }
  }
  async function saveAutoClear() {
    if (!settings) return;
    saving = true; settingsError = "";
    try {
      const minutes = settings.autoClearAfterMinutes === null ? null : Math.round(autoClearValue * (autoClearUnit === "hours" ? 60 : 1));
      await command("set_auto_clear_after_minutes", { autoClearAfterMinutes: minutes });
    } catch (error) { settingsError = error instanceof Error ? error.message : String(error); }
    finally { saving = false; }
  }
  async function installSkill(target: string) {
    installingSkill = target; skillError = "";
    try { skill = await command<SkillInstallation>("install_skill_for_target", { target }); }
    catch (error) { skillError = error instanceof Error ? error.message : String(error); }
    finally { installingSkill = null; }
  }
  async function installMcp(harness: string) {
    installingMcp = harness; skillError = "";
    try { skill = await command<SkillInstallation>("install_mcp_for_harness", { harness }); }
    catch (error) { skillError = error instanceof Error ? error.message : String(error); }
    finally { installingMcp = null; }
  }
  async function uninstallMcp(harness: string) {
    installingMcp = harness; skillError = "";
    try { skill = await command<SkillInstallation>("uninstall_mcp_for_harness", { harness }); }
    catch (error) { skillError = error instanceof Error ? error.message : String(error); }
    finally { installingMcp = null; }
  }

  onMount(() => {
    const onThemeChange = () => { if (settings?.theme === "system") applyTheme(settings.theme); };
    systemTheme.addEventListener("change", onThemeChange);
    let unlisten: UnlistenFn | undefined;
    void listen<Settings>("settings-update", (event) => { settings = event.payload; applyTheme(settings.theme); applyPrimaryColor(settings.primaryColor); }).then((stop) => { unlisten = stop; });
    void (async () => { settings = await command<Settings>("get_settings"); if (settings.autoClearAfterMinutes !== null) { autoClearUnit = settings.autoClearAfterMinutes % 60 === 0 ? "hours" : "minutes"; autoClearValue = settings.autoClearAfterMinutes / (autoClearUnit === "hours" ? 60 : 1); } applyTheme(settings.theme); applyPrimaryColor(settings.primaryColor); skill = await command<SkillInstallation>("get_skill_installation"); })().catch((error) => { skillError = error instanceof Error ? error.message : String(error); });
    return () => { systemTheme.removeEventListener("change", onThemeChange); unlisten?.(); };
  });
</script>

<main class="settings-page">
  <header><h1>Settings</h1><p>Choose what ctx-ppteer shows and how it appears.</p></header>
  {#if settings}
    <section class="settings-group appearance">
      <div class="group-heading"><h2>Appearance</h2><p>Set the colors and text size used in the viewer.</p></div>
      <div class="setting-row"><label for="theme">Theme</label><select id="theme" bind:value={settings.theme} onchange={() => void saveAppearance()}><option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option></select></div>
      <div class="setting-row"><label for="primary-color">Primary color</label><div class="color-control"><input id="primary-color" type="color" bind:value={settings.primaryColor} onchange={() => void saveAppearance()} /><code>{settings.primaryColor}</code><div class="color-palette" aria-label="Curated primary colors">{#each primaryColors as color}<button class:active={settings.primaryColor.toLowerCase() === color.value.toLowerCase()} class="color-swatch" type="button" style:background-color={color.value} aria-label={color.name} title={color.name} onclick={() => selectPrimaryColor(color.value)}></button>{/each}</div></div></div>
      <div class="setting-row"><label for="text-size">Text size</label><select id="text-size" bind:value={settings.textScale} onchange={() => void saveAppearance()}><option value={0.85}>Small</option><option value={1}>Default</option><option value={1.15}>Large</option><option value={1.35}>Extra large</option></select></div>
    </section>
    <section class="settings-group">
      <div class="group-heading"><h2>Viewer</h2><p>Control how the status window stays visible.</p></div>
      <label class="checkbox"><input type="checkbox" bind:checked={settings.pinned} onchange={() => void saveAppearance()} /> Keep viewer always on top</label>
    </section>
    <section class="settings-group">
      <div class="group-heading"><h2>Status</h2><p>Choose how project updates are organized and retained.</p></div>
      <div class="setting-row"><label for="project-sort-order">Project note order</label><select id="project-sort-order" bind:value={settings.projectSortOrder} onchange={() => void saveProjectSortOrder()}><option value="project-directory-asc">Project directory A-Z</option></select></div>
      <div class="auto-clear"><div><h3>Auto-clear status items</h3><p>Remove items after their submitted time. This only runs while ctx-ppteer is open.</p></div><label class="checkbox"><input type="checkbox" checked={settings.autoClearAfterMinutes !== null} onchange={(event) => { settings.autoClearAfterMinutes = event.currentTarget.checked ? Math.round(autoClearValue * (autoClearUnit === "hours" ? 60 : 1)) : null; void saveAutoClear(); }} /> Clear old items</label>{#if settings.autoClearAfterMinutes !== null}<div class="auto-clear-controls"><input aria-label="Auto-clear age" type="number" min="1" max={autoClearUnit === "hours" ? 720 : 43200} bind:value={autoClearValue} onchange={() => void saveAutoClear()} /><select aria-label="Auto-clear age unit" bind:value={autoClearUnit} onchange={() => void saveAutoClear()}><option value="minutes">Minutes</option><option value="hours">Hours</option></select></div>{/if}</div>
    </section>
    <section class="integration"><div><h2>Agent integration</h2><p>Install the bundled skill into the locations used by each agent, then configure MCP separately when needed.</p></div>
      {#if skill}
        <div class="skill-targets" aria-label="Skill installation targets">{#each skill.targets as target}<div class="skill-target"><div><span>{target.name}</span><small>{target.path}</small></div><span class:installed={target.installed}>{target.installed ? target.upToDate ? "Skill installed" : "Skill update available" : "Skill not installed"}</span><button onclick={() => void installSkill(target.id)} disabled={installingSkill !== null}>{installingSkill === target.id ? "Installing…" : !target.installed ? "Install skill" : target.upToDate ? "Reinstall" : "Update skill"}</button></div>{/each}</div>
        {#if skill.additionalTargets.length > 0}<details class="additional"><summary>Additional skill directories ({skill.additionalTargets.length})</summary><div class="skill-targets">{#each skill.additionalTargets as target}<div class="skill-target"><div><span>{target.name}</span><small>{target.path}</small></div><span class:installed={target.installed}>{target.installed ? target.upToDate ? "Skill installed" : "Skill update available" : "Skill not installed"}</span><button onclick={() => void installSkill(target.id)} disabled={installingSkill !== null}>{installingSkill === target.id ? "Installing…" : !target.installed ? "Install skill" : target.upToDate ? "Reinstall" : "Update skill"}</button></div>{/each}</div></details>{/if}
        <div class="agent-status" aria-label="Agent integration status">{#each skill.agents as agent}<div class="agent"><div><span>{agent.name}</span><small>{agent.configPath}</small></div><span class:mcp-registered={agent.mcpRegistered}>{agent.mcpRegistered ? "MCP registered" : "MCP not registered"}</span><button onclick={() => void (agent.mcpRegistered ? uninstallMcp(agent.id) : installMcp(agent.id))} disabled={installingMcp !== null}>{installingMcp === agent.id ? "Updating…" : agent.mcpRegistered ? "Uninstall MCP" : "Install MCP"}</button></div>{#if agent.error}<p class="agent-error">{agent.error}</p>{/if}{/each}</div>
        {#if skill.additionalAgents.length > 0}<details class="additional"><summary>Additional MCP configs ({skill.additionalAgents.length})</summary><div class="agent-status">{#each skill.additionalAgents as agent}<div class="agent"><div><span>{agent.name}</span><small>{agent.configPath}</small></div><span class:mcp-registered={agent.mcpRegistered}>{agent.mcpRegistered ? "MCP registered" : "MCP not registered"}</span><button onclick={() => void (agent.mcpRegistered ? uninstallMcp(agent.id) : installMcp(agent.id))} disabled={installingMcp !== null}>{installingMcp === agent.id ? "Updating…" : agent.mcpRegistered ? "Uninstall MCP" : "Install MCP"}</button></div>{#if agent.error}<p class="agent-error">{agent.error}</p>{/if}{/each}</div></details>{/if}
        {#if skill.agents.length === 0}<p class="status">No supported agent harnesses were found.</p>{/if}
      {/if}
      {#if skillError}<p class="error">{skillError}</p>{/if}
    </section>
    <section class="reset settings-group"><div><h2>Reset settings</h2><p>Restore appearance, viewer, and status settings.</p></div><button onclick={() => void resetSettings()} disabled={saving}>Reset settings</button></section>
    {#if saving}<p class="status">Saving…</p>{/if}{#if settingsError}<p class="error">{settingsError}</p>{/if}
  {/if}
</main>

<style>
  button { flex: none; border: 0; border-radius: 0.4rem; padding: 0.5rem 0.7rem; color: #111; background: #f2f2f2; font: inherit; font-size: 0.8rem; font-weight: 600; cursor: pointer; } button:disabled { cursor: wait; opacity: 0.65; }.error { color: #c43d35 !important; }
  .settings-page { display: block; overflow: auto; padding: 28px; }.settings-page header { margin-bottom: 28px; }.settings-page h1, .settings-page h2, .settings-page h3 { margin: 0; }.settings-page h1 { font-size: 1.5rem; }.settings-page h2 { font-size: 1rem; }.settings-page h3 { font-size: 0.9rem; }.settings-page header p, .group-heading p, .integration p, .reset p, .auto-clear p { color: var(--muted); margin: 0.4rem 0 0; }.settings-page section { display: grid; gap: 1rem; margin: 0 0 24px; }.settings-page label { font-weight: 600; }select, input[type="number"] { border: 1px solid var(--border); border-radius: 0.4rem; padding: 0.55rem 0.65rem; color: var(--text); background: var(--bg); font: inherit; }.settings-group, .integration { padding: 1.1rem; border: 1px solid var(--border); border-radius: 0.65rem; }.group-heading { display: grid; gap: 0.1rem; }.setting-row { display: grid; grid-template-columns: minmax(150px, 1fr) minmax(0, 2fr); align-items: center; gap: 1rem; }.setting-row select, .setting-row .color-control { width: 100%; }.color-control { display: flex; flex-wrap: wrap; align-items: center; gap: 0.65rem; }.color-control input { width: 2.4rem; height: 2rem; border: 1px solid var(--border); border-radius: 0.4rem; padding: 0.15rem; background: var(--bg); cursor: pointer; }.color-control code { color: var(--muted); font-size: 0.8rem; }.color-palette { display: flex; flex-wrap: wrap; gap: 0.35rem; width: 100%; }.color-swatch { width: 1.45rem; height: 1.45rem; padding: 0; border: 2px solid transparent; border-radius: 50%; box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.12); }.color-swatch:hover { transform: scale(1.08); }.color-swatch.active { border-color: var(--text); box-shadow: 0 0 0 2px var(--bg), 0 0 0 3px var(--text); }.checkbox { display: flex; align-items: center; gap: 0.55rem; }.auto-clear { display: grid; gap: 0.7rem; padding-top: 1rem; border-top: 1px solid var(--border); }.auto-clear-controls { display: flex; gap: 0.5rem; }.auto-clear-controls input { width: 7rem; }.reset { grid-template-columns: 1fr auto; align-items: center; }.skill-targets, .agent-status { border: 1px solid var(--border); border-radius: 0.4rem; overflow: hidden; }.additional { display: grid; gap: 0.45rem; }.additional summary { color: var(--muted); cursor: pointer; font-size: 0.8rem; }.skill-target + .skill-target, .agent + .agent { border-top: 1px solid var(--border); }.skill-target, .agent { display: grid; grid-template-columns: minmax(130px, 1fr) 1fr auto; align-items: center; gap: 0.7rem; padding: 0.55rem 0.7rem; font-size: 0.8rem; }.skill-target small, .agent small { display: block; overflow: hidden; color: var(--muted); font-size: 0.7rem; text-align: left; text-overflow: ellipsis; white-space: nowrap; }.skill-target > span, .agent > span { color: var(--muted); }.skill-target > span.installed, .agent > span.mcp-registered { color: var(--accent); }.agent-error { margin: 0; color: #c43d35; font-size: 0.8rem; }.status { color: var(--muted); }@media (max-width: 520px) { .settings-page { padding: 20px; }.setting-row, .reset { grid-template-columns: 1fr; gap: 0.6rem; }.reset button { justify-self: start; }.skill-target, .agent { grid-template-columns: 1fr; gap: 0.35rem; } }
</style>
