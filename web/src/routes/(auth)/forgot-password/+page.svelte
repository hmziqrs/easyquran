<script lang="ts">
  import { goto } from "$app/navigation";
  import AuthForm from "$lib/auth/components/AuthForm.svelte";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { createForgotPasswordFlow } from "$lib/auth/flows.svelte";

  const flow = createForgotPasswordFlow();

  async function handleSubmit(): Promise<void> {
    if (flow.step === "request") {
      await flow.request();
    } else if (flow.step === "verify") {
      const ok = await flow.verifyCode();
      if (!ok) return;
    } else if (flow.step === "reset") {
      const ok = await flow.reset();
      if (!ok) return;
      await goto("/login");
    }
  }

  function invalid(field: string): boolean {
    return Boolean(flow.fieldErrors[field]);
  }
</script>

{#if flow.step === "request"}
  <AuthForm
    heading="Reset your password"
    subheading="Enter your account email and we'll send a reset code to it."
    submitLabel="Send reset code"
    pending={flow.pending}
    serverError={flow.genericError}
    successNotice={flow.successMessage}
    onsubmit={handleSubmit}
  >
    <div class="flex flex-col gap-1.5">
      <Label for="fp-email">Email</Label>
      <Input
        id="fp-email"
        type="email"
        autocomplete="email"
        bind:value={flow.email}
        aria-invalid={invalid("email")}
        data-invalid={invalid("email")}
      />
      {#if flow.fieldErrors.email}
        <span class="text-xs text-destructive">{flow.fieldErrors.email}</span>
      {/if}
    </div>
    {#snippet footer()}
      <span>Remembered it? <a href="/login" class="text-accent hover:underline">Back to sign in</a></span>
    {/snippet}
  </AuthForm>
{:else if flow.step === "verify"}
  <AuthForm
    heading="Enter your reset code"
    subheading="We sent an 8-character code to your email if an account exists for it."
    submitLabel="Verify code"
    pending={flow.pending}
    serverError={flow.genericError}
    onsubmit={handleSubmit}
  >
    <div class="flex flex-col gap-1.5">
      <Label for="fp-code">Reset code</Label>
      <Input
        id="fp-code"
        type="text"
        inputmode="text"
        autocomplete="off"
        bind:value={flow.code}
        aria-invalid={invalid("code")}
        data-invalid={invalid("code")}
      />
      {#if flow.fieldErrors.code}
        <span class="text-xs text-destructive">{flow.fieldErrors.code}</span>
      {/if}
    </div>
  </AuthForm>
{:else if flow.step === "reset"}
  <AuthForm
    heading="Set a new password"
    subheading="Choose a strong password of at least 12 characters."
    submitLabel="Reset password"
    pending={flow.pending}
    serverError={flow.genericError}
    onsubmit={handleSubmit}
  >
    <div class="flex flex-col gap-1.5">
      <Label for="fp-password">New password</Label>
      <Input
        id="fp-password"
        type="password"
        autocomplete="new-password"
        minlength={12}
        bind:value={flow.password}
        aria-invalid={invalid("password")}
        data-invalid={invalid("password")}
      />
      {#if flow.fieldErrors.password}
        <span class="text-xs text-destructive">{flow.fieldErrors.password}</span>
      {/if}
    </div>
    <div class="flex flex-col gap-1.5">
      <Label for="fp-confirm">Confirm new password</Label>
      <Input
        id="fp-confirm"
        type="password"
        autocomplete="new-password"
        minlength={12}
        bind:value={flow.confirmPassword}
        aria-invalid={invalid("confirm_password")}
        data-invalid={invalid("confirm_password")}
      />
      {#if flow.fieldErrors.confirm_password}
        <span class="text-xs text-destructive">{flow.fieldErrors.confirm_password}</span>
      {/if}
    </div>
  </AuthForm>
{:else}
  <AuthForm
    heading="Password reset"
    subheading="Your password has been reset. You can sign in with your new password now."
    submitLabel="Back to sign in"
    onsubmit={async () => {
      await goto("/login");
    }}
  />
{/if}
