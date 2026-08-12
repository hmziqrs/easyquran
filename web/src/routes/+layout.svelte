<script lang="ts">
  import "./layout.css";
  import favicon from "$lib/assets/favicon.svg";
  import { onMount } from "svelte";
  import { afterNavigate, beforeNavigate } from "$app/navigation";
  import { updated } from "$app/state";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { consent } from "$lib/stores/consent.svelte";
  import { notifications } from "$lib/stores/notifications.svelte";
  import { update } from "$lib/offline/update.svelte";
  import { online } from "$lib/offline/online.svelte";
  import { offline } from "$lib/offline/offline-store.svelte";
  import { APP_READY } from "$lib/offline/messages";
  import { NotificationToast } from "$lib/components/notifications";
  import { GlobalSearch } from "$lib/components/search";
  import { DownloadBar, UpdateToast } from "$lib/components/status";
  import { SITE } from "$lib/config/site";
  import { startServiceWorker } from "$lib/boot/service-worker";
  import { startAnalytics } from "$lib/boot/analytics";
  import { startCrashReporting } from "$lib/boot/crash-reporting";
  import { startOfflineEngine } from "$lib/boot/offline-engine";
  import { deLocalizeUrl } from "$lib/paraglide/runtime";

  let { children } = $props();

  let offlineTeardown: (() => void) | null = null;
  let firstPaintComplete = false;
  let paintFrame = 0;
  let postPaintFrame = 0;
  const ensureOfflineEngine = (pathname: string): void => {
    const canonicalPath = deLocalizeUrl(new URL(pathname, location.origin)).pathname;
    if (canonicalPath !== "/app" && !canonicalPath.startsWith("/app/")) return;
    if (offlineTeardown) return;
    offlineTeardown = startOfflineEngine();
  };

  onMount(() => {
    consent.hydrate();
    prefs.hydrate();
    prefs.apply();
    notifications.hydrate();
    update.hydrate();
    online.hydrate();
    offline.hydrate();

    const cleanups = [startServiceWorker(), startAnalytics(), startCrashReporting()];

    const hadControllerAtBoot = Boolean(navigator.serviceWorker?.controller);
    const postAppReady = (): void => {
      const ctrl = navigator.serviceWorker?.controller;
      if (ctrl) ctrl.postMessage({ type: APP_READY });
    };
    postAppReady();
    const onControllerChange = (): void => {
      if (!hadControllerAtBoot) postAppReady();
    };
    navigator.serviceWorker?.addEventListener("controllerchange", onControllerChange);
    cleanups.push(() => navigator.serviceWorker?.removeEventListener("controllerchange", onControllerChange));

    paintFrame = requestAnimationFrame(() => {
      postPaintFrame = requestAnimationFrame(() => {
        firstPaintComplete = true;
        ensureOfflineEngine(location.pathname);
      });
    });

    return () => {
      cancelAnimationFrame(paintFrame);
      cancelAnimationFrame(postPaintFrame);
      for (const teardown of cleanups) teardown();
      update.dispose();
      online.dispose();
      offlineTeardown?.();
      offlineTeardown = null;
    };
  });

  beforeNavigate(({ willUnload, to }) => {
    if (updated.current && !willUnload && to?.url) location.href = to.url.href;
  });

  afterNavigate((navigation) => {
    if (navigation.type !== "enter") {
      void import("$lib/firebase/analytics")
        .then(({ pageView }) => pageView(location.pathname))
        .catch(() => {});
    }
    if (firstPaintComplete) ensureOfflineEngine(location.pathname);
  });

  const jsonLd = {
    "@context": "https://schema.org",
    "@graph": [
      {
        "@type": "WebSite",
        "@id": `${SITE.url}/#website`,
        name: SITE.name,
        url: SITE.url,
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
  const jsonLdText = JSON.stringify(jsonLd);
</script>

<svelte:head>
  <link rel="icon" type="image/png" sizes="32x32" href="/icons/icon-32.png" />
  <link rel="icon" type="image/png" sizes="16x16" href="/icons/icon-16.png" />
  <link rel="icon" type="image/svg+xml" href={favicon} />
  <link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png" />
  <link rel="manifest" href="/manifest.webmanifest" />
  <meta name="application-name" content={SITE.name} />
  <svelte:element this={"script"} type="application/ld+json">{jsonLdText}</svelte:element>
</svelte:head>

<a
  href="#main"
  class="sr-only focus:not-sr-only focus:absolute focus:left-3 focus:top-3 focus:z-[100] focus:rounded focus:bg-bg-1 focus:px-3 focus:py-2 focus:text-sm focus:text-fg focus:shadow-lg"
  >Skip to content</a
>
<NotificationToast />
<UpdateToast />
<DownloadBar />
<GlobalSearch />
{@render children()}
