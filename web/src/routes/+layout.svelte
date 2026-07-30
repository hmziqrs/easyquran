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
  import { SITE } from "$lib/config/site";
  import { bootOfflineEngine } from "$lib/quran/offline";

  let { children } = $props();

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

    // Register the root Service Worker (app-shell + FCM) in production so the
    // reader is offline-capable after first visit. Skipped in dev (it would
    // cache over HMR). firebase/messaging.ts re-uses this same registration.
    if (import.meta.env.PROD && "serviceWorker" in navigator) {
      navigator.serviceWorker.register("/sw.js", { scope: "/" }).catch((err) => {
        console.warn("[sw] registration failed:", err);
      });
    }

    // Proactively cache the Quran for offline use as soon as the page mounts —
    // don't wait for the reader to be opened. The two Arabic DBs (~2.5 MB total)
    // download in a background Worker into OPFS, so the first visit to /app is
    // already cached. Best-effort (it never blocks the main thread or first
    // paint); the reader renders from prerendered data regardless.
    void bootOfflineEngine();

    // Lazy-import the feature modules so the Firebase SDK never enters the
    // critical modulepreload graph — it starts only after hydration.
    void (async () => {
      // fbAnalytics stays undefined if the dynamic import or init fails; the
      // consent bridge below is then a safe no-op, so a later retry or manual
      // consent change isn't silently lost.
      let fbAnalytics: typeof import("$lib/firebase/analytics") | undefined;

      // Push the user's consent choices into GA4 consent mode + the analytics
      // collection toggle, and re-apply whenever they change (Settings panel).
      // (Performance has no runtime toggle — see lib/firebase/performance.ts —
      // it's consent-gated only at init above; the control reloads to apply it.)
      const applyConsent = () => {
        if (!fbAnalytics) return;
        fbAnalytics.setConsentState(consent.consentSettings);
        fbAnalytics.setAnalyticsCollectionEnabled(consent.analytics);
      };

      try {
        fbAnalytics = await import("$lib/firebase/analytics");
        const fbPerf = await import("$lib/firebase/performance");

        // Start analytics + performance, gated by consent. (Performance flags
        // are honored at init; analytics can be toggled freely at runtime.)
        await fbAnalytics.initAnalytics();
        fbPerf.initPerformance({
          dataCollectionEnabled: consent.performance,
          instrumentationEnabled: consent.performance,
        });
        applyConsent();

        // First-load page view (after consent is applied, so the first event
        // respects the user's consent-mode state).
        fbAnalytics.pageView(location.pathname);
      } catch (err) {
        // Analytics is best-effort — a failed dynamic import or init must not
        // throw unhandled or disable the consent bridge registered below.
        console.warn("[firebase] init failed:", err);
      }

      // Register the consent bridge outside the try/catch so it survives an
      // init failure (applyConsent is a no-op until fbAnalytics is assigned).
      window.addEventListener("easyquran:consent", applyConsent);
    })();

    // Crash reporting — forward uncaught errors + unhandled promise rejections to
    // GA4 as exceptions (Crashlytics has no web SDK; this is the Firebase-native
    // equivalent). Consent-gated: logException → track() drops it while collection
    // is off. Dynamic-imported so the analytics module stays out of the critical
    // bundle (errors are rare; the module caches after first load).
    const reportException = (description: string) =>
      import("$lib/firebase/analytics")
        .then(({ logException }) => logException(description, true))
        .catch(() => {
          /* crash reporting is best-effort */
        });
    const onError = (event: ErrorEvent) =>
      reportException(
        `Uncaught: ${event.message} @ ${event.filename}:${event.lineno}:${event.colno}`,
      );
    const onRejection = (event: PromiseRejectionEvent) => {
      const reason =
        event.reason instanceof Error
          ? `${event.reason.name}: ${event.reason.message}`
          : String(event.reason);
      reportException(`Unhandled rejection: ${reason}`);
    };
    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onRejection);

    return () => {
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onRejection);
    };
  });

  // The site is a prerendered SPA, so client-side route changes don't reload the
  // page — log a screen/page view on each navigation so GA4 sees them. Skip the
  // initial 'enter' navigation: the onMount call above already logged it (and
  // runs after consent is applied), so this fires only on subsequent navigations.
  afterNavigate((navigation) => {
    if (navigation.type === "enter") return;
    void import("$lib/firebase/analytics")
      .then(({ pageView }) => pageView(location.pathname))
      .catch(() => {
        /* analytics is best-effort */
      });
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
{@render children()}
