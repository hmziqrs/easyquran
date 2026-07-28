<!--
  Marketing layout — the public site chrome: nav, footer and the appearance
  panel. Renders the <main> that the root layout's skip link targets, and that
  the .md / .txt text variants extract their content from.
-->
<script lang="ts">
  import { Nav } from "$lib/components/nav";
  import { Footer } from "$lib/components/footer";
  import { Tweaks } from "$lib/components/tweaks";

  let { children } = $props();
</script>

<Nav />
<main id="main" tabindex="-1">{@render children()}</main>
<Footer />
<Tweaks />
