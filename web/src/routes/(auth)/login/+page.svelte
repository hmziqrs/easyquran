<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import SignInForm from "#lib/auth/components/SignInForm.svelte";
  import OAuthButtons from "#lib/auth/components/OAuthButtons.svelte";
  import { createLoginFlow } from "#lib/auth/flows.svelte.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import { postAuthPath } from "#lib/auth/post-auth-path.js";
  import {
    appPathFromReferrer,
    consumeReturnTarget,
    isInternalPath,
    setReturnTarget,
  } from "#lib/auth/return-target.js";

  const flow = createLoginFlow();

  onMount(() => {
    // Remember where a successful sign-in should land: an explicit ?returnTo
    // wins, otherwise the in-app page that linked here. Invalid returnTo values
    // are ignored entirely rather than falling back to the referrer.
    const explicit = page.url.searchParams.get("returnTo");
    if (explicit !== null) {
      if (isInternalPath(explicit)) setReturnTarget(explicit);
      return;
    }
    const fromReferrer = appPathFromReferrer(document.referrer);
    if (fromReferrer) setReturnTarget(fromReferrer);
  });

  async function onsuccess(): Promise<void> {
    const user = authState.user;
    const target = consumeReturnTarget();
    await goto(postAuthPath(user, target));
  }
</script>

<div class="flex flex-col gap-6">
  <SignInForm {flow} onsuccess={onsuccess} />
  <OAuthButtons onPasskeySuccess={onsuccess} />
</div>
