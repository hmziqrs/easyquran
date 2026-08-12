<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { deLocalizeUrl } from "$lib/paraglide/runtime";
  import {
    SourceKind,
    surahLocalPagePathFor,
    globalPagePathFor,
    juzPathFor,
    translationIdFromSegments,
    translationSegmentsFromId,
    type SurahRouteContext,
  } from "$lib/data/quran";
  import { catalogueStore } from "$lib/quran/catalogue-store.svelte";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor } from "$lib/i18n/reader";
  import { publicHref } from "$lib/i18n/public-href";
  import { noteTranslationChosen } from "$lib/quran/engagement";
  import { readerSource } from "$lib/stores/reader-settings.svelte";
  import { useSidebar } from "$lib/components/ui/sidebar";
  import { Icon } from "$lib/components/icon";
  import { cn } from "$lib/utils";

  type Position =
    | { kind: "surah"; slug: string; localPage: number; lang?: string; translator?: string }
    | { kind: "globalPage"; n: number; lang?: string; translator?: string }
    | { kind: "juz"; n: number; lang?: string; translator?: string }
    | null;

  type Target = "arabic" | { id: string; lang: string; translator: string };

  function toNum(s: string | undefined): number {
    const n = s ? Number(s) : NaN;
    return Number.isSafeInteger(n) && n > 0 ? n : 1;
  }

  function positionOf(pathname: string): Position {
    const segs = pathname.replace(/^\/app\/?/, "").split("/").filter(Boolean);
    if (segs.length === 0) return null;
    const tIdx = segs.indexOf("t");
    if (tIdx === -1) {
      if (segs[0] === "page") return segs[1] ? { kind: "globalPage", n: toNum(segs[1]) } : null;
      if (segs[0] === "juz") return segs[1] ? { kind: "juz", n: toNum(segs[1]) } : null;
      const slug = segs[0];
      if (segs[1] === "page" && segs[2]) return { kind: "surah", slug, localPage: toNum(segs[2]) };
      return { kind: "surah", slug, localPage: 1 };
    }
    const lang = segs[tIdx + 1];
    const translator = segs[tIdx + 2];
    if (!lang || !translator) return null;
    const rest = segs.slice(tIdx + 3);
    if (tIdx === 1) {
      const slug = segs[0];
      if (rest[0] === "page" && rest[1])
        return { kind: "surah", slug, localPage: toNum(rest[1]), lang, translator };
      return { kind: "surah", slug, localPage: 1, lang, translator };
    }
    if (rest[0] === "page" && rest[1])
      return { kind: "globalPage", n: toNum(rest[1]), lang, translator };
    if (rest[0] === "juz" && rest[1]) return { kind: "juz", n: toNum(rest[1]), lang, translator };
    return null;
  }

  function ctxFor(target: Target): SurahRouteContext {
    return target === "arabic"
      ? { kind: "arabic" }
      : { kind: "translation", lang: target.lang, translator: target.translator };
  }

  function hrefFor(pos: Position, target: Target): `/app/${string}` | null {
    if (!pos) return null;
    const ctx = ctxFor(target);
    if (pos.kind === "surah") return surahLocalPagePathFor(ctx, pos.slug, pos.localPage);
    if (pos.kind === "globalPage") return globalPagePathFor(ctx, pos.n);
    return juzPathFor(ctx, pos.n);
  }

  const sidebar = useSidebar();
  const copy = getReaderUiCopy();
  const position = $derived(positionOf(deLocalizeUrl(page.url).pathname));
  const activeTranslationId = $derived(
    position && position.lang
      ? translationIdFromSegments(position.lang, position.translator ?? "")
      : null,
  );
  const isArabicActive = $derived(activeTranslationId === null);
  const summary = $derived(activeTranslationId ?? copy.sources.arabic);
  const arabicHref = $derived(hrefFor(position, SourceKind.Arabic));

  onMount(() => {
    void catalogueStore.ensure();
  });

  let detailsEl: HTMLDetailsElement | null = $state(null);
  let revealed = false;

  // Open on a non-Arabic source and bring the picked row into view, once the
  // catalogue rows exist. Runs once per mount so user toggles are not fought.
  $effect(() => {
    const el = detailsEl;
    const ready = catalogueStore.translations.length > 0;
    if (!el || revealed || (!ready && activeTranslationId !== null)) return;
    revealed = true;
    if (activeTranslationId === null) return;
    el.open = true;
    requestAnimationFrame(() => {
      el.querySelector('[aria-current="page"]')?.scrollIntoView({ block: "nearest" });
    });
  });

  function rowClass(active: boolean, disabled = false): string {
    return cn(
      "flex items-center gap-2 rounded-[7px] px-3 py-2 text-[12.5px] transition-colors",
      disabled && "cursor-not-allowed text-fg-4 opacity-60",
      !disabled &&
        (active ? "bg-bg-3 font-medium text-fg" : "text-fg-2 hover:bg-bg-2 hover:text-fg"),
    );
  }

  function onSelect(target: Target): void {
    readerSource.setSourceId(target === SourceKind.Arabic ? null : target.id);
    if (target !== SourceKind.Arabic) void noteTranslationChosen(target.id);
    sidebar.setOpenMobile(false);
  }
