<script lang="ts">
  import { page } from "$app/state";
  import { Seo } from "$lib/components";
  import { hizbPathFor, type SurahRouteContext } from "$lib/data/quran";
  import { HIZB_COUNT } from "$lib/data/mushaf-divisions";
  import { getReaderUiCopy } from "$lib/i18n/reader-copy";
  import { readerHrefFor } from "$lib/i18n/reader";
  import ReaderShell from "../../../../../_reader/ReaderShell.svelte";
  import RangeReader from "../../../../../_reader/RangeReader.svelte";

  let { data } = $props();
  const copy = getReaderUiCopy();
  const ctx = $derived.by<SurahRouteContext>(() => {
    const lang = page.params.lang;
    const translator = page.params.translator;
    if (lang !== undefined && translator !== undefined) {
      return { kind: "translation", lang, translator };
    }
    return { kind: "arabic" };
  });
  const canonicalPath = $derived(hizbPathFor(ctx, data.index));
  const canonicalPublicPath = $derived(readerHrefFor("en", canonicalPath));
  const currentPublicPath = $derived(readerHrefFor(copy.locale, canonicalPath));
  const extent = $derived(`${data.first} – ${data.last}`);
  const seoTitle = $derived(copy.seo.hizbTitle(data.index, data.first, data.last));
  const seoDescription = $derived(
    copy.seo.translationHizbDescription(data.index, data.first, data.last),
  );
  const contentLanguage = $derived(page.params.lang ?? "en");
  const pending = $derived(data.ayahs.length === 0);
  const prevHref = $derived(data.index > 1 ? hizbPathFor(ctx, data.index - 1) : null);
  const nextHref = $derived(
    data.index < HIZB_COUNT ? hizbPathFor(ctx, data.index + 1) : null,
  );
</script>

<Seo
  path={canonicalPublicPath}
  title={seoTitle}
  description={seoDescription}
  includeTextVariants
  includePlainVariant={false}
  inLanguage={contentLanguage}
  noindex={pending}
  crumbs={[
    { name: copy.seo.home, href: "/" },
    { name: copy.range.item("hizb", data.index), href: currentPublicPath },
  ]}
/>

<ReaderShell>
  {#snippet header()}
    <h1 class="text-sm font-medium text-foreground-secondary">{copy.range.item("hizb", data.index)}</h1>
    <span class="ms-auto font-mono text-[12px] text-muted">{extent}</span>
  {/snippet}
  <RangeReader {data} />
</ReaderShell>
