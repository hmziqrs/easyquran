<script lang="ts">
  import { tick } from "svelte";
  import { getLocale } from "$lib/paraglide/runtime.js";
  import { Card, OfflinePack } from "$lib/components";
  import { isArabicSourceId } from "$lib/data/quran-types";
  import { bakedTranslationCatalogue, findCatalogueEntry } from "$lib/quran/catalogue";
  import { sourceProfile } from "$lib/quran/view/source-profiles";
  import { purgeUserCaches } from "$lib/offline/messages";
  import type { StorageArtifactInfo } from "$lib/quran/protocol";
  import {
    isQuotaHigh,
    isTranslationCapHigh,
    storageReport,
    type DeleteOutcome,
  } from "$lib/stores/storage-report.svelte";
  import { readerSource } from "$lib/stores/reader-settings.svelte";
  import { stackedTranslations } from "$lib/stores/stacked-translations.svelte";
  import { formatBytes } from "$lib/utils";
  import { cn } from "$lib/utils";
  import type { SettingsCopy } from "$lib/i18n/settings-copy";
  import type { UiLocale } from "$lib/i18n/locales";
  import UsageBar from "./UsageBar.svelte";
  import StorageArtifactRow from "./StorageArtifactRow.svelte";

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
  let opfsAbsent = $state(false);
  let hasController = $state(false);
  const usedLabel = $derived(
    usage !== null && quota !== null
      ? copy.usedOf(formatBytes(usage), formatBytes(quota))
      : copy.usage,
  );
  const quotaHigh = $derived(isQuotaHigh(usage, quota));
  const capHigh = $derived(isTranslationCapHigh(layers));

  let confirmingAll = $state(false);
  let busyAll = $state(false);
  let actionNotice = $state<string | null>(null);
  let refocusRemoveAll = $state(false);
  let downloadsHeading = $state<HTMLHeadingElement>();
  let persistHeading = $state<HTMLHeadingElement>();
  let removeAllButton = $state<HTMLButtonElement>();
  let confirmAllButton = $state<HTMLButtonElement>();
  let clearingPages = $state(false);
  let clearedPages = $state(false);
  let persistBusy = $state(false);
  let persistDeclined = $state(false);

  $effect(() => {
    opfsAbsent = !opfsSupported();
    const sw = navigator.serviceWorker;
    const sync = () => {
      hasController = !!sw?.controller;
    };
    sync();
    sw?.addEventListener("controllerchange", sync);
    return () => sw?.removeEventListener("controllerchange", sync);
  });

  $effect(() => {
    report.hydrate();
    return () => report.dispose();
  });

  $effect(() => {
    if (confirmingAll && confirmAllButton) confirmAllButton.focus();
  });

  $effect(() => {
    if (refocusRemoveAll && removeAllButton) {
      removeAllButton.focus();
      refocusRemoveAll = false;
    }
  });

  function artifactName(sourceId: string): string {
    if (isArabicSourceId(sourceId)) {
      return copy.scripts[sourceProfile(sourceId).script];
    }
    const entry = findCatalogueEntry(catalogue, sourceId);
    if (entry && entry.kind === "translation") return entry.entry.name;
    return sourceId;
  }

  function storeLabelFor(store: StorageArtifactInfo["store"]): string {
    if (store === "opfs") return copy.stores.opfs;
    if (store === "session") return copy.stores.memory;
    return copy.stores.idb;
  }

  function localizedLanguageName(code: string, fallback: string): string {
    try {
      return new Intl.DisplayNames([locale], { type: "language" }).of(code) ?? fallback;
    } catch {
      return fallback;
    }
  }

  function artifactLanguage(sourceId: string): string | null {
    if (isArabicSourceId(sourceId)) return null;
    const entry = findCatalogueEntry(catalogue, sourceId);
    if (entry && entry.kind === "translation") {
      return localizedLanguageName(entry.entry.languageCode, entry.entry.language);
    }
    return null;
  }

  async function removeOne(artifactId: string): Promise<DeleteOutcome> {
    const outcome = await report.deleteArtifact(artifactId);
    if (outcome === "ok") {
      actionNotice = copy.removed(artifactName(artifactId));
      await tick();
      downloadsHeading?.focus();
    }
    return outcome;
  }

  function cancelConfirmAll() {
    if (busyAll) return;
    confirmingAll = false;
    refocusRemoveAll = true;
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && confirmingAll) {
      event.stopPropagation();
      cancelConfirmAll();
    }
  }

  async function confirmRemoveAll() {
    if (busyAll) return;
    busyAll = true;
    const targets = translationArtifacts.filter((a) => !inUseIds.has(a.id)).length;
    const result = await report.clearAllTranslations([...inUseIds]);
    busyAll = false;
    confirmingAll = false;
    const removed = targets - result.failures;
    if (result.freedBytes > 0) actionNotice = copy.freed(formatBytes(result.freedBytes));
    else if (removed > 0) actionNotice = copy.removedAll;
    else actionNotice = copy.empty;
    if (result.failures > 0) actionNotice += ` · ${copy.busyError}`;
    await tick();
    downloadsHeading?.focus();
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
    const granted = await report.requestPersist();
    persistBusy = false;
    if (granted) {
      persistDeclined = false;
      persistHeading?.focus();
    } else {
      persistDeclined = true;
    }
  }
