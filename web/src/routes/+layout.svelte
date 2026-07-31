<!--
  Root layout — everything that is true of every route in both groups: the
  stylesheet, the icon/manifest head, site-level structured data, preference
  application and analytics.

  The page chrome lives in the group layouts instead:
    • (marketing)/+layout.svelte  — Nav + Footer + Tweaks
    • (application)/app/+layout.svelte — the app shell
  Each of those renders its own <main id="main">, which the skip link targets.
-->
<script lang="ts">
  import "./layout.css";
  import favicon from "$lib/assets/favicon.svg";
  import { onMount } from "svelte";
  import { afterNavigate } from "$app/navigation";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { consent } from "$lib/stores/consent.svelte";
  import { notifications } from "$lib/stores/notifications.svelte";
  import { NotificationToast } from "$lib/components/notifications";
  import { DownloadBar } from "$lib/components/status";
  import { SITE } from "$lib/config/site";
  import { startServiceWorker } from "$lib/boot/service-worker";
  import { startAnalytics } from "$lib/boot/analytics";
  import { startCrashReporting } from "$lib/boot/crash-reporting";
  import { startOfflineEngine } from "$lib/boot/offline-engine";

  let { children } = $props();

  // The offline Quran engine (~2.5 MB corpus + sqlite-wasm worker boot) is
  // GATED to /app so marketing routes (/, /about, …) don't pay the bandwidth,
  // battery, and main-thread-contension cost. The reader always paints from
  // prerendered page.data regardless of when (or whether) it boots. The root
  // onMount fires once at initial entry; afterNavigate covers subsequent
  // marketing → /app navigations. startOfflineEngine is idempotent, so calling
  // it on every /app entry is safe and cheap.
  let offlineTeardown: (() => void) | null = null;
  const ensureOfflineEngine = (pathname: string): void => {
    if (!pathname.startsWith("/app")) return;
    if (offlineTeardown) return;
    offlineTeardown = startOfflineEngine();
  };

  // The inline <head> script in app.html already applied saved prefs before
  // paint; re-apply on mount so the reactive store and the DOM stay in sync.
  // `onMount` only runs in the browser, so this is also where Firebase is safe
  // to start (gtag.js + the SDK + cookies can't run during SSR/prerender).
  onMount(() => {
    // Hydrate durable state BEFORE starting Firebase so init reads real choices.
    consent.hydrate();
    prefs.hydrate();
    prefs.apply();
    notifications.hydrate();

    // Lifecycle-owned boot services — each returns its teardown so the listeners
    // they register (service worker, consent bridge, crash handlers) are removed
    // if the root layout ever unmounts. The consent bridge is the key fix: it
    // used to be registered inside a fire-and-forget IIFE with no way to remove
    // it (see lib/boot/analytics.ts).
    const cleanups = [startServiceWorker(), startAnalytics(), startCrashReporting()];

    // Boot the offline engine immediately when the user lands directly on /app.
    // (Marketing entries skip it; afterNavigate below starts it on /app entry.)
    ensureOfflineEngine(location.pathname);

    return () => {
      for (const teardown of cleanups) teardown();
      offlineTeardown?.();
      offlineTeardown = null;
    };
  });

  // The site is a prerendered SPA, so client-side route changes don't reload the
  // page. Two concerns run on each navigation:
  //   • log a screen/page view so GA4 sees them (skip the initial 'enter'
  //     navigation: onMount's analytics init already logged it, after consent),
  //   • start the offline engine when the user enters /app from a marketing
  //     route (idempotent — no-op if already booted on direct entry).
  afterNavigate((navigation) => {
    if (navigation.type !== "enter") {
      void import("$lib/firebase/analytics")
        .then(({ pageView }) => pageView(location.pathname))
        .catch(() => {
          /* analytics is best-effort */
        });
    }
    ensureOfflineEngine(location.pathname);
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
<NotificationToast />
<DownloadBar />
{@render children()}
