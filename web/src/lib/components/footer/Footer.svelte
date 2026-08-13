<script lang="ts">
  import { SITE } from "$lib/config/site";
  import { Icon } from "$lib/components/icon";
  import { externalLinkAttrs } from "$lib/utils";
  import type { OwnerPublic } from "$lib/types/owner";
  import type {
    FooterResolvedCopy,
    MarketingFooterLinks,
  } from "$lib/i18n/marketing-copy";
  import { publicHref } from "$lib/i18n/public-href";

  let {
    owner,
    year,
    copy,
    links,
  }: {
    owner: OwnerPublic;
    year: number;
    copy: FooterResolvedCopy;
    links: MarketingFooterLinks;
  } = $props();

  const brand = SITE.name.toLowerCase();

  const colHeading = "eyebrow mb-0";
  const link = "text-[14.5px] text-fg-2 transition-colors hover:text-fg";
</script>

<footer class="border-t border-line bg-bg-elev">
  <div class="mx-auto max-w-[1180px] px-6 sm:px-7">
    <div
      class="grid grid-cols-2 gap-8 pt-[52px] pb-10 md:grid-cols-[1.4fr_1fr_1fr_1fr] md:gap-9"
    >
      <div class="col-span-2 flex flex-col gap-3 md:col-span-1">
        <div class="flex items-center gap-2.5">
          <span
            class="flex size-7 items-center justify-center rounded-[9px] bg-accent font-arabic text-[16px] leading-none text-accent-fg"
            lang="ar"
            dir="rtl"
            aria-hidden="true">ق</span
          >
          <span class="text-[17px] font-semibold tracking-[-0.02em] text-fg">{brand}</span>
        </div>
        <p class="max-w-[30ch] text-[14.5px] leading-relaxed text-fg-2">
          {copy.blurb}
        </p>
        <div class="flex items-center gap-2">
          <a
            href={owner.x}
            aria-label={copy.socialX}
            {...externalLinkAttrs(owner.x, { me: true })}
            class="inline-flex size-8 items-center justify-center rounded-lg border border-line-2 text-fg-3 transition-colors hover:border-line-3 hover:text-fg"
          >
            <Icon name="x-brand" size={15} />
          </a>
        </div>
      </div>

      <nav aria-label={copy.productLabel} class="flex flex-col gap-2.5">
        <h2 class={colHeading}>{copy.productHeading}</h2>
        <ul class="grid gap-2.5">
          {#each links.product as p (p.id)}
            <li><a class={link} href={publicHref(p.href)}>{p.label}</a></li>
          {/each}
        </ul>
      </nav>

      {#if links.company.length > 0}
        <nav aria-label={copy.companyLabel} class="flex flex-col gap-2.5">
          <h2 class={colHeading}>{copy.companyHeading}</h2>
          <ul class="grid gap-2.5">
            {#each links.company as p (p.id)}
              <li><a class={link} href={publicHref(p.href)}>{p.label}</a></li>
            {/each}
          </ul>
        </nav>
      {/if}

      {#if links.legal.length > 0}
        <nav aria-label={copy.legalLabel} class="flex flex-col gap-2.5">
          <h2 class={colHeading}>{copy.legalHeading}</h2>
          <ul class="grid gap-2.5">
            {#each links.legal as p (p.id)}
              <li><a class={link} href={publicHref(p.href)}>{p.label}</a></li>
            {/each}
          </ul>
        </nav>
      {/if}
    </div>

    <div class="flex flex-wrap items-center justify-between gap-5 pb-11">
      <span class="text-[13.5px] text-fg-3">
        © {year} {brand}. {copy.builtBy}
        <a
          class="text-fg-2 underline underline-offset-2 hover:text-fg"
          href={SITE.makerUrl}
          {...externalLinkAttrs(SITE.makerUrl)}>oxlabs.dev</a
        > · {copy.projectBy}
        <a
          class="text-fg-2 underline underline-offset-2 hover:text-fg"
          href={SITE.ownerUrl}
          {...externalLinkAttrs(SITE.ownerUrl)}>hmziq.rs</a
        >.
      </span>
      <span lang="ar" dir="rtl" class="font-arabic text-[17px] leading-none text-fg-3">
        وَنَزَّلْنَا عَلَيْكَ الْكِتَابَ تِبْيَانًا لِّكُلِّ شَيْءٍ
      </span>
    </div>
  </div>
</footer>
