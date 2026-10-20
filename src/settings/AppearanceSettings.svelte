<script lang="ts">
  import type { Settings } from '../shared/types';
  export let settings: Settings;
  export let primaryColors: { name: string; value: string }[] = [];
  export let onSave: () => void;
</script>

<section class="settings-group appearance">
  <div class="group-heading">
    <h2>Appearance</h2>
    <p>Set the colors and text size used in the viewer.</p>
  </div>
  <div class="setting-row">
    <label for="theme">Theme</label><select id="theme" bind:value={settings.theme} onchange={onSave}
      ><option value="system">System</option><option value="light">Light</option><option
        value="dark">Dark</option
      ></select
    >
  </div>
  <div class="setting-row">
    <label for="primary-color">Primary color</label>
    <div class="color-control">
      <input
        id="primary-color"
        type="color"
        bind:value={settings.primaryColor}
        onchange={onSave}
      /><code>{settings.primaryColor}</code>
      <div class="color-palette" aria-label="Curated primary colors">
        {#each primaryColors as color}<button
            class:active={settings.primaryColor.toLowerCase() === color.value.toLowerCase()}
            class="color-swatch"
            type="button"
            style:background-color={color.value}
            aria-label={color.name}
            title={color.name}
            onclick={() => {
              settings.primaryColor = color.value;
              onSave();
            }}
          ></button>{/each}
      </div>
    </div>
  </div>
  <div class="setting-row">
    <label for="text-size">Text size</label><select
      id="text-size"
      bind:value={settings.textScale}
      onchange={onSave}
      ><option value={0.85}>Small</option><option value={1}>Default</option><option value={1.15}
        >Large</option
      ><option value={1.35}>Extra large</option></select
    >
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
  .settings-group h2 {
    margin: 0;
    font-size: 1rem;
  }
  .group-heading {
    display: grid;
    gap: 0.1rem;
  }
  .group-heading p {
    color: var(--muted);
    margin: 0.4rem 0 0;
  }
  .setting-row {
    display: grid;
    grid-template-columns: minmax(150px, 1fr) minmax(0, 2fr);
    align-items: center;
    gap: 1rem;
  }
  .setting-row select,
  .setting-row .color-control {
    width: 100%;
  }
  .setting-row label {
    font-weight: 600;
  }
  select {
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    padding: 0.55rem 0.65rem;
    color: var(--text);
    background: var(--bg);
    font: inherit;
  }
  .color-control {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.65rem;
  }
  .color-control input {
    width: 2.4rem;
    height: 2rem;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    padding: 0.15rem;
    background: var(--bg);
    cursor: pointer;
  }
  .color-control code {
    color: var(--muted);
    font-size: 0.8rem;
  }
  .color-palette {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
    width: 100%;
  }
  .color-swatch {
    width: 1.45rem;
    height: 1.45rem;
    padding: 0;
    border: 2px solid transparent;
    border-radius: 50%;
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.12);
  }
  .color-swatch:hover {
    transform: scale(1.08);
  }
  .color-swatch.active {
    border-color: var(--text);
    box-shadow:
      0 0 0 2px var(--bg),
      0 0 0 3px var(--text);
  }
  @media (max-width: 520px) {
    .setting-row {
      grid-template-columns: 1fr;
      gap: 0.6rem;
    }
  }
</style>
