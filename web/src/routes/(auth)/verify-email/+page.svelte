<script lang="ts">
  import { goto } from "$app/navigation";
  import { createForm, revalidateLogic } from "@tanstack/svelte-form";
  import AuthForm from "$lib/auth/components/AuthForm.svelte";
  import AuthField from "$lib/auth/components/AuthField.svelte";
  import { Button } from "$lib/components/ui/button";
  import { createVerifyEmailFlow } from "$lib/auth/flows.svelte";
  import {
    dynamicValidator,
    fieldError,
    ServerFieldErrors,
  } from "$lib/auth/form-validation.svelte";
  import { verifyEmailSchema } from "$lib/auth/schemas";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { VERIFY_EMAIL_NEXT } from "$lib/auth/auth-copy";

  const flow = createVerifyEmailFlow();
  const serverErrors = new ServerFieldErrors();

  const alreadyVerified = $derived(authState.user?.is_verified === true || flow.alreadyVerified);

  const form = createForm(() => ({
    defaultValues: { code: flow.code },
    validationLogic: revalidateLogic({ mode: "submit", modeAfterSubmission: "change" }),
    validators: { onDynamic: dynamicValidator(verifyEmailSchema()) },
    onSubmit: async ({ value }) => {
      serverErrors.clearAll();
      flow.code = value.code.trim();
      const ok = await flow.verify();
      if (!ok) {
        serverErrors.adopt(flow.fieldErrors);
        return;
      }
      await goto("/app");
    },
  }));
</script>

{#if alreadyVerified && !flow.verified}
  <AuthForm
    heading="You're verified"
    subheading="Your email is already verified. There's nothing more to do here."
    submitLabel="Go to the reader"
    onsubmit={async () => {
      await goto("/app");
    }}
  />
{:else if flow.verified}
  <AuthForm
    heading="Email verified"
    subheading="Thanks for confirming your email. Your account is ready."
    submitLabel="Continue to the reader"
    onsubmit={async () => {
      await goto("/app");
    }}
  />
{:else}
  <AuthForm
    heading="Verify your email"
    subheading="Enter the code we sent to your inbox to confirm your account."
    submitLabel="Confirm email"
    pending={flow.pending}
    serverError={flow.genericError}
    successNotice={flow.successMessage}
    onsubmit={() => form.handleSubmit()}
  >
    <form.Field name="code">
      {#snippet children(field)}
        <AuthField
          id="ve-code"
          name={field.name}
          label="Verification code"
          inputmode="numeric"
          autocomplete="one-time-code"
          maxlength={8}
          value={field.state.value}
          error={fieldError(field.state.meta.errors) ?? serverErrors.current.code ?? null}
          oninput={(next) => {
            serverErrors.clearField(field.name);
            field.handleChange(next);
          }}
          onblur={field.handleBlur}
        />
      {/snippet}
    </form.Field>
    <div class="flex items-center justify-between gap-3">
      <Button
        type="button"
        variant="quiet"
        size="sm"
        disabled={flow.resendPending}
        onclick={() => flow.resend()}
      >
        {flow.resendPending ? "Sending…" : "Resend code"}
      </Button>
      <span class="text-xs text-fg-3">{VERIFY_EMAIL_NEXT}</span>
    </div>
    {#snippet footer()}
      <span>Need to switch accounts? <a href="/login" class="text-accent hover:underline">Sign in again</a></span>
    {/snippet}
  </AuthForm>
{/if}
