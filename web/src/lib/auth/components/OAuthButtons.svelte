<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import KeyIcon from "phosphor-svelte/lib/KeyIcon";
  import { createOAuthFlows } from "$lib/auth/flows.svelte";
  import { createPasskeyFlow } from "$lib/auth/passkey-flow.svelte";
  import type { OAuthProvider } from "$lib/auth/oauth-flow.svelte";
  import OAuthIcon from "./OAuthIcon.svelte";
  import { getAuthCopy } from "$lib/i18n/auth-copy";

  const copy = getAuthCopy();

  type Props = {
    onPasskeySuccess?: () => void | Promise<void>;
  };

  let { onPasskeySuccess }: Props = $props();

  const oauth = createOAuthFlows();
  const passkey = createPasskeyFlow();

  const providers: ReadonlyArray<{ id: OAuthProvider; label: string }> = [
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
    oauthErrorCode ? copy.oauthError : passkey.genericError,
  );

  async function begin(provider: OAuthProvider): Promise<void> {
    await oauth[provider].begin();
  }

  async function passkeyLogin(): Promise<void> {
    const ok = await passkey.login();
    if (!ok) return;
    await onPasskeySuccess?.();
  }
</script>

<section aria-label={copy.moreSignInOptionsAria} class="flex flex-col gap-3">
  <div class="flex items-center gap-3" aria-hidden="true">
    <span class="h-px flex-1 bg-line-2"></span>
    <span class="eyebrow">{copy.orContinueWith}</span>
    <span class="h-px flex-1 bg-line-2"></span>
  </div>

  {#if socialError}
    <p role="alert" aria-live="assertive" class="text-center text-sm text-destructive">
      {socialError}
    </p>
  {/if}

  <div class="flex flex-col gap-2.5">
    {#each providers as p (p.id)}
      <Button
        type="button"
        variant="ghost"
        size="lg"
        class="relative w-full"
        disabled={socialPending}
        onclick={() => begin(p.id)}
      >
        <OAuthIcon provider={p.id} size={16} class="absolute left-[18px] top-1/2 -translate-y-1/2" />
        <span>{oauth[p.id].pending ? copy.pleaseWait : copy.continueWithProvider(p.label)}</span>
      </Button>
    {/each}
    {#if passkey.supported}
      <Button
        type="button"
        variant="ghost"
        size="lg"
        class="relative w-full"
        disabled={socialPending}
        onclick={passkeyLogin}
      >
        <KeyIcon
          weight="fill"
          size={16}
          class="absolute left-[18px] top-1/2 -translate-y-1/2"
          aria-hidden="true"
        />
        <span>{passkey.pending ? copy.pleaseWait : copy.continueWithPasskey}</span>
      </Button>
    {/if}
  </div>
</section>
