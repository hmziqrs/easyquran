<script lang="ts">
  import "./layout.css";
  import favicon from "$lib/assets/favicon.svg";
  import { onMount } from "svelte";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { Nav } from "$lib/components/nav";
  import { Footer } from "$lib/components/footer";
  import { Tweaks } from "$lib/components/tweaks";
  import { SITE } from "$lib/config/site";

  let { children } = $props();

  // The inline <head> script in app.html already applied saved prefs before
  // paint; re-apply on mount so the reactive store and the DOM stay in sync.
  // `onMount` only runs in the browser, so this is also where analytics is
  // safe to start (gtag.js + cookies can't run during SSR).
  onMount(() => {
    prefs.apply();
    // Firebase is imported dynamically so its SDK + config never enter the
    // critical modulepreload graph — analytics starts only after hydration.
    void import("$lib/firebase").then(({ initAnalytics }) => initAnalytics());
  });

  // Site-level structured data. A @graph of WebSite + Organization, each with
  // a stable @id so per-page WebPage/Breadcrumb nodes (emitted by <Seo>) can
  // cross-reference them. Built as a complete <script> string and emitted via
  // {@html} because Svelte treats <script> tag bodies as raw text and will not
  // interpolate {expressions} in them.
  const jsonLd = {
    "@context": "https://schema.org",
    "@graph": [
      {
        "@type": "WebSite",
        "@id": `${SITE.url}/#website`,
        name: SITE.name,
        url: SITE.url,
        inLanguage: "en",
        publisher: { "@id": `${SITE.url}/#organization` },
      },
      {
        "@type": "Organization",
        "@id": `${SITE.url}/#organization`,
        name: SITE.name,
        url: SITE.url,
        logo: {
          "@type": "ImageObject",
          url: `${SITE.url}/logo.png`,
          width: 512,
          height: 512,
        },
        sameAs: [SITE.github],
      },
    ],
  };
  // Closing end-tag built from two pieces so its literal never appears in
  // source (the parser ends this code block on that sequence).
  const jsonLdScript =
    `<script type="application/ld+json">${JSON.stringify(jsonLd)}` + "<" + "/script>";
</script>

<svelte:head>
  <link rel="icon" type="image/png" sizes="32x32" href="/icons/icon-32.png" />
  <link rel="icon" type="image/png" sizes="16x16" href="/icons/icon-16.png" />
  <link rel="icon" type="image/svg+xml" href={favicon} />
  <link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png" />
  <link rel="shortcut icon" href="/favicon.ico" />
  <link rel="manifest" href="/manifest.webmanifest" />
  <meta name="application-name" content={SITE.name} />
  <!-- eslint-disable-next-line svelte/no-at-html-tags -- structured data, safe by construction -->
  {@html jsonLdScript}
</svelte:head>

<a
  href="#main"
  class="sr-only focus:not-sr-only focus:absolute focus:left-3 focus:top-3 focus:z-[100] focus:rounded focus:bg-bg-1 focus:px-3 focus:py-2 focus:text-sm focus:text-fg focus:shadow-lg"
  >Skip to content</a
>
<Nav />
<main id="main" tabindex="-1">{@render children()}</main>
<Footer />
<Tweaks />
