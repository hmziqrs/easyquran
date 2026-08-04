<script lang="ts">
  import { page } from "$app/state";
  import { resolve } from "$app/paths";
  import {
    surahLocalPagePath,
    translationSurahPath,
    translationGlobalPagePath,
    translationJuzPath,
    translationIdFromSegments,
    translationSegmentsFromId,
  } from "$lib/data/quran";
  import { resolveSourceCatalogue, translationCatalogue } from "$lib/quran/catalogue";
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
      if (segs[0] === "page" && segs[1]) return { kind: "globalPage", n: toNum(segs[1]) };
      if (segs[0] === "juz" && segs[1]) return { kind: "juz", n: toNum(segs[1]) };
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

  function hrefFor(pos: Position, target: Target): `/app/${string}` | null {
    if (!pos) return null;
    if (target === "arabic") {
      if (pos.kind === "surah") return surahLocalPagePath(pos.slug, pos.localPage);
      if (pos.kind === "globalPage") return `/app/page/${pos.n}`;
      return `/app/juz/${pos.n}`;
    }
    if (pos.kind === "surah")
      return translationSurahPath(pos.slug, target.lang, target.translator, pos.localPage);
    if (pos.kind === "globalPage")
      return translationGlobalPagePath(target.lang, target.translator, pos.n);
    return translationJuzPath(target.lang, target.translator, pos.n);
  }

  const sidebar = useSidebar();
  const position = $derived(positionOf(page.url.pathname));
  const activeTranslationId = $derived(
    position && position.lang
      ? translationIdFromSegments(position.lang, position.translator ?? "")
      : null,
  );
  const isArabicActive = $derived(activeTranslationId === null);
  const summary = $derived(activeTranslationId ?? "Arabic");
  const cataloguePromise = resolveSourceCatalogue();

  function rowClass(active: boolean, disabled = false): string {
    return cn(
      "flex items-center gap-2 rounded-[7px] px-3 py-2 text-[12.5px] transition-colors",
      disabled && "cursor-not-allowed text-fg-4 opacity-60",
      !disabled &&
        (active ? "bg-bg-3 font-medium text-fg" : "text-fg-2 hover:bg-bg-2 hover:text-fg"),
    );
  }

  function onSelect(target: Target): void {
    readerSource.setSourceId(target === "arabic" ? null : target.id);
    sidebar.setOpenMobile(false);
  }
</script>

<details class="group px-1">
  <summary
    class="flex cursor-pointer list-none items-center justify-between gap-2 rounded-[9px] border border-line bg-bg-2 px-3 py-2 text-[12.5px] text-fg-2 transition-colors hover:border-line-3 hover:text-fg"
  >
    <span class="flex min-w-0 items-center gap-2">
      <span class="text-[10.5px] uppercase tracking-wide text-fg-4">Source</span>
      <span class="truncate font-medium text-fg">{summary}</span>
    </span>
    <Icon
      name="arrow-right"
      size={13}
      class="flex-none text-fg-3 transition-transform group-open:rotate-90"
    />
  </summary>

  <ul class="mt-1 flex max-h-[220px] flex-col gap-0.5 overflow-y-auto py-0.5">
    {#await cataloguePromise}
      <li class="px-3 py-2 text-[12px] text-fg-3">Loading sources…</li>
    {:then entries}
      {@const arabicHref = hrefFor(position, "arabic")}
      <li>
        {#if arabicHref}
          <a
            href={resolve(arabicHref)}
            data-sveltekit-preload-data="hover"
            aria-current={isArabicActive ? "page" : undefined}
            onclick={() => onSelect("arabic")}
            class={rowClass(isArabicActive)}
          >
            <span class="truncate">Arabic</span>
            <span class="ml-auto text-[10.5px] text-fg-4">Original</span>
          </a>
        {:else}
          <span class={rowClass(false, true)}>
            <span class="truncate">Arabic</span>
          </span>
        {/if}
      </li>
      {#each translationCatalogue(entries) as t (t.id)}
        {@const seg = translationSegmentsFromId(t.id)}
        {@const target = { id: t.id, lang: seg.lang, translator: seg.translator }}
        {@const href = hrefFor(position, target)}
        {@const active = activeTranslationId === t.id}
        {@const preferred = readerSource.sourceId === t.id}
        <li>
          {#if href}
            <a
              href={resolve(href)}
              data-sveltekit-preload-data="hover"
              aria-current={active ? "page" : undefined}
              onclick={() => onSelect(target)}
              class={rowClass(active)}
            >
              <span class="truncate">{t.language}{t.translator ? ` · ${t.translator}` : ""}</span>
              {#if preferred && !active}
                <span class="ml-auto text-[10.5px] text-fg-4">default</span>
              {/if}
            </a>
          {:else}
            <span class={rowClass(false, true)}>
              <span class="truncate">{t.language}</span>
            </span>
          {/if}
        </li>
      {:else}
        <li class="px-3 py-2 text-[12px] text-fg-3">No translations available yet.</li>
      {/each}
    {:catch}
      <li class="px-3 py-2 text-[12px] text-fg-3">Sources could not be loaded.</li>
    {/await}
  </ul>
</details>

<style>
  summary::-webkit-details-marker {
    display: none;
  }
</style>
