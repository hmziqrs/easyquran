<script lang="ts">
  import { goto } from "$app/navigation";
  import { Panel } from "$lib/components";
  import { Brand } from "$lib/components/brand";
  import SignInForm from "$lib/auth/components/SignInForm.svelte";
  import OAuthButtons from "$lib/auth/components/OAuthButtons.svelte";
  import { createLoginFlow } from "$lib/auth/flows.svelte";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { getAuthCopy } from "$lib/i18n/auth-copy";
  import { marketingHomeHref } from "$lib/i18n/marketing-copy";

  const flow = createLoginFlow();
  const copy = getAuthCopy();

  async function onsuccess(): Promise<void> {
    const user = authState.user;
    await goto(user && !user.is_verified ? "/verify-email" : "/app");
  }
</script>

<div class="flex flex-col gap-6">
  <!-- Brand band (recon: login had no brand presence). Accent fill + on-accent
       lockup; the form keeps its own heading — no duplicated copy. -->
  <Panel variant="accent" class="flex px-6 py-5">
    <Brand tone="on-accent" homeHref={marketingHomeHref(copy.locale)} />
  </Panel>
  <SignInForm {flow} onsuccess={onsuccess} />
  <OAuthButtons onPasskeySuccess={onsuccess} />
</div>
