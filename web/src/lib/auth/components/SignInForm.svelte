<script lang="ts">
  import AuthForm from "./AuthForm.svelte";
  import AuthField from "./AuthField.svelte";
  import PasswordInput from "./PasswordInput.svelte";
  import { Icon } from "$lib/components/icon";
  import type { LoginFlow } from "$lib/auth/flows.svelte";

  type Props = {
    flow: LoginFlow;
    variant?: "page" | "modal";
    onsuccess: () => void | Promise<void>;
  };

  let { flow, variant = "page", onsuccess }: Props = $props();

  async function submit(): Promise<void> {
    const ok =
      flow.step === "totp" ? await flow.submitTotp() : await flow.submitCredentials();
    if (ok) await onsuccess();
  }
</script>

{#if flow.step === "totp"}
  <AuthForm
    heading="Two-factor code"
    subheading="Enter the 6-digit code from your authenticator app to finish signing in."
    submitLabel="Continue"
    pending={flow.pending}
    serverError={flow.genericError}
    {variant}
    onsubmit={submit}
  >
    <AuthField
      id="signin-totp"
      label="Authentication code"
      inputmode="numeric"
      autocomplete="one-time-code"
      maxlength={6}
      bind:value={flow.code}
      error={flow.fieldErrors.code}
    />
    {#snippet footer()}
      <button type="button" class="text-accent hover:underline" onclick={() => flow.cancelTotp()}>
        Use a different account
      </button>
    {/snippet}
  </AuthForm>
{:else}
  <AuthForm
    heading="Sign in"
    subheading="Welcome back. Sign in to sync your bookmarks and reading progress."
    submitLabel="Sign in"
    pending={flow.pending}
    serverError={flow.genericError}
    {variant}
    onsubmit={submit}
  >
    <AuthField
      id="signin-email"
      label="Email"
      type="email"
      autocomplete="email"
      placeholder="you@example.com"
      bind:value={flow.email}
      error={flow.fieldErrors.email}
    >
      {#snippet leadingIcon()}<Icon name="mail" size={16} />{/snippet}
    </AuthField>
    <PasswordInput
      id="signin-password"
      label="Password"
      autocomplete="current-password"
      placeholder="Your password"
      bind:value={flow.password}
      error={flow.fieldErrors.password}
    />
    {#snippet footer()}
      <span>
        Forgot your password?
        <a href="/forgot-password" class="text-accent hover:underline">Reset it</a>
      </span>
      <span>
        No account?
        <a href="/register" class="text-accent hover:underline">Create one</a>
      </span>
    {/snippet}
  </AuthForm>
{/if}
