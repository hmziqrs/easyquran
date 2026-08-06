<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { resolve } from "$app/paths";
  import { Seo } from "$lib/components";
  import { reader } from "$lib/stores/reader.svelte";
  import { readerSource } from "$lib/stores/reader-settings.svelte";
  import { loadQuranData } from "$lib/data/quran-data-client";
  import { surahPathFor, surahRouteContext } from "$lib/data/quran";

  onMount(() => {
    reader.hydrate();
    const num = reader.lastRead?.num ?? 1;
    void loadQuranData().then((quranData) => {
      const surah = quranData.surahByNum(num) ?? quranData.surahs[0]!;
      const sourceId = readerSource.sourceId;
      const ctx = sourceId
        ? surahRouteContext(sourceId)
        : { kind: "arabic" as const };
      return goto(resolve(surahPathFor(ctx, surah)), { replaceState: true });
    });
  });
</script>

<Seo path="/app" noindex />

<div class="mx-auto max-w-[1320px] px-5 pt-16 sm:px-7" aria-busy="true">
  <p class="text-sm text-fg-3">Opening the reader…</p>
</div>
