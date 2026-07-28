<script lang="ts">
  import { Container, Eyebrow, Seo } from "$lib/components";

  // Placeholder content — the first ayahs of al-Fatihah, so the Arabic type
  // stack and the `arabic` utility are exercised by something real.
  const ayahs = [
    "بِسْمِ ٱللَّٰهِ ٱلرَّحْمَٰنِ ٱلرَّحِيمِ",
    "ٱلْحَمْدُ لِلَّٰهِ رَبِّ ٱلْعَٰلَمِينَ",
    "ٱلرَّحْمَٰنِ ٱلرَّحِيمِ",
    "مَٰلِكِ يَوْمِ ٱلدِّينِ",
  ];
</script>

<Seo path="/app/read" noindex />

<Container width="narrow" class="py-10">
  <Eyebrow>Surah 1</Eyebrow>
  <h1 class="mt-3 text-xl">Al-Fatihah</h1>

  <div class="mt-8 grid gap-6">
    {#each ayahs as ayah, i (i)}
      <div class="border-b border-line pb-6 last:border-0">
        <span class="font-mono text-xs text-fg-4">1:{i + 1}</span>
        <p class="arabic mt-3">{ayah}</p>
      </div>
    {/each}
  </div>

  <p class="mt-8 text-sm text-fg-3">
    Placeholder — wire this to the real mushaf data source, with surah/ayah routing and saved
    reading position.
  </p>
</Container>
