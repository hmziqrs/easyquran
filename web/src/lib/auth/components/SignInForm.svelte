<script lang="ts">
  import { createForm, revalidateLogic } from "@tanstack/svelte-form";
  import AuthForm from "./AuthForm.svelte";
  import AuthField from "./AuthField.svelte";
  import PasswordInput from "./PasswordInput.svelte";
  import { Icon } from "$lib/components/icon";
  import type { LoginFlow } from "$lib/auth/flows.svelte";
  import {
    dynamicValidator,
    fieldError,
    ServerFieldErrors,
  } from "$lib/auth/form-validation.svelte";
  import { loginSchema, totpSchema } from "$lib/auth/schemas";
  import { getAuthCopy } from "$lib/i18n/auth-copy";
  import { getAuthValidationCopy } from "$lib/i18n/auth-validation-copy";

  const copy = getAuthCopy();
  const validationCopy = getAuthValidationCopy();

  type Props = {
    flow: LoginFlow;
    variant?: "page" | "modal";
    onsuccess: () => void | Promise<void>;
  };

  let { flow, variant = "page", onsuccess }: Props = $props();

  const credentialErrors = new ServerFieldErrors();
  const totpErrors = new ServerFieldErrors();

  const credentialsForm = createForm(() => ({
    defaultValues: { email: flow.email, password: flow.password },
    validationLogic: revalidateLogic({ mode: "submit", modeAfterSubmission: "change" }),
    validators: { onDynamic: dynamicValidator(loginSchema(validationCopy)) },
    onSubmit: async ({ value }) => {
      credentialErrors.clearAll();
      flow.email = value.email.trim();
      flow.password = value.password;
      const ok = await flow.submitCredentials();
      if (!ok) {
        credentialErrors.adopt(flow.fieldErrors);
        return;
      }
      await onsuccess();
    },
  }));

  const totpForm = createForm(() => ({
    defaultValues: { code: flow.code },
    validationLogic: revalidateLogic({ mode: "submit", modeAfterSubmission: "change" }),
    validators: { onDynamic: dynamicValidator(totpSchema(validationCopy)) },
    onSubmit: async ({ value }) => {
      totpErrors.clearAll();
      flow.code = value.code.trim();
      const ok = await flow.submitTotp();
      if (!ok) {
        totpErrors.adopt(flow.fieldErrors);
        return;
      }
      await onsuccess();
    },
  }));

  function cancelTotp(): void {
    flow.cancelTotp();
    totpErrors.clearAll();
    totpForm.reset();
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
    onsubmit={() => totpForm.handleSubmit()}
  >
    <totpForm.Field name="code">
      {#snippet children(field)}
        <AuthField
          id="signin-totp"
          name={field.name}
          label={copy.fieldAuthenticationCode}
          inputmode="numeric"
          autocomplete="one-time-code"
          maxlength={6}
          value={field.state.value}
          error={fieldError(field.state.meta.errors) ?? totpErrors.current.code ?? null}
          oninput={(next) => {
            totpErrors.clearField(field.name);
            field.handleChange(next);
          }}
          onblur={field.handleBlur}
        />
      {/snippet}
    </totpForm.Field>
    {#snippet footer()}
      <button type="button" class="text-accent hover:underline" onclick={cancelTotp}>
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
    onsubmit={() => credentialsForm.handleSubmit()}
  >
    <credentialsForm.Field name="email">
      {#snippet children(field)}
        <AuthField
          id="signin-email"
          name={field.name}
          label={copy.fieldEmail}
          type="email"
          autocomplete="email"
          placeholder={copy.emailPlaceholder}
          value={field.state.value}
          error={fieldError(field.state.meta.errors) ?? credentialErrors.current.email ?? null}
          oninput={(next) => {
            credentialErrors.clearField(field.name);
            field.handleChange(next);
          }}
          onblur={field.handleBlur}
        >
          {#snippet leadingIcon()}<Icon name="mail" size={16} />{/snippet}
        </AuthField>
      {/snippet}
    </credentialsForm.Field>
    <credentialsForm.Field name="password">
      {#snippet children(field)}
        <PasswordInput
          id="signin-password"
          name={field.name}
          label={copy.fieldPassword}
          autocomplete="current-password"
          placeholder={copy.passwordPlaceholder}
          value={field.state.value}
          error={fieldError(field.state.meta.errors) ?? credentialErrors.current.password ?? null}
          oninput={(next) => {
            credentialErrors.clearField(field.name);
            field.handleChange(next);
          }}
          onblur={field.handleBlur}
        />
      {/snippet}
    </credentialsForm.Field>
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
