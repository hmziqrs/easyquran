<script lang="ts">
  import { SITE, PAGE_META, MARKETING_PAGES } from "$lib/config/site";

  type FaqItem = { q: string; a: string };
  type Crumb = { name: string; href: string };

  const LOCALE_TERRITORY: Record<string, string> = {
    en: "en_US",
    ar: "ar_SA",
    ms: "ms_MY",
    id: "id_ID",
    ur: "ur_PK",
    fa: "fa_IR",
    zh: "zh_CN",
    tl: "tl_PH",
    bn: "bn_BD",
    hi: "hi_IN",
  };

  let {
    path,
    schemaSubtype = "WebPage",
    title,
    description,
    faq,
    extraLd,
    includeTextVariants = true,
    noindex = false,
    inLanguage = "en",
    crumbs,
  }: {
    path: string;
    schemaSubtype?: string;
    title?: string;
    description?: string;
    faq?: FaqItem[];
    extraLd?: Record<string, unknown>[];
    includeTextVariants?: boolean;
    noindex?: boolean;
    inLanguage?: string;
    crumbs?: Crumb[];
  } = $props();

  const meta = $derived(Object.values(PAGE_META).find((p) => p.path === path));
  const pageTitle = $derived(title ?? meta?.title ?? SITE.name);
  const pageDescription = $derived(description ?? meta?.description ?? SITE.tagline);

  const base = $derived(path === "/" ? "/index" : path);
  const mdHref = $derived(base + ".md");
  const txtHref = $derived(base + ".txt");
  const canonical = $derived(SITE.url + path);
  const ogImage = $derived(`${SITE.url}/og.png`);
  const ogImageAlt = `${SITE.name} — preview card`;
  const ogLocale = $derived(
    LOCALE_TERRITORY[inLanguage] ?? `${inLanguage}_${inLanguage.toUpperCase()}`,
  );

  const current = $derived(MARKETING_PAGES.find((p) => p.href === path));

  const webpageLd = $derived({
    "@context": "https://schema.org",
    "@type": schemaSubtype,
    "@id": canonical + "#webpage",
    url: canonical,
    name: pageTitle,
    description: pageDescription,
    inLanguage,
    isPartOf: { "@id": `${SITE.url}/#website` },
    about: { "@id": `${SITE.url}/#organization` },
  });

  const breadcrumbLd = $derived(
    crumbs && crumbs.length
      ? {
          "@context": "https://schema.org",
          "@type": "BreadcrumbList",
          itemListElement: crumbs.map((c, i) => ({
            "@type": "ListItem",
            position: i + 1,
            name: c.name,
            item: `${SITE.url}${c.href}`,
          })),
        }
      : path !== "/" && current
        ? {
            "@context": "https://schema.org",
            "@type": "BreadcrumbList",
            itemListElement: [
              { "@type": "ListItem", position: 1, name: "Home", item: `${SITE.url}/` },
              { "@type": "ListItem", position: 2, name: current.label, item: canonical },
            ],
          }
        : null,
  );

  const faqLd = $derived(
    faq && faq.length
      ? {
          "@context": "https://schema.org",
          "@type": "FAQPage",
          mainEntity: faq.map((i) => ({
            "@type": "Question",
            name: i.q,
            acceptedAnswer: { "@type": "Answer", text: i.a },
          })),
        }
      : null,
  );

  const ld = (obj: Record<string, unknown>) =>
    `<script type="application/ld+json">${
      JSON.stringify(obj)
        .replace(/</g, "\\u003c")
        .replace(/\u2028/g, "\\u2028")
        .replace(/\u2029/g, "\\u2029")
    }` + "<" + "/script>";

  const ldNodes = $derived(
    [
      webpageLd,
      breadcrumbLd,
      faqLd,
      ...(extraLd ?? []).filter(
        (n) => n["@type"] !== "WebSite" && n["@type"] !== "Organization",
      ),
    ].filter(Boolean) as Record<string, unknown>[],
  );
</script>

<svelte:head>
  <title>{pageTitle}</title>
  <meta name="description" content={pageDescription} />

  {#if noindex}
    <meta name="robots" content="noindex, follow" />
  {:else}
    <link rel="canonical" href={canonical} />
    <meta
      name="robots"
      content="index, follow, max-image-preview:large, max-snippet:-1, max-video-preview:-1"
    />

    <meta property="og:site_name" content={SITE.name} />
    <meta property="og:type" content="website" />
    <meta property="og:locale" content={ogLocale} />
    <meta property="og:title" content={pageTitle} />
    <meta property="og:description" content={pageDescription} />
    <meta property="og:url" content={canonical} />
    <meta property="og:image" content={ogImage} />
    <meta property="og:image:secure_url" content={ogImage} />
    <meta property="og:image:type" content="image/png" />
    <meta property="og:image:width" content="1200" />
    <meta property="og:image:height" content="630" />
    <meta property="og:image:alt" content={ogImageAlt} />

    <meta name="twitter:card" content="summary_large_image" />
    <meta name="twitter:title" content={pageTitle} />
    <meta name="twitter:description" content={pageDescription} />
    <meta name="twitter:image" content={ogImage} />
    <meta name="twitter:image:alt" content={ogImageAlt} />

    {#if includeTextVariants}
      <link rel="alternate" type="text/markdown" href={mdHref} />
      <link rel="alternate" type="text/plain" href={txtHref} />
    {/if}

    {#each ldNodes as node, idx (idx)}
      {@html ld(node)}
    {/each}
  {/if}
</svelte:head>
