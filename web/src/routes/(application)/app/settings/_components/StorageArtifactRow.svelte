<script lang="ts">
  import type { StorageArtifactInfo } from "$lib/quran/protocol";
  import type { SettingsCopy } from "$lib/i18n/settings-copy";
  import type { UiLocale } from "$lib/i18n/locales";
  import type { DeleteOutcome } from "$lib/stores/storage-report.svelte";
  import { cn, formatBytes } from "$lib/utils";

  const MINUTE_MS = 60_000;
  const HOUR_MS = 60 * MINUTE_MS;
  const DAY_MS = 24 * HOUR_MS;
  const MONTH_MS = 30 * DAY_MS;

  function relativeLastUsed(when: number, locale: UiLocale): string {
    const rtf = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
    const delta = when - Date.now();
    const abs = Math.abs(delta);
    if (abs < MINUTE_MS) return rtf.format(Math.round(delta / 1000), "second");
    if (abs < HOUR_MS) return rtf.format(Math.round(delta / MINUTE_MS), "minute");
    if (abs < DAY_MS) return rtf.format(Math.round(delta / HOUR_MS), "hour");
    if (abs < MONTH_MS) return rtf.format(Math.round(delta / DAY_MS), "day");
    return rtf.format(Math.round(delta / MONTH_MS), "month");
  }

  const badge = "rounded border border-line-2 px-1.5 py-0.5 text-[10px] leading-none text-fg-3";
  const actionBtn =
    "rounded-md border px-2.5 py-1 text-[11px] transition-colors duration-150 disabled:cursor-not-allowed disabled:opacity-50";

  let {
    artifact,
    name,
    language,
    inUse = false,
    copy,
    locale,
    onremove,
  }: {
    artifact: StorageArtifactInfo;
    name: string;
    language: string | null;
    inUse?: boolean;
    copy: SettingsCopy["storage"];
    locale: UiLocale;
    onremove: (id: string) => Promise<DeleteOutcome>;
  } = $props();

  let confirming = $state(false);
  let busy = $state(false);
  let errorText = $state<string | null>(null);
  let refocusRemove = $state(false);
  let removeButton = $state<HTMLButtonElement>();
  let confirmButton = $state<HTMLButtonElement>();

  const storeLabel = $derived(
    artifact.store === "opfs" ? copy.stores.opfs : copy.stores.idb,
  );
  const lastUsedLabel = $derived(
    artifact.lastUsed === null
      ? copy.neverUsed
      : copy.lastUsed(relativeLastUsed(artifact.lastUsed, locale)),
  );

  $effect(() => {
    if (confirming && confirmButton) confirmButton.focus();
  });

  $effect(() => {
    if (refocusRemove && removeButton) {
      removeButton.focus();
      refocusRemove = false;
    }
  });

  function startConfirm() {
    errorText = null;
    confirming = true;
  }

  function cancelConfirm() {
    confirming = false;
    refocusRemove = true;
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && confirming) {
      event.stopPropagation();
      cancelConfirm();
    }
  }

  async function confirmRemove() {
    busy = true;
    const outcome = await onremove(artifact.id);
    busy = false;
    confirming = false;
    if (outcome === "ok") {
      errorText = null;
      return;
    }
    if (outcome === "arabic") errorText = copy.arabicError;
    else if (outcome === "busy") errorText = copy.busyError;
    else errorText = copy.error;
    refocusRemove = true;
  }
</script>

<div class="flex items-center gap-2 py-1.5">
  <div class="min-w-0 flex-1">
    <div class="flex flex-wrap items-center gap-1.5">
      <span class="truncate text-xs text-fg">{name}</span>
      {#if language}
        <span class={badge}>{language}</span>
      {/if}
      <span class={badge}>{storeLabel}</span>
      {#if inUse}
        <span
          class="rounded border border-accent-line bg-accent-soft px-1.5 py-0.5 text-[10px] leading-none text-fg-2"
        >
          {copy.inUse}
        </span>
      {/if}
    </div>
    <div class="text-[11px] text-fg-4">{formatBytes(artifact.sizeBytes)} · {lastUsedLabel}</div>
  </div>

  <span class="sr-only" aria-live="polite">{errorText}</span>
  <div class="flex items-center gap-1.5" aria-live="polite">
    {#if confirming}
      <span class="text-[11px] text-fg-3">{copy.removeConfirmTitle(name)}</span>
      <button
        type="button"
        bind:this={confirmButton}
        disabled={busy}
        onclick={confirmRemove}
        onkeydown={onKeydown}
        class={cn(actionBtn, "border-red-500/60 bg-red-500/10 text-red-400 hover:bg-red-500/20")}
      >
        {copy.removeConfirmAction}
      </button>
      <button
        type="button"
        disabled={busy}
        onclick={cancelConfirm}
        onkeydown={onKeydown}
        class={cn(actionBtn, "border-line-2 text-fg-2 hover:text-fg")}
      >
        {copy.removeCancel}
      </button>
    {:else}
      {#if errorText}
        <span class="max-w-40 truncate text-[11px] text-red-400" title={errorText}>
          {errorText}
        </span>
      {/if}
      <button
        type="button"
        bind:this={removeButton}
        disabled={inUse}
        title={inUse ? copy.inUse : copy.removeConfirmTitle(name)}
        aria-label={copy.removeConfirmTitle(name)}
        onclick={startConfirm}
        class={cn(actionBtn, "border-line-2 text-fg-3 hover:border-line hover:text-fg")}
      >
        {copy.remove}
      </button>
    {/if}
  </div>
</div>
