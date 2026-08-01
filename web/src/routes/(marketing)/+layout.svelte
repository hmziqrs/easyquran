<script lang="ts">
  import { Nav } from "$lib/components/nav";
  import { Footer } from "$lib/components/footer";
  import { Tweaks } from "$lib/components/tweaks";

  let { data, children } = $props();
</script>

<Nav />
<main id="main" tabindex="-1">{@render children()}</main>
<Footer owner={data.owner} year={data.year} />
<Tweaks />
