<script lang="ts">
  import { Seo } from "$lib/components";
  import { globalPagePathFor, type SurahRouteContext } from "$lib/data/quran";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor } from "$lib/i18n/reader";
  import ReaderShell from "../../_reader/ReaderShell.svelte";
  import RangeReader from "../../_reader/RangeReader.svelte";

  let { data } = $props();
  const copy = getReaderUiCopy();
  const arabicCtx: SurahRouteContext = { kind: "arabic" };
  const canonicalPath = $derived(readerHrefFor("en", globalPagePathFor(arabicCtx, data.index)));
  const extent = $derived(`${data.first} – ${data.last}`);
  const seoTitle = $derived(copy.seo.pageTitle(data.index, data.first, data.last));
  const seoDescription = $derived(
    copy.seo.pageDescription(data.index, data.first, data.last),
  );
</script>

<Seo
  path={canonicalPath}
  title={seoTitle}
  description={seoDescription}
  includeTextVariants={false}
/>

<ReaderShell>
  {#snippet header()}
    <h1 class="text-sm font-medium text-fg-2">{copy.range.item("page", data.index)}</h1>
    <span class="ml-auto font-mono text-[12px] text-fg-3">{extent}</span>
  {/snippet}
  <RangeReader {data} />
</ReaderShell>
