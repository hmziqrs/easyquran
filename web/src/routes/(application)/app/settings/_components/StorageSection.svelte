<script lang="ts">
  import { browser } from "$app/environment";
  import { getLocale } from "$lib/paraglide/runtime.js";
  import { Card, OfflinePack, OfflinePackBar } from "$lib/components";
  import { isArabicSourceId, QuranScript } from "$lib/data/quran-types";
  import { bakedTranslationCatalogue, findCatalogueEntry } from "$lib/quran/catalogue";
  import { sourceProfile } from "$lib/quran/view/source-profiles";
  import { purgeUserCaches } from "$lib/offline/messages";
  import {
    isQuotaHigh,
    isTranslationCapHigh,
    storageReport,
  } from "$lib/stores/storage-report.svelte";
  import { readerSource } from "$lib/stores/reader-settings.svelte";
  import { stackedTranslations } from "$lib/stores/stacked-translations.svelte";
  import { formatBytes } from "$lib/utils";
  import { cn } from "$lib/utils";
  import type { SettingsCopy } from "$lib/i18n/settings-copy";
  import type { UiLocale } from "$lib/i18n/locales";
  import UsageBar from "./UsageBar.svelte";
  import StorageArtifactRow from "./StorageArtifactRow.svelte";

  const SCRIPT_LABELS = {
    [QuranScript.Uthmani]: "Uthmani",
    [QuranScript.SimpleClean]: "Simple-clean",
    [QuranScript.IndoPak]: "IndoPak",
    [QuranScript.Tajweed]: "Tajweed",
    [QuranScript.Translation]: "Translation",
  } satisfies Readonly<Record<QuranScript, string>>;

  let {
    id,
    heading,
    copy,
  }: {
    id: string;
    heading: string;
    copy: SettingsCopy["storage"];
  } = $props();

  const catalogue = bakedTranslationCatalogue();
  // SAFETY: paraglide is compiled for exactly the UI locales (en/ar in messages/), so getLocale() only ever returns a UiLocale at runtime.
  const locale = getLocale() as UiLocale;

  const report = storageReport;

  const phase = $derived(report.phase);
  const artifacts = $derived(report.artifacts);
  const layers = $derived(report.layers);
  const quota = $derived(report.quota);
  const usage = $derived(report.usage);

  const arabicArtifacts = $derived(artifacts.filter((a) => isArabicSourceId(a.id)));
  const translationArtifacts = $derived(
    artifacts
      .filter((a) => !isArabicSourceId(a.id))
      .toSorted((a, b) => b.sizeBytes - a.sizeBytes),
  );
  const inUseIds = $derived(
    new Set([...(readerSource.sourceId ? [readerSource.sourceId] : []), ...stackedTranslations.ids]),
  );
  function opfsSupported(): boolean {
    // eslint-disable-next-line anti-slop/no-runtime-typeof -- capability probe mirroring hasOpfs(); `navigator` is absent in some runtimes and getDirectory is an optional member, so typeof is the honest check
    return typeof navigator !== "undefined" && !!navigator.storage?.getDirectory;
  }
  const opfsAbsent = $derived(browser && !opfsSupported());
  const hasController = $derived(
    browser && !!navigator.serviceWorker?.controller,
  );
  const usedLabel = $derived(
    usage !== null && quota !== null
      ? copy.usedOf(formatBytes(usage), formatBytes(quota))
      : copy.usage,
  );
  const quotaHigh = $derived(isQuotaHigh(usage, quota));
  const capHigh = $derived(isTranslationCapHigh(layers));

  let confirmingAll = $state(false);
  let busyAll = $state(false);
  let freedSummary = $state<string | null>(null);
  let removeAllButton = $state<HTMLButtonElement>();
  let confirmAllButton = $state<HTMLButtonElement>();
  let clearingPages = $state(false);
  let clearedPages = $state(false);
  let persistBusy = $state(false);

  $effect(() => {
    report.hydrate();
  });

  $effect(() => {
    if (confirmingAll && confirmAllButton) confirmAllButton.focus();
  });

  function artifactName(sourceId: string): string {
    if (isArabicSourceId(sourceId)) {
      return SCRIPT_LABELS[sourceProfile(sourceId).script];
    }
    const entry = findCatalogueEntry(catalogue, sourceId);
    if (entry && entry.kind === "translation") return entry.entry.name;
    return sourceId;
  }

  function artifactLanguage(sourceId: string): string | null {
    if (isArabicSourceId(sourceId)) return null;
    const entry = findCatalogueEntry(catalogue, sourceId);
    if (entry && entry.kind === "translation") return entry.entry.language;
    return null;
  }

  async function removeOne(artifactId: string) {
    return report.deleteArtifact(artifactId);
  }

  function cancelConfirmAll() {
    confirmingAll = false;
    removeAllButton?.focus();
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && confirmingAll) {
      event.stopPropagation();
      cancelConfirmAll();
    }
  }

  async function confirmRemoveAll() {
    busyAll = true;
    const result = await report.clearAllTranslations([...inUseIds]);
    busyAll = false;
    confirmingAll = false;
    freedSummary = copy.freed(formatBytes(result.freedBytes));
    removeAllButton?.focus();
  }

  async function clearPages() {
    clearingPages = true;
    await purgeUserCaches();
    clearingPages = false;
    clearedPages = true;
    await report.refresh();
  }

  async function requestPersist() {
    persistBusy = true;
    await report.requestPersist();
    persistBusy = false;
  }
