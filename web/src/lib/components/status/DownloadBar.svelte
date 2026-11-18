<script lang="ts">
  import { page } from "$app/state";
  import { quran } from "#lib/stores/quran.svelte.js";
  import { offline } from "#lib/offline/offline-store.svelte.js";
  import { QURAN } from "#lib/config/site.js";
  import { isArabicSourceId, QuranScript } from "#lib/data/quran-types.js";
  import { sourceProfile } from "#lib/quran/view/source-profiles.js";
  import { Icon } from "#lib/components/icon/index.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { getLocale } from "#lib/paraglide/runtime.js";
  import type { UiLocale } from "#lib/i18n/locales.js";
  import {
    reader_download_details,
    reader_download_hide,
    reader_download_script_indopak,
    reader_download_script_simple_clean,
    reader_download_script_tajweed,
    reader_download_script_translation,
    reader_download_script_uthmani,
    reader_downloading_quran,
    reader_offline_pack_ready,
    reader_preparing_offline_pack,
  } from "#lib/i18n/m/reader.js";
  import { downloadDismissal } from "./download-dismissal.svelte.js";
  import { milestonePercent, quranRunFraction, toPercent } from "./download-progress.js";

  // Locale changes are full navigations, so resolving once at init is sound — same idiom as UpdateToast.
  // SAFETY: paraglide is compiled for exactly the UI locales (en/ar in messages/), so getLocale() only ever returns a UiLocale at runtime.
  const locale = getLocale() as UiLocale;
  const options = { locale } as const;

  const copy = {
    quran: reader_downloading_quran(undefined, options),
    pack: reader_preparing_offline_pack(undefined, options),
    packReady: reader_offline_pack_ready(undefined, options),
    hide: reader_download_hide(undefined, options),
    details: reader_download_details(undefined, options),
  };

  const SCRIPT_LABELS = {
    [QuranScript.Uthmani]: reader_download_script_uthmani(undefined, options),
    [QuranScript.SimpleClean]: reader_download_script_simple_clean(undefined, options),
    [QuranScript.IndoPak]: reader_download_script_indopak(undefined, options),
    [QuranScript.Tajweed]: reader_download_script_tajweed(undefined, options),
    [QuranScript.Translation]: reader_download_script_translation(undefined, options),
  } satisfies Readonly<Record<QuranScript, string>>;

  const DOWNLOADS_PATH = "/downloads";
  const detailsHref = publicHref(DOWNLOADS_PATH);

  interface ProgressRow {
    readonly id: "quran" | "pack";
    readonly label: string;
    readonly detail: string;
    readonly pct: number;
  }

  const quranActive = $derived(
    quran.download !== null && quran.status !== "ready" && quran.status !== "error",
  );
  const packActive = $derived(offline.status === "downloading" || offline.status === "staging");
  // The downloads screen shows the same progress in full; no popup on top of it.
  const onDownloadsScreen = $derived((page.route.id ?? "").endsWith(DOWNLOADS_PATH));

  function scriptLabel(): string {
    const script = quran.download?.script;
    if (script === undefined) return "";
    if (isArabicSourceId(script)) return SCRIPT_LABELS[sourceProfile(script).script];
    return SCRIPT_LABELS[QuranScript.Translation];
  }

  function visibleRows(): ProgressRow[] {
    if (onDownloadsScreen) return [];
    const rows: ProgressRow[] = [];
    if (quranActive && !downloadDismissal.quranHidden) {
      rows.push({
        id: "quran",
        label: copy.quran,
        detail: scriptLabel(),
        pct: toPercent(quranRunFraction(quran.download, QURAN.scripts)),
      });
    }
    if (packActive && !downloadDismissal.packHidden) {
      rows.push({ id: "pack", label: copy.pack, detail: "", pct: offline.pct });
    }
    return rows;
  }

  const rows = $derived(visibleRows());

  function milestoneText(row: ProgressRow): string {
    if (row.id === "pack" && row.pct >= 100) return copy.packReady;
    return `${row.label} ${milestonePercent(row.pct)}%`;
  }

  const milestones = $derived(rows.map((row) => milestoneText(row)).join(". "));

  // Published height so the bottom-center toaster stacks above the popup (root +layout.svelte).
  let height = $state(0);
  $effect(() => {
    const root = document.documentElement;
    const h = rows.length > 0 ? height : 0;
    root.style.setProperty("--eq-status-h", `${h}px`);
    return () => root.style.removeProperty("--eq-status-h");
  });
</script>

{#if rows.length > 0}
  <!-- Bottom on every viewport: at the top it covered the mobile header. Sits above the audio
       dock (which publishes --eq-dock-h) and clears the home-indicator safe area. -->
  <div
    bind:offsetHeight={height}
    class="pointer-events-none fixed inset-x-0 bottom-[calc(var(--eq-dock-h,0px)+env(safe-area-inset-bottom))] z-[80] flex justify-center px-3 pb-3"
  >
    <div
      class="pointer-events-auto flex w-full max-w-sm items-start gap-2 rounded-xl border border-border bg-surface py-2.5 ps-3.5 pe-2 text-caption text-foreground shadow-md"
    >
      <div class="grid min-w-0 flex-1 gap-2">
        {#each rows as row (row.id)}
          <div class="grid gap-1.5">
            <div class="flex items-center gap-2">
              <span class="inline-block size-1.5 shrink-0 animate-pulse rounded-full bg-primary" aria-hidden="true"></span>
              <span class="truncate">
                {row.label}{#if row.detail}<span class="opacity-60"> · {row.detail}</span>{/if}
              </span>
              <span class="ms-auto tabular-nums opacity-70" aria-hidden="true">{row.pct}%</span>
            </div>
            <div
              role="progressbar"
              aria-label={row.label}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={row.pct}
              class="h-1 w-full overflow-hidden rounded-pill border border-border bg-background-subtle"
            >
              <div
                class="h-full rounded-pill bg-primary transition-[width] duration-150 ease-out"
                style:width={`${row.pct}%`}
              ></div>
            </div>
          </div>
        {/each}
        <a
          href={detailsHref}
          class="justify-self-start rounded-sm text-caption font-medium text-primary underline-offset-2 hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          >{copy.details}</a
        >
      </div>
      <button
        type="button"
        onclick={() => downloadDismissal.hide()}
        aria-label={copy.hide}
        title={copy.hide}
        class="grid size-8 shrink-0 place-items-center rounded-pill text-muted transition-colors hover:bg-background-subtle hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
      >
        <Icon name="x" size={14} />
      </button>
    </div>
  </div>
{/if}
<!-- Always mounted so screen readers pick up changes; 25% steps, never every byte. -->
<span class="sr-only" role="status" aria-live="polite">{milestones}</span>
