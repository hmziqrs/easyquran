<script lang="ts">
  import { goto } from "$app/navigation";
  import AuthForm from "$lib/auth/components/AuthForm.svelte";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { createLoginFlow, createOAuthFlows } from "$lib/auth/flows.svelte";
  import { createPasskeyFlow } from "$lib/auth/passkey-flow.svelte";
  import { authState } from "$lib/auth/auth-state.svelte";
  import type { OAuthProvider } from "$lib/auth/oauth-flow.svelte";

  const flow = createLoginFlow();
  const oauth = createOAuthFlows();
  const passkey = createPasskeyFlow();

  const oauthProviders: ReadonlyArray<{ id: OAuthProvider; label: string }> = [
    { id: "google", label: "Google" },
    { id: "apple", label: "Apple" },
    { id: "facebook", label: "Facebook" },
    { id: "github", label: "GitHub" },
  ];

  const oauthPending = $derived(
    oauth.google.pending ||
      oauth.apple.pending ||
      oauth.facebook.pending ||
      oauth.github.pending,
  );
  const socialPending = $derived(oauthPending || passkey.pending);
  const oauthErrorCode = $derived(
    oauth.google.lastErrorCode ??
      oauth.apple.lastErrorCode ??
      oauth.facebook.lastErrorCode ??
      oauth.github.lastErrorCode,
  );
  const socialError = $derived(
    oauthErrorCode ? "Couldn't start sign-in. Please try again." : passkey.genericError,
  );

  async function handleSubmit(): Promise<void> {
    const ok = flow.step === "totp" ? await flow.submitTotp() : await flow.submitCredentials();
    if (!ok) return;
    const user = authState.user;
    await goto(user && !user.is_verified ? "/verify-email" : "/app");
  }

  async function beginOauth(provider: OAuthProvider): Promise<void> {
    await oauth[provider].begin();
  }

  async function passkeyLogin(): Promise<void> {
    const ok = await passkey.login();
    if (!ok) return;
    const user = authState.user;
    await goto(user && !user.is_verified ? "/verify-email" : "/app");
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

    <section class="flex flex-col gap-3" aria-label="More sign-in options">
      <div class="flex items-center gap-3" aria-hidden="true">
        <span class="h-px flex-1 bg-line-2"></span>
        <span class="text-xs text-fg-3">or</span>
        <span class="h-px flex-1 bg-line-2"></span>
      </div>

      {#if socialError}
        <p role="alert" aria-live="assertive" class="text-center text-sm text-destructive">
          {socialError}
        </p>
      {/if}

      <div class="flex flex-col gap-2.5">
        {#each oauthProviders as p (p.id)}
          <Button
            type="button"
            variant="ghost"
            size="lg"
            class="w-full"
            disabled={socialPending}
            onclick={() => beginOauth(p.id)}
          >
            {oauth[p.id].pending ? "Please wait…" : `Continue with ${p.label}`}
          </Button>
        {/each}
        {#if passkey.supported}
          <Button
            type="button"
            variant="ghost"
            size="lg"
            class="w-full"
            disabled={socialPending}
            onclick={passkeyLogin}
          >
            {passkey.pending ? "Please wait…" : "Continue with passkey"}
          </Button>
        {/if}
      </div>
    </section>
  </div>
{/if}
