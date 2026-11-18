<script lang="ts">
  import { QURAN } from "#lib/config/site.js";
  import { isArabicSourceId, QuranSourceId } from "#lib/data/quran-types.js";
  import { OfflinePack } from "#lib/components/status/index.js";
  import { downloadDismissal } from "#lib/components/status/download-dismissal.svelte.js";
  import { quranRunFraction, toPercent } from "#lib/components/status/download-progress.js";
  import { getDownloadsCopy, type DownloadRowState } from "#lib/i18n/downloads-copy.js";
  import { getSettingsCopy } from "#lib/i18n/settings-copy.js";
  import { online } from "#lib/offline/online.svelte.js";
  import { offline } from "#lib/offline/offline-store.svelte.js";
  import { update } from "#lib/offline/update.svelte.js";
  import { TRANSLATION_CATALOGUE_BY_ID } from "#lib/quran/catalogue.js";
  import { sourceProfile } from "#lib/quran/view/source-profiles.js";
  import { quran } from "#lib/stores/quran.svelte.js";
  import { stackedTranslations } from "#lib/stores/stacked-translations.svelte.js";
  import { storageReport, type DeleteOutcome } from "#lib/stores/storage-report.svelte.js";
  import { cn, formatBytes } from "#lib/utils.js";
  import UsageBar from "../settings/_components/UsageBar.svelte";
  import StorageArtifactRow from "../settings/_components/StorageArtifactRow.svelte";

  const copy = getDownloadsCopy();
  const settings = getSettingsCopy(copy.locale);
  const storageCopy = settings.storage;
  const report = storageReport;

  interface DbRow {
    readonly id: string;
    readonly name: string;
    readonly state: DownloadRowState;
    readonly sizeBytes: number;
    readonly loaded: number;
  }

  let hasController = $state(false);

  $effect(() => {
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

  const quranActive = $derived(
    quran.download !== null && quran.status !== "ready" && quran.status !== "error",
  );
  const packWorking = $derived(offline.status === "downloading" || offline.status === "staging");
  const storedById = $derived(new Map(report.artifacts.map((a) => [a.id, a])));

  function sourceName(sourceId: string): string {
    if (isArabicSourceId(sourceId)) {
      // Both Uthmani editions share QuranScript.Uthmani; resolve per profile so the rows get distinct labels.
      if (sourceId === QuranSourceId.AnnotatedUthmani) return storageCopy.annotatedUthmani;
      return storageCopy.scripts[sourceProfile(sourceId).script];
    }
    return TRANSLATION_CATALOGUE_BY_ID.get(sourceId)?.name ?? sourceId;
  }

  function activeArabicIndex(): number {
    const d = quran.download;
    if (!quranActive || !d || !isArabicSourceId(d.script)) return -1;
    return QURAN.scripts.findIndex((s) => s.id === d.script);
  }

  function arabicState(index: number, activeIndex: number, stored: boolean): DownloadRowState {
    // Arabic scripts download in QURAN.scripts order, so earlier ones are already done.
    if (activeIndex >= 0) {
      if (index === activeIndex) return "downloading";
      if (stored || index < activeIndex) return "ready";
      return "queued";
    }
    if (stored) return "ready";
    if (report.phase === "ready") return "missing";
    return "checking";
  }

  const arabicRows = $derived.by((): DbRow[] => {
    const activeIndex = activeArabicIndex();
    return QURAN.scripts.map((script, index) => {
      const stored = storedById.get(script.id);
      const state = arabicState(index, activeIndex, stored !== undefined);
      return {
        id: script.id,
        name: sourceName(script.id),
        state,
        sizeBytes: stored?.sizeBytes ?? script.sizeBytes,
        loaded: state === "downloading" ? (quran.download?.loaded ?? 0) : 0,
      };
    });
  });

  const readyCount = $derived(arabicRows.filter((row) => row.state === "ready").length);

  function overallPct(): number {
    const d = quran.download;
    if (quranActive && d && isArabicSourceId(d.script)) {
      return toPercent(quranRunFraction(d, QURAN.scripts));
    }
    if (arabicRows.length === 0) return 0;
    return Math.round((readyCount / arabicRows.length) * 100);
  }

  const overall = $derived(overallPct());

  /** The translation downloading right now, when it is not stored yet. */
  const pendingTranslation = $derived.by((): DbRow | null => {
    const d = quran.download;
    if (!quranActive || !d || isArabicSourceId(d.script)) return null;
    return {
      id: d.script,
      name: sourceName(d.script),
      state: "downloading",
      sizeBytes: d.total,
      loaded: d.loaded,
    };
  });

  const storedTranslations = $derived(
    report.artifacts
      .filter((a) => !isArabicSourceId(a.id) && a.id !== pendingTranslation?.id)
      .toSorted((a, b) => b.sizeBytes - a.sizeBytes),
  );
  const inUseIds = $derived(new Set(stackedTranslations.ids));

  const popupHidden = $derived(
    (quranActive && downloadDismissal.quranHidden) || (packWorking && downloadDismissal.packHidden),
  );

  function rowPct(row: DbRow): number {
    if (row.sizeBytes <= 0) return 0;
    return Math.min(100, Math.round((row.loaded / row.sizeBytes) * 100));
  }

  function rowDetail(row: DbRow): string {
    if (row.state === "downloading") {
      return copy.progress(rowPct(row), formatBytes(row.loaded), formatBytes(row.sizeBytes));
    }
    return formatBytes(row.sizeBytes);
  }

  function localizedLanguageName(code: string, fallback: string): string {
    try {
      return new Intl.DisplayNames([copy.locale], { type: "language" }).of(code) ?? fallback;
    } catch {
      return fallback;
    }
  }

  function artifactLanguage(sourceId: string): string | null {
    const entry = TRANSLATION_CATALOGUE_BY_ID.get(sourceId);
    if (!entry) return null;
    return localizedLanguageName(entry.languageCode, entry.language);
  }

  function removeTranslation(artifactId: string): Promise<DeleteOutcome> {
    return report.deleteArtifact(artifactId);
  }

  const STATE_BADGE = {
    queued: "border-border-strong text-muted",
    downloading: "border-primary-line bg-primary-soft text-foreground",
    ready: "border-border-strong text-foreground-secondary",
    missing: "border-border-strong text-muted",
    checking: "border-border-strong text-muted",
  } satisfies Readonly<Record<DownloadRowState, string>>;

  const usedLabel = $derived(
    report.usage !== null && report.quota !== null
      ? storageCopy.usedOf(formatBytes(report.usage))
      : storageCopy.usage,
  );

  const card = "divide-y divide-border overflow-hidden rounded-xl border border-border-strong bg-surface";
  const sectionHeading = "text-[17px] font-medium tracking-[-0.02em] text-foreground";
  const badge = "rounded border px-1.5 py-0.5 text-xs leading-none";
  const quiet =
    "rounded-pill border border-border-strong px-3.5 py-2 text-caption text-foreground-secondary transition-colors hover:border-border hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring";
</script>

<svelte:head>
  <title>{copy.title} · EasyQuran</title>
</svelte:head>

{#snippet progressBar(pct: number, label: string)}
  <div
    role="progressbar"
    aria-label={label}
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={pct}
    class="h-1.5 w-full overflow-hidden rounded-pill border border-border bg-background-subtle"
  >
    <div
      class="h-full rounded-pill bg-primary transition-[width] duration-150 ease-out"
      style:width={`${pct}%`}
    ></div>
  </div>
{/snippet}

{#snippet dbRow(row: DbRow)}
  <div class="grid gap-2 px-4 py-3.5 sm:px-5">
    <div class="flex items-center justify-between gap-4">
      <div class="flex min-w-0 flex-wrap items-center gap-1.5">
        <span class="truncate text-[14.5px] text-foreground">{row.name}</span>
        <span class={cn(badge, STATE_BADGE[row.state])}>{copy.states[row.state]}</span>
      </div>
      <span class="shrink-0 text-end text-caption tabular-nums text-muted">{rowDetail(row)}</span>
    </div>
    {#if row.state === "downloading"}
      {@render progressBar(rowPct(row), row.name)}
    {/if}
  </div>
{/snippet}

<div lang={copy.locale} dir={copy.direction} class="mx-auto max-w-[860px] px-6 pt-5 pb-10 sm:px-7 sm:pt-6 sm:pb-12">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <h1 class="text-[28px] font-medium leading-tight tracking-[-0.02em] text-foreground">{copy.title}</h1>
    <span
      class="inline-flex items-center gap-2 rounded-pill border border-border-strong bg-surface px-3 py-1 text-caption text-foreground-secondary"
      role="status"
    >
      <span
        class={cn("inline-block size-2 rounded-full", online.online ? "bg-success" : "bg-[var(--fg-4)]")}
        aria-hidden="true"
      ></span>
      {online.online ? copy.online : copy.offline}
    </span>
  </div>
  <p class="mt-1 max-w-[70ch] text-[14.5px] leading-relaxed text-foreground-secondary">{copy.intro}</p>

  {#if popupHidden}
    <div class="mt-4 flex flex-wrap items-center gap-3 rounded-xl border border-border-strong bg-surface px-4 py-3 sm:px-5">
      <span class="text-caption text-muted">{copy.popupHidden}</span>
      <button type="button" class={quiet} onclick={() => downloadDismissal.show()}>{copy.popupShow}</button>
    </div>
  {/if}

  <section class="mt-6" aria-labelledby="downloads-quran">
    <h2 id="downloads-quran" class={sectionHeading}>{copy.quranHeading}</h2>
    <p class="mt-1 max-w-[70ch] text-caption leading-snug text-muted">{copy.quranNote}</p>

    <div class={cn(card, "mt-3")}>
      <div class="grid gap-2 px-4 py-3.5 sm:px-5">
        <div class="flex items-center justify-between gap-4">
          <span class="text-[14.5px] font-medium text-foreground">
            {copy.overall(readyCount, arabicRows.length)}
          </span>
          <span class="text-caption tabular-nums text-muted">{overall}%</span>
        </div>
        {@render progressBar(overall, copy.overallLabel)}
        {#if quran.status === "error"}
          <p class="text-caption text-danger" role="alert">{copy.quranError}</p>
        {:else if quran.status === "idle" && report.phase === "boot" && !quranActive}
          <p class="text-caption text-muted">{copy.quranIdle}</p>
        {/if}
      </div>
      {#each arabicRows as row (row.id)}
        {@render dbRow(row)}
      {/each}
    </div>

    <h3 class="mt-5 text-[14.5px] font-medium text-foreground">{copy.translationsHeading}</h3>
    <div class={cn(card, "mt-2")}>
      {#if pendingTranslation}
        {@render dbRow(pendingTranslation)}
      {/if}
      {#each storedTranslations as artifact (artifact.id)}
        <div class="px-4 sm:px-5">
          <StorageArtifactRow
            {artifact}
            name={sourceName(artifact.id)}
            language={artifactLanguage(artifact.id)}
            inUse={inUseIds.has(artifact.id)}
            copy={storageCopy}
            locale={copy.locale}
            onremove={removeTranslation}
          />
        </div>
      {/each}
      {#if !pendingTranslation && storedTranslations.length === 0}
        <p class="px-4 py-3.5 text-caption text-muted sm:px-5">
          {report.phase === "ready" ? copy.translationsEmpty : copy.states.checking}
        </p>
      {/if}
    </div>
  </section>

  <section class="mt-8" aria-labelledby="downloads-pack">
    <h2 id="downloads-pack" class={sectionHeading}>{copy.packHeading}</h2>
    <p class="mt-1 max-w-[70ch] text-caption leading-snug text-muted">{storageCopy.offlinePackNote}</p>
    <div class={cn(card, "mt-3")}>
      <div class="grid gap-3 px-4 py-3.5 sm:px-5">
        <OfflinePack copy={storageCopy.offlinePack} />
        {#if packWorking}
          <div class="grid gap-1.5">
            <span class="text-end text-caption tabular-nums text-muted">{offline.pct}%</span>
            {@render progressBar(offline.pct, storageCopy.offlinePack.heading)}
          </div>
        {/if}
      </div>
    </div>
  </section>

  <section class="mt-8" aria-labelledby="downloads-app">
    <h2 id="downloads-app" class={sectionHeading}>{copy.appHeading}</h2>
    <dl class={cn(card, "mt-3")}>
      <div class="flex items-center justify-between gap-4 px-4 py-3.5 sm:px-5">
        <dt class="text-[14.5px] text-foreground">{copy.appOffline}</dt>
        <dd class={cn("text-end text-caption", hasController ? "text-foreground-secondary" : "text-muted")}>
          {hasController ? copy.appActive : copy.appInactive}
        </dd>
      </div>
      <div class="flex items-center justify-between gap-4 px-4 py-3.5 sm:px-5">
        <dt class="text-[14.5px] text-foreground">{settings.app.installed}</dt>
        <dd class="text-end text-caption tabular-nums text-muted">{update.installedVersion ?? "—"}</dd>
      </div>
      <div class="flex items-center justify-between gap-4 px-4 py-3.5 sm:px-5">
        <dt class="text-[14.5px] text-foreground">{settings.app.latest}</dt>
        <dd class="text-end text-caption tabular-nums text-muted">{update.latestVersion}</dd>
      </div>
    </dl>
  </section>

  <section class="mt-8" aria-labelledby="downloads-storage">
    <h2 id="downloads-storage" class={sectionHeading}>{settings.nav.storage}</h2>
    <div class={cn(card, "mt-3")}>
      <div class="px-4 py-3.5 sm:px-5">
        {#if report.usage === null && report.quota === null && report.phase !== "boot"}
          <p class="text-caption text-muted">{copy.storageUnknown}</p>
        {:else}
          <p class="text-[15px] font-medium tabular-nums text-foreground">{usedLabel}</p>
          {#if report.quota !== null}
            <p class="mt-0.5 text-caption tabular-nums text-muted">
              {storageCopy.quotaNote(formatBytes(report.quota))}
            </p>
          {/if}
          <div class="mt-2.5">
            <UsageBar layers={report.layers} labels={storageCopy.layers} usageBytes={report.usage} {usedLabel} />
          </div>
        {/if}
      </div>
    </div>
  </section>
</div>
