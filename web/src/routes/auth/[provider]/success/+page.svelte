<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { createOAuthFlow, isOAuthProvider } from "$lib/auth/oauth-flow.svelte";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { successDestination } from "./success-destination";

  let status = $state<"working" | "error">("working");
  let started = false;

  onMount(() => {
    if (started) return;
    started = true;
    void complete();
  });

  async function complete(): Promise<void> {
    try {
      const provider = page.params.provider;
      if (!isOAuthProvider(provider)) {
        await goto("/login");
        return;
      }
      const flow = createOAuthFlow(provider);
      const result = await flow.finish();
      if (result.ok) {
        const dest = successDestination(result.returnTarget, authState.user);
        await goto(dest);
        return;
      }
      const code = result.errorCode ?? "oauth_failed";
      await goto(`/auth/${provider}/failure?ec=${encodeURIComponent(code)}`);
    } catch {
      status = "error";
    }
  }

  async function retry(): Promise<void> {
    status = "working";
    await complete();
  }
</script>

<svelte:head>
  <title>Signing you in…</title>
</svelte:head>

<div class="flex flex-col items-center gap-4 text-center" role="status" aria-live="polite">
  {#if status === "working"}
    <span
      class="h-8 w-8 animate-spin rounded-full border-2 border-line-2 border-t-accent"
      aria-hidden="true"></span>
    <div class="flex flex-col gap-1">
      <h1 class="text-xl font-semibold">Signing you in…</h1>
      <p class="text-sm text-fg-2">Finishing your sign-in. You'll be redirected shortly.</p>
    </div>
  {:else}
    <div class="flex flex-col gap-3">
      <h1 class="text-xl font-semibold">Sign-in didn't finish</h1>
      <button type="button" class="text-accent hover:underline" onclick={retry}>Try again</button>
    </div>
  {/if}
</div>
