<script lang="ts">
  import { page } from "$app/state";
  import { Seo } from "$lib/components";
  import { juzPathFor, type SurahRouteContext } from "$lib/data/quran";
  import ReaderShell from "../../../../../_reader/ReaderShell.svelte";
  import RangeReader from "../../../../../_reader/RangeReader.svelte";

  let { data } = $props();
  const ctx = $derived.by<SurahRouteContext>(() => {
    const lang = page.params.lang;
    const translator = page.params.translator;
    if (typeof lang === "string" && typeof translator === "string") {
      return { kind: "translation", lang, translator };
    }
    return { kind: "arabic" };
  });
  const canonicalPath = $derived(juzPathFor(ctx, data.index));
  const extent = $derived(`${data.first} – ${data.last}`);
  const seoTitle = $derived(`${data.label} (${extent}) — Qur'an · EasyQuran`);
  const seoDescription = $derived(
    `Read ${data.label} of the Qur'an (${data.first}–${data.last}) in translation. Free, fast, and works offline.`,
  );
  const contentLanguage = $derived(page.params.lang ?? "en");
  const pending = $derived(data.ayahs.length === 0);
</script>

<Seo
  path={canonicalPath}
  title={seoTitle}
  description={seoDescription}
  includeTextVariants={false}
  inLanguage={contentLanguage}
  noindex={pending}
  crumbs={[
    { name: "Home", href: "/" },
    { name: data.label, href: canonicalPath },
  ]}
/>

<ReaderShell>
  {#snippet header()}
    <h1 class="text-sm font-medium text-fg-2">{data.label}</h1>
    <span class="ml-auto font-mono text-[12px] text-fg-3">{extent}</span>
  {/snippet}
  <RangeReader {data} />
</ReaderShell>
