<script lang="ts">
  import { onMount } from "svelte";
  import { updated } from "$app/state";
  import { Button } from "$lib/components/ui/button";
  import { Card } from "$lib/components";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { createLogoutFlow } from "$lib/auth/flows.svelte";
  import { installPurgeHook } from "$lib/auth/purge-hook";
  import { isConfigured } from "$lib/firebase";
  import { notificationsStatus } from "$lib/components/notifications/notifications-copy";
  import { update } from "$lib/offline/update.svelte";
  import { consent } from "$lib/stores/consent.svelte";
  import { notifications } from "$lib/stores/notifications.svelte";
  import { cn } from "$lib/utils";
  import type { SettingsCopy } from "$lib/i18n/settings-copy";

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
  let checking = $state(false);

  onMount(() => {
    installPurgeHook(authState);
  });

  const pill =
    "flex w-full items-center justify-between gap-2 rounded-md border px-3 py-1.5 text-xs transition-colors duration-150 text-start";
  const on = "border-accent bg-accent-soft text-fg";
  const off = "border-line-2 text-fg-2 hover:text-fg";

  function toggleAnalytics(): void {
    consent.setAnalytics(!consent.analytics);
  }

  function togglePerformance(): void {
    consent.setPerformance(!consent.performance);
    location.reload();
  }

  async function onCheckUpdates(): Promise<void> {
    if (update.available) {
      update.apply();
      return;
    }
    if (checking) return;
    checking = true;
    try {
      await updated.check();
    } finally {
      checking = false;
    }
  }

  async function onSignOut(): Promise<void> {
    if (logout.pending) return;
    await logout.run();
  }

  const notificationsLabel = $derived(
    notificationsStatus(copy.notificationsStatus)({
      configured: isConfigured,
      supported: notifications.supported,
      permission: notifications.permission,
      subscribed: notifications.subscribed,
    }),
  );
</script>

<Card id={id} tabindex={-1} class="scroll-mt-24">
  <h2 class="text-sm font-semibold text-fg">{heading}</h2>
  <p class="mt-1 text-xs text-fg-3">{copy.intro}</p>

  <div class="mt-3 grid gap-2">
    <button
      type="button"
      aria-pressed={consent.analytics}
      onclick={toggleAnalytics}
      class={cn(pill, consent.analytics ? on : off)}
    >
      <span>{copy.analytics}</span>
      <span class="text-[11px]">{consent.analytics ? copy.on : copy.off}</span>
    </button>
    <div>
      <button
        type="button"
        aria-pressed={consent.performance}
        title={copy.performanceReload}
        onclick={togglePerformance}
        class={cn(pill, consent.performance ? on : off)}
      >
        <span>{copy.performance}</span>
        <span class="text-[11px]">{consent.performance ? copy.on : copy.off}</span>
      </button>
      <p class="mt-1.5 text-[11px] leading-snug text-fg-4">{copy.performanceReload}</p>
    </div>
  </div>

  <hr class="my-3 border-line" />

  <div class="flex items-center justify-between gap-2">
    <span class="text-xs text-fg-3">{copy.notifications}</span>
    <span class="text-end text-[11px] leading-tight text-fg-3">
      {notificationsLabel}
    </span>
  </div>

  <div class="mt-2.5 flex flex-wrap items-center justify-between gap-2" role="status">
    <span class="text-xs text-fg-3">{copy.version}</span>
    <div class="flex items-center gap-2">
      {#if update.available}
        <span class="text-[11px] leading-tight text-fg-3">{copy.updateAvailable}</span>
      {:else if !checking}
        <span class="text-[11px] leading-tight text-fg-3">{copy.upToDate}</span>
      {/if}
      <button
        type="button"
        disabled={checking}
        onclick={onCheckUpdates}
        class={cn(
          "rounded-md border px-3 py-1 text-xs transition-colors duration-150",
          update.available ? on : off,
          checking && "cursor-not-allowed opacity-50",
        )}
      >
        {update.available ? copy.reloadUpdate : copy.checkUpdates}
      </button>
    </div>
  </div>

  <p class="mt-3 text-[11px] leading-snug text-fg-4">{copy.syncNote}</p>

  {#if signedIn}
    <hr class="my-3 border-line" />
    <div class="flex flex-col gap-1.5">
      <Button
        variant="ghost"
        size="sm"
        class="self-start"
        disabled={logout.pending}
        onclick={onSignOut}>{copy.signOut}</Button
      >
      {#if logout.genericError}
        <p role="alert" aria-live="assertive" class="text-xs text-destructive">
          {copy.signOutError}
        </p>
      {/if}
    </div>
  {/if}
</Card>
