<script lang="ts">
  import { command } from '../shared/commands';
  import {
    beginRetentionSave,
    completeRetentionSave,
    editRetentionDraft,
    reconcileRetentionDraft,
    retentionDraft,
    type RetentionDraft,
  } from './retention';
  import type { Settings } from '../shared/types';
  export let settings: Settings;
  export let saving = false;
  export let onSaved: (minutes: number | null) => void;
  export let onError: (error: unknown) => void;
  export let onSortChange: () => void;
  export let resetVersion = 0;
  let draft: RetentionDraft = retentionDraft(settings.autoClearAfterMinutes);
  let lastResetVersion = resetVersion;
  let request = 0;
  let savingRequest = false;
  let observedBaseline = settings.autoClearAfterMinutes;
  $: if (settings.autoClearAfterMinutes !== observedBaseline) {
    observedBaseline = settings.autoClearAfterMinutes;
    draft = reconcileRetentionDraft(draft, observedBaseline);
  }
  $: if (resetVersion !== lastResetVersion) {
    lastResetVersion = resetVersion;
    request += 1;
    savingRequest = false;
    draft = retentionDraft(settings.autoClearAfterMinutes);
    observedBaseline = settings.autoClearAfterMinutes;
  }
  function updateDraft() {
    draft = editRetentionDraft(draft, {});
    if (!draft.confirmation) void save();
  }
  async function save() {
    if (saving || savingRequest || !draft.dirty) return;
    const current = ++request;
    const saveSnapshot = beginRetentionSave(draft, current);
    savingRequest = true;
    try {
      await command('set_auto_clear_after_minutes', {
        autoClearAfterMinutes: saveSnapshot.minutes,
      });
      if (current !== request) return;
      draft = completeRetentionSave(draft, saveSnapshot, request);
      onSaved(saveSnapshot.minutes);
    } catch (error) {
      if (current === request) onError(error);
    } finally {
      if (current === request) savingRequest = false;
    }
  }
  function toggle(enabled: boolean) {
    draft = editRetentionDraft(draft, { enabled });
    updateDraft();
  }
</script>

<section class="settings-group">
  <div class="group-heading">
    <h2>Status</h2>
    <p>Choose how project updates are organized and retained.</p>
  </div>
  <div class="setting-row">
    <label for="project-sort-order">Project note order</label><select
      id="project-sort-order"
      bind:value={settings.projectSortOrder}
      onchange={onSortChange}
      ><option value="updated-at-desc">Newest first</option><option value="updated-at-asc"
        >Oldest first</option
      ><option value="project-directory-asc">Project directory A-Z</option></select
    >
  </div>
  <div class="auto-clear">
    <div>
      <h3>Auto-clear status items</h3>
      <p>Remove items after their submitted time. This only runs while ctx-ppteer is open.</p>
    </div>
    <label class="checkbox"
      ><input
        type="checkbox"
        checked={draft.enabled}
        disabled={saving || savingRequest}
        onchange={(event) => toggle(event.currentTarget.checked)}
      /> Clear old items</label
    >{#if draft.enabled}<div class="auto-clear-controls">
        <input
          aria-label="Auto-clear age"
          type="number"
          min="1"
          max={draft.unit === 'hours' ? 720 : 43200}
          bind:value={draft.value}
          disabled={saving || savingRequest}
          onchange={updateDraft}
        /><select
          aria-label="Auto-clear age unit"
          bind:value={draft.unit}
          onchange={updateDraft}
          disabled={saving || savingRequest}
          ><option value="minutes">Minutes</option><option value="hours">Hours</option></select
        >
      </div>
      {#if draft.confirmation}<button
          class="confirm-auto-clear"
          onclick={() => void save()}
          disabled={saving || savingRequest}>Apply shorter retention</button
        >{/if}{/if}
  </div>
</section>

<style>
  .settings-group {
    display: grid;
    gap: 1rem;
    margin: 0 0 24px;
    padding: 1.1rem;
    border: 1px solid var(--border);
    border-radius: 0.65rem;
  }
  .settings-group h2,
  .settings-group h3 {
    margin: 0;
  }
  .settings-group h2 {
    font-size: 1rem;
  }
  .settings-group h3 {
    font-size: 0.9rem;
  }
  .group-heading p,
  .auto-clear p {
    color: var(--muted);
    margin: 0.4rem 0 0;
  }
  .setting-row {
    display: grid;
    grid-template-columns: minmax(150px, 1fr) minmax(0, 2fr);
    align-items: center;
    gap: 1rem;
  }
  .setting-row label {
    font-weight: 600;
  }
  select,
  input[type='number'] {
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    padding: 0.55rem 0.65rem;
    color: var(--text);
    background: var(--bg);
    font: inherit;
  }
  .auto-clear {
    display: grid;
    gap: 0.7rem;
    padding-top: 1rem;
    border-top: 1px solid var(--border);
  }
  .checkbox {
    display: flex;
    align-items: center;
    gap: 0.55rem;
  }
  .auto-clear-controls {
    display: flex;
    gap: 0.5rem;
  }
  .auto-clear-controls input {
    width: 7rem;
  }
  button {
    border: 0;
    border-radius: 0.4rem;
    padding: 0.5rem 0.7rem;
    color: #111;
    background: #f2f2f2;
    font: inherit;
    font-size: 0.8rem;
    font-weight: 600;
  }
  @media (max-width: 520px) {
    .setting-row {
      grid-template-columns: 1fr;
      gap: 0.6rem;
    }
  }
</style>
