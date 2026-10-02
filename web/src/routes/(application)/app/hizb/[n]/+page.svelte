<script lang="ts">
  import { Seo } from "$lib/components";
  import { hizbPathFor, type SurahRouteContext } from "$lib/data/quran";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor } from "$lib/i18n/reader";
  import ReaderShell from "../../_reader/ReaderShell.svelte";
  import RangeReader from "../../_reader/RangeReader.svelte";

  let { data } = $props();
  const copy = getReaderUiCopy();
  const ARABIC: SurahRouteContext = { kind: "arabic" };
  const canonicalPath = $derived(readerHrefFor("en", hizbPathFor(ARABIC, data.index)));
  const extent = $derived(`${data.first} – ${data.last}`);
  const seoTitle = $derived(copy.seo.hizbTitle(data.index, data.first, data.last));
  const seoDescription = $derived(
    copy.seo.hizbDescription(data.index, data.first, data.last),
  );
</script>

<Seo
  path={canonicalPath}
  title={seoTitle}
  description={seoDescription}
  includeTextVariants
  includePlainVariant={false}
/>

<ReaderShell position={{ globalPage: data.globalPage, juz: data.juz, hizb: data.hizb }}>
  {#snippet header()}
    <h1 class="text-sm font-medium text-foreground-secondary">{copy.range.item("hizb", data.index)}</h1>
    <span class="ms-auto font-mono text-[12px] text-muted">{extent}</span>
  {/snippet}
  <RangeReader {data} />
</ReaderShell>
