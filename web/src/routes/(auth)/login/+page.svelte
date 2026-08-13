<script lang="ts">
  import { goto } from "$app/navigation";
  import SignInForm from "$lib/auth/components/SignInForm.svelte";
  import OAuthButtons from "$lib/auth/components/OAuthButtons.svelte";
  import { createLoginFlow } from "$lib/auth/flows.svelte";
  import { authState } from "$lib/auth/auth-state.svelte";

  const flow = createLoginFlow();

  async function onsuccess(): Promise<void> {
    const user = authState.user;
    await goto(user && !user.is_verified ? "/verify-email" : "/app");
  }
</script>

<div class="flex flex-col gap-6">
  <SignInForm {flow} onsuccess={onsuccess} />
  <OAuthButtons onPasskeySuccess={onsuccess} />
</div>
