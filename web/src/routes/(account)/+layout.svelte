<script lang="ts">
  import { onMount } from "svelte";
  import { Container } from "$lib/components";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { installPurgeHook } from "$lib/auth/purge-hook";

  let { children } = $props();

  onMount(() => {
    installPurgeHook(authState);
    authState.hydrate();
  });
</script>

<Container width="narrow" class="py-16">
  {@render children()}
</Container>
