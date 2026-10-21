<script lang="ts">
  import { onMount } from 'svelte';
  import { command } from '../shared/commands';
  import { saveAppearance, savePinned } from './appearance';
  import { subscribeSettings } from './subscription';
  import { applyPrimaryColor, applyTheme, systemTheme } from '../shared/theme';
  import type { Settings } from '../shared/types';
  import AppearanceSettings from './AppearanceSettings.svelte';
  import RetentionSettings from './RetentionSettings.svelte';
  import AgentIntegrations from '../integrations/AgentIntegrations.svelte';
  let settings: Settings | null = null;
  let saving = false;
  let settingsError = '';
  let integrationsError = '';
  let resetVersion = 0;
  let settingsGeneration = 0;
  const primaryColors = [
    { name: 'Terracotta', value: '#c98c5a' },
    { name: 'Rose', value: '#c96b7b' },
    { name: 'Coral', value: '#d97757' },
    { name: 'Gold', value: '#c59a38' },
    { name: 'Olive', value: '#8b9b50' },
    { name: 'Sage', value: '#5f9b85' },
    { name: 'Teal', value: '#3f9a9a' },
    { name: 'Blue', value: '#5d8fc2' },
    { name: 'Violet', value: '#8a75bc' },
  ];
  function errorText(error: unknown) {
    return error instanceof Error ? error.message : String(error);
  }
  async function saveAppearanceSettings() {
    if (!settings) return;
    saving = true;
    settingsError = '';
    try {
      await saveAppearance(settings);
    } catch (error) {
      settingsError = errorText(error);
    } finally {
      saving = false;
    }
  }
  async function saveSortOrder() {
    if (!settings) return;
    saving = true;
    settingsError = '';
    try {
      await command('set_project_sort_order', { projectSortOrder: settings.projectSortOrder });
    } catch (error) {
      settingsError = errorText(error);
    } finally {
      saving = false;
    }
  }
  async function savePinnedSetting(pinned: boolean) {
    const editGeneration = settingsGeneration;
    saving = true;
    settingsError = '';
    try {
      await savePinned(pinned);
    } catch (error) {
      settingsError = errorText(error);
      if (settings && settingsGeneration === editGeneration && settings.pinned === pinned) {
        settings.pinned = !pinned;
      }
    } finally {
      saving = false;
    }
  }
  async function resetSettings() {
    saving = true;
    settingsError = '';
    try {
      settings = await command('reset_settings');
      resetVersion += 1;
      applyTheme(settings.theme);
      applyPrimaryColor(settings.primaryColor);
    } catch (error) {
      settingsError = errorText(error);
    } finally {
      saving = false;
    }
  }
  function applySettings(value: Settings) {
    settingsGeneration += 1;
    settings = value;
    applyTheme(value.theme);
    applyPrimaryColor(value.primaryColor);
  }
  onMount(() => {
    let disposed = false;
    const onThemeChange = () => {
      if (settings?.theme === 'system') applyTheme(settings.theme);
    };
    systemTheme.addEventListener('change', onThemeChange);
    const subscription = subscribeSettings(
      (value) => {
        if (!disposed) applySettings(value);
      },
      (error) => {
        if (!disposed) settingsError = errorText(error);
      },
      () => command('get_settings'),
    );
    return () => {
      disposed = true;
      systemTheme.removeEventListener('change', onThemeChange);
      subscription.dispose();
    };
  });
</script>

<main class="settings-page">
  <header>
    <h1>Settings</h1>
    <p>Choose what ctx-ppteer shows and how it appears.</p>
  </header>
  {#if settings}
    <AppearanceSettings {settings} {primaryColors} onSave={() => void saveAppearanceSettings()} />
    <section class="settings-group">
      <div class="group-heading">
        <h2>Viewer</h2>
        <p>Keep the viewer above other windows.</p>
      </div>
      <label class="checkbox"
        ><input
          type="checkbox"
          bind:checked={settings.pinned}
          onchange={() => void savePinnedSetting(settings!.pinned)}
        /> Keep viewer always on top</label
      >
    </section>
    <RetentionSettings
      {settings}
      {saving}
      {resetVersion}
      onSaved={(minutes) => {
        if (settings) settings.autoClearAfterMinutes = minutes;
      }}
      onError={(error) => (settingsError = errorText(error))}
      onSortChange={() => void saveSortOrder()}
    />
    <AgentIntegrations onError={(error) => (integrationsError = errorText(error))} />
    {#if integrationsError}<p class="error">{integrationsError}</p>{/if}
    <section class="reset settings-group">
      <div>
        <h2>Reset settings</h2>
        <p>Restore appearance, viewer, and status settings.</p>
      </div>
      <button onclick={() => void resetSettings()} disabled={saving}>Reset settings</button>
    </section>
    {#if saving}<p class="status">Saving…</p>{/if}{#if settingsError}<p class="error">
        {settingsError}
      </p>{/if}
  {/if}
</main>

<style>
  .settings-page {
    display: block;
    overflow: auto;
    padding: 28px;
  }
  .settings-page header {
    margin-bottom: 28px;
  }
  .settings-page h1,
  .settings-page h2 {
    margin: 0;
  }
  .settings-page h1 {
    font-size: 1.5rem;
  }
  .settings-page h2 {
    font-size: 1rem;
  }
  .settings-page header p,
  .reset p {
    color: var(--muted);
    margin: 0.4rem 0 0;
  }
  .settings-group {
    display: grid;
    gap: 1rem;
    margin: 0 0 24px;
    padding: 1.1rem;
    border: 1px solid var(--border);
    border-radius: 0.65rem;
  }
  .group-heading {
    display: grid;
    gap: 0.1rem;
  }
  .group-heading p {
    color: var(--muted);
    margin: 0.4rem 0 0;
  }
  .checkbox {
    display: flex;
    align-items: center;
    gap: 0.55rem;
  }
  .reset {
    grid-template-columns: 1fr auto;
    align-items: center;
  }
  .reset button,
  button {
    border: 0;
    border-radius: 0.4rem;
    padding: 0.5rem 0.7rem;
    color: #111;
    background: #f2f2f2;
    font: inherit;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
  }
  .reset button:disabled {
    cursor: wait;
    opacity: 0.65;
  }
  .error {
    color: #c43d35;
  }
  .status {
    color: var(--muted);
  }
  @media (max-width: 520px) {
    .settings-page {
      padding: 20px;
    }
    .reset {
      grid-template-columns: 1fr;
      gap: 0.6rem;
    }
  }
</style>
