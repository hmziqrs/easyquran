<script lang="ts">
  import { Seo } from "#lib/components/index.js";
  import { juzPathFor, type SurahRouteContext } from "#lib/data/quran.js";
  import { getReaderUiCopy } from "#lib/i18n/reader-copy.js";
  import { readerHrefFor } from "#lib/i18n/reader.js";
  import ReaderShell from "../../_reader/ReaderShell.svelte";
  import RangeReader from "../../_reader/RangeReader.svelte";

  let { data } = $props();
  const copy = getReaderUiCopy();
  const arabicCtx: SurahRouteContext = { kind: "arabic" };
  const canonicalPath = $derived(readerHrefFor("en", juzPathFor(arabicCtx, data.index)));
  const extent = $derived(`${data.first} – ${data.last}`);
  const seoTitle = $derived(copy.seo.juzTitle(data.index, data.first, data.last));
  const seoDescription = $derived(
    copy.seo.juzDescription(data.index, data.first, data.last),
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
    <h1 class="text-sm font-medium text-foreground-secondary">{copy.range.item("juz", data.index)}</h1>
    <span class="ms-auto font-mono text-[12px] text-muted">{extent}</span>
  {/snippet}
  <RangeReader {data} />
</ReaderShell>
