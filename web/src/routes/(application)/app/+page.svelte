<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { Seo } from "$lib/components";
  import ReaderPrerenderLinks from "$lib/components/i18n/ReaderPrerenderLinks.svelte";
  import { reader } from "$lib/stores/reader.svelte";
  import { readerSource } from "$lib/stores/reader-settings.svelte";
  import { loadQuranData } from "$lib/data/quran-data-client";
  import { surahPathFor, surahRouteContext } from "$lib/data/quran";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor, readerHomeHrefFor } from "$lib/i18n/reader";
  import { publicHref } from "$lib/i18n/public-href";

  let { data } = $props();
  const copy = getReaderUiCopy();

  onMount(() => {
    reader.hydrate();
    const num = reader.lastRead?.num ?? 1;
    void loadQuranData().then((quranData) => {
      const surah = quranData.surahByNum(num) ?? quranData.surahs[0]!;
      const sourceId = readerSource.sourceId;
      const ctx = sourceId
        ? surahRouteContext(sourceId)
        : { kind: "arabic" as const };
      return goto(publicHref(readerHrefFor(copy.locale, surahPathFor(ctx, surah))), {
        replaceState: true,
      });
    });
  });
</script>

<Seo path={readerHomeHrefFor("en")} title={copy.seo.home} noindex />
<ReaderPrerenderLinks hrefs={data.readerPrerenderHrefs} />

<div class="mx-auto max-w-[1320px] px-5 pt-16 sm:px-7" aria-busy="true">
  <p class="text-sm text-fg-3">{copy.shell.opening}</p>
</div>
