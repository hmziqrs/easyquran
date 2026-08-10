<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { createTwoFactorFlow } from "$lib/auth/flows.svelte";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { VERIFY_EMAIL_NEXT } from "$lib/auth/auth-copy";

  const flow = createTwoFactorFlow();

  const twoFactorEnabled = $derived(authState.user?.two_fa_enabled === true);
  const verified = $derived(authState.user?.is_verified === true);
  const showSetupCta = $derived(verified && !twoFactorEnabled && flow.step === "idle");
  const showVerifyPanel = $derived(flow.step === "verify");
  const showEnabledPanel = $derived(flow.step === "enabled" || (twoFactorEnabled && flow.step === "idle"));
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
        void flow.verify();
      }}
      class="flex flex-col gap-2"
    >
      <Label for="twofa-code">Authentication code</Label>
      <Input
        id="twofa-code"
        type="text"
        inputmode="numeric"
        autocomplete="one-time-code"
        maxlength={6}
        bind:value={flow.verifyCode}
        aria-invalid={Boolean(flow.fieldErrors.code)}
      />
      {#if flow.fieldErrors.code}
        <span class="text-xs text-destructive">{flow.fieldErrors.code}</span>
      {/if}
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
      <p class="text-sm text-fg-2">Disabling removes the second step at sign-in.</p>
    </div>
    {#if flow.genericError}
      <p role="alert" class="text-sm text-destructive">{flow.genericError}</p>
    {/if}
    <Button variant="ghost" class="self-start" disabled={flow.pending} onclick={() => flow.disable()}>
      {flow.pending ? "Disabling…" : "Disable 2FA"}
    </Button>
  </section>
{/if}
