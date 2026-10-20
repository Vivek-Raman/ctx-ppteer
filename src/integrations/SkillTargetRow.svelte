<script lang="ts">
  import type { SkillTarget } from '../shared/types';
  export let target: SkillTarget;
  export let busy = false;
  export let onInstall: (id: string) => void;
</script>

<div class="skill-target">
  <div><span>{target.name}</span><small>{target.path}</small></div>
  <span class:installed={target.installed}
    >{target.installed
      ? target.upToDate
        ? 'Skill installed'
        : 'Skill update available'
      : 'Skill not installed'}</span
  ><button onclick={() => onInstall(target.id)} disabled={busy}
    >{busy
      ? 'Installing…'
      : !target.installed
        ? 'Install skill'
        : target.upToDate
          ? 'Reinstall'
          : 'Update skill'}</button
  >
</div>

<style>
  .skill-target {
    display: grid;
    grid-template-areas: 'details action' 'status action';
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.35rem 0.7rem;
    padding: 0.55rem 0.7rem;
    font-size: 0.8rem;
  }
  .skill-target > div {
    grid-area: details;
  }
  .skill-target > span {
    grid-area: status;
    color: var(--muted);
  }
  .skill-target > span.installed {
    color: var(--accent);
  }
  .skill-target > button {
    grid-area: action;
  }
  .skill-target small {
    display: block;
    overflow: hidden;
    color: var(--muted);
    font-size: 0.7rem;
    text-overflow: ellipsis;
    white-space: nowrap;
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
    cursor: pointer;
  }
  button:disabled {
    cursor: wait;
    opacity: 0.65;
  }
</style>
