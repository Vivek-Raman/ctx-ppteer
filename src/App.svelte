<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Settings from "./Settings.svelte";
  import Viewer from "./Viewer.svelte";

  const isSettings = getCurrentWindow().label === "settings";
</script>

{#if isSettings}
  <Settings />
{:else}
  <Viewer />
{/if}
