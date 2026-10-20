<script lang="ts">
  import type { AgentInstallation } from '../shared/types';
  export let agent: AgentInstallation;
  export let busy = false;
  export let onToggle: (id: string, registered: boolean) => void;
</script>

<div class="agent">
  <div><span>{agent.name}</span><small>{agent.configPath}</small></div>
  <span class:mcp-registered={agent.mcpRegistered}
    >{agent.mcpRegistered ? 'MCP registered' : 'MCP not registered'}</span
  ><button onclick={() => onToggle(agent.id, agent.mcpRegistered)} disabled={busy}
    >{busy ? 'Updating…' : agent.mcpRegistered ? 'Uninstall MCP' : 'Install MCP'}</button
  >
</div>
{#if agent.error}<p class="agent-error">{agent.error}</p>{/if}

<style>
  .agent {
    display: grid;
    grid-template-areas: 'details action' 'status action';
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.35rem 0.7rem;
    padding: 0.55rem 0.7rem;
    font-size: 0.8rem;
  }
  .agent > div {
    grid-area: details;
  }
  .agent > span {
    grid-area: status;
    color: var(--muted);
  }
  .agent > span.mcp-registered {
    color: var(--accent);
  }
  .agent > button {
    grid-area: action;
  }
  .agent small {
    display: block;
    overflow: hidden;
    color: var(--muted);
    font-size: 0.7rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .agent-error {
    margin: 0;
    color: #c43d35;
    font-size: 0.8rem;
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
