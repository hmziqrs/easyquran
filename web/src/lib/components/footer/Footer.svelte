<script lang="ts">
  import { SITE } from "#lib/config/site.js";
  import { externalLinkAttrs } from "#lib/utils.js";
  import type { OwnerPublic } from "#lib/types/owner.js";
  import type {
    FooterLink,
    FooterResolvedCopy,
    MarketingFooterLinks,
  } from "#lib/i18n/marketing-copy.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import BrandMark from "#lib/components/brand/BrandMark.svelte";

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

  const colHeading = "text-micro text-muted";
  const link = "text-[15.5px] font-bold text-foreground-secondary transition-colors hover:text-foreground";
</script>

<footer class="border-t border-border bg-surface">
  <div class="mx-auto w-full max-w-[1200px] px-6">
    <div
      class="grid grid-cols-2 gap-8 pt-[52px] pb-10 md:grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)_minmax(0,1fr)_minmax(0,1fr)] md:gap-10"
    >
      <div class="col-span-2 flex flex-col gap-3 md:col-span-1">
        <div class="flex items-center gap-2.5">
          <span
            class="flex size-[30px] items-center justify-center rounded-sm bg-primary text-primary-foreground"
            aria-hidden="true"><BrandMark class="size-[26px]" /></span
          >
          <span class="text-[19px] font-extrabold tracking-[-0.035em] text-foreground"
            >{brand.slice(0, 4)}<span class="text-primary">{brand.slice(4)}</span></span
          >
        </div>
        <p class="max-w-[34ch] text-body text-foreground-secondary">
          {copy.blurb}
        </p>
      </div>

      {#snippet linkColumn(heading: string, ariaLabel: string, items: FooterLink[])}
        <nav aria-label={ariaLabel} class="flex flex-col gap-2.5">
          <h2 class={colHeading}>{heading}</h2>
          <ul class="grid gap-2.5">
            {#each items as p (p.id)}
              <li><a class={link} href={publicHref(p.href)}>{p.label}</a></li>
            {/each}
          </ul>
        </nav>
      {/snippet}

      {@render linkColumn(copy.productHeading, copy.productLabel, links.product)}

      {#if links.company.length > 0}
        {@render linkColumn(copy.companyHeading, copy.companyLabel, links.company)}
      {/if}

      {#if links.legal.length > 0}
        {@render linkColumn(copy.legalHeading, copy.legalLabel, links.legal)}
      {/if}
    </div>

    <div class="flex flex-wrap items-center gap-5 pb-11">
      <span class="text-body text-muted">
        © {year} {brand}. {copy.builtBy}
        <a
          class="text-foreground-secondary underline underline-offset-2 hover:text-foreground"
          href={SITE.makerUrl}
          {...externalLinkAttrs(SITE.makerUrl)}>oxlabs.dev</a
        > · <a
          class="text-foreground-secondary underline underline-offset-2 hover:text-foreground"
          href={owner.x}
          {...externalLinkAttrs(owner.x, { me: true })}>{owner.xHandle}</a
        >
      </span>
      <span lang="ar" dir="rtl" class="ms-auto font-arabic text-body-l leading-none text-muted">
        وَنَزَّلْنَا عَلَيْكَ الْكِتَابَ تِبْيَانًا لِّكُلِّ شَيْءٍ
      </span>
    </div>
  </div>
</footer>
