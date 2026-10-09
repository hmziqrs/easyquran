<script lang="ts">
  import { page } from "$app/state";
  import { Container, Seo } from "#lib/components/index.js";
  import { SITE } from "#lib/config/site.js";
  import { resolveAboutCopy } from "#lib/i18n/about-copy.js";
  import { marketingLocaleFromPath } from "#lib/i18n/marketing-copy.js";
  import { publicHref } from "#lib/i18n/public-href.js";
  import { externalLinkAttrs } from "#lib/utils.js";

  let { data } = $props();

  const locale = $derived(marketingLocaleFromPath(page.url.pathname));
  const copy = $derived(resolveAboutCopy(locale, data.counts));

  /* Source names are proper nouns, not copy: they stay as published by each source. */
  type Source = { name: string; href?: string };
  const ARABIC_SOURCES: Source[] = [
    { name: "Tanzil.net", href: SITE.tanzilUrl },
    { name: "quran.com", href: "https://quran.com" },
    { name: "Dar Al-Islam" },
    { name: "Naveed Ahmad" },
    { name: "Al Quran Cloud", href: "https://alquran.cloud" },
  ];
  const TRANSLATION_SOURCES: Source[] = [
    { name: "Tanzil.net", href: SITE.tanzilUrl },
    { name: "QuranEnc", href: "https://quranenc.com" },
    { name: "QUL (Tarteel)", href: "https://qul.tarteel.ai" },
  ];
  const sourceGroups = $derived([
    { id: "arabic", label: copy.sourcesArabic, sources: ARABIC_SOURCES },
    { id: "translations", label: copy.sourcesTranslations, sources: TRANSLATION_SOURCES },
  ]);

  const link = "text-foreground underline underline-offset-2 transition-colors hover:text-primary";
</script>

<Seo
  path="/about"
  schemaSubtype="AboutPage"
  title={copy.seo.title}
  description={copy.seo.description}
  inLanguage={locale}
/>

<Container class="py-12 md:py-16">
  <div class="flex max-w-[680px] flex-col gap-10">
    <div class="flex flex-col gap-4">
      <h1 class="text-h1">{copy.heading}</h1>
      <p class="text-body-l text-foreground-secondary">{copy.intro}</p>
      <ul class="flex flex-col gap-2.5 text-body leading-relaxed text-foreground-secondary">
        {#each copy.features as feature (feature.id)}
          <li class="flex gap-3">
            <span class="mt-[0.7em] size-1.5 flex-none rounded-full bg-border-strong" aria-hidden="true"
            ></span>
            <span>{feature.body}</span>
          </li>
        {/each}
      </ul>
    </div>

    <section class="flex flex-col gap-3 border-t border-border pt-8">
      <h2 class="text-h3">{copy.sourcesHeading}</h2>
      <dl class="flex flex-col gap-2 text-body">
        {#each sourceGroups as group (group.id)}
          <div class="flex flex-col gap-0.5 sm:flex-row sm:gap-4">
            <dt class="flex-none text-muted sm:w-32">{group.label}</dt>
            <dd>
              <ul class="flex flex-wrap gap-x-4 gap-y-1 text-foreground-secondary">
                {#each group.sources as source (source.name)}
                  <li>
                    {#if source.href}
                      <a class={link} href={source.href} {...externalLinkAttrs(source.href)}
                        >{source.name}</a
                      >
                    {:else}
                      {source.name}
                    {/if}
                  </li>
                {/each}
              </ul>
            </dd>
          </div>
        {/each}
      </dl>
      <p class="text-caption text-muted">{copy.sourcesNote}</p>
    </section>

    <p class="border-t border-border pt-8 text-body text-foreground-secondary">
      {copy.creditProjectBy}
      <a class={link} href={SITE.ownerUrl} {...externalLinkAttrs(SITE.ownerUrl, { me: true })}
        >hmziq.rs</a
      >, {copy.creditBuiltBy}
      <a class={link} href={SITE.makerUrl} {...externalLinkAttrs(SITE.makerUrl, { me: true })}
        >oxlabs.dev</a
      >. <a class={link} href={publicHref("/contact")}>{copy.cta}</a>
    </p>
  </div>
</Container>
