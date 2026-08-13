<script lang="ts">
  import { page } from "$app/state";
  import { Button, Container, Eyebrow, Icon } from "$lib/components";
  import { resolveLandingCopy } from "$lib/i18n/landing-copy";
  import { marketingLocaleFromPath, marketingReaderHomeHref } from "$lib/i18n/marketing-copy";
  import { marketingHref } from "$lib/i18n/marketing";
  import MarketingSeo from "./_components/MarketingSeo.svelte";
  import { surahPathFor } from "$lib/data/quran";
  import { readerHrefFor } from "$lib/i18n/reader";
  import { publicHref } from "$lib/i18n/public-href";

  let { data } = $props();

  const arabicCtx = { kind: "arabic" } as const;
  const locale = $derived(marketingLocaleFromPath(page.url.pathname));
  const landing = $derived(resolveLandingCopy(locale));
  const aboutHref = $derived(marketingHref("about", locale));
</script>

<MarketingSeo {locale} />

<section class="pt-22 pb-7">
  <Container class="max-w-[1180px] flex flex-col items-center gap-[22px] text-center">
    <span
      class="inline-flex items-center gap-2 rounded-full bg-accent-soft px-3.5 py-[7px] text-[13px] font-medium text-accent"
    >
      {landing.badge}
    </span>
    <h1
      class="max-w-[19ch] text-balance text-5xl leading-[1.05] tracking-tight md:text-6xl"
    >
      {landing.heroTitle}
    </h1>
    <p class="max-w-[52ch] text-pretty text-[19px] leading-[1.6] text-fg-2">
      {landing.heroIntro}
    </p>
    <div class="mt-1.5 flex flex-wrap justify-center gap-2.5">
      <Button variant="accent" size="lg" href={marketingReaderHomeHref(locale)}
        >{landing.primaryCta}</Button
      >
      {#if aboutHref}
        <Button variant="ghost" size="lg" href={aboutHref}
          >{landing.secondaryCta}</Button
        >
      {/if}
    </div>
  </Container>
</section>

<section id="today" class="scroll-mt-20 py-16">
  <Container class="max-w-[1180px] flex flex-col gap-10">
    <div class="flex flex-col gap-3">
      <Eyebrow class="text-accent">{landing.todayEyebrow}</Eyebrow>
      <h2 class="max-w-[24ch] text-[32px] leading-[1.12] tracking-[-0.025em]">
        {landing.todayTitle}
      </h2>
      <p class="max-w-[56ch] text-[17px] leading-[1.6] text-fg-2">
        {landing.todayIntro}
      </p>
    </div>
    <div class="grid grid-cols-1 gap-[18px] sm:grid-cols-2 lg:grid-cols-4">
      {#each landing.values as value (value.id)}
        <div
          class="flex flex-col gap-2 rounded-[14px] border border-line bg-bg-1 px-6 py-7 transition-colors hover:border-line-2"
        >
          <div
            class="mb-2 flex size-8 items-center justify-center rounded-[9px] {value.chip}"
          >
            <Icon
              name={value.icon}
              size={16}
              class={locale === "ar" && value.icon === "arrow-right" ? "rotate-180" : ""}
            />
          </div>
          <div class="text-[17px] font-semibold">{value.title}</div>
          <p class="text-[14.5px] leading-[1.6] text-fg-2">{value.body}</p>
        </div>
      {/each}
    </div>
  </Container>
</section>

<section class="border-t border-line">
  <Container class="max-w-[1180px] flex flex-col gap-8 py-[72px]">
    <div class="flex flex-col gap-3">
      <Eyebrow class="text-accent">All 114 surahs</Eyebrow>
      <h2 class="max-w-[24ch] text-[32px] leading-[1.12] tracking-[-0.025em]">
        The whole Qur&rsquo;an, one tap away.
      </h2>
      <p class="max-w-[56ch] text-[17px] leading-[1.6] text-fg-2">
        Every chapter opens straight into the reader &mdash; no account, no loading screen.
      </p>
    </div>
    <ul class="grid grid-cols-1 gap-2.5 sm:grid-cols-2 lg:grid-cols-3">
      {#each data.surahs as s (s.num)}
        <li>
          <a
            href={publicHref(readerHrefFor(locale, surahPathFor(arabicCtx, s)))}
            class="group flex items-center gap-3.5 rounded-[13px] border border-line bg-bg-1 px-4 py-3 transition-colors hover:border-accent-line hover:bg-bg-2"
          >
            <span
              class="grid size-9 shrink-0 rotate-45 place-items-center rounded-[9px] border border-line-2 text-[12.5px] font-semibold text-fg-3 transition-colors group-hover:border-accent-line group-hover:text-accent"
            >
              <span class="-rotate-45">{s.num}</span>
            </span>
            <span class="flex min-w-0 flex-col">
              <span class="truncate text-[15px] font-semibold">{s.name}</span>
              <span class="truncate text-[12.5px] text-fg-3">
                {s.meaning} · {s.ayahCount} ayahs · {s.place}
              </span>
            </span>
            <span
              dir="rtl"
              class="ml-auto shrink-0 font-arabic text-[19px] leading-none text-fg-2 transition-colors group-hover:text-accent"
              >{s.arabic}</span
            >
          </a>
        </li>
      {/each}
    </ul>
  </Container>
</section>

<section id="roadmap" class="scroll-mt-20 border-t border-line bg-bg-elev">
  <Container class="max-w-[1180px] flex flex-col gap-10 py-[72px]">
    <div class="flex flex-col gap-3">
      <Eyebrow class="text-fg-3">{landing.roadmapEyebrow}</Eyebrow>
      <h2 class="max-w-[22ch] text-[32px] leading-[1.12] tracking-[-0.025em]">
        {landing.roadmapTitle}
      </h2>
      <p class="max-w-[60ch] text-[17px] leading-[1.6] text-fg-2">
        {landing.roadmapIntro}
      </p>
    </div>
    <div class="grid grid-cols-1 gap-[18px] sm:grid-cols-2 lg:grid-cols-4">
      {#each landing.roadmap as item (item.id)}
        <div class="flex flex-col gap-2 rounded-[14px] border border-line bg-bg-1 px-6 py-7">
          <span class="text-[11px] font-semibold uppercase tracking-[0.12em] text-fg-3"
            >{landing.coming}</span
          >
          <div class="text-[17px] font-semibold">{item.title}</div>
          <p class="text-[14.5px] leading-[1.6] text-fg-2">{item.body}</p>
        </div>
      {/each}
    </div>
  </Container>
</section>
