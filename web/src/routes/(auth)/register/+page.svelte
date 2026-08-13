<script lang="ts">
  import { goto } from "$app/navigation";
  import RegisterForm from "$lib/auth/components/RegisterForm.svelte";
  import OAuthButtons from "$lib/auth/components/OAuthButtons.svelte";
  import { createRegisterFlow } from "$lib/auth/flows.svelte";
  import { authState } from "$lib/auth/auth-state.svelte";

  const flow = createRegisterFlow();

  async function onsuccess(): Promise<void> {
    const user = authState.user;
    await goto(user && !user.is_verified ? "/verify-email" : "/app");
  }
</script>

<div class="flex flex-col gap-6">
  <RegisterForm {flow} onsuccess={onsuccess} />
  <OAuthButtons onPasskeySuccess={onsuccess} />
</div>
