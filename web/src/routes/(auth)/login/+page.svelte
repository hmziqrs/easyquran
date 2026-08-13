<script lang="ts">
  import { goto } from "$app/navigation";
  import AuthForm from "$lib/auth/components/AuthForm.svelte";
  import SocialAuth from "$lib/auth/components/SocialAuth.svelte";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { createLoginFlow, createOAuthFlows } from "$lib/auth/flows.svelte";
  import { createPasskeyFlow } from "$lib/auth/passkey-flow.svelte";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { postAuthPath } from "$lib/auth/post-auth-path";

  const flow = createLoginFlow();
  const oauth = createOAuthFlows();
  const passkey = createPasskeyFlow();

  async function handleSubmit(): Promise<void> {
    const ok = flow.step === "totp" ? await flow.submitTotp() : await flow.submitCredentials();
    if (!ok) return;
    await goto(postAuthPath(authState.user));
  }

  async function afterPasskeyLogin(): Promise<void> {
    await goto(postAuthPath(authState.user));
  }

  function invalid(field: string): boolean {
    return Boolean(flow.fieldErrors[field]);
  }
</script>

{#if flow.step === "totp"}
  <AuthForm
    heading="Two-factor code"
    subheading="Enter the 6-digit code from your authenticator app to finish signing in."
    submitLabel="Continue"
    pending={flow.pending}
    serverError={flow.genericError}
    onsubmit={handleSubmit}
  >
    <div class="flex flex-col gap-1.5">
      <Label for="totp-code">Authentication code</Label>
      <Input
        id="totp-code"
        type="text"
        inputmode="numeric"
        autocomplete="one-time-code"
        maxlength={6}
        bind:value={flow.code}
        aria-invalid={invalid("code")}
        data-invalid={invalid("code")}
      />
      {#if flow.fieldErrors.code}
        <span class="text-xs text-destructive">{flow.fieldErrors.code}</span>
      {/if}
    </div>
    {#snippet footer()}
      <button type="button" class="text-accent hover:underline" onclick={() => flow.cancelTotp()}>
        Use a different account
      </button>
    {/snippet}
  </AuthForm>
{:else}
  <div class="flex flex-col gap-6">
    <AuthForm
      heading="Sign in"
      subheading="Welcome back. Sign in to sync your bookmarks and reading progress."
      submitLabel="Sign in"
      pending={flow.pending}
      serverError={flow.genericError}
      onsubmit={handleSubmit}
    >
      <div class="flex flex-col gap-1.5">
        <Label for="login-email">Email</Label>
        <Input
          id="login-email"
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
      <div class="flex flex-col gap-1.5">
        <Label for="login-password">Password</Label>
        <Input
          id="login-password"
          type="password"
          autocomplete="current-password"
          bind:value={flow.password}
          aria-invalid={invalid("password")}
          data-invalid={invalid("password")}
        />
        {#if flow.fieldErrors.password}
          <span class="text-xs text-destructive">{flow.fieldErrors.password}</span>
        {/if}
      </div>
      {#snippet footer()}
        <span>Forgot your password? <a href="/forgot-password" class="text-accent hover:underline">Reset it</a></span>
        <span>No account? <a href="/register" class="text-accent hover:underline">Create one</a></span>
      {/snippet}
    </AuthForm>

    <SocialAuth {oauth} {passkey} ariaLabel="More sign-in options" onPasskeySuccess={afterPasskeyLogin} />
  </div>
{/if}
