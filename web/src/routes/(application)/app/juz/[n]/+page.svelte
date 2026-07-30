<!-- /app/juz/[n] — a juz range view (prerendered). RangeReader renders the
     ayahs grouped by surah; the URL is shareable and SEO-indexable. -->
<script lang="ts">
  import { Seo } from "$lib/components";
  import ReaderShell from "../../_reader/ReaderShell.svelte";
  import RangeReader from "../../_reader/RangeReader.svelte";

  let { data } = $props();
  const extent = $derived(`${data.first} – ${data.last}`);
  const seoTitle = $derived(`${data.label} (${extent}) — Qur'an · EasyQuran`);
  const seoDescription = $derived(
    `Read ${data.label} of the Qur'an (${data.first}–${data.last}) in the Uthmani script. Free, fast, and works offline.`,
  );
</script>

<Seo
  path={`/app/juz/${data.index}`}
  title={seoTitle}
  description={seoDescription}
  includeTextVariants={false}
/>

<ReaderShell>
  {#snippet header()}
    <h1 class="text-sm font-medium text-fg-2">{data.label}</h1>
    <span class="ml-auto font-mono text-[12px] text-fg-3">{extent}</span>
  {/snippet}
  <RangeReader {data} />
</ReaderShell>
