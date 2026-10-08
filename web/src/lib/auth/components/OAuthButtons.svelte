<script lang="ts">
  import { Button } from "#lib/components/ui/button/index.js";
  import KeyIcon from "phosphor-svelte/lib/KeyIcon";
  import { createOAuthFlows } from "#lib/auth/flows.svelte.js";
  import { createPasskeyFlow } from "#lib/auth/passkey-flow.svelte.js";
  import type { OAuthProvider } from "#lib/auth/oauth-flow.svelte.js";
  import OAuthIcon from "./OAuthIcon.svelte";
  import { getAuthCopy } from "#lib/i18n/auth-copy.js";

  const copy = getAuthCopy();

  type Props = {
    onPasskeySuccess?: () => void | Promise<void>;
  };

  let { onPasskeySuccess }: Props = $props();

  const oauth = createOAuthFlows();
  const passkey = createPasskeyFlow();

  const providers: ReadonlyArray<{ id: OAuthProvider; label: string }> = [
    { id: "google", label: "Google" },
    { id: "github", label: "GitHub" },
    { id: "discord", label: "Discord" },
  ];

  const oauthPending = $derived(providers.some((p) => oauth[p.id].pending));
  const socialPending = $derived(oauthPending || passkey.pending);
  const oauthErrorCode = $derived(
    providers.map((p) => oauth[p.id].lastErrorCode).find((c) => c !== null) ?? null,
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
    <span class="h-px flex-1 bg-border-strong"></span>
    <span class="eyebrow">{copy.orContinueWith}</span>
    <span class="h-px flex-1 bg-border-strong"></span>
  </div>

  {#if socialError}
    <p role="alert" aria-live="assertive" class="text-center text-sm text-danger">
      {socialError}
    </p>
  {/if}

  <!-- Providers are icon-only (label lives in aria-label/title); the passkey keeps its
       dedicated labelled button below. -->
  <div class="grid grid-cols-3 gap-2.5">
    {#each providers as p (p.id)}
      <Button
        type="button"
        variant="ghost"
        size="lg"
        aria-label={copy.continueWithProvider(p.label)}
        title={copy.continueWithProvider(p.label)}
        disabled={socialPending}
        onclick={() => begin(p.id)}
      >
        <OAuthIcon
          provider={p.id}
          size={20}
          class="mx-auto transition-opacity {oauth[p.id].pending ? 'opacity-40' : ''}"
        />
      </Button>
    {/each}
  </div>
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
        class="absolute start-[18px] top-1/2 -translate-y-1/2"
        aria-hidden="true"
      />
      <span>{passkey.pending ? copy.pleaseWait : copy.continueWithPasskey}</span>
    </Button>
  {/if}
</section>
