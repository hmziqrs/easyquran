<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import AuthForm from "$lib/auth/components/AuthForm.svelte";
  import { clearReturnTarget } from "$lib/auth/return-target";
  import { failureMessage, OAUTH_FAILURE_HEADING } from "./oauth-failure";

  const errorCode = $derived(page.url.searchParams.get("ec"));
  const message = $derived(failureMessage(errorCode));

  onMount(() => {
    clearReturnTarget();
  });

  async function backToLogin(): Promise<void> {
    await goto("/login");
  }
</script>

<AuthForm
  heading={OAUTH_FAILURE_HEADING}
  submitLabel="Back to sign in"
  serverError={message}
  onsubmit={backToLogin}
>
  {#snippet footer()}
    <a href="/app" class="text-accent hover:underline">Continue without an account</a>
  {/snippet}
</AuthForm>
