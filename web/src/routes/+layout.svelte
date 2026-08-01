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

  let offlineTeardown: (() => void) | null = null;
  const ensureOfflineEngine = (pathname: string): void => {
    if (!pathname.startsWith("/app")) return;
    if (offlineTeardown) return;
    offlineTeardown = startOfflineEngine();
  };

  onMount(() => {
    consent.hydrate();
    prefs.hydrate();
    prefs.apply();
    notifications.hydrate();

    const cleanups = [startServiceWorker(), startAnalytics(), startCrashReporting()];

    ensureOfflineEngine(location.pathname);

    return () => {
      for (const teardown of cleanups) teardown();
      offlineTeardown?.();
      offlineTeardown = null;
    };
  });

  afterNavigate((navigation) => {
    if (navigation.type !== "enter") {
      void import("$lib/firebase/analytics")
        .then(({ pageView }) => pageView(location.pathname))
        .catch(() => {});
    }
    ensureOfflineEngine(location.pathname);
  });

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
  <!-- eslint-disable-next-line svelte/no-at-html-tags -->
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
