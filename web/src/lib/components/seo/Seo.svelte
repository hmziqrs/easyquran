<!--
  Seo — page head metadata. Takes the route path; title + description are
  looked up from PAGE_META. Renders one svelte:head block with the title,
  description, canonical, robots directive, Open Graph (incl. og:image),
  Twitter (summary_large_image), markdown/plain alternate links, and
  per-page structured data: a WebPage (or subtype) node, a BreadcrumbList on
  inner pages, and an optional FAQPage when `faq` is passed.
-->
<script lang="ts">
  import { SITE, PAGE_META, NAV_PAGES } from "$lib/config/site";

  type FaqItem = { q: string; a: string };

  let {
    path,
    /** schema.org @type for the page entity (e.g. "AboutPage"). Defaults to "WebPage". */
    schemaSubtype = "WebPage",
    /** optional Q&A; when present, a FAQPage graph is emitted. */
    faq,
    /** extra JSON-LD objects to emit as their own application/ld+json scripts. */
    extraLd,
  }: {
    path: string;
    schemaSubtype?: string;
    faq?: FaqItem[];
    extraLd?: Record<string, unknown>[];
  } = $props();

  const meta = $derived(Object.values(PAGE_META).find((p) => p.path === path));
  const title = $derived(meta?.title ?? SITE.name);
  const description = $derived(meta?.description ?? SITE.tagline);

  const base = $derived(path === "/" ? "/index" : path);
  const mdHref = $derived(base + ".md");
  const txtHref = $derived(base + ".txt");
  const canonical = $derived(SITE.url + path);
  const ogImage = $derived(`${SITE.url}/og.png`);

  const current = $derived(NAV_PAGES.find((p) => p.href === path));

  // Per-page WebPage entity (or subtype). Cross-references the site-wide
  // WebSite + Organization @ids emitted in +layout.svelte.
  const webpageLd = $derived({
    "@context": "https://schema.org",
    "@type": schemaSubtype,
    "@id": canonical + "#webpage",
    url: canonical,
    name: title,
    description,
    inLanguage: "en",
    isPartOf: { "@id": `${SITE.url}/#website` },
    about: { "@id": `${SITE.url}/#organization` },
  });

  // Breadcrumb on every inner page (Google omits it on the root).
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
  <title>{title}</title>
  <meta name="description" content={description} />
  <link rel="canonical" href={canonical} />
  <meta
    name="robots"
    content="index, follow, max-image-preview:large, max-snippet:-1, max-video-preview:-1"
  />

  <meta property="og:site_name" content={SITE.name} />
  <meta property="og:type" content="website" />
  <meta property="og:locale" content="en_US" />
  <meta property="og:title" content={title} />
  <meta property="og:description" content={description} />
  <meta property="og:url" content={canonical} />
  <meta property="og:image" content={ogImage} />
  <meta property="og:image:secure_url" content={ogImage} />
  <meta property="og:image:type" content="image/png" />
  <meta property="og:image:width" content="1200" />
  <meta property="og:image:height" content="630" />
  <meta property="og:image:alt" content={title} />

  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content={title} />
  <meta name="twitter:description" content={description} />
  <meta name="twitter:image" content={ogImage} />
  <meta name="twitter:image:alt" content={title} />

  <link rel="alternate" type="text/markdown" href={mdHref} />
  <link rel="alternate" type="text/plain" href={txtHref} />

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
</svelte:head>
