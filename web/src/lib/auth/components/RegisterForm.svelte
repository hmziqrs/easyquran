<script lang="ts">
  import AuthForm from "./AuthForm.svelte";
  import AuthField from "./AuthField.svelte";
  import PasswordInput from "./PasswordInput.svelte";
  import { Icon } from "$lib/components/icon";
  import type { RegisterFlow } from "$lib/auth/flows.svelte";

  type Props = {
    flow: RegisterFlow;
    variant?: "page" | "modal";
    onsuccess: () => void | Promise<void>;
  };

  let { flow, variant = "page", onsuccess }: Props = $props();

  async function submit(): Promise<void> {
    if (flow.step === "totp") {
      const ok = await flow.login.submitTotp();
      if (ok) await onsuccess();
      return;
    }
    const ok = await flow.submit();
    if (ok) await onsuccess();
  }
</script>

{#if flow.step === "totp"}
  <AuthForm
    heading="Two-factor code"
    subheading="Enter the 6-digit code from your authenticator app to finish signing in."
    submitLabel="Continue"
    pending={flow.login.pending}
    serverError={flow.login.genericError}
    {variant}
    onsubmit={submit}
  >
    <AuthField
      id="register-totp"
      label="Authentication code"
      inputmode="numeric"
      autocomplete="one-time-code"
      maxlength={6}
      bind:value={flow.login.code}
      error={flow.login.fieldErrors.code}
    />
    {#snippet footer()}
      <button type="button" class="text-accent hover:underline" onclick={() => flow.login.cancelTotp()}>
        Use a different account
      </button>
    {/snippet}
  </AuthForm>
{:else}
  <AuthForm
    heading="Create your account"
    subheading="A free account syncs your bookmarks, notes and reading place across devices."
    submitLabel="Create account"
    pending={flow.pending}
    serverError={flow.genericError}
    {variant}
    onsubmit={submit}
  >
    <AuthField
      id="register-name"
      label="Display name"
      autocomplete="name"
      placeholder="Your name"
      bind:value={flow.name}
      error={flow.fieldErrors.name}
    >
      {#snippet leadingIcon()}<Icon name="user" size={16} />{/snippet}
    </AuthField>
    <AuthField
      id="register-email"
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
      id="register-password"
      label="Password"
      autocomplete="new-password"
      minlength={12}
      placeholder="At least 12 characters"
      bind:value={flow.password}
      error={flow.fieldErrors.password}
    />
    <PasswordInput
      id="register-confirm"
      label="Confirm password"
      autocomplete="new-password"
      minlength={12}
      bind:value={flow.confirmPassword}
      error={flow.fieldErrors.confirm_password}
    />
    {#snippet footer()}
      <span>
        Already have an account?
        <a href="/login" class="text-accent hover:underline">Sign in</a>
      </span>
    {/snippet}
  </AuthForm>
{/if}
