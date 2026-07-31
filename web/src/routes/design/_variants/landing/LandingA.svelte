<!--
  Landing A — "Mihrab".

  Reverent and centred. The hero is an arch (a mihrab niche, drawn purely with
  border-radius so it costs nothing and re-colours with the palette) holding the
  real opening ayahs; the headline sits under it rather than over it. Everything
  below is centred, generously spaced, and stripped of cards — the page behaves
  like a doorway into the text instead of a product page about it.

  Palette-wise this variant leans on the accent for the glow and the arch
  hairline only; the surface family does all the heavy lifting, so it changes
  character noticeably between Ink, Paper and Mocha.
-->
<script lang="ts">
  import { Button } from "$lib/components";
  import { HERO, VALUES, ROADMAP, SECTIONS, FACTS } from "../copy";

  let { verses, arabicName }: { verses: string[]; arabicName: string } = $props();

  // Just the opening lines — the arch is a specimen, not a reading surface.
  const specimen = $derived(verses.slice(0, 4));
</script>

<div class="relative overflow-hidden">
  <!-- accent glow behind the arch; sits under everything and never intercepts
       pointer events -->
  <div
    aria-hidden="true"
    class="pointer-events-none absolute left-1/2 top-[-180px] h-[560px] w-[860px] -translate-x-1/2 rounded-full opacity-70 blur-[120px]"
    style="background: radial-gradient(closest-side, var(--accent-soft), transparent)"
  ></div>

  <section class="relative px-5 pb-16 pt-14">
    <div class="mx-auto flex w-full max-w-[860px] flex-col items-center gap-9 text-center">
      <!-- the niche -->
      <div
        class="relative w-full max-w-[520px] rounded-t-full border border-accent-line bg-bg-1/60 px-8 pb-9 pt-16 backdrop-blur-sm"
      >
        <div
          aria-hidden="true"
          class="mx-auto mb-7 h-px w-16 bg-accent-line"
        ></div>
        <div dir="rtl" class="flex flex-col items-center gap-3.5">
          <span class="font-arabic text-[22px] leading-none text-accent">{arabicName}</span>
          {#each specimen as v, i (i)}
            <p class="font-arabic text-[19px] leading-[2] text-fg-2">{v}</p>
          {/each}
        </div>
      </div>

      <span
        class="inline-flex items-center gap-2 rounded-full border border-line px-3.5 py-[7px] text-[13px] font-medium text-fg-3"
      >
        {HERO.badge}
      </span>

      <h1 class="max-w-[16ch] text-balance text-[clamp(38px,6vw,68px)] leading-[1.03] tracking-[-0.035em]">
        {HERO.title}
      </h1>
      <p class="max-w-[48ch] text-pretty text-[18px] leading-[1.65] text-fg-2">
        {HERO.sub}
      </p>

      <div class="flex flex-wrap justify-center gap-2.5">
        <Button variant="accent" size="lg" href={HERO.primary.href}>{HERO.primary.label}</Button>
        <Button variant="ghost" size="lg" href={HERO.secondary.href}>{HERO.secondary.label}</Button>
      </div>
    </div>
  </section>

  <!-- facts as a quiet rule, not a stat-card row -->
  <section class="border-y border-line">
    <div class="mx-auto grid w-full max-w-[860px] grid-cols-2 sm:grid-cols-4">
      {#each FACTS as f (f.label)}
        <div
          class="flex flex-col items-center gap-1 border-line px-4 py-7 text-center [&:not(:last-child)]:border-r"
        >
          <span class="font-arabic text-[30px] leading-none text-fg">{f.value}</span>
          <span class="text-[12.5px] text-fg-4">{f.label}</span>
        </div>
      {/each}
    </div>
  </section>

  <!-- what's here today: centred rows separated by hairlines, no cards -->
  <section class="px-5 py-20">
    <div class="mx-auto flex w-full max-w-[720px] flex-col items-center gap-10 text-center">
      <div class="flex flex-col items-center gap-3">
        <span class="eyebrow text-accent">{SECTIONS.today.eyebrow}</span>
        <h2 class="max-w-[20ch] text-[30px] leading-[1.15] tracking-[-0.025em]">
          {SECTIONS.today.title}
        </h2>
      </div>
      <div class="flex w-full flex-col">
        {#each VALUES as v (v.title)}
          <div class="flex flex-col gap-1.5 border-b border-line py-6 last:border-b-0">
            <span class="text-[17px] font-semibold">{v.title}</span>
            <p class="text-[15px] leading-[1.65] text-fg-2">{v.body}</p>
          </div>
        {/each}
      </div>
    </div>
  </section>

  <!-- on the way -->
  <section class="border-t border-line bg-bg-2 px-5 py-20">
    <div class="mx-auto flex w-full max-w-[720px] flex-col items-center gap-9 text-center">
      <div class="flex flex-col items-center gap-3">
        <span class="eyebrow text-fg-3">{SECTIONS.soon.eyebrow}</span>
        <h2 class="max-w-[20ch] text-[30px] leading-[1.15] tracking-[-0.025em]">
          {SECTIONS.soon.title}
        </h2>
        <p class="max-w-[52ch] text-[15.5px] leading-[1.65] text-fg-3">{SECTIONS.soon.body}</p>
      </div>
      <ul class="flex w-full flex-col gap-3.5">
        {#each ROADMAP as r (r.title)}
          <li class="flex flex-col gap-1 rounded-[12px] border border-line px-6 py-5">
            <span class="text-[15.5px] font-semibold">{r.title}</span>
            <span class="text-[14px] leading-[1.6] text-fg-3">{r.body}</span>
          </li>
        {/each}
      </ul>
      <Button variant="accent" size="lg" href={HERO.primary.href}>{HERO.primary.label}</Button>
    </div>
  </section>
</div>
