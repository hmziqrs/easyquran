<script lang="ts">
  import { createForm, revalidateLogic } from "@tanstack/svelte-form";
  import AuthForm from "./AuthForm.svelte";
  import AuthField from "./AuthField.svelte";
  import PasswordInput from "./PasswordInput.svelte";
  import { Icon } from "$lib/components/icon";
  import type { RegisterFlow } from "$lib/auth/flows.svelte";
  import {
    dynamicValidator,
    fieldError,
    ServerFieldErrors,
  } from "$lib/auth/form-validation.svelte";
  import { registerSchema, totpSchema } from "$lib/auth/schemas";
  import { getAuthCopy } from "$lib/i18n/auth-copy";
  import { getAuthValidationCopy } from "$lib/i18n/auth-validation-copy";

  const copy = getAuthCopy();
  const validationCopy = getAuthValidationCopy();

  type Props = {
    flow: RegisterFlow;
    variant?: "page" | "modal";
    onsuccess: () => void | Promise<void>;
  };

  let { flow, variant = "page", onsuccess }: Props = $props();

  const registerErrors = new ServerFieldErrors();
  const totpErrors = new ServerFieldErrors();

  const form = createForm(() => ({
    defaultValues: {
      name: flow.name,
      email: flow.email,
      password: flow.password,
      confirm_password: flow.confirmPassword,
    },
    validationLogic: revalidateLogic({ mode: "submit", modeAfterSubmission: "change" }),
    validators: { onDynamic: dynamicValidator(registerSchema(validationCopy)) },
    onSubmit: async ({ value }) => {
      registerErrors.clearAll();
      flow.name = value.name.trim();
      flow.email = value.email.trim();
      flow.password = value.password;
      flow.confirmPassword = value.confirm_password;
      const ok = await flow.submit();
      if (!ok) {
        registerErrors.adopt(flow.fieldErrors);
        return;
      }
      await onsuccess();
    },
  }));

  const totpForm = createForm(() => ({
    defaultValues: { code: flow.login.code },
    validationLogic: revalidateLogic({ mode: "submit", modeAfterSubmission: "change" }),
    validators: { onDynamic: dynamicValidator(totpSchema(validationCopy)) },
    onSubmit: async ({ value }) => {
      totpErrors.clearAll();
      flow.login.code = value.code.trim();
      const ok = await flow.login.submitTotp();
      if (!ok) {
        totpErrors.adopt(flow.login.fieldErrors);
        return;
      }
      await onsuccess();
    },
  }));

  function cancelTotp(): void {
    flow.login.cancelTotp();
    flow.step = "form";
    totpErrors.clearAll();
    totpForm.reset();
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
    onsubmit={() => totpForm.handleSubmit()}
  >
    <totpForm.Field name="code">
      {#snippet children(field)}
        <AuthField
          id="register-totp"
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
    heading={copy.registerHeading}
    subheading={copy.registerSubheading}
    submitLabel={copy.createAccount}
    pending={flow.pending}
    serverError={flow.genericError}
    {variant}
    onsubmit={() => form.handleSubmit()}
  >
    <form.Field name="name">
      {#snippet children(field)}
        <AuthField
          id="register-name"
          name={field.name}
          label={copy.fieldDisplayName}
          autocomplete="name"
          placeholder={copy.displayNamePlaceholder}
          value={field.state.value}
          error={fieldError(field.state.meta.errors) ?? registerErrors.current.name ?? null}
          oninput={(next) => {
            registerErrors.clearField(field.name);
            field.handleChange(next);
          }}
          onblur={field.handleBlur}
        >
          {#snippet leadingIcon()}<Icon name="user" size={16} />{/snippet}
        </AuthField>
      {/snippet}
    </form.Field>
    <form.Field name="email">
      {#snippet children(field)}
        <AuthField
          id="register-email"
          name={field.name}
          label={copy.fieldEmail}
          type="email"
          autocomplete="email"
          placeholder={copy.emailPlaceholder}
          value={field.state.value}
          error={fieldError(field.state.meta.errors) ?? registerErrors.current.email ?? null}
          oninput={(next) => {
            registerErrors.clearField(field.name);
            field.handleChange(next);
          }}
          onblur={field.handleBlur}
        >
          {#snippet leadingIcon()}<Icon name="mail" size={16} />{/snippet}
        </AuthField>
      {/snippet}
    </form.Field>
    <form.Field name="password">
      {#snippet children(field)}
        <PasswordInput
          id="register-password"
          name={field.name}
          label={copy.fieldPassword}
          autocomplete="new-password"
          minlength={12}
          placeholder={copy.passwordMinPlaceholder}
          value={field.state.value}
          error={fieldError(field.state.meta.errors) ?? registerErrors.current.password ?? null}
          oninput={(next) => {
            registerErrors.clearField(field.name);
            field.handleChange(next);
          }}
          onblur={field.handleBlur}
        />
      {/snippet}
    </form.Field>
    <form.Field name="confirm_password">
      {#snippet children(field)}
        <PasswordInput
          id="register-confirm"
          name={field.name}
          label={copy.fieldConfirmPassword}
          autocomplete="new-password"
          minlength={12}
          value={field.state.value}
          error={fieldError(field.state.meta.errors) ??
            registerErrors.current.confirm_password ??
            null}
          oninput={(next) => {
            registerErrors.clearField(field.name);
            field.handleChange(next);
          }}
          onblur={field.handleBlur}
        />
      {/snippet}
    </form.Field>
    {#snippet footer()}
      <span>
        {copy.haveAccountPrompt}
        <a href="/login" class="text-accent hover:underline">{copy.signIn}</a>
      </span>
    {/snippet}
  </AuthForm>
{/if}
