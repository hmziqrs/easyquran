<script lang="ts">
  import { SITE } from "$lib/config/site";
  import {
    marketingHomeHref,
    resolveMarketingCopy,
    type MarketingLocale,
  } from "$lib/i18n/marketing-copy";
  import { UI_LOCALES } from "$lib/i18n/locales";

  let { locale }: { locale: MarketingLocale } = $props();

  const copy = $derived(resolveMarketingCopy(locale));
  const path = $derived(marketingHomeHref(locale));
  const canonical = $derived(`${SITE.url}${path}`);
  const image = `${SITE.url}/og.png`;
  const pageJsonLd = $derived(
    JSON.stringify({
      "@context": "https://schema.org",
      "@type": "WebPage",
      "@id": `${canonical}#webpage`,
      url: canonical,
      name: copy.seo.title,
      description: copy.seo.description,
      inLanguage: locale,
      isPartOf: { "@id": `${SITE.url}/#website` },
      about: { "@id": `${SITE.url}/#organization` },
    }),
  );
</script>

<svelte:head>
  <title>{copy.seo.title}</title>
  <meta name="description" content={copy.seo.description} />
  <link rel="canonical" href={canonical} />
  <link rel="alternate" hreflang="en" href={`${SITE.url}/`} />
  <link rel="alternate" hreflang="ar" href={`${SITE.url}/ar/`} />
  <link rel="alternate" hreflang="x-default" href={`${SITE.url}/`} />
  <meta
    name="robots"
    content="index, follow, max-image-preview:large, max-snippet:-1, max-video-preview:-1"
  />

  <meta property="og:site_name" content={SITE.name} />
  <meta property="og:type" content="website" />
  <meta property="og:locale" content={UI_LOCALES[locale].openGraphLocale} />
  <meta
    property="og:locale:alternate"
    content={UI_LOCALES[locale === "en" ? "ar" : "en"].openGraphLocale}
  />
  <meta property="og:title" content={copy.seo.title} />
  <meta property="og:description" content={copy.seo.description} />
  <meta property="og:url" content={canonical} />
  <meta property="og:image" content={image} />
  <meta property="og:image:secure_url" content={image} />
  <meta property="og:image:type" content="image/png" />
  <meta property="og:image:width" content="1200" />
  <meta property="og:image:height" content="630" />
  <meta property="og:image:alt" content={copy.seo.imageAlt} />

  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content={copy.seo.title} />
  <meta name="twitter:description" content={copy.seo.description} />
  <meta name="twitter:image" content={image} />
  <meta name="twitter:image:alt" content={copy.seo.imageAlt} />

  {#if locale === "en"}
    <link rel="alternate" type="text/markdown" href="/index.md" />
    <link rel="alternate" type="text/plain" href="/index.txt" />
  {/if}

  <svelte:element this={"script"} type="application/ld+json">{pageJsonLd}</svelte:element>
</svelte:head>
