<!--
  Landing C — "Editorial".

  A split hero: the claim on the left, a live mushaf specimen panel on the
  right — a real, framed slice of the reader rendering real Uthmani text, so
  the first thing a visitor sees is the product rather than a description of
  it. Below, numbered editorial sections alternate a heading column against a
  content column.

  The specimen panel is the whole first impression, which makes this the
  variant most sensitive to the palette: it should be checked in every surface
  family before it's chosen.
-->
<script lang="ts">
  import { Button, Icon } from "$lib/components";
  import { HERO, VALUES, ROADMAP, FACTS, SECTIONS } from "../copy";

  let {
    verses,
    arabicName,
    name,
    ayahCount,
  }: { verses: string[]; arabicName: string; name: string; ayahCount: number } = $props();

  const specimen = $derived(verses.slice(0, 5));
  const arabicDigits = (n: number) =>
    String(n).replace(/\d/g, (d) => "٠١٢٣٤٥٦٧٨٩"[Number(d)]!);
</script>

<!-- ── hero ─────────────────────────────────────────────────────────────── -->
<section class="border-b border-line">
  <div
    class="mx-auto grid w-full max-w-[1240px] items-center gap-12 px-5 py-16 lg:grid-cols-[1fr_minmax(400px,520px)] lg:py-20"
  >
    <div class="flex flex-col items-start gap-6">
      <span
        class="inline-flex items-center gap-2 rounded-full bg-accent-soft px-3.5 py-[7px] text-[13px] font-medium text-accent"
      >
        {HERO.badge}
      </span>
      <h1 class="max-w-[15ch] text-[clamp(38px,5.6vw,64px)] leading-[1.03] tracking-[-0.035em]">
        {HERO.title}
      </h1>
      <p class="max-w-[46ch] text-[18px] leading-[1.65] text-fg-2">{HERO.sub}</p>
      <div class="flex flex-wrap gap-2.5">
        <Button variant="accent" size="lg" href={HERO.primary.href}>{HERO.primary.label}</Button>
        <Button variant="ghost" size="lg" href={HERO.secondary.href}>{HERO.secondary.label}</Button>
      </div>

      <div class="mt-2 flex flex-wrap items-baseline gap-x-7 gap-y-2">
        {#each FACTS as f (f.label)}
          <span class="flex items-baseline gap-1.5">
            <span class="text-[19px] font-semibold text-fg">{f.value}</span>
            <span class="text-[13px] text-fg-4">{f.label}</span>
          </span>
        {/each}
      </div>
    </div>

    <!-- specimen: a framed slice of the actual reading surface -->
    <div
      class="overflow-hidden rounded-2xl border border-line bg-bg-1 shadow-[0_24px_60px_-30px_rgba(0,0,0,0.55)]"
    >
      <div class="flex items-center gap-2 border-b border-line bg-bg-2 px-4 py-2.5">
        <span class="flex items-center gap-1.5 text-fg-4">
          <Icon name="book" size={13} />
          <span class="font-mono text-[11px]">{name}</span>
        </span>
        <span dir="rtl" class="ml-auto font-arabic text-sm text-fg-3">{arabicName}</span>
      </div>

      <div dir="rtl" class="flex flex-col">
        {#each specimen as v, i (i)}
          <p
            class="border-b border-line px-6 py-4 font-arabic text-[21px] leading-[2.1] text-fg last:border-b-0"
          >
            {v}<span class="ayah-marker">{arabicDigits(i + 1)}</span>
          </p>
        {/each}
      </div>

      <div class="flex items-center justify-between border-t border-line bg-bg-2 px-4 py-2.5">
        <span class="font-mono text-[11px] text-fg-4">1 – {specimen.length} of {ayahCount}</span>
        <span class="text-[11px] text-accent">Uthmani script</span>
      </div>
    </div>
  </div>
</section>

<!-- ── what's here today ────────────────────────────────────────────────── -->
<section class="border-b border-line">
  <div class="mx-auto grid w-full max-w-[1240px] gap-10 px-5 py-16 lg:grid-cols-[22rem_1fr]">
    <div class="flex flex-col gap-3 lg:sticky lg:top-24 lg:self-start">
      <span class="eyebrow text-accent">{SECTIONS.today.eyebrow}</span>
      <h2 class="max-w-[18ch] text-[32px] leading-[1.12] tracking-[-0.028em]">
        {SECTIONS.today.title}
      </h2>
      <p class="max-w-[38ch] text-[15.5px] leading-[1.6] text-fg-3">{SECTIONS.today.body}</p>
    </div>

    <div class="grid gap-5 sm:grid-cols-2">
      {#each VALUES as v (v.title)}
        <div class="flex flex-col gap-2.5 rounded-[14px] border border-line bg-bg-1 px-6 py-6">
          <span class="flex size-8 items-center justify-center rounded-[9px] bg-accent-soft text-accent">
            <Icon name={v.icon} size={15} />
          </span>
          <span class="text-[17px] font-semibold">{v.title}</span>
          <p class="text-[14.5px] leading-[1.62] text-fg-2">{v.body}</p>
        </div>
      {/each}
    </div>
  </div>
</section>

<!-- ── on the way ───────────────────────────────────────────────────────── -->
<section class="bg-bg-2">
  <div class="mx-auto grid w-full max-w-[1240px] gap-10 px-5 py-16 lg:grid-cols-[22rem_1fr]">
    <div class="flex flex-col gap-3">
      <span class="eyebrow text-fg-3">{SECTIONS.soon.eyebrow}</span>
      <h2 class="max-w-[18ch] text-[32px] leading-[1.12] tracking-[-0.028em]">
        {SECTIONS.soon.title}
      </h2>
      <p class="max-w-[38ch] text-[15.5px] leading-[1.6] text-fg-3">{SECTIONS.soon.body}</p>
      <div class="mt-3">
        <Button variant="accent" size="lg" href={HERO.primary.href}>{HERO.primary.label}</Button>
      </div>
    </div>

    <ol class="flex flex-col">
      {#each ROADMAP as r, i (r.title)}
        <li
          class="grid grid-cols-[2.5rem_1fr] gap-x-5 gap-y-1.5 border-b border-line-2 py-6 first:pt-0 last:border-b-0"
        >
          <span class="font-mono text-[12px] text-fg-4">{String(i + 1).padStart(2, "0")}</span>
          <div class="flex flex-col gap-1.5">
            <span class="flex items-center gap-2.5">
              <span class="text-[17px] font-semibold">{r.title}</span>
              <span
                class="rounded-full border border-line-2 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.1em] text-fg-4"
                >Coming</span
              >
            </span>
            <p class="max-w-[54ch] text-[14.5px] leading-[1.62] text-fg-2">{r.body}</p>
          </div>
        </li>
      {/each}
    </ol>
  </div>
</section>