</script>

<details bind:this={detailsEl} class="group px-1">
  <summary
    title={copy.sources.source}
    class="flex cursor-pointer list-none items-center justify-between gap-2 rounded-[9px] border border-line bg-bg-2 px-3 py-2 text-[12.5px] text-fg-2 transition-colors hover:border-line-3 hover:text-fg"
  >
    <span class="flex min-w-0 items-center gap-2">
      <span class="text-[10.5px] uppercase tracking-wide text-fg-4">{copy.sources.source}</span>
      <span class="truncate font-medium text-fg">{summary}</span>
    </span>
    <Icon
      name="arrow-right"
      size={13}
      class="flex-none text-fg-3 transition-transform group-open:rotate-90"
    />
  </summary>

  <ul
    class="mt-1 flex max-h-[220px] flex-col gap-0.5 overflow-y-auto py-0.5"
    aria-label={copy.sources.source}
  >
    <li>
      {#if arabicHref}
        <a
          href={publicHref(readerHrefFor(copy.locale, arabicHref))}
          data-sveltekit-preload-data="hover"
          aria-current={isArabicActive ? "page" : undefined}
          aria-label={copy.sources.arabic}
          title={copy.sources.arabic}
          onclick={() => onSelect(SourceKind.Arabic)}
          class={rowClass(isArabicActive)}
        >
          <span class="truncate">{copy.sources.arabic}</span>
          <span class="ml-auto text-[10.5px] text-fg-4">{copy.sources.original}</span>
        </a>
      {:else}
        <span class={rowClass(false, true)}>
          <span class="truncate">{copy.sources.arabic}</span>
        </span>
      {/if}
    </li>
    {#each catalogueStore.translations as t (t.id)}
      {@const seg = translationSegmentsFromId(t.id)}
      {@const target = { id: t.id, lang: seg.lang, translator: seg.translator }}
      {@const href = hrefFor(position, target)}
      {@const active = activeTranslationId === t.id}
      {@const preferred = readerSource.sourceId === t.id}
      <li>
        {#if href}
          <a
            href={publicHref(readerHrefFor(copy.locale, href))}
            data-sveltekit-preload-data="hover"
            aria-current={active ? "page" : undefined}
            onclick={() => onSelect(target)}
            class={rowClass(active)}
          >
            <span class="truncate">{t.language}{t.translator ? ` · ${t.translator}` : ""}</span>
            {#if preferred && !active}
              <span class="ml-auto text-[10.5px] text-fg-4">{copy.sources.default}</span>
            {/if}
          </a>
        {:else}
          <span class={rowClass(false, true)}>
            <span class="truncate">{t.language}</span>
          </span>
        {/if}
      </li>
    {:else}
      <li class="px-3 py-2 text-[12px] text-fg-3" role="status">
        {copy.sources.noTranslations}
      </li>
    {/each}
  </ul>
</details>

<style>
  summary::-webkit-details-marker {
    display: none;
  }
</style>
