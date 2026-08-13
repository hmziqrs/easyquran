<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import type { OAuthFlow, OAuthProvider } from "$lib/auth/oauth-flow.svelte";
  import type { PasskeyFlow } from "$lib/auth/passkey-flow.svelte";

  type Props = {
    oauth: Readonly<Record<OAuthProvider, OAuthFlow>>;
    passkey: PasskeyFlow;
    ariaLabel: string;
    onPasskeySuccess: () => void | Promise<void>;
  };

  let { oauth, passkey, ariaLabel, onPasskeySuccess }: Props = $props();

  const oauthProviders: ReadonlyArray<{ id: OAuthProvider; label: string }> = [
    { id: "google", label: "Google" },
    { id: "apple", label: "Apple" },
    { id: "facebook", label: "Facebook" },
    { id: "github", label: "GitHub" },
  ];

  const oauthPending = $derived(
    oauth.google.pending || oauth.apple.pending || oauth.facebook.pending || oauth.github.pending,
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

  async function beginOauth(provider: OAuthProvider): Promise<void> {
    await oauth[provider].begin();
  }

  async function passkeyLogin(): Promise<void> {
    const ok = await passkey.login();
    if (!ok) return;
    await onPasskeySuccess();
  }
</script>

<section class="flex flex-col gap-3" aria-label={ariaLabel}>
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
