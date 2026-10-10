<script lang="ts">
  import { onMount } from "svelte";
  import { Button } from "#lib/components/ui/button/index.js";
  import {
    accountClient,
    canUnlinkIdentity,
    identityProviderLabel,
    type AccountClient,
    type IdentityProvider,
    type IdentitySummary,
  } from "#lib/auth/account-client.js";
  import { GENERIC_TRY_AGAIN } from "#lib/auth/auth-copy.js";
  import { rememberLinkProvider } from "#lib/auth/link-attempt.js";
  import { getAuthCopy } from "#lib/i18n/auth-copy.js";

  interface Props {
    client?: AccountClient;
    navigate?: (url: string) => void;
    onAnonymous?: () => void | Promise<void>;
  }

  function defaultNavigate(url: string): void {
    window.location.assign(url);
  }

  let { client = accountClient, navigate = defaultNavigate, onAnonymous }: Props = $props();

  const copy = getAuthCopy();

  let summary = $state<IdentitySummary | null>(null);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let pendingProvider = $state<IdentityProvider | null>(null);
  let confirmingProvider = $state<IdentityProvider | null>(null);
  let unlinkError = $state<string | null>(null);

  const canUnlink = $derived(summary ? canUnlinkIdentity(summary) : false);
  const linkableProviders = $derived.by((): IdentityProvider[] => {
    if (!summary) return [];
    const linked = new Set(summary.identities.map((identity) => identity.provider));
    return summary.linkableProviders.filter((provider) => !linked.has(provider));
  });

  onMount(() => {
    void load();
  });

  async function load(): Promise<void> {
    loading = true;
    loadError = null;
    const res = await client.getIdentities();
    loading = false;
    if (res.status === "ok" && res.data) {
      summary = res.data;
      return;
    }
    if (res.status === "anonymous") {
      await onAnonymous?.();
      return;
    }
    loadError = GENERIC_TRY_AGAIN;
  }

  function link(provider: IdentityProvider): void {
    rememberLinkProvider(provider);
    navigate(`${client.auth.apiBase}/auth/${provider}/v1/link`);
  }

  async function unlink(provider: IdentityProvider): Promise<void> {
    if (pendingProvider) return;
    pendingProvider = provider;
    unlinkError = null;
    const res = await client.unlinkIdentity(provider);
    pendingProvider = null;
    confirmingProvider = null;
    if (res.status === "ok") {
      if (res.data) summary = res.data;
      else await load();
      return;
    }
    if (res.status === "anonymous") {
      await onAnonymous?.();
      return;
    }
    unlinkError = res.error?.message ?? copy.connectedUnlinkError;
  }

  function linkedOn(createdAt: string): string {
    const date = new Date(createdAt);
    if (Number.isNaN(date.getTime())) return createdAt;
    return new Intl.DateTimeFormat(copy.locale, { dateStyle: "medium" }).format(date);
  }
</script>

<section class="flex flex-col gap-4">
  <div class="flex flex-col gap-1">
    <h2 class="text-lg font-medium">{copy.connectedAccountsHeading}</h2>
    <p class="text-sm text-fg-2">{copy.connectedAccountsDescription}</p>
  </div>

  {#if loadError}
    <p role="alert" aria-live="assertive" class="text-sm text-destructive">{loadError}</p>
    <Button variant="ghost" size="sm" class="self-start" onclick={() => void load()}>Retry</Button>
  {:else if loading && !summary}
    <p class="text-sm text-fg-2" role="status" aria-live="polite">Loading connected accounts…</p>
  {:else if summary}
    {#if summary.identities.length > 0}
      <ul class="flex flex-col gap-2">
        {#each summary.identities as identity (identity.provider)}
          <li
            class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-line-2 bg-bg-2 px-4 py-3"
          >
            <div class="flex flex-col gap-0.5">
              <span class="text-sm font-medium">{identityProviderLabel(identity.provider)}</span>
              {#if identity.createdAt}
                <span class="text-xs text-fg-3">
                  {copy.connectedLinkedOn(linkedOn(identity.createdAt))}
                </span>
              {/if}
            </div>
            {#if confirmingProvider === identity.provider}
              <div class="flex items-center gap-2">
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={pendingProvider !== null}
                  onclick={() => unlink(identity.provider)}
                >
                  {pendingProvider === identity.provider
                    ? copy.pleaseWait
                    : copy.connectedUnlinkConfirm}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={pendingProvider !== null}
                  onclick={() => {
                    confirmingProvider = null;
                  }}
                >
                  {copy.connectedUnlinkCancel}
                </Button>
              </div>
            {:else}
              <Button
                variant="ghost"
                size="sm"
                disabled={!canUnlink || pendingProvider !== null}
                onclick={() => {
                  confirmingProvider = identity.provider;
                }}
              >
                {copy.connectedUnlink}
              </Button>
            {/if}
          </li>
        {/each}
      </ul>
      {#if !canUnlink}
        <p class="text-xs text-fg-3">{copy.connectedLastMethodHint}</p>
      {/if}
    {/if}

    {#if unlinkError}
      <p role="alert" aria-live="assertive" class="text-sm text-destructive">{unlinkError}</p>
    {/if}

    {#if linkableProviders.length > 0}
      <div class="flex flex-wrap gap-2">
        {#each linkableProviders as provider (provider)}
          <Button variant="secondary" size="sm" onclick={() => link(provider)}>
            {copy.connectedLinkProvider(identityProviderLabel(provider))}
          </Button>
        {/each}
      </div>
    {/if}
  {/if}
</section>
