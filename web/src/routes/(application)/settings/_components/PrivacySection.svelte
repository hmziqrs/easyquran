<script lang="ts">
  import { onMount } from "svelte";
  import { Button } from "#lib/components/ui/button/index.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import { createLogoutFlow } from "#lib/auth/flows.svelte.js";
  import { installPurgeHook } from "#lib/auth/purge-hook.js";
  import { isMessagingConfigured } from "#lib/firebase/index.js";
  import { notificationsStatus } from "#lib/components/notifications/notifications-copy.js";
  import { consent } from "#lib/stores/consent.svelte.js";
  import { notifications } from "#lib/stores/notifications.svelte.js";
  import { cn } from "#lib/utils.js";
  import type { SettingsCopy } from "#lib/i18n/settings-copy.js";

  let {
    id,
    heading,
    copy,
  }: {
    id: string;
    heading: string;
    copy: SettingsCopy["privacy"];
  } = $props();

  const logout = createLogoutFlow();
  const signedIn = $derived(authState.status === "authenticated");

  onMount(() => {
    installPurgeHook(authState);
  });

  const toggle = "rounded-pill border px-3.5 py-2.5 text-caption transition-colors duration-150";
  const on = "border-primary bg-primary-soft text-foreground";
  const off = "border-border-strong text-foreground-secondary hover:border-border hover:text-foreground";

  function toggleAnalytics(): void {
    consent.setAnalytics(!consent.analytics);
  }

  function togglePerformance(): void {
    consent.setPerformance(!consent.performance);
    location.reload();
  }

  async function onSignOut(): Promise<void> {
    if (logout.pending) return;
    await logout.run();
  }

  const notificationsLabel = $derived(
    notificationsStatus(copy.notificationsStatus)({
      configured: isMessagingConfigured,
      supported: notifications.supported,
      permission: notifications.permission,
      subscribed: notifications.subscribed,
      pushError: notifications.pushError,
    }),
  );
</script>

<div id={id} tabindex="-1" class="scroll-mt-24">
  <h2 class="text-[17px] font-medium tracking-[-0.02em] text-foreground">{heading}</h2>
  <p class="mt-1 max-w-[70ch] text-[14.5px] leading-relaxed text-foreground-secondary">{copy.intro}</p>

  <div class="mt-4 divide-y divide-border overflow-hidden rounded-xl border border-border-strong bg-surface">
    <div class="flex flex-wrap items-center justify-between gap-4 px-4 py-3.5 sm:px-5">
      <span class="text-[14.5px] font-medium text-foreground">{copy.analytics}</span>
      <button
        type="button"
        aria-pressed={consent.analytics}
        onclick={toggleAnalytics}
        class={cn(toggle, "shrink-0", consent.analytics ? on : off)}
      >
        {consent.analytics ? copy.on : copy.off}
      </button>
    </div>

    <div class="flex flex-wrap items-center justify-between gap-4 px-4 py-3.5 sm:px-5">
      <div class="min-w-0">
        <span class="text-[14.5px] font-medium text-foreground">{copy.performance}</span>
        <p class="mt-0.5 text-caption leading-snug text-muted">{copy.performanceReload}</p>
      </div>
      <button
        type="button"
        aria-pressed={consent.performance}
        onclick={togglePerformance}
        class={cn(toggle, "shrink-0", consent.performance ? on : off)}
      >
        {consent.performance ? copy.on : copy.off}
      </button>
    </div>

    <div class="flex flex-wrap items-center justify-between gap-4 px-4 py-3.5 sm:px-5">
      <span class="text-[14.5px] font-medium text-foreground">{copy.notifications}</span>
      <span class="shrink-0 text-end text-caption leading-tight text-muted">
        {notificationsLabel}
      </span>
    </div>

    {#if signedIn}
      <div class="flex flex-wrap items-center gap-3 px-4 py-3.5 sm:px-5">
        {#if logout.genericError}
          <p role="alert" aria-live="assertive" class="text-caption text-destructive">
            {copy.signOutError}
          </p>
        {/if}
        <Button
          variant="ghost"
          size="sm"
          class="ms-auto shrink-0"
          disabled={logout.pending}
          onclick={onSignOut}>{copy.signOut}</Button
        >
      </div>
    {/if}
  </div>

  <p class="mt-4 text-caption leading-snug text-muted">{copy.syncNote}</p>
</div>
