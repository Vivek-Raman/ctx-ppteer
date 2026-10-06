<script lang="ts">
  import { onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { command } from "./commands";
  import { applyTheme, systemTheme } from "./theme";
  import type { Settings, SkillInstallation } from "./app-types";

  let settings: Settings | null = null;
  let skill: SkillInstallation | null = null;
  let installingSkill: string | null = null;
  let installingMcp: string | null = null;
  let skillError = "";
  let saving = false;
  let settingsError = "";

  async function chooseFile() { const path = await command<string | null>("pick_source"); if (path) await command("set_source", { path }); }
  async function useDefaultFile() { settings = await command<Settings>("use_default_source"); }
  async function saveAppearance() {
    if (!settings) return;
    saving = true; settingsError = "";
    try { await command("set_appearance_settings", { theme: settings.theme, textScale: Number(settings.textScale) }); await command("set_pinned", { pinned: settings.pinned }); }
    catch (error) { settingsError = error instanceof Error ? error.message : String(error); }
    finally { saving = false; }
  }
  async function saveProjectSortOrder() {
    if (!settings) return;
    saving = true; settingsError = "";
    try { await command("set_project_sort_order", { projectSortOrder: settings.projectSortOrder }); }
    catch (error) { settingsError = error instanceof Error ? error.message : String(error); }
    finally { saving = false; }
  }
  async function saveDoubleClickToEdit() {
    if (!settings) return;
    saving = true; settingsError = "";
    try { await command("set_double_click_to_edit", { doubleClickToEdit: settings.doubleClickToEdit }); }
    catch (error) { settingsError = error instanceof Error ? error.message : String(error); }
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
    void listen<Settings>("settings-update", (event) => { settings = event.payload; applyTheme(settings.theme); }).then((stop) => { unlisten = stop; });
    void (async () => { settings = await command<Settings>("get_settings"); applyTheme(settings.theme); skill = await command<SkillInstallation>("get_skill_installation"); })().catch((error) => { skillError = error instanceof Error ? error.message : String(error); });
    return () => { systemTheme.removeEventListener("change", onThemeChange); unlisten?.(); };
  });
</script>

<main class="settings-page">
  <header><h1>Settings</h1><p>Choose what ctx-ppteer shows and how it appears.</p></header>
  {#if settings}
    <section><label for="source">Markdown file</label><div class="file-picker"><input id="source" value={settings.sourcePath} readonly title={settings.sourcePath} /><button onclick={() => void chooseFile()}>Choose…</button></div><div class="source-actions"><button class="link-button" onclick={() => void useDefaultFile()}>Use default file</button><button class="link-button" onclick={() => void command("reveal_source")}>Reveal in file manager</button></div></section>
    <section class="appearance"><label for="theme">Theme</label><select id="theme" bind:value={settings.theme} onchange={() => void saveAppearance()}><option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option></select><label for="text-size">Text size</label><select id="text-size" bind:value={settings.textScale} onchange={() => void saveAppearance()}><option value={0.85}>Small</option><option value={1}>Default</option><option value={1.15}>Large</option><option value={1.35}>Extra large</option></select></section>
    <section><label for="project-sort-order">Project note order</label><select id="project-sort-order" bind:value={settings.projectSortOrder} onchange={() => void saveProjectSortOrder()}><option value="project-directory-asc">Project directory A-Z</option></select></section>
    <section><label class="checkbox"><input type="checkbox" bind:checked={settings.pinned} onchange={() => void saveAppearance()} /> Keep viewer always on top</label><label class="checkbox"><input type="checkbox" bind:checked={settings.doubleClickToEdit} onchange={() => void saveDoubleClickToEdit()} /> Double-click text to edit</label></section>
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
    {#if saving}<p class="status">Saving…</p>{/if}{#if settingsError}<p class="error">{settingsError}</p>{/if}
  {/if}
</main>

<style>
  button { flex: none; border: 0; border-radius: 0.4rem; padding: 0.5rem 0.7rem; color: #111; background: #f2f2f2; font: inherit; font-size: 0.8rem; font-weight: 600; cursor: pointer; } button:disabled { cursor: wait; opacity: 0.65; }.error { color: #c43d35 !important; }
  .settings-page { display: block; overflow: auto; padding: 28px; }.settings-page header { margin-bottom: 28px; }.settings-page h1 { margin: 0; font-size: 1.5rem; }.settings-page header p, .integration p { color: var(--muted); margin: 0.4rem 0 0; }.settings-page section { display: grid; gap: 0.6rem; margin: 0 0 24px; }.settings-page label { font-weight: 600; }.file-picker { display: flex; gap: 0.6rem; }.file-picker input, select { border: 1px solid var(--border); border-radius: 0.4rem; padding: 0.55rem 0.65rem; color: var(--text); background: var(--bg); font: inherit; }.file-picker input { flex: 1; min-width: 0; }.source-actions { display: flex; gap: 0.8rem; }.link-button { justify-self: start; padding: 0; color: var(--accent); background: transparent; font-weight: 500; }.appearance { grid-template-columns: 1fr 1fr; align-items: center; }.appearance select { grid-column: 2; }.checkbox { display: flex; align-items: center; gap: 0.55rem; }.integration { padding-top: 1rem; border-top: 1px solid var(--border); }.integration h2 { margin: 0; font-size: 1rem; }.skill-targets, .agent-status { border: 1px solid var(--border); border-radius: 0.4rem; overflow: hidden; }.additional { display: grid; gap: 0.45rem; }.additional summary { color: var(--muted); cursor: pointer; font-size: 0.8rem; }.skill-target + .skill-target, .agent + .agent { border-top: 1px solid var(--border); }.skill-target, .agent { display: grid; grid-template-columns: minmax(130px, 1fr) 1fr auto; align-items: center; gap: 0.7rem; padding: 0.55rem 0.7rem; font-size: 0.8rem; }.skill-target small, .agent small { display: block; overflow: hidden; color: var(--muted); font-size: 0.7rem; text-align: left; text-overflow: ellipsis; white-space: nowrap; }.skill-target > span, .agent > span { color: var(--muted); }.skill-target > span.installed, .agent > span.mcp-registered { color: var(--accent); }.agent-error { margin: 0; color: #c43d35; font-size: 0.8rem; }.status { color: var(--muted); }
</style>
