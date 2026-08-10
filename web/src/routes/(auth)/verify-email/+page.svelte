<script lang="ts">
  import { goto } from "$app/navigation";
  import AuthForm from "$lib/auth/components/AuthForm.svelte";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { Button } from "$lib/components/ui/button";
  import { createVerifyEmailFlow } from "$lib/auth/flows.svelte";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { VERIFY_EMAIL_NEXT } from "$lib/auth/auth-copy";

  const flow = createVerifyEmailFlow();

  const alreadyVerified = $derived(authState.user?.is_verified === true || flow.alreadyVerified);

  async function handleSubmit(): Promise<void> {
    const ok = await flow.verify();
    if (ok) await goto("/app");
  }
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
    onsubmit={handleSubmit}
  >
    <div class="flex flex-col gap-1.5">
      <Label for="ve-code">Verification code</Label>
      <Input
        id="ve-code"
        type="text"
        inputmode="numeric"
        autocomplete="one-time-code"
        bind:value={flow.code}
        aria-invalid={Boolean(flow.fieldErrors.code)}
        data-invalid={Boolean(flow.fieldErrors.code)}
      />
      {#if flow.fieldErrors.code}
        <span class="text-xs text-destructive">{flow.fieldErrors.code}</span>
      {/if}
    </div>
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
