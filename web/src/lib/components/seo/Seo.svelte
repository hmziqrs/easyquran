<!--
  Seo — page head metadata. Takes the route path; title + description are
  looked up from PAGE_META (or overridden explicitly for dynamic pages like
  /app/<slug>). Renders one svelte:head block with the title, description,
  canonical, robots directive, Open Graph (incl. og:image), Twitter
  (summary_large_image), markdown/plain alternate links (marketing pages only),
  and per-page structured data: a WebPage (or subtype) node, a BreadcrumbList on
  inner pages, and an optional FAQPage when `faq` is passed.
-->
<script lang="ts">
  import { SITE, PAGE_META, MARKETING_PAGES } from "$lib/config/site";

  type FaqItem = { q: string; a: string };

  let {
    path,
    /** schema.org @type for the page entity (e.g. "AboutPage"). Defaults to "WebPage". */
    schemaSubtype = "WebPage",
    /** explicit title override (dynamic pages like /app/<slug>); else PAGE_META. */
    title,
    /** explicit description override; else PAGE_META. */
    description,
    /** optional Q&A; when present, a FAQPage graph is emitted. */
    faq,
    /** extra JSON-LD objects to emit as their own application/ld+json scripts. */
    extraLd,
    /** emit the .md/.txt alternate links (marketing pages only; default true). */
    includeTextVariants = true,
    /**
     * Keep the page out of search indexes. The /app index route uses this; the
     * per-surah /app/<slug> pages are now indexable.
     */
    noindex = false,
  }: {
    path: string;
    schemaSubtype?: string;
    title?: string;
    description?: string;
    faq?: FaqItem[];
    extraLd?: Record<string, unknown>[];
    includeTextVariants?: boolean;
    noindex?: boolean;
  } = $props();

  const meta = $derived(Object.values(PAGE_META).find((p) => p.path === path));
  const pageTitle = $derived(title ?? meta?.title ?? SITE.name);
  const pageDescription = $derived(description ?? meta?.description ?? SITE.tagline);

  const base = $derived(path === "/" ? "/index" : path);
  const mdHref = $derived(base + ".md");
  const txtHref = $derived(base + ".txt");
  const canonical = $derived(SITE.url + path);
  const ogImage = $derived(`${SITE.url}/og.png`);

  const current = $derived(MARKETING_PAGES.find((p) => p.href === path));

  // Per-page WebPage entity (or subtype). Cross-references the site-wide
  // WebSite + Organization @ids emitted in +layout.svelte.
  const webpageLd = $derived({
    "@context": "https://schema.org",
    "@type": schemaSubtype,
    "@id": canonical + "#webpage",
    url: canonical,
    name: pageTitle,
    description: pageDescription,
    inLanguage: "en",
    isPartOf: { "@id": `${SITE.url}/#website` },
    about: { "@id": `${SITE.url}/#organization` },
  });

  // Breadcrumb on every inner marketing page (Google omits it on the root).
  const breadcrumbLd = $derived(
    path !== "/" && current
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

  // Build a complete application/ld+json <script> string. The closing
  // end-tag is split so its literal never appears in this source file.
  const ld = (obj: Record<string, unknown>) =>
    `<script type="application/ld+json">${JSON.stringify(obj)}` + "<" + "/script>";
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
    <meta property="og:locale" content="en_US" />
    <meta property="og:title" content={pageTitle} />
    <meta property="og:description" content={pageDescription} />
    <meta property="og:url" content={canonical} />
    <meta property="og:image" content={ogImage} />
    <meta property="og:image:secure_url" content={ogImage} />
    <meta property="og:image:type" content="image/png" />
    <meta property="og:image:width" content="1200" />
    <meta property="og:image:height" content="630" />
    <meta property="og:image:alt" content={pageTitle} />

    <meta name="twitter:card" content="summary_large_image" />
    <meta name="twitter:title" content={pageTitle} />
    <meta name="twitter:description" content={pageDescription} />
    <meta name="twitter:image" content={ogImage} />
    <meta name="twitter:image:alt" content={pageTitle} />

    {#if includeTextVariants}
      <link rel="alternate" type="text/markdown" href={mdHref} />
      <link rel="alternate" type="text/plain" href={txtHref} />
    {/if}

    <!-- eslint-disable-next-line svelte/no-at-html-tags -- structured data, safe by construction -->
    {@html ld(webpageLd)}
    {#if breadcrumbLd}
      <!-- eslint-disable-next-line svelte/no-at-html-tags -- structured data, safe by construction -->
      {@html ld(breadcrumbLd)}
    {/if}
    {#if faqLd}
      <!-- eslint-disable-next-line svelte/no-at-html-tags -- structured data, safe by construction -->
      {@html ld(faqLd)}
    {/if}
    {#if extraLd}
      {#each extraLd as node, idx (idx)}
        <!-- eslint-disable-next-line svelte/no-at-html-tags -- structured data, safe by construction -->
        {@html ld(node)}
      {/each}
    {/if}
  {/if}
</svelte:head>
