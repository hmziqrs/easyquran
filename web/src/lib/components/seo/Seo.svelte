<!--
  Seo — page head metadata. Takes only the route path; the title and
  description are looked up from PAGE_META. Renders one svelte:head block
  with the title, description, canonical, Open Graph and Twitter tags.
-->
<script lang="ts">
  import { SITE, PAGE_META } from "$lib/config/site";

  let {
    path,
  }: {
    path: string;
  } = $props();

  const meta = $derived(Object.values(PAGE_META).find((p) => p.path === path));
  const title = $derived(meta?.title ?? SITE.name);
  const description = $derived(meta?.description ?? SITE.tagline);
  const canonical = $derived(SITE.url + path);
</script>

<svelte:head>
  <title>{title}</title>
  <meta name="description" content={description} />
  <link rel="canonical" href={canonical} />

  <meta property="og:site_name" content={SITE.name} />
  <meta property="og:type" content="website" />
  <meta property="og:title" content={title} />
  <meta property="og:description" content={description} />
  <meta property="og:url" content={canonical} />

  <meta name="twitter:card" content="summary" />
  <meta name="twitter:title" content={title} />
  <meta name="twitter:description" content={description} />
</svelte:head>
