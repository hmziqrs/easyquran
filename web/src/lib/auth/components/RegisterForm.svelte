<script lang="ts">
  import AuthForm from "./AuthForm.svelte";
  import AuthField from "./AuthField.svelte";
  import PasswordInput from "./PasswordInput.svelte";
  import { Icon } from "$lib/components/icon";
  import type { RegisterFlow } from "$lib/auth/flows.svelte";
  import { getAuthCopy } from "$lib/i18n/auth-copy";

  const copy = getAuthCopy();

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
    heading={copy.totpHeading}
    subheading={copy.totpSubheading}
    submitLabel={copy.continue}
    pending={flow.login.pending}
    serverError={flow.login.genericError}
    {variant}
    onsubmit={submit}
  >
    <AuthField
      id="register-totp"
      label={copy.fieldAuthenticationCode}
      inputmode="numeric"
      autocomplete="one-time-code"
      maxlength={6}
      bind:value={flow.login.code}
      error={flow.login.fieldErrors.code}
    />
    {#snippet footer()}
      <button type="button" class="text-accent hover:underline" onclick={() => flow.login.cancelTotp()}>
        {copy.useDifferentAccount}
      </button>
    {/snippet}
  </AuthForm>
{:else}
  <AuthForm
    heading={copy.registerHeading}
    subheading={copy.registerSubheading}
    submitLabel={copy.createAccount}
    pending={flow.pending}
    serverError={flow.genericError}
    {variant}
    onsubmit={submit}
  >
    <AuthField
      id="register-name"
      label={copy.fieldDisplayName}
      autocomplete="name"
      placeholder={copy.displayNamePlaceholder}
      bind:value={flow.name}
      error={flow.fieldErrors.name}
    >
      {#snippet leadingIcon()}<Icon name="user" size={16} />{/snippet}
    </AuthField>
    <AuthField
      id="register-email"
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
      id="register-password"
      label={copy.fieldPassword}
      autocomplete="new-password"
      minlength={12}
      placeholder={copy.passwordMinPlaceholder}
      bind:value={flow.password}
      error={flow.fieldErrors.password}
    />
    <PasswordInput
      id="register-confirm"
      label={copy.fieldConfirmPassword}
      autocomplete="new-password"
      minlength={12}
      bind:value={flow.confirmPassword}
      error={flow.fieldErrors.confirm_password}
    />
    {#snippet footer()}
      <span>
        {copy.haveAccountPrompt}
        <a href="/login" class="text-accent hover:underline">{copy.signIn}</a>
      </span>
    {/snippet}
  </AuthForm>
{/if}