</script>

<svelte:window onkeydown={onKeydown} />

<Card id={id} class="scroll-mt-24">
  <h2 class="text-sm font-semibold text-fg">{heading}</h2>
  <p class="mt-1 text-xs text-fg-3">{copy.intro}</p>

  {#if phase === "boot"}
    <p class="mt-4 text-xs text-fg-3" role="status">{copy.loading}</p>
  {:else if phase === "error"}
    <div class="mt-4 grid gap-2">
      <p class="text-xs text-fg-3">{copy.error}</p>
      <div>
        <button
          type="button"
          onclick={() => report.refresh()}
          class="rounded-md border border-line-2 px-2.5 py-1 text-[11px] text-fg-2 transition-colors hover:text-fg"
        >
          {copy.retry}
        </button>
      </div>
    </div>
  {:else}
    <section class="mt-4" aria-label={copy.usage}>
      {#if quota !== null}
        <UsageBar layers={layers} labels={copy.layers} quotaBytes={quota} {usedLabel} />
        <p class="mt-1.5 text-[11px] text-fg-2 tabular-nums">{usedLabel}</p>
      {:else}
        <UsageBar layers={layers} labels={copy.layers} quotaBytes={null} {usedLabel} />
        <p class="mt-1.5 text-[11px] text-fg-2">{copy.usage}</p>
      {/if}
      {#if quotaHigh}
        <p class="mt-1.5 text-[11px] text-red-400">{copy.quotaWarning}</p>
      {/if}
      {#if capHigh}
        <p class="mt-1.5 text-[11px] text-fg-3">{copy.capNote}</p>
      {/if}
      <p class="mt-1.5 text-[11px] leading-snug text-fg-4">{copy.estimateNote}</p>
    </section>

    <section class="mt-5" aria-label={copy.persistHeading}>
      <h3 class="text-xs font-medium text-fg-2">{copy.persistHeading}</h3>
      <p class={cn("mt-1 text-[11px] leading-snug", report.persisted === false ? "text-fg-4" : "text-fg-3")}>
        {#if report.persisted === true}{copy.persistGranted}{:else if report.persisted === false}
          {copy.persistDenied}
        {:else}{copy.loading}{/if}
      </p>
      {#if report.persisted === false}
        <button
          type="button"
          disabled={persistBusy}
          onclick={requestPersist}
          class="mt-1.5 rounded-md border border-line-2 px-2.5 py-1 text-[11px] text-fg-2 transition-colors hover:text-fg disabled:cursor-not-allowed disabled:opacity-50"
        >
          {copy.persistRequest}
        </button>
      {/if}
    </section>

    {#if opfsAbsent}
      <p class="mt-4 rounded-md border border-line bg-bg-2 px-3 py-2 text-[11px] text-fg-3">
        {copy.opfsAbsent}
      </p>
    {/if}

    {#if arabicArtifacts.length > 0}
      <section class="mt-5" aria-label={copy.requiredGroup}>
        <h3 class="text-xs font-medium text-fg-2">{copy.requiredGroup}</h3>
        <p class="mt-1 text-[11px] leading-snug text-fg-4">{copy.requiredNote}</p>
        <ul class="mt-1.5">
          {#each arabicArtifacts as artifact (artifact.id)}
            <li>
              <div class="flex items-center gap-2 py-1.5">
                <div class="min-w-0 flex-1">
                  <div class="flex flex-wrap items-center gap-1.5">
                    <span class="truncate text-xs text-fg">{artifactName(artifact.id)}</span>
                    <span
                      class="rounded border border-line-2 px-1.5 py-0.5 text-[10px] leading-none text-fg-3"
                    >
                      {artifact.store === "opfs" ? copy.stores.opfs : copy.stores.idb}
                    </span>
                  </div>
                  <div class="text-[11px] text-fg-4">{formatBytes(artifact.sizeBytes)}</div>
                </div>
              </div>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    <section class="mt-5" aria-label={copy.downloadsHeading}>
      <h3 class="text-xs font-medium text-fg-2">{copy.downloadsHeading}</h3>
      {#if translationArtifacts.length === 0}
        <p class="mt-1.5 text-[11px] text-fg-4">{copy.empty}</p>
      {:else}
        <ul class="mt-0.5 divide-y divide-line">
          {#each translationArtifacts as artifact (artifact.id)}
            <li>
              <StorageArtifactRow
                {artifact}
                name={artifactName(artifact.id)}
                language={artifactLanguage(artifact.id)}
                inUse={inUseIds.has(artifact.id)}
                {copy}
                {locale}
                onremove={removeOne}
              />
            </li>
          {/each}
        </ul>
        <div class="mt-3 flex flex-wrap items-center gap-2" aria-live="polite">
          {#if confirmingAll}
            <span class="text-[11px] text-fg-3">{copy.removeAllConfirm}</span>
            <button
              type="button"
              bind:this={confirmAllButton}
              disabled={busyAll}
              onclick={confirmRemoveAll}
              class="rounded-md border border-red-500/60 bg-red-500/10 px-2.5 py-1 text-[11px] text-red-400 transition-colors hover:bg-red-500/20 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {copy.removeConfirmAction}
            </button>
            <button
              type="button"
              disabled={busyAll}
              onclick={cancelConfirmAll}
              class="rounded-md border border-line-2 px-2.5 py-1 text-[11px] text-fg-2 transition-colors hover:text-fg"
            >
              {copy.removeCancel}
            </button>
          {:else}
            <button
              type="button"
              bind:this={removeAllButton}
              onclick={() => (confirmingAll = true)}
              class={cn(
                "rounded-md border border-red-500/50 px-2.5 py-1 text-[11px] text-red-400 transition-colors hover:bg-red-500/10",
              )}
            >
              {copy.removeAll}
            </button>
            {#if freedSummary}
              <span class="text-[11px] text-fg-3">{freedSummary}</span>
            {/if}
          {/if}
        </div>
      {/if}
    </section>

    <section class="mt-5">
      <OfflinePack copy={copy.offlinePack} />
      <p class="mt-1.5 text-[11px] leading-snug text-fg-4">{copy.offlinePackNote}</p>
    </section>

    <OfflinePackBar />

    <section class="mt-5">
      <button
        type="button"
        disabled={!hasController || clearingPages}
        title={hasController ? undefined : copy.clearPagesUnavailable}
        onclick={clearPages}
        class="rounded-md border border-line-2 px-2.5 py-1 text-[11px] text-fg-2 transition-colors hover:text-fg disabled:cursor-not-allowed disabled:opacity-50"
      >
        {copy.clearPages}
      </button>
      <span class="sr-only" aria-live="polite">
        {#if clearedPages}{copy.clearPagesDone}{/if}
      </span>
    </section>

    <p class="mt-5 text-[11px] leading-snug text-fg-4">{copy.retentionNote}</p>
  {/if}
</Card>
