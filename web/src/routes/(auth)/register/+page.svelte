<script lang="ts">
  import { goto } from "$app/navigation";
  import RegisterForm from "#lib/auth/components/RegisterForm.svelte";
  import OAuthButtons from "#lib/auth/components/OAuthButtons.svelte";
  import { createRegisterFlow } from "#lib/auth/flows.svelte.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";

  const flow = createRegisterFlow();

  async function onsuccess(): Promise<void> {
    const user = authState.user;
    await goto(user && !user.is_verified ? "/verify-email" : "/surah");
  }
</script>

<div class="flex flex-col gap-6">
  <RegisterForm {flow} onsuccess={onsuccess} />
  <OAuthButtons onPasskeySuccess={onsuccess} />
</div>
