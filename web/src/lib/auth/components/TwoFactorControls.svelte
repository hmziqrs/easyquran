<script lang="ts">
  import { createForm, revalidateLogic } from "@tanstack/svelte-form";
  import { Button } from "$lib/components/ui/button";
  import AuthField from "$lib/auth/components/AuthField.svelte";
  import { createTwoFactorFlow } from "$lib/auth/flows.svelte";
  import {
    dynamicValidator,
    fieldError,
    ServerFieldErrors,
  } from "$lib/auth/form-validation.svelte";
  import { twoFactorDisableSchema, twoFactorVerifySchema } from "$lib/auth/schemas";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { VERIFY_EMAIL_NEXT } from "$lib/auth/auth-copy";

  const flow = createTwoFactorFlow();

  const verifyErrors = new ServerFieldErrors();
  const disableErrors = new ServerFieldErrors();

  const verifyForm = createForm(() => ({
    defaultValues: { code: flow.verifyCode },
    validationLogic: revalidateLogic({ mode: "submit", modeAfterSubmission: "change" }),
    validators: { onDynamic: dynamicValidator(twoFactorVerifySchema()) },
    onSubmit: async ({ value }) => {
      verifyErrors.clearAll();
      flow.verifyCode = value.code.trim();
      const ok = await flow.verify();
      if (!ok) verifyErrors.adopt(flow.fieldErrors);
    },
  }));

  const disableForm = createForm(() => ({
    defaultValues: { code: flow.disableCode },
    validationLogic: revalidateLogic({ mode: "submit", modeAfterSubmission: "change" }),
    validators: { onDynamic: dynamicValidator(twoFactorDisableSchema()) },
    onSubmit: async ({ value }) => {
      disableErrors.clearAll();
      flow.disableCode = value.code.trim();
      const ok = await flow.disable();
      if (ok) {
        disableForm.reset();
        return;
      }
      disableErrors.adopt(flow.fieldErrors);
    },
  }));

  const twoFactorEnabled = $derived(authState.user?.two_fa_enabled === true);
  const verified = $derived(authState.user?.is_verified === true);
  const showSetupCta = $derived(verified && !twoFactorEnabled && flow.step === "idle");
  const showVerifyPanel = $derived(flow.step === "verify");
  const showEnabledPanel = $derived(flow.step === "enabled" || (twoFactorEnabled && flow.step === "idle"));
  const showDisabledNotice = $derived(flow.step === "disabled");
</script>

{#if !verified}
  <p class="text-sm text-fg-2">{VERIFY_EMAIL_NEXT}</p>
{:else if showVerifyPanel && flow.setupData}
  <section class="flex flex-col gap-4 rounded-xl border border-line-2 bg-bg-2 p-5">
    <div class="flex flex-col gap-1">
      <h3 class="text-base font-semibold">Enter your authenticator code</h3>
      <p class="text-sm text-fg-2">
        Add this secret to your authenticator app, then enter the 6-digit code it generates.
      </p>
    </div>
    <div class="flex flex-col gap-1">
      <span class="text-xs font-medium uppercase tracking-wide text-fg-3">Secret (keep private)</span>
      <code class="break-all rounded-lg bg-bg-1 px-3 py-2 font-mono text-sm">{flow.setupData.secret}</code>
    </div>
    {#if flow.setupData.backupCodes.length > 0}
      <details class="flex flex-col gap-1.5">
        <summary class="cursor-pointer text-sm font-medium text-fg-2">Show backup codes</summary>
        <p class="text-xs text-fg-3">Save these once. They will not be shown again.</p>
        <ul class="grid grid-cols-2 gap-1 font-mono text-sm">
          {#each flow.setupData.backupCodes as code (code)}
            <li>{code}</li>
          {/each}
        </ul>
      </details>
    {/if}
    <form
      onsubmit={(e) => {
        e.preventDefault();
        void verifyForm.handleSubmit();
      }}
      class="flex flex-col gap-2"
      novalidate
    >
      <verifyForm.Field name="code">
        {#snippet children(field)}
          <AuthField
            id="twofa-code"
            name={field.name}
            label="Authentication code"
            inputmode="numeric"
            autocomplete="one-time-code"
            maxlength={6}
            value={field.state.value}
            error={fieldError(field.state.meta.errors) ?? verifyErrors.current.code ?? null}
            oninput={(next) => {
              verifyErrors.clearField(field.name);
              field.handleChange(next);
            }}
            onblur={field.handleBlur}
          />
        {/snippet}
      </verifyForm.Field>
      {#if flow.genericError}
        <span role="alert" class="text-xs text-destructive">{flow.genericError}</span>
      {/if}
      <Button type="submit" variant="accent" disabled={flow.pending}>
        {flow.pending ? "Verifying…" : "Confirm and enable"}
      </Button>
    </form>
  </section>
{:else if showSetupCta}
  <section class="flex flex-col gap-3">
    <div class="flex flex-col gap-1">
      <h3 class="text-base font-semibold">Two-factor authentication</h3>
      <p class="text-sm text-fg-2">Add a second step at sign-in using an authenticator app.</p>
    </div>
    {#if flow.genericError}
      <p role="alert" class="text-sm text-destructive">{flow.genericError}</p>
    {/if}
    <Button variant="accent" class="self-start" disabled={flow.pending} onclick={() => flow.setup()}>
      {flow.pending ? "Starting…" : "Set up 2FA"}
    </Button>
  </section>
{:else if showEnabledPanel}
  <section class="flex flex-col gap-3">
    <div class="flex flex-col gap-1">
      <h3 class="text-base font-semibold">Two-factor authentication is on</h3>
      <p class="text-sm text-fg-2">
        Enter your authenticator code to remove the second step at sign-in.
      </p>
    </div>
    <form
      onsubmit={(e) => {
        e.preventDefault();
        void disableForm.handleSubmit();
      }}
      class="flex flex-col gap-2"
      novalidate
    >
      <disableForm.Field name="code">
        {#snippet children(field)}
          <AuthField
            id="twofa-disable-code"
            name={field.name}
            label="Authentication code"
            autocomplete="one-time-code"
            maxlength={64}
            hint="Your 6-digit code or a backup code."
            value={field.state.value}
            error={fieldError(field.state.meta.errors) ?? disableErrors.current.code ?? null}
            oninput={(next) => {
              disableErrors.clearField(field.name);
              field.handleChange(next);
            }}
            onblur={field.handleBlur}
          />
        {/snippet}
      </disableForm.Field>
      {#if flow.genericError}
        <span role="alert" class="text-xs text-destructive">{flow.genericError}</span>
      {/if}
      <Button type="submit" variant="ghost" class="self-start" disabled={flow.pending}>
        {flow.pending ? "Disabling…" : "Disable 2FA"}
      </Button>
    </form>
  </section>
{:else if showDisabledNotice}
  <section class="flex flex-col gap-3">
    <div class="flex flex-col gap-1">
      <h3 class="text-base font-semibold">Two-factor authentication is off</h3>
      <p class="text-sm text-fg-2">Two-factor authentication has been disabled.</p>
    </div>
    {#if flow.genericError}
      <p role="alert" class="text-sm text-destructive">{flow.genericError}</p>
    {/if}
    <Button variant="accent" class="self-start" disabled={flow.pending} onclick={() => flow.setup()}>
      {flow.pending ? "Starting…" : "Set up 2FA"}
    </Button>
  </section>
{/if}
