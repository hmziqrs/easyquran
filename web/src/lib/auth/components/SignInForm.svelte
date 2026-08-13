<script lang="ts">
  import AuthForm from "./AuthForm.svelte";
  import AuthField from "./AuthField.svelte";
  import PasswordInput from "./PasswordInput.svelte";
  import { Icon } from "$lib/components/icon";
  import type { LoginFlow } from "$lib/auth/flows.svelte";
  import { getAuthCopy } from "$lib/i18n/auth-copy";

  const copy = getAuthCopy();

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
    heading={copy.totpHeading}
    subheading={copy.totpSubheading}
    submitLabel={copy.continue}
    pending={flow.pending}
    serverError={flow.genericError}
    {variant}
    onsubmit={submit}
  >
    <AuthField
      id="signin-totp"
      label={copy.fieldAuthenticationCode}
      inputmode="numeric"
      autocomplete="one-time-code"
      maxlength={6}
      bind:value={flow.code}
      error={flow.fieldErrors.code}
    />
    {#snippet footer()}
      <button type="button" class="text-accent hover:underline" onclick={() => flow.cancelTotp()}>
        {copy.useDifferentAccount}
      </button>
    {/snippet}
  </AuthForm>
{:else}
  <AuthForm
    heading={copy.signIn}
    subheading={copy.signInSubheading}
    submitLabel={copy.signIn}
    pending={flow.pending}
    serverError={flow.genericError}
    {variant}
    onsubmit={submit}
  >
    <AuthField
      id="signin-email"
      label={copy.fieldEmail}
      type="email"
      autocomplete="email"
      placeholder={copy.emailPlaceholder}
      bind:value={flow.email}
      error={flow.fieldErrors.email}
    >
      {#snippet leadingIcon()}<Icon name="mail" size={16} />{/snippet}
    </AuthField>
    <PasswordInput
      id="signin-password"
      label={copy.fieldPassword}
      autocomplete="current-password"
      placeholder={copy.passwordPlaceholder}
      bind:value={flow.password}
      error={flow.fieldErrors.password}
    />
    {#snippet footer()}
      <span>
        {copy.forgotPasswordPrompt}
        <a href="/forgot-password" class="text-accent hover:underline">{copy.resetPasswordLink}</a>
      </span>
      <span>
        {copy.noAccountPrompt}
        <a href="/register" class="text-accent hover:underline">{copy.createOneLink}</a>
      </span>
    {/snippet}
  </AuthForm>
{/if}
