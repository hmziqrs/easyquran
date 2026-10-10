<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import AuthForm from "#lib/auth/components/AuthForm.svelte";
  import { isOAuthProvider } from "#lib/auth/oauth-flow.svelte.js";
  import { notifyOAuthPopupOpener } from "#lib/auth/oauth-popup.js";
  import { clearReturnTarget } from "#lib/auth/return-target.js";
  import { failureMessage, OAUTH_FAILURE_HEADING } from "./oauth-failure";

  const errorCode = $derived(page.url.searchParams.get("ec"));
  const message = $derived(failureMessage(errorCode));

  onMount(() => {
    clearReturnTarget();
    const provider = page.params.provider;
    // Popup flow: tell the opener the attempt failed, then close; the opener
    // falls back to the full-page redirect flow.
    if (provider && isOAuthProvider(provider)) {
      notifyOAuthPopupOpener(provider, "failure");
    }
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
    <a href="/surah" class="text-accent hover:underline">Continue without an account</a>
  {/snippet}
</AuthForm>
