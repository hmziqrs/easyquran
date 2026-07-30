<!--
  /app — resume the last-read surah (or Al-Fatihah). The reader itself lives at
  /app/[surah]; this index just picks a destination client-side, after the store
  hydrates, so "Open the app" always lands you back where you were. (Hydrate is
  idempotent — the app layout also calls it — so order across onMount hooks is
  irrelevant.)
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { Seo } from "$lib/components";
  import { reader } from "$lib/stores/reader.svelte";
  import { slugFor } from "$lib/data/quran";

  onMount(() => {
    reader.hydrate();
    const num = reader.lastRead?.num ?? 1;
    goto(`/app/${slugFor(num)}`, { replaceState: true });
  });
</script>

<Seo path="/app" noindex />

<!-- bare shell while the redirect resolves -->
<div class="mx-auto max-w-[1320px] px-5 pt-16 sm:px-7" aria-busy="true">
  <p class="text-sm text-fg-3">Opening the reader…</p>
</div>
