<script lang="ts">
  import Router from "svelte-spa-router";
  import Settings from "./Settings.svelte";
  import Viewer from "./Viewer.svelte";

  const routes = {
    "/": Viewer,
    "/settings": Settings,
  };
</script>

<Router {routes} />
