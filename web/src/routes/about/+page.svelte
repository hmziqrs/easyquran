<script lang="ts">
  import { Button, Section, Seo } from "$lib/components";
  import { SITE } from "$lib/config/site";
</script>

<Seo path="/about" />

<Section border={false} width="narrow">
  <p class="eyebrow">About</p>
  <h1 class="mt-4 text-xl">{SITE.name}</h1>
  <p class="mt-4 text-fg-2">
    Placeholder copy. Page content lives here; site-wide facts (name, URL, nav,
    metadata) come from <code class="mono text-fg-3">src/lib/config/site.ts</code>.
  </p>

  <div class="mt-10">
    <Button variant="ghost" href="/">← Home</Button>
  </div>
</Section>
