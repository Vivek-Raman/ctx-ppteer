<script lang="ts">
  import Router from 'svelte-spa-router';
  import Settings from './settings/Settings.svelte';
  import Viewer from './viewer/Viewer.svelte';

  const routes = {
    '/': Viewer,
    '/settings': Settings,
  };
</script>

<Router {routes} />
