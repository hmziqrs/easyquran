<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { authClient } from "#lib/auth/auth-client.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import { createLoginFlow } from "#lib/auth/flows.svelte.js";
  import { isOAuthProvider } from "#lib/auth/oauth-flow.svelte.js";
  import SignInForm from "#lib/auth/components/SignInForm.svelte";
  import { getAuthCopy } from "#lib/i18n/auth-copy.js";
  import { pendingTotpToken } from "./pending-proof";

  const flow = createLoginFlow();
  const copy = getAuthCopy();
  let ready = $state(false);
  let failed = $state(false);

  async function loadPendingProof(): Promise<void> {
    failed = false;
    try {
      const provider = page.params.provider;
      if (!provider || (provider !== "passkey" && !isOAuthProvider(provider))) {
        await goto("/login");
        return;
      }
      const response = await authClient.get<unknown>("/auth/v1/login/totp/pending");
      if (!response.ok) {
        failed = true;
        return;
      }
      const token = pendingTotpToken(response.data);
      if (token === null) {
        await goto("/login");
        return;
      }
      flow.adoptTotpToken(token);
      authState.setTwoFaPending(true);
      ready = true;
    } catch {
      failed = true;
    }
  }

  async function complete(): Promise<void> {
    const provider = page.params.provider;
    if (!provider || (provider !== "passkey" && !isOAuthProvider(provider))) {
      await goto("/login");
      return;
    }
    if (provider === "passkey") {
      await goto("/");
      return;
    }
    await goto(`/auth/${provider}/success`);
  }

  onMount(() => {
    void loadPendingProof();
  });
</script>

<svelte:head>
  <title>{copy.totpHeading}</title>
</svelte:head>

{#if ready}
  <SignInForm {flow} onsuccess={complete} />
{:else if failed}
  <div role="status" class="flex flex-col gap-3 text-center">
    <p>Sign-in couldn't continue. Try again.</p>
    <button type="button" class="text-primary hover:underline" onclick={loadPendingProof}>
      Try again
    </button>
  </div>
{:else}
  <p role="status" class="text-center">Finishing your sign-in…</p>
{/if}