</script>

<Card id={id} tabindex={-1} class="scroll-mt-24">
  <h2 class="text-base font-semibold text-fg">{heading}</h2>
  <p class="mt-1 text-sm text-fg-2">{copy.intro}</p>

  {#if phase === "boot"}
    <p class="mt-4 text-xs text-fg-3" role="status">{copy.loading}</p>
  {:else if phase === "error"}
    <div class="mt-4 grid gap-2">
      <p class="text-xs text-fg-3">{copy.error}</p>
      <div>
        <button
          type="button"
          onclick={() => report.refresh()}
          class="rounded-md border border-line-2 px-2.5 py-1.5 text-sm text-fg-2 transition-colors hover:text-fg"
        >
          {copy.retry}
        </button>
      </div>
    </div>
  {:else}
    <section class="mt-4" aria-label={copy.usage}>
      {#if quota !== null}
        <UsageBar layers={layers} labels={copy.layers} quotaBytes={quota} {usedLabel} />
        <p class="mt-1.5 text-xs text-fg-2 tabular-nums">{usedLabel}</p>
      {:else}
        <UsageBar layers={layers} labels={copy.layers} quotaBytes={null} {usedLabel} />
        <p class="mt-1.5 text-xs text-fg-2">{copy.usage}</p>
      {/if}
      {#if quotaHigh}
        <p class="mt-1.5 text-xs text-red-400">{copy.quotaWarning}</p>
      {/if}
      {#if capHigh}
        <p class="mt-1.5 text-xs text-fg-3">{copy.capNote}</p>
      {/if}
      <p class="mt-1.5 text-xs leading-snug text-fg-4">{copy.estimateNote}</p>
    </section>

    <section class="mt-5" aria-label={copy.persistHeading}>
      <h3 class="text-sm font-medium text-fg-2" tabindex="-1" bind:this={persistHeading}>
        {copy.persistHeading}
      </h3>
      <p
        aria-live="polite"
        class={cn("mt-1 text-xs leading-snug", report.persisted === false ? "text-fg-4" : "text-fg-3")}
      >
        {#if report.persisted === true}
          {copy.persistGranted}
        {:else if persistDeclined && report.persisted === false}
          {copy.persistDeclined}
        {:else if report.persisted === false}
          {copy.persistDenied}
        {:else}
          {copy.persistUnavailable}
        {/if}
      </p>
      {#if report.persisted === false}
        <button
          type="button"
          disabled={persistBusy}
          onclick={requestPersist}
          class="mt-1.5 rounded-md border border-line-2 px-2.5 py-1.5 text-sm text-fg-2 transition-colors hover:text-fg disabled:cursor-not-allowed disabled:opacity-50"
        >
          {copy.persistRequest}
        </button>
      {/if}
    </section>

    {#if opfsAbsent}
      <p class="mt-4 rounded-md border border-line bg-bg-2 px-3 py-2 text-xs text-fg-3">
        {copy.opfsAbsent}
      </p>
    {/if}

    {#if arabicArtifacts.length > 0}
      <section class="mt-5" aria-label={copy.requiredGroup}>
        <h3 class="text-sm font-medium text-fg-2">{copy.requiredGroup}</h3>
        <p class="mt-1 text-xs leading-snug text-fg-4">{copy.requiredNote}</p>
        <ul class="mt-1.5">
          {#each arabicArtifacts as artifact (artifact.id)}
            <li>
              <div class="flex items-center gap-2 py-1.5">
                <div class="min-w-0 flex-1">
                  <div class="flex flex-wrap items-center gap-1.5">
                    <span class="truncate text-xs text-fg">{artifactName(artifact.id)}</span>
                    <span
                      class="rounded border border-line-2 px-1.5 py-0.5 text-xs leading-none text-fg-3"
                    >
                      {storeLabelFor(artifact.store)}
                    </span>
                  </div>
                  <div class="text-xs text-fg-4">{formatBytes(artifact.sizeBytes)}</div>
                </div>
              </div>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    <section class="mt-5" aria-label={copy.downloadsHeading}>
      <h3 class="text-sm font-medium text-fg-2" tabindex="-1" bind:this={downloadsHeading}>
        {copy.downloadsHeading}
      </h3>
      {#if translationArtifacts.length === 0}
        <p class="mt-1.5 text-xs text-fg-4">{copy.empty}</p>
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
      {/if}
      <div class="mt-3 flex flex-wrap items-center gap-2" aria-live="polite">
        {#if confirmingAll}
          <span class="text-xs text-fg-3">{copy.removeAllConfirm}</span>
          <button
            type="button"
            bind:this={confirmAllButton}
            onclick={confirmRemoveAll}
            onkeydown={onKeydown}
            class="rounded-md border border-red-500/60 bg-red-500/10 px-2.5 py-1.5 text-sm text-red-400 transition-colors hover:bg-red-500/20"
          >
            {copy.removeConfirmAction}
          </button>
          <button
            type="button"
            onclick={cancelConfirmAll}
            onkeydown={onKeydown}
            class="rounded-md border border-line-2 px-2.5 py-1.5 text-sm text-fg-2 transition-colors hover:text-fg"
          >
            {copy.removeCancel}
          </button>
        {:else}
          {#if translationArtifacts.length > 0}
            <button
              type="button"
              bind:this={removeAllButton}
              onclick={() => (confirmingAll = true)}
              class={cn(
                "rounded-md border border-red-500/50 px-2.5 py-1.5 text-sm text-red-400 transition-colors hover:bg-red-500/10",
              )}
            >
              {copy.removeAll}
            </button>
          {/if}
          {#if actionNotice}
            <span class="text-xs text-fg-3">{actionNotice}</span>
          {/if}
        {/if}
      </div>
    </section>

    <section class="mt-5" aria-label={copy.offlinePack.heading}>
      <OfflinePack copy={copy.offlinePack} headingTag="h3" />
      <p class="mt-1.5 text-xs leading-snug text-fg-4">{copy.offlinePackNote}</p>
    </section>

    <section class="mt-5">
      <button
        type="button"
        disabled={!hasController || clearingPages}
        title={hasController ? undefined : copy.clearPagesUnavailable}
        onclick={clearPages}
        class="rounded-md border border-line-2 px-2.5 py-1.5 text-sm text-fg-2 transition-colors hover:text-fg disabled:cursor-not-allowed disabled:opacity-50"
      >
        {copy.clearPages}
      </button>
      {#if !hasController}
        <span class="sr-only">{copy.clearPagesUnavailable}</span>
      {/if}
      {#if clearedPages}
        <span class="text-xs text-fg-3" aria-live="polite">{copy.clearPagesDone}</span>
      {/if}
    </section>

    <p class="mt-5 text-xs leading-snug text-fg-4">{copy.retentionNote}</p>
  {/if}
</Card>
