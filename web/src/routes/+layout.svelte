<script lang="ts">
  import "./layout.css";
  import favicon from "$lib/assets/favicon.svg";
  import { onMount } from "svelte";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { SITE } from "$lib/config/site";
  import { initAnalytics } from "$lib/firebase";

  let { children } = $props();

  // The inline <head> script in app.html already applied saved prefs before
  // paint; re-apply on mount so the reactive store and the DOM stay in sync.
  // `onMount` only runs in the browser, so this is also where analytics is
  // safe to start (gtag.js + cookies can't run during SSR).
  onMount(() => {
    prefs.apply();
    void initAnalytics();
  });

  // Site-level structured data. Built as a complete <script> string and emitted
  // via {@html}, because Svelte treats <script> tag bodies as raw text and will
  // not interpolate {expressions} inside them.
  const jsonLd = {
    "@context": "https://schema.org",
    "@graph": [
      {
        "@type": "WebSite",
        name: SITE.name,
        url: SITE.url,
      },
    ],
  };
  // Closing end-tag built from two pieces so its literal never appears in
  // source (the parser would end this code block on that sequence).
  const jsonLdScript =
    `<script type="application/ld+json">${JSON.stringify(jsonLd)}` + "<" + "/script>";
</script>

<svelte:head>
  <link rel="icon" href={favicon} />
  <!-- eslint-disable-next-line svelte/no-at-html-tags -- structured data, safe by construction -->
  {@html jsonLdScript}
</svelte:head>

<main>{@render children()}</main>
