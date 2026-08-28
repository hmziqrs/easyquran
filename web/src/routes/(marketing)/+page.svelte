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
      class="inline-flex items-center gap-2 rounded-pill bg-primary-soft px-3.5 py-[7px] text-body-s font-medium text-primary"
    >
      {landing.badge}
    </span>
    <h1
      class="max-w-[19ch] text-balance text-display-l tracking-tight md:text-display-xl"
    >
      {landing.heroTitle}
    </h1>
    <p class="max-w-[52ch] text-pretty text-body-xl text-foreground-secondary">
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
      <Eyebrow>{landing.todayEyebrow}</Eyebrow>
      <h2 class="max-w-[24ch] text-h2">
        {landing.todayTitle}
      </h2>
      <p class="max-w-[56ch] text-body-l text-foreground-secondary">
        {landing.todayIntro}
      </p>
    </div>
    <div class="grid grid-cols-1 gap-[18px] sm:grid-cols-2 lg:grid-cols-4">
      {#each landing.values as value (value.id)}
        <div
          class="flex flex-col gap-2 rounded-md border border-border bg-surface px-6 py-7 transition-colors hover:border-border-strong"
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
          <div class="text-body-l font-semibold">{value.title}</div>
          <p class="text-body-s leading-[1.6] text-foreground-secondary">{value.body}</p>
        </div>
      {/each}
    </div>
  </Container>
</section>

<section class="border-t border-border">
  <Container class="max-w-[1180px] flex flex-col gap-8 py-[72px]">
    <div class="flex flex-col gap-3">
      <Eyebrow>All 114 surahs</Eyebrow>
      <h2 class="max-w-[24ch] text-h2">
        The whole Qur&rsquo;an, one tap away.
      </h2>
      <p class="max-w-[56ch] text-body-l text-foreground-secondary">
        Every chapter opens straight into the reader &mdash; no account, no loading screen.
      </p>
    </div>
    <ul class="grid grid-cols-1 gap-2.5 sm:grid-cols-2 lg:grid-cols-3">
      {#each data.surahs as s (s.num)}
        <li>
          <a
            href={publicHref(readerHrefFor(locale, surahPathFor(arabicCtx, s)))}
            class="group flex items-center gap-3.5 rounded-md border border-border bg-surface px-4 py-3 transition-colors hover:border-primary hover:bg-surface-hover"
          >
            <span
              class="grid size-9 shrink-0 rotate-45 place-items-center rounded-sm border border-border-strong text-caption font-semibold text-muted transition-colors group-hover:border-primary group-hover:text-primary"
            >
              <span class="-rotate-45">{s.num}</span>
            </span>
            <span class="flex min-w-0 flex-col">
              <span class="truncate text-body font-semibold">{s.name}</span>
              <span class="truncate text-caption text-muted">
                {s.meaning} · {s.ayahCount} ayahs · {s.place}
              </span>
            </span>
            <span
              dir="rtl"
              class="ms-auto shrink-0 font-arabic text-body-l leading-none text-foreground-secondary transition-colors group-hover:text-primary"
              >{s.arabic}</span
            >
          </a>
        </li>
      {/each}
    </ul>
  </Container>
</section>

<section id="roadmap" class="scroll-mt-20 border-t border-border bg-surface-raised">
  <Container class="max-w-[1180px] flex flex-col gap-10 py-[72px]">
    <div class="flex flex-col gap-3">
      <Eyebrow>{landing.roadmapEyebrow}</Eyebrow>
      <h2 class="max-w-[22ch] text-h2">
        {landing.roadmapTitle}
      </h2>
      <p class="max-w-[60ch] text-body-l text-foreground-secondary">
        {landing.roadmapIntro}
      </p>
    </div>
    <div class="grid grid-cols-1 gap-[18px] sm:grid-cols-2 lg:grid-cols-4">
      {#each landing.roadmap as item (item.id)}
        <div class="flex flex-col gap-2 rounded-md border border-border bg-surface px-6 py-7">
          <span class="text-micro uppercase tracking-[0.12em] text-muted"
            >{landing.coming}</span
          >
          <div class="text-body-l font-semibold">{item.title}</div>
          <p class="text-body-s leading-[1.6] text-foreground-secondary">{item.body}</p>
        </div>
      {/each}
    </div>
  </Container>
</section>
