<script lang="ts">
  import { onMount } from "svelte";
  import { Container } from "$lib/components";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { installPurgeHook } from "$lib/auth/purge-hook";

  let { children } = $props();

  onMount(() => {
    installPurgeHook(authState);
    authState.hydrate({ force: true });
  });
</script>

<Container width="narrow" class="flex min-h-[70vh] flex-col justify-center py-16">
  <main id="main" class="mx-auto w-full max-w-[420px]">{@render children()}</main>
</Container>
