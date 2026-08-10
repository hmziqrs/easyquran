<script lang="ts">
  import { Seo } from "$lib/components";
  import { juzPathFor, type SurahRouteContext } from "$lib/data/quran";
  import ReaderShell from "../_reader/ReaderShell.svelte";

  let { data } = $props();

  const arabicCtx: SurahRouteContext = { kind: "arabic" };

  const seoTitle = "Juz index — Qur'an · EasyQuran";
  const seoDescription = "Browse the 30 juz of the Qur'an. Free, fast, and works offline.";
</script>

<Seo
  path="/app/juz"
  title={seoTitle}
  description={seoDescription}
  includeTextVariants={false}
/>

<ReaderShell>
  {#snippet header()}
    <h1 class="text-sm font-medium text-fg-2">Juz index</h1>
    <span class="ml-auto font-mono text-[12px] text-fg-3">30 ajzāʼ</span>
  {/snippet}

  <ul class="grid grid-cols-1 gap-2 sm:grid-cols-2">
    {#each data.ajzur as juz (juz.index)}
      <li>
        <a
          href={juzPathFor(arabicCtx, juz.index)}
          data-sveltekit-preload-data="hover"
          class="flex items-center gap-3 rounded-lg border border-line px-4 py-3 transition-colors hover:bg-bg-2"
        >
          <span
            class="flex h-7 min-w-7 items-center justify-center rounded-full border border-line px-2 text-[11px] text-fg-3"
          >
            {juz.index}
          </span>
          <span class="text-sm font-medium text-fg">Juz {juz.index}</span>
          <span class="ml-auto font-mono text-[12px] text-fg-3">{juz.first} – {juz.last}</span>
        </a>
      </li>
    {/each}
  </ul>
</ReaderShell>
