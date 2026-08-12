<script lang="ts">
  import { page } from "$app/state";
  import { Button, Container, Eyebrow, Icon } from "$lib/components";
  import {
    marketingLocaleFromPath,
    marketingReaderHomeHref,
    resolveMarketingCopy,
  } from "$lib/i18n/marketing-copy";
  import { marketingHref } from "$lib/i18n/marketing";
  import MarketingSeo from "./_components/MarketingSeo.svelte";

  const locale = $derived(marketingLocaleFromPath(page.url.pathname));
  const copy = $derived(resolveMarketingCopy(locale));
  const aboutHref = $derived(marketingHref("about", locale));
</script>

<MarketingSeo {locale} />

<section class="pt-22 pb-7">
  <Container class="max-w-[1180px] flex flex-col items-center gap-[22px] text-center">
    <span
      class="inline-flex items-center gap-2 rounded-full bg-accent-soft px-3.5 py-[7px] text-[13px] font-medium text-accent"
    >
      {copy.landing.badge}
    </span>
    <h1
      class="max-w-[19ch] text-balance text-5xl leading-[1.05] tracking-tight md:text-6xl"
    >
      {copy.landing.heroTitle}
    </h1>
    <p class="max-w-[52ch] text-pretty text-[19px] leading-[1.6] text-fg-2">
      {copy.landing.heroIntro}
    </p>
    <div class="mt-1.5 flex flex-wrap justify-center gap-2.5">
      <Button variant="accent" size="lg" href={marketingReaderHomeHref(locale)}
        >{copy.landing.primaryCta}</Button
      >
      {#if aboutHref}
        <Button variant="ghost" size="lg" href={aboutHref}
          >{copy.landing.secondaryCta}</Button
        >
      {/if}
    </div>
  </Container>
</section>

<section id="today" class="scroll-mt-20 py-16">
  <Container class="max-w-[1180px] flex flex-col gap-10">
    <div class="flex flex-col gap-3">
      <Eyebrow class="text-accent">{copy.landing.todayEyebrow}</Eyebrow>
      <h2 class="max-w-[24ch] text-[32px] leading-[1.12] tracking-[-0.025em]">
        {copy.landing.todayTitle}
      </h2>
      <p class="max-w-[56ch] text-[17px] leading-[1.6] text-fg-2">
        {copy.landing.todayIntro}
      </p>
    </div>
    <div class="grid grid-cols-1 gap-[18px] sm:grid-cols-2 lg:grid-cols-4">
      {#each copy.landing.values as value (value.id)}
        <div class="flex flex-col gap-2 rounded-[14px] bg-bg-2 px-6 py-7">
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

<section id="roadmap" class="scroll-mt-20 border-t border-line bg-bg-2">
  <Container class="max-w-[1180px] flex flex-col gap-10 py-[72px]">
    <div class="flex flex-col gap-3">
      <Eyebrow class="text-fg-3">{copy.landing.roadmapEyebrow}</Eyebrow>
      <h2 class="max-w-[22ch] text-[32px] leading-[1.12] tracking-[-0.025em]">
        {copy.landing.roadmapTitle}
      </h2>
      <p class="max-w-[60ch] text-[17px] leading-[1.6] text-fg-2">
        {copy.landing.roadmapIntro}
      </p>
    </div>
    <div class="grid grid-cols-1 gap-[18px] sm:grid-cols-2 lg:grid-cols-4">
      {#each copy.landing.roadmap as item (item.id)}
        <div class="flex flex-col gap-2 rounded-[14px] border border-line-2 bg-bg-3 px-6 py-7">
          <span class="text-[11px] font-semibold uppercase tracking-[0.12em] text-fg-3"
            >{copy.landing.coming}</span
          >
          <div class="text-[17px] font-semibold">{item.title}</div>
          <p class="text-[14.5px] leading-[1.6] text-fg-2">{item.body}</p>
        </div>
      {/each}
    </div>
  </Container>
</section>
