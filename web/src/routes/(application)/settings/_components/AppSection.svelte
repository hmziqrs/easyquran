<script lang="ts">
  import { update } from "#lib/offline/update.svelte.js";
  import { cn } from "#lib/utils.js";
  import type { SettingsCopy } from "#lib/i18n/settings-copy.js";

  let {
    id,
    heading,
    copy,
  }: {
    id: string;
    heading: string;
    copy: SettingsCopy["app"];
  } = $props();

  // The beforeinstallprompt payload is not in the DOM lib; only prompt() and
  // userChoice are ever consumed, so a minimal structural type is enough.
  interface InstallPromptEvent extends Event {
    prompt(): Promise<void>;
    userChoice: Promise<{ outcome: "accepted" | "dismissed" }>;
  }

  // display-mode standalone (already running as the installed PWA). Written
  // only inside the client-only $effect below, so SSR renders the default.
  let standalone = $state(false);
  // Captured install prompt; $state.raw (reassigned by identity, never mutated).
  let installEvent = $state.raw<InstallPromptEvent | null>(null);

  const toggle = "rounded-pill border px-3.5 py-2.5 text-caption transition-colors duration-150";
  const on = "border-primary bg-primary-soft text-foreground";
  const off = "border-border-strong text-foreground-secondary hover:border-border hover:text-foreground";

  // Client-only: $effect never runs during SSR, so window/matchMedia access
  // here is safe (same guard shape as the hash-sync effect in +page.svelte).
  $effect(() => {
    const query = window.matchMedia("(display-mode: standalone)");
    standalone = query.matches;
    const onChange = (event: MediaQueryListEvent): void => {
      standalone = event.matches;
    };
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  });

  // Client-only capture: preventDefault keeps the browser's own install
  // mini-infobar down so this row's button owns when the prompt shows.
  $effect(() => {
    const onInstallPrompt = (event: Event): void => {
      event.preventDefault();
      // SAFETY: the beforeinstallprompt event's only consumed surface is prompt()/userChoice.
      installEvent = event as InstallPromptEvent;
    };
    window.addEventListener("beforeinstallprompt", onInstallPrompt);
    return () => window.removeEventListener("beforeinstallprompt", onInstallPrompt);
  });

  function statusLabel(): string {
    if (update.checking) return copy.statusChecking;
    if (update.available) return copy.statusUpdateReady;
    // Up to date needs the controller's handshake answer to match this build;
    // a pending/timed-out handshake stays indeterminate.
    if (update.installedVersion !== null && update.installedVersion === update.latestVersion) {
      return copy.statusUpToDate;
    }
    return copy.statusUnknown;
  }

  function statusClass(): string {
    if (update.available) return "text-[14.5px] font-medium text-primary";
    if (update.checking) return "text-[14.5px] font-medium text-foreground-secondary";
    return "text-[14.5px] font-medium text-foreground";
  }

  function installedDisplay(): string {
    if (update.installedVersion !== null) return update.installedVersion;
    // No controller yet: the page build version is the closest truth.
    return update.latestVersion;
  }

  function onApplyUpdate(): void {
    update.apply();
  }

  async function onCheckUpdates(): Promise<void> {
    await update.checkNow();
  }

  async function onInstall(): Promise<void> {
    const event = installEvent;
    if (!event) return;
    try {
      await event.prompt();
    } finally {
      // prompt() is one-shot per captured event; drop it so the row falls
      // back to the manual-install note instead of a dead button.
      installEvent = null;
    }
  }
</script>

<div id={id} tabindex="-1" class="scroll-mt-24">
  <h2 class="text-[17px] font-medium tracking-[-0.02em] text-foreground">{heading}</h2>
  <p class="mt-1 max-w-[70ch] text-[14.5px] leading-relaxed text-foreground-secondary">{copy.intro}</p>

  <div class="mt-4 divide-y divide-border overflow-hidden rounded-xl border border-border-strong bg-surface">
    <div class="flex flex-wrap items-center justify-between gap-4 px-4 py-3.5 sm:px-5">
      <span class="text-[14.5px] font-medium text-foreground">{copy.installed}</span>
      <div class="min-w-0 text-end">
        <span class="text-caption leading-tight text-muted">{installedDisplay()}</span>
        {#if update.installedVersion === null}
          <p class="mt-0.5 text-caption leading-snug text-muted">{copy.versionNotControlling}</p>
        {/if}
      </div>
    </div>

    <div class="flex flex-wrap items-center justify-between gap-4 px-4 py-3.5 sm:px-5">
      <span class="text-[14.5px] font-medium text-foreground">{copy.latest}</span>
      <span class="shrink-0 text-end text-caption leading-tight text-muted">
        {update.latestVersion}
      </span>
    </div>

    <div class="flex flex-wrap items-center justify-between gap-4 px-4 py-3.5 sm:px-5" role="status">
      <span class={statusClass()}>{statusLabel()}</span>
      {#if update.available}
        <button type="button" onclick={onApplyUpdate} class={cn(toggle, "shrink-0", on)}>
          {copy.reloadUpdate}
        </button>
      {/if}
    </div>

    <div class="flex flex-wrap items-center justify-between gap-4 px-4 py-3.5 sm:px-5">
      <span class="text-[14.5px] font-medium text-foreground">{copy.checkUpdates}</span>
      <button
        type="button"
        disabled={update.checking}
        onclick={onCheckUpdates}
        class={cn(
          toggle,
          "shrink-0",
          update.checking ? on : off,
          update.checking && "cursor-not-allowed opacity-50",
        )}
      >
        {copy.checkUpdates}
      </button>
    </div>

    <div class="flex flex-wrap items-center justify-between gap-4 px-4 py-3.5 sm:px-5">
      {#if standalone}
        <p class="min-w-0 text-caption leading-snug text-muted">{copy.installStandaloneNote}</p>
      {:else if installEvent}
        <span class="text-[14.5px] font-medium text-foreground">{copy.install}</span>
        <button type="button" onclick={onInstall} class={cn(toggle, "shrink-0", on)}>
          {copy.install}
        </button>
      {:else}
        <p class="min-w-0 text-caption leading-snug text-muted">{copy.installBrowserNote}</p>
      {/if}
    </div>
  </div>
</div>
