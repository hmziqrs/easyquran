<script lang="ts">
  import { SITE } from "$lib/config/site";
  import { Icon } from "$lib/components/icon";
  import { externalLinkAttrs } from "$lib/utils";
  import type { OwnerPublic } from "$lib/types/owner";
  import type {
    FooterLink,
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
  const link = "text-body-s text-foreground-secondary transition-colors hover:text-foreground";
</script>

<footer class="border-t border-border bg-surface-raised">
  <div class="mx-auto max-w-[1180px] px-6 sm:px-7">
    <div
      class="grid grid-cols-2 gap-8 pt-[52px] pb-10 md:grid-cols-[1.4fr_1fr_1fr_1fr] md:gap-9"
    >
      <div class="col-span-2 flex flex-col gap-3 md:col-span-1">
        <div class="flex items-center gap-2.5">
          <span
            class="flex size-7 items-center justify-center rounded-sm bg-primary font-arabic text-[16px] leading-none text-primary-foreground"
            lang="ar"
            dir="rtl"
            aria-hidden="true">ق</span
          >
          <span class="text-body-l font-semibold tracking-[-0.02em] text-foreground">{brand}</span>
        </div>
        <p class="max-w-[30ch] text-body-s leading-relaxed text-foreground-secondary">
          {copy.blurb}
        </p>
        <div class="flex items-center gap-2">
          <a
            href={owner.x}
            aria-label={copy.socialX}
            {...externalLinkAttrs(owner.x, { me: true })}
            class="inline-flex size-8 items-center justify-center rounded-md border border-border text-muted transition-colors hover:border-border-strong hover:text-foreground"
          >
            <Icon name="x-brand" size={15} />
          </a>
        </div>
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

    <div class="flex flex-wrap items-center justify-between gap-5 pb-11">
      <span class="text-body-s text-muted">
        © {year} {brand}. {copy.builtBy}
        <a
          class="text-foreground-secondary underline underline-offset-2 hover:text-foreground"
          href={SITE.makerUrl}
          {...externalLinkAttrs(SITE.makerUrl)}>oxlabs.dev</a
        > · {copy.projectBy}
        <a
          class="text-foreground-secondary underline underline-offset-2 hover:text-foreground"
          href={SITE.ownerUrl}
          {...externalLinkAttrs(SITE.ownerUrl)}>hmziq.rs</a
        >.
      </span>
      <span lang="ar" dir="rtl" class="font-arabic text-body-l leading-none text-muted">
        وَنَزَّلْنَا عَلَيْكَ الْكِتَابَ تِبْيَانًا لِّكُلِّ شَيْءٍ
      </span>
    </div>
  </div>
</footer>
